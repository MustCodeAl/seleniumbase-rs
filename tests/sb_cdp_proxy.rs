//! Per-context proxies and their passwords, against a scripted browser.
//! Run with `cargo test --features test-util`.

#![cfg(feature = "test-util")]

use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, ContextOptions, MockCtrl, Proxy, Rule};
use serde_json::json;

fn browser() -> (Browser, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    mock.reply(
        "Target.createBrowserContext",
        json!({ "browserContextId": "CTX1" }),
    );
    (browser, mock)
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

fn challenge(mock: &MockCtrl) {
    mock.emit(
        "Fetch.authRequired",
        json!({ "requestId": "AUTH1" }),
        Some("S1"),
    );
}

fn paused(mock: &MockCtrl, id: &str) {
    mock.emit(
        "Fetch.requestPaused",
        json!({ "requestId": id, "resourceType": "Document", "request": { "url": "http://t.test/", "method": "GET" } }),
        Some("S1"),
    );
}

fn proxied() -> ContextOptions {
    ContextOptions::new()
        .proxy(Proxy::parse("alice:s3cret@proxy.test:8080").unwrap())
        .bypass("*.internal.test")
        .bypass("localhost")
}

#[tokio::test(start_paused = true)]
async fn a_context_proxy_is_sent_without_its_password() {
    let (browser, mock) = browser();

    browser.new_context_with(proxied()).await.unwrap();

    let params = &mock.calls_to("Target.createBrowserContext")[0].params;
    assert_eq!(params["proxyServer"], "proxy.test:8080");
    assert_eq!(params["proxyBypassList"], "*.internal.test;localhost");
    assert!(!params.to_string().contains("s3cret") && !params.to_string().contains("alice"));
}

#[tokio::test(start_paused = true)]
async fn a_plain_context_asks_for_no_proxy() {
    let (browser, mock) = browser();

    browser.new_context().await.unwrap();

    assert_eq!(
        mock.calls_to("Target.createBrowserContext")[0].params,
        json!({})
    );
}

#[tokio::test(start_paused = true)]
async fn a_proxied_tab_has_auth_handling_on_before_its_first_request() {
    let (browser, mock) = browser();
    let context = browser.new_context_with(proxied()).await.unwrap();

    context.new_page(Some("http://t.test/")).await.unwrap();

    let calls = mock.calls();
    let enable = calls
        .iter()
        .position(|c| c.method == "Fetch.enable")
        .expect("Fetch is enabled");
    let navigate = calls
        .iter()
        .position(|c| c.method == "Page.navigate")
        .expect("the tab navigates");
    assert!(
        enable < navigate,
        "auth must be ready before the first request"
    );
    assert_eq!(calls[enable].params["handleAuthRequests"], true);
    assert_eq!(
        calls[enable].session_id.as_deref(),
        Some("S1"),
        "enabled on the tab, not the browser"
    );
}

#[tokio::test(start_paused = true)]
async fn the_proxys_password_prompt_is_answered_with_its_credentials() {
    let (browser, mock) = browser();
    let context = browser.new_context_with(proxied()).await.unwrap();
    context.new_page(None::<&str>).await.unwrap();

    challenge(&mock);
    settle().await;

    let answer = &mock.calls_to("Fetch.continueWithAuth")[0].params;
    assert_eq!(answer["requestId"], "AUTH1");
    assert_eq!(answer["authChallengeResponse"]["username"], "alice");
    assert_eq!(answer["authChallengeResponse"]["password"], "s3cret");
}

#[tokio::test(start_paused = true)]
async fn requests_paused_by_auth_handling_are_released() {
    let (browser, mock) = browser();
    let context = browser.new_context_with(proxied()).await.unwrap();
    context.new_page(None::<&str>).await.unwrap();

    paused(&mock, "R1");
    settle().await;

    assert_eq!(
        mock.calls_to("Fetch.continueRequest")[0].params,
        json!({ "requestId": "R1" })
    );
}

#[tokio::test(start_paused = true)]
async fn the_password_appears_only_in_the_answer_to_a_prompt() {
    let (browser, mock) = browser();
    let context = browser.new_context_with(proxied()).await.unwrap();
    let page = context.new_page(Some("http://t.test/")).await.unwrap();
    challenge(&mock);
    settle().await;

    for call in mock.calls() {
        if call.method == "Fetch.continueWithAuth" {
            continue;
        }
        assert!(
            !call.params.to_string().contains("s3cret"),
            "{} leaked the password",
            call.method
        );
    }
    assert!(!format!("{browser:?} {context:?} {page:?}").contains("s3cret"));
}

#[tokio::test(start_paused = true)]
async fn a_tab_in_an_unproxied_context_gets_no_auth_handling() {
    let (browser, mock) = browser();
    let context = browser.new_context().await.unwrap();

    context.new_page(None::<&str>).await.unwrap();
    challenge(&mock);
    settle().await;

    assert!(mock.calls_to("Fetch.enable").is_empty());
    assert!(
        mock.calls_to("Fetch.continueWithAuth").is_empty(),
        "nobody has a password to give"
    );
}

#[tokio::test(start_paused = true)]
async fn an_interception_owns_the_requests_but_the_password_is_still_supplied() {
    let (browser, mock) = browser();
    let context = browser.new_context_with(proxied()).await.unwrap();
    let page = context.new_page(None::<&str>).await.unwrap();
    let interception = page.intercept(vec![Rule::block()]).await.unwrap();

    paused(&mock, "R2");
    challenge(&mock);
    settle().await;

    assert_eq!(
        mock.calls_to("Fetch.failRequest").len(),
        1,
        "the interception answers the request"
    );
    assert!(
        mock.calls_to("Fetch.continueRequest").is_empty(),
        "the auth responder must not also continue it"
    );
    assert_eq!(mock.calls_to("Fetch.continueWithAuth").len(), 1);
    let enables = mock.calls_to("Fetch.enable");
    assert_eq!(
        enables.last().unwrap().params["handleAuthRequests"],
        true,
        "interception keeps auth on"
    );
    drop(interception);
}

#[tokio::test(start_paused = true)]
async fn stopping_an_interception_restores_the_tabs_password_handling() {
    let (browser, mock) = browser();
    let context = browser.new_context_with(proxied()).await.unwrap();
    let page = context.new_page(None::<&str>).await.unwrap();
    let interception = page.intercept(vec![Rule::block()]).await.unwrap();

    interception.stop().await.unwrap();
    paused(&mock, "R3");
    challenge(&mock);
    settle().await;

    let calls = mock.calls();
    let disable = calls
        .iter()
        .rposition(|c| c.method == "Fetch.disable")
        .expect("Fetch was disabled");
    let reenable = calls
        .iter()
        .rposition(|c| c.method == "Fetch.enable")
        .expect("and re-enabled");
    assert!(
        reenable > disable,
        "auth handling comes back after interception ends"
    );
    assert_eq!(
        mock.calls_to("Fetch.continueRequest").len(),
        1,
        "requests are released again"
    );
    assert_eq!(mock.calls_to("Fetch.continueWithAuth").len(), 1);
}

#[tokio::test(start_paused = true)]
async fn disposing_a_context_forgets_its_password() {
    let (browser, mock) = browser();
    let context = browser.new_context_with(proxied()).await.unwrap();
    context.new_page(None::<&str>).await.unwrap();

    context.dispose().await.unwrap();
    challenge(&mock);
    settle().await;

    assert!(
        mock.calls_to("Fetch.continueWithAuth").is_empty(),
        "a gone tab's password must not linger"
    );
}

#[test]
fn context_options_never_print_the_password() {
    let shown = format!("{:?}", proxied());
    assert!(!shown.contains("s3cret"), "{shown}");
    assert!(shown.contains("proxy.test"), "{shown}");
}
