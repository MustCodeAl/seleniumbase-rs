//! The `sb` and `sb_cdp` fixtures for `cargo test` (feature `testing`).
//!
//! `cargo test` is the one runner and [`rstest`](https://docs.rs/rstest)
//! supplies the fixtures: a test asks for `sb: Sb` and calls
//! [`Sb::run`] with its body, and the library does the rest with the run
//! configuration (the settings file, `SB_SETTINGS_FILE` and the `SB_*`
//! variables that `sbrs test` sets from its flags):
//!
//! - the browser options, the test data (`--env`, `--data`, `--var1`, ...)
//!   and the run settings;
//! - reruns (`--reruns`, `--reruns-delay`) and a hard time limit
//!   (`--time-limit`);
//! - on a failure, `latest_logs/<test id>/` with `screenshot.png`,
//!   `page_source.html` and `basic_test_info.txt`, taken while the browser is
//!   still open (`--no-screenshot`, `--screenshot` and `--list-fail-page`);
//! - the debug prompts (`--trace`, `--pdb`, `--final-debug`) and the
//!   browser closed whatever happened.
//!
//! ```no_run
//! use rstest::rstest;
//! use seleniumbase_rs::testing::{sb, Sb};
//! use seleniumbase_rs::Result;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn valid_login(sb: Sb) -> Result<()> {
//!     sb.run(async |sb| {
//!         sb.open("https://example.com/login").await?;
//!         sb.type_text("#user", sb.account().unwrap_or("demo")).await?;
//!         sb.assert_title("Example Domain").await
//!     })
//!     .await
//! }
//! ```
//!
//! `sb_cdp: SbCdp` is the same for the pure CDP engine, with a
//! [`Page`](crate::sb_cdp::Page) in place of the `BaseCase`. Per-test
//! overrides, the Rust form of Python's `SB(...)` arguments, go through
//! [`Sb::configure`]. Anything that wants to know how each attempt ended (the
//! reports, the plugin host, the recorder) is a [`Finalizer`].

mod deadline;
mod evidence;
mod fixture;
mod lifecycle;

pub use fixture::{sb, sb_cdp, Sb, SbCdp};
pub use lifecycle::{AttemptEnd, Finalizer, Outcome};
/// rstest's attribute for a fixture of your own, so a test file needs one import.
pub use rstest::fixture;

pub(crate) use deadline::Deadline;
