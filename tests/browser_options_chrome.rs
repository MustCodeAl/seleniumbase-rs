//! Real-browser checks of the browser options: mobile mode and the Chromium
//! switches.
//!
//! Mobile mode (`--mobile`, `--metrics`) must give the page a phone: the
//! viewport width, the device pixel ratio, a touch screen and an Android user
//! agent, in the WebDriver mode (the `mobileEmulation` capability) and in the
//! undetected mode (the same phone sent over DevTools). The switches are read
//! back from `chrome://version`, which prints the command line Chrome started
//! with.
//!
//! These tests are ignored by default. Run them locally with Chrome
//! installed, one binary at a time:
//!
//! ```bash
//! cargo test --test browser_options_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a chromedriver you started instead of the one
//! `BaseCase` starts.

use seleniumbase_rs::browser::config::DriverMode;
use seleniumbase_rs::browser::options::DeviceMetrics;
use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A page with a `viewport` meta tag, as a mobile-friendly page has: without
/// one a phone lays the page out 980 pixels wide whatever its screen is.
const PAGE: &str = "data:text/html,<html><head><title>phone</title>\
    <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
    </head><body>hi</body></html>";

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

/// What the page sees of the screen and the browser.
const PROBE: &str = "return { width: window.innerWidth, ratio: window.devicePixelRatio, \
    touch: navigator.maxTouchPoints, agent: navigator.userAgent };";

/// Opens the page with `config` and returns what the page sees.
async fn probe(config: BrowserConfig) -> serde_json::Value {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = BaseCase::new(config).await.expect("connect to browser");
    sb.open(PAGE).await.expect("open the page");
    let seen = sb.execute_script(PROBE).await.expect("the probe runs");
    sb.quit().await.expect("quit");
    seen
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn mobile_alone_gives_the_default_phone() {
    let seen = probe(test_config().with_mobile(true)).await;
    assert_eq!(seen["width"], 390, "{seen}");
    assert_eq!(seen["ratio"], 3.0, "{seen}");
    assert!(seen["touch"].as_u64().unwrap() > 0, "{seen}");
    assert!(
        seen["agent"].as_str().unwrap().contains("Android"),
        "{seen}"
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn metrics_choose_the_screen_and_turn_mobile_on() {
    let seen = probe(test_config().with_device_metrics(DeviceMetrics::new(360, 640, 2.0))).await;
    assert_eq!(seen["width"], 360, "{seen}");
    assert_eq!(seen["ratio"], 2.0, "{seen}");
    assert!(seen["touch"].as_u64().unwrap() > 0, "{seen}");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn the_undetected_mode_gets_the_same_phone_over_devtools() {
    let seen = probe(test_config().with_mode(DriverMode::Uc).with_mobile(true)).await;
    assert_eq!(seen["width"], 390, "{seen}");
    assert_eq!(seen["ratio"], 3.0, "{seen}");
    assert!(seen["touch"].as_u64().unwrap() > 0, "{seen}");
    assert!(
        seen["agent"].as_str().unwrap().contains("Android"),
        "{seen}"
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_desktop_session_is_not_a_phone() {
    let seen = probe(test_config()).await;
    assert!(seen["width"].as_u64().unwrap() > 390, "{seen}");
    assert!(
        !seen["agent"].as_str().unwrap().contains("Android"),
        "{seen}"
    );
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn the_switches_are_on_the_command_line_chrome_started_with() {
    let _guard = BROWSER_LOCK.lock().await;
    let config = test_config()
        .with_dark_mode(true)
        .with_incognito(true)
        .with_disable_ws(true)
        .with_chromium_arg("mute-audio,disable-features=Translate")
        .with_disable_features(["MediaRouter"])
        .with_host_resolver_rules("MAP blocked.test 127.0.0.1")
        .with_locale("ar_eg");
    let mut sb = BaseCase::new(config).await.expect("connect to browser");
    sb.open("chrome://version")
        .await
        .expect("open chrome://version");
    let line = sb
        .execute_script("return document.getElementById('command_line').textContent;")
        .await
        .expect("read the command line");
    let line = line.as_str().expect("the command line is text");
    for wanted in [
        "--enable-features=WebContentsForceDark",
        "--incognito",
        "--disable-web-security",
        "--ignore-certificate-errors",
        "--mute-audio",
        "--host-resolver-rules=MAP blocked.test 127.0.0.1",
        "--lang=ar-EG",
    ] {
        assert!(line.contains(wanted), "{wanted} is not in {line}");
    }
    // The two lists of features are one switch, which Chrome would otherwise
    // read only the last of.
    let disabled: Vec<&str> = line
        .split_whitespace()
        .filter(|part| part.starts_with("--disable-features="))
        .collect();
    assert_eq!(disabled.len(), 1, "{line}");
    assert!(disabled[0].contains("Translate") && disabled[0].contains("MediaRouter"));
    sb.quit().await.expect("quit");
}

/// A proxy on a free port that answers every request with a small page, and
/// sends the first line of each request it was asked for down the channel.
async fn fake_proxy() -> (u16, tokio::sync::mpsc::UnboundedReceiver<String>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the proxy");
    let port = listener.local_addr().expect("the proxy's address").port();
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let sender = sender.clone();
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut chunk = [0_u8; 1024];
                while !head.windows(4).any(|window| window == b"\r\n\r\n") {
                    match stream.read(&mut chunk).await {
                        Ok(read) if read > 0 => head.extend_from_slice(&chunk[..read]),
                        _ => return,
                    }
                }
                let text = String::from_utf8_lossy(&head);
                let _ = sender.send(text.lines().next().unwrap_or_default().to_owned());
                let body = "<html><body>through the proxy</body></html>";
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes()).await;
            });
        }
    });
    (port, receiver)
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn chrome_loads_a_page_through_the_proxy_and_its_command_line_has_no_login() {
    let _guard = BROWSER_LOCK.lock().await;
    let (port, mut asked) = fake_proxy().await;
    let config = test_config().with_proxy(format!("user:secret@127.0.0.1:{port}"));
    let mut sb = BaseCase::new(config).await.expect("connect to browser");

    sb.open("http://sbrs-proxy-check.test/")
        .await
        .expect("open a page through the proxy");
    let text = sb
        .execute_script("return document.body.textContent;")
        .await
        .expect("read the page");
    assert_eq!(text.as_str(), Some("through the proxy"), "{text}");
    let request = asked.try_recv().expect("the proxy was asked for the page");
    assert!(
        request.starts_with("GET http://sbrs-proxy-check.test/ HTTP/1.1"),
        "{request}"
    );

    sb.open("chrome://version")
        .await
        .expect("open chrome://version");
    let line = sb
        .execute_script("return document.getElementById('command_line').textContent;")
        .await
        .expect("read the command line");
    let line = line.as_str().expect("the command line is text");
    assert!(
        line.contains(&format!("--proxy-server=127.0.0.1:{port}")),
        "{line}"
    );
    assert!(!line.contains("secret"), "{line}");
    sb.quit().await.expect("quit");
}
