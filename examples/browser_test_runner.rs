//! `run_browser_test`: run a browser test and always close the browser.
//!
//! `BaseCase::new` starts a real browser and chromedriver. If a test returns
//! early with `?` and nothing closes them, they can outlive the test, and
//! `Drop` cannot await the shutdown. `run_browser_test` wraps the whole
//! lifecycle: it starts the browser, runs your async test body, closes the
//! browser whether the body passed or failed, and returns the body's error (or
//! the cleanup error, or both together if both failed).
//!
//! The example runs two tests through it: one that passes, and one whose
//! assertion fails on purpose, to show that the failure comes back as a normal
//! `Err` and the browser is closed either way. It exits with an error only if
//! the passing test fails or the failing test does not.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example browser_test_runner
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result, SeleniumBaseError};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);

    // A test that passes.
    let passing_url = url.clone();
    run_browser_test(BrowserConfig::default(), async |sb| {
        sb.open(&passing_url).await?;
        sb.assert_title("Web Testing Page").await
    })
    .await?;
    println!("passing test: ok");

    // A test that fails on purpose. The error is returned, not lost, and the
    // browser has already been closed by the time it arrives.
    let outcome = run_browser_test(BrowserConfig::default(), async |sb| {
        sb.open(&url).await?;
        sb.assert_title("A title this page does not have").await
    })
    .await;
    match outcome {
        Err(SeleniumBaseError::AssertionFailed(message)) => {
            println!("failing test: failed as intended ({message})");
            Ok(())
        }
        other => Err(SeleniumBaseError::AssertionFailed(format!(
            "the failing test should have returned an assertion error, got {other:?}"
        ))),
    }
}
