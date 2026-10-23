//! `BaseCase::capture_fingerprint` against a real Chrome driven through
//! chromedriver, and the same page read by the pure CDP engine.
//!
//! The mock tests check the plumbing; this checks that the one capture script runs
//! as WebDriver's asynchronous script in a real browser and that the two engines
//! read the same browser the same way. It needs Chrome and a matching chromedriver
//! (`CHROMEDRIVER_PATH` names one, or `downloaded_drivers/` holds it), so it is
//! ignored by default. Run it with:
//!
//! ```text
//! cargo test --test base_case_capture_chrome -- --ignored --nocapture
//! ```

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{BaseCase, BrowserConfig, BrowserType};

const PAGE: &str = "<!doctype html><title>capture</title><p>capture</p>";

/// Serves `PAGE` on a loopback port. `127.0.0.1` is a secure context.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{PAGE}",
                PAGE.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}/")
}

#[tokio::test]
#[ignore = "requires a local Chrome and chromedriver"]
async fn webdriver_and_the_pure_cdp_engine_read_the_same_chrome_the_same_way() {
    let url = serve();

    let mut sb = BaseCase::new(BrowserConfig::default().with_headless(true))
        .await
        .expect("Chrome starts through chromedriver");
    sb.open(&url).await.unwrap();
    let through_webdriver = sb.capture_fingerprint().await.expect("the capture runs");
    sb.quit().await.unwrap();

    let browser = Browser::launch(LaunchOptions::builder().headless(true).build().unwrap())
        .await
        .expect("Chrome starts");
    let page = browser.default_page().await.unwrap();
    page.goto(&url).await.unwrap();
    let through_cdp = page.capture_fingerprint().await.expect("the capture runs");
    browser.close().await.unwrap();

    let (wd, cdp) = (&through_webdriver.fingerprint, &through_cdp.fingerprint);
    println!(
        "settings that differ between the engines: {:?}",
        cdp.diff(wd).iter().map(|d| d.setting).collect::<Vec<_>>()
    );
    println!("webdriver unanswered: {:?}", through_webdriver.unanswered());
    assert_eq!(wd.browser_type(), BrowserType::Chromium);
    assert!(
        wd.client_hints().is_value(),
        "a secure page has Client Hints"
    );
    assert!(wd.validate().is_coherent(), "{:?}", wd.validate().errors);
    // The same browser reads the same through either engine.
    for (name, a, b) in [
        ("platform", wd.platform(), cdp.platform()),
        ("vendor", wd.vendor(), cdp.vendor()),
        ("locale", wd.locale(), cdp.locale()),
        ("languages", wd.languages(), cdp.languages()),
        ("timezone", wd.timezone(), cdp.timezone()),
        ("product_sub", wd.product_sub(), cdp.product_sub()),
    ] {
        assert_eq!(a, b, "{name}");
    }
    assert_eq!(wd.hardware_concurrency(), cdp.hardware_concurrency());
    // Chrome started by chromedriver has no WebGL in headless mode here; the capture
    // then says so and does not invent the strings.
    if wd.webgl().is_natural() {
        assert!(
            through_webdriver
                .unanswered()
                .iter()
                .any(|(setting, _)| *setting == "webgl"),
            "{:?}",
            through_webdriver.unanswered()
        );
    } else {
        assert_eq!(wd.webgl(), cdp.webgl());
    }
    assert_eq!(wd.core_version(), cdp.core_version());
    assert_eq!(
        wd.client_hints()
            .value()
            .map(|h| h.platform_version.clone()),
        cdp.client_hints()
            .value()
            .map(|h| h.platform_version.clone())
    );
    // A page driven through chromedriver says so.
    assert_eq!(
        through_webdriver.unmapped["webdriver"],
        serde_json::json!(true)
    );
}
