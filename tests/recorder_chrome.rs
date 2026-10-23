//! Recorder Mode on `BaseCase` against a real Chrome.
//!
//! A person's actions are made with WebDriver here, which sends the same
//! trusted mouse and key events, so the page script records them as it records
//! a person's. Each test starts a headless Chrome through chromedriver, so
//! they are `#[ignore]`d. Run them one at a time:
//!
//! ```text
//! cargo test --test recorder_chrome -- --ignored --test-threads=1
//! ```
//!
//! The pages are served on two loopback ports, which are two origins.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use seleniumbase_rs::api::recorder::{RecorderEnd, RecordingTarget, SHIFT_ESC_JS};
use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Key codes of the WebDriver protocol.
const ESCAPE: &str = "\u{E00C}";
const SHIFT: &str = "\u{E008}";

const HOME: &str = r##"<!doctype html><title>Home</title>
<h1 id="title">Home page</h1>
<form action="/search" method="get"><input id="q" name="q" type="text"></form>
<label id="agree-label" for="agree">I agree</label><input id="agree" type="checkbox">
<input id="pw" type="password">
<button id="paused" type="button">Paused</button>
<button id="resumed" type="button">Resumed</button>
<a id="other" href="OTHER/">Other origin</a>
"##;

const SEARCH: &str = r##"<!doctype html><title>Results</title><h1 id="results">Results</h1>"##;

const OTHER: &str = r##"<!doctype html><title>Other</title>
<button id="b" type="button">B</button><p id="msg">Other origin</p>"##;

/// Serves `pages` (path, body) on a loopback port and returns the port.
fn serve(listener: std::net::TcpListener, pages: Vec<(&'static str, String)>) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            let target = request.split_whitespace().nth(1).unwrap_or("/");
            let path = target.split('?').next().unwrap_or("/");
            let body = pages
                .iter()
                .find(|(page, _)| *page == path)
                .map_or("", |(_, body)| body.as_str());
            let status = if body.is_empty() {
                "404 Not Found"
            } else {
                "200 OK"
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
}

/// The addresses of the two origins: the home site and the other one.
fn sites() -> (String, String) {
    let home = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let other = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let home_url = format!("http://{}", home.local_addr().unwrap());
    let other_url = format!("http://{}", other.local_addr().unwrap());
    serve(
        home,
        vec![
            ("/", HOME.replace("OTHER", &other_url)),
            ("/search", SEARCH.to_owned()),
        ],
    );
    serve(other, vec![("/", OTHER.to_owned())]);
    (home_url, other_url)
}

fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "sbrs-recorder-chrome-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// A headless Chrome with Recorder Mode on, whose recording goes to `folder`.
async fn recording(folder: &Path) -> BaseCase {
    let config = BrowserConfig {
        headless: true,
        ..BrowserConfig::default()
    }
    .with_recorder_mode(true);
    let mut sb = BaseCase::new(config).await.expect("Chrome starts");
    sb.set_recording_output(
        RecordingTarget::new("session", "Session", "test_session").in_folder(folder),
    );
    sb
}

/// The test the recording was written as.
fn written(folder: &Path) -> String {
    std::fs::read_to_string(folder.join("session_rec.rs"))
        .unwrap_or_else(|error| panic!("no recording in {}: {error}", folder.display()))
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_label_click_and_typing_with_enter_are_written_as_a_test() {
    let (home, _) = sites();
    let folder = folder("typing");
    let mut sb = recording(&folder).await;

    sb.open(&format!("{home}/")).await.unwrap();
    sb.click("#agree").await.unwrap();
    sb.type_text("#q", "rust\n").await.unwrap();
    sb.sleep(1.0).await;
    sb.quit().await.unwrap();

    let code = written(&folder);
    assert!(code.contains("sb.goto(\""), "{code}");
    assert_eq!(
        code.matches("check_if_unchecked(\"input#agree\")").count(),
        1,
        "{code}"
    );
    assert!(
        code.contains("sb.type_text(\"input#q\", \"rust\\n\")"),
        "{code}"
    );
    assert!(code.contains("goto_if_not_url"), "the search page: {code}");
    assert!(folder.join("session_rec.json").exists());
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn caret_then_click_asserts_an_element_and_ampersand_then_click_asserts_text() {
    let (home, _) = sites();
    let folder = folder("asserts");
    let mut sb = recording(&folder).await;

    sb.open(&format!("{home}/")).await.unwrap();
    sb.add_text("body", "^").await.unwrap();
    sb.click("#title").await.unwrap();
    sb.add_text("body", "&").await.unwrap();
    sb.click("#title").await.unwrap();
    sb.sleep(0.5).await;
    sb.quit().await.unwrap();

    let code = written(&folder);
    assert!(code.contains("sb.assert_element(\"h1#title\")"), "{code}");
    assert!(code.contains("(\"Home page\", \"h1#title\")"), "{code}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn escape_pauses_the_recording_and_tilde_resumes_it() {
    let (home, _) = sites();
    let folder = folder("pause");
    let mut sb = recording(&folder).await;

    sb.open(&format!("{home}/")).await.unwrap();
    sb.add_text("body", ESCAPE).await.unwrap();
    sb.click("#paused").await.unwrap();
    sb.add_text("body", "~").await.unwrap();
    sb.click("#resumed").await.unwrap();
    sb.sleep(0.5).await;
    sb.quit().await.unwrap();

    let code = written(&folder);
    assert!(
        !code.contains("#paused"),
        "a paused click is not recorded: {code}"
    );
    assert!(code.contains("sb.click(\"button#resumed\")"), "{code}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_link_to_another_origin_opens_a_tab_and_both_tabs_are_recorded_in_order() {
    let (home, other) = sites();
    let folder = folder("tabs");
    let mut sb = recording(&folder).await;

    sb.open(&format!("{home}/")).await.unwrap();
    sb.click("#other").await.unwrap();
    // The watch follows the new tab and starts the recorder in it.
    let end = sb
        .wait_for_recorder_end(Duration::from_secs(3))
        .await
        .unwrap();
    assert_eq!(end, RecorderEnd::TimeLimit);
    sb.click("#b").await.unwrap();
    sb.sleep(0.5).await;
    sb.quit().await.unwrap();

    let code = written(&folder);
    let at = |needle: &str| {
        code.find(needle)
            .unwrap_or_else(|| panic!("{needle} in {code}"))
    };
    assert!(at(&home) < at("sb.click(\"a#other\")"), "{code}");
    assert!(at("sb.click(\"a#other\")") < at(&other), "{code}");
    assert!(at(&other) < at("sb.click(\"button#b\")"), "{code}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn shift_escape_ends_the_wait_for_the_end_of_the_recording() {
    let (home, _) = sites();
    let folder = folder("end");
    let mut sb = recording(&folder).await;

    sb.open(&format!("{home}/")).await.unwrap();
    sb.execute_script(SHIFT_ESC_JS).await.unwrap();
    sb.add_text("body", &format!("{SHIFT}{ESCAPE}"))
        .await
        .unwrap();
    let end = sb
        .wait_for_recorder_end(Duration::from_secs(20))
        .await
        .unwrap();
    assert_eq!(end, RecorderEnd::ShiftEsc);
    sb.quit().await.unwrap();
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_dialog_does_not_end_the_watch_and_stays_open() {
    let (home, _) = sites();
    let folder = folder("alert");
    let mut sb = recording(&folder).await;

    sb.open(&format!("{home}/")).await.unwrap();
    sb.execute_script("setTimeout(function () { alert('hi'); }, 50); return 1;")
        .await
        .unwrap();
    let end = sb
        .wait_for_recorder_end(Duration::from_secs(2))
        .await
        .unwrap();
    assert_eq!(
        end,
        RecorderEnd::TimeLimit,
        "the alert did not end the watch"
    );
    // Polling did not dismiss it: it is still there to be accepted.
    sb.accept_alert().await.expect("the dialog stayed open");
    sb.quit().await.unwrap();
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_typed_password_is_read_from_the_environment_by_the_test() {
    let (home, _) = sites();
    let folder = folder("secret");
    let mut sb = recording(&folder).await;

    sb.open(&format!("{home}/")).await.unwrap();
    sb.type_text("#pw", "hunter2").await.unwrap();
    sb.click("#title").await.unwrap();
    sb.sleep(0.5).await;
    sb.quit().await.unwrap();

    let code = written(&folder);
    assert!(
        !code.contains("hunter2"),
        "the password was written down: {code}"
    );
    assert!(code.contains("recorded_secret"), "{code}");
    let json = std::fs::read_to_string(folder.join("session_rec.json")).unwrap();
    assert!(!json.contains("hunter2"), "{json}");
    let _ = std::fs::remove_dir_all(&folder);
}
