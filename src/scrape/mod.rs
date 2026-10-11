//! Stealth scraping without TLS spoofing: one pure module per concern, and the runtime that does the I/O.
//!
//! The main types are re-exported here, so a caller does not need the module paths:
//! `BrowserScraper` and its builder, `ScrapeError`, the egress types (`EgressRef`, `EgressKey`,
//! `ProxyPool`, `EgressSwitch`), the identity (`StealthIdentity`, `SessionPolicy`), the cookie
//! jar (`CookieJar`), the challenge types (`Detection`, `ChallengeKind`, `MitigationPolicy`, ...)
//! and the events with their sinks (`ScraperEvent`, `EventSink` and the three sinks that ship).
//! The pure modules stay as they were; the I/O sits under `runtime`.
//!
//! The scrape layer is experimental, like the rest of the stealth layer: its types may change
//! between releases.

#![forbid(unsafe_code)]

pub mod challenge;
pub mod egress;
pub mod error;
pub mod jar;
pub mod ledger;
pub mod passport;
pub mod runtime;

pub use challenge::detect::{ChallengeKind, Detection, Evidence, ResponseView, Vendor};
pub use challenge::mitigation::{Action, AttemptContext, GiveUpReason, MitigationPolicy, Mode};
pub use egress::geo::{CountryCode, GeoResolver, StaticGeoResolver};
pub use egress::pool::{ProxyPool, RotationStrategy};
pub use egress::redact::{RedactedUrl, Secret};
pub use egress::route::{EgressKey, EgressRef};
pub use error::ScrapeError;
pub use jar::store::CookieJar;
pub use ledger::events::{CollectingEventSink, EventSink, NoopEventSink, ScraperEvent};
pub use ledger::state::{DomainState, InMemoryStateStore, StateStore};
pub use passport::identity::{EmulationRef, IdentityError, StealthIdentity};
pub use passport::policy::{SessionPolicy, Transition};
pub use runtime::browser_scraper::{
    BrowserFetch, BrowserLauncher, BrowserScraper, BrowserScraperBuilder, ChromeLauncher,
};
pub use runtime::egress_switch::{ContextProxySwitch, EgressSwitch};
pub use runtime::json_store::JsonStateStore;
pub use runtime::tracing_sink::TracingEventSink;
