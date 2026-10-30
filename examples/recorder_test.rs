//! Record a test as it runs, then generate Rust code from the recording.
//!
//! `BaseCase` logs every action it performs (open, click, type, ...).
//! `start_recording` marks where a recording begins and `stop_recording`
//! returns the actions since then. An `ActionRecorder` turns those actions
//! into source code: `to_rust_script` for a standalone `main`, and
//! `to_rust_test` for a `#[tokio::test]` function. `save_recording_to_logs`
//! writes both the JSON and the Rust form to `latest_logs/` in one call.
//!
//! The example performs a few actions on a local page, prints the code the
//! recording generates, and checks that the code contains each action. The
//! interactive recorder, which captures a person's clicks in the browser, is
//! `sbrs recorder`; the `record_replay` example shows replaying a recording.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example recorder_test
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::api::recorder::ActionRecorder;
use seleniumbase_rs::{
    run_browser_test, tokio, BaseCase, BrowserConfig, Result, SeleniumBaseError,
};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    run_browser_test(BrowserConfig::default(), async |sb| demo(sb, url).await).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.start_recording();
    sb.open(&url).await?;
    // The human_* methods type and click at a human rhythm; they are recorded
    // under their own names so a replay keeps the same pace.
    sb.human_type("#myTextInput", "Rustlang").await?;
    sb.human_click("#myButton").await?;
    sb.assert_text("Purple", "#pText").await?;

    let actions = sb.stop_recording()?;
    println!("recorded {} actions:", actions.len());
    for action in &actions {
        println!("  {} {:?} {:?}", action.name, action.target, action.value);
    }

    let recorder = ActionRecorder { actions };
    let test = recorder.to_rust_test("demo_page_flow");
    println!("\nGenerated test:\n{test}");

    // Every action must have made it into the generated code.
    let open = format!("sb.open({url:?})");
    for expected in [
        open.as_str(),
        "sb.human_type(\"#myTextInput\", \"Rustlang\")",
        "sb.human_click(\"#myButton\")",
        "sb.assert_text(\"Purple\", \"#pText\")",
        "async fn demo_page_flow()",
    ] {
        if !test.contains(expected) {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "the generated test is missing `{expected}`"
            )));
        }
    }

    // Both forms of the whole session, written in one call.
    let (json, rust) = sb.save_recording_to_logs()?;
    println!("saved {} and {}", json.display(), rust.display());
    Ok(())
}
