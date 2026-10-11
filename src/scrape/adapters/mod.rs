//! Adapters that do I/O for the scrape layer: JSON files, tracing, captures and the CDP browser leg.

#![forbid(unsafe_code)]

pub mod browser_scraper;
pub mod capture;
pub mod cdp;
pub mod egress_switch;
pub mod identity_file;
pub mod json_store;
pub mod tracing_sink;
