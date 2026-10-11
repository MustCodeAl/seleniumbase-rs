//! Pure CDP behaviour that the parity audit of `parity/api.toml` found missing
//! or different from Python SeleniumBase, checked against a scripted browser.
//!
//! Each test names the `sb.cdp` method it covers. No Chrome is needed; run them
//! with `cargo test --features test-util --test sb_cdp_parity_mock`.

#![cfg(feature = "test-util")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use seleniumbase_rs::sb_cdp::{Browser, MockCtrl, Page, Permission};
use seleniumbase_rs::utils::selectors::SelectorBuf;
use serde_json::{json, Value};

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

/// A page on a mocked browser, with a short default wait.
async fn quick_page() -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    let page = browser.default_page().await.unwrap();
    (page.with_timeout(Duration::from_millis(200)), mock)
}

/// The expressions sent to `Runtime.evaluate`, oldest first, without the
/// helper script that opening a page installs.
fn evaluated(mock: &MockCtrl) -> Vec<String> {
    mock.calls_to("Runtime.evaluate")
        .into_iter()
        .filter_map(|call| call.params["expression"].as_str().map(str::to_owned))
        .filter(|expression| !expression.contains("window.__sbcdp = {"))
        .collect()
}

/// Makes every `Runtime.evaluate` answer with `answer`.
fn answer_every_script_with(mock: &MockCtrl, answer: Value) {
    mock.reply("Runtime.evaluate", value(answer));
}

// ----------------------------------------------------------------------
// find_element_by_text, find_elements_by_text, click_link, :contains()
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_contains_selector_is_sent_to_the_page_as_xpath() {
    let (page, mock) = quick_page().await;
    answer_every_script_with(&mock, json!(1));

    page.locator("button:contains(\"Go on\")")
        .count()
        .await
        .unwrap();

    let sent = evaluated(&mock).pop().unwrap();
    assert!(sent.contains(r#""kind":"xpath""#), "{sent}");
    assert!(sent.contains("//button[contains(., 'Go on')]"), "{sent}");
}

#[test]
fn a_contains_selector_converts_in_a_selector_buf_and_plain_css_stays_css() {
    assert_eq!(
        SelectorBuf::from("span:contains(\"Championship\")"),
        SelectorBuf::xpath("//span[contains(., 'Championship')]")
    );
    assert_eq!(
        SelectorBuf::from("div.card > a"),
        SelectorBuf::css("div.card > a")
    );
}

#[test]
fn text_selectors_find_any_element_not_only_links() {
    assert_eq!(
        SelectorBuf::containing_text("Sign in", Some("button")),
        SelectorBuf::xpath("//button[contains(normalize-space(.), 'Sign in')]")
    );
    // Without a tag the innermost element that holds the text is meant, so the
    // page's <html> and <body> do not match.
    assert_eq!(
        SelectorBuf::containing_text("Sign in", None),
        SelectorBuf::xpath("//*[text()[contains(normalize-space(.), 'Sign in')]]")
    );
}

#[test]
fn text_selectors_collapse_white_space_and_quote_awkward_text() {
    assert_eq!(
        SelectorBuf::containing_text("Sign \n   in", Some("a")),
        SelectorBuf::xpath("//a[contains(normalize-space(.), 'Sign in')]")
    );
    assert_eq!(
        SelectorBuf::containing_text("it's", Some("p")),
        SelectorBuf::xpath("//p[contains(normalize-space(.), \"it's\")]")
    );
    // A tag that is not a name cannot smuggle XPath in.
    assert_eq!(
        SelectorBuf::containing_text("x", Some("a] | //b[")),
        SelectorBuf::containing_text("x", None)
    );
}

// ----------------------------------------------------------------------
// evaluate, execute_script
// ----------------------------------------------------------------------

#[tokio::test]
async fn evaluate_drops_the_return_keyword_of_the_last_line_like_python() {
    let (page, mock) = quick_page().await;
    answer_every_script_with(&mock, json!(2));

    page.evaluate("return document.title").await.unwrap();
    page.evaluate("const a = 1;\n  return a + 1;\n")
        .await
        .unwrap();
    page.evaluate("document.title").await.unwrap();
    page.evaluate("(() => {\n  return 5;\n})()").await.unwrap();

    assert_eq!(
        evaluated(&mock),
        [
            "document.title",
            "const a = 1;\na + 1;",
            "document.title",
            "(() => {\n  return 5;\n})()",
        ]
    );
}

// ----------------------------------------------------------------------
// goto_if_not_url
// ----------------------------------------------------------------------

fn navigations(mock: &MockCtrl) -> Vec<String> {
    mock.calls_to("Page.navigate")
        .into_iter()
        .filter_map(|call| call.params["url"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn goto_if_not_url_stays_put_on_the_page_it_is_already_on() {
    let (page, mock) = quick_page().await;
    answer_every_script_with(&mock, json!("https://example.com/"));

    page.goto_if_not_url("https://example.com").await.unwrap();
    page.goto_if_not_url("example.com/").await.unwrap();

    assert!(navigations(&mock).is_empty(), "{:?}", navigations(&mock));
}

#[tokio::test]
async fn goto_if_not_url_navigates_when_the_page_differs() {
    let (page, mock) = quick_page().await;
    answer_every_script_with(&mock, json!("https://example.com/a"));

    page.goto_if_not_url("https://example.com/b").await.unwrap();

    assert_eq!(navigations(&mock), ["https://example.com/b"]);
}

#[tokio::test]
async fn goto_if_not_url_ignores_what_a_search_engine_tacks_on() {
    let (page, mock) = quick_page().await;
    answer_every_script_with(
        &mock,
        json!("https://www.bing.com/search?q=SeleniumBase&source=hp"),
    );

    page.goto_if_not_url("https://www.bing.com/search?q=SeleniumBase")
        .await
        .unwrap();

    assert!(navigations(&mock).is_empty(), "{:?}", navigations(&mock));
}

// ----------------------------------------------------------------------
// get_active_element, get_active_element_css
// ----------------------------------------------------------------------

#[tokio::test]
async fn the_active_element_is_named_by_a_selector_and_found_again_through_it() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(
            if expression.contains("sbBestSelector(document.activeElement)") {
                value(json!("input#email"))
            } else {
                value(Value::Null)
            },
        )
    });

    assert_eq!(page.active_element_css().await.unwrap(), "input#email");
    assert_eq!(
        page.active_element().await.unwrap().to_string(),
        "input#email"
    );

    // The script is wrapped in a function: a bare top-level `return` would be
    // a syntax error in Runtime.evaluate.
    let sent = evaluated(&mock).remove(0);
    assert!(sent.trim_start().starts_with("(function"), "{sent}");
    assert!(sent.trim_end().ends_with("})()"), "{sent}");
}

// ----------------------------------------------------------------------
// get_element_rect, get_element_position
// ----------------------------------------------------------------------

#[tokio::test]
async fn the_viewport_rect_is_the_window_relative_rectangle() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains(".length > 0") {
            value(json!(true))
        } else if expression.contains("getBoundingClientRect") && !expression.contains("scrollX") {
            value(json!({ "x": 10.0, "y": 20.0, "width": 30.0, "height": 40.0 }))
        } else {
            value(Value::Null)
        })
    });

    let rect = page.locator("#box").viewport_rect().await.unwrap();

    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (10.0, 20.0, 30.0, 40.0)
    );
}

// ----------------------------------------------------------------------
// click_and_hold
// ----------------------------------------------------------------------

#[tokio::test]
async fn click_and_hold_presses_waits_and_releases_at_the_centre() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("__sbcdp.center(") {
            value(json!({ "x": 120.0, "y": 80.0, "covered": false }))
        } else if expression.contains(".some(__sbcdp.visible)")
            || expression.contains(".length > 0")
        {
            value(json!(true))
        } else {
            value(Value::Null)
        })
    });

    let started = Instant::now();
    page.locator("#drag")
        .click_and_hold(Duration::from_millis(250))
        .await
        .unwrap();

    assert!(started.elapsed() >= Duration::from_millis(250));
    let events: Vec<_> = mock
        .calls_to("Input.dispatchMouseEvent")
        .into_iter()
        .map(|call| call.params["type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(events, ["mouseMoved", "mousePressed", "mouseReleased"]);
}

// ----------------------------------------------------------------------
// find_elements, find_all, select_all
// ----------------------------------------------------------------------

#[tokio::test]
async fn all_waiting_waits_for_the_first_match_and_then_returns_every_match() {
    let (page, mock) = quick_page().await;
    let looks = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&looks);
    // The list renders on the fourth look.
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        let ready = seen.fetch_add(1, Ordering::SeqCst) >= 3;
        Ok(if expression.contains(".length > 0") {
            value(json!(ready))
        } else if expression.contains(".length") {
            value(json!(if ready { 3 } else { 0 }))
        } else {
            value(Value::Null)
        })
    });

    let found = page
        .with_timeout(Duration::from_secs(3))
        .locator("li")
        .all_waiting()
        .await
        .unwrap();

    assert_eq!(found.len(), 3);
    assert_eq!(found[2].to_string(), "li [nth=2]");
}

#[tokio::test]
async fn all_waiting_gives_an_empty_list_when_nothing_ever_matches() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains(".length > 0") {
            value(json!(false))
        } else {
            value(json!(0))
        })
    });

    let found = page.locator("li").all_waiting().await.unwrap();

    assert!(found.is_empty());
}

// ----------------------------------------------------------------------
// flash
// ----------------------------------------------------------------------

#[tokio::test]
async fn flash_with_passes_the_colour_and_the_duration_to_the_page() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains(".length > 0") {
            value(json!(true))
        } else {
            value(Value::Null)
        })
    });

    page.locator("#logo")
        .flash_with("#44CC88", Duration::from_millis(1500))
        .await
        .unwrap();

    let sent = evaluated(&mock).pop().unwrap();
    assert!(
        sent.contains(r##"__sbcdp.flash(el, "#44CC88", 1500)"##),
        "{sent}"
    );
}

// ----------------------------------------------------------------------
// get_rd_host, get_rd_url, get_endpoint_url
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_browser_reports_the_host_its_devtools_endpoint_is_on() {
    let (browser, _mock) = Browser::new_mocked();

    assert_eq!(browser.debugging_host(), "127.0.0.1");
    assert_eq!(browser.http_url(), "http://127.0.0.1:0");
}

// ----------------------------------------------------------------------
// grant_all_permissions, grant_permissions
// ----------------------------------------------------------------------

#[test]
fn every_permission_python_grants_is_known_and_named_once() {
    let names: std::collections::BTreeSet<_> = Permission::ALL
        .iter()
        .map(|permission| permission.name())
        .collect();

    assert_eq!(Permission::ALL.len(), 23);
    assert_eq!(names.len(), 23, "a name is used twice");
    for expected in [
        "audioCapture",
        "backgroundSync",
        "backgroundFetch",
        "clipboardReadWrite",
        "clipboardSanitizedWrite",
        "displayCapture",
        "durableStorage",
        "geolocation",
        "idleDetection",
        "localFonts",
        "midi",
        "midiSysex",
        "nfc",
        "notifications",
        "paymentHandler",
        "periodicBackgroundSync",
        "sensors",
        "storageAccess",
        "topLevelStorageAccess",
        "videoCapture",
        "wakeLockScreen",
        "wakeLockSystem",
        "windowManagement",
    ] {
        assert!(names.contains(expected), "{expected} is missing");
    }
}

#[tokio::test]
async fn grant_all_permissions_asks_for_every_kind_for_all_origins() {
    let (browser, mock) = Browser::new_mocked();

    browser.grant_all_permissions().await.unwrap();

    let call = mock.calls_to("Browser.grantPermissions").pop().unwrap();
    assert_eq!(call.params["permissions"].as_array().unwrap().len(), 23);
    assert!(call.params.get("origin").is_none(), "{}", call.params);
}

#[tokio::test]
async fn permissions_can_be_granted_for_one_origin() {
    let (browser, mock) = Browser::new_mocked();

    browser
        .grant_permissions_for("https://example.com", &[Permission::Camera])
        .await
        .unwrap();

    let call = mock.calls_to("Browser.grantPermissions").pop().unwrap();
    assert_eq!(call.params["permissions"], json!(["videoCapture"]));
    assert_eq!(call.params["origin"], "https://example.com");
}

// ----------------------------------------------------------------------
// set_locale
// ----------------------------------------------------------------------

#[tokio::test]
async fn setting_the_locale_also_sets_the_accept_language_the_page_reports() {
    let (page, mock) = quick_page().await;
    let metadata =
        json!({ "brands": [{ "brand": "Chromium", "version": "155" }], "platform": "macOS" });
    let reply = json!({ "userAgent": "Mozilla/5.0 Test", "metadata": metadata });
    mock.on("Runtime.evaluate", move |_| Ok(value(reply.clone())));

    page.emulation().locale("de-DE").await.unwrap();

    let locale = mock.calls_to("Emulation.setLocaleOverride").pop().unwrap();
    assert_eq!(locale.params["locale"], "de-DE");
    let agent = mock
        .calls_to("Emulation.setUserAgentOverride")
        .pop()
        .unwrap();
    assert_eq!(agent.params["userAgent"], "Mozilla/5.0 Test");
    assert_eq!(agent.params["acceptLanguage"], "de-DE");
    // The client hints the page had stay as they were.
    assert_eq!(agent.params["userAgentMetadata"], metadata);
}

#[tokio::test]
async fn setting_the_locale_keeps_working_on_a_page_without_client_hints() {
    let (page, mock) = quick_page().await;
    mock.on("Runtime.evaluate", |_| {
        Ok(value(
            json!({ "userAgent": "Mozilla/5.0 Test", "metadata": null }),
        ))
    });

    page.emulation().locale("fr-FR").await.unwrap();

    let agent = mock
        .calls_to("Emulation.setUserAgentOverride")
        .pop()
        .unwrap();
    assert_eq!(agent.params["acceptLanguage"], "fr-FR");
    assert!(agent.params.get("userAgentMetadata").is_none());
}
