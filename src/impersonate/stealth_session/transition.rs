//! Moving between the two legs: starting one, taking the identity out of the other, and the
//! manual `escalate` and `demote`.

use std::sync::Arc;

use super::browser::BrowserLeg;
use super::{Leg, StealthError, StealthSession, Wiring};
use crate::impersonate::client::ImpersonationProfile;
use crate::impersonate::http_session::HttpSession;
use crate::impersonate::mitm::MitmProxy;
use crate::scrape::challenge::mitigation::Mode;
use crate::scrape::egress::route::EgressRef;
use crate::scrape::ledger::events::ScraperEvent;
use crate::scrape::passport::identity::StealthIdentity;
use crate::scrape::passport::policy::{DemoteReason, EscalateReason, Transition};

/// Starts the leg for `mode` presenting `identity` through `egress`.
///
/// An HTTP leg is a session over the identity's cookies. A browser leg first makes sure the MITM
/// proxy runs (starting it on first use, and pointing it at the identity's profile otherwise),
/// then starts a Chrome behind it with the identity's fingerprint and cookies. Nothing of the
/// session changes: the caller puts the leg in place once this has worked.
pub(super) async fn open_leg(
    wiring: &Wiring,
    mitm: &mut Option<MitmProxy>,
    mode: Mode,
    identity: &StealthIdentity,
    egress: Option<&EgressRef>,
) -> Result<Leg, StealthError> {
    match mode {
        Mode::Http => Ok(Leg::Http(HttpSession::from_identity(
            identity,
            egress.cloned(),
            wiring.http.clone(),
        )?)),
        Mode::Browser => {
            let profile = ImpersonationProfile::from_fingerprint(&identity.fingerprint)?;
            let claimed = profile.major();
            let proxy = proxy_for(wiring, mitm, profile, egress).await?;
            let leg = BrowserLeg::launch(wiring, &proxy, identity, claimed).await?;
            wiring.sink.emit(&ScraperEvent::IdentityRestored {
                cookies: identity.cookies.len(),
            });
            Ok(Leg::Browser(leg))
        }
    }
}

/// The MITM proxy for `profile`: the running one pointed at the profile, or a new one.
async fn proxy_for(
    wiring: &Wiring,
    mitm: &mut Option<MitmProxy>,
    profile: ImpersonationProfile,
    egress: Option<&EgressRef>,
) -> Result<MitmProxy, StealthError> {
    if let Some(running) = mitm.as_ref() {
        running.swap_profile(profile)?;
        return Ok(running.clone());
    }
    let started = MitmProxy::start(
        profile,
        egress.cloned(),
        wiring.mitm.clone(),
        Arc::clone(&wiring.sink),
    )
    .await?;
    *mitm = Some(started.clone());
    Ok(started)
}

impl StealthSession {
    /// Goes to the browser now, with the identity as it stands, whatever the policies would say.
    ///
    /// A session already in the browser is left as it is. The cookies of the HTTP session move
    /// into the browser, which starts behind the MITM proxy with the same fingerprint and egress.
    /// [`ScraperEvent::ModeChanged`] is emitted with [`EscalateReason::Manual`].
    ///
    /// # Errors
    ///
    /// Returns [`StealthError::Impersonate`] when the Chrome that starts is not the measured
    /// version, [`StealthError::Mitm`] when the proxy cannot start and [`StealthError::Scrape`]
    /// when no browser can be started. On an error the session is as it was.
    #[tracing::instrument(skip_all)]
    pub async fn escalate(&mut self) -> Result<(), StealthError> {
        if self.mode() == Mode::Browser {
            return Ok(());
        }
        self.switch_to(Mode::Browser, Transition::Escalate(EscalateReason::Manual))
            .await
    }

    /// Goes back to plain HTTP now, with the cookies the browser holds, and closes the browser.
    ///
    /// A session already in HTTP mode is left as it is. [`ScraperEvent::ModeChanged`] is emitted
    /// with [`DemoteReason::Manual`].
    ///
    /// A browser that cannot be read any more (it crashed, say) does not trap the session: the
    /// identity as of the last time its cookies were read is used, and the loss is logged.
    ///
    /// # Errors
    ///
    /// Returns [`StealthError::Http`] when the identity cannot back an HTTP session. On an error
    /// the session is as it was.
    #[tracing::instrument(skip_all)]
    pub async fn demote(&mut self) -> Result<(), StealthError> {
        if self.mode() == Mode::Http {
            return Ok(());
        }
        let identity = match self.synced_identity().await {
            Ok(identity) => identity,
            Err(error) => {
                tracing::warn!(%error, "the browser could not be read; using the last known identity");
                self.identity.clone()
            }
        };
        self.move_to(
            identity,
            Mode::Http,
            Transition::Demote(DemoteReason::Manual),
        )
        .await
    }

    /// Takes the identity out of the leg now in use, moves to `mode`, and says why.
    pub(super) async fn switch_to(
        &mut self,
        mode: Mode,
        reason: Transition,
    ) -> Result<(), StealthError> {
        let identity = self.synced_identity().await?;
        self.move_to(identity, mode, reason).await
    }

    /// Presents `identity` in `mode` and emits [`ScraperEvent::ModeChanged`].
    pub(super) async fn move_to(
        &mut self,
        identity: StealthIdentity,
        mode: Mode,
        reason: Transition,
    ) -> Result<(), StealthError> {
        let from = self.mode();
        self.enter(mode, identity).await?;
        self.emit(&ScraperEvent::ModeChanged {
            from,
            to: mode,
            reason,
        });
        Ok(())
    }

    /// Starts the leg for `mode` presenting `identity` and replaces the leg in use with it. The
    /// old leg is closed once the new one is up; on an error the old one stays.
    ///
    /// A running browser is the exception: it shares the MITM proxy with whatever replaces it,
    /// so it is closed first ([`replace_browser`](Self::replace_browser)).
    pub(super) async fn enter(
        &mut self,
        mode: Mode,
        identity: StealthIdentity,
    ) -> Result<(), StealthError> {
        if mode == Mode::Browser && self.mode() == Mode::Browser {
            // Boxed: the future holds a stand-in leg and two launches, and the fetch loop that
            // reaches it through its mitigation steps would otherwise carry all of it.
            Box::pin(self.replace_browser(identity, None)).await?;
            return Ok(());
        }
        let leg = open_leg(
            &self.wiring,
            &mut self.mitm,
            mode,
            &identity,
            self.egress.current.as_ref(),
        )
        .await?;
        let old = std::mem::replace(&mut self.leg, leg);
        self.identity = identity;
        old.close().await;
        Ok(())
    }

    /// Replaces the running browser by one presenting `identity`, through the egress `reroute`
    /// when one is given and through the one in use otherwise. Returns whether the MITM proxy
    /// reported the change of egress.
    ///
    /// The old browser and the new one share the MITM proxy, and a browser that is still open
    /// keeps loading pages, sending its cookies and the clearances it earned. So the order is:
    /// close the old browser, point the proxy at the new egress and profile, start the new
    /// browser. Pointing the proxy first would send the old identity through the new address.
    ///
    /// The session needs a transport all the while, so an HTTP leg for the old identity stands
    /// in until the new browser is up. If the new browser cannot be started, the egress and the
    /// profile are put back and a browser is started again for the old identity, with the
    /// cookies it held; the error is returned. If that cannot be started either, the stand-in
    /// stays: the session goes on over HTTP with its old identity.
    pub(super) async fn replace_browser(
        &mut self,
        identity: StealthIdentity,
        reroute: Option<EgressRef>,
    ) -> Result<bool, StealthError> {
        let old_identity = self.identity_with_cookies().await;
        let old_egress = self.egress.current.clone();
        let standby = open_leg(
            &self.wiring,
            &mut self.mitm,
            Mode::Http,
            &old_identity,
            old_egress.as_ref(),
        )
        .await?;
        std::mem::replace(&mut self.leg, standby).close().await;

        let mut pointed = false;
        if let Some(next) = &reroute {
            self.egress.current = Some(next.clone());
            match self.route_proxy(Some(next.clone())) {
                Ok(routed) => pointed = routed,
                Err(error) => {
                    self.bring_back(old_identity, old_egress).await;
                    return Err(error);
                }
            }
        }
        let started = open_leg(
            &self.wiring,
            &mut self.mitm,
            Mode::Browser,
            &identity,
            self.egress.current.as_ref(),
        )
        .await;
        match started {
            Ok(leg) => {
                self.leg = leg;
                self.identity = identity;
                Ok(pointed)
            }
            Err(error) => {
                if pointed {
                    self.undo_route(old_egress.clone());
                }
                self.bring_back(old_identity, old_egress).await;
                Err(error)
            }
        }
    }

    /// Puts the egress back and starts a browser again for `identity` after a replacement failed.
    /// If that fails too, the HTTP stand-in stays the leg in use.
    async fn bring_back(&mut self, identity: StealthIdentity, egress: Option<EgressRef>) {
        self.egress.current.clone_from(&egress);
        let started = open_leg(
            &self.wiring,
            &mut self.mitm,
            Mode::Browser,
            &identity,
            egress.as_ref(),
        )
        .await;
        match started {
            Ok(leg) => self.leg = leg,
            Err(error) => {
                tracing::warn!(%error, "the browser could not be started again; going on over HTTP");
            }
        }
        self.identity = identity;
    }

    /// The identity as it stands with the cookies the browser in use holds now. Nothing is emitted
    /// and nothing fails: a browser that cannot be read leaves the identity as it was.
    async fn identity_with_cookies(&self) -> StealthIdentity {
        let now = self.now();
        let mut identity = self.identity.clone();
        if let Leg::Browser(leg) = &self.leg {
            match leg.cookies(now).await {
                Ok(cookies) => {
                    identity.cookies = cookies;
                    identity.refresh_clearances(now);
                }
                Err(error) => {
                    tracing::warn!(%error, "the browser could not be read; using the last known cookies");
                }
            }
        }
        identity
    }

    /// The identity with the cookies the leg in use holds now, and its clearances refreshed. The
    /// session keeps it as its identity. Emits [`ScraperEvent::IdentityExported`].
    pub(super) async fn synced_identity(&mut self) -> Result<StealthIdentity, StealthError> {
        let identity = self.snapshot().await?;
        self.identity = identity.clone();
        Ok(identity)
    }

    /// Like [`synced_identity`](Self::synced_identity), without keeping the result.
    pub(super) async fn snapshot(&self) -> Result<StealthIdentity, StealthError> {
        let now = self.now();
        match &self.leg {
            Leg::Http(http) => Ok(http.export_identity(&self.identity)),
            Leg::Browser(leg) => {
                let mut identity = self.identity.clone();
                identity.cookies = leg.cookies(now).await?;
                identity.refresh_clearances(now);
                self.emit(&ScraperEvent::IdentityExported {
                    cookies: identity.cookies.len(),
                });
                Ok(identity)
            }
        }
    }
}
