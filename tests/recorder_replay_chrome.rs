//! Replaying a recording against a real Chrome.
//!
//! The test starts a headless Chrome through chromedriver, so it is
//! `#[ignore]`d. Run it by itself:
//!
//! ```text
//! cargo test --test recorder_replay_chrome -- --ignored --test-threads=1
//! ```
//!
//! The recording is the kind a person's session ends in (`goto`, `type_text`,
//! `check_if_unchecked`, a link click, `goto_if_not_url`, assertions). It is
//! saved as JSON and as YAML, loaded again and replayed from each file, and the
//! page is looked at between the steps to see that they did what they say.

use std::io::{Read, Write};

use seleniumbase_rs::api::recorder::{RecordedAction, RecorderSession};
use seleniumbase_rs::{BaseCase, BrowserConfig};

const HOME: &str = r##"<!doctype html><title>Home</title>
<h1 id="title">Home page</h1>
<input id="name" type="text">
<label id="agree-label" for="agree">I agree</label><input id="agree" type="checkbox">
<select id="color"><option value="r">Red</option><option value="g">Green</option></select>
<a id="next" href="/b">Next</a>
"##;

const SECOND: &str = r##"<!doctype html><title>B</title>
<button id="done" type="button" onclick="document.getElementById('msg').textContent = 'clicked'">Done</button>
<p id="msg"></p>
"##;

/// Serves the two pages on a loopback port and returns the address.
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

fn action(name: &str, target: Option<&str>, value: Option<&str>) -> RecordedAction {
    RecordedAction::new(name, target, value)
}

/// The first half of the visit: what it does is visible on the home page.
fn on_the_home_page(base: &str) -> Vec<RecordedAction> {
    vec![
        action("goto", Some(&format!("{base}/")), None),
        action("assert_text", Some("#title"), Some("Home page")),
        action("type_text", Some("#name"), Some("Ada")),
        action("check_if_unchecked", Some("#agree"), None),
        action("select_option_by_value", Some("#color"), Some("g")),
    ]
}

/// The second half: a link, the page it leads to, and a button on it.
fn on_the_second_page(base: &str) -> Vec<RecordedAction> {
    vec![
        action("click", Some("#next"), None),
        action("goto_if_not_url", Some(&format!("{base}/b")), None),
        action("click", Some("#done"), None),
        action("assert_text", Some("#msg"), Some("clicked")),
        action("assert_title", Some("B"), None),
    ]
}

async fn headless() -> BaseCase {
    let config = BrowserConfig {
        headless: true,
        ..BrowserConfig::default()
    };
    BaseCase::new(config).await.expect("Chrome starts")
}

async fn field(sb: &BaseCase, id: &str) -> String {
    let script = format!("return document.getElementById('{id}').value");
    let value = sb.execute_script(&script).await.expect("the script runs");
    value.as_str().unwrap_or_default().to_owned()
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_saved_recording_replays_every_step_in_a_real_browser() {
    let base = serve_pages();
    let mut actions = on_the_home_page(&base);
    let home_steps = actions.len();
    actions.extend(on_the_second_page(&base));

    // Both formats read back as written, and replay the same.
    let folder = tempfile::tempdir().expect("a temporary folder");
    let session = RecorderSession::from_actions(actions);
    for file in ["recording.json", "recording.yaml"] {
        let path = session.save(&folder.path().join(file)).expect("saved");
        let loaded = RecorderSession::load(&path).expect("loaded");
        assert_eq!(loaded.len(), session.len(), "{file}");

        let mut sb = headless().await;
        let first = RecorderSession::replay(&mut sb, &loaded[..home_steps])
            .await
            .expect("the first half replays");
        // Between the halves the page shows what the steps did.
        let seen = (
            field(&sb, "name").await,
            sb.is_selected("#agree").await.expect("the box is there"),
            field(&sb, "color").await,
        );
        let second = RecorderSession::replay(&mut sb, &loaded[home_steps..])
            .await
            .expect("the second half replays");
        let url = sb.get_current_url().await.expect("a current url");
        sb.quit().await.expect("Chrome quits");

        let failures: Vec<_> = first
            .steps
            .iter()
            .chain(&second.steps)
            .filter(|step| !step.passed())
            .map(|step| format!("{}: {:?}", step.description, step.error))
            .collect();
        assert!(failures.is_empty(), "{file}: {failures:?}");
        assert_eq!(
            (first.steps.len(), first.skipped),
            (home_steps, 0),
            "{file}"
        );
        assert_eq!((second.steps.len(), second.skipped), (5, 0), "{file}");
        assert_eq!(seen, ("Ada".to_owned(), true, "g".to_owned()), "{file}");
        assert_eq!(url, format!("{base}/b"), "{file}");
    }
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_step_that_fails_is_reported_and_the_steps_after_it_still_run() {
    let base = serve_pages();
    let mut sb = headless().await;
    let actions = vec![
        action("goto", Some(&format!("{base}/")), None),
        action(
            "assert_text",
            Some("#title"),
            Some("A page that is not this one"),
        ),
        action("levitate", Some("#title"), None),
        action("type_text", Some("#name"), Some("still typed")),
    ];

    let report = RecorderSession::replay(&mut sb, &actions)
        .await
        .expect("a failed step is in the report, not an error");
    let typed = field(&sb, "name").await;
    sb.quit().await.expect("Chrome quits");

    assert!(!report.is_success());
    assert_eq!(
        (report.passed(), report.failed(), report.skipped),
        (2, 1, 1)
    );
    assert_eq!(report.skipped_names, ["levitate"]);
    assert_eq!(report.steps[1].name, "assert_text");
    assert!(report.steps[1].error.is_some());
    assert_eq!(typed, "still typed");
}
