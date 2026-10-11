//! The `TracingEventSink`, which writes redacted scraper events to `tracing`.
//!
//! Each event becomes one `tracing` event on the [`TARGET`] with an `event` field naming it and
//! one field per value. A [`ScraperEvent`] holds only redacted URLs and egress keys, so nothing
//! secret is formatted here. The sink does no I/O of its own, so it does not block; the
//! subscriber decides where the line goes.

use std::fmt;

use crate::scrape::egress::route::EgressKey;
use crate::scrape::ledger::events::{EventSink, ScraperEvent};

#[cfg(test)]
mod tests;

/// The `tracing` target of every event this sink writes.
pub const TARGET: &str = "seleniumbase_rs::scrape";

/// An event sink that writes to `tracing`.
///
/// Requests and decisions are logged at `DEBUG`, challenges, changes of mode, egress or profile and
/// identity moves at `INFO`, and a cooldown at `WARN`.
#[derive(Clone, Copy, Debug, Default)]
pub struct TracingEventSink;

/// An egress for a log field: `direct` or `scheme://host:port`.
struct Shown<'a>(&'a Option<EgressKey>);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(key) => write!(f, "{key}"),
            None => f.write_str("direct"),
        }
    }
}

impl EventSink for TracingEventSink {
    fn emit(&self, event: &ScraperEvent) {
        let name = event.name();
        match event {
            ScraperEvent::Request {
                url,
                status,
                mode,
                elapsed_ms,
            } => {
                tracing::debug!(target: TARGET, event = name, %url, status, ?mode, elapsed_ms);
            }
            ScraperEvent::ChallengeDetected {
                url,
                kind,
                vendor,
                mode,
            } => {
                tracing::info!(target: TARGET, event = name, %url, ?kind, ?vendor, ?mode);
            }
            ScraperEvent::ChallengeSolved {
                url,
                kind,
                attempts,
                elapsed_ms,
            } => {
                tracing::info!(target: TARGET, event = name, %url, ?kind, attempts, elapsed_ms);
            }
            ScraperEvent::ActionChosen { site, action } => {
                tracing::debug!(target: TARGET, event = name, %site, ?action);
            }
            ScraperEvent::ModeChanged { from, to, reason } => {
                tracing::info!(target: TARGET, event = name, ?from, ?to, ?reason);
            }
            ScraperEvent::EgressRotated { from, to } => {
                tracing::info!(target: TARGET, event = name, from = %Shown(from), to = %Shown(to));
            }
            ScraperEvent::ProfileRotated { from_seed, to_seed } => {
                tracing::info!(target: TARGET, event = name, from_seed, to_seed);
            }
            ScraperEvent::ClearanceExpiring {
                site,
                remaining_secs,
            } => {
                tracing::info!(target: TARGET, event = name, %site, remaining_secs);
            }
            ScraperEvent::CooldownStarted { site, until } => {
                tracing::warn!(target: TARGET, event = name, %site, until);
            }
            ScraperEvent::IdentityExported { cookies }
            | ScraperEvent::IdentityRestored { cookies } => {
                tracing::info!(target: TARGET, event = name, cookies);
            }
        }
    }
}
