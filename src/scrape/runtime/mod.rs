//! The runtime: everything in the scrape layer that does I/O, from JSON files to the CDP browser.

#![forbid(unsafe_code)]

pub mod browser_scraper;
pub mod capture;
pub mod cdp;
pub mod egress_switch;
pub mod identity_file;
pub mod json_store;
pub mod tracing_sink;
