//! Drive a `BrowserScraper` against a loopback page that guards itself with a script challenge,
//! and print how the scraper detected it, solved it and loaded the page again.
//!
//! Needs: the `scrape` feature (pure Rust) and a Chrome on this machine. It runs headless. The
//! scraper does no TLS spoofing, so it needs no measured system and runs wherever Chrome does;
//! the measured Chrome 155 tables are not involved. The site is started here on `127.0.0.1` and
//! speaks plain HTTP, so no outside network is used. Only one Chrome-driving program should run
//! on the machine at a time.
//!
//! ```text
//! cargo run --features scrape --example browser_scraper
//! ```
//!
//! The site is a stand-in written for this example, not any vendor's challenge. `/` answers with a
//! script challenge until the visitor holds the clearance cookie; the challenge page's script
//! calls `/__solve`, which sets the cookie, and reloads. The scraper detects the challenge, waits
//! for the clearance in Chrome, and loads the page again.

use std::error::Error;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::process::ExitCode;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::Duration;

use seleniumbase_rs::sb_cdp::LaunchOptions;
use seleniumbase_rs::{BrowserScraper, CollectingEventSink, ScraperEvent};

/// The cookie that proves the challenge was passed.
const CLEARANCE: &str = "cf_clearance";
/// The value of the clearance cookie. It is made up for the example and opens nothing.
const CLEARANCE_VALUE: &str = "example-clearance-value";
/// What the real page says.
const REAL_MARKER: &str = "the real page";
/// The real page.
const REAL_HTML: &str = "<!doctype html><title>shop</title><p id=real>the real page</p>";
/// The challenge page: Cloudflare's title and options object, and a script that earns the cookie
/// and reloads, the way the browser's own challenge does. The script waits half a second first, as
/// the computation of a real challenge takes a moment. A scraper learns the clearance value it
/// had before the solve, and waits for a new one, so a cookie set before it looked would not
/// count as new.
const CHALLENGE_HTML: &str = "<!doctype html><html><head><title>Just a moment...</title></head>\
    <body><script>window._cf_chl_opt = { cType: 'managed' };\
    setTimeout(function () {\
        fetch('/__solve').then(function () { location.reload(); });\
    }, 500);</script></body></html>";
/// How long Chrome is given to run the challenge page's script.
const SOLVE_WITHIN: Duration = Duration::from_secs(30);
/// How long a connection may sit without sending a request. Chrome opens spare connections.
const IDLE_CONNECTION: Duration = Duration::from_secs(5);
/// The most bytes of a request head the site reads.
const MAX_HEAD_BYTES: usize = 64 * 1024;

/// What the site saw, one line per request.
type Log = Arc<Mutex<Vec<String>>>;

/// The site: a listener on the loopback address and the record of its requests.
struct Site {
    port: u16,
    log: Log,
}

impl Site {
    /// Binds the loopback address on a port the system picks and starts serving on threads that
    /// end with the process.
    fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let log = Log::default();
        let shared = Arc::clone(&log);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = Arc::clone(&shared);
                thread::spawn(move || answer(stream, &log));
            }
        });
        Ok(Self { port, log })
    }

    /// The URL of `path` on the site.
    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    /// What the site saw so far, oldest first.
    fn seen(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Reads one request and writes one response.
fn answer(mut stream: TcpStream, log: &Mutex<Vec<String>>) {
    if stream.set_read_timeout(Some(IDLE_CONNECTION)).is_err() {
        return;
    }
    let Some(head) = read_head(&mut stream) else {
        return;
    };
    let path = head.split_whitespace().nth(1).unwrap_or("/");
    let cookie = header(&head, "cookie").unwrap_or_default();
    let cleared = cookie.contains(&format!("{CLEARANCE}={CLEARANCE_VALUE}"));
    let reply = match path {
        "/open" => response("200 OK", "", REAL_HTML),
        "/__solve" => response(
            "200 OK",
            &format!(
                "Set-Cookie: {CLEARANCE}={CLEARANCE_VALUE}; Path=/; Max-Age=600; HttpOnly\r\n"
            ),
            "ok",
        ),
        _ if path.starts_with("/favicon") => response("404 Not Found", "", ""),
        _ if cleared => response("200 OK", "", REAL_HTML),
        _ => response(
            "403 Forbidden",
            "cf-mitigated: challenge\r\n",
            CHALLENGE_HTML,
        ),
    };
    if !path.starts_with("/favicon") {
        let carried = if cleared {
            "the clearance"
        } else {
            "no clearance"
        };
        log.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(format!("{path:<9} with {carried}"));
    }
    // The browser may have gone away; there is nobody to tell.
    let _written = stream.write_all(reply.as_bytes());
}

/// The head of the request on `stream`, up to the blank line.
fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut head = Vec::new();
    let mut chunk = [0_u8; 4096];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        let read = stream.read(&mut chunk).ok().filter(|&read| read > 0)?;
        head.extend_from_slice(chunk.get(..read)?);
        if head.len() > MAX_HEAD_BYTES {
            return None;
        }
    }
    Some(String::from_utf8_lossy(&head).into_owned())
}

/// The value of the first header called `name` in a request head.
fn header(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

/// An HTTP/1.1 response that closes the connection after the body.
fn response(status: &str, extra_headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html\r\n{extra_headers}\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("browser_scraper: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let site = Site::start()?;
    let sink = CollectingEventSink::new();
    let mut scraper = BrowserScraper::builder()
        .launch_options(LaunchOptions::builder().headless(true))
        .with_event_sink(Arc::new(sink.clone()))
        .solve_timeout(SOLVE_WITHIN)
        .build()
        .await?;

    let fetched = scraper.fetch(&site.url("/")).await;
    // Close the browser whatever the fetch did.
    scraper.close().await?;
    let page = fetched?;

    println!(
        "fetched /: status {}, {} loads, challenge solved on the way: {:?}",
        page.status, page.attempts, page.solved
    );
    println!(
        "the page is the real one: {}",
        page.body.contains(REAL_MARKER)
    );

    println!("\nwhat the scraper reported");
    for event in sink.events() {
        match event {
            ScraperEvent::ChallengeDetected { kind, vendor, .. } => {
                println!("  challenge detected: {kind:?}, vendor {vendor:?}");
            }
            ScraperEvent::ChallengeSolved {
                kind,
                attempts,
                elapsed_ms,
                ..
            } => {
                println!("  challenge solved: {kind:?} after {attempts} loads, {elapsed_ms} ms");
            }
            _ => {}
        }
    }

    println!("\nwhat the site saw");
    for line in site.seen() {
        println!("  {line}");
    }
    Ok(())
}
