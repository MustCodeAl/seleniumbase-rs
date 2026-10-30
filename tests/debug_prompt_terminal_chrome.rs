//! The debug prompts as a person sees them, on a real headless Chrome.
//!
//! Unlike `debug_prompt_chrome.rs`, which scripts a `DebugPrompt` by hand, these
//! tests go through the calls a test makes (`page.breakpoint(..)`, the `pdb`
//! setting of the fixture), so the prompt is the real one: it reads the
//! terminal, paints when the terminal allows, and lists the source of this very
//! file. They need a terminal and a Chrome, so they are `#[ignore]`d. Run one in
//! a terminal and type at the prompt:
//!
//! ```text
//! cargo test --test debug_prompt_terminal_chrome --features testing -- \
//!     --ignored --nocapture --test-threads 1 a_breakpoint_lists_the_code
//! ```
//!
//! or give it a script through a pseudo terminal (`script` on macOS and Linux):
//!
//! ```text
//! printf 'l\nw\nc\n' | script -q /dev/null cargo test --test debug_prompt_terminal_chrome \
//!     --features testing -- --ignored --nocapture --test-threads 1 a_breakpoint_lists_the_code
//! ```
//!
//! `NO_COLOR=1` in front of it gives the plain text.
#![cfg(feature = "testing")]

use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::testing::SbCdp;

const PAGE: &str = "data:text/html,<title>Demo</title><p id=hi>hello from the page</p>";

#[tokio::test]
#[ignore = "needs a terminal and a Chrome"]
async fn a_breakpoint_lists_the_code_around_it() {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build().unwrap())
        .await
        .expect("Chrome starts");
    let page = browser.default_page().await.expect("a tab");
    page.goto(PAGE).await.expect("the page loads");

    page.breakpoint("Paused on the demo page.")
        .await
        .expect("the prompt ends with c");

    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "needs a terminal and a Chrome"]
async fn a_failing_test_ends_at_a_post_mortem() {
    let result = SbCdp::from_env()
        .configure(|settings| {
            settings.headless = true;
            settings.test.pdb = true;
        })
        .run(async |page| {
            page.goto(PAGE).await?;
            let page = page.with_timeout(Duration::from_secs(2));
            page.locator("#hi").click().await?;
            page.locator("#go").click().await?;
            Ok(())
        })
        .await;

    assert!(result.is_err(), "the button is not on the page");
}
