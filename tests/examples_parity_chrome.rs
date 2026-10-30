//! Ports of Python SeleniumBase's own examples (`examples/` of the upstream
//! repository), run headless on a real Chrome, to measure how well this crate
//! matches the Python library.
//!
//! One test per Python example, named after it, in two modules by engine:
//!
//! - [`base_case`]: the examples built on the `BaseCase` class, which map to
//!   the WebDriver engine (`seleniumbase_rs::BaseCase`);
//! - [`cdp`]: the examples of `cdp_mode/` (`sb_cdp.Chrome()` and
//!   `sb.activate_cdp_mode()`), which map to the pure CDP engine
//!   (`seleniumbase_rs::sb_cdp`).
//!
//! The Python files were first run through `sbrs import-python`; what it left
//! as a TODO is in `parity/examples.toml` next to what the port found out. A
//! test keeps the Python example's steps, in its order, with this crate's
//! names for them. Where this crate has no equal of a step, the test says so
//! in a `MISMATCH` comment and uses the nearest composition, or, when there
//! is none, keeps the Python behaviour and fails.
//!
//! The tests open the public demo pages that the Python examples use
//! (seleniumbase.io, saucedemo.com, todomvc.com, xkcd.com), so they need a
//! network. Every test is `#[ignore]`d: run them one at a time, as the Chrome
//! tests of this repository are run:
//!
//! ```text
//! cargo test --features "mcp-server test-util testing" --test examples_parity_chrome \
//!     -- --ignored --test-threads=1
//! ```

#[path = "examples_parity/base_case.rs"]
mod base_case;
#[path = "examples_parity/cdp.rs"]
mod cdp;
#[path = "examples_parity/support.rs"]
mod support;
