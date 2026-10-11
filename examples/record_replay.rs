//! Record browser interactions, save them, and replay them in a fresh session.
//!
//! `RecorderSession` drives a `BaseCase` and writes down what it does: each
//! `goto`, `type_text`, `click` and `assert_text` becomes a `RecordedAction`
//! with a name, a target and a value. (`install` and `drain` also capture what
//! a person does by hand in the page.) The actions can be saved as JSON or YAML
//! and replayed against a brand-new browser with `RecorderSession::replay`,
//! which returns a report of every step.
//!
//! The example records four actions on a page it serves from `127.0.0.1`,
//! checks them, replays them in a second browser, and fails unless every step
//! of the replay passes.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example record_replay
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbase doctor`
//! checks both) and does not need the network: the page is served by
//! `local_site/mod.rs`. The browser runs headless.

mod local_site;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::api::recorder::RecorderSession;
use seleniumbase_rs::{BaseCase, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    let config = BrowserConfig::default();

    // ----- Record -----
    let mut sb = BaseCase::new(config.clone()).await?;
    let recorded = record(&mut sb, &url).await;
    let closed = sb.quit().await;
    let actions = recorded?.into_actions();
    closed?;

    println!("Recorded {} action(s)", actions.len());
    for action in &actions {
        println!("  {} {:?}", action.name, action.target);
    }
    ensure(
        actions.len() >= 4,
        "the open, type, click and assert were all recorded",
    )?;

    // ----- Replay -----
    let mut replay_sb = BaseCase::new(config).await?;
    let replayed = RecorderSession::replay(&mut replay_sb, &actions).await;
    let closed = replay_sb.quit().await;
    let report = replayed?;
    closed?;

    for step in &report.steps {
        match &step.error {
            Some(error) => println!("FAIL  {} -> {error}", step.description),
            None => println!("ok    {}", step.description),
        }
    }
    println!(
        "{} passed, {} failed, {} skipped",
        report.passed(),
        report.failed(),
        report.skipped
    );
    ensure(
        report.failed() == 0 && report.passed() > 0,
        "every replayed step passed",
    )
}

/// Drives the page once, recording each step.
async fn record(sb: &mut BaseCase, url: &str) -> Result<RecorderSession> {
    let mut session = RecorderSession::new();
    session.goto(sb, url).await?;
    session.install(sb).await?;
    session.type_text(sb, "#myTextInput", "recorded").await?;
    session.click(sb, "#myButton").await?;
    session.assert_text(sb, "#pText", "Purple").await?;
    session.drain(sb).await?;
    Ok(session)
}
