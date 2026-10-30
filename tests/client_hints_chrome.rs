//! What the installed Chrome really reports in its Client Hints, read through
//! the Pure CDP engine and compared with what the stealth tables claim for the
//! same major (`evasion::randomize::chromium_hints` and
//! `KNOWN_FULL_VERSIONS`).
//!
//! This is the measurement behind the rows marked `MeasuredOnMac`. It needs a
//! branded Google Chrome (Chromium has no `Google Chrome` brand), so it is
//! ignored by default. Run it with the output shown:
//!
//! ```text
//! cargo test --test client_hints_chrome -- --ignored --nocapture
//! ```
//!
//! It starts Chrome as `LaunchOptions` does by default: headed on macOS and
//! Windows, headless on Linux, where the user agent says `HeadlessChrome`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::evasion::randomize::{chromium_hints, known_full_version, FullVersionSource};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use serde_json::Value;

const PAGE: &str = "<!doctype html><title>hints</title><p>client hints</p>";

/// Serves `PAGE` on a loopback port. `127.0.0.1` is a secure context, the only
/// kind of page Chrome gives `navigator.userAgentData`.
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

/// Everything the page can read about its user agent.
const READ_HINTS: &str = r"(async () => {
  const data = navigator.userAgentData;
  const high = await data.getHighEntropyValues([
    'platformVersion', 'fullVersionList', 'uaFullVersion',
    'architecture', 'bitness', 'model', 'wow64',
  ]);
  return {
    userAgent: navigator.userAgent,
    brands: data.brands,
    mobile: data.mobile,
    platform: data.platform,
    high,
  };
})()";

/// `[{brand, version}]` as `(brand, version)` pairs, in order.
fn pairs(list: &Value) -> Vec<(String, String)> {
    list.as_array()
        .expect("a brand list")
        .iter()
        .map(|entry| {
            (
                entry["brand"].as_str().unwrap_or_default().to_owned(),
                entry["version"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// The macOS version as `sw_vers` prints it, padded to the three parts Chrome
/// reports (`27.0` becomes `27.0.0`).
fn macos_version() -> Option<String> {
    let output = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    let version = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    let mut parts: Vec<&str> = version.split('.').collect();
    while parts.len() < 3 {
        parts.push("0");
    }
    Some(parts.join("."))
}

#[tokio::test]
#[ignore = "needs a branded Google Chrome; run with --ignored --nocapture"]
async fn the_installed_chrome_reports_what_the_stealth_tables_claim() {
    let url = serve();
    let options = LaunchOptions::builder()
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .expect("options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.expect("a tab");
    page.goto(&url).await.expect("the page loads");
    let seen = page
        .evaluate(READ_HINTS)
        .await
        .expect("the hints are readable");
    browser.close().await.expect("Chrome closes");

    println!(
        "measured: {}",
        serde_json::to_string_pretty(&seen).expect("JSON")
    );

    let user_agent = seen["userAgent"].as_str().expect("a user agent");
    let brands = pairs(&seen["brands"]);
    let full_list = pairs(&seen["high"]["fullVersionList"]);
    let full_version = seen["high"]["uaFullVersion"].as_str().unwrap_or_default();
    let (_, chrome_version) = full_list
        .iter()
        .find(|(brand, _)| brand == "Google Chrome")
        .expect("a branded Google Chrome");
    let major: u32 = chrome_version
        .split('.')
        .next()
        .and_then(|m| m.parse().ok())
        .expect("a numeric major");
    println!("Chrome major {major}, full version {full_version}");

    // The user agent keeps the frozen `major.0.0.0`.
    assert!(
        user_agent.contains(&format!("Chrome/{major}.0.0.0")),
        "{user_agent}"
    );

    // The brand list, its decoy, order and versions are what the generator
    // builds for this major.
    let expected = chromium_hints(major);
    let expected_brands: Vec<(String, String)> = expected
        .brands
        .iter()
        .map(|b| (b.brand.clone(), b.version.clone()))
        .collect();
    assert_eq!(brands, expected_brands, "the brand list of Chrome {major}");
    let expected_names: Vec<&str> = expected
        .full_version_list
        .iter()
        .map(|b| b.brand.as_str())
        .collect();
    let seen_names: Vec<&str> = full_list.iter().map(|(brand, _)| brand.as_str()).collect();
    assert_eq!(seen_names, expected_names, "the full version list's order");
    let decoy = full_list
        .iter()
        .find(|(brand, _)| brand.starts_with("Not"))
        .expect("a decoy brand");
    assert!(
        decoy.1.ends_with(".0.0.0"),
        "the decoy's full version: {decoy:?}"
    );

    // The full version: a table row marked as measured must be this build.
    match known_full_version(major) {
        Some(row) if row.source == FullVersionSource::MeasuredOnMac => assert_eq!(
            full_version, row.full_version,
            "KNOWN_FULL_VERSIONS says {} was measured; this Chrome reports {full_version}. \
             Chrome updated itself: measure again and update the row.",
            row.full_version
        ),
        Some(row) => println!(
            "the table's {} for {major} comes from Google's releases; this Chrome reports {full_version}",
            row.full_version
        ),
        None => println!("the table has no row for {major}; this Chrome reports {full_version}"),
    }
    assert_eq!(
        chrome_version, full_version,
        "uaFullVersion matches the list"
    );

    if cfg!(target_os = "macos") {
        assert_eq!(seen["platform"], "macOS");
        // Chromium freezes the macOS token, even on Apple Silicon and macOS 27.
        assert!(
            user_agent.contains("(Macintosh; Intel Mac OS X 10_15_7)"),
            "{user_agent}"
        );
        let platform_version = seen["high"]["platformVersion"].as_str().unwrap_or_default();
        let os = macos_version().expect("sw_vers");
        println!("macOS {os}, platformVersion {platform_version}");
        assert_eq!(
            platform_version, os,
            "major.minor.patch of the running macOS"
        );
        assert_eq!(seen["high"]["architecture"], "arm");
        assert_eq!(seen["high"]["bitness"], "64");
    }
}
