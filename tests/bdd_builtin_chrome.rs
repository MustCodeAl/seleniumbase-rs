//! The built-in steps on a real Chrome, over WebDriver. `#[ignore]`d: run it
//! with `SB_HEADLESS=true cargo test --features "testing bdd" --test
//! bdd_builtin_chrome -- --ignored`, one binary at a time.
//! `tests/bdd_builtin_steps.rs` runs the same steps on a mock WebDriver.
#![cfg(feature = "bdd")]

use rstest_bdd_macros::scenario;
use seleniumbase_rs::testing::bdd::{sb, SbCase};

#[scenario(
    path = "tests/features/bdd_builtin_chrome.feature",
    name = "A small form"
)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts a real Chrome"]
async fn a_small_form(#[future(awt)] sb: SbCase) {
    let _ = &sb;
}
