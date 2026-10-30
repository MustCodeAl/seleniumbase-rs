//! A `Locator` entering frames, against a scripted browser with no Chrome.
//!
//! The page helper cannot read into a frame of another origin, a `data:`
//! frame, a sandboxed one or a frame of another site. It says so by throwing
//! `sbcdp:frames`, and the locator enters the frame through the protocol:
//! `DOM.describeNode` names the frame, `Page.createIsolatedWorld` gives a
//! frame of the tab's own process a context to run in, and a frame of another
//! process is attached to as a target of its own. These tests assert the
//! sequence of commands and where each one goes; the real Chrome tests are in
//! `tests/sb_cdp_frames_chrome.rs`.
//!
//! Run them with `cargo test --features test-util --test sb_cdp_frames`.

#![cfg(feature = "test-util")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, Call, MockCtrl, Page};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};

/// The remote object `Runtime.evaluate` returns for the frame element.
const FRAME_ELEMENT: &str = "frame-element";

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

/// A script exception, as the page helpers throw them.
fn exception(description: &str) -> Value {
    json!({ "exceptionDetails": { "exception": { "description": description } } })
}

/// A page on a mocked browser, with a short default wait so the failure-path
/// tests finish quickly.
async fn quick_page() -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    let page = browser.default_page().await.unwrap();
    (page.with_timeout(Duration::from_millis(150)), mock)
}

/// Scripts a page with a frame `iframe#f` the page script cannot read into,
/// holding `present` matches of `#b`. The frame is in the tab's own process, or,
/// with `remote`, in another one: the tab refuses to make a context in it.
///
/// A chain that names `iframe#f` is the page's own, and asks for the frame to
/// be entered; any other is the frame's, and is answered from `present`.
fn script_frame(mock: &MockCtrl, present: usize, remote: bool) {
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        if expression.contains("__sbcdp.blockedSteps(") {
            return Ok(value(json!([1])));
        }
        if expression.contains("__sbcdp.blockedFrame(") {
            return Ok(json!({ "result": {
                "type": "object", "subtype": "node", "objectId": FRAME_ELEMENT,
            }}));
        }
        if expression.contains("iframe#f") {
            return Ok(exception("Error: sbcdp:frames\n    at resolve"));
        }
        Ok(if expression.contains("__sbcdp.center(") {
            value(json!({ "x": 10.0, "y": 20.0, "covered": false }))
        } else if expression.contains(".some(__sbcdp.visible)") {
            value(json!(present > 0))
        } else if expression.contains(".length === 0") {
            value(json!(present == 0))
        } else if expression.contains(".length > 0") {
            value(json!(present > 0))
        } else if expression.contains(".length") {
            value(json!(present))
        } else if present == 0 && expression.contains("__sbcdp.one(") {
            exception("Error: sbcdp:not-found")
        } else {
            value(Value::Null)
        })
    });
    mock.reply("DOM.describeNode", json!({ "node": { "frameId": "F2" } }));
    if remote {
        mock.fail("Page.createIsolatedWorld", "No frame for given id found");
    } else {
        mock.reply(
            "Page.createIsolatedWorld",
            json!({ "executionContextId": 7 }),
        );
    }
    // Where the frame's content starts in the page: 30 right, 100 down.
    mock.on("Runtime.callFunctionOn", |params| {
        let function = params["functionDeclaration"].as_str().unwrap_or_default();
        Ok(if function.contains("frameOrigin") {
            value(json!({ "x": 30.0, "y": 100.0 }))
        } else {
            json!({})
        })
    });
}

/// The index of the first received `method` command that `matches`.
fn position(calls: &[Call], method: &str, matches: impl Fn(&Call) -> bool) -> Option<usize> {
    calls
        .iter()
        .position(|call| call.method == method && matches(call))
}

/// Whether `call` asks a frame for the centre of the element. The page's own
/// chain names `iframe#f`, and its answer is to ask for the frame to be entered.
fn is_frame_center(call: &Call) -> bool {
    call.params["expression"]
        .as_str()
        .is_some_and(|expression| {
            expression.contains("__sbcdp.center(") && !expression.contains("iframe#f")
        })
}

/// The mouse events received, as (type, x, y, session).
fn mouse_events(mock: &MockCtrl) -> Vec<(String, f64, f64, Option<String>)> {
    mock.calls_to("Input.dispatchMouseEvent")
        .into_iter()
        .map(|call| {
            (
                call.params["type"].as_str().unwrap_or_default().to_owned(),
                call.params["x"].as_f64().unwrap_or_default(),
                call.params["y"].as_f64().unwrap_or_default(),
                call.session_id,
            )
        })
        .collect()
}

#[tokio::test]
async fn a_chain_the_page_can_search_sends_no_frame_command() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |_| Ok(value(json!(1))));
    let before = mock.calls_to("Runtime.evaluate").len();

    assert_eq!(page.locator("#a").locator("#b").count().await.unwrap(), 1);

    assert_eq!(mock.calls_to("Runtime.evaluate").len() - before, 1);
    for method in [
        "DOM.describeNode",
        "Page.createIsolatedWorld",
        "Runtime.callFunctionOn",
        "Runtime.releaseObject",
    ] {
        assert!(mock.calls_to(method).is_empty(), "{method} was sent");
    }
}

#[tokio::test]
async fn a_frame_of_the_tabs_process_is_entered_in_an_isolated_world() {
    let (page, mock) = quick_page().await;
    script_frame(&mock, 1, false);

    page.locator("iframe#f")
        .locator("#b")
        .click()
        .await
        .unwrap();

    let calls = mock.calls();
    let describe = position(&calls, "DOM.describeNode", |_| true).expect("the frame is described");
    assert_eq!(
        calls[describe].params,
        json!({ "objectId": FRAME_ELEMENT }),
        "the frame element is what is described"
    );
    assert_eq!(calls[describe].session_id.as_deref(), Some("S1"));
    let world = position(&calls, "Page.createIsolatedWorld", |_| true).expect("a world is made");
    assert!(describe < world);
    assert_eq!(
        calls[world].params,
        json!({ "frameId": "F2", "worldName": "sbcdp" })
    );
    assert_eq!(calls[world].session_id.as_deref(), Some("S1"));

    // The script that finds the element runs in that world, with the rest of
    // the chain only: the page's own selector for the frame is not in it.
    let center =
        position(&calls, "Runtime.evaluate", is_frame_center).expect("the centre is asked for");
    assert_eq!(calls[center].params["contextId"], json!(7));
    assert_eq!(calls[center].session_id.as_deref(), Some("S1"));
    let expression = calls[center].params["expression"].as_str().unwrap();
    assert!(!expression.contains("iframe#f"), "{expression}");
    assert!(expression.contains("#b"), "{expression}");

    // The tab has no target to attach to for it.
    assert_eq!(
        mock.calls_to("Target.attachToTarget").len(),
        1,
        "only the tab is attached to"
    );
}

#[tokio::test]
async fn a_click_in_a_frame_lands_where_the_frame_starts_plus_the_point_in_it() {
    let (page, mock) = quick_page().await;
    script_frame(&mock, 1, false);

    page.locator("iframe#f")
        .locator("#b")
        .click()
        .await
        .unwrap();

    // The point in the frame is (10, 20) and the frame's content starts at
    // (30, 100); the mouse is the tab's, so its events go to the tab.
    let events = mouse_events(&mock);
    let at = |kind: &str| (kind.to_owned(), 40.0, 120.0, Some("S1".to_owned()));
    assert_eq!(
        events,
        [at("mouseMoved"), at("mousePressed"), at("mouseReleased")]
    );

    // The frame is scrolled into view, and given a moment to be drawn, before
    // the point is taken and clicked.
    let calls = mock.calls();
    let reveal = position(&calls, "Runtime.callFunctionOn", |call| {
        call.params["functionDeclaration"]
            .as_str()
            .is_some_and(|function| function.contains("scrollIntoView"))
    })
    .expect("the frame is scrolled into view");
    assert_eq!(calls[reveal].params["objectId"], json!(FRAME_ELEMENT));
    assert_eq!(calls[reveal].params["awaitPromise"], json!(true));
    let center = position(&calls, "Runtime.evaluate", is_frame_center).unwrap();
    let press = position(&calls, "Input.dispatchMouseEvent", |_| true).unwrap();
    assert!(reveal < center && center < press);
}

#[tokio::test]
async fn the_handles_of_the_frame_elements_are_let_go_after_use() {
    let (page, mock) = quick_page().await;
    script_frame(&mock, 1, false);

    page.locator("iframe#f")
        .locator("#b")
        .click()
        .await
        .unwrap();

    let described = mock.calls_to("DOM.describeNode").len();
    let released = mock.calls_to("Runtime.releaseObject");
    assert_eq!(released.len(), described, "every handle is released");
    assert!(released
        .iter()
        .all(|call| call.params["objectId"] == json!(FRAME_ELEMENT)));
}

#[tokio::test]
async fn a_frame_of_another_process_is_attached_to_and_scripts_go_to_its_session() {
    let (page, mock) = quick_page().await;
    script_frame(&mock, 1, true);
    let attaches = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&attaches);
    // The browser has no host for the frame until the targets are listed.
    mock.on("Target.attachToTarget", move |params| {
        if params["targetId"] != "F2" {
            return Ok(json!({ "sessionId": "S1" }));
        }
        if seen.fetch_add(1, Ordering::SeqCst) == 0 {
            Err("No target with given id found".to_owned())
        } else {
            Ok(json!({ "sessionId": "S2" }))
        }
    });

    page.locator("iframe#f")
        .locator("#b")
        .click()
        .await
        .unwrap();

    let calls = mock.calls();
    let refused = position(&calls, "Target.attachToTarget", |call| {
        call.params["targetId"] == "F2"
    })
    .expect("the frame is attached to");
    let listed = refused
        + calls[refused..]
            .iter()
            .position(|call| call.method == "Target.getTargets")
            .expect("the targets are listed");
    let attached = calls
        .iter()
        .rposition(|call| call.method == "Target.attachToTarget" && call.params["targetId"] == "F2")
        .unwrap();
    assert!(
        refused < listed && listed < attached,
        "{refused} {listed} {attached}"
    );
    assert_eq!(
        calls[attached].params,
        json!({ "targetId": "F2", "flatten": true })
    );
    assert_eq!(calls[attached].session_id, None, "the browser attaches");

    // The frame's own scripts are sent to its session, in its main world; the
    // tab's session gets no script for the frame, only the one that asked.
    let framed: Vec<&Call> = calls
        .iter()
        .filter(|call| call.session_id.as_deref() == Some("S2"))
        .collect();
    assert!(!framed.is_empty());
    assert!(framed.iter().all(|call| call.method == "Runtime.evaluate"));
    assert!(framed
        .iter()
        .all(|call| call.params.get("contextId").is_none()));
    assert!(framed.iter().any(|call| call.params["expression"]
        .as_str()
        .is_some_and(|expression| expression.contains("__sbcdp.center("))));

    // The mouse is still the tab's.
    assert!(mouse_events(&mock)
        .iter()
        .all(|(_, x, y, session)| (*x, *y) == (40.0, 120.0) && session.as_deref() == Some("S1")));
}

#[tokio::test]
async fn the_session_of_a_frame_of_another_process_is_attached_once() {
    let (page, mock) = quick_page().await;
    script_frame(&mock, 1, true);
    mock.on("Target.attachToTarget", |params| {
        Ok(json!({ "sessionId": if params["targetId"] == "F2" { "S2" } else { "S1" } }))
    });

    let button = page.locator("iframe#f").locator("#b");
    button.click().await.unwrap();
    button.click().await.unwrap();
    assert!(button.is_visible().await.unwrap());

    let to_frame = mock
        .calls_to("Target.attachToTarget")
        .into_iter()
        .filter(|call| call.params["targetId"] == "F2")
        .count();
    assert_eq!(to_frame, 1, "one session serves every later use");
}

#[tokio::test]
async fn a_frame_that_holds_no_match_gives_not_found_and_clicks_nothing() {
    let (page, mock) = quick_page().await;
    script_frame(&mock, 0, false);
    let button = page.locator("iframe#f").locator("#b");

    assert!(!button.exists().await.unwrap());
    assert_eq!(button.count().await.unwrap(), 0);
    assert!(!button.is_visible().await.unwrap());
    assert!(button.infos().await.unwrap().is_empty());
    let error = button.click().await.unwrap_err();
    assert!(
        matches!(&error, SeleniumBaseError::ElementNotFound { .. }),
        "unexpected error: {error}"
    );
    assert!(mouse_events(&mock).is_empty(), "nothing may be clicked");
}

#[tokio::test]
async fn a_frame_that_cannot_be_entered_is_a_frame_without_matches() {
    // The frame element has no frame behind it (it is not in the page any
    // longer): `DOM.describeNode` names none.
    let (page, mock) = quick_page().await;
    script_frame(&mock, 1, false);
    mock.reply("DOM.describeNode", json!({ "node": {} }));

    assert!(!page
        .locator("iframe#f")
        .locator("#b")
        .exists()
        .await
        .unwrap());
}

#[tokio::test]
async fn a_frame_that_goes_away_during_the_click_is_looked_for_again() {
    let (page, mock) = quick_page().await;
    script_frame(&mock, 1, false);
    // The first context is gone by the time the centre is asked for.
    let tries = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&tries);

    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        let in_frame = !expression.contains("iframe#f");
        if in_frame
            && expression.contains("__sbcdp.center(")
            && seen.fetch_add(1, Ordering::SeqCst) == 0
        {
            return Ok(exception("Error: Cannot find context with specified id"));
        }
        if expression.contains("__sbcdp.blockedSteps(") {
            return Ok(value(json!([1])));
        }
        if expression.contains("__sbcdp.blockedFrame(") {
            return Ok(json!({ "result": { "objectId": FRAME_ELEMENT } }));
        }
        if expression.contains("iframe#f") {
            return Ok(exception("Error: sbcdp:frames"));
        }
        Ok(if expression.contains("__sbcdp.center(") {
            value(json!({ "x": 10.0, "y": 20.0 }))
        } else if expression.contains(".some(__sbcdp.visible)") {
            value(json!(true))
        } else {
            value(json!(1))
        })
    });

    page.locator("iframe#f")
        .locator("#b")
        .click()
        .await
        .unwrap();

    assert_eq!(mouse_events(&mock).len(), 3, "the click happened once");
    assert!(
        mock.calls_to("Page.createIsolatedWorld").len() >= 3,
        "the frame was entered again after it went away"
    );
}
