//! A WebDriver session logs in to a proxy that asks for a password.
//!
//! Chrome reads no login from `--proxy-server` and chromedriver cannot answer
//! the proxy's `407`, so the session attaches a DevTools client beside
//! chromedriver (`browser::proxy_auth`). These tests stand up an HTTP proxy
//! that wants `Proxy-Authorization` and serves a few pages itself, and drive a
//! real Chrome through it with `BaseCase`: the first page, a new window, a tab
//! the page opens, a website that asks for its own password, and the command
//! line Chrome was started with.
//!
//! The Chrome tests are ignored by default. Run them with Chrome installed, one
//! binary at a time:
//!
//! ```bash
//! cargo test --test proxy_auth_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a chromedriver you started yourself.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use seleniumbase_rs::{BaseCase, BrowserConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const USER: &str = "alice";
const PASSWORD: &str = "s3cret-proxy-password";
/// The website the pages come from; the proxy plays it. It is not a loopback
/// name, so Chrome sends it through the proxy.
const SITE: &str = "site.test";

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// What the proxy saw.
#[derive(Default)]
struct Seen {
    /// Requests that carried the right `Proxy-Authorization`.
    proxy_logins: usize,
    /// Requests refused with `407`.
    refused: usize,
    /// Requests answered `401` as the website.
    website_prompts: usize,
    /// The `Authorization` header of every request that had one: what Chrome told the website.
    website_logins: Vec<String>,
    /// The paths served, in order.
    served: Vec<String>,
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

/// A page with a title, as an HTTP response.
fn page(title: &str, body: &str) -> String {
    let html = format!("<html><head><title>{title}</title></head><body>{body}</body></html>");
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{html}",
        html.len()
    )
}

/// The path of an absolute-form request line (`GET http://site.test/x HTTP/1.1`).
fn path_of(head: &str) -> String {
    let target = head.split_whitespace().nth(1).unwrap_or("/");
    let without_scheme = target.split_once("://").map_or(target, |(_, rest)| rest);
    without_scheme
        .find('/')
        .map_or("/", |at| &without_scheme[at..])
        .to_owned()
}

/// A proxy that asks for the password, then answers like the website.
async fn serve(mut tcp: TcpStream, seen: Arc<Mutex<Seen>>) {
    let expected = format!("Basic {}", STANDARD.encode(format!("{USER}:{PASSWORD}")));
    let mut buffered = Vec::new();
    while let Some(head) = read_head(&mut tcp, &mut buffered).await {
        let answer = {
            let mut state = seen.lock().unwrap_or_else(PoisonError::into_inner);
            let website_login = header(&head, "authorization");
            if let Some(login) = website_login {
                state.website_logins.push(login.to_owned());
            }
            if header(&head, "proxy-authorization") == Some(expected.as_str()) {
                state.proxy_logins += 1;
                let path = path_of(&head);
                state.served.push(path.clone());
                match path.as_str() {
                    "/" => page(
                        "through the proxy",
                        &format!(
                            "<a id=\"out\" href=\"http://{SITE}/second\" target=\"_blank\">out</a>"
                        ),
                    ),
                    "/second" => page("second tab", "second"),
                    "/site-auth" => {
                        state.website_prompts += 1;
                        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"site\"\r\n\
                         Content-Type: text/plain\r\nContent-Length: 6\r\n\r\ndenied"
                            .to_owned()
                    }
                    _ => page("not found", "not found"),
                }
            } else {
                state.refused += 1;
                "HTTP/1.1 407 Proxy Authentication Required\r\n\
                 Proxy-Authenticate: Basic realm=\"proxy\"\r\nContent-Length: 0\r\n\r\n"
                    .to_owned()
            }
        };
        if tcp.write_all(answer.as_bytes()).await.is_err() {
            return;
        }
    }
}

/// Starts the proxy; the task ends when the handle is aborted.
async fn start_proxy() -> (u16, Arc<Mutex<Seen>>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("binds loopback");
    let port = listener.local_addr().expect("an address").port();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let record = Arc::clone(&seen);
    let task = tokio::spawn(async move {
        while let Ok((tcp, _peer)) = listener.accept().await {
            tokio::spawn(serve(tcp, Arc::clone(&record)));
        }
    });
    (port, seen, task)
}

fn config(port: u16) -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    }
    .with_proxy(format!("{USER}:{PASSWORD}@127.0.0.1:{port}"));
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

/// Waits until the window's title is `want`, or says what it was.
async fn title_becomes(sb: &mut BaseCase, want: &str) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..50 {
        last = sb.get_title().await.unwrap_or_default();
        if last == want {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    Err(format!("the title stayed {last:?}, wanted {want:?}"))
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_page_loads_through_a_proxy_that_asks_for_a_password() {
    let _guard = BROWSER_LOCK.lock().await;
    let (port, seen, proxy) = start_proxy().await;
    let mut sb = BaseCase::new(config(port))
        .await
        .expect("connect to browser");

    sb.open(&format!("http://{SITE}/"))
        .await
        .expect("the page opens");
    let title = sb.get_title().await.expect("a title");
    sb.quit().await.expect("quit");
    proxy.abort();

    assert_eq!(title, "through the proxy");
    let seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(seen.proxy_logins >= 1, "the proxy never saw the login");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_new_window_loads_through_the_proxy_too() {
    let _guard = BROWSER_LOCK.lock().await;
    let (port, _seen, proxy) = start_proxy().await;
    let mut sb = BaseCase::new(config(port))
        .await
        .expect("connect to browser");

    sb.open(&format!("http://{SITE}/"))
        .await
        .expect("the page opens");
    sb.switch_to_new_window().await.expect("a new window");
    sb.open(&format!("http://{SITE}/second"))
        .await
        .expect("the second page opens");
    let title = sb.get_title().await.expect("a title");
    sb.quit().await.expect("quit");
    proxy.abort();

    assert_eq!(title, "second tab");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_tab_the_page_opens_loads_through_the_proxy_too() {
    let _guard = BROWSER_LOCK.lock().await;
    let (port, seen, proxy) = start_proxy().await;
    let mut sb = BaseCase::new(config(port))
        .await
        .expect("connect to browser");

    sb.open(&format!("http://{SITE}/"))
        .await
        .expect("the page opens");
    sb.click("#out").await.expect("the link opens a tab");
    sb.switch_to_newest_window().await.expect("the new tab");
    let loaded = title_becomes(&mut sb, "second tab").await;
    sb.quit().await.expect("quit");
    proxy.abort();

    loaded.expect("the tab the page opened loaded through the proxy");
    let seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(
        seen.served.contains(&"/second".to_owned()),
        "{:?}",
        seen.served
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_website_that_asks_for_a_password_never_gets_the_proxy_account() {
    let _guard = BROWSER_LOCK.lock().await;
    let (port, seen, proxy) = start_proxy().await;
    let mut sb = BaseCase::new(config(port))
        .await
        .expect("connect to browser");

    // The "website" answers `401` once the proxy has its password.
    let _loaded = sb.open(&format!("http://{SITE}/site-auth")).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    sb.quit().await.expect("quit");
    proxy.abort();

    let seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    assert!(
        seen.proxy_logins >= 1,
        "Chrome never logged in to the proxy"
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

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn the_browsers_command_line_has_no_password() {
    let _guard = BROWSER_LOCK.lock().await;
    let (port, _seen, proxy) = start_proxy().await;
    let mut sb = BaseCase::new(config(port))
        .await
        .expect("connect to browser");

    // `chrome://version` prints the command line Chrome started with.
    sb.open("chrome://version")
        .await
        .expect("the version page opens");
    let command_line = sb
        .get_text("#command_line")
        .await
        .expect("the command line");
    sb.quit().await.expect("quit");
    proxy.abort();

    assert!(
        command_line.contains(&format!("--proxy-server=127.0.0.1:{port}")),
        "{command_line}"
    );
    assert!(!command_line.contains(PASSWORD), "{command_line}");
}

/// No Chrome needed: a remote grid cannot share its browser's DevTools endpoint,
/// so a proxy with a login is refused before a session is requested.
#[tokio::test]
async fn a_remote_grid_with_a_proxy_login_is_refused_before_anything_starts() {
    let config = BrowserConfig {
        headless: true,
        auto_start_driver: false,
        ..BrowserConfig::default()
    }
    .with_proxy(format!("{USER}:{PASSWORD}@proxy.test:8080"))
    .with_webdriver_url("http://USER:KEY@hub.browserstack.com/wd/hub");

    let Err(error) = BaseCase::new(config).await else {
        panic!("a session started on a remote grid with a proxy login");
    };

    let message = error.to_string();
    assert!(
        message.contains("needs a browser this machine starts"),
        "{message}"
    );
    assert!(
        !message.contains(PASSWORD) && !message.contains("KEY"),
        "{message}"
    );
}
