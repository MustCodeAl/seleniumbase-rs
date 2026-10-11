//! Tests of the context-per-proxy egress switch against a scripted browser, with no Chrome.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::sb_cdp::MockCtrl;
use crate::scrape::egress::geo::CountryCode;

/// Makes the mocked browser hand out the contexts `C1`, `C2`, and so on.
fn script_contexts(mock: &MockCtrl) {
    let made = Arc::new(AtomicUsize::new(0));
    mock.on("Target.createBrowserContext", move |_| {
        let id = made.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(json!({ "browserContextId": format!("C{id}") }))
    });
}

fn egress(spec: &str) -> EgressRef {
    let country = CountryCode::new("DE").expect("a valid code");
    EgressRef::parse(spec, Some(country)).expect("a valid spec")
}

fn methods(mock: &MockCtrl) -> Vec<String> {
    mock.calls().into_iter().map(|call| call.method).collect()
}

fn position(methods: &[String], wanted: &str, nth: usize) -> usize {
    methods
        .iter()
        .enumerate()
        .filter(|(_, method)| method.as_str() == wanted)
        .nth(nth)
        .map_or_else(
            || panic!("{wanted} #{nth} was never sent: {methods:?}"),
            |(at, _)| at,
        )
}

#[tokio::test]
async fn a_switch_opens_a_context_behind_the_proxy_and_a_tab_in_it() {
    let (browser, mock) = Browser::new_mocked();
    script_contexts(&mock);

    let page = ContextProxySwitch::new()
        .switch(&browser, Some(&egress("http://proxy.example:8080")))
        .await
        .expect("the switch succeeds");

    let created = mock.calls_to("Target.createBrowserContext");
    assert_eq!(created.len(), 1);
    assert_eq!(
        created[0].params,
        json!({ "proxyServer": "http://proxy.example:8080" })
    );
    let tab = mock.calls_to("Target.createTarget");
    assert_eq!(tab[0].params["browserContextId"], "C1");
    assert_eq!(page.id(), "T2");
}

#[tokio::test]
async fn a_proxy_password_is_never_sent_as_part_of_the_server_or_printed() {
    let (browser, mock) = Browser::new_mocked();
    script_contexts(&mock);
    let switch = ContextProxySwitch::new();

    switch
        .switch(
            &browser,
            Some(&egress("http://alice:s3cret@proxy.example:8080")),
        )
        .await
        .expect("the switch succeeds");

    let created = mock.calls_to("Target.createBrowserContext");
    assert_eq!(
        created[0].params["proxyServer"],
        "http://proxy.example:8080"
    );
    for call in mock.calls() {
        let sent = call.params.to_string();
        assert!(
            !sent.contains("s3cret") && !sent.contains("alice"),
            "{sent}"
        );
    }
    let auth = mock.calls_to("Fetch.enable");
    assert_eq!(
        auth.len(),
        1,
        "the tab answers its own proxy's password prompt"
    );
    assert_eq!(auth[0].params["handleAuthRequests"], true);
    let shown = format!("{switch:?}");
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
}

#[tokio::test]
async fn the_old_context_goes_only_after_the_new_one_is_ready() {
    let (browser, mock) = Browser::new_mocked();
    script_contexts(&mock);
    let switch = ContextProxySwitch::new();

    switch
        .switch(&browser, Some(&egress("http://one.example:8080")))
        .await
        .expect("the first switch succeeds");
    switch
        .switch(&browser, Some(&egress("http://two.example:8080")))
        .await
        .expect("the second switch succeeds");

    let sent = methods(&mock);
    let second_context = position(&sent, "Target.createBrowserContext", 1);
    let second_tab = position(&sent, "Target.createTarget", 1);
    let disposed = position(&sent, "Target.disposeBrowserContext", 0);
    assert!(
        second_context < second_tab && second_tab < disposed,
        "{sent:?}"
    );
    let gone = mock.calls_to("Target.disposeBrowserContext");
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].params["browserContextId"], "C1");
}

#[tokio::test]
async fn a_direct_switch_is_a_context_without_a_proxy() {
    let (browser, mock) = Browser::new_mocked();
    script_contexts(&mock);

    ContextProxySwitch::new()
        .switch(&browser, None)
        .await
        .expect("the switch succeeds");

    let created = mock.calls_to("Target.createBrowserContext");
    assert_eq!(created[0].params, json!({}));
}

#[tokio::test]
async fn a_tab_that_cannot_open_discards_the_new_context_and_keeps_the_old_one() {
    let (browser, mock) = Browser::new_mocked();
    script_contexts(&mock);
    let switch = ContextProxySwitch::new();
    switch
        .switch(&browser, Some(&egress("http://one.example:8080")))
        .await
        .expect("the first switch succeeds");
    mock.fail("Target.createTarget", "no window");

    let error = switch
        .switch(&browser, Some(&egress("http://two.example:8080")))
        .await
        .expect_err("the tab cannot open");

    assert!(matches!(error, ScrapeError::Browser(_)), "{error:?}");
    let gone = mock.calls_to("Target.disposeBrowserContext");
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].params["browserContextId"], "C2");
    assert!(
        format!("{switch:?}").contains("C1"),
        "the old context is still held"
    );
}

#[tokio::test]
async fn a_refused_context_is_an_error_and_disposes_nothing() {
    let (browser, mock) = Browser::new_mocked();
    mock.fail("Target.createBrowserContext", "no contexts");

    let error = ContextProxySwitch::new()
        .switch(&browser, Some(&egress("http://one.example:8080")))
        .await
        .expect_err("the context is refused");

    assert!(matches!(error, ScrapeError::Browser(_)), "{error:?}");
    assert_eq!(mock.calls_to("Target.disposeBrowserContext").len(), 0);
}

#[tokio::test]
async fn an_old_context_that_is_already_gone_does_not_fail_the_switch() {
    let (browser, mock) = Browser::new_mocked();
    script_contexts(&mock);
    let switch = ContextProxySwitch::new();
    switch
        .switch(&browser, None)
        .await
        .expect("the first switch succeeds");
    mock.fail("Target.disposeBrowserContext", "no such context");

    let second = switch.switch(&browser, None).await;

    assert!(second.is_ok(), "{second:?}");
}
