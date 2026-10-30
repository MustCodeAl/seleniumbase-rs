//! Opt-in live check: does the solver pass https://nowsecure.nl, the public
//! Cloudflare bot-detection test page?
//!
//! **This is for a person to run, not for CI and not for an agent.** It opens a
//! real third-party site that is behind Cloudflare, so it is `#[ignore]`d and it
//! also returns at once unless `SB_LIVE_SITES=1`. A skipped run says so on
//! stderr; it is not a result.
//!
//! "Pass" means the site's own page loaded (not merely that no challenge was
//! seen), any Turnstile on the way was cleared, and a `cf_clearance` cookie is
//! held afterwards. The check is made for both engines that claim stealth:
//!
//! - the Pure CDP engine (`sb_cdp`), and
//! - the UC (undetected) WebDriver engine (`BaseCase` with `DriverMode::Uc`).
//!
//! ```text
//! SB_LIVE_SITES=1 cargo test --test captcha_live_chrome -- --ignored --test-threads=1 --nocapture
//! ```
//!
//! Both run headless unless `SB_HEADED=1`, which shows a window (a person may
//! want to watch). `SB_LIVE_URL` replaces the address, and `SB_LIVE_MARKER` is a
//! script expression that is true on the site's own page, for when the page
//! changes; by default the marker is a page that is not the challenge and that
//! names the site.
//!
//! The same check is one command with the `sbrs` binary:
//!
//! ```text
//! sbrs --uc --url https://nowsecure.nl solve-captcha --expect-text nowsecure --expect-cookie cf_clearance
//! ```
//!
//! If a run fails, keep the printed report: it is what to turn into a fixture
//! that reproduces the gap (`tests/fixtures/captcha/`).

use std::time::{Duration, Instant};

use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, SolveOptions};
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};
use serde_json::Value;

const DEFAULT_URL: &str = "https://nowsecure.nl";

/// How long the real page is given to appear after the solver returns.
const REAL_PAGE_WITHIN: Duration = Duration::from_secs(30);

fn live_enabled() -> bool {
    if std::env::var("SB_LIVE_SITES").as_deref() == Ok("1") {
        return true;
    }
    eprintln!("SKIPPED (not a pass): set SB_LIVE_SITES=1 to open the live site");
    false
}

fn url() -> String {
    std::env::var("SB_LIVE_URL").unwrap_or_else(|_| DEFAULT_URL.to_owned())
}

fn headless() -> bool {
    std::env::var("SB_HEADED").as_deref() != Ok("1")
}

/// A script expression, true when the site's own page is showing.
///
/// Not the challenge (its title is "Just a moment..."), without the challenge's
/// words, and naming the site.
fn marker() -> String {
    std::env::var("SB_LIVE_MARKER").unwrap_or_else(|_| {
        "(() => { const text = (document.body ? document.body.innerText : '').toLowerCase(); \
         return !document.title.startsWith('Just a moment') \
         && !text.includes('verify you are human') \
         && !text.includes('checking your browser') \
         && text.includes('nowsecure'); })()"
            .to_owned()
    })
}

#[tokio::test]
#[ignore = "opens a real third-party site; set SB_LIVE_SITES=1 and run on purpose"]
async fn nowsecure_passes_with_the_pure_cdp_engine() {
    if !live_enabled() {
        return;
    }
    let options = LaunchOptions::builder()
        .headless(headless())
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .expect("launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser.default_page().await.expect("a tab");

    let started = Instant::now();
    page.goto(url()).await.expect("the site loads");
    let solved = page
        .solve_captcha_with(&SolveOptions::default().success_marker(marker()))
        .await;
    println!("sb_cdp: solve_captcha_with -> {solved:?}");

    let on_real_page = page
        .with_timeout(REAL_PAGE_WITHIN)
        .wait_for_function(marker())
        .await
        .is_ok();
    let cookies: Vec<String> = page
        .cookies()
        .all()
        .await
        .expect("cookies")
        .into_iter()
        .map(|cookie| cookie.name)
        .collect();
    let title = page.title().await.unwrap_or_default();
    println!(
        "sb_cdp: after {:.1?}: title {title:?}, real page {on_real_page}, cookies {cookies:?}",
        started.elapsed()
    );
    browser.close().await.ok();

    assert!(
        on_real_page,
        "the site's own page did not load (title {title:?})"
    );
    assert!(
        cookies.iter().any(|name| name == "cf_clearance"),
        "no cf_clearance cookie among {cookies:?}"
    );
}

#[tokio::test]
#[ignore = "opens a real third-party site; set SB_LIVE_SITES=1 and run on purpose"]
async fn nowsecure_passes_with_the_uc_webdriver_engine() {
    if !live_enabled() {
        return;
    }
    let config = BrowserConfig {
        headless: headless(),
        auto_start_driver: true,
        ..BrowserConfig::default()
    }
    .with_mode(DriverMode::Uc);
    let mut sb = BaseCase::new(config).await.expect("start Chrome");

    let started = Instant::now();
    sb.uc_open_with_reconnect(&url())
        .await
        .expect("the site loads");
    let solved = sb
        .solve_captcha_with(&SolveOptions::default().success_marker(marker()))
        .await;
    println!("uc: solve_captcha_with -> {solved:?}");

    let script = format!("return !!({});", marker());
    let deadline = Instant::now() + REAL_PAGE_WITHIN;
    let mut on_real_page = false;
    while Instant::now() < deadline {
        if sb.execute_script(&script).await.ok() == Some(Value::Bool(true)) {
            on_real_page = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let cookies = sb.get_cookies().await.expect("cookies").to_string();
    let title = sb.get_title().await.unwrap_or_default();
    println!(
        "uc: after {:.1?}: title {title:?}, real page {on_real_page}, cookies {cookies}",
        started.elapsed()
    );
    sb.quit().await.ok();

    assert!(
        on_real_page,
        "the site's own page did not load (title {title:?})"
    );
    assert!(
        cookies.contains("cf_clearance"),
        "no cf_clearance cookie among {cookies}"
    );
}
