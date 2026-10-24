//! Real-browser checks of the website tours: `create_tour`, `add_tour_step`,
//! `play_tour` and `export_tour`. The same calls are checked against a mock
//! WebDriver in `src/api/base_case_impls/base_case_impl_tours.rs`, and the
//! scripts in `src/api/tour.rs`.
//!
//! A tour waits for a person, so the tests either make it play by itself (an
//! `interval`) or play the person with a script that runs in the page and
//! clicks the buttons. The browser is headless, where `play_tour` returns at
//! once without showing anything, so the case is told that somebody watches
//! (`BaseCase::with_somebody_watching`, in the `test-util` feature).
//!
//! These tests are ignored by default; they need Chrome and a network that can
//! reach the CDNs the tour libraries come from (cdnjs and jsDelivr). Run them
//! locally with:
//!
//! ```bash
//! cargo test --test tours_chrome --features test-util -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

#![cfg(feature = "test-util")]

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use seleniumbase_rs::api::tour::ShepherdTheme;
use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions, and the changes of the working directory, so
/// parallel tests do not race.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A page with two elements to point at.
const PAGE: &str = "<html><head><title>tour</title></head><body><h1 id='one'>One</h1><p id='two'>Two</p></body></html>";

/// Serves `PAGE` on a loopback port and returns its address. A `data:` page
/// will not do: Hopscotch keeps its state in a cookie, and Chrome has none
/// for a `data:` URL.
fn serve_page() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    // The line break of an HTTP header.
    let crlf = String::from_utf8(vec![13, 10]).expect("ASCII");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);
            let head = [
                "HTTP/1.1 200 OK".to_owned(),
                "Content-Type: text/html".to_owned(),
                format!("Content-Length: {}", PAGE.len()),
                "Connection: close".to_owned(),
                String::new(),
                PAGE.to_owned(),
            ];
            let _ = stream.write_all(head.join(&crlf).as_bytes());
        }
    });
    format!("http://{address}/")
}

/// The person of an IntroJS tour: every 400 ms it reads the step on show and
/// presses Next (Done on the last step).
const PERSON: &str = "window.__tourSeen = []; \
    setInterval(function () { \
        var text = document.querySelector('.introjs-tooltiptext'); \
        var button = document.querySelector('.introjs-nextbutton'); \
        if (!text || !button) { return; } \
        window.__tourSeen.push(text.textContent); \
        button.click(); \
    }, 400); \
    return true;";

fn test_config() -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    };
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

/// A case in a headless browser that behaves as if somebody watches, on a page.
async fn watched_case() -> BaseCase {
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser")
        .with_somebody_watching();
    // A tour nobody finishes waits for ever; the time limit ends it.
    sb.set_time_limit(120).expect("set the limit");
    sb.open(&serve_page()).await.expect("open the page");
    sb
}

/// A temporary folder the process works in, so that the exported files do not
/// land in the crate. The working directory goes back when it is dropped, so
/// that the next test can start a driver from a folder that exists.
struct WorkDir {
    dir: tempfile::TempDir,
    previous: std::path::PathBuf,
}

impl WorkDir {
    fn path(&self) -> &std::path::Path {
        self.dir.path()
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

/// Moves the process into a new folder. The caller keeps the guard alive for
/// the whole test.
fn work_in_a_temporary_folder() -> WorkDir {
    let previous = std::env::current_dir().expect("a working directory");
    let dir = tempfile::tempdir().expect("a temporary folder");
    std::env::set_current_dir(dir.path()).expect("move into it");
    WorkDir { dir, previous }
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to the CDNs"]
async fn a_two_step_shepherd_tour_with_an_interval_ends_by_itself() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;
    sb.create_shepherd_tour()
        .theme(ShepherdTheme::Dark)
        .await
        .unwrap();
    sb.add_tour_step("The <b>heading</b>")
        .selector("#one")
        .title("One")
        .await
        .unwrap();
    sb.add_tour_step("The paragraph")
        .selector("#two")
        .await
        .unwrap();

    let started = Instant::now();
    sb.play_tour()
        .interval(Duration::from_millis(500))
        .await
        .expect("the tour plays to its end");
    let played = started.elapsed();

    // Each of the two steps is shown for the interval; the library loads
    // first, so this is a floor and not an exact time.
    assert!(
        played >= Duration::from_millis(1000),
        "played for {played:?}"
    );
    assert!(played < Duration::from_secs(90), "played for {played:?}");
    let over = sb
        .execute_script("return !Shepherd.activeTour;")
        .await
        .unwrap();
    assert_eq!(over, serde_json::json!(true), "no tour is active any more");
    let shown = sb
        .is_element_visible(".shepherd-step")
        .await
        .unwrap_or(false);
    assert!(!shown, "no step is left on show");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to the CDNs"]
async fn an_introjs_tour_shows_its_steps_to_a_person_and_play_tour_returns_at_the_end() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;
    sb.set_introjs_colors(Some("#aa3300"), None).unwrap();
    // No theme: IntroJS, as in Python.
    sb.create_tour().await.unwrap();
    sb.add_tour_step("The <b>heading</b>")
        .selector("#one")
        .title("One")
        .await
        .unwrap();
    sb.add_tour_step("The paragraph")
        .selector("#two")
        .await
        .unwrap();
    sb.execute_script(PERSON)
        .await
        .expect("the person is ready");

    sb.play_tour().await.expect("the tour plays to its end");

    let seen = sb
        .execute_script("return window.__tourSeen;")
        .await
        .unwrap();
    let seen: Vec<String> = serde_json::from_value(seen).unwrap();
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert!(
        seen[0].contains("One") && seen[0].contains("The heading"),
        "{seen:?}"
    );
    assert!(seen[1].contains("The paragraph"), "{seen:?}");
    // The colours were applied to the buttons.
    let styled = sb
        .execute_script(
            "return Array.from(document.querySelectorAll('style')) \
             .some(function (s) { return s.textContent.indexOf('#aa3300') >= 0; });",
        )
        .await
        .unwrap();
    assert_eq!(styled, serde_json::json!(true));
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to the CDNs"]
async fn an_exported_tour_plays_when_it_is_run_in_the_console_of_a_page() {
    let _guard = BROWSER_LOCK.lock().await;
    let dir = work_in_a_temporary_folder();
    let mut sb = watched_case().await;
    sb.create_introjs_tour().await.unwrap();
    sb.add_tour_step("The exported heading")
        .selector("#one")
        .await
        .unwrap();

    let path = sb
        .export_tour()
        .filename("exported.js")
        .await
        .expect("the tour is exported");
    assert!(
        path.ends_with("tours_exported/exported.js"),
        "{}",
        path.display()
    );
    assert!(dir.path().join("tours_exported/exported.js").is_file());
    let script = std::fs::read_to_string(&path).unwrap();

    // The console runs a script in the global scope; so does an indirect eval.
    let console = format!(
        "(0, eval)({}); return true;",
        serde_json::to_string(&script).unwrap()
    );
    sb.execute_script(&console).await.expect("the script runs");

    let started = Instant::now();
    let mut shown = false;
    while started.elapsed() < Duration::from_secs(30) {
        if sb
            .is_element_visible(".introjs-tooltip")
            .await
            .unwrap_or(false)
        {
            shown = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    assert!(shown, "the exported tour showed its first step");
    let text = sb
        .execute_script("return document.querySelector('.introjs-tooltiptext').textContent;")
        .await
        .unwrap();
    assert!(
        text.as_str()
            .unwrap_or_default()
            .contains("The exported heading"),
        "{text}"
    );
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and network access to the CDNs"]
async fn bootstrap_driverjs_and_hopscotch_tours_end_by_themselves_with_an_interval() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = watched_case().await;
    sb.create_bootstrap_tour().name("bootstrap").await.unwrap();
    sb.create_driverjs_tour().name("driverjs").await.unwrap();
    sb.create_hopscotch_tour().name("hopscotch").await.unwrap();
    for name in ["bootstrap", "driverjs", "hopscotch"] {
        sb.add_tour_step("The heading")
            .name(name)
            .selector("#one")
            .title("One")
            .await
            .unwrap();
        sb.add_tour_step("The paragraph")
            .name(name)
            .selector("#two")
            .await
            .unwrap();
    }
    for name in ["bootstrap", "driverjs", "hopscotch"] {
        let started = Instant::now();
        sb.play_tour()
            .name(name)
            .interval(Duration::from_secs(1))
            .await
            .unwrap_or_else(|error| panic!("the {name} tour plays to its end: {error}"));
        let played = started.elapsed();
        assert!(
            played >= Duration::from_secs(1),
            "{name} played for {played:?}"
        );
        assert!(
            played < Duration::from_secs(90),
            "{name} played for {played:?}"
        );
    }
    sb.quit().await.expect("quit");
}
