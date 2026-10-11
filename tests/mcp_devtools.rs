//! The developer tools and the failure reports of the Pure CDP MCP server,
//! against a scripted browser, with no Chrome.
//!
//! The page side of the tools is one script; these tests answer it by the
//! operation it is asked for and check what the server makes of the answer.
//! `tests/mcp_devtools_chrome.rs` runs the script itself in a real Chrome.
//! Run them with `cargo test --features "mcp-server test-util"`.

#![cfg(all(feature = "mcp-server", feature = "test-util"))]

use std::path::Path;
use std::time::Duration;

use seleniumbase_rs::mcp::cdp::{self, Cdp};
use seleniumbase_rs::mcp::{Host, Output, Settings, ToolError};
use seleniumbase_rs::sb_cdp::{Browser, MockCtrl};
use serde_json::{json, Map, Value};

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

async fn running(output_dir: &Path) -> (Host<Cdp>, MockCtrl) {
    let host = cdp::host(Settings::new(output_dir));
    let (browser, mock) = Browser::new_mocked();
    host.ctx()
        .start(async { Ok::<_, ToolError>(Cdp::attach(browser).await?) })
        .await
        .expect("the mocked session starts");
    (host, mock)
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

/// The developer-tools operation an expression runs, if it runs one.
fn operation(expression: &str) -> Option<String> {
    if !expression.contains("unknown developer-tools operation") {
        return None;
    }
    let start = expression.rfind("})(\"")? + 4;
    let end = start + expression[start..].find('"')?;
    Some(expression[start..end].to_owned())
}

/// Answers the developer-tools script with `answer(op, expression)`, the
/// engine's own error list with `errors`, and every locator with "nothing
/// found".
fn script_page(
    mock: &MockCtrl,
    errors: Value,
    answer: impl Fn(&str, &str) -> Value + Send + 'static,
) {
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(value(if let Some(op) = operation(expression) {
            answer(&op, expression)
        } else if expression.contains("__sbcdp.errors") {
            errors.clone()
        } else if expression.contains("resolve(") && expression.contains(".length === 0") {
            json!(true)
        } else if expression.contains("resolve(") && expression.contains(".length") {
            json!(0)
        } else {
            Value::Null
        }))
    });
}

// ----------------------------------------------------------------------
// Failure reports
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_failed_click_is_reported_with_the_page_and_the_tools_to_try_next() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(
        &mock,
        json!(["TypeError: cart is undefined"]),
        |op, _| match op {
            "state" => json!({
                "url": "https://shop.test/cart",
                "title": "Cart",
                "ready_state": "complete",
                "selector": { "value": "#buy", "matches": 0, "visible": 0 },
            }),
            _ => Value::Null,
        },
    );

    let report = host
        .call_explained(
            "click_element",
            arguments(json!({ "selector": "#buy", "timeout": 0.1 })),
        )
        .await
        .unwrap_err();

    assert_eq!(report.kind, "element_not_found");
    let page = report.page.as_ref().expect("the page was read");
    assert_eq!(page["url"], "https://shop.test/cart");
    assert_eq!(page["javascript_errors"][0], "TypeError: cart is undefined");
    assert!(
        report.next_steps[0].starts_with("diagnose_selector {\"selector\": \"#buy\"}"),
        "{:?}",
        report.next_steps
    );
    let text = report.render();
    assert!(
        text.starts_with("click_element failed: element not found: #buy"),
        "{text}"
    );
    assert!(
        text.contains("\nselector \"#buy\": 0 matching, 0 visible\n"),
        "{text}"
    );
    assert!(
        text.contains("\nhint: Nothing on the page matched"),
        "{text}"
    );
    assert_eq!(report.to_json()["kind"], "element_not_found");
}

#[tokio::test]
async fn a_bad_argument_is_reported_without_touching_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    let before = mock.calls_to("Runtime.evaluate").len();

    let report = host
        .call_explained("click_element", arguments(json!({})))
        .await
        .unwrap_err();

    assert_eq!(report.kind, "invalid_argument");
    assert!(report.page.is_none());
    assert_eq!(mock.calls_to("Runtime.evaluate").len(), before);
}

#[tokio::test]
async fn a_mistyped_choice_suggests_the_nearest_option() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(&host, "manage_history", json!({ "action": "bakc" }))
        .await
        .unwrap_err();

    assert!(
        error.to_string().ends_with("did you mean \"back\"?"),
        "{error}"
    );
}

// ----------------------------------------------------------------------
// Looking at the page
// ----------------------------------------------------------------------

#[tokio::test]
async fn inspect_page_passes_its_bounds_to_the_page_and_returns_the_outline() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, json!([]), |op, expression| match op {
        "inspect" => json!({
            "url": "https://shop.test/",
            "interactive": [{ "role": "button", "name": "Buy", "selector": "#buy" }],
            "limit_seen": expression.contains("\"limit\":7"),
            "scope_seen": expression.contains("{\"kind\":\"css\",\"value\":\"main\"}"),
        }),
        _ => Value::Null,
    });

    let outline = json_of(
        &host,
        "inspect_page",
        json!({ "limit": 7, "scope": "main" }),
    )
    .await;

    assert_eq!(outline["interactive"][0]["selector"], "#buy");
    assert_eq!(outline["limit_seen"], true);
    assert_eq!(outline["scope_seen"], true);
    for limit in [0, 501] {
        assert!(matches!(
            call(&host, "inspect_page", json!({ "limit": limit })).await,
            Err(ToolError::InvalidArgument { .. })
        ));
    }
}

#[tokio::test]
async fn a_scope_that_matches_nothing_is_an_argument_error() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(
        &mock,
        json!([]),
        |_, _| json!({ "error": "the scope selector matched nothing" }),
    );

    let error = call(&host, "inspect_page", json!({ "scope": "#none" }))
        .await
        .unwrap_err();

    assert!(
        matches!(&error, ToolError::InvalidArgument { name, .. } if name == "scope"),
        "{error}"
    );
}

#[tokio::test]
async fn diagnose_selector_says_the_element_is_in_a_frame_and_what_to_do() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, json!([]), |_, _| {
        json!({
            "matches": 0,
            "frames": [{ "frame": "iframe#pay", "same_origin": true, "matches": 1 }],
            "shadow_roots": [],
            "near_misses": [],
        })
    });

    let report = json_of(&host, "diagnose_selector", json!({ "selector": "#card" })).await;

    assert!(
        report["verdict"]
            .as_str()
            .unwrap()
            .contains("inside the frame \"iframe#pay\""),
        "{report}"
    );
    let steps = report["next_steps"].as_array().unwrap();
    assert!(!steps.is_empty() && steps.len() <= 3, "{steps:?}");
    assert!(steps[0].as_str().unwrap().contains("top document only"));
}

#[tokio::test]
async fn diagnose_selector_points_out_a_contains_selector_and_a_mistyped_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(
        &mock,
        json!([]),
        |_, _| json!({ "syntax_error": "'button:contains(\"Go\")' is not a valid selector", "matches": 0 }),
    );

    let contains = json_of(
        &host,
        "diagnose_selector",
        json!({ "selector": "button:contains(\"Go\")" }),
    )
    .await;
    let prefix = json_of(
        &host,
        "diagnose_selector",
        json!({ "selector": "lnk=Home" }),
    )
    .await;

    assert!(contains["verdict"]
        .as_str()
        .unwrap()
        .starts_with("The browser cannot parse this selector"));
    assert!(
        contains["advice"]
            .to_string()
            .contains("//button[contains(., 'Go')]"),
        "{contains}"
    );
    assert!(
        prefix["advice"].to_string().contains("`link=Home`"),
        "{prefix}"
    );
}

#[tokio::test]
async fn suggest_selectors_needs_something_to_look_for_and_says_when_nothing_matched() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(
        &mock,
        json!([]),
        |_, _| json!({ "url": "https://a.test/", "elements": [] }),
    );

    assert!(matches!(
        call(&host, "suggest_selectors", json!({})).await,
        Err(ToolError::InvalidArgument { .. })
    ));
    let found = json_of(&host, "suggest_selectors", json!({ "text": "Checkout" })).await;
    assert!(found["advice"].as_str().unwrap().contains("inspect_page"));
}

// ----------------------------------------------------------------------
// Console and network
// ----------------------------------------------------------------------

#[tokio::test]
async fn console_messages_include_the_errors_the_engine_saw_without_recording() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(
        &mock,
        json!(["ReferenceError: x is not defined"]),
        |op, expression| match op {
            "console" => json!({
                "recording": false,
                "entries": [],
                "install_seen": expression.contains("\"install\":false"),
            }),
            _ => Value::Null,
        },
    );

    let messages = json_of(&host, "get_console_messages", json!({})).await;

    assert_eq!(
        messages["page_errors"][0],
        "ReferenceError: x is not defined"
    );
    assert_eq!(messages["install_seen"], true, "recording is opt-in");
    assert!(messages["note"].as_str().unwrap().contains("start true"));
    assert!(
        mock.calls_to("Runtime.enable").is_empty(),
        "Runtime.enable would show the page it is automated"
    );
}

#[tokio::test]
async fn failed_requests_are_recorded_once_the_tab_is_watched() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(
        &mock,
        json!([]),
        |_, _| json!({ "supported": true, "checked": 3, "status_unknown": 0, "failed": [] }),
    );

    let first = json_of(&host, "get_failed_requests", json!({ "watch": true })).await;
    assert_eq!(first["watching"], true);
    assert_eq!(mock.calls_to("Network.enable").len(), 1);
    mock.emit(
        "Network.requestWillBeSent",
        json!({ "requestId": "7", "type": "Script", "request": { "url": "https://cdn.test/app.js" } }),
        Some("S1"),
    );
    mock.emit(
        "Network.loadingFailed",
        json!({ "requestId": "7", "type": "Script", "errorText": "net::ERR_NAME_NOT_RESOLVED" }),
        Some("S1"),
    );
    mock.emit(
        "Network.responseReceived",
        json!({ "requestId": "8", "type": "XHR", "response": { "url": "https://shop.test/api", "status": 503, "statusText": "Service Unavailable" } }),
        Some("S1"),
    );
    mock.emit(
        "Network.loadingFailed",
        json!({ "requestId": "9", "errorText": "elsewhere" }),
        Some("S-other"),
    );

    let mut recorded = Value::Null;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        recorded = json_of(&host, "get_failed_requests", json!({})).await["recorded"].clone();
        if recorded["total"] == 2 {
            break;
        }
    }

    assert_eq!(recorded["total"], 2, "{recorded}");
    let failures = recorded["failures"].as_array().unwrap();
    assert_eq!(failures[0]["url"], "https://cdn.test/app.js");
    assert_eq!(failures[0]["error"], "net::ERR_NAME_NOT_RESOLVED");
    assert_eq!(failures[1]["status"], 503);
    let again = json_of(&host, "get_failed_requests", json!({ "watch": true })).await;
    assert_eq!(again["watching"], true);
    assert_eq!(
        mock.calls_to("Network.enable").len(),
        1,
        "watching the same tab again starts nothing new"
    );
}

// ----------------------------------------------------------------------
// Debug bundles and exported tests
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_debug_bundle_holds_the_evidence_but_no_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, json!([]), |op, _| match op {
        "environment" => {
            json!({ "url": "https://shop.test/", "title": "Shop", "local_storage_keys": ["cart"] })
        }
        "source" => {
            json!({ "html": "<html><input type=\"password\" value=\"[redacted]\"></html>", "redacted_inputs": 1 })
        }
        "console" => json!({ "recording": false, "entries": [] }),
        "resources" => json!({ "supported": true, "failed": [] }),
        _ => json!("hunter2-token"),
    });
    mock.reply("Page.captureScreenshot", json!({ "data": "iVBORw0KGgo=" }));
    mock.reply(
        "Network.getAllCookies",
        json!({ "cookies": [{ "name": "session", "value": "s3cret-cookie", "domain": "shop.test", "path": "/" }] }),
    );
    call(
        &host,
        "run_javascript",
        json!({ "expression": "login('hunter2-token')" }),
    )
    .await
    .unwrap();

    let saved = json_of(
        &host,
        "capture_debug_bundle",
        json!({ "name": "../../cart bug" }),
    )
    .await;

    let folder = Path::new(saved["folder"].as_str().unwrap());
    assert!(folder.starts_with(dir.path()), "{}", folder.display());
    assert!(folder
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("______cart_bug_"));
    for file in ["screenshot.png", "page.html", "bundle.json"] {
        assert!(folder.join(file).is_file(), "{file} is missing: {saved}");
    }
    let bundle = std::fs::read_to_string(folder.join("bundle.json")).unwrap();
    assert!(
        !bundle.contains("hunter2"),
        "a script argument leaked:\n{bundle}"
    );
    assert!(
        !bundle.contains("s3cret"),
        "a cookie value leaked:\n{bundle}"
    );
    let bundle: Value = serde_json::from_str(&bundle).unwrap();
    assert_eq!(bundle["page"]["title"], "Shop");
    assert_eq!(bundle["cookie_names"], json!(["session"]));
    let recent = bundle["tool_calls"]["recent"].as_array().unwrap();
    assert_eq!(recent[0]["tool"], "run_javascript");
    assert_eq!(
        recent[0]["arguments"]["expression"],
        "[redacted, 22 characters]"
    );
}

#[tokio::test]
async fn a_debug_bundle_does_not_repeat_a_value_an_earlier_tool_returned() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, json!([]), |op, _| match op {
        "environment" => json!({ "url": "https://shop.test/", "title": "Shop" }),
        "source" => json!({ "html": "<html></html>", "redacted_inputs": 0 }),
        "console" => json!({ "recording": false, "entries": [] }),
        "resources" => json!({ "supported": true, "failed": [] }),
        _ => Value::Null,
    });
    mock.reply("Page.captureScreenshot", json!({ "data": "iVBORw0KGgo=" }));
    mock.reply(
        "Network.getAllCookies",
        json!({ "cookies": [{ "name": "session", "value": "s3cret-cookie", "domain": "shop.test", "path": "/" }] }),
    );

    // The agent reads the cookies, as it does after logging in.
    let cookies = json_of(&host, "manage_cookies", json!({ "action": "get_all" })).await;
    assert!(
        cookies.to_string().contains("s3cret-cookie"),
        "the caller gets the value: {cookies}"
    );

    let saved = json_of(&host, "capture_debug_bundle", json!({ "name": "login" })).await;
    let folder = Path::new(saved["folder"].as_str().unwrap());
    let bundle = std::fs::read_to_string(folder.join("bundle.json")).unwrap();
    assert!(
        !bundle.contains("s3cret"),
        "a cookie value from an earlier call leaked:\n{bundle}"
    );
    let bundle: Value = serde_json::from_str(&bundle).unwrap();
    let recent = bundle["tool_calls"]["recent"].as_array().unwrap();
    assert_eq!(recent[0]["tool"], "manage_cookies");
    assert_eq!(recent[0]["ok"], true);
}

#[tokio::test]
async fn the_session_is_exported_as_a_test_inside_the_output_directory() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    assert!(
        matches!(
            call(&host, "export_session_as_test", json!({})).await,
            Err(ToolError::Refused(_))
        ),
        "an empty session has nothing to export"
    );
    call(&host, "open_url", json!({ "url": "seleniumbase.io" }))
        .await
        .unwrap();
    call(&host, "scroll_page", json!({ "direction": "up" }))
        .await
        .unwrap();

    let exported = json_of(
        &host,
        "export_session_as_test",
        json!({ "test_name": "home_page_opens" }),
    )
    .await;

    let path = dir.path().join("exported_tests").join("home_page_opens.rs");
    let code = std::fs::read_to_string(&path).unwrap();
    assert!(code.contains("async fn home_page_opens()"), "{code}");
    assert!(
        code.contains("sb.open(\"seleniumbase.io\").await?;"),
        "{code}"
    );
    assert!(code.contains("// skipped:"), "{code}");
    assert_eq!(exported["steps"], 1);
    assert_eq!(exported["skipped"][0]["tool"], "scroll_page");
    assert!(matches!(
        call(
            &host,
            "export_session_as_test",
            json!({ "test_name": "../escape" })
        )
        .await,
        Err(ToolError::InvalidArgument { .. })
    ));
}

#[tokio::test]
async fn diagnose_environment_needs_no_browser() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));

    let report = json_of(&host, "diagnose_environment", json!({})).await;

    assert_eq!(report["browser_session"], "none");
    assert_eq!(report["output_dir"]["exists"], true);
    let free = &report["output_dir"]["free_disk_bytes"];
    assert!(free.is_u64() || free.is_null(), "{free}");
    assert!(report["platform"]["os"].is_string());
    assert!(report["sb_settings"].is_object());
    assert!(report["versions"]["fit"].is_string());
    assert!(report["problems"].is_array());
}
