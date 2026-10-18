//! `Page::capture_fingerprint` and `Page::compare_fingerprint` against a
//! scripted browser, with no Chrome.
//!
//! The mock answers the one `Runtime.evaluate` the capture sends with a
//! recorded-shape reading (`tests/fixtures/fingerprint`), so the tests check what
//! reaches the tab and what comes back, not what a real browser would read. The
//! real-browser run is `tests/sb_cdp_capture_chrome.rs`. Run these with
//! `cargo test --features test-util --test sb_cdp_capture`.

#![cfg(feature = "test-util")]

use seleniumbase_rs::sb_cdp::{Browser, MockCtrl, Page};
use seleniumbase_rs::{BrowserType, Fingerprint, OsType, SeleniumBaseError};
use serde_json::{json, Value};

const CHROME_MACOS: &str = include_str!("fixtures/fingerprint/probe_chrome_macos.json");

async fn page_that_reads(readings: &str) -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    let value: Value = serde_json::from_str(readings).expect("a fixture is JSON");
    mock.reply("Runtime.evaluate", json!({ "result": { "value": value } }));
    let page = browser.default_page().await.expect("the first tab");
    (page, mock)
}

#[tokio::test]
async fn a_capture_sends_one_awaited_evaluation_to_the_tab_and_nothing_else() {
    let (page, mock) = page_that_reads(CHROME_MACOS).await;
    let before = mock.calls().len();

    page.capture_fingerprint().await.unwrap();

    let sent: Vec<_> = mock.calls().into_iter().skip(before).collect();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].method, "Runtime.evaluate");
    assert_eq!(sent[0].session_id.as_deref(), Some("S1"), "to the tab");
    assert_eq!(sent[0].params["awaitPromise"], json!(true));
    assert_eq!(sent[0].params["returnByValue"], json!(true));
    let expression = sent[0].params["expression"].as_str().unwrap();
    assert!(expression.starts_with("(async function () {"));
    assert!(expression.contains("getHighEntropyValues"));
}

#[tokio::test]
async fn the_readings_come_back_as_a_fingerprint_with_the_page_and_the_browser() {
    let (page, _mock) = page_that_reads(CHROME_MACOS).await;

    let captured = page.capture_fingerprint().await.unwrap();

    let fp = &captured.fingerprint;
    assert_eq!(fp.platform().as_str(), Some("MacIntel"));
    assert_eq!(fp.os_type(), OsType::Macos);
    assert_eq!(fp.browser_type(), BrowserType::Chromium);
    assert_eq!(fp.core_version(), Some(155));
    assert_eq!(fp.timezone().as_str(), Some("Europe/Berlin"));
    assert!(fp.client_hints().is_value());
    assert_eq!(captured.probe.url, "https://example.test/");
    assert_eq!(captured.probe.browser_version.as_deref(), Some("155.0.8059.40"));
    assert!(
        captured.probe.captured_at.ends_with('Z') && captured.probe.captured_at.contains('T'),
        "an RFC 3339 time in UTC: {}",
        captured.probe.captured_at
    );
    assert_eq!(captured.unmapped["canvas_hash"], json!("5fa3c2d1"));
}

#[tokio::test]
async fn a_capture_is_the_one_file_format_once_saved() {
    let (page, _mock) = page_that_reads(CHROME_MACOS).await;
    let captured = page.capture_fingerprint().await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("real.json");

    captured.fingerprint.save(&path).unwrap();

    assert_eq!(Fingerprint::load(&path).unwrap(), captured.fingerprint);
}

#[tokio::test]
async fn a_script_that_throws_is_a_protocol_error() {
    let (browser, mock) = Browser::new_mocked();
    mock.reply(
        "Runtime.evaluate",
        json!({ "exceptionDetails": { "exception": { "description": "ReferenceError: x" } } }),
    );
    let page = browser.default_page().await.unwrap();

    let error = page.capture_fingerprint().await.unwrap_err();

    assert!(matches!(error, SeleniumBaseError::CdpDriver(_)), "{error:?}");
    assert!(error.to_string().contains("ReferenceError: x"), "{error}");
}

#[tokio::test]
async fn readings_of_another_version_are_a_stealth_error_not_a_fingerprint() {
    let (page, _mock) = page_that_reads(r#"{ "version": 9 }"#).await;

    let error = page.capture_fingerprint().await.unwrap_err();

    assert!(matches!(error, SeleniumBaseError::Stealth { .. }), "{error:?}");
}

#[tokio::test]
async fn a_failure_the_script_reports_is_named() {
    let (page, _mock) = page_that_reads(r#"{ "version": 1, "failure": "boom" }"#).await;

    let error = page.capture_fingerprint().await.unwrap_err();

    assert!(error.to_string().contains("boom"), "{error}");
}

#[tokio::test]
async fn comparing_a_fingerprint_with_the_live_page_lists_what_differs_and_what_cannot_be_read() {
    let (page, _mock) = page_that_reads(CHROME_MACOS).await;
    // The browser is a Mac in Berlin with 8 cores; the claim is a Windows PC in
    // New York with 8 cores and a position.
    let claimed = Fingerprint::windows_desktop()
        .to_builder()
        .geolocation((40.7128, -74.006))
        .build()
        .unwrap();

    let comparison = page.compare_fingerprint(&claimed).await.unwrap();

    assert!(!comparison.presents());
    let differing: Vec<_> = comparison.differences.iter().map(|d| d.setting).collect();
    for setting in ["user_agent", "platform", "timezone", "webgl", "screen"] {
        assert!(differing.contains(&setting), "{setting} in {differing:?}");
    }
    assert!(!differing.contains(&"hardware_concurrency"), "both have 8 cores");
    assert!(comparison.matched.contains(&"hardware_concurrency"));
    assert!(
        comparison.unverifiable.contains(&"geolocation"),
        "a page cannot read the position: {:?}",
        comparison.unverifiable
    );
    assert!(!comparison.is_verified());
}

#[tokio::test]
async fn the_fingerprint_a_capture_gave_is_presented_by_the_page_it_came_from() {
    let (page, _mock) = page_that_reads(CHROME_MACOS).await;
    let captured = page.capture_fingerprint().await.unwrap();

    let comparison = page.compare_fingerprint(&captured.fingerprint).await.unwrap();

    assert!(comparison.presents(), "{:?}", comparison.differences);
    assert_eq!(
        comparison.unverifiable,
        ["accept_languages"],
        "only the header a Chromium capture derived cannot be read back"
    );
}
