//! The Pure CDP behaviour the parity audit of `parity/api.toml` fixed, against a
//! real Chrome.
//!
//! These tests need a Chrome or Chromium on the machine, so they are ignored by
//! default. Run them one test binary at a time with:
//!
//! ```text
//! cargo test --test sb_cdp_parity_chrome -- --ignored
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page, Permission};
use seleniumbase_rs::utils::selectors::SelectorBuf;
use serde_json::json;

const FIXTURE: &str = r##"<!doctype html>
<html><head><title>Parity</title></head>
<body style="height:3000px">
<h1 id="title">Welcome back</h1>
<a href="#one">Sign in</a> <a href="#two">Sign in help</a>
<button id="go" type="button">Go on</button>
<span class="note">Plain   note
  text</span>
<input id="user" name="user">
<button id="hold" type="button"
  onmousedown="window.downAt=performance.now()"
  onmouseup="window.holdMs=performance.now()-window.downAt">hold</button>
<div id="lower" style="margin-top:1200px">lower</div>
<script>
  window.marker = 1;
  setTimeout(() => {
    const late = document.createElement('div');
    late.className = 'late'; late.textContent = 'arrived';
    document.body.insertBefore(late, document.getElementById('lower'));
  }, 600);
</script></body></html>"##;

/// Serves the fixture on a loopback port and remembers the `Accept-Language`
/// the browser sent with its last request.
fn serve() -> (String, Arc<Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    let language = Arc::new(Mutex::new(String::new()));
    let seen = Arc::clone(&language);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            if let Some(line) = request
                .lines()
                .find(|line| line.to_ascii_lowercase().starts_with("accept-language:"))
            {
                *seen.lock().unwrap() = line["accept-language:".len()..].trim().to_owned();
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{FIXTURE}",
                FIXTURE.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (format!("http://{address}"), language)
}

async fn open() -> (Browser, Page, String, Arc<Mutex<String>>) {
    let (base, language) = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1000, 800)
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.unwrap();
    page.goto(format!("{base}/")).await.unwrap();
    (
        browser,
        page.with_timeout(Duration::from_secs(3)),
        base,
        language,
    )
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn text_selectors_find_any_element_and_contains_works_as_css() {
    let (browser, page, ..) = open().await;

    assert_eq!(
        page.locator("button:contains(\"Go on\")")
            .count()
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        page.locator("a:contains(\"Sign in\")")
            .count()
            .await
            .unwrap(),
        2
    );
    let links = SelectorBuf::containing_text("Sign in", Some("a"));
    assert_eq!(page.locator(links).count().await.unwrap(), 2);
    let help = SelectorBuf::containing_text("Sign in help", Some("a"));
    assert_eq!(page.locator(help).count().await.unwrap(), 1);

    // Without a tag only the innermost element that holds the text matches, so
    // <html> and <body> do not.
    let heading = page.locator(SelectorBuf::containing_text("Welcome", None));
    assert_eq!(heading.count().await.unwrap(), 1);
    assert_eq!(heading.info().await.unwrap().tag, "h1");

    // White space in the page and in the text counts as one space.
    let note = SelectorBuf::containing_text("Plain note text", Some("span"));
    assert_eq!(page.locator(note).count().await.unwrap(), 1);

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn evaluate_drops_the_return_of_the_last_line_like_python() {
    let (browser, page, ..) = open().await;

    assert_eq!(
        page.evaluate("return document.title").await.unwrap(),
        "Parity"
    );
    assert_eq!(
        page.evaluate("const a = 2;\n  return a * 3;\n")
            .await
            .unwrap(),
        6
    );

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn goto_if_not_url_only_navigates_to_another_page() {
    let (browser, page, base, _) = open().await;
    // A reload would put the fixture's own value (1) back.
    page.evaluate("window.marker = 2").await.unwrap();

    page.goto_if_not_url(format!("{base}/")).await.unwrap();
    assert_eq!(
        page.evaluate("window.marker").await.unwrap(),
        2,
        "the page was reloaded although it was already there"
    );

    page.goto_if_not_url(format!("{base}/elsewhere"))
        .await
        .unwrap();
    assert_eq!(
        page.evaluate("window.marker").await.unwrap(),
        1,
        "the other address is a new page"
    );
    assert!(page.url().await.unwrap().ends_with("/elsewhere"));

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_active_element_is_named_and_found_again() {
    let (browser, page, ..) = open().await;

    page.locator("#user").focus().await.unwrap();

    assert_eq!(page.active_element_css().await.unwrap(), "input#user");
    let active = page.active_element().await.unwrap();
    assert_eq!(
        active.attribute("name").await.unwrap().as_deref(),
        Some("user")
    );

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_viewport_rect_ignores_how_far_the_page_is_scrolled() {
    let (browser, page, ..) = open().await;
    let button = page.locator("#go");
    let at_top = button.viewport_rect().await.unwrap();

    page.evaluate("window.scrollTo(0, 400)").await.unwrap();
    let scrolled = button.viewport_rect().await.unwrap();
    let document = button.bounding_box().await.unwrap();

    assert!(
        (at_top.y - scrolled.y - 400.0).abs() < 1.0,
        "{at_top:?} -> {scrolled:?}"
    );
    assert!(
        (document.y - at_top.y).abs() < 1.0,
        "the document rect does not move with the scroll: {document:?}"
    );
    assert!((scrolled.width - at_top.width).abs() < 0.01);

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn click_and_hold_keeps_the_button_down_for_the_time_asked() {
    let (browser, page, ..) = open().await;

    page.locator("#hold")
        .click_and_hold(Duration::from_millis(400))
        .await
        .unwrap();

    let held = page.evaluate("window.holdMs").await.unwrap();
    assert!(
        held.as_f64().unwrap() >= 350.0,
        "the page saw the button held for {held} ms"
    );
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn all_waiting_waits_for_a_list_that_renders_after_the_page_loads() {
    let (browser, page, ..) = open().await;

    let found = page.locator(".late").all_waiting().await.unwrap();

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].text().await.unwrap(), "arrived");
    let none = page
        .with_timeout(Duration::from_millis(300))
        .locator(".nothing")
        .all_waiting()
        .await
        .unwrap();
    assert!(none.is_empty());

    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn flash_with_outlines_the_element_in_the_colour_asked() {
    let (browser, page, ..) = open().await;

    page.locator("#title")
        .flash_with("#44CC88", Duration::from_secs(5))
        .await
        .unwrap();

    let outline = page
        .evaluate("document.getElementById('title').style.outline")
        .await
        .unwrap();
    assert!(
        outline.as_str().unwrap().contains("rgb(68, 204, 136)"),
        "{outline}"
    );
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn chrome_grants_every_permission_python_grants_and_one_for_an_origin() {
    let (browser, page, base, _) = open().await;

    browser.grant_all_permissions().await.unwrap();
    browser.reset_permissions().await.unwrap();
    browser
        .grant_permissions_for(&base, &[Permission::Geolocation])
        .await
        .unwrap();

    let state = page
        .evaluate("navigator.permissions.query({ name: 'geolocation' }).then(s => s.state)")
        .await
        .unwrap();
    assert_eq!(state, "granted");
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn setting_the_locale_changes_the_language_the_page_and_the_server_see() {
    let (browser, page, base, language) = open().await;

    page.emulation().locale("de-DE").await.unwrap();
    page.goto(format!("{base}/")).await.unwrap();

    assert_eq!(page.evaluate("navigator.language").await.unwrap(), "de-DE");
    assert_eq!(
        page.evaluate("new Intl.DateTimeFormat().resolvedOptions().locale")
            .await
            .unwrap(),
        "de-DE"
    );
    assert!(
        language.lock().unwrap().starts_with("de-DE"),
        "Accept-Language was {:?}",
        language.lock().unwrap()
    );
    // The client hints the page had are still there.
    let brands = page
        .evaluate("navigator.userAgentData ? navigator.userAgentData.brands.length : -1")
        .await
        .unwrap();
    assert!(brands.as_i64().unwrap() > 0, "brands: {brands}");
    assert_eq!(
        page.evaluate("navigator.userAgent.includes('Chrome')")
            .await
            .unwrap(),
        json!(true)
    );
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_browser_attached_by_name_reports_the_host_it_was_reached_on() {
    let (browser, _page, ..) = open().await;
    let port = browser.debugging_port();

    assert_eq!(browser.debugging_host(), "127.0.0.1");
    let attached = Browser::connect("localhost", port).await.unwrap();

    assert_eq!(attached.debugging_host(), "localhost");
    assert_eq!(attached.http_url(), format!("http://localhost:{port}"));
    assert_eq!(attached.debugging_port(), port);
    let started = Instant::now();
    assert!(!attached.list_pages().await.unwrap().is_empty());
    assert!(started.elapsed() < Duration::from_secs(5));

    attached.close().await.unwrap();
    browser.close().await.unwrap();
}
