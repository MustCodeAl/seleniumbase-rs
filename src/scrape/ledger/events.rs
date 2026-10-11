//! The scraper events and the `EventSink` trait, with every URL and egress in them already redacted.
//!
//! A [`ScraperEvent`] can only hold a URL as a [`RedactedUrl`] and a proxy as an [`EgressKey`], so
//! a password, a token in a query or any other credential cannot reach a sink by construction.
//! The other fields are counts, times, kinds and site names. Events serialize as JSON objects with
//! an `event` field naming the variant in `snake_case`.
//!
//! A sink is handed each event as it happens, from whatever task produced it, so [`EventSink::emit`]
//! must not block. [`NoopEventSink`] drops events, [`CollectingEventSink`] keeps the newest ones for
//! tests, and the `TracingEventSink` in the runtime writes them to `tracing`.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use crate::sb_cdp::sync::locked;
use crate::scrape::challenge::detect::{ChallengeKind, Vendor};
use crate::scrape::challenge::mitigation::{Action, Mode};
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::egress::route::EgressKey;
use crate::scrape::passport::policy::Transition;

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;

/// The most events a [`CollectingEventSink`] keeps. Past it, the oldest is dropped for each new one.
pub const MAX_COLLECTED_EVENTS: usize = 10_000;

/// Something a scrape did or decided, with nothing secret in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ScraperEvent {
    /// A request was answered.
    Request {
        /// The URL requested, redacted.
        url: RedactedUrl,
        /// The HTTP status of the answer.
        status: u16,
        /// How the request was made.
        mode: Mode,
        /// How long it took, in milliseconds.
        elapsed_ms: u64,
    },
    /// An answer was a challenge.
    ChallengeDetected {
        /// The URL requested, redacted.
        url: RedactedUrl,
        /// What the answer asked for.
        kind: ChallengeKind,
        /// The vendor behind it, when known.
        vendor: Option<Vendor>,
        /// How the request was made.
        mode: Mode,
    },
    /// A challenge was solved.
    ChallengeSolved {
        /// The URL requested, redacted.
        url: RedactedUrl,
        /// What the challenge was.
        kind: ChallengeKind,
        /// How many attempts it took.
        attempts: u32,
        /// How long it took, in milliseconds.
        elapsed_ms: u64,
    },
    /// The mitigation policy chose what to do next.
    ActionChosen {
        /// The site the decision is about.
        site: String,
        /// What was chosen.
        action: Action,
    },
    /// The session moved between HTTP and the browser.
    ModeChanged {
        /// The mode it left.
        from: Mode,
        /// The mode it entered.
        to: Mode,
        /// Why.
        reason: Transition,
    },
    /// The session now leaves through another egress.
    EgressRotated {
        /// The egress it left, `None` for the direct connection.
        from: Option<EgressKey>,
        /// The egress it entered, `None` for the direct connection.
        to: Option<EgressKey>,
    },
    /// The session now presents another browser profile.
    ProfileRotated {
        /// The noise seed of the profile it left.
        from_seed: u64,
        /// The noise seed of the profile it entered.
        to_seed: u64,
    },
    /// A clearance is about to end.
    ClearanceExpiring {
        /// The site the clearance is for.
        site: String,
        /// Seconds left.
        remaining_secs: u64,
    },
    /// A site is to be left alone for a while.
    CooldownStarted {
        /// The site.
        site: String,
        /// When it may be tried again, in seconds since the Unix epoch.
        until: u64,
    },
    /// An identity was taken out of the session.
    IdentityExported {
        /// How many cookies it holds.
        cookies: usize,
    },
    /// An identity was put into the session.
    IdentityRestored {
        /// How many cookies it holds.
        cookies: usize,
    },
}

impl ScraperEvent {
    /// The name of the variant in `snake_case`, the value of the `event` field when serialized.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Request { .. } => "request",
            Self::ChallengeDetected { .. } => "challenge_detected",
            Self::ChallengeSolved { .. } => "challenge_solved",
            Self::ActionChosen { .. } => "action_chosen",
            Self::ModeChanged { .. } => "mode_changed",
            Self::EgressRotated { .. } => "egress_rotated",
            Self::ProfileRotated { .. } => "profile_rotated",
            Self::ClearanceExpiring { .. } => "clearance_expiring",
            Self::CooldownStarted { .. } => "cooldown_started",
            Self::IdentityExported { .. } => "identity_exported",
            Self::IdentityRestored { .. } => "identity_restored",
        }
    }
}

/// Where scraper events go.
///
/// Implementations are shared between tasks and are called from the scrape's own path, so
/// [`emit`](Self::emit) must not block or do slow I/O.
pub trait EventSink: Send + Sync {
    /// Takes in one event.
    fn emit(&self, event: &ScraperEvent);
}

/// A sink that drops every event.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: &ScraperEvent) {}
}

/// A sink that keeps the newest [`MAX_COLLECTED_EVENTS`] events, for tests and short runs.
///
/// Clones share one list.
#[derive(Clone, Default)]
pub struct CollectingEventSink {
    events: Arc<Mutex<VecDeque<ScraperEvent>>>,
}

impl CollectingEventSink {
    /// A sink with no events.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The events so far, oldest first.
    #[must_use]
    pub fn events(&self) -> Vec<ScraperEvent> {
        locked(&self.events).iter().cloned().collect()
    }

    /// The events so far, oldest first, removed from the sink.
    #[must_use]
    pub fn take(&self) -> Vec<ScraperEvent> {
        locked(&self.events).drain(..).collect()
    }

    /// How many events the sink holds.
    #[must_use]
    pub fn len(&self) -> usize {
        locked(&self.events).len()
    }

    /// Whether the sink holds no event.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Debug for CollectingEventSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CollectingEventSink")
            .field("events", &self.len())
            .finish()
    }
}

impl EventSink for CollectingEventSink {
    fn emit(&self, event: &ScraperEvent) {
        let mut events = locked(&self.events);
        if events.len() >= MAX_COLLECTED_EVENTS {
            events.pop_front();
        }
        events.push_back(event.clone());
    }
}
