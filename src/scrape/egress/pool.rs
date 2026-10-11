//! The proxy pool: rotation strategies, quarantine after repeated failures, and sticky egress per site.

#![expect(
    clippy::module_name_repetitions,
    reason = "ProxyPool is the public name fixed by the design plan"
)]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

use super::geo::CountryCode;
use super::route::{EgressError, EgressKey, EgressRef};
use crate::sb_cdp::sync::locked;
use crate::stealth::humanize::Rng;

/// Failures in a row that quarantine an egress, unless the pool says otherwise.
pub const DEFAULT_QUARANTINE_AFTER: u32 = 3;

/// Seconds a quarantined egress stays out of rotation, unless the pool says otherwise.
pub const DEFAULT_QUARANTINE_SECS: u64 = 600;

/// How the pool chooses among the egresses that are available.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationStrategy {
    /// Each pick moves on to the next available egress.
    RoundRobin,
    /// A choice drawn from a generator seeded with `seed`, so the same seed gives the same order.
    Random {
        /// The seed of the draw.
        seed: u64,
    },
    /// The available egress with the fewest failures in a row.
    LeastFailures,
    /// A site keeps the egress it was first given, while that egress is available. Other picks
    /// round-robin.
    StickyPerSite,
}

/// What a request through an egress came to.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EgressOutcome {
    /// The request completed.
    Success,
    /// The connection through the egress failed.
    ConnectError,
    /// The site answered with a challenge.
    Challenged,
    /// The site refused the egress outright.
    Blocked,
}

/// One egress and its health.
struct Slot {
    egress: EgressRef,
    failures: u32,
    /// The time, in seconds, before which the egress is out of rotation.
    quarantined_until: Option<u64>,
}

impl Slot {
    fn available(&self, now: u64) -> bool {
        self.quarantined_until.is_none_or(|until| now >= until)
    }
}

/// The mutable part of the pool, behind one lock.
struct State {
    slots: Vec<Slot>,
    cursor: usize,
    rng: Rng,
    sticky: BTreeMap<String, EgressKey>,
}

/// A rotating set of egresses, with failure counting and quarantine. Every decision takes the
/// current time in seconds as an argument, so the pool itself reads no clock.
pub struct ProxyPool {
    strategy: RotationStrategy,
    quarantine_after: u32,
    quarantine_secs: u64,
    state: Mutex<State>,
}

impl fmt::Debug for ProxyPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProxyPool")
            .field("strategy", &self.strategy)
            .field("quarantine_after", &self.quarantine_after)
            .field("quarantine_secs", &self.quarantine_secs)
            .finish_non_exhaustive()
    }
}

impl ProxyPool {
    /// A pool over `egresses`, with the default quarantine.
    #[must_use]
    pub fn new(egresses: Vec<EgressRef>, strategy: RotationStrategy) -> Self {
        let seed = match strategy {
            RotationStrategy::Random { seed } => seed,
            _ => 0,
        };
        let slots = egresses
            .into_iter()
            .map(|egress| Slot {
                egress,
                failures: 0,
                quarantined_until: None,
            })
            .collect();
        Self {
            strategy,
            quarantine_after: DEFAULT_QUARANTINE_AFTER,
            quarantine_secs: DEFAULT_QUARANTINE_SECS,
            state: Mutex::new(State {
                slots,
                cursor: 0,
                rng: Rng::new(seed),
                sticky: BTreeMap::new(),
            }),
        }
    }

    /// A pool built from `(spec, country)` pairs, each spec parsed the way [`EgressRef::parse`]
    /// parses it.
    ///
    /// # Errors
    ///
    /// Returns the first [`EgressError`] of the specs.
    pub fn with_geo_proxies<I, S>(items: I, strategy: RotationStrategy) -> Result<Self, EgressError>
    where
        I: IntoIterator<Item = (S, CountryCode)>,
        S: AsRef<str>,
    {
        let egresses = items
            .into_iter()
            .map(|(spec, country)| EgressRef::parse(spec.as_ref(), Some(country)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::new(egresses, strategy))
    }

    /// Sets how many failures in a row quarantine an egress, and for how many seconds.
    #[must_use]
    pub fn quarantine(mut self, after_failures: u32, for_secs: u64) -> Self {
        self.quarantine_after = after_failures;
        self.quarantine_secs = for_secs;
        self
    }

    /// The egress for a request to `site` at time `now`, in seconds. `None` when the pool is empty
    /// or every egress is quarantined.
    #[must_use]
    pub fn pick(&self, site: Option<&str>, now: u64) -> Option<EgressRef> {
        self.choose(|_| true, site, now)
    }

    /// Like [`ProxyPool::pick`], limited to egresses in `country`.
    #[must_use]
    pub fn pick_in(&self, country: CountryCode, site: Option<&str>, now: u64) -> Option<EgressRef> {
        self.choose(|key| key.country == Some(country), site, now)
    }

    /// Like [`ProxyPool::pick`], excluding `current`, so that a rotation really moves.
    #[must_use]
    pub fn pick_other(
        &self,
        current: &EgressKey,
        site: Option<&str>,
        now: u64,
    ) -> Option<EgressRef> {
        self.choose(|key| key != current, site, now)
    }

    /// Records what happened to a request through `egress` at time `now`. A success clears the
    /// failure count. A failure adds one, and once the count reaches the pool's limit the egress is
    /// quarantined and its count starts again.
    pub fn report(&self, egress: &EgressKey, outcome: EgressOutcome, now: u64) {
        let mut state = locked(&self.state);
        let Some(slot) = state
            .slots
            .iter_mut()
            .find(|slot| slot.egress.key() == egress)
        else {
            return;
        };
        match outcome {
            EgressOutcome::Success => slot.failures = 0,
            EgressOutcome::ConnectError | EgressOutcome::Challenged | EgressOutcome::Blocked => {
                slot.failures = slot.failures.saturating_add(1);
                if slot.failures >= self.quarantine_after {
                    slot.quarantined_until = Some(now.saturating_add(self.quarantine_secs));
                    slot.failures = 0;
                }
            }
        }
    }

    fn choose(
        &self,
        accept: impl Fn(&EgressKey) -> bool,
        site: Option<&str>,
        now: u64,
    ) -> Option<EgressRef> {
        let mut state = locked(&self.state);
        let eligible: Vec<usize> = state
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.available(now) && accept(slot.egress.key()))
            .map(|(index, _)| index)
            .collect();
        if eligible.is_empty() {
            return None;
        }
        let sticky = matches!(self.strategy, RotationStrategy::StickyPerSite)
            .then_some(site)
            .flatten();
        if let Some(site) = sticky {
            let kept = state.sticky.get(site).and_then(|key| {
                eligible
                    .iter()
                    .copied()
                    .find(|&index| state.slots[index].egress.key() == key)
            });
            if let Some(index) = kept {
                return Some(state.slots[index].egress.clone());
            }
        }
        let index = self.select(&mut state, &eligible);
        let chosen = state.slots[index].egress.clone();
        if let Some(site) = sticky {
            state.sticky.insert(site.to_owned(), chosen.key().clone());
        }
        Some(chosen)
    }

    /// Picks one of `eligible` (never empty) by the strategy.
    fn select(&self, state: &mut State, eligible: &[usize]) -> usize {
        match self.strategy {
            RotationStrategy::RoundRobin | RotationStrategy::StickyPerSite => {
                let position = state.cursor % eligible.len();
                state.cursor = state.cursor.wrapping_add(1);
                eligible[position]
            }
            RotationStrategy::Random { .. } => {
                let bound = u64::try_from(eligible.len()).unwrap_or(u64::MAX);
                let draw = usize::try_from(state.rng.next_u64() % bound).unwrap_or(0);
                eligible[draw]
            }
            RotationStrategy::LeastFailures => eligible
                .iter()
                .copied()
                .min_by_key(|&index| state.slots[index].failures)
                .unwrap_or(eligible[0]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn egress(spec: &str, country: Option<&str>) -> EgressRef {
        let country = country.map(|code| CountryCode::new(code).expect("valid code"));
        EgressRef::parse(spec, country).expect("valid spec")
    }

    fn three() -> Vec<EgressRef> {
        vec![
            egress("http://p1.example:8080", None),
            egress("http://p2.example:8080", None),
            egress("http://p3.example:8080", None),
        ]
    }

    fn host_of(egress: Option<EgressRef>) -> String {
        egress.map(|e| e.key().host.clone()).unwrap_or_default()
    }

    #[test]
    fn round_robin_cycles_through_every_egress() {
        let pool = ProxyPool::new(three(), RotationStrategy::RoundRobin);
        let hosts: Vec<String> = (0..6).map(|_| host_of(pool.pick(None, 0))).collect();
        assert_eq!(hosts, ["p1.example", "p2.example", "p3.example"].repeat(2));
    }

    #[test]
    fn an_empty_pool_has_nothing_to_pick() {
        let pool = ProxyPool::new(Vec::new(), RotationStrategy::RoundRobin);
        assert!(pool.pick(None, 0).is_none());
    }

    #[test]
    fn three_failures_quarantine_an_egress_until_its_time_is_up() {
        let pool = ProxyPool::new(
            vec![
                egress("http://p1.example:8080", None),
                egress("http://p2.example:8080", None),
            ],
            RotationStrategy::RoundRobin,
        )
        .quarantine(3, 600);
        let p1 = egress("http://p1.example:8080", None);
        for _ in 0..3 {
            pool.report(p1.key(), EgressOutcome::ConnectError, 100);
        }
        for _ in 0..4 {
            assert_eq!(host_of(pool.pick(None, 699)), "p2.example");
        }
        let released: Vec<String> = (0..2).map(|_| host_of(pool.pick(None, 700))).collect();
        assert!(released.contains(&"p1.example".to_owned()), "{released:?}");
    }

    #[test]
    fn a_success_clears_the_failure_count() {
        let pool = ProxyPool::new(
            vec![egress("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        );
        let p1 = egress("http://p1.example:8080", None);
        pool.report(p1.key(), EgressOutcome::Blocked, 0);
        pool.report(p1.key(), EgressOutcome::Blocked, 0);
        pool.report(p1.key(), EgressOutcome::Success, 0);
        pool.report(p1.key(), EgressOutcome::Blocked, 0);
        pool.report(p1.key(), EgressOutcome::Blocked, 0);
        assert!(
            pool.pick(None, 0).is_some(),
            "two failures after a success must not quarantine"
        );
    }

    #[test]
    fn least_failures_avoids_the_egress_that_failed() {
        let pool = ProxyPool::new(three(), RotationStrategy::LeastFailures);
        pool.report(
            egress("http://p1.example:8080", None).key(),
            EgressOutcome::Challenged,
            0,
        );
        for _ in 0..4 {
            assert_ne!(host_of(pool.pick(None, 0)), "p1.example");
        }
    }

    #[test]
    fn sticky_per_site_keeps_the_first_egress_for_that_site() {
        let pool = ProxyPool::new(three(), RotationStrategy::StickyPerSite);
        let first = host_of(pool.pick(Some("a.example"), 0));
        for _ in 0..5 {
            assert_eq!(host_of(pool.pick(Some("a.example"), 0)), first);
        }
    }

    #[test]
    fn a_sticky_egress_that_is_quarantined_gives_way() {
        let pool = ProxyPool::new(three(), RotationStrategy::StickyPerSite).quarantine(1, 600);
        let first = pool.pick(Some("a.example"), 0).expect("an egress");
        pool.report(first.key(), EgressOutcome::Blocked, 0);
        let next = host_of(pool.pick(Some("a.example"), 0));
        assert_ne!(next, first.key().host);
    }

    #[test]
    fn random_with_one_seed_gives_one_order() {
        let order = |seed| {
            let pool = ProxyPool::new(three(), RotationStrategy::Random { seed });
            (0..10)
                .map(|_| host_of(pool.pick(None, 0)))
                .collect::<Vec<_>>()
        };
        assert_eq!(order(11), order(11));
    }

    #[test]
    fn pick_other_never_returns_the_current_egress() {
        let pool = ProxyPool::new(three(), RotationStrategy::RoundRobin);
        let current = egress("http://p2.example:8080", None);
        for _ in 0..6 {
            assert_ne!(
                host_of(pool.pick_other(current.key(), None, 0)),
                "p2.example"
            );
        }
    }

    #[test]
    fn pick_in_only_returns_egresses_in_that_country() {
        let de = CountryCode::new("DE").expect("valid");
        let pool = ProxyPool::new(
            vec![
                egress("http://fr.example:8080", Some("FR")),
                egress("http://de.example:8080", Some("DE")),
            ],
            RotationStrategy::RoundRobin,
        );
        for _ in 0..4 {
            assert_eq!(host_of(pool.pick_in(de, None, 0)), "de.example");
        }
    }

    #[test]
    fn with_geo_proxies_refuses_a_bad_spec() {
        let de = CountryCode::new("DE").expect("valid");
        let result =
            ProxyPool::with_geo_proxies([("ftp://x.example:21", de)], RotationStrategy::RoundRobin);
        assert!(result.is_err());
    }
}
