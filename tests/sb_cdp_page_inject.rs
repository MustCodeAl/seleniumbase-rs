//! `Page::add_script_tag`, `add_style_tag`, `add_init_script` and
//! `remove_init_script`, against a scripted browser, with no Chrome.
//!
//! The tests read back the protocol commands the mock received: which command
//! reaches which tab, and that what a caller gives ends up in the page as a
//! string, never as code. Run them with
//! `cargo test --features test-util --test sb_cdp_page_inject`.

#![cfg(feature = "test-util")]

use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, Call, MockCtrl, Page, TagSource};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};

/// The first tab of a mocked browser.
async fn page() -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    let page = browser.default_page().await.unwrap();
    (page, mock)
}

/// The `Runtime.evaluate` commands whose expression contains `needle`.
fn evaluated(mock: &MockCtrl, needle: &str) -> Vec<Call> {
    mock.calls_to("Runtime.evaluate")
        .into_iter()
        .filter(|call| {
            call.params["expression"]
                .as_str()
                .is_some_and(|expression| expression.contains(needle))
        })
        .collect()
}

/// The expression of the one evaluation that contains `needle`.
fn the_expression(mock: &MockCtrl, needle: &str) -> String {
    let calls = evaluated(mock, needle);
    assert_eq!(calls.len(), 1, "one evaluation with {needle:?}: {calls:?}");
    assert_eq!(
        calls[0].session_id.as_deref(),
        Some("S1"),
        "sent to the tab"
    );
    assert_eq!(
        calls[0].params["awaitPromise"],
        json!(true),
        "the tag is waited for"
    );
    calls[0].params["expression"].as_str().unwrap().to_owned()
}

/// Makes every evaluation that contains `needle` throw `description`.
fn throw_on(mock: &MockCtrl, needle: &'static str, description: &'static str) {
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains(needle) {
            json!({ "exceptionDetails": { "exception": { "description": description } } })
        } else {
            json!({ "result": { "value": Value::Null } })
        })
    });
}

#[tokio::test]
async fn an_init_script_is_added_to_this_tab_and_removed_by_its_identifier() {
    let (page, mock) = page().await;
    mock.reply(
        "Page.addScriptToEvaluateOnNewDocument",
        json!({ "identifier": "7" }),
    );

    let id = page.add_init_script("window.early = 1;").await.unwrap();

    assert_eq!(id.as_str(), "7");
    let added = mock.calls_to("Page.addScriptToEvaluateOnNewDocument");
    let ours = added.last().unwrap();
    assert_eq!(ours.params, json!({ "source": "window.early = 1;" }));
    assert_eq!(ours.session_id.as_deref(), Some("S1"), "for this tab only");
    assert!(
        evaluated(&mock, "window.early").is_empty(),
        "the document already loaded is left as it is, as in Playwright"
    );

    page.remove_init_script(&id).await.unwrap();

    let removed = mock.calls_to("Page.removeScriptToEvaluateOnNewDocument");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].params, json!({ "identifier": "7" }));
    assert_eq!(removed[0].session_id.as_deref(), Some("S1"));
}

#[tokio::test]
async fn an_init_script_without_an_identifier_or_refused_is_an_error() {
    let (page, mock) = page().await;
    // The mock answers `{}` unless told otherwise: nothing to remove it by.
    let error = page.add_init_script("1").await.unwrap_err();
    assert!(
        matches!(&error, SeleniumBaseError::CdpDriver(message) if message.contains("identifier")),
        "{error:?}"
    );

    mock.fail("Page.addScriptToEvaluateOnNewDocument", "no such target");
    let error = page.add_init_script("1").await.unwrap_err();
    assert!(
        matches!(&error, SeleniumBaseError::CdpDriver(message) if message.contains("no such target")),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_script_tag_from_a_url_waits_for_the_file_to_load_or_fail() {
    let (page, mock) = page().await;
    let page = page.with_timeout(Duration::from_millis(1500));
    let url = "https://cdn.example.com/lib@1.2.3/lib.min.js";

    page.add_script_tag(TagSource::url(url)).await.unwrap();

    let expression = the_expression(&mock, "lib.min.js");
    assert!(
        expression.contains(&serde_json::to_string(url).unwrap()),
        "{expression}"
    );
    assert!(
        expression.contains("createElement('script')"),
        "{expression}"
    );
    assert!(expression.contains(".src = "), "{expression}");
    assert!(expression.contains("'load'"), "{expression}");
    assert!(expression.contains("'error'"), "{expression}");
    assert!(
        expression.contains("1500"),
        "it gives up after the page's timeout: {expression}"
    );
}

#[tokio::test]
async fn a_script_file_that_does_not_load_is_an_error() {
    let (page, mock) = page().await;
    throw_on(
        &mock,
        "missing.js",
        "Error: the script at https://example.com/missing.js did not load",
    );

    let error = page
        .add_script_tag(TagSource::url("https://example.com/missing.js"))
        .await
        .unwrap_err();

    assert!(
        matches!(&error, SeleniumBaseError::CdpDriver(message) if message.contains("did not load")),
        "{error:?}"
    );
}

#[tokio::test]
async fn script_content_reaches_the_tag_as_a_string_and_its_error_comes_back() {
    let (page, mock) = page().await;
    let code = r#"window.quoted = "a</script>'b\n";"#;

    page.add_script_tag(TagSource::content(code)).await.unwrap();

    let expression = the_expression(&mock, "window.quoted");
    assert!(
        expression.contains(&serde_json::to_string(code).unwrap()),
        "the code is a JSON string literal in the expression: {expression}"
    );
    assert!(expression.contains("textContent"), "{expression}");
    assert!(!expression.contains(".src = "), "{expression}");

    throw_on(&mock, "boom()", "ReferenceError: boom is not defined");
    let error = page
        .add_script_tag(TagSource::content("boom();"))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, SeleniumBaseError::CdpDriver(message) if message.contains("boom is not defined")),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_style_tag_from_a_url_is_a_stylesheet_link_that_is_waited_for() {
    let (page, mock) = page().await;
    let url = "https://cdn.example.com/intro.js@7.2.0/introjs.min.css";

    page.add_style_tag(TagSource::url(url)).await.unwrap();

    let expression = the_expression(&mock, "introjs.min.css");
    assert!(
        expression.contains(&serde_json::to_string(url).unwrap()),
        "{expression}"
    );
    assert!(expression.contains("createElement('link')"), "{expression}");
    assert!(expression.contains("'stylesheet'"), "{expression}");
    assert!(expression.contains("'load'"), "{expression}");
    assert!(expression.contains("'error'"), "{expression}");

    throw_on(
        &mock,
        "gone.css",
        "Error: the stylesheet at https://example.com/gone.css did not load",
    );
    let error = page
        .add_style_tag(TagSource::url("https://example.com/gone.css"))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, SeleniumBaseError::CdpDriver(message) if message.contains("did not load")),
        "{error:?}"
    );
}

#[tokio::test]
async fn style_content_goes_into_a_style_element() {
    let (page, mock) = page().await;
    let css = "body { zoom: 0.9 } /* \"quoted\" */";

    page.add_style_tag(TagSource::content(css)).await.unwrap();

    let expression = the_expression(&mock, "zoom: 0.9");
    assert!(
        expression.contains(&serde_json::to_string(css).unwrap()),
        "{expression}"
    );
    assert!(
        expression.contains("createElement('style')"),
        "{expression}"
    );
    assert!(!expression.contains("'stylesheet'"), "{expression}");
}
