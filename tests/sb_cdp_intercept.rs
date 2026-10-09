//! Request interception against a scripted browser, with no Chrome.
//! Run with `cargo test --features test-util`.

#![cfg(feature = "test-util")]

use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, MockCtrl, Outcome, Page, ResourceType, Response, Rule};
use serde_json::{json, Value};

async fn page() -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    (browser.default_page().await.unwrap(), mock)
}

/// Pauses a request the way Chrome does.
fn pause(mock: &MockCtrl, id: &str, url: &str, method: &str, kind: &str) {
    mock.emit(
        "Fetch.requestPaused",
        json!({
            "requestId": id,
            "resourceType": kind,
            "request": { "url": url, "method": method, "headers": { "Accept": "*/*", "Cookie": "sid=1" } },
        }),
        Some("S1"),
    );
}

/// Lets the interception task answer what it was sent.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

fn only(mock: &MockCtrl, method: &str) -> Value {
    let calls = mock.calls_to(method);
    assert_eq!(calls.len(), 1, "expected one {method}, got {calls:?}");
    calls[0].params.clone()
}

#[tokio::test(start_paused = true)]
async fn starting_enables_the_fetch_domain_for_every_url() {
    let (page, mock) = page().await;

    let _interception = page.intercept(vec![Rule::block()]).await.unwrap();

    let enable = only(&mock, "Fetch.enable");
    assert_eq!(enable["patterns"][0]["urlPattern"], "*");
    assert_eq!(
        enable["handleAuthRequests"], false,
        "no proxy credentials, nothing to answer"
    );
}

#[tokio::test(start_paused = true)]
async fn a_blocked_request_is_failed_as_blocked_by_the_client() {
    let (page, mock) = page().await;
    let _i = page
        .intercept(vec![Rule::block().resource_type(ResourceType::Image)])
        .await
        .unwrap();

    pause(&mock, "R1", "https://a.test/logo.png", "GET", "Image");
    settle().await;

    let failed = only(&mock, "Fetch.failRequest");
    assert_eq!(failed["requestId"], "R1");
    assert_eq!(failed["errorReason"], "BlockedByClient");
    assert!(mock.calls_to("Fetch.continueRequest").is_empty());
}

#[tokio::test(start_paused = true)]
async fn a_fulfilled_request_gets_the_made_up_response() {
    let (page, mock) = page().await;
    let _i = page
        .intercept(vec![Rule::fulfill(Response::json(
            200,
            &json!({"plan": "pro"}),
        ))
        .url("*/api/account")])
        .await
        .unwrap();

    pause(&mock, "R2", "https://a.test/api/account", "GET", "Fetch");
    settle().await;

    let fulfilled = only(&mock, "Fetch.fulfillRequest");
    assert_eq!(fulfilled["requestId"], "R2");
    assert_eq!(fulfilled["responseCode"], 200);
    assert!(fulfilled["body"].as_str().is_some_and(|b| !b.is_empty()));
}

#[tokio::test(start_paused = true)]
async fn a_modified_request_continues_with_its_headers_changed() {
    let (page, mock) = page().await;
    let _i = page
        .intercept(vec![Rule::modify()
            .set_header("X-Run", "42")
            .remove_header("Cookie")])
        .await
        .unwrap();

    pause(&mock, "R3", "https://a.test/", "GET", "Document");
    settle().await;

    let sent = only(&mock, "Fetch.continueRequest");
    let names: Vec<_> = sent["headers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["name"].as_str().unwrap().to_owned())
        .collect();
    assert!(names.contains(&"X-Run".to_owned()));
    assert!(
        names.contains(&"Accept".to_owned()),
        "untouched headers are kept"
    );
    assert!(!names.contains(&"Cookie".to_owned()));
}

#[tokio::test(start_paused = true)]
async fn a_request_no_rule_matches_continues_untouched() {
    let (page, mock) = page().await;
    let _i = page
        .intercept(vec![Rule::block().url("*.png")])
        .await
        .unwrap();

    pause(&mock, "R4", "https://a.test/page.html", "GET", "Document");
    settle().await;

    let sent = only(&mock, "Fetch.continueRequest");
    assert_eq!(sent, json!({ "requestId": "R4" }));
}

#[tokio::test(start_paused = true)]
async fn the_first_matching_rule_wins() {
    let (page, mock) = page().await;
    let _i = page
        .intercept(vec![
            Rule::fulfill(Response::new(204, "")).url("https://a.test/*"),
            Rule::block(),
        ])
        .await
        .unwrap();

    pause(&mock, "R5", "https://a.test/x", "GET", "Fetch");
    pause(&mock, "R6", "https://b.test/x", "GET", "Fetch");
    settle().await;

    assert_eq!(only(&mock, "Fetch.fulfillRequest")["requestId"], "R5");
    assert_eq!(only(&mock, "Fetch.failRequest")["requestId"], "R6");
}

#[tokio::test(start_paused = true)]
async fn the_log_records_each_request_and_how_it_was_answered() {
    let (page, mock) = page().await;
    let interception = page
        .intercept(vec![
            Rule::block().resource_type(ResourceType::Image),
            Rule::modify().set_header("X", "1").method("POST"),
        ])
        .await
        .unwrap();

    pause(&mock, "A", "https://a.test/i.png", "GET", "Image");
    pause(&mock, "B", "https://a.test/form", "POST", "Fetch");
    pause(&mock, "C", "https://a.test/", "GET", "Document");
    settle().await;

    let log = interception.log();
    let outcomes: Vec<_> = log
        .iter()
        .map(|seen| (seen.request.url.as_str(), seen.outcome))
        .collect();
    assert_eq!(
        outcomes,
        [
            ("https://a.test/i.png", Outcome::Blocked),
            ("https://a.test/form", Outcome::Modified),
            ("https://a.test/", Outcome::Continued),
        ]
    );
    assert_eq!(log[1].request.method, "POST");
    assert_eq!(log[0].request.resource_type, ResourceType::Image);
}

#[tokio::test(start_paused = true)]
async fn requests_paused_for_another_tab_are_left_alone() {
    let (page, mock) = page().await;
    let _i = page.intercept(vec![Rule::block()]).await.unwrap();

    mock.emit(
        "Fetch.requestPaused",
        json!({ "requestId": "X", "resourceType": "Image", "request": { "url": "https://a.test/", "method": "GET" } }),
        Some("SOME-OTHER-TAB"),
    );
    settle().await;

    assert!(
        mock.calls_to("Fetch.failRequest").is_empty(),
        "interception is per page"
    );
}

#[tokio::test(start_paused = true)]
async fn one_failed_answer_does_not_stop_the_next_request_being_handled() {
    let (page, mock) = page().await;
    let interception = page.intercept(vec![Rule::block()]).await.unwrap();
    mock.fail("Fetch.failRequest", "request already gone");

    pause(&mock, "R7", "https://a.test/1", "GET", "Image");
    settle().await;
    mock.reply("Fetch.failRequest", json!({}));
    pause(&mock, "R8", "https://a.test/2", "GET", "Image");
    settle().await;

    assert_eq!(
        mock.calls_to("Fetch.failRequest").len(),
        2,
        "both were attempted"
    );
    assert_eq!(interception.log().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn stopping_disables_fetch_and_later_requests_are_ignored() {
    let (page, mock) = page().await;
    let interception = page.intercept(vec![Rule::block()]).await.unwrap();

    interception.stop().await.unwrap();
    pause(&mock, "R9", "https://a.test/", "GET", "Image");
    settle().await;

    assert_eq!(mock.calls_to("Fetch.disable").len(), 1);
    assert!(mock.calls_to("Fetch.failRequest").is_empty());
}

#[tokio::test(start_paused = true)]
async fn dropping_an_interception_also_disables_fetch() {
    let (page, mock) = page().await;
    {
        let _interception = page.intercept(vec![Rule::block()]).await.unwrap();
    }
    settle().await;

    assert_eq!(mock.calls_to("Fetch.disable").len(), 1);
}
