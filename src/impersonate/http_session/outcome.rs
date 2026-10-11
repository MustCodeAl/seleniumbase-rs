//! The outcome of a fetch in the terms the session policy acts on: is the reply a challenge, what
//! does the mitigation policy do about it, and does the session change mode.
//!
//! [`HttpSession::fetch`] sends the request and judges the reply in one call, and
//! [`HttpSession::assess`] judges a reply that [`HttpRequest::send`](super::HttpRequest::send)
//! already returned. [`HttpSession::preflight`] asks the other question, before the request:
//! whether the identity's clearance for the site is about to end, so that the session should go
//! to the browser first. The decisions are those of `scrape::challenge::mitigation` and
//! `scrape::passport::policy`; this module only fills in what an HTTP session knows.

use wreq::Method;

use super::error::HttpError;
use super::redirect::site_name;
use super::reply::HttpReply;
use super::request::parse_url;
use super::HttpSession;
use crate::scrape::challenge::detect::{ChallengeKind, Detection};
use crate::scrape::challenge::mitigation::{Action, AttemptContext, MitigationPolicy, Mode};
use crate::scrape::ledger::events::ScraperEvent;
use crate::scrape::ledger::state::DomainState;
use crate::scrape::passport::identity::StealthIdentity;
use crate::scrape::passport::policy::{EscalateReason, SessionPolicy, Transition};

/// Where a fetch stands, as the mitigation policy needs to know it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attempt {
    /// The attempt at this URL, counting from one.
    pub number: u32,
    /// The challenges the site showed in a row before this reply.
    pub previous_challenges: u32,
    /// Whether another egress is available.
    pub can_rotate_egress: bool,
    /// Whether a browser is available to take over.
    pub can_escalate: bool,
}

impl Default for Attempt {
    /// The first attempt, no earlier challenge, no other egress, a browser available.
    fn default() -> Self {
        Self {
            number: 1,
            previous_challenges: 0,
            can_rotate_egress: false,
            can_escalate: true,
        }
    }
}

/// The two policies that judge a reply.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rules {
    /// What to do about a challenge.
    pub mitigation: MitigationPolicy,
    /// When the session changes mode.
    pub session: SessionPolicy,
}

/// What a reply means for the session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// Whether the reply is a challenge, and the evidence.
    pub detection: Detection,
    /// What the mitigation policy chose.
    pub action: Action,
    /// Whether the session changes mode because of it.
    pub transition: Transition,
}

impl Verdict {
    /// Whether the reply is a real answer: it is not a challenge, a block or a rate limit.
    #[must_use]
    pub fn is_clear(&self) -> bool {
        self.detection.kind == ChallengeKind::None
    }
}

/// A reply and the verdict on it.
#[derive(Clone, Debug)]
pub struct Fetched {
    /// The reply.
    pub reply: HttpReply,
    /// What it means for the session.
    pub verdict: Verdict,
}

impl HttpSession {
    /// Judges a reply: detects a challenge, asks the mitigation policy what to do, and asks the
    /// session policy whether that changes the mode. Emits `ChallengeDetected` and
    /// `ActionChosen` events.
    ///
    /// `identity` is the identity the session works for; the session is in HTTP mode, so only
    /// an escalation can come out of it.
    #[must_use]
    pub fn assess(
        &self,
        reply: &HttpReply,
        attempt: &Attempt,
        rules: &Rules,
        identity: &StealthIdentity,
    ) -> Verdict {
        let detection = reply.detect();
        let challenged = detection.kind != ChallengeKind::None;
        let context = AttemptContext {
            attempt: attempt.number,
            mode: Mode::Http,
            consecutive_challenges: attempt
                .previous_challenges
                .saturating_add(u32::from(challenged)),
            can_rotate_egress: attempt.can_rotate_egress,
            can_escalate: attempt.can_escalate,
            now: self.now(),
        };
        let action = rules.mitigation.decide(&detection, &context);
        let transition = rules
            .session
            .after_fetch(Mode::Http, &detection, &action, identity);
        if challenged {
            self.emit(&ScraperEvent::ChallengeDetected {
                url: reply.redacted_url(),
                kind: detection.kind,
                vendor: detection.evidence.vendor,
                mode: Mode::Http,
            });
            self.emit(&ScraperEvent::ActionChosen {
                site: site_name(reply.url()),
                action,
            });
        }
        Verdict {
            detection,
            action,
            transition,
        }
    }

    /// Fetches `url` with a `GET` and judges the reply; see [`assess`](Self::assess).
    ///
    /// # Errors
    ///
    /// The errors of [`HttpRequest::send`](super::HttpRequest::send).
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn fetch(
        &self,
        url: &str,
        attempt: &Attempt,
        rules: &Rules,
        identity: &StealthIdentity,
    ) -> Result<Fetched, HttpError> {
        let reply = self.request(Method::GET, url).send().await?;
        let verdict = self.assess(&reply, attempt, rules, identity);
        Ok(Fetched { reply, verdict })
    }

    /// Asks, before a request to `url`, whether the session should go to the browser first
    /// because the identity's clearance for the site is about to end, is missing and required, or
    /// the site's history says plain requests do not work.
    ///
    /// The identity is first brought up to date with the cookies of this session. Emits
    /// `ClearanceExpiring` when that is the reason.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Url`] when `url` cannot be requested.
    pub fn preflight(
        &self,
        url: &str,
        identity: &StealthIdentity,
        state: Option<&DomainState>,
        rules: &Rules,
    ) -> Result<Transition, HttpError> {
        let url = parse_url(url)?;
        let site = site_name(&url);
        let now = self.now();
        let current = self.identity_at(identity, now);
        let transition = rules
            .session
            .before_fetch(Mode::Http, &current, &site, state, now);
        if let Transition::Escalate(EscalateReason::ClearanceExpiring { remaining_secs }) =
            transition
        {
            self.emit(&ScraperEvent::ClearanceExpiring {
                site,
                remaining_secs,
            });
        }
        Ok(transition)
    }

    /// The identity `base` with the cookies of this session in it, and the clearances found in
    /// them. Emits `IdentityExported`.
    #[must_use]
    pub fn export_identity(&self, base: &StealthIdentity) -> StealthIdentity {
        let exported = self.identity_at(base, self.now());
        self.emit(&ScraperEvent::IdentityExported {
            cookies: exported.cookies.len(),
        });
        exported
    }

    /// `base` with this session's cookies, its clearances refreshed at `now`.
    ///
    /// A clearance that ended by the clock since `base` recorded it stays on the list, so the
    /// policy sees an ended clearance and sends the session to the browser, instead of seeing no
    /// clearance at all. One the site removed early is gone from the jar and from the list.
    fn identity_at(&self, base: &StealthIdentity, now: u64) -> StealthIdentity {
        let mut identity = base.clone();
        identity.cookies = self.inner().jar.snapshot();
        let earlier = std::mem::take(&mut identity.clearances);
        identity.refresh_clearances(now);
        for ended in earlier.into_iter().filter(|c| c.expires_at <= now) {
            let known = identity
                .clearances
                .iter()
                .any(|c| c.site == ended.site && c.cookie == ended.cookie);
            if !known {
                identity.clearances.push(ended);
            }
        }
        identity
    }
}
