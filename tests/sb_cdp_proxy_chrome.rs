//! The proxy account against the real Chrome: it is given to the proxy and never to a website.
//!
//! A website can answer any request with `401` and `WWW-Authenticate: Basic`, and Chrome repeats
//! the request with whatever `Fetch.continueWithAuth` supplies. The proxy's password must not be
//! what is supplied. The test stands up an HTTP proxy that asks for a password (`407`) and then
//! answers the page's request like a website that asks for one (`401`), and looks at what Chrome
//! sends back.
//!
//! It starts Chrome, so it is ignored by default. Only one Chrome-driving test binary may run on
//! the machine at a time:
//!
//! ```text
//! cargo test --test sb_cdp_proxy_chrome -- --ignored --test-threads=1
//! ```

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const USER: &str = "alice";
const PASSWORD: &str = "s3cret-proxy-password";
/// The website the page asks for; the proxy plays it.
const SITE: &str = "site.test";
/// How long a test waits for Chrome to have made its requests.
const SETTLE: Duration = Duration::from_millis(1500);

/// What the proxy saw.
#[derive(Default)]
struct Seen {
    /// Requests that carried the right `Proxy-Authorization`.
    proxy_logins: usize,
    /// Requests answered `401` as the website.
    website_prompts: usize,
    /// The `Authorization` header of every request that had one: what Chrome told the website.
    website_logins: Vec<String>,
}

/// Reads one request head, `None` when the connection ends first.
async fn read_head(tcp: &mut TcpStream, buffered: &mut Vec<u8>) -> Option<String> {
    loop {
        if let Some(end) = buffered.windows(4).position(|window| window == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffered[..end]).into_owned();
            buffered.drain(..end + 4);
            return Some(head);
        }
        let mut chunk = [0_u8; 4096];
        let read = tcp.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffered.extend_from_slice(&chunk[..read]);
    }
}

/// The value of header `name` in a request head.
fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case(name).then_some(value.trim())
    })
}

/// A proxy that asks for the password, then answers like a website that asks for one too.
async fn serve(mut tcp: TcpStream, seen: Arc<Mutex<Seen>>) {
    let expected = format!("Basic {}", STANDARD.encode(format!("{USER}:{PASSWORD}")));
    let mut buffered = Vec::new();
    while let Some(head) = read_head(&mut tcp, &mut buffered).await {
        let answer = {
            let mut state = seen.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(login) = header(&head, "authorization") {
                state.website_logins.push(login.to_owned());
            }
            if header(&head, "proxy-authorization") == Some(expected.as_str()) {
                state.proxy_logins += 1;
                state.website_prompts += 1;
                "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"site\"\r\n\
                 Content-Type: text/plain\r\nContent-Length: 6\r\n\r\ndenied"
            } else {
                "HTTP/1.1 407 Proxy Authentication Required\r\n\
                 Proxy-Authenticate: Basic realm=\"proxy\"\r\nContent-Length: 0\r\n\r\n"
            }
        };
        if tcp.write_all(answer.as_bytes()).await.is_err() {
            return;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn a_website_that_asks_for_a_password_never_gets_the_proxy_account() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("binds loopback");
    let port = listener.local_addr().expect("an address").port();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let record = Arc::clone(&seen);
    let proxy = tokio::spawn(async move {
        while let Ok((tcp, _peer)) = listener.accept().await {
            tokio::spawn(serve(tcp, Arc::clone(&record)));
        }
    });
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .proxy(&format!("http://{USER}:{PASSWORD}@127.0.0.1:{port}"))
        .build()
        .expect("launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.expect("a tab");

    // The navigation is answered `401` by the "website" once the proxy has its password.
    let _loaded = page.goto(format!("http://{SITE}/")).await;
    tokio::time::sleep(SETTLE).await;
    browser.close().await.expect("Chrome closes");
    proxy.abort();

    let seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(
        seen.proxy_logins >= 1,
        "Chrome never logged in to the proxy, so the test saw nothing"
    );
    assert!(
        seen.website_prompts >= 1,
        "the website never asked for a password"
    );
    assert_eq!(
        seen.website_logins,
        Vec::<String>::new(),
        "Chrome answered the website's password prompt with the proxy account"
    );
}
