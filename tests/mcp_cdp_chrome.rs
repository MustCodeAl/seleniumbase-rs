//! The Pure CDP MCP server driving a real Chrome, tool call by tool call.
//!
//! These tests need a Chrome or Chromium on the machine, so they are ignored
//! by default. Run them with:
//!
//! ```text
//! cargo test --features mcp-server --test mcp_cdp_chrome -- --ignored
//! ```

#![cfg(feature = "mcp-server")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;

use seleniumbase_rs::mcp::cdp::{self, Cdp};
use seleniumbase_rs::mcp::{Host, Output, Settings, ToolError};
use serde_json::{json, Map, Value};

const PAGE: &str = r##"<!doctype html>
<html><head><title>MCP Fixture</title></head>
<body>
<h1 id="title">Welcome</h1>
<form onsubmit="document.title='submitted:'+document.getElementById('user').value;return false;">
  <input id="user" name="user"> <button id="go" type="submit">Go</button>
</form>
<a href="/about">About</a> <a href="#top">Top</a> <img src="/logo.png">
<select id="country">
  <option value="us">United States</option><option value="de">Germany</option>
</select>
<ul><li>one</li><li>two</li><li>three</li></ul>
<div class="cf-turnstile" style="width:300px;height:65px;background:#eee;margin-top:40px"
     onclick="window.hit=[Math.round(event.offsetX),Math.round(event.offsetY),event.isTrusted]"></div>
</body></html>"##;

/// Serves the fixture on a loopback port and sets a cookie on every response.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: sid=abc123; Path=/\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}")
}

fn arguments(v: Value) -> Map<String, Value> {
    v.as_object()
        .cloned()
        .expect("tool arguments are an object")
}

async fn call(host: &Host<Cdp>, tool: &str, v: Value) -> Result<Output, ToolError> {
    host.call(tool, arguments(v)).await
}

async fn text_of(host: &Host<Cdp>, tool: &str, v: Value) -> String {
    match call(host, tool, v).await {
        Ok(Output::Text(text)) => text,
        other => panic!("{tool} should answer with text, got {other:?}"),
    }
}

async fn json_of(host: &Host<Cdp>, tool: &str, v: Value) -> Value {
    match call(host, tool, v).await {
        Ok(Output::Json(value)) => value,
        other => panic!("{tool} should answer with JSON, got {other:?}"),
    }
}

async fn started(output_dir: &Path) -> (Host<Cdp>, String) {
    let base = serve();
    let host = cdp::host(Settings::new(output_dir));
    let answer = text_of(
        &host,
        "start_browser",
        json!({ "headless": true, "url": base }),
    )
    .await;
    assert!(
        answer.starts_with("Started Pure CDP Mode browser"),
        "{answer}"
    );
    (host, base)
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_model_can_fill_a_form_read_the_page_and_check_the_result() {
    let dir = tempfile::tempdir().unwrap();
    let (host, base) = started(dir.path()).await;

    let info = json_of(&host, "get_page_info", json!({})).await;
    assert_eq!(info["title"], "MCP Fixture");
    assert!(info["url"].as_str().unwrap().starts_with(&base));
    assert!(info["user_agent"].as_str().unwrap().contains("Chrome"));

    text_of(
        &host,
        "type_text",
        json!({ "selector": "#user", "text": "demo_user" }),
    )
    .await;
    text_of(&host, "click_element", json!({ "selector": "#go" })).await;
    text_of(
        &host,
        "assert_condition",
        json!({ "check": "title", "expected": "submitted:demo_user" }),
    )
    .await;

    text_of(
        &host,
        "select_option",
        json!({ "dropdown_selector": "#country", "value": "Germany" }),
    )
    .await;
    let chosen = json_of(
        &host,
        "run_javascript",
        json!({ "expression": "document.getElementById('country').value" }),
    )
    .await;
    assert_eq!(chosen, "de");

    let found = json_of(
        &host,
        "find_elements",
        json!({ "selector": "li", "include_html": true }),
    )
    .await;
    assert_eq!(found["count"], 3);
    assert_eq!(found["matches"][1]["text"], "two");
    assert_eq!(found["matches"][1]["html"], "<li>two</li>");

    text_of(
        &host,
        "click_element",
        json!({ "selector": "li", "nth": 3 }),
    )
    .await;
    let missing = json_of(
        &host,
        "check_if_condition",
        json!({ "check": "present", "selector": "#nope" }),
    )
    .await;
    assert_eq!(missing, json!(false));

    let urls = json_of(&host, "get_content", json!({ "output_format": "urls" })).await;
    assert_eq!(
        urls,
        json!([format!("{base}/about"), format!("{base}/logo.png")])
    );

    assert_eq!(
        text_of(&host, "close_browser", json!({})).await,
        "The browser session was closed."
    );
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn tabs_cookies_and_saved_pages_work_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let (host, base) = started(dir.path()).await;

    // A second tab, then back to the first by number.
    text_of(
        &host,
        "manage_tabs",
        json!({ "action": "open_new_tab", "url": format!("{base}/second") }),
    )
    .await;
    let tabs = json_of(&host, "manage_tabs", json!({})).await;
    assert_eq!(tabs.as_array().unwrap().len(), 2);
    text_of(
        &host,
        "manage_tabs",
        json!({ "action": "switch_to_tab", "tab_index": 0 }),
    )
    .await;
    let on = json_of(&host, "get_page_info", json!({})).await;
    assert_eq!(on["url"], tabs[0]["url"]);
    text_of(
        &host,
        "manage_tabs",
        json!({ "action": "close_active_tab" }),
    )
    .await;
    assert_eq!(
        json_of(&host, "manage_tabs", json!({}))
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Cookies survive a save, a clear and a load.
    let before = json_of(&host, "manage_cookies", json!({})).await;
    assert_eq!(before[0]["name"], "sid");
    text_of(
        &host,
        "manage_cookies",
        json!({ "action": "save", "filename": "session" }),
    )
    .await;
    text_of(&host, "manage_cookies", json!({ "action": "clear" })).await;
    assert_eq!(json_of(&host, "manage_cookies", json!({})).await, json!([]));
    text_of(
        &host,
        "manage_cookies",
        json!({ "action": "load", "filename": "session" }),
    )
    .await;
    assert_eq!(
        json_of(&host, "manage_cookies", json!({})).await[0]["value"],
        "abc123"
    );

    // Pages are saved where the server was told to put them.
    text_of(&host, "save_page", json!({})).await;
    text_of(
        &host,
        "save_page",
        json!({ "format": "html", "folder": "pages" }),
    )
    .await;
    text_of(&host, "save_page", json!({ "format": "pdf" })).await;
    let png = std::fs::read(dir.path().join("screenshot.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    let html = std::fs::read_to_string(dir.path().join("pages").join("page_source.html")).unwrap();
    assert!(html.contains("<h1 id=\"title\">Welcome</h1>"));
    let pdf = std::fs::read(dir.path().join("page.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF"));

    text_of(&host, "close_browser", json!({})).await;
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn solving_a_captcha_clicks_where_the_checkbox_is_with_a_trusted_event() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _base) = started(dir.path()).await;

    let answer = text_of(&host, "solve_captcha", json!({})).await;
    assert_eq!(answer, "Attempted to solve a Cloudflare Turnstile CAPTCHA.");

    let hit = json_of(
        &host,
        "run_javascript",
        json!({ "expression": "window.hit" }),
    )
    .await;
    let x = hit[0].as_i64().unwrap();
    let y = hit[1].as_i64().unwrap();
    assert!((x - 28).abs() <= 1, "pressed {x}px from the left edge");
    assert!((y - 33).abs() <= 1, "pressed {y}px from the top edge");
    assert_eq!(hit[2], true, "the click must be a trusted event");

    text_of(&host, "close_browser", json!({})).await;
}
