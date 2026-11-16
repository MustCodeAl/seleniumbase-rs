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
//! A control keeps one profile folder for two launches (`a_kept_profile_folder_links_two_visits`):
//! the second visit then finds the first one's mark in the cookie (the fixture's has an expiry,
//! a session cookie dies with the browser), `localStorage`, IndexedDB, the Cache API and the
//! service worker, and the server is asked to revalidate the first one's `ETag` and
//! `Last-Modified`. It proves the harness would see a link.
//!
//! The tests need a Chrome, so they are ignored by default:
//!
//! ```text
//! SB_HEADLESS=true cargo test -p seleniumbase-rs --features "mcp-server test-util testing bdd" \
//!   --test chrome identity_linking_chrome:: -- --ignored --test-threads=1 --nocapture
//! ```

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicU8, Ordering};
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
    if_modified_since: Vec<String>,

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
                    if let Some(since) = header(&request, "If-Modified-Since") {
                        seen.if_modified_since.push(since.to_owned());
                    }

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
                            format!("ETag: \"{visitor}\"\r\nLast-Modified: Sun, 01 Nov 2026 00:00:{:02} GMT\r\nCache-Control: no-cache\r\n", seen.visitors % 60),
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
        let empty = held.is_null() || held.as_u64() == Some(0);
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

    {
        let log = seen.lock().expect("the log");
        assert!(
            !log.cookies
                .iter()
                .any(|cookie| cookie.contains("alpha") && cookie.contains("beta")),
            "one request carried both identities' cookies: {log:?}"
        );
        // The one revalidation is the first identity's own second visit, and a validator
        // never travels alone: the date comes with the ETag. A second one would be linking.
        assert_eq!(
            (log.if_none_match.len(), log.if_modified_since.len()),
            (1, 1),
            "only the first identity revalidated its own entry: {log:?}"
        );

        if favicons_after_first > 0 {
            assert!(
                log.favicons > favicons_after_first,
                "the favicon cache of the first identity served the second: {log:?}"
            );
        } else {
            println!("headless Chrome asked for no favicon, so the favicon cache is not checked");
        }
    }

    second_context.dispose().await.expect("the context closes");
    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]
async fn a_kept_profile_folder_links_two_visits() {
    // The control: the harness can see a link when there is one. Two launches share one
    // profile folder, so the second visit finds the first one's mark in every store that
    // outlives a browser, and the server is asked to revalidate the first one's cache entry.
    let seen = Arc::new(Mutex::new(Seen::default()));
    let base = serve(Arc::clone(&seen));
    let folder = tempfile::tempdir().expect("a profile folder");
    let kept = || {
        LaunchOptions::builder()
            .headless(true)
            .no_sandbox(std::env::var_os("CI").is_some())
            .user_data_dir(folder.path())
            .build()
            .expect("options")
    };

    let first_browser = Browser::launch(kept()).await.expect("Chrome starts");
    let page = first_browser.default_page().await.expect("a tab");
    let first = visit(&page, &base, "alpha").await;
    // Awaited: what a browser flushes at close is what the next one finds.
    first_browser.close().await.expect("Chrome closes");

    let second_browser = Browser::launch(kept()).await.expect("Chrome starts again");
    let page = second_browser.default_page().await.expect("a tab");
    let second = visit(&page, &base, "beta").await;
    second_browser.close().await.expect("Chrome closes");

    for store in ["cookie", "localStorage", "indexedDb", "cacheApi"] {
        assert!(
            second[store]
                .as_str()
                .is_some_and(|held| held.contains("alpha")),
            "the kept profile lost the first visit's mark in {store}: {second} (the first left {first})"
        );
    }
    assert_eq!(
        second["serviceWorkers"], 1,
        "the registration outlived the browser: {second}"
    );
    assert_eq!(
        second["etag"], first["etag"],
        "the HTTP cache of the kept profile answered with the first visitor's body"
    );
    let log = seen.lock().expect("the log");
    assert!(
        !log.if_none_match.is_empty() || !log.if_modified_since.is_empty(),
        "the server saw no validator from the second launch: {log:?}"
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
#[serial_test::serial(chrome)]

async fn a_second_browser_context_opens_its_own_connections() {
    // HTTP/1.1 keep-alive: a socket a page left open is reused by the next request to the
    // same origin of the same network context. A socket two identities shared would be a
    // link a server sees without reading a single cookie. (HTTP/2 needs TLS and is not
    // tested here.)
    let requests: Requests = Arc::default();
    let phase = Arc::new(AtomicU8::new(0));
    let base = serve_keep_alive(Arc::clone(&requests), Arc::clone(&phase));
    let browser = Browser::launch(options()).await.expect("Chrome starts");

    // The first identity visits twice, as the default context.
    phase.store(1, Ordering::SeqCst);
    let page = browser.default_page().await.expect("a tab");
    for visit in 1..=2 {
        page.goto(format!("{base}?visit={visit}"))
            .await
            .expect("the page loads");
    }
    // The second identity, in a browser context beside it.
    phase.store(2, Ordering::SeqCst);
    let context = browser.new_context().await.expect("a context");
    let second_page = context.new_page().await.expect("a page in the context");
    for visit in 3..=4 {
        second_page
            .goto(format!("{base}?visit={visit}"))
            .await
            .expect("the page loads");
    }

    // The pages each identity asked for, by the number in the address: a late request of the
    // first identity's page (a favicon) may come in the second phase and is not the second
    // identity's.
    let log = requests.lock().expect("the log").clone();
    let pages_of = |visits: std::ops::RangeInclusive<u32>| -> Vec<usize> {
        log.iter()
            .filter(|(_, _, target)| {
                visits
                    .clone()
                    .any(|visit| target.ends_with(&format!("?visit={visit}")))
            })
            .map(|(connection, _, _)| *connection)
            .collect()
    };
    let (first, second) = (pages_of(1..=2), pages_of(3..=4));
    assert_eq!(first.len(), 2, "the first identity's pages: {log:?}");
    assert_eq!(second.len(), 2, "the second identity's pages: {log:?}");
    assert_eq!(
        first[0], first[1],
        "the control: the first identity's two visits shared a connection, so reuse is visible: {log:?}"
    );
    let (first, second): (BTreeSet<_>, BTreeSet<_>) =
        (first.into_iter().collect(), second.into_iter().collect());
    assert!(
        first.is_disjoint(&second),
        "the two identities shared a connection: {first:?} and {second:?} in {log:?}"
    );
    // Whatever else was asked in the second phase came on the second identity's sockets or
    // was the first identity's own late request; print it so a run shows it.
    println!("the requests of the run: {log:?}");
    context.dispose().await.expect("the context closes");
    browser.close().await.expect("Chrome closes");
}

/// Every request the keep-alive server answered: the connection it came on, numbered from 1 in
/// the order the sockets were accepted, the phase the test was in and the request target.
type Requests = Arc<Mutex<Vec<(usize, u8, String)>>>;

/// The end of a request head (`\r\n\r\n`), as an offset past it.
fn end_of_head(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|at| at + 4)
}

/// Serves a page and keeps each connection open: one thread per socket reads requests in a loop
/// and answers each with a length-delimited body, logging `(connection, phase, target)`.
fn serve_keep_alive(log: Requests, phase: Arc<AtomicU8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        let mut accepted = 0;
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            accepted += 1;
            let connection = accepted;
            let (log, phase) = (Arc::clone(&log), Arc::clone(&phase));
            thread::spawn(move || {
                let mut pending = Vec::new();
                let mut buffer = [0_u8; 8192];
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) | Err(_) => return,
                        Ok(read) => pending.extend_from_slice(&buffer[..read]),
                    }
                    while let Some(end) = end_of_head(&pending) {
                        let head = String::from_utf8_lossy(&pending[..end]).into_owned();
                        let target = head
                            .lines()
                            .next()
                            .and_then(|line| line.split_whitespace().nth(1))
                            .unwrap_or_default()
                            .to_owned();
                        pending.drain(..end);
                        log.lock().expect("the log").push((
                            connection,
                            phase.load(Ordering::SeqCst),
                            target,
                        ));
                        let body = "<!doctype html><title>keep-alive</title><p>keep-alive</p>";
                        let answer = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: keep-alive\r\n\r\n{body}",
                            body.len()
                        );
                        if stream.write_all(answer.as_bytes()).is_err() {
                            return;
                        }
                    }
                }
            });
        }
    });
    format!("http://{address}/")
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
    assert_eq!(
        seen.lock().expect("the log").if_modified_since,
        Vec::<String>::new(),
        "neither browser sent a validator of the other's cache entry"
    );

    second_browser.close().await.expect("Chrome closes");
}
