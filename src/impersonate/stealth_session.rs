//! `StealthSession`: the dual-mode session that moves between plain HTTP and a real browser.
//!
//! A session presents one [`StealthIdentity`] (a fingerprint, the measured browser it belongs to,
//! the cookies it earned and the egress they were earned through) in one of two modes:
//!
//! - [`Mode::Http`]: an [`HttpSession`] sends the requests. It is fast and cheap, and its TLS
//!   `ClientHello`, HTTP/2 start and headers are those of the measured Chrome.
//! - [`Mode::Browser`]: a real Chrome sends them, behind the [`MitmProxy`], which ends the
//!   browser's TLS and replays each request with the same measured `ClientHello` and HTTP/2 start.
//!   Both modes therefore look alike on the wire, and a clearance earned in one is valid in the
//!   other.
//!
//! # The loop
//!
//! A session starts in the mode its [`SessionPolicy`] prefers (HTTP unless told otherwise) and
//! [`fetch`](StealthSession::fetch) asks the policies three questions:
//!
//! 1. Before the request, [`SessionPolicy::before_fetch`]: does the identity hold a clearance for
//!    the site that is about to end, or must it hold one and does not, or does the site's history
//!    say plain requests fail? Then the session goes to the browser first.
//! 2. After the answer, [`MitigationPolicy::decide`]: a real page ends the loop; a challenge met in
//!    HTTP mode sends the session to the browser ([`Transition::Escalate`]); in the browser the
//!    challenge is solved and the cookies move into the identity; a block or a rate limit starts a
//!    cooldown, rotates the egress or the profile, or stops the fetch.
//! 3. After a solve, [`SessionPolicy::after_fetch`]: a browser that earned a clearance hands the
//!    identity back to HTTP ([`Transition::Demote`]), which carries it on from there.
//!
//! Every change of mode emits [`ScraperEvent::ModeChanged`] with its reason. The browser is closed
//! when the session goes back to HTTP and is started again, with the same identity and egress,
//! when it is needed. The MITM proxy lives from the first time a browser is needed until the
//! session is closed, so its port and its certificate stay the same across browsers.
//!
//! ```no_run
//! use seleniumbase_rs::impersonate::stealth_session::StealthSession;
//! use seleniumbase_rs::scrape::egress::geo::CountryCode;
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let mut session = StealthSession::builder()
//!     .with_geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::new("DE")?)])?
//!     .build()
//!     .await?;
//! let page = session.fetch("https://example.com/").await?;
//! println!("{} bytes, {:?} mode", page.body.len(), page.mode);
//! session.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Egress and identity
//!
//! Both legs share one egress: the HTTP session connects through it and the MITM proxy sends the
//! browser's requests through it. A clearance belongs to the address that earned it, so changing
//! the egress ([`rotate_egress`](StealthSession::rotate_egress)) drops the cookies and the
//! clearances, and an identity can only be restored through the egress it names
//! ([`StealthSessionBuilder::restore`], [`StealthSession::import_identity`]).
//!
//! # Lifetime
//!
//! A launched browser lives until the session leaves the browser mode or is closed. There is no
//! idle timeout and no deadline in the session: it can sit unused for as long as the process lives
//! and be used again.
//!
//! # What it does not do
//!
//! A matching TLS and HTTP/2 start is one signal among many, and a session does not make
//! automation permitted by a site's terms. The measured Chrome and the installed Chrome must have
//! the same major version: a browser that is another version is refused before it loads a page.
//!
//! [`SessionPolicy`]: crate::scrape::passport::policy::SessionPolicy
//! [`SessionPolicy::before_fetch`]: crate::scrape::passport::policy::SessionPolicy::before_fetch
//! [`SessionPolicy::after_fetch`]: crate::scrape::passport::policy::SessionPolicy::after_fetch
//! [`MitigationPolicy::decide`]: crate::scrape::challenge::mitigation::MitigationPolicy::decide
//! [`Transition::Escalate`]: crate::scrape::passport::policy::Transition::Escalate
//! [`Transition::Demote`]: crate::scrape::passport::policy::Transition::Demote

mod browser;
mod builder;
mod error;
mod fetch;
mod pace;
mod rotate;
mod transition;

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

pub use self::builder::{StealthSessionBuilder, DEFAULT_ATTEMPTS, MITM_SHUTDOWN_GRACE};
pub use self::error::StealthError;
pub use self::fetch::StealthFetch;
pub use super::http_session::Clock;

use self::browser::BrowserLeg;
use self::pace::SiteLimits;
use super::http_session::{HttpOptions, HttpSession, Rules};
use super::mitm::{MitmOptions, MitmProxy};
use crate::sb_cdp::{LaunchOptionsBuilder, Page};
use crate::scrape::challenge::mitigation::Mode;
use crate::scrape::egress::geo::GeoResolver;
use crate::scrape::egress::pool::ProxyPool;
use crate::scrape::egress::route::{EgressKey, EgressRef};
use crate::scrape::ledger::events::{EventSink, ScraperEvent};
use crate::scrape::ledger::state::StateStore;
use crate::scrape::passport::identity::StealthIdentity;
use crate::scrape::runtime::browser_scraper::BrowserLauncher;
use crate::stealth::fingerprint::OsType;

/// The ports and settings that are fixed when the session is built.
struct Wiring {
    launch: LaunchOptionsBuilder,
    launcher: Arc<dyn BrowserLauncher>,
    store: Arc<dyn StateStore>,
    sink: Arc<dyn EventSink>,
    geo: Arc<dyn GeoResolver>,
    rules: Rules,
    limits: Option<SiteLimits>,
    http: HttpOptions,
    mitm: MitmOptions,
    solve_timeout: Duration,
    clock: Clock,
}

/// The browser profile the session presents.
struct Persona {
    seed: u64,
    /// The Chrome major the measured TLS and HTTP/2 belong to.
    major: u32,
    os: OsType,
}

/// The egress pool and the egress in use.
struct Egress {
    pool: Option<ProxyPool>,
    size: usize,
    current: Option<EgressRef>,
    /// The site fetched last, so a rotation can keep a site on one egress when the pool does.
    site: Option<String>,
}

/// The transport that sends the requests now.
enum Leg {
    Http(HttpSession),
    Browser(BrowserLeg),
}

impl Leg {
    /// The mode the leg stands for.
    fn mode(&self) -> Mode {
        match self {
            Self::Http(_) => Mode::Http,
            Self::Browser(_) => Mode::Browser,
        }
    }

    /// Closes the browser of a browser leg. An HTTP leg has nothing to close.
    async fn close(self) {
        if let Self::Browser(leg) = self {
            leg.close().await;
        }
    }
}

/// A scraping session that moves between HTTP and a real browser. See the
/// [module documentation](self).
pub struct StealthSession {
    wiring: Wiring,
    persona: Persona,
    identity: StealthIdentity,
    leg: Leg,
    /// The proxy the browser leg runs behind, once a browser has been needed.
    mitm: Option<MitmProxy>,
    egress: Egress,
}

impl fmt::Debug for StealthSession {
    /// The mode, the seed and the egress key; never a proxy credential, a token or a cookie.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StealthSession")
            .field("mode", &self.leg.mode())
            .field("seed", &self.persona.seed)
            .field("egress", &self.egress_key())
            .field("egresses", &self.egress.size)
            .field("mitm", &self.mitm.is_some())
            .finish_non_exhaustive()
    }
}

impl StealthSession {
    /// Starts configuring a session.
    #[must_use]
    pub fn builder() -> StealthSessionBuilder {
        StealthSessionBuilder::new()
    }

    /// The mode the requests are sent in now.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.leg.mode()
    }

    /// The identity as of the last change of mode, rotation, import or restore.
    ///
    /// The cookies the current leg has gathered since are not in it;
    /// [`export_identity`](Self::export_identity) reads them.
    #[must_use]
    pub fn identity(&self) -> &StealthIdentity {
        &self.identity
    }

    /// The egress both legs leave through; `None` for the direct connection. It carries no
    /// credentials.
    #[must_use]
    pub fn egress(&self) -> Option<&EgressKey> {
        self.egress_key()
    }

    /// The noise seed of the profile in use.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.persona.seed
    }

    /// The HTTP session, in HTTP mode: for requests of your own, such as an API call, that carry
    /// the cookies the session has earned. `None` in browser mode.
    #[must_use]
    pub fn http(&self) -> Option<&HttpSession> {
        match &self.leg {
            Leg::Http(http) => Some(http),
            Leg::Browser(_) => None,
        }
    }

    /// The tab of the browser, in browser mode. `None` in HTTP mode.
    #[must_use]
    pub fn page(&self) -> Option<&Page> {
        match &self.leg {
            Leg::Browser(leg) => Some(leg.page()),
            Leg::Http(_) => None,
        }
    }

    /// Closes the browser if one is running, and stops the MITM proxy.
    ///
    /// A session that is dropped instead leaves both to their own drop.
    ///
    /// # Errors
    ///
    /// Returns [`StealthError::Scrape`] when the browser cannot be told to close. The proxy is
    /// stopped either way.
    #[tracing::instrument(skip_all)]
    pub async fn close(self) -> Result<(), StealthError> {
        let closed = match self.leg {
            Leg::Browser(leg) => leg.close_checked().await,
            Leg::Http(_) => Ok(()),
        };
        if let Some(mitm) = self.mitm {
            mitm.shutdown(MITM_SHUTDOWN_GRACE).await;
        }
        closed.map_err(StealthError::from)
    }

    /// The time now in seconds since the Unix epoch, from the session's clock.
    fn now(&self) -> u64 {
        (self.wiring.clock)()
    }

    /// Hands an event to the sink.
    fn emit(&self, event: &ScraperEvent) {
        self.wiring.sink.emit(event);
    }

    /// The key of the egress in use.
    fn egress_key(&self) -> Option<&EgressKey> {
        self.egress.current.as_ref().map(EgressRef::key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_send<T: Send>(_value: T) {}

    /// Never called: it only has to compile, which proves the session and the futures of its
    /// methods can be moved to another task.
    #[expect(
        dead_code,
        reason = "compiled to prove the futures are Send, never run"
    )]
    fn the_session_can_be_driven_from_a_spawned_task(mut session: StealthSession) {
        is_send(session.fetch("https://example.com/"));
        is_send(session.escalate());
        is_send(session.demote());
        is_send(session.rotate_profile());
        is_send(session.rotate_egress());
        is_send(session.export_identity());
        is_send(StealthSession::builder().build());
        is_send(session);
    }
}
