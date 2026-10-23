//! The Recorder Mode page script (`RECORDER_JS`) and the pipeline behind it,
//! against a real Chrome.
//!
//! The test starts a headless Chrome through chromedriver, so it is
//! `#[ignore]`d. Run it by itself:
//!
//! ```text
//! cargo test --test recorder_script_chrome -- --ignored --test-threads=1
//! ```
//!
//! A person's actions are made through WebDriver (a click on a label, typing
//! into fields, Enter in a search box), which Chrome turns into real mouse and
//! keyboard events. The page script stores what it saw in sessionStorage, and
//! `ActionRecorder::from_raw_actions` turns that into the calls of a test.

use std::io::{Read, Write};

use seleniumbase_rs::api::recorder::{ActionRecorder, RawAction, RECORDER_JS};
use seleniumbase_rs::{BaseCase, BrowserConfig};

const HOME: &str = r##"<!doctype html><title>Home</title>
<h1>Home page</h1>
<input id="name" type="text">
<input id="pw" type="password">
<label id="agree-label" for="agree">I agree</label><input id="agree" type="checkbox">
<form action="/search" method="get"><input name="q" id="q" type="text"></form>
"##;

const FOUND: &str = r##"<!doctype html><title>Results</title><p id="found">found</p>"##;

/// Serves the pages on a loopback port and returns the address.
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
                let body = if request.starts_with("GET /search") {
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

/// What sessionStorage `recorded_actions` holds on the current page.
async fn stored_actions(sb: &BaseCase) -> Vec<RawAction> {
    let json = sb
        .execute_script("return sessionStorage.getItem('recorded_actions')")
        .await
        .expect("the script runs");
    RawAction::parse_list(json.as_str().expect("the page stored actions")).expect("valid actions")
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_label_click_typing_a_password_and_enter_become_the_calls_of_a_test() {
    let base = serve_pages();
    let config = BrowserConfig {
        headless: true,
        ..BrowserConfig::default()
    };
    let mut sb = BaseCase::new(config).await.expect("Chrome starts");
    sb.open(&format!("{base}/")).await.expect("the home page");
    sb.execute_script(RECORDER_JS)
        .await
        .expect("the script installs");

    sb.click("#agree-label").await.unwrap();
    sb.type_text("#name", "Ada").await.unwrap();
    sb.type_text("#pw", "hunter2").await.unwrap();
    sb.type_text("#q", "cats\n").await.unwrap();
    sb.assert_text("found", "#found")
        .await
        .expect("the form was sent");
    // A new document: the page script is installed on it, as the browser's
    // new-document hook will do.
    sb.execute_script(RECORDER_JS)
        .await
        .expect("the script installs again");

    let raw = stored_actions(&sb).await;
    let kinds: Vec<&str> = raw.iter().map(|action| action.kind.as_str()).collect();
    assert_eq!(kinds.first(), Some(&"begin"), "{kinds:?}");
    assert!(kinds.contains(&"_url_"), "{kinds:?}");
    let password = raw
        .iter()
        .rev()
        .find(|action| action.kind == "input" && action.secret);
    assert!(password.is_some(), "the password is marked: {raw:?}");

    let recorder = ActionRecorder::from_raw_actions(raw, 0, &[], false);
    let calls: Vec<(&str, &str, &str)> = recorder
        .actions
        .iter()
        .map(|a| {
            (
                a.name.as_str(),
                a.target.as_deref().unwrap_or(""),
                a.value.as_deref().unwrap_or(""),
            )
        })
        .collect();
    // One call for the label (the box is not toggled twice), the three typings
    // (the password is not written down) and the page the form led to.
    assert_eq!(calls[0].0, "goto", "{calls:?}");
    assert!(
        calls
            .iter()
            .any(|c| c.0 == "js_click" && c.1.contains("agree-label")),
        "{calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| c.0 == "type_text" && c.1.contains("name") && c.2 == "Ada"),
        "{calls:?}"
    );
    assert!(
        calls.iter().any(|c| c.0 == "type_text"
            && c.1.contains("pw")
            && c.2.starts_with("{{SB_RECORDED_SECRET}}")),
        "{calls:?}"
    );
    assert!(
        calls.iter().any(|c| c.0 == "type_text" && c.2 == "cats\n"),
        "{calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|c| c.0 == "goto_if_not_url" && c.1.ends_with("/search?q=cats")),
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.2.contains("hunter2")),
        "the password must not be written: {calls:?}"
    );
    sb.quit().await.unwrap();
}
