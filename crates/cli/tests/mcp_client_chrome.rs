//! A real Chrome driven from the command line: one `sbrs-mcp batch --url` session starts the
//! browser, snapshots a page and clicks the button by its `ref=`, which only works because the
//! calls share one MCP session. `#[ignore]`d because it launches Chrome; run it alone with
//! `cargo test -p seleniumbase-rs-cli --test mcp_client_chrome -- --ignored`.

#![cfg(feature = "mcp-client")]

#[path = "support/mcp_http.rs"]
mod support;

use std::process::Stdio;

use serde_json::{json, Value};
use support::Server;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

const PAGE: &str =
    "data:text/html,<title>before</title><button onclick=\"document.title='clicked'\">Go</button>";

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_batch_over_http_clicks_a_button_by_its_ref_in_one_session() {
    let server = Server::start(&["--server", "cdp", "--listen", "127.0.0.1:0"], &[]).await;
    let calls = [
        json!({ "tool": "start_browser", "arguments": { "headless": true } }),
        json!({ "tool": "open_url", "arguments": { "url": PAGE } }),
        json!({ "tool": "page_snapshot", "arguments": {} }),
        json!({ "tool": "click_element", "arguments": { "selector": "ref=e1" } }),
        json!({ "tool": "get_page_info", "arguments": {} }),
        json!({ "tool": "close_browser", "arguments": {} }),
    ]
    .map(|call| call.to_string())
    .join("\n");

    let mut child = Command::new(env!("CARGO_BIN_EXE_sbrs-mcp"))
        .args(["batch", "--raw", "--url", &server.url])
        .env_remove("SB_MCP_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("start sbrs-mcp");
    let mut input = child.stdin.take().expect("standard input");
    input
        .write_all(calls.as_bytes())
        .await
        .expect("write the calls");
    drop(input);
    let output = timeout(support::STEP, child.wait_with_output())
        .await
        .expect("the batch ended in time")
        .expect("run sbrs-mcp");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    let results: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line per call"))
        .collect();
    assert_eq!(results.len(), 6, "{stdout}");
    let snapshot = results[2]["text"].as_str().expect("the snapshot text");
    assert!(snapshot.contains("e1"), "{snapshot}");
    let info: Value =
        serde_json::from_str(results[4]["text"].as_str().expect("text")).expect("JSON text");
    assert_eq!(info["title"], "clicked", "{info}");
}
