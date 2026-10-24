//! The fingerprint tools of the MCP servers, with no Chrome.
//!
//! The browser tools (`capture_fingerprint`, `compare_fingerprint`,
//! `explain_fingerprint`) run against a mocked Pure CDP browser that answers the
//! capture script with a recorded reading (`tests/fixtures/fingerprint`); the tools
//! that need no browser (`diff_fingerprints`, `patch_fingerprint`,
//! `build_fingerprint`, `validate_fingerprint`) run on the `sb` server.
//! `tests/mcp_fingerprint_chrome.rs` runs the same tools in a real Chrome.
//! Run them with `cargo test --features "mcp-server test-util" --test mcp_fingerprint`.

#![cfg(all(feature = "mcp-server", feature = "test-util"))]

use std::path::Path;
use std::sync::{Arc, Mutex};

use seleniumbase_rs::cli::fingerprint_cmd::{export, ExportSource};
use seleniumbase_rs::mcp::cdp::{self, Cdp};
use seleniumbase_rs::mcp::{
    driver, sb, Host, Output, ProgressSink, ProgressUpdate, Settings, ToolError,
};
use seleniumbase_rs::sb_cdp::{Browser, MockCtrl};
use seleniumbase_rs::{Fingerprint, OsType};
use serde_json::{json, Map, Value};

const MACOS: &str = include_str!("fixtures/fingerprint/probe_chrome_macos.json");

const BROWSER_TOOLS: [&str; 3] = [
    "capture_fingerprint",
    "compare_fingerprint",
    "explain_fingerprint",
];

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

/// The reading of a browser that shows a page: the macOS fixture with `headless`
/// markers set.
fn readings(markers: &[&str]) -> Value {
    let mut readings: Value = serde_json::from_str(MACOS).expect("the fixture is JSON");
    readings["headless"] = json!(markers);
    readings
}

/// A Pure CDP server on a mocked browser whose page shows `readings`.
async fn running(output_dir: &Path, readings: Value) -> (Host<Cdp>, MockCtrl) {
    let host = cdp::host(Settings::new(output_dir));
    let (browser, mock) = Browser::new_mocked();
    mock.on("Runtime.evaluate", move |_params| {
        Ok(json!({ "result": { "value": readings.clone() } }))
    });
    host.ctx()
        .start(async { Ok::<_, ToolError>(Cdp::attach(browser).await?) })
        .await
        .expect("the mocked session starts");
    (host, mock)
}

fn invalid(error: &ToolError, name: &str) -> bool {
    matches!(error, ToolError::InvalidArgument { name: given, .. } if given == name)
}

// ----------------------------------------------------------------------
// Catalogue
// ----------------------------------------------------------------------

#[test]
fn every_browser_server_offers_the_fingerprint_tools_as_untrusted_with_a_schema() {
    let dir = tempfile::tempdir().unwrap();
    let settings = || Settings::new(dir.path());
    let cdp = cdp::host(settings());
    let driver = driver::host(settings());
    let sb = sb::host(settings());

    let described = [
        ("cdp", cdp.describe_tools(), cdp.untrusted_tool_names()),
        (
            "driver",
            driver.describe_tools(),
            driver.untrusted_tool_names(),
        ),
        ("sb", sb.describe_tools(), sb.untrusted_tool_names()),
    ];
    for (server, tools, untrusted) in described {
        for name in BROWSER_TOOLS {
            let tool = tools
                .iter()
                .find(|tool| tool.name == name)
                .unwrap_or_else(|| panic!("{server} lacks {name}"));
            assert!(
                tool.output_schema
                    .as_ref()
                    .is_some_and(|schema| schema.get("type") == Some(&json!("object"))
                        && schema.get("required").is_some()),
                "{server}: {name} declares no object output schema"
            );
            assert!(
                untrusted.contains(&name),
                "{server}: {name} is not untrusted"
            );
        }
        let start = tools
            .iter()
            .find(|tool| tool.name == "start_browser")
            .unwrap();
        let properties = start.input_schema.get("properties").unwrap();
        for argument in ["fingerprint", "fingerprint_path", "fingerprint_preset"] {
            assert!(properties.get(argument).is_some(), "{server}: {argument}");
        }
    }
    // Only the sb server has the stealth tools.
    assert!(sb.tool_names().contains(&"diff_fingerprints"));
    assert!(!cdp.tool_names().contains(&"diff_fingerprints"));
}

// ----------------------------------------------------------------------
// capture_fingerprint
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_capture_returns_the_fingerprint_saves_a_private_file_and_reports_progress() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path(), readings(&[])).await;
    let before = mock.calls_to("Runtime.evaluate").len();
    let seen = Arc::new(Mutex::new(Vec::<ProgressUpdate>::new()));
    let log = Arc::clone(&seen);
    let sink = ProgressSink::new(move |update| {
        log.lock().unwrap().push(update);
        std::future::ready(Ok::<(), String>(()))
    });

    let output = host
        .call_with(
            "capture_fingerprint",
            arguments(json!({ "save_to": "seen.json" })),
            Some(sink),
        )
        .await
        .unwrap();
    let Output::Structured { text, data } = output else {
        panic!("a capture has text and data")
    };

    assert!(
        data["fingerprint"]["user_agent"]
            .as_str()
            .unwrap()
            .contains("Macintosh"),
        "{data}"
    );
    assert!(data["probe"]["url"].is_string());
    assert!(data["settings_read"].as_u64().unwrap() > 5);
    assert!(data["unmapped"].is_object());
    assert!(text.starts_with("Captured "), "{text}");
    assert_eq!(
        mock.calls_to("Runtime.evaluate").len() - before,
        1,
        "one evaluation, nothing else"
    );

    let file = dir.path().join("seen.json");
    assert_eq!(data["saved"], file.display().to_string());
    let saved = Fingerprint::load(&file).unwrap();
    assert_eq!(redacted_json(&saved), data["fingerprint"]);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "a fingerprint file is private");
    }

    let updates = seen.lock().unwrap().clone();
    assert_eq!(updates.len(), 2, "{updates:?}");
    assert!(updates[0].progress < updates[1].progress);
    assert_eq!(updates[1].total, Some(2));
}

fn redacted_json(fingerprint: &Fingerprint) -> Value {
    serde_json::from_str(&fingerprint.to_json_pretty().unwrap()).unwrap()
}

#[tokio::test]
async fn a_capture_refuses_a_name_outside_the_output_folder_before_reading() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path(), readings(&[])).await;
    let before = mock.calls_to("Runtime.evaluate").len();

    let error = call(
        &host,
        "capture_fingerprint",
        json!({ "save_to": "../escape.json" }),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, ToolError::Refused(_)), "{error}");
    assert_eq!(mock.calls_to("Runtime.evaluate").len(), before);
}

#[tokio::test]
async fn the_fingerprint_tools_need_a_browser_to_read() {
    let dir = tempfile::tempdir().unwrap();
    let host = cdp::host(Settings::new(dir.path()));

    for (tool, v) in [
        ("capture_fingerprint", json!({})),
        ("compare_fingerprint", json!({ "preset": "windows" })),
        ("explain_fingerprint", json!({})),
    ] {
        let error = call(&host, tool, v).await.unwrap_err();
        assert!(matches!(error, ToolError::NoBrowser), "{tool}: {error}");
    }
}

// ----------------------------------------------------------------------
// compare_fingerprint
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_browser_presents_its_own_capture_and_not_another_systems_preset() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path(), readings(&[])).await;
    call(
        &host,
        "capture_fingerprint",
        json!({ "save_to": "seen.json" }),
    )
    .await
    .unwrap();

    let (text, data) = both(&host, "compare_fingerprint", json!({ "path": "seen.json" })).await;
    assert!(text.starts_with("presents: yes, verified:"), "{text}");
    assert_eq!(data["presents"], true);
    assert_eq!(data["differences"], json!([]));
    assert!(!data["matched"].as_array().unwrap().is_empty());

    let (text, data) = both(&host, "compare_fingerprint", json!({ "preset": "ios" })).await;
    assert!(text.starts_with("presents: no, verified: no\n"), "{text}");
    assert_eq!(data["presents"], false);
    let differences = data["differences"].as_array().unwrap();
    assert!(!differences.is_empty());
    for difference in differences {
        assert!(difference["setting"].is_string());
        assert!(difference.get("expected").is_some() && difference.get("actual").is_some());
    }
    assert!(data["unverifiable"].is_array(), "{data}");
}

#[tokio::test]
async fn a_comparison_needs_exactly_one_fingerprint_source() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path(), readings(&[])).await;

    for v in [
        json!({}),
        json!({ "preset": "windows", "path": "x.json" }),
        json!({ "preset": "windows", "fingerprint": {} }),
    ] {
        let error = call(&host, "compare_fingerprint", v.clone())
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::InvalidArgument { .. }),
            "{v}: {error}"
        );
    }
    let outside = call(
        &host,
        "compare_fingerprint",
        json!({ "path": "/etc/hosts" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(outside, ToolError::Refused(_)), "{outside}");
    let inline = call(
        &host,
        "compare_fingerprint",
        json!({ "fingerprint": { "timezone": "Antarctica/Troll" } }),
    )
    .await
    .unwrap();
    assert_eq!(inline.as_json().unwrap()["presents"], false);
}

// ----------------------------------------------------------------------
// explain_fingerprint
// ----------------------------------------------------------------------

#[tokio::test]
async fn an_explanation_names_the_signs_of_automation_and_what_to_change() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(
        dir.path(),
        readings(&["navigator_webdriver_true", "software_renderer"]),
    )
    .await;

    let (text, data) = both(&host, "explain_fingerprint", json!({})).await;

    assert!(text.starts_with("Risk: high."), "{text}");
    assert_eq!(data["risk"], "high");
    let leaks = data["leaks"].as_array().unwrap();
    assert_eq!(leaks.len(), 2);
    assert!(leaks.iter().all(|leak| leak["fix"].is_string()
        && leak["a_detector_sees"].is_string()
        && leak["severity"].is_string()));
    assert!(data["coherence"]["coherent"].is_boolean());
    assert!(data["detector_view"]["user_agent"].is_string());
    assert!(data["not_readable"].as_array().unwrap().len() >= 4);
}

#[tokio::test]
async fn a_browser_with_no_markers_has_no_risk() {
    let dir = tempfile::tempdir().unwrap();
    let (host, _mock) = running(dir.path(), readings(&[])).await;

    let (text, data) = both(&host, "explain_fingerprint", json!({})).await;

    assert_eq!(data["risk"], "none", "{text}");
    assert_eq!(data["leaks"], json!([]));
}

// ----------------------------------------------------------------------
// Tools that need no browser
// ----------------------------------------------------------------------

fn sb_host(dir: &Path) -> Host<seleniumbase_rs::BaseCase> {
    sb::host(Settings::new(dir))
}

#[tokio::test]
async fn two_fingerprints_differ_by_the_settings_the_expected_one_gives() {
    let dir = tempfile::tempdir().unwrap();
    let host = sb_host(dir.path());

    let (text, data) = both(
        &host,
        "diff_fingerprints",
        json!({ "a": "windows", "b": "macos" }),
    )
    .await;
    assert_eq!(data["identical"], false);
    assert!(!data["differences"].as_array().unwrap().is_empty());
    assert!(text.contains("differ"), "{text}");

    let (_, same) = both(
        &host,
        "diff_fingerprints",
        json!({ "a": "windows", "b": "windows" }),
    )
    .await;
    assert_eq!(same["identical"], true);

    let tokyo = json!({ "timezone": "Asia/Tokyo" }).to_string();
    let (_, tz) = both(
        &host,
        "diff_fingerprints",
        json!({ "a": tokyo, "b": "windows" }),
    )
    .await;
    assert_eq!(tz["differences"][0]["setting"], "timezone");
    assert_eq!(tz["differences"][0]["expected"], "Asia/Tokyo");

    let missing = call(&host, "diff_fingerprints", json!({ "a": "windows" }))
        .await
        .unwrap_err();
    assert!(invalid(&missing, "b"), "{missing}");
}

#[tokio::test]
async fn a_patch_merges_removes_and_reports_the_key_it_cannot_read() {
    let dir = tempfile::tempdir().unwrap();
    let host = sb_host(dir.path());

    let (_, data) = both(
        &host,
        "patch_fingerprint",
        json!({ "preset": "windows", "patch": { "timezone": "Asia/Tokyo" } }),
    )
    .await;
    assert_eq!(data["fingerprint"]["timezone"], "Asia/Tokyo");
    assert!(data["valid"].is_boolean());
    assert!(data["errors"].is_array() && data["warnings"].is_array());

    let (_, removed) = both(
        &host,
        "patch_fingerprint",
        json!({ "preset": "windows", "patch": { "timezone": null } }),
    )
    .await;
    assert!(
        removed["fingerprint"].get("timezone").is_none(),
        "a null removes the setting: {removed}"
    );

    let bad = call(
        &host,
        "patch_fingerprint",
        json!({ "preset": "windows", "patch": { "screen_width": 1920 } }),
    )
    .await
    .unwrap_err();
    assert!(invalid(&bad, "patch"), "{bad}");
    assert!(bad.to_string().contains("screen_width"), "{bad}");
}

#[tokio::test]
async fn a_proxy_password_is_shown_as_stars_and_kept_in_the_saved_file() {
    let dir = tempfile::tempdir().unwrap();
    let host = sb_host(dir.path());

    let output = call(
        &host,
        "patch_fingerprint",
        json!({
            "preset": "windows",
            "patch": { "proxy": { "type": "http", "host": "p.test", "port": 8080,
                "username": "u", "password": "hunter2", "save_traffic": false } },
            "save_to": "proxied.json",
        }),
    )
    .await
    .unwrap();

    let Output::Structured { text, data } = output else {
        panic!("a patch has text and data")
    };
    assert_eq!(data["fingerprint"]["proxy"]["password"], "***");
    assert!(!text.contains("hunter2") && !data.to_string().contains("hunter2"));
    let file = dir.path().join("proxied.json");
    assert!(std::fs::read_to_string(&file).unwrap().contains("hunter2"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[tokio::test]
async fn a_random_identity_is_the_one_sbrs_fingerprint_export_writes() {
    let dir = tempfile::tempdir().unwrap();
    let host = sb_host(dir.path());

    let output = call(
        &host,
        "build_fingerprint",
        json!({ "random": true, "seed": 7, "os": "macos", "save_to": "drawn.json" }),
    )
    .await
    .unwrap();

    let exported = export(&ExportSource::Random {
        os: Some(OsType::Macos),
        seed: Some(7),
    })
    .unwrap();
    assert_eq!(output.as_json().unwrap(), &redacted_json(&exported));
    assert_eq!(
        Fingerprint::load(dir.path().join("drawn.json")).unwrap(),
        exported
    );

    for bad in [
        json!({ "seed": 7 }),
        json!({ "os": "macos" }),
        json!({ "random": true, "preset": "windows" }),
    ] {
        let error = call(&host, "build_fingerprint", bad.clone())
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::InvalidArgument { .. }),
            "{bad}: {error}"
        );
    }
    // The presets and the overrides keep working.
    let windows = call(&host, "build_fingerprint", json!({ "screen_width": 800 }))
        .await
        .unwrap();
    assert_eq!(windows.as_json().unwrap()["screen"]["width"], 800);
}

#[tokio::test]
async fn validate_reads_a_fingerprint_object_a_file_or_a_preset() {
    let dir = tempfile::tempdir().unwrap();
    let host = sb_host(dir.path());
    Fingerprint::macos_desktop()
        .save(dir.path().join("mac.json"))
        .unwrap();

    for v in [
        json!({ "preset": "macos" }),
        json!({ "path": "mac.json" }),
        json!({ "fingerprint": { "timezone": "Europe/Berlin" } }),
    ] {
        let report = call(&host, "validate_fingerprint", v.clone())
            .await
            .unwrap();
        let report = report.as_json().unwrap();
        assert!(report["coherent"].is_boolean(), "{v}: {report}");
        assert!(report["errors"].is_array());
    }
    let error = call(&host, "validate_fingerprint", json!({}))
        .await
        .unwrap_err();
    assert!(
        matches!(error, ToolError::InvalidArgument { .. }),
        "{error}"
    );
}

// ----------------------------------------------------------------------
// start_browser
// ----------------------------------------------------------------------

#[tokio::test]
async fn start_browser_refuses_a_bad_fingerprint_before_it_launches_anything() {
    let dir = tempfile::tempdir().unwrap();
    let cdp = cdp::host(Settings::new(dir.path()));
    let driver = driver::host(Settings::new(dir.path()));
    let sb = sb::host(Settings::new(dir.path()));

    for v in [
        json!({ "fingerprint_preset": "windows", "fingerprint_path": "x.json" }),
        json!({ "fingerprint_preset": "beos" }),
        json!({ "fingerprint": { "screen_width": 1920 } }),
        json!({ "fingerprint_path": "missing.json" }),
    ] {
        for (server, result) in [
            ("cdp", call(&cdp, "start_browser", v.clone()).await),
            ("driver", call(&driver, "start_browser", v.clone()).await),
            ("sb", call(&sb, "start_browser", v.clone()).await),
        ] {
            let error = result.unwrap_err();
            assert!(
                matches!(error, ToolError::InvalidArgument { .. }),
                "{server}: {v}: {error}"
            );
        }
    }

    assert!(
        cdp.ctx().try_session().await.is_none(),
        "nothing was started"
    );
}
