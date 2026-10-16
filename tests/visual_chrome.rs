//! Visual regression checks (feature `visual`) against a real Chrome.
//!
//! The first test drives Chrome through the pure CDP engine and checks that a
//! page matches its own baseline, that a rewritten heading fails with the
//! actual and diff images written, and prints the scores it saw, which are the
//! measurements behind the default threshold. The second runs
//! `BaseCase::assert_visual_match` through chromedriver next to
//! `check_window`, on a change only the pixels show.
//!
//! Both are ignored by default. Run them one at a time with Chrome installed:
//!
//! ```bash
//! cargo test --features "visual test-util" --test visual_chrome -- --ignored --test-threads=1 --nocapture
//! ```
#![cfg(feature = "visual")]

use std::path::PathBuf;
use std::time::Duration;

use seleniumbase_rs::api::run_settings::RunSettings;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use seleniumbase_rs::visual::{VisualBaseline, VisualOutcome, DEFAULT_THRESHOLD};
use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};

/// A page with a heading, a line of small print and a coloured block.
fn page_with(heading: &str, small_print: &str) -> String {
    format!(
        "data:text/html,<!doctype html><html><body style=\"margin:20px;background:%23fff\">\
         <h1 style=\"font:bold 48px sans-serif;color:%23123\">{heading}</h1>\
         <p style=\"font:16px sans-serif\">{small_print}</p>\
         <div style=\"width:200px;height:80px;background:%233a7\"></div></body></html>"
    )
}

async fn launch() -> (Browser, Page) {
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .window_size(800, 600)
        .build()
        .unwrap();
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser
        .default_page()
        .await
        .unwrap()
        .with_timeout(Duration::from_secs(15));
    (browser, page)
}

/// The similarity of the page now to the baseline `name`, whatever it is.
async fn score(baselines: &VisualBaseline, name: &str, page: &Page) -> f64 {
    match baselines
        .clone()
        .with_threshold(0.0)
        .check_page(name, page)
        .await
    {
        Ok(VisualOutcome::Matched { score }) => score,
        other => panic!("expected a score, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_page_matches_its_baseline_and_a_rewritten_heading_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let baselines = VisualBaseline::new(dir.path());
    let (browser, page) = launch().await;

    page.goto(&page_with(
        "Visual check",
        "The small print stays the same.",
    ))
    .await
    .expect("the page opens");
    let created = baselines.check_page("page", &page).await.unwrap();
    assert!(
        matches!(created, VisualOutcome::Created { .. }),
        "{created:?}"
    );
    let same = baselines.check_page("page", &page).await.unwrap();
    let VisualOutcome::Matched { score: same } = same else {
        panic!("the unchanged page matches: {same:?}");
    };
    eprintln!("unchanged page: {same}");

    // One word of the small print: reported, not asserted (see the module docs).
    page.goto(&page_with(
        "Visual check",
        "The small print stays the safe.",
    ))
    .await
    .unwrap();
    let one_word = score(&baselines, "page", &page).await;
    eprintln!("one word of 16px text changed: {one_word}");

    page.goto(&page_with(
        "Visual regression",
        "The small print stays the same.",
    ))
    .await
    .unwrap();
    let heading = score(&baselines, "page", &page).await;
    eprintln!("48px heading rewritten: {heading}");
    let error = baselines.check_page("page", &page).await.unwrap_err();
    browser.close().await.unwrap();

    assert!(same >= DEFAULT_THRESHOLD, "unchanged page scored {same}");
    assert!(
        heading < DEFAULT_THRESHOLD,
        "rewritten heading scored {heading}"
    );
    let SeleniumBaseError::AssertionFailed(message) = error else {
        panic!("expected an assertion failure, got {error:?}");
    };
    eprintln!("{message}");
    for file in ["page.png", "page.actual.png", "page.diff.png"] {
        let bytes = std::fs::read(dir.path().join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(bytes.starts_with(b"\x89PNG"), "{file} is a PNG");
    }
}

/// The name of the test below: `BaseCase` keeps its baselines in a folder
/// named after the running test.
const BASE_CASE_TEST: &str = "assert_visual_match_sees_a_colour_change_check_window_does_not";

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn assert_visual_match_sees_a_colour_change_check_window_does_not() {
    let folder = PathBuf::from("visual_baseline").join(BASE_CASE_TEST);
    let _ = std::fs::remove_dir_all(&folder);
    let config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    };
    let mut sb = BaseCase::new(config).await.expect("connect to browser");
    sb.open(&page_with("Visual check", "Hello"))
        .await
        .expect("open the page");

    sb.check_window("home", 3.0).await.expect("layout baseline");
    sb.assert_visual_match("home")
        .await
        .expect("pixel baseline");
    assert!(folder.join("home.png").is_file(), "the baseline is saved");
    sb.assert_visual_match("home")
        .await
        .expect("the unchanged page matches");

    sb.execute_script("document.body.style.background = 'red'; return 1;")
        .await
        .expect("the script runs");
    let layout = sb.check_window("home", 3.0).await;
    let pixels = sb.assert_visual_match("home").await;

    // A reset makes the red page the baseline.
    sb.set_run_settings(RunSettings::default().with_visual_baseline_reset(true));
    sb.assert_visual_match("home").await.expect("reset");
    sb.set_run_settings(RunSettings::default());
    let after_reset = sb.assert_visual_match("home").await;
    sb.quit().await.expect("quit");
    let diff_written = folder.join("home.diff.png").is_file();
    let _ = std::fs::remove_dir_all(&folder);
    // The parent folder too, but only when no other test left anything in it.
    let _ = std::fs::remove_dir("visual_baseline");

    assert!(layout.is_ok(), "check_window compares tags: {layout:?}");
    assert!(
        matches!(pixels, Err(SeleniumBaseError::AssertionFailed(_))),
        "assert_visual_match compares pixels: {pixels:?}"
    );
    assert!(after_reset.is_ok(), "{after_reset:?}");
    assert!(
        !diff_written,
        "the match after the reset removed the old diff"
    );
}
