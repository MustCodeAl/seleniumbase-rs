//! Record browser interactions and replay them in a fresh session.
//!
//! Run with: `cargo run --example record_replay`

use seleniumbase_rs::api::recorder::RecorderSession;
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig {
        headless: true,
        ..Default::default()
    };

    // ----- Record -----
    let mut sb = BaseCase::new(config.clone()).await?;
    let mut session = RecorderSession::new();
    session
        .goto(&mut sb, "https://seleniumbase.io/demo_page")
        .await?;
    session.install(&sb).await?;
    session
        .type_text(&mut sb, "#myTextInput", "recorded")
        .await?;
    session.click(&mut sb, "#myButton").await?;
    session.assert_text(&mut sb, "#pText", "This").await?;
    session.drain(&mut sb).await?;
    sb.quit().await?;

    let actions = session.into_actions();
    println!("Recorded {} action(s)", actions.len());
    for action in &actions {
        println!("  {} {:?}", action.name, action.target);
    }

    // ----- Replay -----
    let mut replay_sb = BaseCase::new(config).await?;
    let report = RecorderSession::replay(&mut replay_sb, &actions).await?;
    replay_sb.quit().await?;

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

    Ok(())
}
