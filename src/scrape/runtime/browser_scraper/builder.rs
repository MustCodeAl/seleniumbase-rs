//! The builder of a `BrowserScraper`, and how a scraper starts its browser.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::launcher::{browser_major, host_os, persona_for, BrowserLauncher, ChromeLauncher};
use super::pacing::SiteLimits;
use super::{BrowserScraper, Current, Egress, Persona, Wiring};
use crate::sb_cdp::{LaunchOptionsBuilder, SessionStore};
use crate::scrape::challenge::mitigation::MitigationPolicy;
use crate::scrape::egress::geo::{CountryCode, GeoResolver, StaticGeoResolver};
use crate::scrape::egress::pool::{ProxyPool, RotationStrategy};
use crate::scrape::egress::route::EgressRef;
use crate::scrape::error::ScrapeError;
use crate::scrape::ledger::events::EventSink;
use crate::scrape::ledger::state::{InMemoryStateStore, StateStore};
use crate::scrape::runtime::cdp::EgressLocale;
use crate::scrape::runtime::egress_switch::{ContextProxySwitch, EgressSwitch};
use crate::scrape::runtime::tracing_sink::TracingEventSink;
use crate::stealth::fingerprint::Fingerprint;

/// How long a challenge is given to show as solved, unless the builder says otherwise.
pub const DEFAULT_SOLVE_TIMEOUT: Duration = Duration::from_secs(30);

/// Tells the scraper the time, in seconds since the Unix epoch. Tests replace it.
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Configures a [`BrowserScraper`]; see [`BrowserScraper::builder`].
///
/// Every setting has a default: Chrome with the platform's launch options, no proxy, a state kept
/// in memory, events written to `tracing`, three attempts per fetch, and a profile drawn from a
/// random seed.
pub struct BrowserScraperBuilder {
    launch: LaunchOptionsBuilder,
    egresses: Vec<EgressRef>,
    strategy: RotationStrategy,
    attempts: Option<u32>,
    policy: MitigationPolicy,
    store: Option<Arc<dyn StateStore>>,
    sink: Option<Arc<dyn EventSink>>,
    geo: Option<Arc<dyn GeoResolver>>,
    limits: Option<(usize, Duration)>,
    seed: Option<u64>,
    fingerprint: Option<Fingerprint>,
    switch: Option<Box<dyn EgressSwitch>>,
    launcher: Option<Arc<dyn BrowserLauncher>>,
    sessions: Option<SessionStore>,
    solve_timeout: Duration,
    clock: Option<Clock>,
}

impl fmt::Debug for BrowserScraperBuilder {
    /// Counts the proxies instead of printing them.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserScraperBuilder")
            .field("egresses", &self.egresses.len())
            .field("strategy", &self.strategy)
            .field("seed", &self.seed)
            .finish_non_exhaustive()
    }
}

impl BrowserScraperBuilder {
    pub(super) fn new() -> Self {
        Self {
            launch: LaunchOptionsBuilder::default(),
            egresses: Vec::new(),
            strategy: RotationStrategy::RoundRobin,
            attempts: None,
            policy: MitigationPolicy::default(),
            store: None,
            sink: None,
            geo: None,
            limits: None,
            seed: None,
            fingerprint: None,
            switch: None,
            launcher: None,
            sessions: None,
            solve_timeout: DEFAULT_SOLVE_TIMEOUT,
            clock: None,
        }
    }

    /// The launch options of the browser. The fingerprint of the scraper is applied after them, so
    /// a setting made here (a user agent, say) wins over the fingerprint's.
    #[must_use]
    pub fn launch_options(mut self, launch: LaunchOptionsBuilder) -> Self {
        self.launch = launch;
        self
    }

    /// Adds proxies, each with the country it exits in. The country decides the language and the
    /// time zone the browser presents while it leaves through that proxy.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Egress`] for a spec that is not `scheme://[user:password@]host:port`
    /// with a scheme of http, https, socks5 or socks5h. The error never echoes the credentials.
    pub fn with_geo_proxies<I, S>(mut self, proxies: I) -> Result<Self, ScrapeError>
    where
        I: IntoIterator<Item = (S, CountryCode)>,
        S: AsRef<str>,
    {
        for (spec, country) in proxies {
            self.egresses
                .push(EgressRef::parse(spec.as_ref(), Some(country))?);
        }
        Ok(self)
    }

    /// How the pool picks an egress when one is needed: at the start and at each rotation.
    #[must_use]
    pub fn proxy_strategy(mut self, strategy: RotationStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// How many loads of one URL a fetch may make before it gives up. Zero is read as one.
    #[must_use]
    pub fn with_max_challenge_attempts(mut self, attempts: u32) -> Self {
        self.attempts = Some(attempts);
        self
    }

    /// The whole mitigation policy: when to rotate, how long to cool down. The attempts set by
    /// [`with_max_challenge_attempts`](Self::with_max_challenge_attempts) still apply on top.
    #[must_use]
    pub fn mitigation(mut self, policy: MitigationPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Where what is learned about each site is kept between fetches.
    #[must_use]
    pub fn with_state_store(mut self, store: Arc<dyn StateStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Where the scraper reports what it does.
    #[must_use]
    pub fn with_event_sink(mut self, sink: Arc<dyn EventSink>) -> Self {
        self.sink = Some(sink);
        self
    }

    /// How the country, time zone and language of an egress are found.
    #[must_use]
    pub fn geo_resolver(mut self, geo: Arc<dyn GeoResolver>) -> Self {
        self.geo = Some(geo);
        self
    }

    /// At most `calls` fetches of one site in `window`. The limit is per site, not global.
    #[must_use]
    pub fn per_site_rate(mut self, calls: usize, window: Duration) -> Self {
        self.limits = Some((calls, window));
        self
    }

    /// The seed of the first profile. The same seed gives the same profile, and each rotation
    /// derives the next seed from the last, so a run can be repeated.
    #[must_use]
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// The first profile itself, instead of one drawn from the seed. A rotation replaces it with a
    /// generated profile of the same operating system.
    #[must_use]
    pub fn fingerprint(mut self, fingerprint: Fingerprint) -> Self {
        self.fingerprint = Some(fingerprint);
        self
    }

    /// How the egress is changed. The default opens a browser context behind the new proxy.
    #[must_use]
    pub fn egress_switch(mut self, switch: Box<dyn EgressSwitch>) -> Self {
        self.switch = Some(switch);
        self
    }

    /// How a browser is started. The default starts Chrome.
    #[must_use]
    pub fn launcher(mut self, launcher: Arc<dyn BrowserLauncher>) -> Self {
        self.launcher = Some(launcher);
        self
    }

    /// The store [`save_session`](BrowserScraper::save_session) writes to, to share logins with
    /// other scrapers or a [`BrowserPool`](crate::sb_cdp::BrowserPool).
    #[must_use]
    pub fn session_store(mut self, sessions: SessionStore) -> Self {
        self.sessions = Some(sessions);
        self
    }

    /// How long a solved challenge is waited for. The default is [`DEFAULT_SOLVE_TIMEOUT`].
    #[must_use]
    pub fn solve_timeout(mut self, timeout: Duration) -> Self {
        self.solve_timeout = timeout;
        self
    }

    /// The clock the scraper reads. The default is the system clock.
    #[must_use]
    pub fn clock(mut self, clock: Clock) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Starts the browser and returns the scraper.
    ///
    /// Unless a [`fingerprint`](Self::fingerprint) was given, a browser is started once to learn
    /// which Chrome is installed (headless, and closed again), so the profile can claim that
    /// version. The browser that is kept starts with the profile, in the locale of the first
    /// egress, and in a context behind that egress's proxy when there is one.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when no browser can be started, and [`ScrapeError::Geo`]
    /// when the country of an egress has no locale.
    #[tracing::instrument(skip_all)]
    pub async fn build(self) -> Result<BrowserScraper, ScrapeError> {
        let (wiring, start) = self.into_parts();
        let size = start.egresses.len();
        let pool = (size > 0).then(|| ProxyPool::new(start.egresses, start.strategy));
        let first = pool
            .as_ref()
            .and_then(|pool| pool.pick(None, (wiring.clock)()));
        let current = match first {
            Some(egress) => {
                let placed = EgressLocale::resolve(&*wiring.geo, &egress).await?;
                Some(Current { egress, placed })
            }
            None => None,
        };
        let persona =
            start_persona(&wiring, start.fingerprint, start.seed, current.as_ref()).await?;
        let egress = Egress {
            pool,
            size,
            current,
            site: None,
        };
        start_browser(wiring, persona, egress).await
    }

    /// Fills the defaults and splits the builder into the fixed wiring and the start settings.
    fn into_parts(self) -> (Wiring, Start) {
        let mut policy = self.policy;
        if let Some(attempts) = self.attempts {
            policy.max_challenge_attempts = attempts;
        }
        let wiring = Wiring {
            launch: self.launch,
            launcher: self.launcher.unwrap_or_else(|| Arc::new(ChromeLauncher)),
            switch: self
                .switch
                .unwrap_or_else(|| Box::new(ContextProxySwitch::new())),
            store: self
                .store
                .unwrap_or_else(|| Arc::new(InMemoryStateStore::new())),
            sink: self.sink.unwrap_or_else(|| Arc::new(TracingEventSink)),
            geo: self.geo.unwrap_or_else(|| Arc::new(StaticGeoResolver)),
            policy,
            limits: self
                .limits
                .map(|(calls, window)| SiteLimits::new(calls, window)),
            sessions: self.sessions.unwrap_or_default(),
            solve_timeout: self.solve_timeout,
            clock: self.clock.unwrap_or_else(system_clock),
        };
        let start = Start {
            egresses: self.egresses,
            strategy: self.strategy,
            fingerprint: self.fingerprint,
            seed: self.seed.unwrap_or_else(random_seed),
        };
        (wiring, start)
    }
}

/// Starts the browser that stays, in the first egress's context when there is one.
async fn start_browser(
    wiring: Wiring,
    mut persona: Persona,
    egress: Egress,
) -> Result<BrowserScraper, ScrapeError> {
    let options = wiring.launch.clone().fingerprint(&persona.fingerprint);
    let browser = wiring.launcher.launch(options.build()?).await?;
    let page = match wiring.open_page(&browser, egress.current.as_ref()).await {
        Ok(page) => page,
        Err(error) => {
            if let Err(close) = browser.close().await {
                tracing::debug!(error = %close, "the browser did not close cleanly");
            }
            return Err(error);
        }
    };
    if persona.major.is_none() {
        persona.major = browser_major(&browser).await;
    }
    Ok(BrowserScraper {
        browser,
        page,
        wiring,
        persona,
        egress,
        last: None,
    })
}

/// The settings that only matter while the scraper starts.
struct Start {
    egresses: Vec<EgressRef>,
    strategy: RotationStrategy,
    fingerprint: Option<Fingerprint>,
    seed: u64,
}

/// The profile the scraper starts with: the one given, or one generated for this machine that
/// claims the installed Chrome's version, in the locale of the first egress.
async fn start_persona(
    wiring: &Wiring,
    explicit: Option<Fingerprint>,
    seed: u64,
    current: Option<&Current>,
) -> Result<Persona, ScrapeError> {
    let (mut fingerprint, major) = if let Some(fingerprint) = explicit {
        (fingerprint, None)
    } else {
        let major = probe_major(wiring).await?;
        (persona_for(host_os(), seed, major), major)
    };
    if let Some(current) = current {
        current.placed.apply_to(&mut fingerprint);
    }
    Ok(Persona {
        fingerprint,
        seed,
        major,
    })
}

/// Starts a headless browser to read the version of Chrome, and closes it.
async fn probe_major(wiring: &Wiring) -> Result<Option<u32>, ScrapeError> {
    let options = wiring.launch.clone().headless(true).build()?;
    let probe = wiring.launcher.launch(options).await?;
    let major = browser_major(&probe).await;
    if let Err(error) = probe.close().await {
        tracing::debug!(%error, "the probe browser did not close cleanly");
    }
    Ok(major)
}

/// The system clock, in seconds since the Unix epoch.
fn system_clock() -> Clock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs())
    })
}

/// A seed from the operating system's randomness.
fn random_seed() -> u64 {
    RandomState::new().build_hasher().finish()
}
