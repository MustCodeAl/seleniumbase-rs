//! The scripted world the session tests run in: the shop, a launcher of scripted browsers, a clock
//! the test moves, and the sinks that record what the session did.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use seleniumbase_rs::impersonate::stealth_session::{StealthSession, StealthSessionBuilder};
use seleniumbase_rs::scrape::challenge::mitigation::Mode;
use seleniumbase_rs::scrape::egress::geo::CountryCode;
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, ScraperEvent};
use seleniumbase_rs::scrape::ledger::state::InMemoryStateStore;
use seleniumbase_rs::scrape::passport::policy::Transition;
use seleniumbase_rs::stealth::fingerprint::{Fingerprint, OsType};
use seleniumbase_rs::wire::{emulation, BrowserKind};

use crate::browser::ScriptedLauncher;
use crate::site::ChallengeSite;

/// The seed of the first profile in the tests.
pub const SEED: u64 = 7;

/// How long a solve is waited for in a test that expects it to fail.
pub const BRIEF_SOLVE: Duration = Duration::from_millis(80);

/// How long a solve is waited for in a test that expects it to work.
const PATIENT_SOLVE: Duration = Duration::from_secs(20);

/// The Chrome major the measured TLS and HTTP/2 of macOS belong to.
pub fn measured_major() -> u32 {
    emulation::get(BrowserKind::Chrome, OsType::Macos)
        .expect("macOS is measured")
        .major
}

/// A fingerprint claiming the measured Chrome on macOS.
pub fn fingerprint(seed: u64) -> Fingerprint {
    Fingerprint::randomized_with_major(OsType::Macos, seed, measured_major())
}

/// A country code that is known to be valid.
pub fn country(code: &str) -> CountryCode {
    CountryCode::new(code).expect("a valid country code")
}

/// The seconds since the Unix epoch, from the system clock.
fn system_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs()
}

/// Everything a test needs to run a session and look at what it did.
pub struct World {
    pub site: ChallengeSite,
    pub launcher: Arc<ScriptedLauncher>,
    pub sink: CollectingEventSink,
    pub store: InMemoryStateStore,
    pub clock: Arc<AtomicU64>,
}

impl World {
    /// A shop, and browsers that report the measured Chrome.
    pub async fn new() -> Self {
        let site = ChallengeSite::start().await;
        let clock = Arc::new(AtomicU64::new(system_now()));
        let product = format!("Chrome/{}.0.8059.40", measured_major());
        let launcher = Arc::new(ScriptedLauncher::new(
            product,
            site.token_source(),
            Arc::clone(&clock),
        ));
        Self {
            site,
            launcher,
            sink: CollectingEventSink::new(),
            store: InMemoryStateStore::new(),
            clock,
        }
    }

    /// A builder wired to this world: the scripted launcher, the shop's authority trusted by both
    /// legs, the clock, the sinks, and the profile of the seed.
    pub fn session(&self) -> StealthSessionBuilder {
        let clock = Arc::clone(&self.clock);
        let launcher = Arc::clone(&self.launcher);
        StealthSession::builder()
            .launcher(launcher)
            .with_event_sink(Arc::new(self.sink.clone()))
            .with_state_store(Arc::new(self.store.clone()))
            .clock(Arc::new(move || clock.load(Ordering::SeqCst)))
            .http_options(self.site.http_options())
            .mitm_options(self.site.mitm_options())
            .fingerprint(fingerprint(SEED))
            .solve_timeout(PATIENT_SOLVE)
    }

    /// Moves the session's clock forward.
    pub fn advance(&self, seconds: u64) {
        self.clock.fetch_add(seconds, Ordering::SeqCst);
    }

    /// The names of the events so far, oldest first.
    pub fn names(&self) -> Vec<&'static str> {
        self.sink.events().iter().map(ScraperEvent::name).collect()
    }

    /// The changes of mode so far, oldest first.
    pub fn changes(&self) -> Vec<(Mode, Mode, Transition)> {
        self.sink
            .events()
            .into_iter()
            .filter_map(|event| match event {
                ScraperEvent::ModeChanged { from, to, reason } => Some((from, to, reason)),
                _ => None,
            })
            .collect()
    }
}
