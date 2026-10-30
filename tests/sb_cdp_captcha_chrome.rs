//! The pure CDP CAPTCHA solver against imitation widgets, in a real headless
//! Chrome.
//!
//! Every page is a local fixture from `tests/fixtures/captcha/`, served on two
//! loopback origins by `tests/support/captcha_site.rs`; no real challenge or
//! bot-protection site is contacted. Each test is a way a real Turnstile page
//! defeated a solver that only measured the widget once and clicked it:
//!
//! - the widget is in a frame from another origin, or in a shadow root;
//! - it appears late, its box draws late, or it is still moving;
//! - its checkbox is at the left edge of the box, not in the middle;
//! - an ordinary checkbox on the form must be left alone;
//! - it needs a second, verified check;
//! - it clears only for a pointer that moved like a hand;
//! - the page after it must really be the site's own page.
//!
//! The tests start a real Chrome, so they are `#[ignore]`d. Run them one binary
//! at a time:
//!
//! ```text
//! cargo test --test sb_cdp_captcha_chrome -- --ignored --test-threads=1
//! ```

#[path = "support/captcha_site.rs"]
mod captcha_site;

use std::time::{Duration, Instant};

use captcha_site::Site;
use seleniumbase_rs::sb_cdp::{Browser, Captcha, LaunchOptions, Page};
use seleniumbase_rs::SeleniumBaseError;

/// How long a test waits for the page to show it cleared.
const CLEAR_WITHIN: Duration = Duration::from_secs(8);

async fn launch() -> (Browser, Page) {
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .expect("launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.expect("a tab");
    (browser, page)
}

/// Whether the page shows it was cleared within `within`.
async fn cleared_within(page: &Page, within: Duration) -> bool {
    page.with_timeout(within)
        .wait_for_function("document.body && document.body.dataset.cleared === '1'")
        .await
        .is_ok()
}

/// Opens `path` on the fixture site and returns the page with a short script
/// timeout.
async fn open(page: &Page, site: &Site, path: &str) {
    page.goto(site.url(path)).await.expect("the fixture loads");
}

/// The text of the failing reason that `human.html` keeps in `window.__verdict`.
async fn verdict(page: &Page) -> String {
    page.evaluate("JSON.stringify(window.__verdict || null)")
        .await
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Presses the widget with `solve_captcha` and asserts the page clears.
async fn solves(path: &str) {
    let site = Site::start();
    let (browser, page) = launch().await;
    open(&page, &site, path).await;

    let attempted = page.solve_captcha().await;
    assert!(
        matches!(attempted, Ok(Some(Captcha::Turnstile))),
        "{path}: solve_captcha returned {attempted:?}"
    );
    assert!(
        cleared_within(&page, CLEAR_WITHIN).await,
        "{path}: the page did not clear; verdict {}",
        verdict(&page).await
    );
    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "starts a headless Chrome"]
async fn a_turnstile_in_a_cross_origin_frame_clears() {
    solves("/iframe").await;
}

#[tokio::test]
#[ignore = "starts a headless Chrome"]
async fn a_turnstile_inside_an_open_shadow_root_is_found_and_clears() {
    solves("/shadow").await;
}

#[tokio::test]
#[ignore = "starts a headless Chrome"]
async fn a_widget_that_appears_late_is_waited_for_and_clears() {
    solves("/late").await;
}

#[tokio::test]
#[ignore = "starts a headless Chrome"]
async fn a_widget_that_is_still_moving_is_pressed_where_it_ends_up() {
    solves("/moving").await;
}

#[tokio::test]
#[ignore = "starts a headless Chrome"]
async fn a_widget_that_needs_a_second_verified_check_clears() {
    solves("/iframe?widget=fail=1").await;
}

#[tokio::test]
#[ignore = "starts a headless Chrome"]
async fn a_widget_that_judges_the_pointer_path_clears_for_a_human_path() {
    solves("/human").await;
}

#[tokio::test]
#[ignore = "starts a headless Chrome"]
async fn the_press_lands_on_the_checkbox_at_the_left_edge_not_the_middle() {
    let site = Site::start();
    let (browser, page) = launch().await;
    open(&page, &site, "/iframe").await;

    page.solve_captcha().await.expect("solve_captcha");
    assert!(cleared_within(&page, CLEAR_WITHIN).await, "did not clear");

    let clicks = page
        .evaluate("JSON.stringify(window.__clicks)")
        .await
        .expect("the clicks");
    let clicks: Vec<serde_json::Value> =
        serde_json::from_str(clicks.as_str().expect("a string")).expect("json");
    let first = clicks.first().expect("the widget saw a press");
    assert_eq!(first["trusted"], true, "{first}");
    assert_eq!(first["inside"], true, "the press missed the box: {first}");
    // The box spans 12..36 across a 300 wide widget.
    let x = first["x"].as_f64().expect("x");
    assert!(
        (12.0..=36.0).contains(&x),
        "the press was at {x}, not on the left-edge box"
    );
    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "starts a headless Chrome"]
async fn an_ordinary_form_checkbox_is_never_clicked() {
    let site = Site::start();
    let (browser, page) = launch().await;

    // A page with checkboxes and no challenge at all: nothing is pressed.
    open(&page, &site, "/decoy?widget=none").await;
    let none = page.solve_captcha().await.expect("solve_captcha");
    assert_eq!(none, None, "there is no widget on the page");
    let presses: u32 = page
        .evaluate_as("window.__mouse")
        .await
        .expect("press count");
    assert_eq!(
        presses, 0,
        "the solver pressed something on a page without a widget"
    );

    // The same form with a widget: the widget is solved, the boxes are left.
    open(&page, &site, "/decoy").await;
    page.solve_captcha().await.expect("solve_captcha");
    assert!(cleared_within(&page, CLEAR_WITHIN).await, "did not clear");
    let boxes: [bool; 2] = page
        .evaluate_as("[document.getElementById('newsletter').checked, document.getElementById('terms').checked]")
        .await
        .expect("checkbox state");
    assert_eq!(boxes, [false, false], "a form checkbox was ticked");
    browser.close().await.expect("Chrome closes");
}

#[tokio::test]
#[ignore = "starts a headless Chrome"]
async fn an_interstitial_clears_to_the_real_page_with_a_clearance_cookie() {
    let site = Site::start();
    let (browser, page) = launch().await;
    open(&page, &site, "/interstitial").await;

    page.solve_captcha().await.expect("solve_captcha");

    // A positive marker of the site's own page, not merely "no challenge".
    page.with_timeout(CLEAR_WITHIN)
        .wait_for_function("document.getElementById('real-page')")
        .await
        .expect("the real page loaded");
    let cookies = page.cookies().all().await.expect("cookies");
    assert!(
        cookies.iter().any(|cookie| cookie.name == "cf_clearance"),
        "no cf_clearance cookie among {:?}",
        cookies.iter().map(|c| c.name.as_str()).collect::<Vec<_>>()
    );
    browser.close().await.expect("Chrome closes");
}

/// The fixtures the table measures, with their path and the time they get.
const MEASURED: [(&str, &str); 7] = [
    ("frame", "/iframe"),
    ("shadow root", "/shadow"),
    ("late widget", "/late"),
    ("moving widget", "/moving"),
    ("second check", "/iframe?widget=fail=1"),
    ("human path", "/human"),
    ("interstitial", "/interstitial"),
];

/// Success rate and time of `solve_captcha` over the fixtures.
///
/// Not an assertion: it prints a table (run with `--nocapture`) for the
/// numbers in the changelog. A trial succeeds when the page shows it cleared
/// (or, for the interstitial, shows the real page) within 8 seconds of the
/// call returned (the time counted is from before the call).
#[tokio::test]
#[ignore = "measures, starts a headless Chrome; run with --nocapture"]
async fn measure_success_rate_and_time_over_the_fixtures() {
    const TRIALS: u32 = 5;
    let site = Site::start();
    let (browser, _first) = launch().await;

    println!("\nfixture          success   mean time (successes)");
    let (mut won_all, mut total_all) = (0_u32, 0_u32);
    for (name, path) in MEASURED {
        let (mut won, mut spent) = (0_u32, Duration::ZERO);
        for _ in 0..TRIALS {
            let page = browser.new_page().await.expect("a tab");
            open(&page, &site, path).await;
            let started = Instant::now();
            let outcome: Result<_, SeleniumBaseError> = page.solve_captcha().await;
            let marker = if path == "/interstitial" {
                "document.getElementById('real-page')"
            } else {
                "document.body && document.body.dataset.cleared === '1'"
            };
            let cleared = outcome.is_ok()
                && page
                    .with_timeout(Duration::from_secs(8))
                    .wait_for_function(marker)
                    .await
                    .is_ok();
            if cleared {
                won += 1;
                spent += started.elapsed();
            }
            page.close().await.ok();
        }
        total_all += TRIALS;
        won_all += won;
        let mean = if won > 0 {
            format!("{:.2}s", spent.as_secs_f64() / f64::from(won))
        } else {
            "-".to_owned()
        };
        println!("{name:<16} {won}/{TRIALS}      {mean}");
    }
    println!("{:<16} {won_all}/{total_all}\n", "total");
    browser.close().await.expect("Chrome closes");
}
