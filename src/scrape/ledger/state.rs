//! Per-site state (successes, challenges, cooldowns) and the `StateStore` trait it is saved through.
//!
//! This module is pure: the types, the update rule and the in-memory store. The store that
//! writes a file is [`JsonStateStore`](crate::scrape::runtime::json_store::JsonStateStore).

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::sb_cdp::sync::locked;
use crate::scrape::challenge::detect::{ChallengeKind, Detection};
use crate::scrape::challenge::mitigation::Mode;
use crate::scrape::egress::route::EgressKey;

/// The version of the state file layout. A file with another version is refused, not guessed at.
pub const STATE_SCHEMA: u32 = 1;

/// How many sites a store keeps. A scrape can touch an unbounded number of sites, so a store
/// that is full forgets the site it saw longest ago to make room for a new one.
pub const MAX_STATE_SITES: usize = 10_000;

/// What is known about one site: how it has answered, and how it should be approached next.
/// `site` is the key; callers pass the registrable domain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainState {
    /// The site this state is about.
    pub site: String,
    /// Responses that were not challenges.
    #[serde(default)]
    pub successes: u64,
    /// Script challenges, interactive challenges and rate limits.
    #[serde(default)]
    pub challenges: u64,
    /// Outright blocks.
    #[serde(default)]
    pub blocks: u64,
    /// Challenges and blocks since the last response that was not one.
    #[serde(default)]
    pub consecutive_challenges: u32,
    /// The kind of the latest challenge or block.
    #[serde(default)]
    pub last_challenge: Option<ChallengeKind>,
    /// When the site last answered, in seconds since the Unix epoch.
    #[serde(default)]
    pub last_seen: u64,
    /// The site is to be left alone until this time, in seconds since the Unix epoch.
    #[serde(default)]
    pub cooldown_until: Option<u64>,
    /// How the site is best fetched. `Http` once a plain request has worked; `Browser` once one
    /// was challenged.
    #[serde(default)]
    pub preferred_mode: Option<Mode>,
    /// The egress the site last answered through. Never its credentials.
    #[serde(default)]
    pub last_egress: Option<EgressKey>,
}

impl DomainState {
    /// A site that has not been seen.
    #[must_use]
    pub fn new(site: impl Into<String>) -> Self {
        Self {
            site: site.into(),
            successes: 0,
            challenges: 0,
            blocks: 0,
            consecutive_challenges: 0,
            last_challenge: None,
            last_seen: 0,
            cooldown_until: None,
            preferred_mode: None,
            last_egress: None,
        }
    }

    /// Takes in one response, fetched in `mode` at time `now` (seconds since the Unix epoch).
    ///
    /// A response that is not a challenge counts as a success, ends the streak and lifts a
    /// cooldown; in [`Mode::Http`] it also makes plain requests the preferred way. A block
    /// counts in `blocks`, and a script challenge, interactive challenge or rate limit in
    /// `challenges`; each extends the streak and is remembered in `last_challenge`. A challenge
    /// or block met in [`Mode::Http`] makes the browser the preferred way; a rate limit does not,
    /// because a browser would be limited too.
    pub fn record(&mut self, detection: &Detection, mode: Mode, now: u64) {
        self.last_seen = now;
        match detection.kind {
            ChallengeKind::None => {
                self.successes = self.successes.saturating_add(1);
                self.consecutive_challenges = 0;
                self.cooldown_until = None;
                if mode == Mode::Http {
                    self.preferred_mode = Some(Mode::Http);
                }
            }
            kind => {
                if kind == ChallengeKind::Blocked {
                    self.blocks = self.blocks.saturating_add(1);
                } else {
                    self.challenges = self.challenges.saturating_add(1);
                }
                self.consecutive_challenges = self.consecutive_challenges.saturating_add(1);
                self.last_challenge = Some(kind);
                let rate_limited = matches!(kind, ChallengeKind::RateLimited { .. });
                if mode == Mode::Http && !rate_limited {
                    self.preferred_mode = Some(Mode::Browser);
                }
            }
        }
    }

    /// Leaves the site alone until `until`. A cooldown that is already longer stays.
    pub fn start_cooldown(&mut self, until: u64) {
        self.cooldown_until = Some(
            self.cooldown_until
                .map_or(until, |current| current.max(until)),
        );
    }

    /// Whether the site is to be left alone at time `now`.
    #[must_use]
    pub fn in_cooldown(&self, now: u64) -> bool {
        self.cooldown_until.is_some_and(|until| now < until)
    }
}

/// Why a state store failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StateError {
    /// The file could not be read or written.
    #[error("cannot read or write the state file {}: {source}", .path.display())]
    Io {
        /// The state file.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: io::Error,
    },
    /// The file is not valid state. It is left as it is, never reset.
    #[error("the state file {} is not valid: {source}", .path.display())]
    Corrupt {
        /// The state file.
        path: PathBuf,
        /// Where the parse failed.
        #[source]
        source: serde_json::Error,
    },
    /// The file was written with a layout this version does not know.
    #[error("the state file has schema {0}, which this version cannot read")]
    Schema(u32),
    /// The file is larger than a state file is allowed to be.
    #[error("the state file {} is larger than {limit} bytes", .path.display())]
    TooLarge {
        /// The state file.
        path: PathBuf,
        /// The most it may hold.
        limit: u64,
    },
}

/// Where per-site state lives between requests.
#[async_trait]
pub trait StateStore: Send + Sync {
    /// The state of `site`, when there is any.
    ///
    /// # Errors
    ///
    /// Returns [`StateError`] when the store cannot be read.
    async fn get(&self, site: &str) -> Result<Option<DomainState>, StateError>;

    /// Saves `state`, replacing what was there for its site.
    ///
    /// # Errors
    ///
    /// Returns [`StateError`] when the store cannot be written. Nothing is changed then.
    async fn put(&self, state: DomainState) -> Result<(), StateError>;

    /// Forgets `site`. Forgetting a site that is not there is not an error.
    ///
    /// # Errors
    ///
    /// Returns [`StateError`] when the store cannot be written. Nothing is changed then.
    async fn remove(&self, site: &str) -> Result<(), StateError>;

    /// Every site that has state, in order.
    ///
    /// # Errors
    ///
    /// Returns [`StateError`] when the store cannot be read.
    async fn sites(&self) -> Result<Vec<String>, StateError>;
}

/// Puts `state` in `sites`, first dropping the site seen longest ago (the first by name among
/// equals) while a new site would take the table past `cap`.
pub(crate) fn insert_bounded(
    sites: &mut BTreeMap<String, DomainState>,
    state: DomainState,
    cap: usize,
) {
    if !sites.contains_key(&state.site) {
        while sites.len() >= cap.max(1) {
            let oldest = sites
                .values()
                .min_by_key(|known| (known.last_seen, &known.site))
                .map(|known| known.site.clone());
            match oldest {
                Some(site) => sites.remove(&site),
                None => break,
            };
        }
    }
    sites.insert(state.site.clone(), state);
}

/// A store that keeps everything in memory, behind an `Arc`, so that clones share it.
#[derive(Clone, Debug, Default)]
pub struct InMemoryStateStore {
    sites: Arc<Mutex<BTreeMap<String, DomainState>>>,
}

impl InMemoryStateStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn get(&self, site: &str) -> Result<Option<DomainState>, StateError> {
        Ok(locked(&self.sites).get(site).cloned())
    }

    async fn put(&self, state: DomainState) -> Result<(), StateError> {
        insert_bounded(&mut locked(&self.sites), state, MAX_STATE_SITES);
        Ok(())
    }

    async fn remove(&self, site: &str) -> Result<(), StateError> {
        locked(&self.sites).remove(site);
        Ok(())
    }

    async fn sites(&self) -> Result<Vec<String>, StateError> {
        Ok(locked(&self.sites).keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrape::challenge::detect::Evidence;
    use futures_util::FutureExt;

    fn seen(kind: ChallengeKind) -> Detection {
        Detection {
            kind,
            evidence: Evidence::default(),
        }
    }

    fn ok() -> Detection {
        seen(ChallengeKind::None)
    }

    fn state(site: &str, last_seen: u64) -> DomainState {
        DomainState {
            last_seen,
            ..DomainState::new(site)
        }
    }

    /// The stores await nothing, so one poll finishes each call and no runtime is needed.
    fn now<T>(future: impl std::future::Future<Output = T>) -> T {
        future.now_or_never().expect("finishes in one poll")
    }

    #[test]
    fn a_new_site_has_seen_nothing() {
        let fresh = DomainState::new("example.test");
        assert_eq!(fresh.site, "example.test");
        assert_eq!(
            (
                fresh.successes,
                fresh.challenges,
                fresh.blocks,
                fresh.consecutive_challenges
            ),
            (0, 0, 0, 0)
        );
        assert!(fresh.last_challenge.is_none() && fresh.preferred_mode.is_none());
        assert!(!fresh.in_cooldown(0));
    }

    #[test]
    fn a_success_counts_and_ends_the_streak() {
        let mut site = DomainState::new("example.test");
        site.record(&seen(ChallengeKind::JsChallenge), Mode::Browser, 10);
        site.record(&seen(ChallengeKind::JsChallenge), Mode::Browser, 11);
        assert_eq!(site.consecutive_challenges, 2);
        site.record(&ok(), Mode::Browser, 12);
        assert_eq!(site.successes, 1);
        assert_eq!(site.challenges, 2);
        assert_eq!(site.consecutive_challenges, 0);
        assert_eq!(site.last_seen, 12);
        assert_eq!(
            site.last_challenge,
            Some(ChallengeKind::JsChallenge),
            "the history of the last challenge stays"
        );
    }

    #[test]
    fn blocks_and_challenges_are_counted_apart() {
        let mut site = DomainState::new("example.test");
        site.record(&seen(ChallengeKind::Blocked), Mode::Http, 1);
        site.record(&seen(ChallengeKind::Interactive), Mode::Browser, 2);
        site.record(
            &seen(ChallengeKind::RateLimited {
                retry_after_secs: Some(5),
            }),
            Mode::Http,
            3,
        );
        assert_eq!((site.blocks, site.challenges, site.successes), (1, 2, 0));
        assert_eq!(site.consecutive_challenges, 3);
        assert_eq!(
            site.last_challenge,
            Some(ChallengeKind::RateLimited {
                retry_after_secs: Some(5)
            })
        );
    }

    #[test]
    fn the_preferred_mode_follows_what_worked_over_http() {
        let mut site = DomainState::new("example.test");
        site.record(&ok(), Mode::Browser, 1);
        assert_eq!(
            site.preferred_mode, None,
            "a browser success says nothing about plain requests"
        );
        site.record(&ok(), Mode::Http, 2);
        assert_eq!(site.preferred_mode, Some(Mode::Http));
        site.record(&seen(ChallengeKind::JsChallenge), Mode::Http, 3);
        assert_eq!(site.preferred_mode, Some(Mode::Browser));
        site.record(&ok(), Mode::Browser, 4);
        assert_eq!(
            site.preferred_mode,
            Some(Mode::Browser),
            "a browser success does not undo it"
        );
        site.record(&seen(ChallengeKind::Interactive), Mode::Browser, 5);
        assert_eq!(site.preferred_mode, Some(Mode::Browser));
    }

    #[test]
    fn a_rate_limit_does_not_send_the_site_to_the_browser() {
        let mut site = DomainState::new("example.test");
        site.record(&ok(), Mode::Http, 1);
        site.record(
            &seen(ChallengeKind::RateLimited {
                retry_after_secs: None,
            }),
            Mode::Http,
            2,
        );
        assert_eq!(site.preferred_mode, Some(Mode::Http));
        site.record(&seen(ChallengeKind::Blocked), Mode::Http, 3);
        assert_eq!(site.preferred_mode, Some(Mode::Browser));
    }

    #[test]
    fn counters_saturate_instead_of_wrapping() {
        let mut site = DomainState::new("example.test");
        site.successes = u64::MAX;
        site.challenges = u64::MAX;
        site.blocks = u64::MAX;
        site.consecutive_challenges = u32::MAX;
        site.record(&ok(), Mode::Http, 1);
        assert_eq!(site.successes, u64::MAX);
        site.consecutive_challenges = u32::MAX;
        site.record(&seen(ChallengeKind::JsChallenge), Mode::Http, 2);
        site.record(&seen(ChallengeKind::Blocked), Mode::Http, 3);
        assert_eq!((site.challenges, site.blocks), (u64::MAX, u64::MAX));
        assert_eq!(site.consecutive_challenges, u32::MAX);
    }

    #[test]
    fn a_cooldown_holds_until_its_time_and_never_shortens() {
        let mut site = DomainState::new("example.test");
        site.start_cooldown(100);
        assert!(site.in_cooldown(0) && site.in_cooldown(99));
        assert!(!site.in_cooldown(100) && !site.in_cooldown(101));
        site.start_cooldown(50);
        assert_eq!(
            site.cooldown_until,
            Some(100),
            "a shorter cooldown is ignored"
        );
        site.start_cooldown(200);
        assert_eq!(site.cooldown_until, Some(200));
    }

    #[test]
    fn a_success_lifts_a_cooldown() {
        let mut site = DomainState::new("example.test");
        site.start_cooldown(1_000);
        site.record(&seen(ChallengeKind::Blocked), Mode::Http, 10);
        assert!(site.in_cooldown(11), "a block does not lift it");
        site.record(&ok(), Mode::Http, 20);
        assert!(!site.in_cooldown(21));
    }

    #[test]
    fn state_round_trips_and_an_older_file_still_loads() {
        let mut site = DomainState::new("example.test");
        site.record(&seen(ChallengeKind::Blocked), Mode::Http, 5);
        site.start_cooldown(900);
        site.last_egress = Some(EgressKey {
            scheme: "http".to_owned(),
            host: "p1.example".to_owned(),
            port: 8080,
            country: None,
        });
        let json = serde_json::to_string(&site).expect("serializes");
        assert_eq!(
            serde_json::from_str::<DomainState>(&json).expect("parses"),
            site
        );
        let minimal: DomainState =
            serde_json::from_str(r#"{"site":"old.test","unknown_future_field":[1,2]}"#)
                .expect("a file with only the site loads, extra fields are ignored");
        assert_eq!(minimal, DomainState::new("old.test"));
        assert!(serde_json::from_str::<DomainState>(r#"{"successes":1}"#).is_err());
    }

    #[test]
    fn the_state_holds_a_route_but_no_credential() {
        let egress = crate::scrape::egress::route::EgressRef::parse(
            "http://alice:s3cret@p1.example:8080",
            None,
        )
        .expect("valid spec");
        let mut site = DomainState::new("example.test");
        site.last_egress = Some(egress.key().clone());
        let shown = format!(
            "{site:?} {}",
            serde_json::to_string(&site).expect("serializes")
        );
        assert!(shown.contains("p1.example"), "{shown}");
        assert!(
            !shown.contains("s3cret") && !shown.contains("alice"),
            "{shown}"
        );
    }

    #[test]
    fn the_in_memory_store_keeps_replaces_and_forgets() {
        let store = InMemoryStateStore::new();
        assert_eq!(now(store.get("a.test")).expect("reads"), None);
        now(store.put(state("b.test", 1))).expect("writes");
        now(store.put(state("a.test", 2))).expect("writes");
        assert_eq!(now(store.sites()).expect("reads"), ["a.test", "b.test"]);
        let mut changed = state("a.test", 3);
        changed.successes = 9;
        now(store.put(changed.clone())).expect("writes");
        assert_eq!(now(store.get("a.test")).expect("reads"), Some(changed));
        now(store.remove("a.test")).expect("removes");
        now(store.remove("never.test")).expect("forgetting nothing is fine");
        assert_eq!(now(store.sites()).expect("reads"), ["b.test"]);
    }

    #[test]
    fn clones_of_the_in_memory_store_share_it() {
        let store = InMemoryStateStore::new();
        let other = store.clone();
        now(store.put(state("a.test", 1))).expect("writes");
        assert_eq!(now(other.sites()).expect("reads"), ["a.test"]);
    }

    #[test]
    fn a_full_table_forgets_the_site_seen_longest_ago() {
        let mut sites = BTreeMap::new();
        for (name, seen) in [("c.test", 5), ("a.test", 9), ("b.test", 1)] {
            insert_bounded(&mut sites, state(name, seen), 3);
        }
        insert_bounded(&mut sites, state("d.test", 7), 3);
        assert_eq!(
            sites.keys().collect::<Vec<_>>(),
            ["a.test", "c.test", "d.test"]
        );
        insert_bounded(&mut sites, state("a.test", 100), 3);
        assert_eq!(sites.len(), 3, "an update of a known site evicts nothing");
        assert_eq!(sites["a.test"].last_seen, 100);
    }

    #[test]
    fn equal_ages_are_broken_by_name() {
        let mut sites = BTreeMap::new();
        for name in ["b.test", "a.test", "c.test"] {
            insert_bounded(&mut sites, state(name, 4), 3);
        }
        insert_bounded(&mut sites, state("d.test", 4), 3);
        assert_eq!(
            sites.keys().collect::<Vec<_>>(),
            ["b.test", "c.test", "d.test"]
        );
    }

    #[test]
    fn a_table_over_its_cap_shrinks_back_and_a_cap_of_zero_keeps_one() {
        let mut sites: BTreeMap<String, DomainState> = (0..10)
            .map(|i| (format!("s{i}.test"), state(&format!("s{i}.test"), i)))
            .collect();
        insert_bounded(&mut sites, state("new.test", 99), 4);
        assert_eq!(sites.len(), 4);
        assert!(sites.contains_key("new.test"));
        insert_bounded(&mut sites, state("last.test", 100), 0);
        assert_eq!(sites.len(), 1);
        assert!(sites.contains_key("last.test"));
    }

    #[test]
    fn the_in_memory_store_is_bounded() {
        let store = InMemoryStateStore::new();
        for i in 0..MAX_STATE_SITES + 20 {
            now(store.put(state(&format!("s{i}.test"), u64::try_from(i).unwrap_or(0))))
                .expect("writes");
        }
        assert_eq!(now(store.sites()).expect("reads").len(), MAX_STATE_SITES);
        assert_eq!(
            now(store.get("s0.test")).expect("reads"),
            None,
            "the oldest went first"
        );
        assert!(now(store.get(&format!("s{}.test", MAX_STATE_SITES + 19)))
            .expect("reads")
            .is_some());
    }

    #[test]
    fn an_error_names_its_file() {
        let io = StateError::Io {
            path: PathBuf::from("/tmp/state.json"),
            source: io::Error::other("disk on fire"),
        };
        let shown = io.to_string();
        assert!(
            shown.contains("/tmp/state.json") && shown.contains("disk on fire"),
            "{shown}"
        );
        assert!(StateError::Schema(7).to_string().contains('7'));
        let big = StateError::TooLarge {
            path: PathBuf::from("/tmp/state.json"),
            limit: 10,
        };
        assert!(big.to_string().contains("10"));
    }
}
