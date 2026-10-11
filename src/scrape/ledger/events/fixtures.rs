//! One event of every kind, built from a URL that carries every secret redaction must remove.

use std::time::Duration;

use super::ScraperEvent;
use crate::scrape::challenge::detect::{ChallengeKind, Vendor};
use crate::scrape::challenge::mitigation::{Action, Mode};
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::passport::identity::fixtures::egress;
use crate::scrape::passport::policy::{EscalateReason, Transition};

/// A URL with userinfo, a sensitive query value and a fragment, each marked with a word the
/// tests look for.
pub(crate) const SECRET_URL: &str =
    "https://alice:s3cr3t-pass@example.com/path?page=2&token=SECRET-TOKEN#frag-secret";

/// The words in [`SECRET_URL`] that must never reach an event, a log line or an error.
pub(crate) const SECRETS: [&str; 4] = ["alice", "s3cr3t-pass", "SECRET-TOKEN", "frag-secret"];

fn url() -> RedactedUrl {
    RedactedUrl::new(SECRET_URL)
}

/// One event of every variant, in the order of the enum.
pub(crate) fn every_event() -> Vec<ScraperEvent> {
    vec![
        ScraperEvent::Request {
            url: url(),
            status: 200,
            mode: Mode::Http,
            elapsed_ms: 31,
        },
        ScraperEvent::ChallengeDetected {
            url: url(),
            kind: ChallengeKind::JsChallenge,
            vendor: Some(Vendor::Cloudflare),
            mode: Mode::Http,
        },
        ScraperEvent::ChallengeSolved {
            url: url(),
            kind: ChallengeKind::Interactive,
            attempts: 2,
            elapsed_ms: 4_500,
        },
        ScraperEvent::ActionChosen {
            site: "example.com".to_owned(),
            action: Action::Retry {
                after: Duration::from_millis(1_500),
            },
        },
        ScraperEvent::ModeChanged {
            from: Mode::Http,
            to: Mode::Browser,
            reason: Transition::Escalate(EscalateReason::Challenge(ChallengeKind::JsChallenge)),
        },
        ScraperEvent::EgressRotated {
            from: Some(egress("p1.example")),
            to: None,
        },
        ScraperEvent::ProfileRotated {
            from_seed: 7,
            to_seed: 8,
        },
        ScraperEvent::ClearanceExpiring {
            site: "example.com".to_owned(),
            remaining_secs: 90,
        },
        ScraperEvent::CooldownStarted {
            site: "example.com".to_owned(),
            until: 1_800_000_900,
        },
        ScraperEvent::IdentityExported { cookies: 3 },
        ScraperEvent::IdentityRestored { cookies: 3 },
    ]
}
