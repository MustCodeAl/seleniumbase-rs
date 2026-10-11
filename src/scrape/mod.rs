//! Stealth scraping without TLS spoofing: a pure domain layer, the adapters that do I/O, and the CDP browser leg.

#![forbid(unsafe_code)]

pub mod adapters;
pub mod domain;
pub mod error;
