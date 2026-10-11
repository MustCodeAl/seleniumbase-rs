//! `BrowserScraper`, the browser-only scraper that fetches, solves challenges, and rotates profile and egress.
//!
//! A [`BrowserScraper`] drives one real Chrome through [`sb_cdp`](crate::sb_cdp). It does no TLS
//! spoofing: Chrome's own handshake leaves the machine, so the page sees a genuine browser. What
//! the scraper adds is the loop a bot-protected site needs around it.
//!
//! # The fetch loop
//!
//! [`fetch`](BrowserScraper::fetch) loads the URL, reads the answer and the rendered page, and asks
//! the [`MitigationPolicy`] what to do with what it found:
//!
//! - a real page ends the loop;
//! - a challenge is solved in the browser ([`solve_challenge`](BrowserScraper::solve_challenge)),
//!   then the page is loaded again with the clearance it earned;
//! - repeated challenges change the egress ([`rotate_egress`](BrowserScraper::rotate_egress)) and
//!   then the whole browser profile ([`rotate_profile`](BrowserScraper::rotate_profile));
//! - a block or a rate limit starts a cooldown for the site, which the next fetch respects;
//! - when the attempts allowed run out the fetch fails with [`ScrapeError`].
//!
//! What each site did is kept in a [`StateStore`], and what the scraper does is reported to an
//! [`EventSink`], with every URL and proxy redacted.
//!
//! # Egress and profile
//!
//! Proxies are given as `(spec, country)` pairs. The browser leaves through one egress at a time,
//! in a browser context of its own, and the fingerprint, language and time zone follow the country
//! of that egress. An egress changes only when a rotation is asked for or the policy asks for one;
//! a clearance cookie belongs to the address that earned it, so the cookies stay with the egress.
//!
//! # Lifetime
//!
//! A launched browser lives until [`close`](BrowserScraper::close). There is no idle timeout and no
//! deadline in the scraper: it can sit unused for as long as the process lives and be used again.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::scrape::egress::geo::CountryCode;
//! use seleniumbase_rs::scrape::runtime::browser_scraper::BrowserScraper;
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::scrape::error::ScrapeError> {
//! let mut scraper = BrowserScraper::builder()
//!     .with_geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::new("DE")?)])?
//!     .with_max_challenge_attempts(4)
//!     .build()
//!     .await?;
//! let page = scraper.fetch("https://example.com/").await?;
//! println!("{} bytes, status {}", page.body.len(), page.status);
//! scraper.close().await?;
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::sb_cdp::{Browser, LaunchOptionsBuilder, Page, SessionStore};
use crate::scrape::challenge::detect::{ChallengeKind, Detection};
use crate::scrape::challenge::mitigation::MitigationPolicy;
use crate::scrape::egress::geo::GeoResolver;
use crate::scrape::egress::pool::ProxyPool;
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::egress::route::{EgressKey, EgressRef};
use crate::scrape::error::ScrapeError;
use crate::scrape::ledger::events::{EventSink, ScraperEvent};
use crate::scrape::ledger::state::StateStore;
use crate::stealth::fingerprint::Fingerprint;

use super::cdp::{EgressLocale, NavResponse};
use super::egress_switch::EgressSwitch;

mod builder;
mod fetch;
mod launcher;
mod pacing;
mod rotate;
#[cfg(test)]
mod tests;

pub use builder::{BrowserScraperBuilder, Clock, DEFAULT_SOLVE_TIMEOUT};
pub use launcher::{BrowserLauncher, ChromeLauncher};

/// The ports and settings that are fixed when the scraper is built.
struct Wiring {
    launch: LaunchOptionsBuilder,
    launcher: Arc<dyn BrowserLauncher>,
    switch: Box<dyn EgressSwitch>,
    store: Arc<dyn StateStore>,
    sink: Arc<dyn EventSink>,
    geo: Arc<dyn GeoResolver>,
    policy: MitigationPolicy,
    limits: Option<pacing::SiteLimits>,
    sessions: SessionStore,
    solve_timeout: Duration,
    clock: Clock,
}

impl Wiring {
    /// The first tab of a new browser: in a context behind the proxy of `current` when there is
    /// one, else the tab the browser opened.
    async fn open_page(
        &self,
        browser: &Browser,
        current: Option<&Current>,
    ) -> Result<Page, ScrapeError> {
        match current {
            Some(current) => self.switch.switch(browser, Some(&current.egress)).await,
            None => Ok(browser.default_page().await?),
        }
    }
}

/// The browser profile the scraper presents.
struct Persona {
    fingerprint: Fingerprint,
    seed: u64,
    /// The major version of the Chrome that is running, when it could be read.
    major: Option<u32>,
}

/// The egress the browser leaves through now, with the locale of its country.
struct Current {
    egress: EgressRef,
    placed: EgressLocale,
}

/// The egress pool and the egress in use.
struct Egress {
    pool: Option<ProxyPool>,
    size: usize,
    current: Option<Current>,
    /// The site fetched last, so a rotation can keep a site on one egress when the pool does.
    site: Option<String>,
}

/// A page fetched through a real browser.
#[derive(Clone)]
pub struct BrowserFetch {
    /// The URL the browser ended on, after redirects.
    pub url: String,
    /// The HTTP status of the answer.
    pub status: u16,
    /// The response headers as (name, value) pairs.
    pub headers: Vec<(String, String)>,
    /// The rendered page, with the changes scripts made.
    pub body: String,
    /// What detection found. The kind is [`ChallengeKind::None`] for a page returned by a fetch.
    pub detection: Detection,
    /// How many loads the fetch took, counting from one.
    pub attempts: u32,
    /// The challenge that was solved on the way, if one was.
    pub solved: Option<ChallengeKind>,
    /// The egress the page came through; `None` for the direct connection.
    pub egress: Option<EgressKey>,
}

impl fmt::Debug for BrowserFetch {
    /// The URL redacted, the body and headers counted: either can hold a credential.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserFetch")
            .field("url", &RedactedUrl::new(&self.url))
            .field("status", &self.status)
            .field("headers", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .field("attempts", &self.attempts)
            .field("solved", &self.solved)
            .field("egress", &self.egress)
            .finish()
    }
}

/// A scraper that drives one real browser. See the [module documentation](self).
pub struct BrowserScraper {
    browser: Browser,
    page: Page,
    wiring: Wiring,
    persona: Persona,
    egress: Egress,
    /// The answer to the navigation that led to the page now open.
    last: Option<NavResponse>,
}

impl fmt::Debug for BrowserScraper {
    /// The tab, the seed and the egress key; never a proxy credential or a cookie.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserScraper")
            .field("page", &self.page.id())
            .field("seed", &self.persona.seed)
            .field("egress", &self.current_key())
            .field("egresses", &self.egress.size)
            .finish_non_exhaustive()
    }
}

impl BrowserScraper {
    /// Starts configuring a scraper.
    #[must_use]
    pub fn builder() -> BrowserScraperBuilder {
        BrowserScraperBuilder::new()
    }

    /// The tab the scraper loads pages in. It changes when the egress or the profile does.
    #[must_use]
    pub fn page(&self) -> &Page {
        &self.page
    }

    /// The browser the scraper drives. It changes when the profile does.
    #[must_use]
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    /// The fingerprint the browser presents.
    #[must_use]
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.persona.fingerprint
    }

    /// The noise seed of the profile in use.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.persona.seed
    }

    /// The egress the browser leaves through; `None` for the direct connection.
    #[must_use]
    pub fn egress(&self) -> Option<&EgressKey> {
        self.egress
            .current
            .as_ref()
            .map(|current| current.egress.key())
    }

    /// The sessions saved with [`save_session`](Self::save_session).
    #[must_use]
    pub fn sessions(&self) -> &SessionStore {
        &self.wiring.sessions
    }

    /// Closes the browser. A scraper that is dropped instead leaves the browser to its own drop.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when the browser cannot be told to close.
    #[tracing::instrument(skip_all)]
    pub async fn close(self) -> Result<(), ScrapeError> {
        self.browser.close().await?;
        Ok(())
    }

    /// The time now in seconds since the Unix epoch, from the scraper's clock.
    fn now(&self) -> u64 {
        (self.wiring.clock)()
    }

    /// Hands an event to the sink.
    fn emit(&self, event: &ScraperEvent) {
        self.wiring.sink.emit(event);
    }

    /// The key of the egress in use, owned.
    fn current_key(&self) -> Option<EgressKey> {
        self.egress().cloned()
    }
}
