//! Recorder Mode on pages with frames, a CAPTCHA widget, popups and redirects,
//! against a real Chrome.
//!
//! Each test is a symptom of the first real `sbrs mkrec` run on google.com: the
//! frames of a reCAPTCHA were written as `goto` steps, the tile and verify
//! clicks as steps, an empty `about:blank` frame as a page to open. A person's
//! mouse is sent with `Input.dispatchMouseEvent`, so the page script sees the
//! trusted events a real click makes, including the focus that moves into a
//! frame. Every test starts a headless Chrome through chromedriver, so they are
//! `#[ignore]`d. Run them one at a time:
//!
//! ```text
//! cargo test --test recorder_frames_chrome -- --ignored --test-threads=1
//! ```
//!
//! The pages are in `tests/fixtures/recorder_frames/` and are served on two
//! loopback ports, which are two origins; `/recaptcha/...` is a fake widget (a
//! tile grid and a verify button) whose address looks like Google's.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use seleniumbase_rs::api::recorder::RecordingTarget;
use seleniumbase_rs::{BaseCase, BrowserConfig};

const HOME: &str = include_str!("fixtures/recorder_frames/home.html");
const INNER: &str = include_str!("fixtures/recorder_frames/inner.html");
const CAPTCHA: &str = include_str!("fixtures/recorder_frames/captcha.html");
const POPUP: &str = include_str!("fixtures/recorder_frames/popup.html");
const LANDING: &str = include_str!("fixtures/recorder_frames/landing.html");

/// What a loopback server answers: a status line, an optional redirect and a body.
struct Answer {
    status: &'static str,
    location: Option<&'static str>,
    body: String,
}

impl Answer {
    fn page(body: &str) -> Self {
        Self {
            status: "200 OK",
            location: None,
            body: body.to_owned(),
        }
    }

    fn redirect(to: &'static str) -> Self {
        Self {
            status: "302 Found",
            location: Some(to),
            body: String::new(),
        }
    }
}

/// The page at `path`. `far` is the address of the other origin.
fn route(path: &str, far: &str) -> Answer {
    match path {
        "/" => Answer::page(&HOME.replace("FAR", far)),
        "/inner" => Answer::page(INNER),
        "/recaptcha/api2/anchor" | "/recaptcha/enterprise/bframe" => Answer::page(CAPTCHA),
        "/popup" => Answer::page(POPUP),
        "/landing" => Answer::page(LANDING),
        "/help" => Answer::page("<!doctype html><title>Help</title><h1 id=\"help\">Help</h1>"),
        "/redirect" => Answer::redirect("/redirect2"),
        "/redirect2" => Answer::redirect("/landing"),
        _ => Answer {
            status: "404 Not Found",
            location: None,
            body: String::new(),
        },
    }
}

fn answer(mut stream: TcpStream, far: &str) {
    let mut buffer = [0_u8; 4096];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let target = request.split_whitespace().nth(1).unwrap_or("/");
    let path = target.split('?').next().unwrap_or("/");
    let Answer {
        status,
        location,
        body,
    } = route(path, far);
    let location = location.map_or_else(String::new, |to| format!("Location: {to}\r\n"));
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html\r\n{location}Content-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Serves the routes on `listener`, a thread to each connection (Chrome opens
/// sockets it never writes to).
fn serve(listener: TcpListener, far: String) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let far = far.clone();
            std::thread::spawn(move || answer(stream, &far));
        }
    });
}

/// The addresses of the two origins: the home site and the far one.
fn sites() -> (String, String) {
    let home = TcpListener::bind("127.0.0.1:0").unwrap();
    let far = TcpListener::bind("127.0.0.1:0").unwrap();
    let home_url = format!("http://{}", home.local_addr().unwrap());
    let far_url = format!("http://{}", far.local_addr().unwrap());
    serve(home, far_url.clone());
    serve(far, home_url.clone());
    (home_url, far_url)
}

fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "sbrs-recorder-frames-{}-{name}",
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
    let sb = BaseCase::new(config).await.expect("Chrome starts");
    sb.set_window_size(1280, 1000).await.unwrap();
    let mut sb = sb;
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

/// Opens the home page and lets its frames load and the page script run in them.
async fn open_home(sb: &mut BaseCase, home: &str) {
    sb.open(&format!("{home}/")).await.unwrap();
    sb.sleep(1.0).await;
}

// The mouse is moved by WebDriver actions, which send the trusted events of a real pointer.

/// Moves the mouse to a point inside the element `css` and clicks, as a person
/// does: the pointer crosses into a frame and the focus follows the click.
async fn click_inside(sb: &mut BaseCase, css: &str, dx: f64, dy: f64) {
    sb.click_with_offset(css, dx, dy).await.unwrap();
    sb.sleep(0.4).await;
}

/// What the browser keeps looking at while the person works, for `seconds`.
async fn watch(sb: &mut BaseCase, seconds: u64) {
    sb.wait_for_recorder_end(Duration::from_secs(seconds))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn the_frames_of_a_page_are_not_written_as_pages_to_open() {
    let (home, _) = sites();
    let folder = folder("frames-load");
    let mut sb = recording(&folder).await;

    open_home(&mut sb, &home).await;
    watch(&mut sb, 2).await;
    sb.quit().await.unwrap();

    let code = written(&folder);
    assert_eq!(
        code.matches("sb.goto(").count(),
        1,
        "only the page itself is opened: {code}"
    );
    assert!(code.contains(&format!("sb.goto(\"{home}/\")")), "{code}");
    for frame in ["/inner", "/recaptcha/", "about:"] {
        assert!(
            !code.contains(frame),
            "{frame} is a frame, not a page: {code}"
        );
    }
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_click_in_a_captcha_frame_is_one_comment_and_not_a_step() {
    let (home, _) = sites();
    let folder = folder("captcha");
    let mut sb = recording(&folder).await;

    open_home(&mut sb, &home).await;
    // A tile of the grid, then the verify button below it.
    click_inside(&mut sb, "iframe[title=\"reCAPTCHA\"]", 30.0, 20.0).await;
    click_inside(&mut sb, "iframe[title=\"reCAPTCHA\"]", 100.0, 100.0).await;
    watch(&mut sb, 2).await;
    let tabs = sb.get_all_window_handles().await.unwrap().len();
    sb.quit().await.unwrap();

    assert_eq!(tabs, 1, "the widget is not opened in a tab of its own");
    let code = written(&folder);
    assert_eq!(code.matches("solve_captcha").count(), 1, "{code}");
    for step in ["tile", "verify-button", "switch_to_frame", "recaptcha/api2"] {
        assert!(!code.contains(step), "{step} is not a step: {code}");
    }
    assert_eq!(code.matches("sb.goto(").count(), 1, "{code}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_click_in_a_captcha_frame_of_another_origin_is_one_comment() {
    let (home, _) = sites();
    let folder = folder("captcha-far");
    let mut sb = recording(&folder).await;

    open_home(&mut sb, &home).await;
    click_inside(&mut sb, "iframe[title=\"hCaptcha\"]", 30.0, 20.0).await;
    click_inside(&mut sb, "iframe[title=\"hCaptcha\"]", 100.0, 100.0).await;
    watch(&mut sb, 2).await;
    let tabs = sb.get_all_window_handles().await.unwrap().len();
    sb.quit().await.unwrap();

    assert_eq!(tabs, 1, "the widget is not opened in a tab of its own");
    let code = written(&folder);
    assert_eq!(code.matches("solve_captcha").count(), 1, "{code}");
    for step in [
        "tile",
        "verify-button",
        "switch_to_frame",
        "recaptcha/enterprise",
    ] {
        assert!(!code.contains(step), "{step} is not a step: {code}");
    }
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn an_empty_about_blank_frame_is_never_a_step_or_a_tab() {
    let (home, _) = sites();
    let folder = folder("blank");
    let mut sb = recording(&folder).await;

    open_home(&mut sb, &home).await;
    click_inside(&mut sb, "iframe#blank", 40.0, 20.0).await;
    watch(&mut sb, 2).await;
    let tabs = sb.get_all_window_handles().await.unwrap().len();
    sb.quit().await.unwrap();

    assert_eq!(tabs, 1, "about:blank is not opened in a tab");
    let code = written(&folder);
    assert!(!code.contains("about:"), "{code}");
    assert_eq!(code.matches("sb.goto(").count(), 1, "{code}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_person_who_acts_in_an_ordinary_frame_gets_a_switch_to_it() {
    let (home, _) = sites();
    let folder = folder("frame-act");
    let mut sb = recording(&folder).await;

    open_home(&mut sb, &home).await;
    click_inside(&mut sb, "iframe#same", 40.0, 20.0).await;
    watch(&mut sb, 2).await;
    sb.quit().await.unwrap();

    let code = written(&folder);
    assert!(
        code.contains("sb.switch_to_frame(\"iframe#same\")"),
        "{code}"
    );
    assert_eq!(
        code.matches("sb.goto(").count(),
        1,
        "the frame's page is not a step of its own: {code}"
    );
    let _ = std::fs::remove_dir_all(&folder);
}

/// Whether `needles` appear in `code` in this order.
fn in_order(code: &str, needles: &[&str]) -> bool {
    let mut from = 0;
    for needle in needles {
        match code[from..].find(needle) {
            Some(at) => from += at + needle.len(),
            None => return false,
        }
    }
    true
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn selectors_prefer_an_id_a_name_a_test_id_a_label_and_a_text_before_a_path() {
    let (home, _) = sites();
    let folder = folder("selectors");
    let mut sb = recording(&folder).await;

    open_home(&mut sb, &home).await;
    sb.click("#email-field").await.unwrap();
    sb.type_text("input[name=q]", "hello").await.unwrap();
    sb.click("[data-testid=search-go]").await.unwrap();
    sb.click("button.primary").await.unwrap();
    sb.click("span.plain").await.unwrap();
    sb.click("ul.swatches li:nth-child(2) div").await.unwrap();
    sb.click("a[aria-label=\"Open the help page\"]")
        .await
        .unwrap();
    watch(&mut sb, 1).await;
    sb.quit().await.unwrap();

    // What Python's `sbase mkrec` wrote for the same clicks on the same page
    // (SeleniumBase 4.56.2, run headless on these fixtures).
    let code = written(&folder);
    assert!(
        in_order(
            &code,
            &[
                "sb.click(\"input#email-field\")",
                "sb.type_text(\"input[name=\\\"q\\\"]\", \"hello\")",
                "sb.click(\"button[data-testid=\\\"search-go\\\"]\")",
                "sb.click(\"button:contains(\\\"Save changes\\\")\")",
                "sb.click(\"span:contains(\\\"Just some text\\\")\")",
                "sb.click(\"body ul li:nth-of-type(2) div\")",
                "sb.click(\"a[aria-label=\\\"Open the help page\\\"]\")",
            ]
        ),
        "{code}"
    );
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_302_chain_is_one_click_and_the_page_it_ended_on() {
    let (home, _) = sites();
    let folder = folder("redirect");
    let mut sb = recording(&folder).await;

    open_home(&mut sb, &home).await;
    sb.click("#redirect-link").await.unwrap();
    watch(&mut sb, 2).await;
    sb.quit().await.unwrap();

    let code = written(&folder);
    assert!(
        in_order(&code, &["sb.click(\"a#redirect-link\")", "/landing\")"]),
        "{code}"
    );
    assert!(!code.contains("/redirect2"), "{code}");
    assert!(!code.contains(&format!("goto(\"{home}/redirect")), "{code}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_link_that_opens_a_tab_is_a_click_and_then_the_page_it_opened() {
    let (home, far) = sites();
    let folder = folder("popup-link");
    let mut sb = recording(&folder).await;

    open_home(&mut sb, &home).await;
    sb.click("#popup-link").await.unwrap();
    watch(&mut sb, 3).await;
    let tabs = sb.get_all_window_handles().await.unwrap().len();
    sb.quit().await.unwrap();

    // Python writes no tab switch: the pages of every tab are one list.
    assert_eq!(tabs, 2);
    let code = written(&folder);
    assert!(
        in_order(
            &code,
            &["sb.click(\"a#popup-link\")", &format!("{far}/popup\")")]
        ),
        "{code}"
    );
    assert!(!code.contains("switch_to_"), "{code}");
    let _ = std::fs::remove_dir_all(&folder);
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn a_window_open_is_a_click_and_then_the_page_it_opened() {
    let (home, _) = sites();
    let folder = folder("popup-js");
    let mut sb = recording(&folder).await;

    open_home(&mut sb, &home).await;
    sb.click("#popup-js").await.unwrap();
    watch(&mut sb, 3).await;
    sb.quit().await.unwrap();

    let code = written(&folder);
    assert!(
        in_order(
            &code,
            &["sb.click(\"button#popup-js\")", &format!("{home}/popup\")")]
        ),
        "{code}"
    );
    let _ = std::fs::remove_dir_all(&folder);
}
