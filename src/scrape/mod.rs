//! Stealth scraping without TLS spoofing: one pure module per concern, and the runtime that does the I/O.

#![forbid(unsafe_code)]

pub mod challenge;
pub mod egress;
pub mod error;
pub mod jar;
pub mod ledger;
pub mod passport;
pub mod runtime;
