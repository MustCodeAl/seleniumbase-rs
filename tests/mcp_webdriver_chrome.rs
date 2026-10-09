//! The `sb` MCP server driving a real Chrome through chromedriver.
//!
//! Needs Chrome and a matching chromedriver, so it is ignored by default:
//!
//! ```text
//! cargo test --features mcp-server --test mcp_webdriver_chrome -- --ignored
//! ```

#![cfg(feature = "mcp-server")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::mcp::{sb, Host, Output, Settings, ToolError};
use seleniumbase_rs::BaseCase;
use serde_json::{json, Map, Value};

const PAGE: &str = r##"<!doctype html>
<html><head><title>WD Fixture</title></head>
<body>
<h1 id="title">Welcome</h1>
<form onsubmit="document.title='submitted:'+document.getElementById('user').value;return false;">
  <input id="user" name="user"> <button id="go" type="submit">Go</button>
</form>
<select id="country"><option value="us">United States</option><option value="de">Germany</option></select>
<ul><li>one</li><li>two</li><li>three</li></ul>
<iframe id="inner" srcdoc="<button id='inside' onclick='parent.document.title=&quot;clicked-inside&quot;'>In</button>"></iframe>
</body></html>"##;

fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 2048];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}")
}

async fn call(host: &Host<BaseCase>, tool: &str, v: Value) -> Result<Output, ToolError> {
    let arguments: Map<String, Value> = v.as_object().cloned().expect("an object");
    host.call(tool, arguments).await
}

async fn text_of(host: &Host<BaseCase>, tool: &str, v: Value) -> String {
    match call(host, tool, v).await {
        Ok(Output::Text(text)) => text,
        other => panic!("{tool} should answer with text, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_model_can_drive_a_page_through_the_sb_server() {
    let dir = tempfile::tempdir().unwrap();
    let host = sb::host(Settings::new(dir.path()));
    let base = serve();

    let started = text_of(
        &host,
        "start_browser",
        json!({ "headless": true, "uc": false }),
    )
    .await;
    assert!(
        started.starts_with("Started WebDriver session"),
        "{started}"
    );
    assert_eq!(
        text_of(&host, "start_browser", json!({})).await,
        "A browser session is already running. Call close_browser first."
    );

    text_of(&host, "open_url", json!({ "url": base })).await;
    assert_eq!(text_of(&host, "get_title", json!({})).await, "WD Fixture");
    assert_eq!(
        text_of(&host, "get_text", json!({ "selector": "#title" })).await,
        "Welcome"
    );

    text_of(
        &host,
        "type_text",
        json!({ "selector": "#user", "text": "demo_user" }),
    )
    .await;
    text_of(&host, "click_element", json!({ "selector": "#go" })).await;
    text_of(
        &host,
        "assert_title",
        json!({ "title": "submitted:demo_user" }),
    )
    .await;

    text_of(
        &host,
        "select_option_by_text",
        json!({ "dropdown_selector": "#country", "option": "Germany" }),
    )
    .await;
    let chosen = call(
        &host,
        "execute_script",
        json!({ "script": "return document.getElementById('country').value" }),
    )
    .await
    .unwrap();
    assert_eq!(chosen.as_json(), Some(&json!("de")));

    let count = call(&host, "find_elements_count", json!({ "selector": "li" }))
        .await
        .unwrap();
    assert_eq!(count.as_json(), Some(&json!(3)));
    text_of(
        &host,
        "click_nth_visible_element",
        json!({ "selector": "li", "number": 2 }),
    )
    .await;

    text_of(
        &host,
        "nested_click",
        json!({ "parent_selector": "#inner", "selector": "#inside" }),
    )
    .await;
    text_of(&host, "assert_title", json!({ "title": "clicked-inside" })).await;

    let missing = call(&host, "is_element_present", json!({ "selector": "#nope" }))
        .await
        .unwrap();
    assert_eq!(missing.as_json(), Some(&json!(false)));
    let gone = call(
        &host,
        "assert_element",
        json!({ "selector": "#nope", "timeout": 1 }),
    )
    .await;
    assert!(gone.is_err(), "asserting a missing element must fail");

    text_of(&host, "save_screenshot", json!({})).await;
    let png = std::fs::read(dir.path().join("screenshot.png")).unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

    assert_eq!(
        text_of(&host, "close_browser", json!({})).await,
        "Browser closed."
    );
}
