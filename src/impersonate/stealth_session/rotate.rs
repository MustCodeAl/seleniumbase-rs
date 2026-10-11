//! Rotating the profile and the egress of a session, and moving identities in and out.

use super::{StealthError, StealthSession};
use crate::impersonate::client::ImpersonationProfile;
use crate::scrape::egress::route::{EgressKey, EgressRef};
use crate::scrape::error::ScrapeError;
use crate::scrape::ledger::events::ScraperEvent;
use crate::scrape::passport::identity::{EmulationRef, StealthIdentity};
use crate::scrape::runtime::cdp::EgressLocale;
use crate::stealth::fingerprint::{Fingerprint, OsType};
use crate::stealth::humanize::Rng;
use crate::wire::BrowserKind;

/// A fresh persona for `os` and `seed` that claims the Chrome `major`, in the locale of `placed`
/// when there is one.
pub(super) fn persona_fingerprint(
    os: OsType,
    seed: u64,
    major: u32,
    placed: Option<&EgressLocale>,
) -> Fingerprint {
    let mut fingerprint = Fingerprint::randomized_with_major(os, seed, major);
    if let Some(placed) = placed {
        placed.apply_to(&mut fingerprint);
    }
    fingerprint
}

/// Checks that `identity` can be presented through `egress`: that it is valid, that it belongs to
/// that egress, and that it is for the measured Chrome its fingerprint claims.
pub(super) fn check_identity(
    identity: &StealthIdentity,
    egress: Option<&EgressKey>,
) -> Result<(), StealthError> {
    identity.verify()?;
    identity.check_egress(egress)?;
    let profile = ImpersonationProfile::from_fingerprint(&identity.fingerprint)?;
    identity.check_emulation(&EmulationRef {
        kind: profile.kind(),
        major: profile.major(),
        os: profile.os(),
    })?;
    Ok(())
}

impl StealthSession {
    /// Presents a new browser profile through the same egress.
    ///
    /// The seed is replaced by one derived from it, and a new fingerprint is drawn for the same
    /// operating system, claiming the measured Chrome, in the locale of the egress. The cookies
    /// and the clearances are dropped: they belong to the old profile. In browser mode a new
    /// Chrome is started behind the same MITM proxy, on the same port and through the same egress,
    /// and the old one is closed. What is known of each site stays.
    ///
    /// # Errors
    ///
    /// Returns [`StealthError::Scrape`] when the locale of the egress is not known or no browser
    /// can be started, and the errors of [`escalate`](Self::escalate) in browser mode. On an error
    /// the session is as it was.
    #[tracing::instrument(skip_all)]
    pub async fn rotate_profile(&mut self) -> Result<(), StealthError> {
        let from_seed = self.persona.seed;
        let to_seed = Rng::new(from_seed).next_u64();
        let placed = self.placed().await?;
        let fingerprint = persona_fingerprint(
            self.persona.os,
            to_seed,
            self.persona.major,
            placed.as_ref(),
        );
        let identity = StealthIdentity::new(
            fingerprint,
            self.emulation(),
            self.egress_key().cloned(),
            self.now(),
        )?;
        let mode = self.mode();
        self.enter(mode, identity).await?;
        self.persona.seed = to_seed;
        self.emit(&ScraperEvent::ProfileRotated { from_seed, to_seed });
        Ok(())
    }

    /// Leaves through another egress of the pool, with the same fingerprint in the language and
    /// time zone of the new country.
    ///
    /// The cookies and the clearances are dropped: a clearance belongs to the address that earned
    /// it. The MITM proxy, when one runs, is pointed at the new egress first, so its port stays;
    /// in browser mode a new Chrome is started, because the fingerprint a tab carries is written
    /// into every document before its own scripts run. [`ScraperEvent::EgressRotated`] is emitted
    /// once.
    ///
    /// # Errors
    ///
    /// Returns [`StealthError::Scrape`] with [`ScrapeError::NoEgressAvailable`] when the pool has
    /// no other egress that is in rotation, [`StealthError::Mitm`] when the proxy cannot use the
    /// new egress, and the errors of [`rotate_profile`](Self::rotate_profile). On an error the
    /// session is as it was.
    #[tracing::instrument(skip_all)]
    pub async fn rotate_egress(&mut self) -> Result<(), StealthError> {
        let next = self.next_egress()?;
        let placed = EgressLocale::resolve(&*self.wiring.geo, &next).await?;
        let mut fingerprint = self.identity.fingerprint.clone();
        placed.apply_to(&mut fingerprint);
        let identity = StealthIdentity::new(
            fingerprint,
            self.emulation(),
            Some(next.key().clone()),
            self.now(),
        )?;
        let from = self.egress_key().cloned();
        let to = next.key().clone();
        let previous = self.egress.current.replace(next.clone());
        let routed = match self.route_proxy(Some(next)) {
            Ok(routed) => routed,
            Err(error) => {
                self.egress.current = previous;
                return Err(error);
            }
        };
        let mode = self.mode();
        if let Err(error) = self.enter(mode, identity).await {
            self.egress.current.clone_from(&previous);
            self.undo_route(previous);
            return Err(error);
        }
        if !routed {
            self.emit(&ScraperEvent::EgressRotated { from, to: Some(to) });
        }
        Ok(())
    }

    /// Takes the identity out of the session: the fingerprint, the cookies the leg in use holds,
    /// the clearances among them, and the egress they were earned through.
    ///
    /// # Errors
    ///
    /// Returns [`StealthError::Scrape`] when the cookies cannot be read from the browser.
    #[tracing::instrument(skip_all)]
    pub async fn export_identity(&self) -> Result<StealthIdentity, StealthError> {
        self.snapshot().await
    }

    /// Puts an identity into the session: the leg in use is started again with its fingerprint
    /// and cookies.
    ///
    /// The identity must belong to the egress in use: a clearance cookie is bound to the address
    /// that earned it, so one carried to another address would be refused and could mark it.
    ///
    /// # Errors
    ///
    /// Returns [`StealthError::Scrape`] with [`ScrapeError::Identity`] when the identity is not
    /// valid, belongs to another egress, or was made for another Chrome version than the one
    /// measured, and the errors of [`escalate`](Self::escalate) in browser mode. On an error the
    /// session is as it was.
    #[tracing::instrument(skip_all)]
    pub async fn import_identity(
        &mut self,
        identity: &StealthIdentity,
    ) -> Result<(), StealthError> {
        check_identity(identity, self.egress_key())?;
        let mode = self.mode();
        self.enter(mode, identity.clone()).await?;
        self.persona.seed = identity.fingerprint.seed.unwrap_or(self.persona.seed);
        self.persona.major = identity.emulation.major;
        self.persona.os = identity.emulation.os;
        Ok(())
    }

    /// The measured browser the session presents.
    fn emulation(&self) -> EmulationRef {
        EmulationRef {
            kind: BrowserKind::Chrome,
            major: self.persona.major,
            os: self.persona.os,
        }
    }

    /// The locale of the egress in use, when there is an egress.
    async fn placed(&self) -> Result<Option<EgressLocale>, StealthError> {
        match &self.egress.current {
            Some(egress) => Ok(Some(
                EgressLocale::resolve(&*self.wiring.geo, egress).await?,
            )),
            None => Ok(None),
        }
    }

    /// The egress to move to: not the one in use, and in rotation.
    fn next_egress(&self) -> Result<EgressRef, StealthError> {
        let pool = self
            .egress
            .pool
            .as_ref()
            .ok_or(ScrapeError::NoEgressAvailable)?;
        let site = self.egress.site.as_deref();
        let picked = match self.egress_key() {
            Some(current) => pool.pick_other(current, site, self.now()),
            None => pool.pick(site, self.now()),
        };
        Ok(picked.ok_or(ScrapeError::NoEgressAvailable)?)
    }

    /// Points the MITM proxy at `egress`, when one runs. Returns whether it did, in which case
    /// the proxy has reported the change.
    fn route_proxy(&self, egress: Option<EgressRef>) -> Result<bool, StealthError> {
        match &self.mitm {
            Some(mitm) => {
                mitm.swap_upstream(egress)?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Points the proxy back at `egress` after a rotation that did not work. The proxy answers
    /// for the egress it was given before, so a failure here is only logged.
    fn undo_route(&self, egress: Option<EgressRef>) {
        if let Err(error) = self.route_proxy(egress) {
            tracing::debug!(%error, "the proxy could not be pointed back at the old egress");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrape::egress::geo::{CountryCode, Locale};
    use crate::scrape::passport::identity::IdentityError;
    use crate::wire::emulation;

    fn measured() -> u32 {
        emulation::get(BrowserKind::Chrome, OsType::Macos)
            .expect("macOS is measured")
            .major
    }

    fn emulation_of(major: u32) -> EmulationRef {
        EmulationRef {
            kind: BrowserKind::Chrome,
            major,
            os: OsType::Macos,
        }
    }

    fn identity_claiming(major: u32, emulated: u32) -> StealthIdentity {
        let fingerprint = Fingerprint::randomized_with_major(OsType::Macos, 3, major);
        StealthIdentity::new(fingerprint, emulation_of(emulated), None, 0).expect("an identity")
    }

    fn key(host: &str) -> EgressKey {
        EgressKey {
            scheme: "http".to_owned(),
            host: host.to_owned(),
            port: 8080,
            country: None,
        }
    }

    #[test]
    fn a_persona_claims_the_measured_chrome_and_backs_a_profile() {
        let major = measured();
        for seed in 0..32 {
            let fingerprint = persona_fingerprint(OsType::Macos, seed, major, None);
            let profile = ImpersonationProfile::from_fingerprint(&fingerprint).expect("a profile");
            assert_eq!(profile.major(), major, "seed {seed}");
        }
    }

    #[test]
    fn a_persona_is_a_function_of_its_seed() {
        let major = measured();
        let draw = |seed| persona_fingerprint(OsType::Macos, seed, major, None);
        assert_eq!(draw(5), draw(5));
        assert_ne!(draw(5), draw(6));
    }

    #[test]
    fn a_persona_takes_the_language_and_time_zone_of_its_egress() {
        let locale = Locale::for_country(CountryCode::new("DE").expect("a country"))
            .expect("Germany has a locale");
        let placed = EgressLocale {
            locale,
            timezone: "Europe/Berlin".to_owned(),
        };

        let fingerprint = persona_fingerprint(OsType::Macos, 1, measured(), Some(&placed));

        assert_eq!(fingerprint.locale.as_deref(), Some(locale.tag));
        assert_eq!(fingerprint.timezone.as_deref(), Some("Europe/Berlin"));
        assert!(ImpersonationProfile::from_fingerprint(&fingerprint).is_ok());
    }

    #[test]
    fn an_identity_of_this_egress_and_the_measured_chrome_is_accepted() {
        let identity = identity_claiming(measured(), measured());
        assert!(check_identity(&identity, None).is_ok());
    }

    #[test]
    fn an_identity_of_another_egress_is_refused_in_both_directions() {
        let direct = identity_claiming(measured(), measured());
        let error = check_identity(&direct, Some(&key("proxy.example"))).expect_err("a proxy");
        assert!(matches!(
            error,
            StealthError::Scrape(ScrapeError::Identity(IdentityError::EgressMismatch { .. }))
        ));
        let mut proxied = identity_claiming(measured(), measured());
        proxied.egress = Some(key("proxy.example"));
        assert!(check_identity(&proxied, None).is_err());
        assert!(check_identity(&proxied, Some(&key("other.example"))).is_err());
        assert!(check_identity(&proxied, Some(&key("proxy.example"))).is_ok());
    }

    #[test]
    fn an_identity_for_another_chrome_than_the_measured_one_is_refused() {
        let major = measured();
        let claims_other = identity_claiming(major - 1, major - 1);
        assert!(
            matches!(
                check_identity(&claims_other, None),
                Err(StealthError::Impersonate(_))
            ),
            "its user agent names a Chrome that was not measured"
        );
        let mislabelled = identity_claiming(major, major - 1);
        assert!(
            matches!(
                check_identity(&mislabelled, None),
                Err(StealthError::Scrape(ScrapeError::Identity(
                    IdentityError::EmulationMismatch { .. }
                )))
            ),
            "it is labelled as another version than it claims"
        );
    }

    #[test]
    fn an_identity_that_carries_a_proxy_is_refused() {
        let mut identity = identity_claiming(measured(), measured());
        identity.fingerprint.proxy = Some(crate::stealth::fingerprint::ProxyConfig::default());
        assert!(check_identity(&identity, None).is_err());
    }
}
