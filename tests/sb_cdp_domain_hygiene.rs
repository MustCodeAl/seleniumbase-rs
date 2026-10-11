//! The Pure CDP engine never switches on a protocol domain a page can feel.
//!
//! `Runtime.enable` makes the browser report every console call and every
//! execution context to the client, and pages probe for exactly that traffic.
//! `DOM.enable`, `Log.enable`, `Debugger.enable` and `Profiler.enable` start
//! agents with the same kind of side effect. Nothing the engine does needs
//! them: `Runtime.evaluate` and `Page.addScriptToEvaluateOnNewDocument` work
//! without any of them.
//!
//! These tests drive the whole launch-and-navigate flow against a scripted
//! browser and read back every command it received, so a new call that enables
//! one of the five domains fails here. A last test reads the sources for the
//! same names, so the other engines are held to the same rule. No Chrome is
//! needed. Run them with `cargo test --features test-util --test
//! sb_cdp_domain_hygiene`.

#![cfg(feature = "test-util")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{
    Browser, Cookie, Key, MockCtrl, Page, Point, ResourceType, Rule, SelectBy,
};
use seleniumbase_rs::stealth::behavior::Behavior;
use seleniumbase_rs::stealth::cdp::{is_detectable_enable, DETECTABLE_DOMAINS};
use seleniumbase_rs::{Fingerprint, OsType, ProxyConfig};
use serde_json::{json, Value};

/// The domains whose `enable` command is never sent. This is the specification
/// and does not read the library's own list, so the two can be compared.
const FORBIDDEN: [&str; 5] = ["Runtime", "DOM", "Log", "Debugger", "Profiler"];

/// A short wait, so a step that would block fails quickly.
const QUICK: Duration = Duration::from_millis(150);

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

/// A page on a mocked browser, with a short default wait.
async fn quick_page() -> (Browser, Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    let page = browser.default_page().await.unwrap().with_timeout(QUICK);
    (browser, page, mock)
}

/// Answers the page helper calls a locator makes: a count, a centre point, and
/// visibility, as if one visible element were on the page.
fn script_one_visible_element(mock: &MockCtrl) {
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("__sbcdp.center(") {
            value(json!({ "x": 120.0, "y": 80.0, "covered": false }))
        } else if expression.contains("window.innerWidth") {
            value(json!([1000.0, 800.0]))
        } else if expression.contains("__sbcdp.info(") {
            value(json!({
                "tag": "button", "text": "", "html": "", "attributes": {}, "visible": true,
                "rect": { "x": 70.0, "y": 60.0, "width": 100.0, "height": 40.0 },
            }))
        } else if expression.contains(".some(__sbcdp.visible)")
            || expression.contains(".length > 0")
            || expression.contains(".some(")
        {
            value(json!(true))
        } else if expression.contains("resolve(") && expression.contains(".length") {
            value(json!(1))
        } else {
            value(Value::Null)
        })
    });
}

/// Fails, naming each call, if the browser was ever told to enable a forbidden
/// domain. Also checks that the flow really ran, so an empty record cannot pass.
fn assert_nothing_detectable(mock: &MockCtrl, flow: &str) {
    let calls = mock.calls();
    assert!(
        calls.iter().any(|call| call.method == "Page.enable"),
        "{flow}: the flow sent no Page.enable, so nothing was exercised"
    );
    for domain in FORBIDDEN {
        let method = format!("{domain}.enable");
        let sent = mock.calls_to(&method);
        assert!(
            sent.is_empty(),
            "{flow}: {method} was sent {} time(s): {sent:?}",
            sent.len()
        );
    }
    let by_predicate: Vec<_> = calls
        .iter()
        .filter(|call| is_detectable_enable(&call.method))
        .map(|call| call.method.as_str())
        .collect();
    assert!(
        by_predicate.is_empty(),
        "{flow}: the library's own check found {by_predicate:?}"
    );
}

#[test]
fn the_librarys_list_is_the_five_domains() {
    assert_eq!(DETECTABLE_DOMAINS, FORBIDDEN);
}

#[tokio::test(start_paused = true)]
async fn launching_and_attaching_a_tab_enables_none_of_them() {
    let (browser, mock) = Browser::new_mocked();

    let page = browser.default_page().await.unwrap();
    let second = browser.new_page(Some("example.com")).await.unwrap();
    let window = browser.new_window(None::<&str>).await.unwrap();
    let all = browser.pages().await.unwrap();

    assert!(!all.is_empty());
    assert_ne!(page.id(), second.id());
    assert!(!window.id().is_empty());
    assert_nothing_detectable(&mock, "attach");
    // What the engine does send on attach, so a change is a decision.
    let sent: Vec<_> = mock
        .calls()
        .into_iter()
        .filter(|call| call.method.ends_with(".enable"))
        .map(|call| call.method)
        .collect();
    assert!(
        sent.iter().all(|method| method == "Page.enable"),
        "attaching enables only the Page domain, got {sent:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn navigating_and_reading_a_page_enables_none_of_them() {
    let (_browser, page, mock) = quick_page().await;
    mock.reply("Page.captureScreenshot", json!({ "data": "AAAA" }));
    mock.reply("Page.printToPDF", json!({ "data": "AAAA" }));
    // A history step waits for the document to be complete at an address other
    // than the one it left, so every read of the address is a new one.
    let reads = Arc::new(AtomicUsize::new(0));
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("document.readyState") {
            value(json!("complete"))
        } else if expression.contains("window.location.href") {
            let read = reads.fetch_add(1, Ordering::SeqCst);
            value(json!(format!("https://example.com/{read}")))
        } else {
            value(Value::Null)
        })
    });
    mock.reply(
        "Page.getNavigationHistory",
        json!({ "currentIndex": 1, "entries": [
            { "id": 1, "url": "about:blank" },
            { "id": 2, "url": "https://example.com/" },
            { "id": 3, "url": "https://example.com/next" },
        ]}),
    );

    page.goto("https://example.com/").await.unwrap();
    page.reload().await.unwrap();
    page.hard_reload().await.unwrap();
    page.back().await.unwrap();
    page.forward().await.unwrap();
    page.evaluate("1 + 1").await.unwrap();
    let _ = page.title().await;
    let _ = page.url().await;
    let _ = page.content().await;
    let _ = page.user_agent().await;
    let _ = page.js_errors().await;
    page.screenshot().await.unwrap();
    page.pdf().await.unwrap();
    page.bring_to_front().await.unwrap();

    assert_eq!(mock.calls_to("Page.navigate").len(), 1);
    assert_nothing_detectable(&mock, "navigation");
}

#[tokio::test(start_paused = true)]
async fn finding_and_driving_elements_enables_none_of_them() {
    let (_browser, page, mock) = quick_page().await;
    script_one_visible_element(&mock);
    let button = page.locator("#go");
    let field = page.locator("#name");

    button.click().await.unwrap();
    button.hover().await.unwrap();
    field.fill("text").await.unwrap();
    field.type_text("ab").await.unwrap();
    field.press(Key::Enter).await.unwrap();
    let _ = page
        .locator("select")
        .select_option(SelectBy::Text("Mars"))
        .await;
    let _ = field.text().await;
    let _ = button.count().await;

    assert!(!mock.calls_to("Input.dispatchMouseEvent").is_empty());
    assert_nothing_detectable(&mock, "elements");
}

#[tokio::test(start_paused = true)]
async fn applying_every_persona_enables_none_of_them() {
    let (browser, page, mock) = quick_page().await;
    mock.reply(
        "Target.getTargetInfo",
        json!({ "targetInfo": { "browserContextId": "CTX1" } }),
    );

    for (index, os) in [
        OsType::Windows,
        OsType::Macos,
        OsType::Linux,
        OsType::Android,
        OsType::Ios,
    ]
    .into_iter()
    .enumerate()
    {
        let mut fingerprint = Fingerprint::randomized(os, 100 + index as u64);
        fingerprint.flags.grant_permissions = true;
        fingerprint.flags.block_trackers = true;
        fingerprint.proxy = Some(ProxyConfig {
            r#type: "http".to_owned(),
            host: "proxy.example.com".to_owned(),
            port: 8080,
            username: Some("user".to_owned()),
            password: Some("pass".to_owned()),
            save_traffic: false,
        });
        page.apply_fingerprint(&fingerprint).await.unwrap();
        let fresh = browser.new_page(None::<&str>).await.unwrap();
        fresh.apply_fingerprint(&fingerprint).await.unwrap();
        fresh.goto("https://example.com/").await.unwrap();
    }

    assert!(!mock
        .calls_to("Emulation.setDeviceMetricsOverride")
        .is_empty());
    assert_nothing_detectable(&mock, "personas");
}

#[tokio::test(start_paused = true)]
async fn state_and_environment_calls_enable_none_of_them() {
    let (_browser, page, mock) = quick_page().await;
    mock.reply(
        "Network.getAllCookies",
        json!({ "cookies": [
            { "name": "a", "value": "1", "domain": "x", "path": "/", "expires": -1 },
        ]}),
    );

    let cookies = page.cookies();
    cookies
        .set(&[Cookie::new("session", "value")])
        .await
        .unwrap();
    let _ = cookies.all().await;
    let _ = cookies.header().await;
    cookies.clear().await.unwrap();
    for storage in [page.local_storage(), page.session_storage()] {
        storage.set("key", "value").await.unwrap();
        let _ = storage.get("key").await;
        let _ = storage.entries().await;
        storage.remove("key").await.unwrap();
        storage.clear().await.unwrap();
    }
    let emulation = page.emulation();
    emulation.timezone("Europe/Berlin").await.unwrap();
    emulation.locale("de-DE").await.unwrap();
    emulation.geolocation(52.5, 13.4, 10.0).await.unwrap();
    emulation
        .user_agent("agent/1.0", Some("MacIntel"))
        .await
        .unwrap();
    emulation.offline(true).await.unwrap();
    emulation.offline(false).await.unwrap();
    let _ = page.window().bounds().await;

    assert!(!mock.calls_to("Network.setCookies").is_empty());
    assert_nothing_detectable(&mock, "state");
}

#[tokio::test(start_paused = true)]
async fn contexts_interception_and_human_input_enable_none_of_them() {
    let (browser, page, mock) = quick_page().await;
    script_one_visible_element(&mock);
    mock.reply(
        "Target.createBrowserContext",
        json!({ "browserContextId": "CTX2" }),
    );

    let context = browser.new_context().await.unwrap();
    let in_context = context.new_page(Some("example.com")).await.unwrap();
    let interception = page
        .intercept(vec![
            Rule::block().resource_type(ResourceType::Image),
            Rule::block().url("*/ads/*"),
        ])
        .await
        .unwrap();
    let person = page.human(Behavior::builder().seed(9).build().unwrap());
    person
        .move_to(Point::new(300.0, 200.0), 80.0)
        .await
        .unwrap();
    let _ = page.solve_captcha().await;
    in_context.close().await.unwrap();
    let _ = interception.stop().await;
    context.dispose().await.unwrap();

    assert!(!mock.calls_to("Fetch.enable").is_empty());
    assert_nothing_detectable(&mock, "context");
}

// ----------------------------------------------------------------------
// The sources
// ----------------------------------------------------------------------

/// A source file that sends one of the five commands today, with how many
/// times, so it cannot grow. No file does: the WebDriver-hosted CDP page in
/// `src/api/cdp_page.rs` was the last one, and it now reads the page with
/// `Runtime.evaluate`. Adding an entry here is a decision, not a convenience.
const KNOWN_SENDERS: [(&str, &str, usize); 0] = [];

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

/// The lines of `text` that are code outside any test module: comment lines
/// are dropped, and so is everything from the first `#[cfg(test)]` on.
fn production_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines()
        .take_while(|line| line.trim() != "#[cfg(test)]")
        .filter(|line| !line.trim_start().starts_with("//"))
}

#[test]
fn the_source_scan_sees_a_send_and_skips_comments_and_test_modules() {
    let text = "fn send() { client.send(\"Runtime.enable\"); }\n\
                // \"Log.enable\" is only mentioned here\n\
                /// and \"Debugger.enable\" here\n\
                #[cfg(test)]\n\
                mod tests { fn t() { \"DOM.enable\"; } }\n";

    let kept: Vec<_> = production_lines(text).collect();

    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(kept[0].contains("\"Runtime.enable\""));
}

#[test]
fn no_source_file_names_one_of_the_commands_except_the_known_ones() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    assert!(files.len() > 100, "found only {} source files", files.len());

    for file in files {
        let relative = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&file).unwrap();
        for domain in FORBIDDEN {
            let method = format!("{domain}.enable");
            let quoted = format!("\"{method}\"");
            let found = production_lines(&text)
                .filter(|line| line.contains(&quoted))
                .count();
            let allowed = KNOWN_SENDERS
                .iter()
                .find(|(path, name, _)| *path == relative && *name == method)
                .map_or(0, |(_, _, count)| *count);
            assert!(
                found <= allowed,
                "{relative} sends {method} {found} time(s); {allowed} are known"
            );
        }
    }
}
