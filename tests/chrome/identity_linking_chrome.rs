//! Two identities on one machine cannot be linked through stored state.
//!
//! A tracker that cannot fingerprint a browser can still recognise it by what
//! it stored on the first visit: a cookie, `localStorage`, IndexedDB, the Cache
//! API, a service worker registration, the HTTP cache (an `ETag` that names the
//! visitor comes back as `If-None-Match`), the favicon cache. The fixture page
//! (`tests/fixtures/identity/evercookie.html`) writes one mark into every store
//! a page can reach and reports what each store held before it did; a second
//! identity that finds the first one's mark anywhere is linked to it.
//!
//! What core controls and these tests prove: a second `BrowserContext` of one
//! browser, and a second browser with its own profile folder, start with every
//! store empty. What core does not control is noted in
//! `docs/design/fingerprint_consistency.md`: HSTS pins need an `https:` origin
//! this loopback server has not, TLS session tickets and HTTP/2 connection reuse
//! are the stealth repository's, and headless Chrome asks for no favicon, so
//! that cache is checked only when a favicon was fetched at all.
//!
//! The tests need a Chrome, so they are ignored by default:
//!
//! ```text
//! SB_HEADLESS=true cargo test -p seleniumbase-rs --features "mcp-server test-util testing bdd" \
//!   --test chrome identity_linking_chrome:: -- --ignored --test-threads=1 --nocapture
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use serde_json::Value;

const PAGE: &str = include_str!("../fixtures/identity/evercookie.html");
const WORKER: &str = include_str!("../fixtures/identity/sw.js");
/// A one-pixel GIF, the favicon.
const FAVICON: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\x00\x00\x00\xff\xff\xff\x21\xf9\x04\x01\x00\x00\x00\x00\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3b";

/// What the server saw: the `Cookie` headers, the `If-None-Match` values, how
/// many visitors asked for `/mark` without one, and the favicon requests.
#[derive(Default, Debug)]
struct Seen {
    cookies: Vec<String>,
    if_none_match: Vec<String>,
    visitors: usize,
    favicons: usize,
}

fn header<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    request.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// Serves the fixture on a loopback port (`127.0.0.1` is a secure context, which
/// the Cache API and service workers need) and records what each request carried.
fn serve(seen: Arc<Mutex<Seen>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 8192];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
            let path = request
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .split('?')
                .next()
                .unwrap_or("/")
                .to_owned();
            let mut seen = seen.lock().expect("the log");
            if let Some(cookie) = header(&request, "Cookie") {
                seen.cookies.push(cookie.to_owned());
            }
            let (status, kind, extra, body): (&str, &str, String, Vec<u8>) = match path.as_str() {
                "/" => (
                    "200 OK",
                    "text/html",
                    String::new(),
                    PAGE.as_bytes().to_vec(),
                ),
                "/sw.js" => (
                    "200 OK",
                    "application/javascript",
                    String::new(),
                    WORKER.as_bytes().to_vec(),
                ),
                "/favicon.ico" => {
                    seen.favicons += 1;
                    (
                        "200 OK",
                        "image/gif",
                        "Cache-Control: max-age=86400\r\n".to_owned(),
                        FAVICON.to_vec(),
                    )
                }
                "/mark" => {
                    if let Some(tag) = header(&request, "If-None-Match") {
                        seen.if_none_match.push(tag.to_owned());
                        (
                            "304 Not Modified",
                            "text/plain",
                            format!("ETag: {tag}\r\n"),
                            Vec::new(),
                        )
                    } else {
                        seen.visitors += 1;
                        let visitor = format!("visitor-{}", seen.visitors);
                        (
                            "200 OK",
                            "text/plain",
                            format!("ETag: \"{visitor}\"\r\nCache-Control: no-cache\r\n"),
                            visitor.into_bytes(),
                        )
                    }
                }
                _ => ("404 Not Found", "text/plain", String::new(), b"no".to_vec()),
            };
            drop(seen);
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    format!("http://{address}/")
}

fn options() -> LaunchOptions {
    LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .expect("options")
}

/// Opens the fixture as `mark` and returns what every store held before.
async fn visit(page: &Page, base: &str, mark: &str) -> Value {
    page.goto(format!("{base}?mark={mark}"))
        .await
        .expect("the fixture loads");
    for _ in 0..100 {
        let found = page
            .evaluate("window.__found || null")
            .await
            .expect("the page runs script");
        if !found.is_null() {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("the fixture never finished writing its marks")
}

/// The stores a page can read back, each as the fixture names it.
const STORES: [&str; 6] = [
    "cookie",
    "localStorage",
    "sessionStorage",
    "indexedDb",
    "cacheApi",
    "serviceWorkers",
];

/// Fails when any store, the HTTP cache included, shows `first`'s mark to the
/// second identity.
fn assert_unlinked(second: &Value, first: &Value, what: &str) {
    for store in STORES {
        let held = &second[store];
        let empty = held.is_null() || *held == Value::from(0);
        assert!(
            empty,
            "{what}: the second identity found something in {store}: {held} (the first left {})",
            first[store]
        );
    }
    assert_ne!(
        second["etag"], first["etag"],
        "{what}: the HTTP cache gave the second identity the first one's ETag body"
    );
    assert!(
        second["etag"]
            .as_str()
            .is_some_and(|id| id.starts_with("visitor-")),
        "{what}: the second identity did not reach the server for its own mark: {second}"
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn a_second_browser_context_starts_with_every_store_empty() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let base = serve(Arc::clone(&seen));
    let browser = Browser::launch(options()).await.expect("Chrome starts");

    // The first identity, twice: the stores work, so the test means something.
    let page = browser.default_page().await.expect("a tab");
    let first = visit(&page, &base, "alpha").await;
    let again = visit(&page, &base, "alpha").await;
    for store in ["cookie", "localStorage", "indexedDb", "cacheApi"] {
        assert!(
            again[store]
                .as_str()
                .is_some_and(|held| held.contains("alpha")),
            "the first identity lost its own mark in {store}: {again}"
        );
    }
    assert_eq!(
        again["etag"], first["etag"],
        "the HTTP cache revalidated with the ETag"
    );
    assert_eq!(
        again["serviceWorkers"], 1,
        "the registration survived a reload"
    );
    let favicons_after_first = seen.lock().expect("the log").favicons;

    // The second identity: a browser context beside the first.
    let second_context = browser.new_context().await.expect("a context");
    let second_page = second_context
        .new_page()
        .await
        .expect("a page in the context");
    let second = visit(&second_page, &base, "beta").await;
    assert_unlinked(&second, &first, "a second browser context");

    let log = seen.lock().expect("the log");
    assert!(
        !log.cookies
            .iter()
            .any(|cookie| cookie.contains("alpha") && cookie.contains("beta")),
        "one request carried both identities' cookies: {log:?}"
    );
    if favicons_after_first > 0 {
        assert!(
            log.favicons > favicons_after_first,
            "the favicon cache of the first identity served the second: {log:?}"
        );
    } else {
        println!("headless Chrome asked for no favicon, so the favicon cache is not checked");
    }
    drop(log);

    second_context.dispose().await.expect("the context closes");
    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn a_second_browser_with_its_own_profile_starts_with_every_store_empty() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let base = serve(Arc::clone(&seen));

    let first_browser = Browser::launch(options()).await.expect("Chrome starts");
    let page = first_browser.default_page().await.expect("a tab");
    let first = visit(&page, &base, "alpha").await;
    first_browser.close().await.expect("Chrome closes");

    let second_browser = Browser::launch(options())
        .await
        .expect("Chrome starts again");
    let page = second_browser.default_page().await.expect("a tab");
    let second = visit(&page, &base, "beta").await;
    assert_unlinked(&second, &first, "a second browser with its own profile");
    assert_eq!(
        seen.lock().expect("the log").if_none_match,
        Vec::<String>::new(),
        "neither browser revalidated the other's cache entry"
    );
    second_browser.close().await.expect("Chrome closes");
}
