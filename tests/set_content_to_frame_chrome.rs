//! A real-browser check of `BaseCase::set_content_to_frame` and
//! `set_content_to_default`.
//!
//! Python SeleniumBase replaces the page with the content of an iframe that is
//! on it, so the page's own commands work on that content, and
//! `set_content_to_default` puts the page back. When the iframe's `src` is a web
//! address, the address opens in a new tab instead. The page below holds one
//! iframe of each kind and is served from the loopback interface.
//!
//! This test is ignored by default. Run it locally with Chrome and a matching
//! chromedriver installed:
//!
//! ```bash
//! cargo test --test set_content_to_frame_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The page: a heading, an iframe with its content inline, and an iframe whose
/// `src` is a page of the same server.
const TOP_PAGE: &str = "<!doctype html><html><head><title>Top</title></head><body>\
    <h1 id=\"top\">Top</h1>\
    <iframe id=\"inline\" srcdoc=\"&lt;p id=inside&gt;Inside the frame&lt;/p&gt;\"></iframe>\
    <iframe id=\"remote\" src=\"/inner\"></iframe>\
    </body></html>";

/// The page the second iframe holds.
const INNER_PAGE: &str =
    "<!doctype html><html><head><title>Inner</title></head><body><p id=\"inner\">Inner page</p></body></html>";

fn test_config() -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    };
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

/// Answers one connection with [`TOP_PAGE`] at `/` and [`INNER_PAGE`] at
/// `/inner`.
fn answer(mut stream: TcpStream) {
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or("/");
    let (status, body) = match path {
        "/" => ("200 OK", TOP_PAGE),
        "/inner" => ("200 OK", INNER_PAGE),
        _ => ("404 Not Found", ""),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Starts the site and returns its `http://127.0.0.1:port` base.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let base = format!("http://{}", listener.local_addr().expect("bound address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            // A thread each: Chrome opens idle connections ahead of time.
            std::thread::spawn(move || answer(stream));
        }
    });
    base
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_frame_replaces_the_page_and_default_brings_the_page_back() {
    let _guard = BROWSER_LOCK.lock().await;
    let base = serve();
    let top = format!("{base}/");
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    sb.open(&top).await.expect("open the page");
    assert!(sb.is_element_present("#top").await.unwrap());
    assert!(
        !sb.is_element_present("#inside").await.unwrap(),
        "the frame's content is not on the page itself"
    );

    // An iframe with inline content takes the place of the page.
    sb.set_content_to_frame("#inline")
        .await
        .expect("the frame replaces the page");
    assert!(
        sb.is_element_present("#inside").await.unwrap(),
        "the page now holds what the frame held"
    );
    assert!(
        !sb.is_element_present("#top").await.unwrap(),
        "the page's own content is gone"
    );
    assert_eq!(sb.get_text("#inside").await.unwrap(), "Inside the frame");
    assert_eq!(
        sb.get_current_url().await.unwrap(),
        top,
        "the address does not change"
    );

    sb.set_content_to_default()
        .await
        .expect("the page comes back");
    assert!(sb.is_element_present("#top").await.unwrap());
    assert!(!sb.is_element_present("#inside").await.unwrap());
    assert_eq!(sb.get_text("#top").await.unwrap(), "Top");

    // An iframe whose `src` is a web address opens in a new tab.
    let first_tab = sb.get_current_window_handle().await.unwrap();
    sb.set_content_to_frame("#remote")
        .await
        .expect("the frame opens in a tab");
    assert_eq!(sb.get_current_url().await.unwrap(), format!("{base}/inner"));
    assert!(sb.is_element_present("#inner").await.unwrap());
    assert_ne!(sb.get_current_window_handle().await.unwrap(), first_tab);

    sb.set_content_to_default()
        .await
        .expect("the first tab comes back");
    assert_eq!(sb.get_current_window_handle().await.unwrap(), first_tab);
    assert_eq!(sb.get_current_url().await.unwrap(), top);
    assert!(sb.is_element_present("#top").await.unwrap());

    // A frame that does not exist is reported, and the page is left alone.
    sb.set_timeout(1).await.unwrap();
    let error = sb
        .set_content_to_frame("#nope")
        .await
        .expect_err("there is no such frame");
    assert!(error.to_string().contains("#nope"), "{error}");
    assert!(sb.is_element_present("#top").await.unwrap());

    sb.quit().await.expect("the browser closes");
}
