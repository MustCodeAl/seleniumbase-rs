//! Record a test's browser calls, save them, and replay them in a fresh session.
//!
//! `BaseCase` writes down what a test does: each `open`, `type_text`, `click`
//! and `assert_text` becomes a `RecordedAction` with a name, a target and a
//! value. `start_recording` marks where the recording begins and
//! `recorder_session` returns what was recorded since as a `RecorderSession`.
//! The session is saved as JSON (YAML works the same), loaded again, and
//! replayed against a brand-new browser with `RecorderSession::replay`, which
//! returns a report of every step.
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
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served by
//! `local_site/mod.rs`. The browser runs headless.

mod local_site;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::api::recorder::RecorderSession;
use seleniumbase_rs::{tokio, BaseCase, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    let config = BrowserConfig::default();

    // ----- Record -----
    let mut sb = BaseCase::new(config.clone()).await?;
    let recorded = record(&mut sb, &url).await;
    let closed = sb.quit().await;
    let session = recorded?;
    closed?;

    println!("Recorded {} action(s)", session.len());
    for action in session.actions() {
        println!("  {} {:?}", action.name, action.target);
    }
    ensure(
        session.len() >= 4,
        "the open, type, click and assert were all recorded",
    )?;

    // ----- Save and load -----
    let file = std::env::temp_dir().join("record_replay_example.json");
    session.save(&file)?;
    let loaded = RecorderSession::load(&file);
    std::fs::remove_file(&file)?;
    let actions = loaded?;

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
        report.failed() == 0 && report.skipped == 0 && report.passed() > 0,
        "every replayed step passed",
    )
}

/// Drives the page once and returns what `BaseCase` recorded.
async fn record(sb: &mut BaseCase, url: &str) -> Result<RecorderSession> {
    sb.start_recording();
    sb.open(url).await?;
    sb.type_text("#myTextInput", "recorded").await?;
    sb.click("#myButton").await?;
    sb.assert_text("Purple", "#pText").await?;
    sb.recorder_session()
}
