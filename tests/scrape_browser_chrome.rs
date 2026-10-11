//! The browser scraper against a real Chrome and a local loopback server.
//!
//! The server plays a site behind a script challenge: the first load of `/` is a Cloudflare-style
//! interstitial that sets its clearance cookie from script a moment later, and a load that carries
//! the cookie is the real page. The same server also answers as an HTTP proxy, so a fetch of a
//! made-up host through it shows which egress a tab used, and in which language and time zone.
//!
//! These tests need Chrome, so they are ignored by default. Only one Chrome-driving test binary may
//! run on the machine at a time. Run them with:
//!
//! ```text
//! cargo test --features scrape --test scrape_browser_chrome -- --ignored --test-threads=1
//! ```

#![cfg(feature = "scrape")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::scrape::challenge::detect::ChallengeKind;
use seleniumbase_rs::scrape::egress::geo::CountryCode;
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, ScraperEvent};
use seleniumbase_rs::scrape::runtime::browser_scraper::{BrowserScraper, BrowserScraperBuilder};
use serde_json::json;

/// How long the browser sits unused before the next fetch. It is longer than the 30 seconds after
/// which a browser library that closes idle browsers closes them.
const IDLE_WINDOW: Duration = Duration::from_secs(40);

/// How long a solve may take: the challenge page sets its cookie after half a second.
const SOLVE_TIMEOUT: Duration = Duration::from_secs(20);

/// The made-up host fetched through the proxy. The server answers for any host.
const SHOP: &str = "http://shop.example.test/";

/// The Cloudflare script-challenge interstitial the detection knows.
const CHALLENGE: &str = include_str!("fixtures/challenge/cloudflare_legacy_js.html");

/// The script that passes the challenge a moment after the page loads.
const PASS_SCRIPT: &str = "<script>setTimeout(function () { \
    document.cookie = 'cf_clearance=real-chrome-token; path=/'; }, 500);</script>";

/// What the server did, for the test to read.
#[derive(Default)]
struct Served {
    challenges: AtomicUsize,
    pages: AtomicUsize,
    accept_language: Mutex<String>,
}

/// A loopback server that is a site, and an HTTP proxy for any other host.
struct Server {
    port: u16,
    served: Arc<Served>,
}

impl Server {
    fn start(name: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let port = listener.local_addr().expect("a bound address").port();
        let served = Arc::new(Served::default());
        let state = Arc::clone(&served);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let state = Arc::clone(&state);
                thread::spawn(move || answer(stream, name, &state));
            }
        });
        Self { port, served }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn proxy(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn last_accept_language(&self) -> String {
        self.served
            .accept_language
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Reads one request and writes one response.
fn answer(mut stream: TcpStream, name: &str, served: &Served) {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(read) => request.extend_from_slice(&chunk[..read]),
        }
        if request.len() > 64 * 1024 {
            return;
        }
    }
    let text = String::from_utf8_lossy(&request).into_owned();
    let (status, body) = respond(&text, name, served);
    let reply = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nServer: cloudflare\r\n\
         CF-RAY: 8a1b2c3d4e5f6071-AMS\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    // The browser may have gone away; there is nobody to tell.
    let _ = stream.write_all(reply.as_bytes());
}

/// The path of a request line, whether the browser asked in origin form or in the absolute form it
/// uses with a proxy.
fn path_of(request: &str) -> String {
    let target = request.split_whitespace().nth(1).unwrap_or("/");
    match target.strip_prefix("http://") {
        Some(rest) => rest.find('/').map_or("/", |at| &rest[at..]).to_owned(),
        None => target.to_owned(),
    }
}

/// The status and body for a request.
fn respond(request: &str, name: &str, served: &Served) -> (&'static str, String) {
    let header = |wanted: &str| {
        request
            .lines()
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case(wanted)
                    .then(|| value.trim().to_owned())
            })
            .unwrap_or_default()
    };
    *served
        .accept_language
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = header("accept-language");
    let path = path_of(request);
    if path.starts_with("/favicon") {
        return ("404 Not Found", String::new());
    }
    if path == "/" && !header("cookie").contains("cf_clearance=") {
        served.challenges.fetch_add(1, Ordering::SeqCst);
        return (
            "503 Service Unavailable",
            format!("{CHALLENGE}{PASS_SCRIPT}"),
        );
    }
    served.pages.fetch_add(1, Ordering::SeqCst);
    (
        "200 OK",
        format!("<html><head><title>Shop</title></head><body><h1>Normal page</h1><p>served by {name}</p></body></html>"),
    )
}

fn headless() -> seleniumbase_rs::sb_cdp::LaunchOptionsBuilder {
    LaunchOptions::builder().headless(true)
}

fn builder(sink: &CollectingEventSink) -> BrowserScraperBuilder {
    BrowserScraper::builder()
        .launch_options(headless())
        .with_event_sink(Arc::new(sink.clone()))
        .solve_timeout(SOLVE_TIMEOUT)
}

fn country(code: &str) -> CountryCode {
    CountryCode::new(code).expect("a valid country")
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_script_challenge_is_passed_by_a_real_chrome() {
    let server = Server::start("origin");
    let sink = CollectingEventSink::new();
    let mut scraper = builder(&sink).build().await.expect("Chrome starts");

    let page = scraper
        .fetch(&server.url("/"))
        .await
        .expect("the challenge is passed");

    assert_eq!((page.status, page.attempts), (200, 2));
    assert_eq!(page.solved, Some(ChallengeKind::JsChallenge));
    assert!(page.body.contains("Normal page"), "{}", page.body);
    assert_eq!(server.served.challenges.load(Ordering::SeqCst), 1);
    let kinds: Vec<_> = sink
        .events()
        .into_iter()
        .filter_map(|event| match event {
            ScraperEvent::ChallengeDetected { kind, .. } => Some(kind),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, [ChallengeKind::JsChallenge]);
    let exported = scraper
        .export_identity()
        .await
        .expect("the identity exports");
    assert!(
        exported.clearance_for("127.0.0.1").is_some(),
        "the clearance the page set is in the identity"
    );
    scraper.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn an_idle_browser_is_still_usable_after_the_idle_window() {
    let server = Server::start("origin");
    let sink = CollectingEventSink::new();
    let mut scraper = builder(&sink).build().await.expect("Chrome starts");
    scraper
        .fetch(&server.url("/plain"))
        .await
        .expect("the first fetch");

    tokio::time::sleep(IDLE_WINDOW).await;

    assert!(scraper.browser().is_connected(), "Chrome is still there");
    let again = scraper
        .fetch(&server.url("/plain"))
        .await
        .expect("the fetch after the idle window");
    assert_eq!(again.status, 200);
    scraper.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_egress_decides_the_language_and_the_time_zone_and_rotating_moves_the_tab() {
    let germany = Server::start("proxy-de");
    let france = Server::start("proxy-fr");
    let sink = CollectingEventSink::new();
    let mut scraper = builder(&sink)
        .with_geo_proxies([
            (germany.proxy(), country("DE")),
            (france.proxy(), country("FR")),
        ])
        .expect("valid proxies")
        .build()
        .await
        .expect("Chrome starts");

    let first = scraper
        .fetch(&format!("{SHOP}plain"))
        .await
        .expect("through the first proxy");
    let zone = scraper
        .page()
        .evaluate("Intl.DateTimeFormat().resolvedOptions().timeZone")
        .await
        .expect("the zone reads");
    let language = scraper
        .page()
        .evaluate("navigator.language")
        .await
        .expect("it reads");

    assert!(first.body.contains("served by proxy-de"), "{}", first.body);
    assert_eq!(zone, json!("Europe/Berlin"));
    assert_eq!(language, json!("de-DE"));
    assert!(
        germany.last_accept_language().starts_with("de-DE"),
        "{}",
        germany.last_accept_language()
    );

    scraper.rotate_egress().await.expect("the egress rotates");
    let second = scraper
        .fetch(&format!("{SHOP}plain"))
        .await
        .expect("through the second proxy");
    let zone = scraper
        .page()
        .evaluate("Intl.DateTimeFormat().resolvedOptions().timeZone")
        .await
        .expect("the zone reads");

    assert!(
        second.body.contains("served by proxy-fr"),
        "{}",
        second.body
    );
    assert_eq!(zone, json!("Europe/Paris"));
    // Chrome sends background requests through a proxy too, so the count is at least the page.
    assert!(
        france.served.pages.load(Ordering::SeqCst) >= 1,
        "the second proxy served the page"
    );
    scraper.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn rotating_the_profile_starts_a_new_chrome_that_reports_the_new_profile() {
    let server = Server::start("origin");
    let sink = CollectingEventSink::new();
    let mut scraper = builder(&sink)
        .seed(11)
        .build()
        .await
        .expect("Chrome starts");
    scraper
        .fetch(&server.url("/plain"))
        .await
        .expect("the first fetch");
    let before = scraper.fingerprint().clone();
    let cores_before = scraper
        .page()
        .evaluate("navigator.hardwareConcurrency")
        .await
        .expect("it reads");
    assert_eq!(cores_before, json!(before.hardware_concurrency));

    scraper.rotate_profile().await.expect("the profile rotates");
    scraper
        .fetch(&server.url("/plain"))
        .await
        .expect("the fetch after the rotation");

    let after = scraper.fingerprint();
    assert_ne!(&before, after, "another profile was drawn");
    let page = scraper.page();
    let cores = page
        .evaluate("navigator.hardwareConcurrency")
        .await
        .expect("it reads");
    let agent = page
        .evaluate("navigator.userAgent")
        .await
        .expect("it reads");
    assert_eq!(
        cores,
        json!(after.hardware_concurrency),
        "the page reports the new profile"
    );
    assert_eq!(agent, json!(after.user_agent));
    assert!(sink
        .events()
        .iter()
        .any(|event| matches!(event, ScraperEvent::ProfileRotated { .. })));
    scraper.close().await.expect("Chrome closes");
}
