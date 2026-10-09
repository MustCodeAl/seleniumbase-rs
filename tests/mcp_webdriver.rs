//! The WebDriver MCP servers (`driver` and `sb`): their catalogues and the
//! parts that need no browser.
//!
//! Run them with `cargo test --features mcp-server`. The tests that drive a
//! real browser are ignored by default; run them with `-- --ignored`.

#![cfg(feature = "mcp-server")]

use seleniumbase_rs::mcp::{driver, sb, Host, Output, Settings, ToolError};
use seleniumbase_rs::BaseCase;
use serde_json::{json, Map, Value};

const UPSTREAM_DRIVER_TOOLS: [&str; 26] = [
    "activate_cdp_mode",
    "assert_element",
    "assert_text",
    "click_element",
    "close_browser",
    "execute_script",
    "find_elements_count",
    "get_current_url",
    "get_page_source",
    "get_text",
    "get_title",
    "go_back",
    "go_forward",
    "is_element_visible",
    "open_url",
    "refresh_page",
    "save_screenshot",
    "select_option_by_index",
    "select_option_by_text",
    "select_option_by_value",
    "solve_captcha",
    "start_browser",
    "switch_to_default_content",
    "switch_to_frame",
    "type_text",
    "wait_for_element",
];

const UPSTREAM_SB_TOOLS: [&str; 88] = [
    "activate_cdp_mode",
    "assert_element",
    "assert_element_not_visible",
    "assert_element_present",
    "assert_exact_text",
    "assert_no_404_errors",
    "assert_no_js_errors",
    "assert_text",
    "assert_title",
    "assert_url",
    "assert_url_contains",
    "choose_file",
    "clear_input",
    "click_element",
    "click_if_visible",
    "click_link",
    "click_nth_visible_element",
    "click_visible_elements",
    "close_browser",
    "context_click",
    "delete_all_cookies",
    "double_click",
    "download_file",
    "drag_and_drop",
    "enter_mfa_code",
    "evaluate",
    "execute_script",
    "find_elements_count",
    "flash",
    "get_attribute",
    "get_cookies",
    "get_current_url",
    "get_element_html",
    "get_html_source",
    "get_local_storage_item",
    "get_mfa_code",
    "get_origin",
    "get_page_info",
    "get_session_storage_item",
    "get_text",
    "get_title",
    "get_user_agent",
    "get_window_rect",
    "go_back",
    "go_forward",
    "highlight",
    "hover_and_click",
    "is_element_clickable",
    "is_element_present",
    "is_element_visible",
    "is_selected",
    "is_text_visible",
    "load_cookies",
    "maximize_window",
    "minimize_window",
    "nested_click",
    "open_new_tab",
    "open_url",
    "print_to_pdf",
    "refresh_page",
    "save_cookies",
    "save_page_source",
    "save_screenshot",
    "scroll_down",
    "scroll_into_view",
    "scroll_to_bottom",
    "scroll_to_top",
    "scroll_up",
    "select_option_by_index",
    "select_option_by_text",
    "select_option_by_value",
    "set_local_storage_item",
    "set_session_storage_item",
    "set_value",
    "sleep",
    "solve_captcha",
    "start_browser",
    "submit",
    "switch_to_default_content",
    "switch_to_default_window",
    "switch_to_frame",
    "switch_to_newest_tab",
    "type_text",
    "wait_for_element",
    "wait_for_element_absent",
    "wait_for_element_not_visible",
    "wait_for_element_present",
    "wait_for_text",
];

/// The stealth tools this crate adds to the `sb` server.
const STEALTH_TOOLS: [&str; 8] = [
    "build_fingerprint",
    "build_stealth_bootstrap",
    "get_stealth_bootstrap_script",
    "list_engine_spoofing_args",
    "list_evasion_providers",
    "list_fingerprint_presets",
    "patch_chromedriver",
    "validate_fingerprint",
];

fn arguments(v: Value) -> Map<String, Value> {
    v.as_object()
        .cloned()
        .expect("tool arguments are an object")
}

async fn call(host: &Host<BaseCase>, tool: &str, v: Value) -> Result<Output, ToolError> {
    host.call(tool, arguments(v)).await
}

fn sorted(mut names: Vec<&'static str>) -> Vec<&'static str> {
    names.sort_unstable();
    names
}

#[test]
fn the_driver_server_offers_exactly_the_tools_of_the_python_driver_server() {
    let host = driver::host(Settings::new("unused"));

    assert_eq!(sorted(host.tool_names()), UPSTREAM_DRIVER_TOOLS);
}

#[test]
fn the_sb_server_offers_every_python_sb_tool_plus_the_stealth_tools() {
    let host = sb::host(Settings::new("unused"));
    let offered = sorted(host.tool_names());

    let mut expected: Vec<&str> = UPSTREAM_SB_TOOLS.into_iter().chain(STEALTH_TOOLS).collect();
    expected.sort_unstable();
    assert_eq!(offered, expected);
}

#[test]
fn tool_names_are_unique_within_each_server() {
    for host in [
        driver::host(Settings::new("unused")),
        sb::host(Settings::new("unused")),
    ] {
        let names = host.tool_names();
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "{} repeats a tool name",
            host.name()
        );
    }
}

#[test]
fn every_tool_describes_itself_with_a_schema_a_description_and_a_title() {
    for host in [
        driver::host(Settings::new("unused")),
        sb::host(Settings::new("unused")),
    ] {
        for tool in host.describe_tools() {
            let name = tool.name.to_string();
            assert!(
                tool.description
                    .as_deref()
                    .is_some_and(|text| text.len() > 15),
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
            let title = tool.annotations.as_ref().and_then(|a| a.title.clone());
            assert!(title.is_some_and(|t| !t.is_empty()), "{name} has no title");
        }
    }
}

#[tokio::test]
async fn a_browser_tool_before_start_browser_says_to_start_one() {
    let host = sb::host(Settings::new("unused"));

    let error = call(&host, "click_element", json!({ "selector": "#go" }))
        .await
        .unwrap_err();

    assert!(matches!(error, ToolError::NoBrowser));
    assert!(error.to_string().contains("start_browser"), "{error}");
}

#[tokio::test]
async fn page_info_reports_not_running_until_a_browser_starts() {
    let host = sb::host(Settings::new("unused"));

    let info = call(&host, "get_page_info", json!({})).await.unwrap();

    assert_eq!(info.as_json(), Some(&json!({ "running": false })));
}

#[tokio::test]
async fn closing_with_nothing_running_is_harmless() {
    let host = driver::host(Settings::new("unused"));

    let answer = call(&host, "close_browser", json!({})).await.unwrap();

    assert_eq!(answer.as_text(), Some("No browser session was running."));
}

#[tokio::test]
async fn starting_with_incognito_and_guest_together_is_rejected_before_anything_launches() {
    let host = sb::host(Settings::new("unused"));

    let error = call(
        &host,
        "start_browser",
        json!({ "incognito": true, "guest_mode": true }),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(error, ToolError::InvalidArgument { .. }),
        "{error}"
    );
    assert!(
        host.ctx().try_session().await.is_none(),
        "no session may be left behind"
    );
}

#[tokio::test]
async fn an_unknown_browser_lists_the_choices() {
    let host = sb::host(Settings::new("unused"));

    let error = call(&host, "start_browser", json!({ "browser": "netscape" }))
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains("chrome") && message.contains("firefox"),
        "{message}"
    );
}

#[tokio::test]
async fn files_are_refused_outside_the_output_directory_before_the_browser_is_asked() {
    let host = sb::host(Settings::new("out"));

    for (tool, arguments) in [
        ("save_screenshot", json!({ "name": "../escape.png" })),
        ("save_page_source", json!({ "folder": "../escape" })),
        ("print_to_pdf", json!({ "name": "/tmp/escape.pdf" })),
    ] {
        // No browser is running, so a refusal proves the path was checked first.
        let error = call(&host, tool, arguments).await.unwrap_err();
        assert!(matches!(error, ToolError::Refused(_)), "{tool}: {error}");
    }
}

#[tokio::test]
async fn only_http_and_https_urls_can_be_downloaded() {
    let host = sb::host(Settings::new("out"));

    for url in ["ftp://example.com/a.zip", "file:///etc/passwd", "not a url"] {
        let error = call(&host, "download_file", json!({ "file_url": url }))
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::InvalidArgument { .. }),
            "{url}: {error}"
        );
    }
}

#[tokio::test]
async fn cookie_files_cannot_name_a_path_outside_the_cookie_directory() {
    let dir = tempfile::tempdir().unwrap();
    let host = sb::host(Settings::new(dir.path()));

    // Loading a name that was never saved fails, and only ever looks inside
    // saved_cookies: the directory part of the name is dropped.
    let error = call(&host, "load_cookies", json!({ "name": "../../etc/hosts" }))
        .await
        .unwrap_err();

    assert!(
        matches!(error, ToolError::Refused(_)) || matches!(error, ToolError::NoBrowser),
        "{error}"
    );
}

// ----------------------------------------------------------------------
// Stealth tools, which need no browser
// ----------------------------------------------------------------------

#[tokio::test]
async fn the_stealth_tools_work_without_a_browser() {
    let host = sb::host(Settings::new("unused"));

    let presets = call(&host, "list_fingerprint_presets", json!({}))
        .await
        .unwrap();
    assert_eq!(presets.as_json().unwrap().as_array().unwrap().len(), 5);

    for preset in ["windows", "macos", "linux", "android", "ios"] {
        let report = call(&host, "validate_fingerprint", json!({ "preset": preset }))
            .await
            .unwrap();
        let report = report.as_json().unwrap();
        assert_eq!(report["coherent"], true, "{preset}: {report}");

        let script = call(
            &host,
            "build_stealth_bootstrap",
            json!({ "preset": preset }),
        )
        .await
        .unwrap();
        assert!(
            script.as_text().is_some_and(|s| s.len() > 100),
            "{preset} bootstrap"
        );
    }

    let providers = call(&host, "list_evasion_providers", json!({}))
        .await
        .unwrap();
    assert!(!providers.as_json().unwrap().as_array().unwrap().is_empty());
}

#[tokio::test]
async fn a_fingerprint_can_be_overridden_and_a_bad_preset_is_rejected() {
    let host = sb::host(Settings::new("unused"));

    let built = call(
        &host,
        "build_fingerprint",
        json!({ "preset": "windows", "user_agent": "Test/1.0", "screen_width": 1280 }),
    )
    .await
    .unwrap();
    let built = built.as_json().unwrap();
    assert_eq!(built["user_agent"], "Test/1.0");
    assert_eq!(built["screen_width"], 1280);

    let error = call(&host, "build_fingerprint", json!({ "preset": "beos" }))
        .await
        .unwrap_err();
    assert!(
        matches!(error, ToolError::InvalidArgument { .. }),
        "{error}"
    );
}
