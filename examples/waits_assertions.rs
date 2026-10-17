//! Explicit waits and assertions, including deferred ("soft") assertions.
//!
//! Pages change after they load, so a test should wait for the thing it needs
//! rather than sleep and hope. The page here adds an element 700 ms after it
//! loads, and the example waits for it with `wait_for_element_visible` and
//! `wait_for_text`, then waits for an element to be removed with
//! `wait_for_element_absent`. After that it shows the assertion families
//! (text, element, title), what a failed assertion looks like, and deferred
//! assertions: queue several checks, run them together, and get every failure
//! in one error instead of stopping at the first.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example waits_assertions
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result, SeleniumBaseError};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    run_browser_test(BrowserConfig::default(), |sb| Box::pin(demo(sb, url))).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    // Explicit waits. The timeout is in seconds, and each wait returns as soon
    // as its condition holds.
    sb.wait_for_element_present("body", 10).await?;
    sb.wait_for_element_visible("h1", 10).await?;
    sb.wait_for_element_visible("#late", 10).await?;
    sb.wait_for_text("Loaded after a delay", "#late", 10)
        .await?;

    // Wait for something to go away: remove the element shortly from now.
    sb.execute_script("setTimeout(() => document.getElementById('late').remove(), 300)")
        .await?;
    sb.wait_for_element_absent("#late", 10).await?;

    // Text assertions. The text comes first, then where to look for it.
    sb.assert_text_visible("Demo Page", "h1").await?;
    sb.assert_text_not_visible("Error", "body").await?;

    // Element assertions.
    sb.assert_element_present("#myButton").await?;
    sb.assert_element_visible("#myButton").await?;
    sb.assert_element_absent("#late").await?;

    // Title assertions.
    sb.assert_title_contains("Testing").await?;

    // A failed assertion is an ordinary error value you can inspect. The text
    // assertions wait for the default timeout before they fail, so shorten it
    // for a check that is meant to fail.
    sb.set_default_timeout(1);
    let failed = sb.assert_text("Not on this page", "h1").await;
    sb.reset_default_timeout();
    match failed {
        Err(SeleniumBaseError::AssertionFailed(message)) => {
            println!("failed as expected: {message}");
        }
        other => {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "expected an assertion failure, got {other:?}"
            )));
        }
    }

    // Deferred assertions: queue them, then check them all at once. Here one
    // of the three is wrong, and the error reports just that one.
    sb.deferred_assert_element("#myTextInput").await?;
    sb.deferred_assert_text("This Text is Green", "#pText")
        .await?;
    sb.deferred_assert_element("#no-such-element").await?;
    let Err(error) = sb.process_deferred_asserts().await else {
        return Err(SeleniumBaseError::AssertionFailed(
            "one deferred assertion is wrong, so processing should have failed".to_owned(),
        ));
    };
    let summary = error.to_string();
    println!("{summary}");
    ensure(
        summary.contains("1 deferred assertion(s) failed") && summary.contains("#no-such-element"),
        "the deferred error names the single failure",
    )?;

    // With only correct assertions queued, processing succeeds.
    sb.deferred_assert_element("#myTextInput").await?;
    sb.deferred_assert_text("This Text is Green", "#pText")
        .await?;
    sb.process_deferred_asserts().await?;
    println!("deferred assertions passed");
    Ok(())
}
