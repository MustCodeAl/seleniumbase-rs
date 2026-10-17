//! MasterQA: automate the setup, let a human make the judgement calls, and keep
//! a report of the answers.
//!
//! The script opens a page in a **visible** browser, then asks a person to
//! verify what they see. `manual_verify` prints the question in the terminal
//! and waits for `Y` or Enter (pass) or `n` (fail). Each answer is recorded in a
//! QA session, and `save_qa_report` writes the session as a Markdown table to
//! `latest_logs/`. The run fails if any step did not pass; when input is closed
//! (for instance in CI) an unanswered step counts as failed, so an unattended
//! run can never pass its manual checks.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example masterqa
//! ```
//!
//! To try it without a keyboard, pipe in the answers (a browser window still
//! opens briefly):
//!
//! ```bash
//! printf 'y\ny\n' | cargo run --example masterqa
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, Result, SeleniumBaseError};

const TITLE: &str = "Demo page manual verification";

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    // A person has to see the page, so the browser must not be headless.
    let config = BrowserConfig::default().with_headless(false);
    run_browser_test(config, async |sb| verify_by_hand(sb, url).await).await
}

async fn verify_by_hand(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.start_qa_session(TITLE);

    sb.open(&url).await?;
    sb.highlight("h1").await?;
    sb.manual_verify("The demo page loaded and its heading is highlighted");
    sb.manual_verify("The green sentence under the heading is readable");

    let session = sb
        .qa_session()
        .ok_or_else(|| SeleniumBaseError::Unsupported("the QA session was lost".to_owned()))?;
    let passed = session.all_passed();

    let report = sb.save_qa_report()?;
    println!("Report written to: {}", report.display());

    if passed {
        Ok(())
    } else {
        Err(SeleniumBaseError::AssertionFailed(
            "at least one manual verification failed".to_owned(),
        ))
    }
}
