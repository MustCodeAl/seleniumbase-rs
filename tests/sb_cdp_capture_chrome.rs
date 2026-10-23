//! `Page::capture_fingerprint` against a real Chrome.
//!
//! The mock tests (`tests/sb_cdp_capture.rs`) check the mapping and the
//! plumbing; this checks that the capture script runs in a real browser, reads
//! what the browser really reports, and that a fingerprint applied to a Chrome is
//! read back from it. The tests need a Chrome, so they are ignored by default.
//! Run them one at a time with:
//!
//! ```text
//! cargo test --test sb_cdp_capture_chrome -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! `SB_RECORD_PROBE=path.json` also writes the raw readings of the first test, which
//! is how `tests/fixtures/fingerprint/probe_chrome_real_*.json` were recorded.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use seleniumbase_rs::stealth::capture::capture_expression;
use seleniumbase_rs::{BrowserType, Fingerprint, Spoof};

const PAGE: &str = "<!doctype html><title>capture</title><p>capture</p>";

/// Serves `PAGE` on a loopback port. `127.0.0.1` is a secure context, the only
/// kind of page Chrome gives `navigator.userAgentData` and the battery.
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

async fn chrome(fingerprint: Option<&Fingerprint>) -> (Browser, Page) {
    let mut options = LaunchOptions::builder().headless(true);
    if let Some(fingerprint) = fingerprint {
        options = options.fingerprint(fingerprint);
    }
    let browser = Browser::launch(options.build().expect("options"))
        .await
        .expect("Chrome starts");
    let page = browser.default_page().await.expect("a tab");
    page.goto(&serve()).await.expect("the loopback page loads");
    (browser, page)
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn a_real_chrome_is_captured_and_presents_what_it_captured() {
    let (browser, page) = chrome(None).await;

    let captured = page.capture_fingerprint().await.expect("the capture runs");
    let fp = &captured.fingerprint;

    // What the browser reports through plain evaluation is what was captured.
    let user_agent: String = page.evaluate_as("navigator.userAgent").await.unwrap();
    let cores: u32 = page
        .evaluate_as("navigator.hardwareConcurrency")
        .await
        .unwrap();
    let width: u32 = page.evaluate_as("screen.width").await.unwrap();
    let zone: String = page
        .evaluate_as("Intl.DateTimeFormat().resolvedOptions().timeZone")
        .await
        .unwrap();
    assert_eq!(fp.user_agent().as_str(), Some(user_agent.as_str()));
    assert_eq!(fp.hardware_concurrency().value(), Some(&cores));
    assert_eq!(fp.screen().value().map(|s| s.width), Some(width));
    assert_eq!(fp.timezone().as_str(), Some(zone.as_str()));

    // It is a Chromium on this machine's system, and it claims the Chrome it is.
    assert_eq!(fp.browser_type(), BrowserType::Chromium);
    let major: u32 = page
        .evaluate_as("parseInt(navigator.userAgent.match(/Chrome\\/(\\d+)/)[1], 10)")
        .await
        .unwrap();
    assert_eq!(fp.core_version(), Some(major));
    assert!(
        fp.client_hints().is_value(),
        "a secure page has Client Hints"
    );
    assert!(fp.webgl().is_value() || captured.unanswered().iter().any(|(s, _)| *s == "webgl"));
    assert!(fp.media_devices().is_value());
    assert!(captured.probe.url.starts_with("http://127.0.0.1:"));
    assert!(captured.probe.browser_version.is_some());

    // It is coherent, and the file is the one file format.
    let report = fp.validate();
    assert!(report.is_coherent(), "{:?}", report.errors);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("real.json");
    fp.save(&path).unwrap();
    assert_eq!(&Fingerprint::load(&path).unwrap(), fp);

    // The page presents what was captured from it.
    let comparison = page.compare_fingerprint(fp).await.unwrap();
    println!("{}", fp.to_json_pretty().unwrap());
    println!(
        "unmapped: {}",
        serde_json::to_string_pretty(&captured.unmapped).unwrap()
    );
    println!("comparison: {comparison:#?}");
    assert!(comparison.presents(), "{:?}", comparison.differences);

    if let Ok(target) = std::env::var("SB_RECORD_PROBE") {
        let readings = page.evaluate(capture_expression()).await.unwrap();
        std::fs::write(&target, serde_json::to_string_pretty(&readings).unwrap()).unwrap();
        println!("recorded the readings in {target}");
    }
    browser.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires a local Chrome"]
async fn a_chrome_launched_with_a_fingerprint_shows_it_to_a_capture() {
    let claimed = Fingerprint::windows_desktop()
        .to_builder()
        .timezone("Asia/Tokyo")
        .locale("ja-JP")
        .languages("ja-JP,ja")
        .battery(Spoof::Off)
        .build()
        .unwrap();
    let (browser, page) = chrome(Some(&claimed)).await;

    let captured = page.capture_fingerprint().await.expect("the capture runs");
    let comparison = claimed.compare_to(&captured.fingerprint);

    println!("comparison: {comparison:#?}");
    let fp = &captured.fingerprint;
    assert_eq!(fp.timezone().as_str(), Some("Asia/Tokyo"));
    assert_eq!(fp.locale().as_str(), Some("ja-JP"));
    assert_eq!(fp.user_agent(), claimed.user_agent());
    assert!(
        fp.battery().is_off(),
        "the claimed missing battery is missing"
    );
    assert!(fp.webgl().is_value());
    assert!(
        comparison.presents(),
        "the page does not show what was claimed: {:?}",
        comparison.differences
    );
    browser.close().await.unwrap();
}
