//! The developer tools of the Pure CDP MCP server in a real Chrome.
//!
//! `tests/mcp_devtools.rs` checks what the server does with the page script's
//! answers; these tests run the script itself against a fixture page with
//! the cases it must recognise: a control in a same-origin iframe and one in a
//! shadow root, a hidden, a disabled and a covered button, a typo-sized near
//! miss, duplicate ids, an image without alt text, a missing script, and an
//! uncaught error. They need Chrome, so they are ignored by default:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test mcp_devtools_chrome -- --ignored
//! ```

#![cfg(feature = "mcp-server")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;
use std::time::Duration;

use seleniumbase_rs::mcp::cdp::{self, Cdp};
use seleniumbase_rs::mcp::{Host, Output, Settings, ToolError};
use serde_json::{json, Map, Value};

const PAGE: &str = r##"<!doctype html>
<html lang="en"><head><title>DevTools Fixture</title></head>
<body>
<h1>Shop</h1>
<h3>Skipped a level</h3>
<form id="order" action="/order" method="post">
  <label for="email">Email</label><input id="email" name="email" type="email">
  <input id="coupon" name="coupon">
  <button id="submit" type="submit" data-testid="place-order">Place order</button>
</form>
<button id="hidden-btn" style="display:none">Hidden</button>
<button id="disabled-btn" disabled>Disabled</button>
<div style="position:relative;height:80px">
  <button id="covered">Covered</button>
  <div id="overlay" class="cookie-banner"
       style="position:absolute;left:0;top:0;width:400px;height:60px;background:#fff">Cookies</div>
</div>
<span id="dup">a</span><span id="dup">b</span>
<img src="/logo.png">
<iframe id="pay" src="/frame"></iframe>
<div id="host"></div>
<script src="/missing.js"></script>
<script>
  document.getElementById("host").attachShadow({ mode: "open" }).innerHTML =
    '<button id="shadow-btn">Inside the shadow</button>';
  setTimeout(function () { throw new Error("boom from the fixture"); }, 0);
</script>
</body></html>"##;

const FRAME: &str = r##"<!doctype html>
<html><body><input id="card-number" placeholder="Card number"></body></html>"##;

/// Serves the fixture and its frame on a loopback port; anything else is 404.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]);
            let path = request.split_whitespace().nth(1).unwrap_or("/");
            let (status, body) = match path {
                "/" => ("200 OK", PAGE),
                "/frame" => ("200 OK", FRAME),
                _ => ("404 Not Found", "not here"),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}/")
}

fn arguments(v: Value) -> Map<String, Value> {
    v.as_object()
        .cloned()
        .expect("tool arguments are an object")
}

async fn call(host: &Host<Cdp>, tool: &str, v: Value) -> Result<Output, ToolError> {
    host.call(tool, arguments(v)).await
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
    call(
        &host,
        "start_browser",
        json!({ "headless": true, "url": base }),
    )
    .await
    .expect("Chrome starts");
    // The fixture throws from a timer; give it a moment.
    tokio::time::sleep(Duration::from_millis(300)).await;
    (host, base)
}

async fn diagnose(host: &Host<Cdp>, selector: &str) -> Value {
    json_of(host, "diagnose_selector", json!({ "selector": selector })).await
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_page_tools_understand_a_real_page() {
    let dir = tempfile::tempdir().unwrap();
    let (host, base) = started(dir.path()).await;

    // An outline with working selectors, frames and shadow hosts.
    let outline = json_of(&host, "inspect_page", json!({})).await;
    assert_eq!(outline["title"], "DevTools Fixture");
    assert_eq!(outline["counts"]["iframes"], 1, "{outline}");
    assert_eq!(outline["counts"]["shadow_hosts"], 1, "{outline}");
    let controls = outline["interactive"].as_array().unwrap();
    let submit = controls
        .iter()
        .find(|control| control["name"] == "Place order")
        .unwrap_or_else(|| panic!("no Place order control in {outline}"));
    assert_eq!(submit["role"], "button");
    assert_eq!(submit["selector"], "[data-testid=\"place-order\"]");
    let email = controls
        .iter()
        .find(|control| control["selector"] == "#email")
        .unwrap_or_else(|| panic!("no #email control in {outline}"));
    assert_eq!(email["name"], "Email", "the label names the input");
    assert!(controls
        .iter()
        .any(|control| control["in_shadow_root_of"].is_string()));
    assert!(
        !controls.iter().any(|control| control["name"] == "Hidden"),
        "hidden controls are left out unless asked for"
    );
    let disabled = controls
        .iter()
        .find(|control| control["name"] == "Disabled")
        .unwrap();
    assert_eq!(disabled["enabled"], false);
    assert_eq!(outline["headings"][0]["text"], "Shop");
    assert_eq!(outline["truncated"], false);
    let short = json_of(&host, "inspect_page", json!({ "limit": 1 })).await;
    assert_eq!(short["interactive"].as_array().unwrap().len(), 1);
    assert_eq!(short["truncated"], true);

    // Ranked selectors, each checked.
    let suggested = json_of(&host, "suggest_selectors", json!({ "text": "Place order" })).await;
    let best = &suggested["elements"][0]["selectors"][0];
    assert_eq!(
        best["selector"], "[data-testid=\"place-order\"]",
        "{suggested}"
    );
    assert_eq!(best["unique"], true);
    assert_eq!(best["finds_element"], true);

    // Why a selector fails.
    let hidden = diagnose(&host, "#hidden-btn").await;
    assert!(
        hidden["verdict"]
            .as_str()
            .unwrap()
            .contains("display: none"),
        "{hidden}"
    );
    let covered = diagnose(&host, "#covered").await;
    assert!(
        covered["verdict"]
            .as_str()
            .unwrap()
            .contains("covered by div#overlay.cookie-banner"),
        "{covered}"
    );
    let disabled = diagnose(&host, "#disabled-btn").await;
    assert!(
        disabled["verdict"].as_str().unwrap().contains("disabled"),
        "{disabled}"
    );
    let framed = diagnose(&host, "#card-number").await;
    assert!(
        framed["verdict"].as_str().unwrap().contains("frame"),
        "{framed}"
    );
    let shadowed = diagnose(&host, "#shadow-btn").await;
    assert!(
        shadowed["verdict"]
            .as_str()
            .unwrap()
            .contains("shadow root"),
        "{shadowed}"
    );
    let typo = diagnose(&host, "#submitt").await;
    assert_eq!(typo["near_misses"][0]["try"], "#submit", "{typo}");
    let broken = diagnose(&host, "div[").await;
    assert!(broken["syntax_error"].is_string(), "{broken}");
    let fine = diagnose(&host, "#submit").await;
    assert!(
        fine["verdict"]
            .as_str()
            .unwrap()
            .starts_with("The selector works"),
        "{fine}"
    );

    // A failed call carries the page with it.
    let report = host
        .call_explained(
            "click_element",
            arguments(json!({ "selector": "#nope", "timeout": 0.5 })),
        )
        .await
        .unwrap_err();
    assert_eq!(report.kind, "element_not_found");
    let page = report.page.clone().expect("the page was read");
    assert_eq!(page["title"], "DevTools Fixture");
    assert_eq!(page["url"], base);
    assert_eq!(page["selector"]["matches"], 0);
    assert!(
        page["javascript_errors"]
            .to_string()
            .contains("boom from the fixture"),
        "{page}"
    );

    // The audit finds the defects the fixture has.
    let audit = json_of(&host, "audit_page", json!({})).await;
    let rules = audit["by_rule"].as_object().unwrap();
    for rule in [
        "image-alt",
        "duplicate-id",
        "form-control-label",
        "heading-order",
    ] {
        assert!(rules.contains_key(rule), "{rule} missing from {audit}");
    }

    call(&host, "close_browser", json!({})).await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn console_messages_and_failed_requests_are_collected() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _base) = started(dir.path()).await;

    let errors = json_of(&host, "get_console_messages", json!({})).await;
    assert!(
        errors["page_errors"]
            .to_string()
            .contains("boom from the fixture"),
        "{errors}"
    );
    let started = json_of(&host, "get_console_messages", json!({ "start": true })).await;
    assert_eq!(started["recording"], true);
    call(
        &host,
        "run_javascript",
        json!({ "expression": "console.error('saved in the cart'); 1" }),
    )
    .await
    .unwrap();
    let messages = json_of(&host, "get_console_messages", json!({ "level": "error" })).await;
    assert_eq!(
        messages["entries"][0]["text"], "saved in the cart",
        "{messages}"
    );

    let timeline = json_of(&host, "get_failed_requests", json!({ "watch": true })).await;
    assert!(
        timeline["from_performance_timeline"]["failed"]
            .to_string()
            .contains("/missing.js"),
        "{timeline}"
    );
    call(&host, "manage_history", json!({ "action": "reload" }))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let recorded = json_of(&host, "get_failed_requests", json!({})).await;
    let failures = recorded["recorded"]["failures"].to_string();
    assert!(
        failures.contains("/missing.js") && failures.contains("404"),
        "{recorded}"
    );

    call(&host, "close_browser", json!({})).await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_session_becomes_a_bundle_and_a_test() {
    let dir = tempfile::tempdir().unwrap();
    let (host, base) = started(dir.path()).await;
    call(
        &host,
        "type_text",
        json!({ "selector": "#email", "text": "buyer@example.test" }),
    )
    .await
    .unwrap();

    let saved = json_of(&host, "capture_debug_bundle", json!({ "name": "fixture" })).await;
    let folder = Path::new(saved["folder"].as_str().unwrap());
    let png = std::fs::read(folder.join("screenshot.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG"), "a real screenshot");
    let bundle = std::fs::read_to_string(folder.join("bundle.json")).unwrap();
    assert!(!bundle.contains("buyer@example.test"), "typed text leaked");
    let source = std::fs::read_to_string(folder.join("page.html")).unwrap();
    assert!(source.contains("DevTools Fixture"));

    let exported = json_of(
        &host,
        "export_session_as_test",
        json!({ "test_name": "fixture_session" }),
    )
    .await;
    let code = exported["code"].as_str().unwrap();
    assert!(
        code.contains("sb.type_text(\"#email\", \"buyer@example.test\").await?;"),
        "{code}"
    );
    assert!(
        !code.contains(&base),
        "start_browser's own URL is not a step"
    );

    call(&host, "close_browser", json!({})).await.unwrap();
}
