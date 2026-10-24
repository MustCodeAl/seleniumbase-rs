//! The fingerprint tools in a real Chrome, on both engines.
//!
//! `tests/mcp_fingerprint.rs` checks what the tools make of a recorded reading;
//! these run the capture script itself. They need Chrome (the `sb` test also needs
//! chromedriver), so they are ignored by default; run one binary at a time:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test mcp_fingerprint_chrome -- --ignored --test-threads=1
//! ```

#![cfg(feature = "mcp-server")]

use seleniumbase_rs::mcp::{cdp, sb, Host, Output, Settings, ToolError};
use serde_json::{json, Map, Value};

fn arguments(v: Value) -> Map<String, Value> {
    v.as_object()
        .cloned()
        .expect("tool arguments are an object")
}

async fn call<S: Send + 'static>(
    host: &Host<S>,
    tool: &str,
    v: Value,
) -> Result<Output, ToolError> {
    host.call(tool, arguments(v)).await
}

/// The text and the structured content of a tool that answers with both.
async fn both<S: Send + 'static>(host: &Host<S>, tool: &str, v: Value) -> (String, Value) {
    match call(host, tool, v).await {
        Ok(Output::Structured { text, data }) => (text, data),
        other => panic!("{tool} should answer with text and data, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "needs Chrome"]
async fn the_pure_cdp_engine_captures_compares_and_explains() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));
    call(&host, "start_browser", json!({ "headless": true }))
        .await
        .unwrap();

    let (_, captured) = both(
        &host,
        "capture_fingerprint",
        json!({ "url": "about:blank", "save_to": "real.json" }),
    )
    .await;
    assert!(
        captured["fingerprint"]["user_agent"].is_string(),
        "{captured}"
    );
    assert!(
        captured["settings_read"].as_u64().unwrap() > 10,
        "{captured}"
    );

    // The browser presents what it just showed.
    let (text, same) = both(&host, "compare_fingerprint", json!({ "path": "real.json" })).await;
    assert_eq!(same["presents"], true, "{text}");

    // It does not present an iPhone.
    let (text, ios) = both(&host, "compare_fingerprint", json!({ "preset": "ios" })).await;
    assert_eq!(ios["presents"], false, "{text}");
    assert!(!ios["differences"].as_array().unwrap().is_empty());

    let (text, explained) = both(&host, "explain_fingerprint", json!({})).await;
    assert!(explained["risk"].is_string(), "{text}");
    assert!(explained["detector_view"]["user_agent"].is_string());

    call(&host, "close_browser", json!({})).await.unwrap();
}

#[tokio::test]
#[ignore = "needs Chrome"]
async fn the_pure_cdp_engine_applies_a_preset_at_start() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));

    let started = call(
        &host,
        "start_browser",
        json!({ "headless": true, "fingerprint_preset": "macos" }),
    )
    .await
    .unwrap();
    assert!(
        started.as_text().unwrap().contains("Fingerprint applied"),
        "{started:?}"
    );

    let (_, captured) = both(
        &host,
        "capture_fingerprint",
        json!({ "url": "about:blank" }),
    )
    .await;
    assert_eq!(
        captured["fingerprint"]["platform"], "MacIntel",
        "{captured}"
    );
    assert!(captured["fingerprint"]["user_agent"]
        .as_str()
        .unwrap()
        .contains("Macintosh"));

    call(&host, "close_browser", json!({})).await.unwrap();
}

#[tokio::test]
#[ignore = "needs Chrome and chromedriver"]
async fn the_webdriver_engine_captures_and_compares() {
    let dir = tempfile::tempdir().unwrap();
    let host = sb::host(Settings::new(dir.path()));
    call(&host, "start_browser", json!({ "headless": true }))
        .await
        .unwrap();

    let (_, captured) = both(
        &host,
        "capture_fingerprint",
        json!({ "url": "about:blank", "save_to": "driven.json" }),
    )
    .await;
    assert!(
        captured["fingerprint"]["user_agent"].is_string(),
        "{captured}"
    );

    let (text, same) = both(
        &host,
        "compare_fingerprint",
        json!({ "path": "driven.json" }),
    )
    .await;
    assert_eq!(same["presents"], true, "{text}");
    let (text, ios) = both(&host, "compare_fingerprint", json!({ "preset": "ios" })).await;
    assert_eq!(ios["presents"], false, "{text}");

    call(&host, "close_browser", json!({})).await.unwrap();
}
