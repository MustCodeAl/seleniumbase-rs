//! The Pure CDP MCP server against a scripted browser, with no Chrome.
//!
//! Each test calls a tool the way a client would and checks the protocol
//! commands it produced, or the answer the model would read.
//! Run them with `cargo test --features "mcp-server test-util"`.

#![cfg(all(feature = "mcp-server", feature = "test-util"))]

use std::path::Path;

use seleniumbase_rs::mcp::cdp::{self, Cdp};
use seleniumbase_rs::mcp::{Host, Output, Settings, ToolError};
use seleniumbase_rs::sb_cdp::{Browser, MockCtrl};
use serde_json::{json, Map, Value};

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

/// A server whose browser session is already running on a mocked browser.
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

/// Answers the helper calls a locator makes: a count, a centre point, and
/// visibility, from the given state.
fn script_locator(mock: &MockCtrl, present: usize, visible: bool) {
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("__sbcdp.center(") {
            value(json!({ "x": 120.0, "y": 80.0, "covered": false }))
        } else if expression.contains(".some(__sbcdp.visible)") {
            value(json!(visible && present > 0))
        } else if expression.contains("resolve(") && expression.contains(".length === 0") {
            value(json!(present == 0))
        } else if expression.contains("resolve(") && expression.contains(".length > 0") {
            value(json!(present > 0))
        } else if expression.contains("resolve(") && expression.contains(".length") {
            value(json!(present))
        } else {
            value(Value::Null)
        })
    });
}

const UPSTREAM_CDP_TOOLS: [&str; 24] = [
    "assert_condition",
    "check_if_condition",
    "click_element",
    "close_browser",
    "find_elements",
    "focus_element",
    "get_attributes",
    "get_content",
    "get_page_info",
    "hover_action",
    "manage_cookies",
    "manage_history",
    "manage_storage",
    "manage_tabs",
    "manage_window",
    "open_url",
    "run_javascript",
    "save_page",
    "scroll_page",
    "select_option",
    "solve_captcha",
    "start_browser",
    "type_text",
    "wait_for_condition",
];

// ----------------------------------------------------------------------
// Catalogue
// ----------------------------------------------------------------------

#[tokio::test]
async fn the_server_offers_every_tool_of_the_python_cdp_server() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));

    let mut offered = host.tool_names();
    offered.sort_unstable();

    assert_eq!(offered, UPSTREAM_CDP_TOOLS);
}

#[tokio::test]
async fn every_tool_describes_itself_with_an_object_schema_and_a_title() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));

    for tool in host.describe_tools() {
        let name = tool.name.to_string();
        assert!(
            tool.description
                .as_deref()
                .is_some_and(|text| text.len() > 20),
            "{name} needs a description a model can choose by"
        );
        assert_eq!(
            tool.input_schema.get("type"),
            Some(&json!("object")),
            "{name}"
        );
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(Value::as_object);
        let required = tool.input_schema.get("required").and_then(Value::as_array);
        for required in required.into_iter().flatten() {
            let required = required.as_str().unwrap();
            assert!(
                properties.is_some_and(|p| p.contains_key(required)),
                "{name} requires {required}, which it does not declare"
            );
        }
        let annotations = tool
            .annotations
            .as_ref()
            .unwrap_or_else(|| panic!("{name} has no annotations"));
        assert!(annotations.title.is_some(), "{name} has no title");
    }
}

#[tokio::test]
async fn annotations_are_honest_about_what_each_tool_does() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));
    let tools = host.describe_tools();
    let annotations = |name: &str| {
        tools
            .iter()
            .find(|tool| tool.name == name)
            .and_then(|tool| tool.annotations.clone())
            .unwrap()
    };

    // Reading never changes anything.
    assert_eq!(annotations("get_content").read_only_hint, Some(true));
    // Clicking changes the page but is not destructive in itself.
    let click = annotations("click_element");
    assert_eq!(
        (click.read_only_hint, click.idempotent_hint),
        (Some(false), Some(false))
    );
    // Writing a file may replace one.
    assert_eq!(annotations("save_page").destructive_hint, Some(true));
    // One tool, actions of different kinds: no single answer is true.
    let history = annotations("manage_history");
    assert_eq!(history.read_only_hint, None);
    assert_eq!(history.destructive_hint, None);
}

// ----------------------------------------------------------------------
// Session
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_browser_tool_before_start_browser_says_to_start_one() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));

    let error = call(&host, "open_url", json!({ "url": "example.com" }))
        .await
        .unwrap_err();

    assert!(matches!(error, ToolError::NoBrowser));
    assert!(error.to_string().contains("start_browser"), "{error}");
}

#[tokio::test]
async fn an_unknown_tool_is_reported_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(&host, "make_coffee", json!({})).await.unwrap_err();

    assert!(matches!(&error, ToolError::UnknownTool(name) if name == "make_coffee"));
}

#[tokio::test]
async fn page_info_reports_not_running_until_a_browser_starts() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));

    assert_eq!(
        json_of(&host, "get_page_info", json!({})).await,
        json!({ "running": false })
    );
}

#[tokio::test]
async fn page_info_gathers_url_title_origin_and_user_agent() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(value(if expression.contains("location.origin") {
            json!("https://example.com")
        } else if expression.contains("userAgent") {
            json!("TestAgent/1.0")
        } else if expression.contains("document.title") {
            json!("Example Domain")
        } else {
            json!("https://example.com/page?q=1")
        }))
    });

    let info = json_of(&host, "get_page_info", json!({})).await;

    assert_eq!(info["running"], true);
    assert_eq!(info["origin"], "https://example.com");
    assert_eq!(info["title"], "Example Domain");
    assert_eq!(info["user_agent"], "TestAgent/1.0");
}

#[tokio::test]
async fn starting_with_incognito_and_guest_together_is_an_error_before_anything_launches() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));

    let error = call(
        &host,
        "start_browser",
        json!({ "incognito": true, "guest": true }),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, ToolError::Failed(_)), "{error}");
    assert!(
        host.ctx().try_session().await.is_none(),
        "no session may be left behind"
    );
}

#[tokio::test]
async fn starting_twice_keeps_the_first_session() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let answer = text_of(&host, "start_browser", json!({})).await;

    assert_eq!(answer, "A browser session is already running.");
}

#[tokio::test]
async fn closing_ends_the_session_and_closing_again_is_harmless() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    assert_eq!(
        text_of(&host, "close_browser", json!({})).await,
        "The browser session was closed."
    );
    assert_eq!(
        text_of(&host, "close_browser", json!({})).await,
        "No browser session is currently running."
    );
    assert!(matches!(
        call(&host, "get_content", json!({})).await.unwrap_err(),
        ToolError::NoBrowser
    ));
}

// ----------------------------------------------------------------------
// Navigation
// ----------------------------------------------------------------------

#[tokio::test]
async fn open_url_adds_https_to_a_bare_host_and_waits_for_the_load() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;

    let answer = text_of(&host, "open_url", json!({ "url": "seleniumbase.io" })).await;

    assert_eq!(answer, "Navigated to seleniumbase.io");
    assert_eq!(
        mock.calls_to("Page.navigate")[0].params["url"],
        "https://seleniumbase.io"
    );
}

#[tokio::test]
async fn a_missing_required_argument_names_the_argument() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(&host, "open_url", json!({})).await.unwrap_err();

    assert!(
        matches!(&error, ToolError::InvalidArgument { name, .. } if name == "url"),
        "{error}"
    );
}

#[tokio::test]
async fn an_argument_of_the_wrong_type_is_rejected_not_coerced() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(&host, "click_element", json!({ "selector": 7 }))
        .await
        .unwrap_err();

    assert!(
        matches!(error, ToolError::InvalidArgument { .. }),
        "{error}"
    );
}

#[tokio::test]
async fn reload_bypasses_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;

    text_of(&host, "manage_history", json!({ "action": "reload" })).await;

    assert_eq!(mock.calls_to("Page.reload")[0].params["ignoreCache"], true);
}

#[tokio::test]
async fn listing_history_reports_the_position_and_each_entry() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.reply(
        "Page.getNavigationHistory",
        json!({ "currentIndex": 1, "entries": [
            { "id": 7, "url": "https://a.test/", "userTypedURL": "a.test", "title": "A", "transitionType": "typed" },
            { "id": 8, "url": "https://b.test/", "userTypedURL": "https://b.test/", "title": "B", "transitionType": "link" },
        ]}),
    );

    let history = json_of(&host, "manage_history", json!({})).await;

    assert_eq!(history["position"], 1);
    assert_eq!(history["entries"][0]["user_typed_url"], "a.test");
    assert_eq!(history["entries"][1]["transition_type"], "link");
    assert_eq!(history["entries"].as_array().unwrap().len(), 2);
}

// ----------------------------------------------------------------------
// Reading
// ----------------------------------------------------------------------

#[tokio::test]
async fn finding_nothing_is_an_empty_answer_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 0, false);

    let found = json_of(
        &host,
        "find_elements",
        json!({ "selector": ".none", "timeout": 0.1 }),
    )
    .await;

    assert_eq!(found, json!({ "count": 0, "matches": [] }));
}

#[tokio::test]
async fn get_content_reads_text_by_default_from_the_body() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(value(if expression.contains(".length > 0") {
            json!(true)
        } else {
            json!("Hello page")
        }))
    });

    let text = text_of(&host, "get_content", json!({ "timeout": 0.1 })).await;

    assert_eq!(text, "Hello page");
    let sent = mock.calls_to("Runtime.evaluate").pop().unwrap().params["expression"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        sent.contains("body"),
        "the default selector is the body: {sent}"
    );
}

#[tokio::test]
async fn an_unknown_output_format_lists_the_choices() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(&host, "get_content", json!({ "output_format": "markdown" }))
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains("text") && message.contains("html") && message.contains("urls"),
        "{message}"
    );
}

#[tokio::test]
async fn checking_a_missing_element_answers_false_without_waiting() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 0, false);

    let started = std::time::Instant::now();
    let present = json_of(
        &host,
        "check_if_condition",
        json!({ "check": "present", "selector": "#x" }),
    )
    .await;
    let visible = json_of(&host, "check_if_condition", json!({ "selector": "#x" })).await;
    let text = json_of(
        &host,
        "check_if_condition",
        json!({ "selector": "#x", "text": "hi" }),
    )
    .await;

    assert_eq!(
        (present, visible, text),
        (json!(false), json!(false), json!(false))
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "a check must not wait"
    );
}

// ----------------------------------------------------------------------
// Interacting
// ----------------------------------------------------------------------

#[tokio::test]
async fn click_element_presses_at_the_elements_centre() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 1, true);

    let answer = text_of(&host, "click_element", json!({ "selector": "#go" })).await;

    assert_eq!(answer, "Clicked #go");
    let kinds: Vec<_> = mock
        .calls_to("Input.dispatchMouseEvent")
        .iter()
        .map(|call| call.params["type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(kinds, ["mouseMoved", "mousePressed", "mouseReleased"]);
}

#[tokio::test]
async fn nth_counts_from_one_and_zero_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 3, true);

    let answer = text_of(
        &host,
        "click_element",
        json!({ "selector": "li", "nth": 2 }),
    )
    .await;
    assert_eq!(answer, "Clicked match #2 of li");
    let chains: Vec<_> = mock
        .calls_to("Runtime.evaluate")
        .into_iter()
        .filter_map(|call| call.params["expression"].as_str().map(str::to_owned))
        .filter(|expression| expression.contains(r#""nth":"#))
        .collect();
    assert!(
        chains.iter().any(|chain| chain.contains(r#""nth":1"#)),
        "the second match is index 1: {chains:?}"
    );

    let error = call(
        &host,
        "click_element",
        json!({ "selector": "li", "nth": 0 }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, ToolError::InvalidArgument { .. }),
        "{error}"
    );
}

#[tokio::test]
async fn clicking_only_if_visible_leaves_a_hidden_element_alone() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 0, false);

    let answer = text_of(
        &host,
        "click_element",
        json!({ "selector": "#popup", "only_if_visible": true }),
    )
    .await;

    assert!(answer.contains("nothing was clicked"), "{answer}");
    assert!(mock.calls_to("Input.dispatchMouseEvent").is_empty());
}

#[tokio::test]
async fn click_all_matches_clicks_each_visible_one() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 3, true);

    let answer = text_of(
        &host,
        "click_element",
        json!({ "selector": "input", "all_matches": true }),
    )
    .await;

    assert_eq!(answer, "Clicked 3 visible matches of input");
    let presses = mock
        .calls_to("Input.dispatchMouseEvent")
        .iter()
        .filter(|call| call.params["type"] == "mousePressed")
        .count();
    assert_eq!(presses, 3);
}

#[tokio::test]
async fn hover_and_click_needs_a_second_selector() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 1, true);

    let error = call(
        &host,
        "hover_action",
        json!({ "selector": "#menu", "action": "hover_and_click" }),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(&error, ToolError::InvalidArgument { name, .. } if name == "secondary_selector"),
        "{error}"
    );
}

#[tokio::test]
async fn typing_modes_differ_in_how_the_text_reaches_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 1, true);

    // append: key by key, and a newline presses Enter.
    text_of(
        &host,
        "type_text",
        json!({ "selector": "#q", "text": "ab\n", "mode": "append" }),
    )
    .await;
    let keys: Vec<_> = mock
        .calls_to("Input.dispatchKeyEvent")
        .iter()
        .filter(|call| call.params["type"] == "keyDown")
        .map(|call| call.params["key"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(keys, ["a", "b", "Enter"]);
    assert!(mock.calls_to("Input.insertText").is_empty());

    // fast_type: inserted whole, with Enter between lines.
    text_of(
        &host,
        "type_text",
        json!({ "selector": "#q", "text": "one\ntwo", "mode": "fast_type" }),
    )
    .await;
    let inserted: Vec<_> = mock
        .calls_to("Input.insertText")
        .iter()
        .map(|call| call.params["text"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(inserted, ["one", "two"]);

    // set_value: no key events at all.
    let before = mock.calls_to("Input.dispatchKeyEvent").len();
    text_of(
        &host,
        "type_text",
        json!({ "selector": "#q", "text": "42", "mode": "set_value" }),
    )
    .await;
    assert_eq!(mock.calls_to("Input.dispatchKeyEvent").len(), before);
    let last = mock.calls_to("Runtime.evaluate").pop().unwrap().params["expression"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(last.contains("setValue"), "{last}");
}

#[tokio::test]
async fn select_option_accepts_an_index_as_a_number_or_a_string() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 1, true);

    text_of(
        &host,
        "select_option",
        json!({ "dropdown_selector": "select", "value": 2, "by": "index" }),
    )
    .await;
    text_of(
        &host,
        "select_option",
        json!({ "dropdown_selector": "select", "value": "2", "by": "index" }),
    )
    .await;

    let selects: Vec<_> = mock
        .calls_to("Runtime.evaluate")
        .into_iter()
        .filter_map(|call| call.params["expression"].as_str().map(str::to_owned))
        .filter(|expression| expression.contains("__sbcdp.selectOption("))
        .collect();
    assert_eq!(selects.len(), 2);
    assert_eq!(
        selects[0], selects[1],
        "a number and its string must select the same option"
    );
    assert!(selects[0].contains(r#"'index', "2""#), "{selects:?}");

    let error = call(
        &host,
        "select_option",
        json!({ "dropdown_selector": "select", "value": "two", "by": "index" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, ToolError::InvalidArgument { .. }),
        "{error}"
    );
}

// ----------------------------------------------------------------------
// Waiting and asserting
// ----------------------------------------------------------------------

#[tokio::test]
async fn waiting_for_seconds_passed_sleeps_for_the_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let started = std::time::Instant::now();
    let answer = text_of(
        &host,
        "wait_for_condition",
        json!({ "state": "seconds_passed", "timeout": 0.2 }),
    )
    .await;

    assert!(started.elapsed() >= std::time::Duration::from_millis(200));
    assert_eq!(answer, "Waited for 0.2s");
}

#[tokio::test]
async fn waiting_without_a_selector_or_text_is_an_argument_error() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(&host, "wait_for_condition", json!({ "state": "visible" }))
        .await
        .unwrap_err();

    assert!(
        matches!(error, ToolError::InvalidArgument { .. }),
        "{error}"
    );
}

#[tokio::test]
async fn a_wait_that_times_out_is_an_error_naming_the_element_and_state() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 0, false);

    let error = call(
        &host,
        "wait_for_condition",
        json!({ "state": "visible", "selector": ".spinner", "timeout": 0.2 }),
    )
    .await
    .unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains(".spinner") && message.contains("visible"),
        "{message}"
    );
}

#[tokio::test]
async fn title_and_url_assertions_are_immediate() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(value(if expression.contains("document.title") {
            json!("Example Domain")
        } else {
            json!("https://example.com/path")
        }))
    });

    let started = std::time::Instant::now();
    text_of(
        &host,
        "assert_condition",
        json!({ "check": "title", "expected": "Example Domain" }),
    )
    .await;
    text_of(
        &host,
        "assert_condition",
        json!({ "check": "url_contains", "expected": "/path" }),
    )
    .await;
    let wrong = call(
        &host,
        "assert_condition",
        json!({ "check": "title", "expected": "Other" }),
    )
    .await
    .unwrap_err();

    assert!(wrong.to_string().contains("Other"), "{wrong}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "title and URL checks must not wait"
    );
}

#[tokio::test]
async fn an_assertion_that_needs_an_expected_value_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(&host, "assert_condition", json!({ "check": "url" }))
        .await
        .unwrap_err();

    assert!(
        matches!(&error, ToolError::InvalidArgument { name, .. } if name == "expected"),
        "{error}"
    );
}

// ----------------------------------------------------------------------
// Browser state
// ----------------------------------------------------------------------

#[tokio::test]
async fn cookies_are_saved_inside_the_cookie_directory_whatever_the_filename() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.reply(
        "Network.getAllCookies",
        json!({ "cookies": [{ "name": "sid", "value": "abc", "domain": "a.test", "path": "/",
                              "expires": -1, "httpOnly": true, "secure": true }] }),
    );

    let answer = text_of(
        &host,
        "manage_cookies",
        json!({ "action": "save", "filename": "../../etc/passwd" }),
    )
    .await;

    assert!(answer.contains("passwd.txt"), "{answer}");
    let saved = dir.path().join("saved_cookies").join("passwd.txt");
    assert!(
        saved.is_file(),
        "the file must land in the cookie directory"
    );
    assert!(!dir
        .path()
        .join("..")
        .join("..")
        .join("etc")
        .join("passwd.txt")
        .exists());
}

#[tokio::test]
async fn cookies_saved_by_one_call_can_be_loaded_by_another() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.reply(
        "Network.getAllCookies",
        json!({ "cookies": [{ "name": "sid", "value": "abc", "domain": "a.test", "path": "/",
                              "expires": -1, "httpOnly": false, "secure": false }] }),
    );

    text_of(
        &host,
        "manage_cookies",
        json!({ "action": "save", "filename": "login" }),
    )
    .await;
    text_of(
        &host,
        "manage_cookies",
        json!({ "action": "load", "filename": "login.txt" }),
    )
    .await;

    let sent = &mock.calls_to("Network.setCookies")[0].params["cookies"];
    assert_eq!(sent[0]["name"], "sid");
    assert_eq!(sent[0]["value"], "abc");
}

#[tokio::test]
async fn loading_cookies_that_were_never_saved_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(
        &host,
        "manage_cookies",
        json!({ "action": "load", "filename": "ghost" }),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, ToolError::Refused(_)), "{error}");
}

#[tokio::test]
async fn setting_storage_requires_a_value() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(
        &host,
        "manage_storage",
        json!({ "key": "theme", "action": "set" }),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(&error, ToolError::InvalidArgument { name, .. } if name == "value"),
        "{error}"
    );
}

#[tokio::test]
async fn scrolling_by_a_negative_amount_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(
        &host,
        "scroll_page",
        json!({ "direction": "down", "amount": -5 }),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(error, ToolError::InvalidArgument { .. }),
        "{error}"
    );
    assert_eq!(
        text_of(&host, "scroll_page", json!({ "direction": "top" })).await,
        "Scrolled to the top."
    );
}

#[tokio::test]
async fn set_rect_needs_all_four_numbers() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(
        &host,
        "manage_window",
        json!({ "action": "set_rect", "x": 0, "y": 0, "width": 800 }),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(error, ToolError::InvalidArgument { .. }),
        "{error}"
    );
}

#[tokio::test]
async fn opening_a_tab_switches_to_it_and_later_commands_go_there() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;

    let answer = text_of(
        &host,
        "manage_tabs",
        json!({ "action": "open_new_tab", "url": "https://b.test" }),
    )
    .await;

    assert!(answer.contains("switch_to=true"), "{answer}");
    assert_eq!(mock.calls_to("Target.createTarget").len(), 1);
    assert!(!mock.calls_to("Page.bringToFront").is_empty());
}

#[tokio::test]
async fn switching_to_a_tab_out_of_range_lists_how_many_there_are() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path()).await;

    let error = call(
        &host,
        "manage_tabs",
        json!({ "action": "switch_to_tab", "tab_index": 5 }),
    )
    .await
    .unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains("out of range") && message.contains('1'),
        "{message}"
    );
}

#[tokio::test]
async fn listing_tabs_numbers_them() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.reply(
        "Target.getTargets",
        json!({ "targetInfos": [
            { "targetId": "T1", "type": "page", "url": "https://a.test/", "title": "A" },
            { "targetId": "T2", "type": "page", "url": "https://b.test/", "title": "B" },
        ]}),
    );

    let tabs = json_of(&host, "manage_tabs", json!({})).await;

    assert_eq!(
        tabs[0],
        json!({ "index": 0, "url": "https://a.test/", "title": "A" })
    );
    assert_eq!(tabs[1]["index"], 1);
}

#[tokio::test]
async fn solving_a_captcha_on_a_page_without_one_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_locator(&mock, 0, false);

    let answer = text_of(&host, "solve_captcha", json!({})).await;

    assert_eq!(answer, "No supported CAPTCHA was found on the page.");
    assert!(mock.calls_to("Input.dispatchMouseEvent").is_empty());
}

// ----------------------------------------------------------------------
// Output and scripting
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_screenshot_is_written_into_the_output_directory() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    // "PNG!" in base64.
    mock.reply("Page.captureScreenshot", json!({ "data": "UE5HIQ==" }));

    let answer = text_of(&host, "save_page", json!({ "folder": "shots" })).await;

    let path = dir.path().join("shots").join("screenshot.png");
    assert!(answer.contains("screenshot.png"), "{answer}");
    assert_eq!(std::fs::read(path).unwrap(), b"PNG!");
}

#[tokio::test]
async fn saving_outside_the_output_directory_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.reply("Page.captureScreenshot", json!({ "data": "UE5HIQ==" }));

    for arguments in [
        json!({ "filename": "../escape.png" }),
        json!({ "folder": "../escape" }),
        json!({ "filename": "/tmp/escape.png" }),
    ] {
        let error = call(&host, "save_page", arguments.clone())
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::Refused(_)),
            "{arguments}: {error}"
        );
    }
    assert!(
        mock.calls_to("Page.captureScreenshot").is_empty(),
        "nothing is captured for a refused path"
    );
}

#[tokio::test]
async fn javascript_results_come_back_as_json() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.reply(
        "Runtime.evaluate",
        value(json!({ "n": 3, "tags": ["a", "b"] })),
    );

    let result = json_of(&host, "run_javascript", json!({ "expression": "({n: 3})" })).await;

    assert_eq!(result, json!({ "n": 3, "tags": ["a", "b"] }));
}

#[tokio::test]
async fn a_script_error_reaches_the_model_as_a_readable_error() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    mock.reply(
        "Runtime.evaluate",
        json!({ "exceptionDetails": { "exception": { "description": "ReferenceError: nope is not defined" } } }),
    );

    let error = call(&host, "run_javascript", json!({ "expression": "nope" }))
        .await
        .unwrap_err();

    assert!(error.to_string().contains("nope is not defined"), "{error}");
}
