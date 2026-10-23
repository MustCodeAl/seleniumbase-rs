//! The live recorder against a real Chrome, one defect at a time.
//!
//! Every test starts a headless Chrome through chromedriver, so they are
//! `#[ignore]`d. Run them one binary at a time:
//!
//! ```text
//! cargo test --test recorder_chrome -- --ignored --test-threads=1
//! ```
//!
//! The pages come from a small server on a loopback port. Each test drives the
//! page the way a person would (a click on a label, Enter in a field) and
//! checks what the recording holds and whether it replays.

use std::io::{Read, Write};

use seleniumbase_rs::api::recorder::{Poll, RecordedAction, RecorderSession};
use seleniumbase_rs::browser::config::DriverMode;
use seleniumbase_rs::{BaseCase, BrowserConfig};

const HOME: &str = r##"<!doctype html><title>Home</title>
<input id="name">
<input id="pw" type="password">
<label id="agree-label" for="agree">I agree</label><input id="agree" type="checkbox">
<form action="/search" method="get"><input name="q" id="q" type="text"></form>
<a id="next" href="/b">Next</a>
<button id="boom" type="button" onclick="alert('hello')">Alert</button>
<select id="color"><option value="r">Red</option><option value="g">Green</option></select>
<section><div><div><div><div><div><div><div><button class="x">One</button></div></div></div></div></div></div></div></section>
<section><div><div><div><div><div><div><div><button class="x">Two</button></div></div></div></div></div></div></div></section>
"##;

const SECOND: &str =
    r##"<!doctype html><title>B</title><button id="done" type="button">Done</button>"##;

const FOUND: &str = r##"<!doctype html><title>Results</title><p id="found">found</p>"##;

/// Serves the pages on a loopback port and returns the address. A thread
/// answers each connection, as Chrome opens several at once.
fn serve_pages() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            std::thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]);
                let body = if request.starts_with("GET /b") {
                    SECOND
                } else if request.starts_with("GET /search") {
                    FOUND
                } else if request.starts_with("GET / ") {
                    HOME
                } else {
                    ""
                };
                let status = if body.is_empty() {
                    "404 Not Found"
                } else {
                    "200 OK"
                };
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    format!("http://{address}")
}

/// A headless Chrome in `mode`, on the home page, with the listeners installed.
async fn recording(mode: DriverMode) -> (BaseCase, RecorderSession, String) {
    let base = serve_pages();
    let mut config = BrowserConfig {
        headless: true,
        mode,
        ..BrowserConfig::default()
    };
    RecorderSession::prepare_config(&mut config);
    let mut sb = BaseCase::new(config).await.expect("Chrome starts");
    let mut session = RecorderSession::new();
    session
        .goto(&mut sb, &format!("{base}/"))
        .await
        .expect("the home page opens");
    session.install(&sb).await.expect("the listeners install");
    (sb, session, base)
}

fn names(actions: &[RecordedAction]) -> Vec<&str> {
    actions.iter().map(|a| a.name.as_str()).collect()
}

/// How many elements `selector` matches on the page, and the `id` or text of
/// the first.
async fn matches(sb: &BaseCase, selector: &str) -> (u64, String) {
    let quoted = serde_json::to_string(selector).unwrap();
    let script = format!(
        "var found = document.querySelectorAll({quoted}); \
         return JSON.stringify([found.length, found.length ? (found[0].id || found[0].textContent) : '']);"
    );
    let raw = sb.execute_script(&script).await.expect("the script runs");
    let (count, first): (u64, String) = serde_json::from_str(raw.as_str().unwrap()).unwrap();
    (count, first)
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_click_on_a_label_is_recorded_once_and_replays_once() {
    let (mut sb, mut session, base) = recording(DriverMode::Cdp).await;

    sb.click("#agree-label").await.unwrap();
    session.drain(&mut sb).await.unwrap();

    assert_eq!(
        names(session.actions()),
        ["goto", "click"],
        "{:?}",
        session.actions()
    );
    let selector = session.actions()[1].target.clone().unwrap();
    assert_eq!(matches(&sb, &selector).await, (1, "agree-label".to_owned()));

    // Replayed, the box ends up checked: one toggle, not two.
    sb.open(&format!("{base}/")).await.unwrap();
    let report = RecorderSession::replay(&mut sb, session.actions())
        .await
        .unwrap();
    assert!(report.is_success(), "{report:?}");
    let checked = sb
        .execute_script("return document.getElementById('agree').checked")
        .await
        .unwrap();
    assert_eq!(checked.as_bool(), Some(true));
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_click_on_a_select_is_not_recorded_before_its_change() {
    let (mut sb, mut session, _base) = recording(DriverMode::Cdp).await;

    sb.select_option_by_value("#color", "g").await.unwrap();
    session.drain(&mut sb).await.unwrap();

    assert_eq!(
        names(session.actions()),
        ["goto", "select_option_by_value"],
        "{:?}",
        session.actions()
    );
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn enter_in_a_field_is_recorded_with_the_text_and_the_page_it_led_to() {
    let (mut sb, mut session, base) = recording(DriverMode::Cdp).await;

    sb.type_text("#q", "rust\n").await.unwrap();
    sb.wait_for_element_visible("#found", 5).await.unwrap();
    session.drain(&mut sb).await.unwrap();

    let steps: Vec<_> = session
        .actions()
        .iter()
        .map(|a| (a.name.as_str(), a.target.as_deref(), a.value.as_deref()))
        .collect();
    let results = format!("{base}/search?q=rust");
    assert_eq!(
        steps,
        [
            ("goto", Some(format!("{base}/").as_str()), None),
            ("type", Some("#q"), Some("rust\n")),
            ("goto_if_not_url", Some(results.as_str()), None),
        ]
    );

    // Replayed, the form is submitted again.
    sb.open(&format!("{base}/")).await.unwrap();
    let report = RecorderSession::replay(&mut sb, session.actions())
        .await
        .unwrap();
    assert!(report.is_success(), "{report:?}");
    assert_eq!(sb.get_current_url().await.unwrap(), results);
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_typed_password_is_not_written_to_the_recording() {
    let (mut sb, mut session, _base) = recording(DriverMode::Cdp).await;

    sb.type_text("#pw", "hunter2").await.unwrap();
    // Leaving the field is what commits the value.
    sb.click("#name").await.unwrap();
    session.drain(&mut sb).await.unwrap();

    let saved = format!("{}{}", session.to_json().unwrap(), session.to_yaml());
    assert!(!saved.contains("hunter2"), "{saved}");
    let typed = session
        .actions()
        .iter()
        .find(|a| a.name == "type" && a.target.as_deref() == Some("#pw"))
        .expect("the typing is still recorded");
    assert_eq!(typed.value.as_deref(), Some("{{SB_RECORDED_SECRET}}"));
    let script = session.to_action_recorder().to_rust_test("login");
    assert!(!script.contains("hunter2"), "{script}");
    assert!(script.contains("SB_RECORDED_SECRET"), "{script}");
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_selector_is_unique_even_deep_in_a_repeated_structure() {
    let (mut sb, mut session, _base) = recording(DriverMode::Cdp).await;

    sb.click("section:nth-of-type(2) button.x").await.unwrap();
    session.drain(&mut sb).await.unwrap();

    let selector = session.actions()[1].target.clone().unwrap();
    let (count, first) = matches(&sb, &selector).await;
    assert_eq!((count, first.as_str()), (1, "Two"), "selector {selector:?}");
    sb.quit().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn an_alert_neither_ends_the_recording_nor_is_dismissed_by_it() {
    let (mut sb, mut session, _base) = recording(DriverMode::Cdp).await;

    // The page's own click handler raises the alert; the call itself may
    // report that.
    let _ = sb.click("#boom").await;
    assert!(
        matches!(session.poll(&mut sb).await, Poll::Dialog),
        "an open dialog is not the end of the session"
    );
    // The person's dialog is still there to answer.
    assert!(matches!(session.poll(&mut sb).await, Poll::Dialog));
    sb.wait_for_and_accept_alert(5)
        .await
        .expect("the alert was left open");
    assert!(matches!(session.poll(&mut sb).await, Poll::Captured(_)));

    assert_eq!(
        names(session.actions()),
        ["goto", "click"],
        "{:?}",
        session.actions()
    );
    sb.quit().await.unwrap();

    // A window that was closed is the end.
    // (Covered by the CLI: `poll` returns `Poll::Ended` once the browser is gone.)
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_click_that_leaves_the_page_is_recorded_and_the_next_page_is_a_goto_if_not_url() {
    let (mut sb, mut session, base) = recording(DriverMode::Cdp).await;

    sb.click("#next").await.unwrap();
    sb.wait_for_element_visible("#done", 5).await.unwrap();
    // No drain in between: the listeners are in the new page from its first moment.
    sb.click("#done").await.unwrap();
    session.drain(&mut sb).await.unwrap();

    let steps: Vec<_> = session
        .actions()
        .iter()
        .map(|a| (a.name.as_str(), a.target.clone()))
        .collect();
    assert_eq!(
        steps,
        [
            ("goto", Some(format!("{base}/"))),
            ("click", Some("#next".to_owned())),
            ("goto_if_not_url", Some(format!("{base}/b"))),
            ("click", Some("#done".to_owned())),
        ]
    );
    sb.quit().await.unwrap();
}
