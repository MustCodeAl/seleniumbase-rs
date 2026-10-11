//! The proxy pool: rotation strategies, quarantine after repeated failures, and sticky egress per site.

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

/// How many sites [`RotationStrategy::StickyPerSite`] remembers. A scrape can touch an unbounded
/// number of sites, so past this the table drops its alphabetically first entry for each new
/// site: that site is simply given a fresh egress the next time.
pub const MAX_STICKY_SITES: usize = 4096;

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
    /// The available egress with the fewest failures in a row. Egresses that tie take turns.
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
    /// Where the next round-robin pick starts looking. It is a position in `slots`, so the order
    /// holds when some egresses are out of rotation.
    cursor: usize,
    rng: Rng,
    sticky: BTreeMap<String, EgressKey>,
}

impl State {
    /// Takes the first slot at or after the cursor that is in `eligible` (ascending positions)
    /// and passes `keep`, then moves the cursor past it.
    fn next_from_cursor(&mut self, eligible: &[usize], keep: impl Fn(&Slot) -> bool) -> usize {
        let count = self.slots.len();
        let start = self.cursor % count.max(1);
        let chosen = (0..count)
            .map(|offset| (start + offset) % count)
            .find(|position| {
                eligible.binary_search(position).is_ok() && keep(&self.slots[*position])
            })
            .unwrap_or(eligible[0]);
        self.cursor = (chosen + 1) % count;
        chosen
    }

    /// Remembers that `site` uses `key`, forgetting another site when the table is full.
    fn remember(&mut self, site: &str, key: &EgressKey) {
        if !self.sticky.contains_key(site) && self.sticky.len() >= MAX_STICKY_SITES {
            self.sticky.pop_first();
        }
        self.sticky.insert(site.to_owned(), key.clone());
    }
}

/// A rotating set of egresses, with failure counting and quarantine. Every decision takes the
/// current time in seconds as an argument, so the pool itself reads no clock. It is `Send` and
/// `Sync`: share it behind an `Arc`.
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

    /// Sets how many failures in a row quarantine an egress, and for how many seconds. A limit
    /// of zero is read as one: the first failure quarantines.
    #[must_use]
    pub fn quarantine(mut self, after_failures: u32, for_secs: u64) -> Self {
        self.quarantine_after = after_failures.max(1);
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

    /// Like [`ProxyPool::pick`], excluding `current`, so that a rotation really moves. With
    /// [`RotationStrategy::StickyPerSite`] the site then sticks to the new egress.
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
    /// quarantined and its count starts again. An egress the pool does not hold is ignored.
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
            .map(|(position, _)| position)
            .collect();
        if eligible.is_empty() {
            return None;
        }
        let sticky_site = site.filter(|_| self.strategy == RotationStrategy::StickyPerSite);
        if let Some(kept) = sticky_site.and_then(|site| {
            let key = state.sticky.get(site)?;
            eligible
                .iter()
                .copied()
                .find(|&position| state.slots[position].egress.key() == key)
        }) {
            return Some(state.slots[kept].egress.clone());
        }
        let position = self.select(&mut state, &eligible);
        let chosen = state.slots[position].egress.clone();
        if let Some(site) = sticky_site {
            state.remember(site, chosen.key());
        }
        Some(chosen)
    }

    /// Picks one of `eligible` (ascending and never empty) by the strategy.
    fn select(&self, state: &mut State, eligible: &[usize]) -> usize {
        match self.strategy {
            RotationStrategy::Random { .. } => {
                let bound = u64::try_from(eligible.len()).unwrap_or(u64::MAX);
                let draw = usize::try_from(state.rng.next_u64() % bound).unwrap_or(0);
                eligible[draw]
            }
            RotationStrategy::LeastFailures => {
                let fewest = eligible
                    .iter()
                    .map(|&position| state.slots[position].failures)
                    .min()
                    .unwrap_or(0);
                state.next_from_cursor(eligible, |slot| slot.failures == fewest)
            }
            RotationStrategy::RoundRobin | RotationStrategy::StickyPerSite => {
                state.next_from_cursor(eligible, |_| true)
            }
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

    fn key(host: &str) -> EgressKey {
        egress(&format!("http://{host}:8080"), None).key().clone()
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

    /// The hosts of `count` picks in a row at time `now`.
    fn hosts(pool: &ProxyPool, count: usize, now: u64) -> Vec<String> {
        (0..count).map(|_| host_of(pool.pick(None, now))).collect()
    }

    #[test]
    fn the_pool_can_be_shared_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ProxyPool>();
    }

    #[test]
    fn round_robin_cycles_through_every_egress() {
        let pool = ProxyPool::new(three(), RotationStrategy::RoundRobin);
        assert_eq!(
            hosts(&pool, 6, 0),
            ["p1.example", "p2.example", "p3.example"].repeat(2)
        );
    }

    #[test]
    fn an_empty_pool_has_nothing_to_pick() {
        let pool = ProxyPool::new(Vec::new(), RotationStrategy::RoundRobin);
        let de = CountryCode::new("DE").expect("valid");
        assert!(pool.pick(None, 0).is_none());
        assert!(pool.pick_in(de, None, 0).is_none());
        assert!(pool.pick_other(&key("p1.example"), None, 0).is_none());
    }

    #[test]
    fn round_robin_skips_a_quarantined_egress_without_repeating_another() {
        let pool = ProxyPool::new(three(), RotationStrategy::RoundRobin).quarantine(1, 600);
        pool.report(&key("p2.example"), EgressOutcome::ConnectError, 0);
        assert_eq!(
            hosts(&pool, 6, 1),
            ["p1.example", "p3.example"].repeat(3),
            "the cycle holds its order around the gap"
        );
    }

    #[test]
    fn a_released_egress_rejoins_the_cycle() {
        let pool = ProxyPool::new(three(), RotationStrategy::RoundRobin).quarantine(1, 600);
        pool.report(&key("p2.example"), EgressOutcome::ConnectError, 0);
        let _ = hosts(&pool, 2, 1);
        let mut after = hosts(&pool, 3, 600);
        after.sort();
        assert_eq!(after, ["p1.example", "p2.example", "p3.example"]);
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
        for _ in 0..3 {
            pool.report(&key("p1.example"), EgressOutcome::ConnectError, 100);
        }
        assert_eq!(hosts(&pool, 4, 699), ["p2.example"; 4]);
        let released = hosts(&pool, 2, 700);
        assert!(released.contains(&"p1.example".to_owned()), "{released:?}");
    }

    #[test]
    fn the_default_quarantine_is_three_failures_for_ten_minutes() {
        let pool = ProxyPool::new(
            vec![egress("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        );
        for _ in 0..DEFAULT_QUARANTINE_AFTER {
            pool.report(&key("p1.example"), EgressOutcome::Blocked, 10);
        }
        assert!(pool.pick(None, 10 + DEFAULT_QUARANTINE_SECS - 1).is_none());
        assert!(pool.pick(None, 10 + DEFAULT_QUARANTINE_SECS).is_some());
    }

    #[test]
    fn a_released_egress_starts_again_with_no_failures() {
        let pool = ProxyPool::new(
            vec![egress("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        )
        .quarantine(2, 10);
        for _ in 0..2 {
            pool.report(&key("p1.example"), EgressOutcome::ConnectError, 0);
        }
        assert!(pool.pick(None, 9).is_none());
        pool.report(&key("p1.example"), EgressOutcome::ConnectError, 10);
        assert!(
            pool.pick(None, 11).is_some(),
            "one failure after release is below the limit of two"
        );
    }

    #[test]
    fn a_limit_of_zero_means_the_first_failure() {
        let pool = ProxyPool::new(
            vec![egress("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        )
        .quarantine(0, 60);
        assert!(pool.pick(None, 0).is_some());
        pool.report(&key("p1.example"), EgressOutcome::ConnectError, 0);
        assert!(pool.pick(None, 1).is_none());
    }

    #[test]
    fn every_failing_outcome_counts_and_a_success_clears_the_count() {
        for failing in [
            EgressOutcome::ConnectError,
            EgressOutcome::Challenged,
            EgressOutcome::Blocked,
        ] {
            let pool = ProxyPool::new(
                vec![egress("http://p1.example:8080", None)],
                RotationStrategy::RoundRobin,
            );
            let p1 = key("p1.example");
            pool.report(&p1, failing, 0);
            pool.report(&p1, failing, 0);
            pool.report(&p1, EgressOutcome::Success, 0);
            pool.report(&p1, failing, 0);
            pool.report(&p1, failing, 0);
            assert!(
                pool.pick(None, 0).is_some(),
                "two failures after a success must not quarantine ({failing:?})"
            );
            pool.report(&p1, failing, 0);
            assert!(pool.pick(None, 0).is_none(), "the third one does");
        }
    }

    #[test]
    fn a_report_for_an_egress_the_pool_does_not_hold_changes_nothing() {
        let pool = ProxyPool::new(three(), RotationStrategy::RoundRobin).quarantine(1, 600);
        pool.report(&key("elsewhere.example"), EgressOutcome::Blocked, 0);
        assert_eq!(
            hosts(&pool, 3, 0),
            ["p1.example", "p2.example", "p3.example"]
        );
    }

    #[test]
    fn least_failures_avoids_the_egress_that_failed() {
        let pool = ProxyPool::new(three(), RotationStrategy::LeastFailures);
        pool.report(&key("p1.example"), EgressOutcome::Challenged, 0);
        for host in hosts(&pool, 4, 0) {
            assert_ne!(host, "p1.example");
        }
    }

    #[test]
    fn least_failures_rotates_among_equals() {
        let pool = ProxyPool::new(three(), RotationStrategy::LeastFailures);
        assert_eq!(
            hosts(&pool, 6, 0),
            ["p1.example", "p2.example", "p3.example"].repeat(2)
        );
    }

    #[test]
    fn least_failures_follows_the_healthiest_egress() {
        let pool = ProxyPool::new(three(), RotationStrategy::LeastFailures);
        pool.report(&key("p1.example"), EgressOutcome::ConnectError, 0);
        pool.report(&key("p1.example"), EgressOutcome::ConnectError, 0);
        pool.report(&key("p2.example"), EgressOutcome::ConnectError, 0);
        assert_eq!(hosts(&pool, 3, 0), ["p3.example"; 3]);
        pool.report(&key("p3.example"), EgressOutcome::ConnectError, 0);
        assert_eq!(
            hosts(&pool, 4, 0),
            ["p2.example", "p3.example", "p2.example", "p3.example"]
        );
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
    fn sticky_sites_are_independent_and_new_ones_move_on() {
        let pool = ProxyPool::new(three(), RotationStrategy::StickyPerSite);
        let a = host_of(pool.pick(Some("a.example"), 0));
        let b = host_of(pool.pick(Some("b.example"), 0));
        let c = host_of(pool.pick(Some("c.example"), 0));
        assert_eq!(
            [a.as_str(), b.as_str(), c.as_str()],
            ["p1.example", "p2.example", "p3.example"]
        );
        assert_eq!(host_of(pool.pick(Some("a.example"), 0)), a);
        assert_eq!(host_of(pool.pick(Some("b.example"), 0)), b);
    }

    #[test]
    fn sticky_without_a_site_rotates_like_round_robin() {
        let pool = ProxyPool::new(three(), RotationStrategy::StickyPerSite);
        assert_eq!(
            hosts(&pool, 3, 0),
            ["p1.example", "p2.example", "p3.example"]
        );
    }

    #[test]
    fn a_sticky_egress_that_is_quarantined_gives_way() {
        let pool = ProxyPool::new(three(), RotationStrategy::StickyPerSite).quarantine(1, 600);
        let first = pool.pick(Some("a.example"), 0).expect("an egress");
        pool.report(first.key(), EgressOutcome::Blocked, 0);
        let next = host_of(pool.pick(Some("a.example"), 0));
        assert_ne!(next, first.key().host);
        assert_eq!(
            host_of(pool.pick(Some("a.example"), 0)),
            next,
            "the site now sticks to the replacement"
        );
    }

    #[test]
    fn the_sticky_table_stays_bounded() {
        let pool = ProxyPool::new(three(), RotationStrategy::StickyPerSite);
        for index in 0..MAX_STICKY_SITES + 25 {
            let site = format!("site{index}.example");
            assert!(pool.pick(Some(&site), 0).is_some());
        }
        assert_eq!(locked(&pool.state).sticky.len(), MAX_STICKY_SITES);
    }

    #[test]
    fn random_with_one_seed_gives_one_order() {
        let order = |seed| {
            let pool = ProxyPool::new(three(), RotationStrategy::Random { seed });
            hosts(&pool, 10, 0)
        };
        assert_eq!(order(11), order(11));
    }

    #[test]
    fn random_uses_every_egress_and_never_a_quarantined_one() {
        let pool = ProxyPool::new(three(), RotationStrategy::Random { seed: 3 });
        let seen = hosts(&pool, 40, 0);
        for host in ["p1.example", "p2.example", "p3.example"] {
            assert!(seen.iter().any(|h| h == host), "{host} never drawn");
        }
        let pool = ProxyPool::new(three(), RotationStrategy::Random { seed: 3 }).quarantine(1, 600);
        pool.report(&key("p2.example"), EgressOutcome::Blocked, 0);
        assert!(!hosts(&pool, 60, 1).contains(&"p2.example".to_owned()));
    }

    #[test]
    fn pick_other_never_returns_the_current_egress() {
        let pool = ProxyPool::new(three(), RotationStrategy::RoundRobin);
        let current = key("p2.example");
        for _ in 0..6 {
            assert_ne!(host_of(pool.pick_other(&current, None, 0)), "p2.example");
        }
    }

    #[test]
    fn pick_other_with_nothing_else_left_is_none() {
        let pool = ProxyPool::new(
            vec![egress("http://p1.example:8080", None)],
            RotationStrategy::RoundRobin,
        );
        assert!(pool.pick_other(&key("p1.example"), None, 0).is_none());
    }

    #[test]
    fn pick_other_moves_a_sticky_site_to_the_new_egress() {
        let pool = ProxyPool::new(three(), RotationStrategy::StickyPerSite);
        let first = pool.pick(Some("a.example"), 0).expect("an egress");
        let moved = pool
            .pick_other(first.key(), Some("a.example"), 0)
            .expect("another");
        assert_ne!(moved.key(), first.key());
        assert_eq!(
            pool.pick(Some("a.example"), 0).expect("sticky").key(),
            moved.key()
        );
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
        let jp = CountryCode::new("JP").expect("valid");
        assert!(pool.pick_in(jp, None, 0).is_none());
    }

    #[test]
    fn pick_in_finds_nothing_when_the_whole_country_is_quarantined() {
        let de = CountryCode::new("DE").expect("valid");
        let pool = ProxyPool::new(
            vec![
                egress("http://fr.example:8080", Some("FR")),
                egress("http://de.example:8080", Some("DE")),
            ],
            RotationStrategy::RoundRobin,
        )
        .quarantine(1, 600);
        pool.report(
            &key_with_country("de.example", "DE"),
            EgressOutcome::Blocked,
            0,
        );
        assert!(pool.pick_in(de, None, 1).is_none());
        assert!(
            pool.pick(None, 1).is_some(),
            "the other country still serves"
        );
    }

    fn key_with_country(host: &str, country: &str) -> EgressKey {
        egress(&format!("http://{host}:8080"), Some(country))
            .key()
            .clone()
    }

    #[test]
    fn with_geo_proxies_sets_the_country_on_every_key() {
        let de = CountryCode::new("DE").expect("valid");
        let fr = CountryCode::new("FR").expect("valid");
        let pool = ProxyPool::with_geo_proxies(
            [("de.example:8080", de), ("http://fr.example:3128", fr)],
            RotationStrategy::RoundRobin,
        )
        .expect("valid specs");
        assert_eq!(pool.pick_in(fr, None, 0).expect("French").key().port, 3128);
        assert_eq!(
            pool.pick_in(de, None, 0).expect("German").key().country,
            Some(de)
        );
    }

    #[test]
    fn with_geo_proxies_refuses_a_bad_spec() {
        let de = CountryCode::new("DE").expect("valid");
        let result =
            ProxyPool::with_geo_proxies([("ftp://x.example:21", de)], RotationStrategy::RoundRobin);
        assert!(result.is_err());
    }

    #[test]
    fn the_pool_and_its_picks_never_print_a_credential() {
        let pool = ProxyPool::with_geo_proxies(
            [(
                "http://alice:s3cret@p1.example:8080",
                CountryCode::new("DE").expect("valid"),
            )],
            RotationStrategy::StickyPerSite,
        )
        .expect("valid spec");
        let picked = pool.pick(Some("a.example"), 0).expect("an egress");
        let shown = format!("{pool:?} {picked:?} {picked} {:?}", picked.key());
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
        assert!(
            picked.proxy().has_credentials(),
            "the credentials are still there to use"
        );
    }
}
