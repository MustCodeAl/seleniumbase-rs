//! Rotating the egress and the profile of a browser scraper, and moving identities and sessions.

use super::launcher::{claimed_major, host_os, persona_for};
use super::{BrowserScraper, Current};
use crate::sb_cdp::Browser;
use crate::scrape::egress::route::EgressRef;
use crate::scrape::error::ScrapeError;
use crate::scrape::ledger::events::ScraperEvent;
use crate::scrape::passport::identity::{EmulationRef, StealthIdentity};
use crate::scrape::runtime::cdp::{self, EgressLocale};
use crate::stealth::fingerprint::Fingerprint;
use crate::stealth::humanize::Rng;
use crate::wire::BrowserKind;

impl BrowserScraper {
    /// Leaves through another egress of the pool.
    ///
    /// A new browser context is opened behind the new proxy (see
    /// [`ContextProxySwitch`](super::super::egress_switch::ContextProxySwitch)) and the one before is
    /// dropped with its cookies: a clearance belongs to the address that earned it. The language
    /// and the time zone of the tab follow the country of the new egress. The browser is not
    /// restarted. The profile, the seed and what is known of each site stay.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::NoEgressAvailable`] when the pool has no other egress that is in
    /// rotation, [`ScrapeError::Geo`] when the new egress's country has no locale, and
    /// [`ScrapeError::Browser`] when the browser refuses. Up to the switch the scraper is left as it
    /// was on an error; a locale override the browser refuses after it is returned with the scraper
    /// already on the new egress.
    #[tracing::instrument(skip_all)]
    pub async fn rotate_egress(&mut self) -> Result<(), ScrapeError> {
        let next = self.next_egress()?;
        let placed = EgressLocale::resolve(&*self.wiring.geo, &next).await?;
        let page = self
            .wiring
            .switch
            .switch(&self.browser, Some(&next))
            .await?;
        let from = self.current_key();
        let to = next.key().clone();
        self.page = page;
        self.last = None;
        placed.apply_to(&mut self.persona.fingerprint);
        self.egress.current = Some(Current {
            egress: next,
            placed: placed.clone(),
        });
        self.emit(&ScraperEvent::EgressRotated { from, to: Some(to) });
        cdp::apply_locale(&self.page, &placed).await
    }

    /// Presents a new browser profile.
    ///
    /// The seed is replaced by one derived from it, a new fingerprint is drawn for the same
    /// operating system, claiming the version of the Chrome that is running, in the locale of the
    /// egress in use. The browser is started again with it and the old one is closed. The new browser
    /// leaves through the same egress, in a context of its own, so its cookies and clearances start
    /// empty. What is known of each site stays.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when the new browser cannot be started. On an error the old
    /// browser is still the scraper's.
    #[tracing::instrument(skip_all)]
    pub async fn rotate_profile(&mut self) -> Result<(), ScrapeError> {
        let from_seed = self.persona.seed;
        let to_seed = Rng::new(from_seed).next_u64();
        let os = self.persona.fingerprint.os_type;
        let mut fingerprint = persona_for(os, to_seed, self.persona.major);
        if let Some(current) = &self.egress.current {
            current.placed.apply_to(&mut fingerprint);
        }
        self.relaunch(fingerprint).await?;
        self.persona.seed = to_seed;
        self.emit(&ScraperEvent::ProfileRotated { from_seed, to_seed });
        Ok(())
    }

    /// Takes the identity out of the scraper: the fingerprint, the cookies the browser holds, the
    /// clearances among them, and the egress they were earned through.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Identity`] when the fingerprint is not coherent and
    /// [`ScrapeError::Browser`] when the cookies cannot be read.
    #[tracing::instrument(skip_all)]
    pub async fn export_identity(&self) -> Result<StealthIdentity, ScrapeError> {
        let now = self.now();
        let mut identity = StealthIdentity::new(
            self.persona.fingerprint.clone(),
            self.emulation(),
            self.current_key(),
            now,
        )?;
        identity.cookies = cdp::export_cookies(&self.page, now).await?;
        identity.refresh_clearances(now);
        self.emit(&ScraperEvent::IdentityExported {
            cookies: identity.cookies.len(),
        });
        Ok(identity)
    }

    /// Puts an identity into the scraper: the browser is started again with its fingerprint when it
    /// differs from the one in use, and its cookies are set.
    ///
    /// The identity must belong to the egress in use: a clearance cookie is bound to the address
    /// that earned it, so one carried to another address would be refused and could mark it.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Identity`] when the identity is not valid, belongs to another egress,
    /// or was made for another version of Chrome than the one running; [`ScrapeError::Browser`]
    /// when the browser refuses.
    #[tracing::instrument(skip_all)]
    pub async fn import_identity(&mut self, identity: &StealthIdentity) -> Result<(), ScrapeError> {
        identity.verify()?;
        identity.check_egress(self.egress())?;
        if self.persona.major.is_some() {
            identity.check_emulation(&self.emulation())?;
        }
        if identity.fingerprint != self.persona.fingerprint {
            self.relaunch(identity.fingerprint.clone()).await?;
            self.persona.seed = identity.fingerprint.seed.unwrap_or(self.persona.seed);
        }
        cdp::import_cookies(&self.page, &identity.cookies, self.now()).await?;
        self.emit(&ScraperEvent::IdentityRestored {
            cookies: identity.cookies.len(),
        });
        Ok(())
    }

    /// Saves the cookies of the browser context and the local storage of the page's origin in the
    /// scraper's [`SessionStore`](crate::sb_cdp::SessionStore) as `name`, the way a pool lease saves
    /// its login.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when the browser cannot be read.
    pub async fn save_session(&self, name: impl Into<String>) -> Result<(), ScrapeError> {
        let session = cdp::capture_session(&self.page).await?;
        self.wiring.sessions.save(name, session);
        Ok(())
    }

    /// Loads the session saved as `name` into the browser context. Returns `false`, changing
    /// nothing, when there is no such session.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Browser`] when the browser refuses.
    pub async fn load_session(&self, name: &str) -> Result<bool, ScrapeError> {
        let Some(session) = self.wiring.sessions.load(name) else {
            return Ok(false);
        };
        cdp::restore_session(&self.page, session, seconds_f64(self.now())).await?;
        Ok(true)
    }

    /// The egress to move to: not the one in use, and in rotation.
    fn next_egress(&self) -> Result<EgressRef, ScrapeError> {
        let pool = self
            .egress
            .pool
            .as_ref()
            .ok_or(ScrapeError::NoEgressAvailable)?;
        let site = self.egress.site.as_deref();
        let picked = match self.egress() {
            Some(current) => pool.pick_other(current, site, self.now()),
            None => pool.pick(site, self.now()),
        };
        picked.ok_or(ScrapeError::NoEgressAvailable)
    }

    /// Starts a new browser with `fingerprint`, opens its first tab behind the egress in use, and
    /// closes the old browser. The old one is kept when anything before that fails.
    async fn relaunch(&mut self, fingerprint: Fingerprint) -> Result<(), ScrapeError> {
        let options = self.wiring.launch.clone().fingerprint(&fingerprint);
        let browser = self.wiring.launcher.launch(options.build()?).await?;
        let page = match self
            .wiring
            .open_page(&browser, self.egress.current.as_ref())
            .await
        {
            Ok(page) => page,
            Err(error) => {
                close_quietly(&browser).await;
                return Err(error);
            }
        };
        let old = std::mem::replace(&mut self.browser, browser);
        self.page = page;
        self.persona.fingerprint = fingerprint;
        self.last = None;
        close_quietly(&old).await;
        Ok(())
    }

    /// The measured browser an identity is for: the Chrome that is running, on this machine's
    /// operating system.
    fn emulation(&self) -> EmulationRef {
        let major = self
            .persona
            .major
            .or_else(|| claimed_major(&self.persona.fingerprint))
            .unwrap_or_default();
        EmulationRef {
            kind: BrowserKind::Chrome,
            major,
            os: host_os(),
        }
    }
}

/// Closes a browser that is no longer wanted. If it will not close, that is only logged.
async fn close_quietly(browser: &Browser) {
    if let Err(error) = browser.close().await {
        tracing::debug!(%error, "a browser did not close cleanly");
    }
}

/// Seconds since the Unix epoch as the floating-point number the session store takes.
#[expect(
    clippy::cast_precision_loss,
    reason = "seconds since the epoch are far below 2^53, which f64 holds exactly"
)]
fn seconds_f64(seconds: u64) -> f64 {
    seconds as f64
}
