//! A `Locator` reaching into frames of the pure CDP engine, against a real
//! headless Chrome.
//!
//! Python's `nested_click(parent, selector)` clicks inside an iframe whatever
//! its origin; here that is `page.locator(parent).locator(selector).click()`.
//! A script on the page can read into a frame of its own origin only, so a
//! frame whose content a script cannot reach has to be entered through the
//! protocol, one execution context per frame.
//!
//! Two loopback servers serve `tests/fixtures/frames`. The page is on
//! `127.0.0.1:first` and holds a frame of its own origin, a `data:` frame, an
//! `srcdoc` frame, a frame of `127.0.0.1:second` (another origin, the same
//! site, so the same process), a sandboxed frame (an opaque origin, the same
//! process) and, for another site, `localhost:second`, which Chrome runs in a
//! process of its own (an out-of-process frame), twice: once with a frame of
//! the page's site inside it. Each frame has a button, an input and a log.
//!
//! The tests need a Chrome or Chromium and are ignored by default. Run the
//! binary on its own:
//!
//! ```text
//! cargo test --features "mcp-server test-util testing" --test sb_cdp_frames_chrome \
//!     -- --ignored --test-threads=1
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use seleniumbase_rs::SeleniumBaseError;

const INDEX: &str = include_str!("fixtures/frames/index.html");
const FRAME: &str = include_str!("fixtures/frames/frame.html");
const OUTER: &str = include_str!("fixtures/frames/outer.html");

/// The ports of the two loopback servers.
struct Site {
    first: u16,
    second: u16,
}

/// `html` as the value of an HTML attribute.
fn escape_attribute(html: &str) -> String {
    html.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

impl Site {
    /// The page with the frames: the `data:` frame carries the frame page
    /// itself, and so does the `srcdoc` one.
    fn index(&self) -> String {
        let data = base64::engine::general_purpose::STANDARD.encode(FRAME.replace("{WHO}", "data"));
        let srcdoc = escape_attribute(&FRAME.replace("{WHO}", "srcdoc"));
        INDEX
            .replace("{DATA}", &data)
            .replace("{SRCDOC}", &srcdoc)
            .replace("{P2}", &self.second.to_string())
    }

    /// What `path` answers, for a request to `host`: a frame says which host
    /// it was served for.
    fn body(&self, path: &str, host: &str) -> Option<String> {
        match path {
            "/" => Some(self.index()),
            "/frame.html" => Some(FRAME.replace("{WHO}", host)),
            "/outer.html" => Some(
                OUTER
                    .replace("{WHO}", host)
                    .replace("{P1}", &self.first.to_string()),
            ),
            _ => None,
        }
    }
}

/// Answers one connection.
fn answer(mut stream: TcpStream, site: &Site) {
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or("/");
    let host = request
        .lines()
        .find_map(|line| line.strip_prefix("Host: "))
        .unwrap_or("?")
        .trim()
        .to_owned();
    let (status, body) = match site.body(path, &host) {
        Some(body) => ("200 OK", body),
        None => ("404 Not Found", "no such page".to_owned()),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Starts both servers.
fn serve() -> Arc<Site> {
    let first = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let second = TcpListener::bind("127.0.0.1:0").expect("bind a second loopback port");
    let site = Arc::new(Site {
        first: first.local_addr().expect("bound address").port(),
        second: second.local_addr().expect("bound address").port(),
    });
    for listener in [first, second] {
        let site = Arc::clone(&site);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let site = Arc::clone(&site);
                // A thread each: Chrome opens idle connections ahead of time.
                std::thread::spawn(move || answer(stream, &site));
            }
        });
    }
    site
}

/// A headless Chrome on the page with the frames.
async fn open() -> (Browser, Page, Arc<Site>) {
    let site = serve();
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .expect("the launch options are valid");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser
        .default_page()
        .await
        .expect("the browser has a page")
        .with_timeout(Duration::from_secs(8));
    page.goto(format!("http://127.0.0.1:{}/", site.first))
        .await
        .expect("the page loads");
    (browser, page, site)
}

/// Everything a script can ask of an element, done inside the frame
/// `frame`, which says `who` in its `#who`.
async fn works_inside(page: &Page, frame: &str, who: &str) {
    let inside = |css: &str| page.locator(frame).locator(css);

    assert_eq!(inside("#who").text().await.unwrap(), who, "{frame}");
    assert!(inside("#btn").is_visible().await.unwrap(), "{frame}");
    assert_eq!(inside("#btn").count().await.unwrap(), 1, "{frame}");
    assert!(inside("#btn").exists().await.unwrap(), "{frame}");
    assert_eq!(
        inside("#btn").info().await.unwrap().tag,
        "button",
        "{frame}"
    );

    // A real click, twice: the frame's own script logs each one.
    inside("#btn").click().await.unwrap();
    inside("#btn").click().await.unwrap();
    assert_eq!(
        inside("#log").text().await.unwrap(),
        "clicked;clicked;",
        "{frame}"
    );

    // Typing and filling reach the field of the frame.
    inside("#field")
        .type_text("typed in a frame")
        .await
        .unwrap();
    assert_eq!(
        inside("#field").text().await.unwrap(),
        "typed in a frame",
        "{frame}"
    );
    inside("#field").fill("filled").await.unwrap();
    assert_eq!(inside("#field").text().await.unwrap(), "filled", "{frame}");
    inside("#field").send_keys("!").await.unwrap();
    assert_eq!(inside("#field").text().await.unwrap(), "filled!", "{frame}");

    // What is not there is not found, and says so in time.
    assert!(!inside("#nope").exists().await.unwrap(), "{frame}");
    assert_eq!(inside("#nope").count().await.unwrap(), 0, "{frame}");
    let missing = inside("#nope")
        .with_timeout(Duration::from_millis(300))
        .click()
        .await
        .unwrap_err();
    assert!(
        matches!(missing, SeleniumBaseError::ElementNotFound { .. }),
        "{frame}: {missing:?}"
    );

    inside("#btn").expect().to_be_visible().await.unwrap();
    inside("#log")
        .expect()
        .to_have_text("clicked;clicked;")
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn a_frame_of_the_pages_own_origin_is_entered() {
    let (browser, page, site) = open().await;
    works_inside(&page, "iframe#same", &format!("127.0.0.1:{}", site.first)).await;
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn a_data_url_frame_is_entered() {
    let (browser, page, _) = open().await;
    works_inside(&page, "iframe#data", "data").await;
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn an_srcdoc_frame_is_entered() {
    let (browser, page, _) = open().await;
    works_inside(&page, "iframe#srcdoc", "srcdoc").await;
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn a_frame_of_another_origin_in_the_same_site_is_entered() {
    let (browser, page, site) = open().await;
    works_inside(&page, "iframe#cross", &format!("127.0.0.1:{}", site.second)).await;
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn a_sandboxed_frame_is_entered() {
    let (browser, page, site) = open().await;
    works_inside(
        &page,
        "iframe#sandboxed",
        &format!("127.0.0.1:{}", site.first),
    )
    .await;
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn a_frame_of_another_site_in_a_process_of_its_own_is_entered() {
    let (browser, page, site) = open().await;
    works_inside(&page, "iframe#oopif", &format!("localhost:{}", site.second)).await;
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn a_frame_inside_a_frame_of_another_site_is_entered() {
    let (browser, page, site) = open().await;
    let who = format!("127.0.0.1:{}", site.first);
    let nested = page.locator("iframe#nested").locator("iframe#inner");
    assert_eq!(
        page.locator("iframe#nested")
            .locator("#who")
            .text()
            .await
            .unwrap(),
        format!("localhost:{}", site.second)
    );
    assert_eq!(nested.locator("#who").text().await.unwrap(), who);
    nested.locator("#btn").click().await.unwrap();
    assert_eq!(nested.locator("#log").text().await.unwrap(), "clicked;");
    nested.locator("#field").type_text("deep").await.unwrap();
    assert_eq!(nested.locator("#field").text().await.unwrap(), "deep");
    // The frame between the page and the inner one is untouched.
    assert!(!page
        .locator("iframe#nested")
        .locator("#log")
        .exists()
        .await
        .unwrap());
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn frames_are_found_by_their_parent_only_when_the_parent_matches() {
    let (browser, page, _) = open().await;
    // No such frame: not found, whatever is asked inside it.
    let nowhere = page
        .locator("iframe#nowhere")
        .locator("#btn")
        .with_timeout(Duration::from_millis(300));
    assert!(!nowhere.exists().await.unwrap());
    assert!(matches!(
        nowhere.click().await.unwrap_err(),
        SeleniumBaseError::ElementNotFound { .. }
    ));
    // The frames reached with one locator each, the way Python reads
    // `nested_click("iframe#myFrame3", ".fBox")`.
    for frame in ["iframe#data", "iframe#oopif"] {
        page.locator(frame).locator("#btn").click().await.unwrap();
        assert_eq!(
            page.locator(frame).locator("#log").text().await.unwrap(),
            "clicked;"
        );
    }
    browser.close().await.unwrap();
}
