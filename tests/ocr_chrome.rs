//! `Ocr::read_page` against a real Chrome driven by the pure CDP engine.
//!
//! The test starts the local Chrome and, on its first run, downloads the two
//! OCR models (about 12 MB) into the cache folder, so it is ignored by default.
//! Run it alone:
//!
//! ```text
//! cargo test --features "ocr test-util" --test ocr_chrome -- --ignored
//! ```
#![cfg(feature = "ocr")]

use std::time::Duration;

use seleniumbase_rs::ocr::Ocr;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

/// A page with nothing on it but one large, plain heading.
const PAGE: &str = "data:text/html,<!doctype html><html><body style=\"background:white\">\
<h1 style=\"font:72px sans-serif;color:black\">Hello OCR 2026</h1></body></html>";

#[tokio::test]
#[ignore = "launches a real Chrome and downloads the OCR models once"]
async fn read_page_finds_the_text_a_page_shows() {
    let ocr = Ocr::load().await.expect("the OCR models load");
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(1000, 600)
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser
        .default_page()
        .await
        .unwrap()
        .with_timeout(Duration::from_secs(15));
    page.goto(PAGE).await.expect("the page opens");

    let text = ocr.read_page(&page).await.expect("the screenshot is read");
    browser.close().await.unwrap();

    assert!(
        text.contains("Hello OCR"),
        "the heading is read from the screenshot; read: {text:?}"
    );
}

#[tokio::test]
#[ignore = "downloads the OCR models once"]
async fn bytes_that_are_not_a_png_are_an_error() {
    let ocr = Ocr::load().await.expect("the OCR models load");
    assert!(ocr.text_in_png(b"not a png").await.is_err());
}
