//! A real-browser check that `BrowserConfig::ad_block` blocks hosts in a
//! WebDriver session.
//!
//! A page on a loopback server loads three images whose host names Chrome is
//! told to resolve to that same server (`--host-resolver-rules`): one from a
//! host in the block list, one from a subdomain of it, one from a host that is
//! not listed. With `ad_block` on, the requests for the two listed images must
//! never reach the server, and the unlisted one must. A second session with
//! `ad_block` off is the control: the same page then fetches all of them, so the
//! server would have seen the blocked requests had they been made.
//!
//! The page also holds three frames and an image from the listed host at a port
//! of its own, to show how far the block reaches. A frame of the page's own site
//! that loads an image from the listed host is covered: the request never
//! arrives. A frame addressed to the listed host, a frame of another site that
//! loads an image from the listed host, and the image at a port, are not: Chrome
//! 155 loads them. The test expects those requests to arrive. They are limits,
//! not goals; if Chrome ever blocks them, change the test and the docs of
//! `BrowserConfig::ad_block` together.
//!
//! This test is ignored by default. Run it locally with Chrome installed:
//!
//! ```bash
//! cargo test --test ad_block_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a chromedriver you started instead of the one
//! `BaseCase` starts (and finds in `downloaded_drivers/`).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use seleniumbase_rs::config::ad_block_list::AdBlockList;
use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A host that is in the block list.
const BLOCKED: &str = "blocked-ads.test";
/// A subdomain of it, which the list blocks too.
const BLOCKED_SUBDOMAIN: &str = "cdn.blocked-ads.test";
/// A host that is not in the list.
const CLEAN: &str = "clean-assets.test";

const IMAGE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="3" height="3"><rect width="3" height="3"/></svg>"#;

/// The page, served at `/`: one image from each host, one from the listed host
/// at a port (the resolver rules send any port to the server), a frame at the
/// listed host, a frame at the unlisted one, and a frame of the page's own site.
fn page() -> String {
    format!(
        "<!doctype html><html><head><title>ads</title></head><body>\
         <img id=\"blocked\" src=\"http://{BLOCKED}/blocked.svg\">\
         <img id=\"subdomain\" src=\"http://{BLOCKED_SUBDOMAIN}/subdomain.svg\">\
         <img id=\"clean\" src=\"http://{CLEAN}/clean.svg\">\
         <img id=\"ported\" src=\"http://{BLOCKED}:8080/ported.svg\">\
         <iframe id=\"ad-frame\" src=\"http://{BLOCKED}/ad.html\"></iframe>\
         <iframe id=\"frame\" src=\"http://{CLEAN}/frame.html\"></iframe>\
         <iframe id=\"same-site-frame\" src=\"/same.html\"></iframe>\
         </body></html>"
    )
}

/// The page of a frame: one image from the listed host. The unlisted site's
/// frame (`/frame.html`) asks for `framed.svg`, the listed host's (`/ad.html`)
/// for `from-ad-frame.svg`.
fn frame(image: &str) -> String {
    format!(
        "<!doctype html><html><body>\
         <img src=\"http://{BLOCKED}/{image}\">\
         </body></html>"
    )
}

/// A loopback server that answers the page and the three images, and keeps the
/// `Host` header of every request it gets.
struct Site {
    port: u16,
    hosts: Arc<Mutex<Vec<String>>>,
}

impl Site {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let port = listener.local_addr().expect("bound address").port();
        let hosts = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&hosts);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let log = Arc::clone(&log);
                std::thread::spawn(move || answer(stream, &log));
            }
        });
        Self { port, hosts }
    }

    /// The host names of the requests received so far, with their paths.
    fn seen(&self) -> Vec<String> {
        self.hosts.lock().expect("request log").clone()
    }

    /// The resolver rules that send the three image hosts to this server.
    fn resolver_rules(&self) -> String {
        let to = format!("127.0.0.1:{}", self.port);
        format!("MAP {BLOCKED} {to}, MAP *.{BLOCKED} {to}, MAP {CLEAN} {to}")
    }
}

/// Reads one request, notes its host and path, and answers it.
fn answer(mut stream: TcpStream, log: &Mutex<Vec<String>>) {
    let mut buffer = [0_u8; 4096];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or("/")
        .to_owned();
    let host = request
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("host")
                .then(|| value.trim().to_owned())
        })
        .unwrap_or_default();
    log.lock()
        .expect("request log")
        .push(format!("{host}{path}"));

    let (content_type, body) = if path == "/" {
        ("text/html", page())
    } else if path == "/frame.html" {
        ("text/html", frame("framed.svg"))
    } else if path == "/ad.html" {
        ("text/html", frame("from-ad-frame.svg"))
    } else if path == "/same.html" {
        ("text/html", frame("from-same-site-frame.svg"))
    } else if path.ends_with(".svg") {
        ("image/svg+xml", IMAGE.to_owned())
    } else {
        ("text/plain", String::new())
    };
    let status = if body.is_empty() {
        "404 Not Found"
    } else {
        "200 OK"
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Whether the server got a request for `host/path`.
fn reached(seen: &[String], request: &str) -> bool {
    seen.iter().any(|seen| seen == request)
}

fn test_config(site: &Site) -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    }
    .with_extra_args([format!("--host-resolver-rules={}", site.resolver_rules())]);
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

/// Opens the site's page with `config` and gives, for each image, the width
/// the browser decoded it at (0 for one that did not load).
async fn load_images(config: BrowserConfig, site: &Site) -> Vec<(String, u64)> {
    let mut sb = BaseCase::new(config).await.expect("connect to browser");
    sb.open(&format!("http://127.0.0.1:{}/", site.port))
        .await
        .expect("open the page");
    let images = sb
        .execute_script(
            "return Array.from(document.images).map(i => [i.id, i.complete ? i.naturalWidth : -1]);",
        )
        .await
        .expect("read the images");
    sb.quit().await.expect("quit");
    images
        .as_array()
        .expect("a list of images")
        .iter()
        .map(|image| {
            (
                image[0].as_str().expect("an id").to_owned(),
                image[1].as_i64().expect("a width").max(0).unsigned_abs(),
            )
        })
        .collect()
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn ad_block_refuses_the_listed_hosts_and_nothing_else_in_a_webdriver_session() {
    let _guard = BROWSER_LOCK.lock().await;
    let list = AdBlockList::from_hosts([BLOCKED]).expect("a host name");

    // With the block on, only the host that is not listed is ever asked.
    let site = Site::start();
    let config = test_config(&site).with_ad_block_list(list);
    let images = load_images(config, &site).await;
    let seen = site.seen();

    assert_eq!(
        images,
        [
            ("blocked".to_owned(), 0),
            ("subdomain".to_owned(), 0),
            ("clean".to_owned(), 3),
            ("ported".to_owned(), 3),
        ],
        "the listed images fail and the other ones load; the server saw {seen:?}"
    );
    for request in ["clean.svg", "frame.html"] {
        assert!(
            reached(&seen, &format!("{CLEAN}/{request}")),
            "{request} of the host that is not listed is requested: {seen:?}"
        );
    }
    for request in [
        format!("{BLOCKED}/blocked.svg"),
        format!("{BLOCKED_SUBDOMAIN}/subdomain.svg"),
        format!("{BLOCKED}/from-same-site-frame.svg"),
    ] {
        assert!(
            !reached(&seen, &request),
            "{request} never reached the server: {seen:?}"
        );
    }
    // The limits: a frame is navigated to a listed host, a frame of another site
    // makes its own requests, and a URL names a port; none is stopped.
    for request in [
        format!("{BLOCKED}:8080/ported.svg"),
        format!("{BLOCKED}/ad.html"),
        format!("{BLOCKED}/from-ad-frame.svg"),
        format!("{BLOCKED}/framed.svg"),
    ] {
        assert!(
            reached(&seen, &request),
            "{request} reaches the server, as it did in Chrome 155: {seen:?}"
        );
    }

    // The control: without the block the same page fetches the listed hosts too.
    let site = Site::start();
    let images = load_images(test_config(&site), &site).await;
    let seen = site.seen();

    assert_eq!(
        images,
        [
            ("blocked".to_owned(), 3),
            ("subdomain".to_owned(), 3),
            ("clean".to_owned(), 3),
            ("ported".to_owned(), 3),
        ],
        "with ad_block off every image loads; the server saw {seen:?}"
    );
    for request in [
        format!("{BLOCKED}/blocked.svg"),
        format!("{BLOCKED_SUBDOMAIN}/subdomain.svg"),
        format!("{CLEAN}/clean.svg"),
        format!("{BLOCKED}:8080/ported.svg"),
        format!("{BLOCKED}/ad.html"),
        format!("{BLOCKED}/from-ad-frame.svg"),
        format!("{BLOCKED}/framed.svg"),
        format!("{BLOCKED}/from-same-site-frame.svg"),
    ] {
        assert!(
            reached(&seen, &request),
            "{request} is requested when nothing blocks it: {seen:?}"
        );
    }
}
