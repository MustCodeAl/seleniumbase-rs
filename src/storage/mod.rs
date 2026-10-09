//! Embedded storage on [Turso](https://github.com/tursodatabase/turso).
//!
//! Enabled by the `turso` feature, which is off by default. Everything here
//! lives in a single local database file; nothing is sent over the network.
//!
//! - [`ResultStore`] keeps test runs and their results for reporting and
//!   flakiness analysis (`sbase report`).
//! - [`ProfileVault`] keeps browser profiles, which hold cookies and proxy
//!   passwords, encrypted at rest.

mod db;
mod results;
mod vault;

pub use results::{FlakyTest, ResultStore, RunId, RunInfo, RunSummary};
pub use vault::ProfileVault;
