//! Real-browser checks of the presentation and chart helpers
//! (`create_presentation`, `add_slide`, `save_presentation`, `create_*_chart`,
//! `add_data_point`, `display_chart`, `extract_chart`). The same helpers are
//! checked without a browser in `src/api/base_case_impls/base_case_impl_charts.rs`
//! and `base_case_impl_presentations.rs`.
//!
//! These tests are ignored by default. They need Chrome and the network, because
//! the saved pages load reveal.js and Chart.js from a CDN. Run them locally:
//!
//! ```bash
//! cargo test --test presentation_chart_chrome -- --ignored --test-threads=1
//! ```
//!
//! The browser is headless, where nobody watches. So `begin_presentation` shows
//! nothing and returns `None` (checked below), and the presentation that is played
//! by itself is saved, opened and watched by the test instead of `begin_presentation`.
//!
//! Each test works in a folder of its own, because the helpers save into
//! `saved_presentations/` and `saved_charts/` of the working directory.
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

use std::time::{Duration, Instant};

use seleniumbase_rs::api::presentation::{PresentationTheme, Transition};
use seleniumbase_rs::{BaseCase, BrowserConfig};

/// Serializes browser sessions, and the changes of the working directory, so
/// parallel tests do not race.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

/// Moves the process into a new folder, so that the saved files do not land in
/// the crate. The caller keeps the folder alive for the whole test.
fn work_in_a_temporary_folder() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temporary folder");
    std::env::set_current_dir(dir.path()).expect("move into it");
    dir
}

/// Waits until `css` is visible, or `limit` has passed.
async fn becomes_visible(sb: &BaseCase, css: &str, limit: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < limit {
        if sb.is_element_visible(css).await.unwrap_or(false) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and the network"]
async fn a_displayed_chart_is_drawn_and_a_headless_browser_waits_one_second() {
    let _guard = BROWSER_LOCK.lock().await;
    let _dir = work_in_a_temporary_folder();
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");

    sb.create_pie_chart()
        .title("Browsers")
        .unit("%")
        .await
        .unwrap();
    sb.add_data_point("Chrome", 60.0).await.unwrap();
    sb.add_data_point("Firefox", 25.5)
        .color("orange")
        .await
        .unwrap();
    let started = Instant::now();
    let path = sb.display_chart().await.expect("the chart is displayed");
    let waited = started.elapsed();

    assert!(
        path.ends_with("saved_charts/my_chart.html"),
        "{}",
        path.display()
    );
    assert!(path.is_file());
    assert!(waited >= Duration::from_secs(1), "waited {waited:?}");
    assert!(waited < Duration::from_secs(30), "waited {waited:?}");
    assert!(becomes_visible(&sb, "canvas#sb-chart-default", Duration::from_secs(10)).await);
    let drawn = sb
        .execute_script(
            "return typeof Chart !== 'undefined' && !!Chart.getChart('sb-chart-default');",
        )
        .await
        .unwrap();
    assert_eq!(drawn, serde_json::json!(true), "Chart.js drew the canvas");
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and the network"]
async fn two_named_charts_are_extracted_into_one_page_and_both_are_drawn() {
    let _guard = BROWSER_LOCK.lock().await;
    let dir = work_in_a_temporary_folder();
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");

    sb.create_column_chart()
        .chart_name("sales")
        .title("Sales")
        .await
        .unwrap();
    sb.add_data_point("Q1", 100.0)
        .chart_name("sales")
        .await
        .unwrap();
    sb.create_line_chart()
        .chart_name("trend")
        .libs(false)
        .zero(true)
        .await
        .unwrap();
    sb.add_data_point("Q1", 1.5)
        .chart_name("trend")
        .await
        .unwrap();
    let sales = sb.extract_chart().chart_name("sales").await.unwrap();
    let trend = sb.extract_chart().chart_name("trend").await.unwrap();

    // Chart.js once, from the first chart, then both charts.
    let page = dir.path().join("two.html");
    std::fs::write(&page, format!("<html><body>{sales}{trend}</body></html>")).unwrap();
    sb.open_html_file(page.to_str().unwrap()).await.unwrap();
    assert!(becomes_visible(&sb, "canvas#sb-chart-sales", Duration::from_secs(10)).await);
    let both = sb
        .execute_script(
            "return !!Chart.getChart('sb-chart-sales') && !!Chart.getChart('sb-chart-trend');",
        )
        .await
        .unwrap();
    assert_eq!(both, serde_json::json!(true));
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver and the network"]
async fn a_presentation_with_an_interval_plays_by_itself_and_a_chart_on_a_slide_is_drawn() {
    let _guard = BROWSER_LOCK.lock().await;
    let _dir = work_in_a_temporary_folder();
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");

    sb.create_pie_chart()
        .chart_name("slide")
        .title("On a slide")
        .await
        .unwrap();
    sb.add_data_point("A", 1.0)
        .chart_name("slide")
        .await
        .unwrap();
    let chart = sb.extract_chart().chart_name("slide").await.unwrap();
    sb.create_presentation()
        .theme(PresentationTheme::Night)
        .transition(Transition::Fade)
        .await
        .unwrap();
    sb.add_slide(&format!("<h1 id=\"one\">One</h1>{chart}"))
        .notes("first")
        .await
        .unwrap();
    sb.add_slide("<h1 id=\"two\">Two</h1>")
        .code("let x = 1 < 2;")
        .await
        .unwrap();
    let path = sb
        .save_presentation()
        .interval(Duration::from_millis(700))
        .await
        .expect("the presentation is saved");
    assert!(
        path.ends_with("saved_presentations/my_presentation.html"),
        "{}",
        path.display()
    );

    sb.open_html_file(path.to_str().unwrap()).await.unwrap();
    assert!(becomes_visible(&sb, "section.present #one", Duration::from_secs(15)).await);
    let drawn = sb
        .execute_script("return !!Chart.getChart('sb-chart-slide');")
        .await
        .unwrap();
    assert_eq!(
        drawn,
        serde_json::json!(true),
        "the chart on the first slide is drawn"
    );
    // The deck moves on to the second slide without any key being pressed.
    assert!(
        becomes_visible(&sb, "section.present #two", Duration::from_secs(20)).await,
        "the interval plays the deck by itself"
    );
    sb.quit().await.expect("quit");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn begin_presentation_shows_nothing_in_a_headless_browser() {
    let _guard = BROWSER_LOCK.lock().await;
    let dir = work_in_a_temporary_folder();
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");

    sb.add_slide("<h1>Hi</h1>").await.unwrap();
    let started = Instant::now();
    let shown = sb.begin_presentation().await.expect("nothing to fail");
    assert_eq!(shown, None);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        !dir.path().join("saved_presentations").exists(),
        "no file was written either"
    );
    sb.quit().await.expect("quit");
}
