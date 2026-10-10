//! Behavior of the Pure CDP API against a scripted browser, with no Chrome.
//!
//! These tests assert what a caller can observe: which protocol commands are
//! sent, in what order, and how browser answers become results and errors.
//! Run them with `cargo test --features test-util`.

#![cfg(feature = "test-util")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, Cookie, Key, Locator, MockCtrl, Page, SelectBy, State};
use seleniumbase_rs::stealth::evasions::bootstrap_script;
use seleniumbase_rs::{
    AssertionApi, BrowserApi, ElementApi, Fingerprint, OsType, ProxyConfig, ScreenshotApi,
    SeleniumBaseError,
};
use serde_json::{json, Value};

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

/// Answers the page helper calls a locator makes: a count, a centre point, and
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

#[tokio::test]
async fn a_click_moves_presses_and_releases_at_the_elements_centre() {
    let (page, mock) = quick_page().await;
    script_locator(&mock, 1, true);

    page.locator("#go").click().await.unwrap();

    let events: Vec<_> = mock
        .calls_to("Input.dispatchMouseEvent")
        .into_iter()
        .map(|call| {
            (
                call.params["type"].as_str().unwrap().to_owned(),
                call.params["x"].as_f64().unwrap(),
                call.params["y"].as_f64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        events,
        [
            ("mouseMoved".to_owned(), 120.0, 80.0),
            ("mousePressed".to_owned(), 120.0, 80.0),
            ("mouseReleased".to_owned(), 120.0, 80.0),
        ]
    );
}

#[tokio::test]
async fn a_missing_element_is_reported_as_not_found_with_its_locator() {
    let (page, mock) = quick_page().await;
    script_locator(&mock, 0, false);

    let error = page.locator("#nowhere").click().await.unwrap_err();

    assert!(
        matches!(&error, SeleniumBaseError::ElementNotFound { selector, .. } if selector == "#nowhere"),
        "unexpected error: {error}"
    );
    assert!(
        mock.calls_to("Input.dispatchMouseEvent").is_empty(),
        "nothing may be clicked when the element is missing"
    );
}

#[tokio::test]
async fn an_element_that_never_becomes_visible_is_not_interactable_rather_than_missing() {
    let (page, mock) = quick_page().await;
    script_locator(&mock, 1, false);

    let error = page.locator("#hidden").click().await.unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::ElementNotInteractable { .. }),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn waiting_succeeds_once_the_element_appears() {
    let (page, mock) = quick_page().await;
    let probes = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&probes);
    // The element shows up on the fourth look.
    mock.on("Runtime.evaluate", move |_| {
        Ok(value(json!(seen.fetch_add(1, Ordering::SeqCst) >= 3)))
    });

    page.with_timeout(Duration::from_secs(2))
        .locator("#late")
        .wait_for(State::Visible)
        .await
        .unwrap();

    assert!(
        probes.load(Ordering::SeqCst) >= 4,
        "it should have polled until the element appeared"
    );
}

#[tokio::test]
async fn waiting_times_out_with_a_message_naming_the_locator_and_state() {
    let (page, mock) = quick_page().await;
    script_locator(&mock, 0, false);

    let error = page
        .locator(".spinner")
        .wait_for(State::Visible)
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(
        matches!(error, SeleniumBaseError::WaitTimeout { .. }),
        "{message}"
    );
    assert!(
        message.contains(".spinner") && message.contains("visible"),
        "{message}"
    );
}

#[tokio::test]
async fn narrowing_a_locator_is_visible_to_the_page_as_a_resolution_chain() {
    let (page, mock) = quick_page().await;
    script_locator(&mock, 3, true);

    let third_visible = page.locator("ul > li").visible().nth(2);
    third_visible.count().await.unwrap();

    let sent = mock
        .calls_to("Runtime.evaluate")
        .pop()
        .and_then(|call| call.params["expression"].as_str().map(str::to_owned))
        .unwrap();
    assert!(sent.contains(r#""visible":true"#), "{sent}");
    assert!(sent.contains(r#""nth":2"#), "{sent}");
    assert_eq!(third_visible.to_string(), "ul > li [visible, nth=2]");
}

#[tokio::test]
async fn nesting_locators_searches_inside_the_parent() {
    let (page, mock) = quick_page().await;
    script_locator(&mock, 1, true);

    page.locator(".card")
        .locator("//button")
        .count()
        .await
        .unwrap();

    let sent = mock.calls_to("Runtime.evaluate").pop().unwrap().params["expression"]
        .as_str()
        .unwrap()
        .to_owned();
    let card = sent.find(".card").expect("parent step is sent");
    let button = sent.find("//button").expect("child step is sent");
    assert!(
        card < button,
        "the parent must resolve before the child: {sent}"
    );
    assert!(
        sent.contains(r#""kind":"xpath""#),
        "the child kind was decided in Rust: {sent}"
    );
}

#[tokio::test]
async fn fill_clears_then_inserts_in_one_step_and_type_text_sends_a_key_per_character() {
    let (page, mock) = quick_page().await;
    script_locator(&mock, 1, true);

    page.locator("#name").fill("héllo").await.unwrap();
    let inserted = mock.calls_to("Input.insertText");
    assert_eq!(inserted.len(), 1);
    assert_eq!(inserted[0].params["text"], "héllo");
    assert!(
        mock.calls_to("Input.dispatchKeyEvent").is_empty(),
        "fill sends no key events"
    );

    page.locator("#name").type_text("ab").await.unwrap();
    let downs: Vec<_> = mock
        .calls_to("Input.dispatchKeyEvent")
        .into_iter()
        .filter(|call| call.params["type"] == "keyDown")
        .map(|call| call.params["text"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(downs, ["a", "b"]);
}

#[tokio::test]
async fn pressing_enter_produces_a_character_so_forms_submit() {
    let (page, mock) = quick_page().await;
    script_locator(&mock, 1, true);

    page.locator("input").press(Key::Enter).await.unwrap();

    let down = &mock.calls_to("Input.dispatchKeyEvent")[0];
    assert_eq!(down.params["key"], "Enter");
    assert_eq!(down.params["text"], "\r");
}

#[tokio::test]
async fn an_unknown_dropdown_option_is_a_selector_error_naming_the_option() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("selectOption(") {
            exception("Error: sbcdp:no-option:text=Mars\n    at selectOption")
        } else if expression.contains("some(__sbcdp.visible)") || expression.contains(".length > 0")
        {
            value(json!(true))
        } else {
            value(Value::Null)
        })
    });

    let error = page
        .locator("select")
        .select_option(SelectBy::Text("Mars"))
        .await
        .unwrap_err();

    assert!(
        matches!(&error, SeleniumBaseError::InvalidSelector(m) if m.contains("text=Mars")),
        "{error}"
    );
}

#[tokio::test]
async fn goto_normalises_a_bare_host_and_waits_for_the_load_event() {
    let (page, mock) = quick_page().await;

    page.goto("seleniumbase.io").await.unwrap();

    let navigate = mock.calls_to("Page.navigate");
    assert_eq!(navigate.len(), 1);
    assert_eq!(navigate[0].params["url"], "https://seleniumbase.io");
}

#[tokio::test]
async fn a_failed_navigation_reports_the_url_and_the_browsers_reason() {
    let (page, mock) = quick_page().await;
    mock.reply(
        "Page.navigate",
        json!({ "errorText": "net::ERR_NAME_NOT_RESOLVED" }),
    );

    let error = page.goto("https://no-such-host.invalid").await.unwrap_err();

    assert!(
        matches!(&error, SeleniumBaseError::Navigation { url, reason }
            if url.contains("no-such-host") && reason.contains("ERR_NAME_NOT_RESOLVED")),
        "{error}"
    );
}

#[tokio::test]
async fn a_navigation_that_becomes_a_download_is_not_an_error() {
    let (page, mock) = quick_page().await;
    mock.reply("Page.navigate", json!({ "errorText": "net::ERR_ABORTED" }));

    page.goto("https://example.com/report.pdf").await.unwrap();
}

#[tokio::test]
async fn going_back_at_the_start_of_history_does_nothing() {
    let (page, mock) = quick_page().await;

    page.back().await.unwrap();

    assert!(mock.calls_to("Page.navigateToHistoryEntry").is_empty());
}

#[tokio::test]
async fn expectation_failures_say_what_was_expected_and_what_was_seen() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("textOf(") {
            value(json!("  Goodbye  "))
        } else {
            value(json!(true))
        })
    });

    let error = page
        .locator("h1")
        .expect()
        .to_have_text("Welcome")
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(
        matches!(error, SeleniumBaseError::AssertionFailed(_)),
        "{message}"
    );
    assert!(
        message.contains("Welcome") && message.contains("Goodbye"),
        "{message}"
    );
}

#[tokio::test]
async fn a_negated_expectation_passes_when_the_plain_one_would_fail() {
    let (page, mock) = quick_page().await;
    script_locator(&mock, 1, false);

    page.locator(".spinner")
        .expect()
        .not()
        .to_be_visible()
        .await
        .unwrap();
}

#[tokio::test]
async fn page_expectations_retry_until_the_title_changes() {
    let (page, mock) = quick_page().await;
    let page = page.with_timeout(Duration::from_secs(2));
    let looks = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&looks);
    mock.on("Runtime.evaluate", move |_| {
        Ok(value(json!(
            if counter.fetch_add(1, Ordering::SeqCst) < 2 {
                "Loading"
            } else {
                "Dashboard"
            }
        )))
    });

    page.expect().to_have_title("Dashboard").await.unwrap();
}

#[tokio::test]
async fn cookies_survive_a_save_and_load_and_their_values_stay_out_of_debug_output() {
    let (page, mock) = quick_page().await;
    mock.reply(
        "Network.getAllCookies",
        json!({ "cookies": [
            { "name": "session", "value": "s3cret", "domain": "example.com", "path": "/",
              "expires": -1, "httpOnly": true, "secure": true, "sameSite": "Lax" },
            { "name": "theme", "value": "dark", "domain": "other.org", "path": "/", "expires": 2_000_000_000.0 },
        ]}),
    );
    let file = tempfile::NamedTempFile::new().unwrap();

    let saved = page
        .cookies()
        .save(file.path(), Some("example"))
        .await
        .unwrap();
    assert_eq!(
        saved, 1,
        "the filter keeps only cookies whose domain, name or value match"
    );

    let loaded = page.cookies().load(file.path(), None).await.unwrap();
    assert_eq!(loaded, 1);
    let sent = &mock.calls_to("Network.setCookies")[0].params["cookies"][0];
    assert_eq!(sent["name"], "session");
    assert_eq!(sent["value"], "s3cret");
    assert_eq!(sent["httpOnly"], true);

    let cookie = Cookie::new("session", "s3cret");
    assert!(!format!("{cookie:?}").contains("s3cret"));
}

#[tokio::test]
async fn a_session_cookie_has_no_expiry_and_a_persistent_one_keeps_it() {
    let (page, mock) = quick_page().await;
    mock.reply(
        "Network.getAllCookies",
        json!({ "cookies": [
            { "name": "a", "value": "1", "domain": "x", "path": "/", "expires": -1 },
            { "name": "b", "value": "2", "domain": "x", "path": "/", "expires": 1_900_000_000.0 },
        ]}),
    );

    let cookies = page.cookies().all().await.unwrap();

    assert_eq!(cookies[0].expires, None);
    assert_eq!(cookies[1].expires, Some(1_900_000_000.0));
}

#[tokio::test]
async fn a_tab_sees_only_its_own_events() {
    let (page, mock) = quick_page().await;
    let mut events = page.events();

    mock.emit(
        "Page.frameNavigated",
        json!({ "from": "another tab" }),
        Some("S-other"),
    );
    mock.emit(
        "Page.frameNavigated",
        json!({ "from": "this tab" }),
        Some("S1"),
    );

    let event = events
        .wait_for("Page.frameNavigated", Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(event.params["from"], "this tab");
}

#[tokio::test]
async fn closing_a_tab_asks_the_browser_to_close_exactly_that_target() {
    let (page, mock) = quick_page().await;

    page.close().await.unwrap();

    let closes = mock.calls_to("Target.closeTarget");
    assert_eq!(closes.len(), 1);
    assert_eq!(closes[0].params["targetId"], page.id());
}

#[tokio::test]
async fn opening_a_page_attaches_to_the_new_target() {
    let (browser, mock) = Browser::new_mocked();

    let page = browser.new_page(Some("example.com")).await.unwrap();

    assert_eq!(page.id(), "T2");
    assert_eq!(
        mock.calls_to("Target.createTarget")[0].params["url"],
        "about:blank",
        "the tab is created blank so the navigation can be observed"
    );
    assert_eq!(
        mock.calls_to("Page.navigate")[0].params["url"],
        "https://example.com"
    );
}

/// Compiled but never called: it proves the main futures are `Send`, so they
/// can be spawned on a multi-threaded runtime.
#[expect(dead_code, reason = "exists only to be type-checked")]
fn futures_are_send(browser: &Browser, page: &Page, locator: &Locator) {
    fn send<T: Send>(_: T) {}
    send(browser.default_page());
    send(browser.close());
    send(page.goto("https://example.com"));
    send(page.evaluate("1"));
    send(page.cookies().all());
    send(page.expect().to_have_title("title"));
    send(locator.click());
    send(locator.fill("text"));
    send(locator.wait_for(State::Visible));
    send(locator.expect().to_be_visible());
}

// ----------------------------------------------------------------------
// CAPTCHA solving and cache-bypassing reload
// ----------------------------------------------------------------------

/// The page script that finds a widget answers with its kind and box.
fn script_captcha(mock: &MockCtrl, found: Value) {
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(
            if expression.contains("querySelectorAll(selectors[kind])") {
                value(found.clone())
            } else {
                value(Value::Null)
            },
        )
    });
}

#[tokio::test]
async fn solving_a_checkbox_captcha_clicks_near_its_left_edge_at_mid_height() {
    let (page, mock) = quick_page().await;
    // Turnstile is the first kind; a 300x65 widget at (100, 200).
    script_captcha(
        &mock,
        json!({ "kind": 0, "x": 100.0, "y": 200.0, "width": 300.0, "height": 65.0 }),
    );

    let solved = page.solve_captcha().await.unwrap();

    assert_eq!(
        solved.map(|kind| kind.to_string()),
        Some("Cloudflare Turnstile".to_owned())
    );
    let events: Vec<_> = mock
        .calls_to("Input.dispatchMouseEvent")
        .into_iter()
        .map(|call| {
            (
                call.params["type"].as_str().unwrap().to_owned(),
                call.params["x"].as_f64().unwrap(),
                call.params["y"].as_f64().unwrap(),
            )
        })
        .collect();
    assert!(
        events.contains(&("mousePressed".to_owned(), 128.0, 232.5)),
        "{events:?}"
    );
    assert!(
        events.contains(&("mouseReleased".to_owned(), 128.0, 232.5)),
        "{events:?}"
    );
}

#[tokio::test]
async fn solving_a_slider_captcha_drags_across_the_widget() {
    let (page, mock) = quick_page().await;
    // DataDome is the last kind.
    script_captcha(
        &mock,
        json!({ "kind": 4, "x": 0.0, "y": 100.0, "width": 400.0, "height": 60.0 }),
    );

    page.solve_captcha().await.unwrap();

    let xs: Vec<f64> = mock
        .calls_to("Input.dispatchMouseEvent")
        .into_iter()
        .map(|call| call.params["x"].as_f64().unwrap())
        .collect();
    assert_eq!(xs.first(), Some(&30.0), "starts at the handle");
    assert_eq!(xs.last(), Some(&370.0), "ends at the far side");
    assert!(
        xs.windows(2).all(|pair| pair[0] <= pair[1]),
        "the pointer only moves forward: {xs:?}"
    );
}

#[tokio::test]
async fn a_page_with_no_captcha_is_left_alone() {
    let (page, mock) = quick_page().await;
    script_captcha(&mock, Value::Null);

    assert!(page.solve_captcha().await.unwrap().is_none());
    assert!(mock.calls_to("Input.dispatchMouseEvent").is_empty());
}

#[tokio::test]
async fn reload_keeps_the_cache_and_hard_reload_bypasses_it() {
    let (page, mock) = quick_page().await;

    page.reload().await.unwrap();
    page.hard_reload().await.unwrap();

    let reloads: Vec<_> = mock
        .calls_to("Page.reload")
        .into_iter()
        .map(|call| call.params["ignoreCache"].clone())
        .collect();
    assert_eq!(reloads, [json!(false), json!(true)]);
}

// ----------------------------------------------------------------------
// Offsets, screen geometry and URL collection
// ----------------------------------------------------------------------

#[tokio::test]
async fn clicking_at_an_offset_measures_from_the_elements_top_left() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("__sbcdp.center(") {
            // A 100x40 element centred at (200, 100): top-left is (150, 80).
            value(json!({ "x": 200.0, "y": 100.0, "covered": false }))
        } else if expression.contains("__sbcdp.info(") {
            value(json!({
                "tag": "div", "text": "", "html": "", "attributes": {}, "visible": true,
                "rect": { "x": 150.0, "y": 80.0, "width": 100.0, "height": 40.0 },
            }))
        } else if expression.contains(".some(__sbcdp.visible)")
            || expression.contains(".length > 0")
        {
            value(json!(true))
        } else {
            value(Value::Null)
        })
    });

    page.locator("#target")
        .click_at(seleniumbase_rs::sb_cdp::Point { x: 10.0, y: 5.0 })
        .await
        .unwrap();

    let pressed: Vec<_> = mock
        .calls_to("Input.dispatchMouseEvent")
        .into_iter()
        .filter(|call| call.params["type"] == "mousePressed")
        .map(|call| {
            (
                call.params["x"].as_f64().unwrap(),
                call.params["y"].as_f64().unwrap(),
            )
        })
        .collect();
    assert_eq!(pressed, [(160.0, 85.0)]);
}

#[tokio::test]
async fn an_elements_urls_are_absolute_unique_and_in_page_order() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("location.href") {
            value(json!("https://example.com/dir/page"))
        } else if expression.contains(".map(__sbcdp.info)") && expression.contains("[href], [src]") {
            value(json!([
                { "tag": "a", "text": "", "html": "", "attributes": { "href": "/about" }, "rect": { "x": 0, "y": 0, "width": 1, "height": 1 }, "visible": true },
                { "tag": "img", "text": "", "html": "", "attributes": { "src": "logo.png" }, "rect": { "x": 0, "y": 0, "width": 1, "height": 1 }, "visible": true },
                { "tag": "a", "text": "", "html": "", "attributes": { "href": "/about" }, "rect": { "x": 0, "y": 0, "width": 1, "height": 1 }, "visible": true },
                { "tag": "a", "text": "", "html": "", "attributes": { "href": "#top" }, "rect": { "x": 0, "y": 0, "width": 1, "height": 1 }, "visible": true },
            ]))
        } else if expression.contains("__sbcdp.info(") {
            value(json!({ "tag": "body", "text": "", "html": "", "attributes": {}, "rect": { "x": 0, "y": 0, "width": 1, "height": 1 }, "visible": true }))
        } else if expression.contains(".length > 0") {
            value(json!(true))
        } else {
            value(Value::Null)
        })
    });

    let urls = page.locator("body").urls().await.unwrap();

    assert_eq!(
        urls,
        [
            "https://example.com/about",
            "https://example.com/dir/logo.png"
        ]
    );
}

#[tokio::test]
async fn an_elements_screen_rect_adds_the_window_origin_and_toolbar_and_removes_the_scroll() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("window.screenX") {
            // Window at (10, 20), 16px of side frame, 80px of toolbar, scrolled 300px.
            value(json!([10.0, 20.0, 16.0, 80.0, 0.0, 300.0]))
        } else if expression.contains("__sbcdp.info(") {
            value(json!({
                "tag": "div", "text": "", "html": "", "attributes": {}, "visible": true,
                "rect": { "x": 100.0, "y": 500.0, "width": 40.0, "height": 20.0 },
            }))
        } else if expression.contains(".length > 0") {
            value(json!(true))
        } else {
            value(Value::Null)
        })
    });

    let rect = page.locator("#target").screen_rect().await.unwrap();

    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (118.0, 300.0, 40.0, 20.0)
    );
}

// ----------------------------------------------------------------------
// WebRTC leak probing and shielding
// ----------------------------------------------------------------------

#[tokio::test]
async fn the_webrtc_probe_turns_candidate_lines_into_a_report() {
    let (page, mock) = quick_page().await;
    mock.reply(
        "Runtime.evaluate",
        value(json!([
            "candidate:1 1 udp 2113937151 9b1c4d2e-0000-0000-0000-000000000000.local 50000 typ host",
            "candidate:2 1 udp 1677729535 203.0.113.9 61000 typ srflx raddr 0.0.0.0 rport 0",
            "not a candidate",
        ])),
    );

    let report = page.webrtc_report().await.unwrap();

    assert_eq!(
        report.candidates().len(),
        2,
        "lines that are not candidates are dropped"
    );
    assert!(!report.is_clean());
    assert_eq!(report.leaks().len(), 2);
}

#[tokio::test]
async fn a_page_that_gathers_nothing_has_a_clean_report() {
    let (page, mock) = quick_page().await;
    mock.reply("Runtime.evaluate", value(json!([])));

    assert!(page.webrtc_report().await.unwrap().is_clean());
}

#[tokio::test]
async fn shielding_webrtc_installs_the_shim_now_and_for_every_new_document() {
    let (page, mock) = quick_page().await;

    page.shield_webrtc().await.unwrap();

    assert_eq!(
        shim_installs(&mock),
        1,
        "registered once for every new document"
    );
    let now: Vec<_> = mock
        .calls_to("Runtime.evaluate")
        .into_iter()
        .filter(|c| {
            c.params["expression"]
                .as_str()
                .is_some_and(|e| e.contains("iceTransportPolicy"))
        })
        .collect();
    assert!(!now.is_empty(), "the current document is shielded too");
}

#[tokio::test]
async fn a_page_is_not_shielded_unless_asked() {
    let (_page, mock) = quick_page().await;

    assert_eq!(shim_installs(&mock), 0);
}

/// How many scripts registered for new documents carry the WebRTC shim. The
/// page helper is registered the same way, so counting every registration
/// would be wrong.
fn shim_installs(mock: &MockCtrl) -> usize {
    mock.calls_to("Page.addScriptToEvaluateOnNewDocument")
        .into_iter()
        .filter(|call| {
            call.params["source"]
                .as_str()
                .is_some_and(|source| source.contains("iceTransportPolicy"))
        })
        .count()
}

// ----------------------------------------------------------------------
// Making a tab match a fingerprint
// ----------------------------------------------------------------------

/// The commands sent to the tab after `skip` earlier calls, in order.
fn tab_calls_after(mock: &MockCtrl, skip: usize) -> Vec<(String, Value)> {
    mock.calls()
        .into_iter()
        .skip(skip)
        .filter(|call| call.session_id.is_some())
        .map(|call| (call.method, call.params))
        .collect()
}

#[tokio::test]
async fn a_fingerprint_is_applied_script_first_then_overrides() {
    let (page, mock) = quick_page().await;
    let fingerprint = Fingerprint::randomized(OsType::Windows, 7);
    let before = mock.calls().len();

    page.apply_fingerprint(&fingerprint).await.unwrap();

    let calls = tab_calls_after(&mock, before);
    assert_eq!(calls[0].0, "Page.addScriptToEvaluateOnNewDocument");
    assert_eq!(
        calls[0].1["source"].as_str().unwrap(),
        bootstrap_script(&fingerprint),
        "the library's own script for this fingerprint"
    );
    assert_eq!(calls[1].0, "Network.enable");
    let user_agent = calls
        .iter()
        .find(|(method, _)| method == "Network.setUserAgentOverride")
        .expect("a user agent override");
    assert_eq!(
        user_agent.1["userAgent"].as_str(),
        fingerprint.user_agent.as_deref()
    );
    for wanted in [
        "Emulation.setTimezoneOverride",
        "Emulation.setLocaleOverride",
    ] {
        assert!(calls.iter().any(|(method, _)| method == wanted), "{wanted}");
    }
}

#[tokio::test]
async fn the_proxy_password_in_a_fingerprint_is_never_sent_to_the_browser() {
    let (page, mock) = quick_page().await;
    let mut fingerprint = Fingerprint::randomized(OsType::Linux, 3);
    fingerprint.proxy = Some(ProxyConfig {
        r#type: "http".to_owned(),
        host: "proxy.example.com".to_owned(),
        port: 8080,
        username: Some("alice".to_owned()),
        password: Some("fake-proxy-password".to_owned()),
        save_traffic: false,
    });

    page.apply_fingerprint(&fingerprint).await.unwrap();

    assert!(mock.calls_to("Network.setExtraHTTPHeaders").is_empty());
    let everything = format!("{:?}", mock.calls());
    assert!(
        !everything.contains("fake-proxy-password"),
        "the password reached the wire"
    );
    assert!(!everything.contains("Proxy-Authorization"));
}

#[tokio::test]
async fn permissions_are_granted_in_the_tabs_own_browser_context() {
    let (page, mock) = quick_page().await;
    mock.reply(
        "Target.getTargetInfo",
        json!({ "targetInfo": { "browserContextId": "CTX9" } }),
    );
    let mut fingerprint = Fingerprint::randomized(OsType::Windows, 5);
    fingerprint.flags.grant_permissions = true;

    page.apply_fingerprint(&fingerprint).await.unwrap();

    let grants = mock.calls_to("Browser.grantPermissions");
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].params["browserContextId"], "CTX9");
    assert_eq!(grants[0].session_id, None, "a browser-wide command");
}

#[tokio::test]
async fn a_refused_override_is_named_in_the_error() {
    let (page, mock) = quick_page().await;
    mock.fail("Emulation.setTimezoneOverride", "invalid timezone");
    let fingerprint = Fingerprint::randomized(OsType::Macos, 11);

    let error = page.apply_fingerprint(&fingerprint).await.unwrap_err();

    let shown = error.to_string();
    assert!(shown.contains("Emulation.setTimezoneOverride"), "{shown}");
    assert!(shown.contains("invalid timezone"), "{shown}");
}

// ----------------------------------------------------------------------
// Script errors
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_page_without_script_errors_passes_the_assertion() {
    let (page, mock) = quick_page().await;
    mock.reply("Runtime.evaluate", value(json!([])));

    assert_eq!(page.js_errors().await.unwrap(), Vec::<String>::new());
    page.assert_no_js_errors().await.unwrap();
}

#[tokio::test]
async fn the_first_script_error_is_named_and_the_rest_counted() {
    let (page, mock) = quick_page().await;
    mock.reply(
        "Runtime.evaluate",
        value(json!([
            "boom is not defined",
            "Unhandled promise rejection: nope"
        ])),
    );

    let error = page.assert_no_js_errors().await.unwrap_err();

    assert!(
        matches!(error, SeleniumBaseError::AssertionFailed(_)),
        "{error}"
    );
    assert!(
        error
            .to_string()
            .contains("JS error detected: boom is not defined (and 1 more)"),
        "{error}"
    );
}

// ----------------------------------------------------------------------
// The capability traits, on a CDP page
// ----------------------------------------------------------------------

/// A helper written once over the traits, so it must work on any engine.
async fn sign_in<E>(sb: &mut E) -> Result<(), SeleniumBaseError>
where
    E: BrowserApi + ElementApi + AssertionApi,
{
    sb.open("https://example.com/login").await?;
    sb.type_text("#user", "al").await?;
    sb.click("#submit").await?;
    sb.assert_title("Dashboard").await
}

fn script_title_and_locator(mock: &MockCtrl, title: &'static str) {
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("document.title") {
            value(json!(title))
        } else if expression.contains("__sbcdp.center(") {
            value(json!({ "x": 120.0, "y": 80.0, "covered": false }))
        } else if expression.contains(".some(__sbcdp.visible)") {
            value(json!(true))
        } else if expression.contains("resolve(") && expression.contains(".length") {
            value(json!(1))
        } else {
            value(Value::Null)
        })
    });
}

#[tokio::test]
async fn a_helper_written_over_the_traits_runs_on_a_cdp_page() {
    let (mut page, mock) = quick_page().await;
    script_title_and_locator(&mock, "Dashboard");

    sign_in(&mut page).await.unwrap();

    let navigations = mock.calls_to("Page.navigate");
    assert_eq!(navigations[0].params["url"], "https://example.com/login");
    assert!(
        mock.calls_to("Input.dispatchKeyEvent").len() >= 2,
        "the text was typed key by key"
    );
    assert!(
        mock.calls_to("Input.dispatchMouseEvent")
            .iter()
            .any(|c| c.params["type"] == "mousePressed"),
        "the button was clicked"
    );
}

#[tokio::test]
async fn the_same_helper_fails_when_the_title_is_wrong() {
    let (mut page, mock) = quick_page().await;
    script_title_and_locator(&mock, "Login");

    let error = sign_in(&mut page).await.unwrap_err();

    assert!(
        error.to_string().contains("Dashboard"),
        "names what was expected: {error}"
    );
}

#[tokio::test]
async fn quitting_through_the_trait_closes_the_tab() {
    let (mut page, mock) = quick_page().await;

    BrowserApi::quit(&mut page).await.unwrap();

    assert_eq!(mock.calls_to("Target.closeTarget").len(), 1);
}

#[tokio::test]
async fn a_screenshot_through_the_trait_is_the_browsers_png() {
    use base64::Engine;
    let (page, mock) = quick_page().await;
    let png = b"\x89PNG-test-bytes";
    mock.reply(
        "Page.captureScreenshot",
        json!({ "data": base64::engine::general_purpose::STANDARD.encode(png) }),
    );

    assert_eq!(ScreenshotApi::screenshot_as_png(&page).await.unwrap(), png);
}

#[tokio::test]
async fn a_screenshot_name_that_could_leave_the_logs_directory_is_refused() {
    let (page, mock) = quick_page().await;
    mock.reply("Page.captureScreenshot", json!({ "data": "" }));

    for bad in ["../shot.png", "/tmp/shot.png", "a/b.png", ".."] {
        let error = ScreenshotApi::save_screenshot(&page, bad)
            .await
            .unwrap_err();
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{bad}: {error}"
        );
    }
    assert!(
        mock.calls_to("Page.captureScreenshot").is_empty(),
        "nothing was captured for a refused name"
    );
}

#[tokio::test]
async fn every_tab_is_told_it_has_focus_so_input_reaches_it() {
    let (browser, mock) = Browser::new_mocked();
    let _first = browser.default_page().await.unwrap();
    let _second = browser
        .new_page(Some("about:blank".to_owned()))
        .await
        .unwrap();

    // Chrome only acknowledges input for the focused tab, so a click on any
    // other tab would wait for the command timeout.
    let focus = mock.calls_to("Emulation.setFocusEmulationEnabled");
    assert_eq!(focus.len(), 2, "one per tab: {focus:?}");
    assert!(focus
        .iter()
        .all(|call| call.params == json!({ "enabled": true })));
}
