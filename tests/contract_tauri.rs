//! The contract with `tauri-plugin-sbrs-automation`, the Tauri plugin whose feature `stealth`
//! gives an app's own webviews this crate's stealth (`src/stealth.rs` in that repository).
//!
//! The plugin depends on this crate by git and pins a commit, so a change here reaches it only
//! when it moves the pin. This file is what keeps that move a small one: it is the plugin's
//! import list, verbatim, and the calls the plugin makes, written as it makes them. If a change
//! to this crate stops this file compiling, the plugin stops compiling against it too: fix the
//! contract in the same commit, and tell the plugin's owner what to change before the pin moves.
//!
//! `scripts/check-consumers.sh tauri` (`just contracts`) reads the plugin's checkout and fails
//! when it imports a path this file does not list. No feature is needed: the plugin takes this
//! crate with `default-features = false`.

#![allow(dead_code, unused_imports)]

use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

// The plugin's imports (src/stealth.rs, lines 43-46), verbatim.
pub use seleniumbase_rs::sb_cdp::{
    Browser, CdpInbox, CdpRequest, CdpTransport, Page, StealthPlan, TransportError,
};
pub use seleniumbase_rs::{Fingerprint, OsType, SeleniumBaseError};

fn is_debug_and_clone<T: Debug + Clone>() {}

fn is_send_and_sync<T: Send + Sync + 'static>() {}

/// `DevToolsChannel` derives `Debug` and `Clone` over a `Browser` and a `CdpInbox`, hands them to
/// callbacks on other threads, and clones the `StealthPlan` into a task.
#[test]
fn the_types_the_plugin_derives_and_shares_over() {
    is_debug_and_clone::<Browser>();
    is_debug_and_clone::<CdpInbox>();
    is_debug_and_clone::<StealthPlan>();
    is_debug_and_clone::<CdpRequest>();
    is_send_and_sync::<Browser>();
    is_send_and_sync::<CdpInbox>();
    is_send_and_sync::<Page>();
}

/// `DevToolsChannel::new(transport: impl CdpTransport)` is called with a closure over a
/// `CdpRequest`, and `cef::channel` and `webview2::channel` read every field of the request.
#[test]
fn a_closure_over_a_request_is_a_transport() {
    fn takes_a_transport(_transport: impl CdpTransport) {}

    takes_a_transport(move |request: CdpRequest| -> Result<(), TransportError> {
        // `cef::channel`.
        let native: i32 = i32::try_from(request.id)?;
        let mut message = json!({
            "id": native,
            "method": request.method,
            "params": request.params,
        });
        if let Some(session) = request.session_id {
            message["sessionId"] = Value::String(session);
        }
        let _text: String = message.to_string();
        Ok(())
    });
    takes_a_transport(move |request: CdpRequest| -> Result<(), TransportError> {
        // `webview2::channel`.
        let inbox: Option<CdpInbox> = None;
        let _inbox = inbox.as_ref().ok_or("the channel is not ready")?;
        let _method: String = request.method;
        let _params_json: String = request.params.to_string();
        let _session_id: Option<String> = request.session_id;
        let _id: u64 = request.id;
        Err("the browser is gone".into())
    });
    // A `TransportError` is made from what the plugin's callbacks fail with.
    takes_a_transport(|_: CdpRequest| -> Result<(), TransportError> {
        let _parsed: Value = serde_json::from_slice(b"{}")?;
        let _narrowed = i32::try_from(7_usize)?;
        Ok(())
    });
}

/// The calls of `DevToolsChannel` and the two bridges, in the types the plugin passes.
async fn the_calls_the_plugin_makes(
    browser: &Browser,
    inbox: &CdpInbox,
    plan: &StealthPlan,
) -> Result<(), SeleniumBaseError> {
    // `DevToolsChannel::new`.
    let (_browser, _inbox): (Browser, CdpInbox) =
        Browser::over_channel(|_: CdpRequest| -> Result<(), TransportError> { Ok(()) });

    // `DevToolsChannel::apply`: the overrides, in order, each after the answer to the one before.
    for (method, params) in plan.overrides() {
        let _: &str = method;
        let _: Value = browser.execute(method, params.clone()).await?;
    }
    // `DevToolsChannel::page` and the tests that drive it.
    let page: Page = browser.default_page().await?;
    page.goto("https://example.com/").await?;
    let _raw = browser
        .execute(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "Asia/Tokyo" }),
        )
        .await?;

    // `cef::Observer` and `webview2::{Reply, event}`.
    inbox.respond(1_u64, Ok(json!({})));
    inbox.respond(2_u64, Err("Invalid timezone ID".to_owned()));
    inbox.event("Page.loadEventFired", json!({ "timestamp": 1 }), None);
    inbox.event("Network.loadingFinished", json!({}), Some("FRAME"));
    inbox.close();

    // The tests of the plugin.
    let _connected: bool = browser.is_connected();
    let mut events = browser.events();
    let first = events.next().await.expect("an event");
    let _: &str = first.method.as_str();
    let _: &Value = &first.params;
    let _: Option<&str> = first.session_id.as_deref();
    Ok(())
}

/// `StealthScriptExt::stealth_script` and the plugin's doc example.
#[test]
fn a_plan_has_a_script_and_overrides() {
    let plan = StealthPlan::new(&Fingerprint::randomized(OsType::Windows, 7));
    let script: &str = plan.init_script();
    assert!(!script.is_empty(), "the init script is what a webview runs");
    let overrides: Vec<(&str, &Value)> = plan.overrides().collect();
    assert_eq!(overrides[0].0, "Network.enable");
    assert!(
        overrides
            .iter()
            .all(|(method, _)| *method != "Page.addScriptToEvaluateOnNewDocument"),
        "the script goes in as the webview's initialization script, not as an override"
    );
    assert!(
        overrides.iter().any(|(method, params)| {
            *method == "Network.setUserAgentOverride" && params.get("userAgentMetadata").is_some()
        }),
        "the user agent comes with its Client Hints"
    );
}

/// What the plugin's fake CEF and WebView2 tests rely on: a command leaves through the transport
/// with the library's id, and the answer put into the inbox completes it.
#[tokio::test]
async fn a_command_leaves_through_the_transport_and_is_answered_through_the_inbox() {
    let sent: Arc<Mutex<Vec<CdpRequest>>> = Arc::default();
    let log = Arc::clone(&sent);
    let (browser, inbox) =
        Browser::over_channel(move |request: CdpRequest| -> Result<(), TransportError> {
            log.lock().unwrap().push(request);
            Ok(())
        });
    let call = {
        let browser = browser.clone();
        tokio::spawn(async move {
            browser
                .execute(
                    "Emulation.setTimezoneOverride",
                    json!({ "timezoneId": "Asia/Tokyo" }),
                )
                .await
        })
    };
    let request = loop {
        if let Some(request) = sent.lock().unwrap().first().cloned() {
            break request;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    };
    assert_eq!(request.method, "Emulation.setTimezoneOverride");
    assert_eq!(request.params, json!({ "timezoneId": "Asia/Tokyo" }));
    assert_eq!(
        request.session_id, None,
        "no sessionId: the channel reaches the webview's page"
    );

    inbox.respond(request.id, Ok(json!({})));
    assert_eq!(call.await.unwrap().unwrap(), json!({}));
}

/// The plugin's tests expect an error that names the command and carries the browser's message.
#[tokio::test]
async fn a_refused_command_carries_its_name_and_the_browsers_message() {
    let sent: Arc<Mutex<Vec<CdpRequest>>> = Arc::default();
    let log = Arc::clone(&sent);
    let (browser, inbox) =
        Browser::over_channel(move |request: CdpRequest| -> Result<(), TransportError> {
            log.lock().unwrap().push(request);
            Ok(())
        });
    let call = {
        let browser = browser.clone();
        tokio::spawn(async move {
            browser
                .execute(
                    "Emulation.setTimezoneOverride",
                    json!({ "timezoneId": "Mars/Base" }),
                )
                .await
        })
    };
    let request = loop {
        if let Some(request) = sent.lock().unwrap().first().cloned() {
            break request;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    };

    inbox.respond(request.id, Err("Invalid timezone ID".to_owned()));

    let error = call.await.unwrap().unwrap_err().to_string();
    assert!(error.contains("Emulation.setTimezoneOverride"), "{error}");
    assert!(error.contains("Invalid timezone ID"), "{error}");
}

/// Closing the inbox fails what waits, at once, with `BrowserDisconnected`, and the browser knows.
#[tokio::test]
async fn a_closed_inbox_fails_what_waits_and_disconnects_the_browser() {
    let sent: Arc<Mutex<Vec<CdpRequest>>> = Arc::default();
    let log = Arc::clone(&sent);
    let (browser, inbox) =
        Browser::over_channel(move |request: CdpRequest| -> Result<(), TransportError> {
            log.lock().unwrap().push(request);
            Ok(())
        });
    let call = {
        let browser = browser.clone();
        tokio::spawn(async move { browser.execute("Page.enable", json!({})).await })
    };
    while sent.lock().unwrap().is_empty() {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    inbox.close();

    let outcome = tokio::time::timeout(Duration::from_secs(2), call)
        .await
        .expect("closing does not wait for the timeout")
        .unwrap();
    assert!(matches!(
        outcome,
        Err(SeleniumBaseError::BrowserDisconnected { .. })
    ));
    assert!(!browser.is_connected());
}

/// The events the plugin passes to the inbox reach the browser's listeners as the page's, and an
/// event for an attached target keeps its session.
#[tokio::test]
async fn an_event_put_into_the_inbox_reaches_the_listeners() {
    let (browser, inbox) =
        Browser::over_channel(|_: CdpRequest| -> Result<(), TransportError> { Ok(()) });
    let mut events = browser.events();

    inbox.event("Page.loadEventFired", json!({ "timestamp": 2 }), None);
    inbox.event("Network.loadingFinished", json!({}), Some("FRAME"));

    let first = events.next().await.unwrap();
    assert_eq!(first.method, "Page.loadEventFired");
    assert_eq!(first.params["timestamp"], 2);
    let second = events.next().await.unwrap();
    assert_eq!(second.session_id.as_deref(), Some("FRAME"));
}
