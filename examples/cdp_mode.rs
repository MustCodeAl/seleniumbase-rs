//! Drive Chrome over the DevTools Protocol (CDP) instead of through WebDriver
//! commands. Two ways are shown, on the same page:
//!
//! 1. **CDP mode in `BaseCase`** (`DriverMode::Cdp`). The session is still
//!    started through chromedriver, but the `cdp_*` helpers (`cdp_open`,
//!    `cdp_click`, `cdp_type`, `cdp_get_text`, ...) talk to Chrome over CDP.
//!    `Page.navigate` returns before the page has loaded, so the example waits
//!    for an element with `cdp_wait_for_element_present` rather than sleeping.
//! 2. **Pure CDP** (`seleniumbase_rs::sb_cdp`). Chrome is launched directly, with
//!    no chromedriver at all and none of the markers WebDriver leaves behind.
//!    Locators wait for their element, and `expect()` assertions retry until
//!    they hold.
//!
//! Each part checks its own results and fails the example if they are wrong.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example cdp_mode
//! ```
//!
//! Part 1 needs Chrome and a matching chromedriver on your `PATH` (`sbrs
//! doctor` checks both); part 2 needs only Chrome. Neither needs the network:
//! the page is served from `127.0.0.1` by `local_site/mod.rs`. The screenshot is
//! written to `latest_logs/`.

mod local_site;

use std::time::Duration;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::artifacts::ensure_latest_logs_dir;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, DriverMode, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);

    println!("== CDP mode in BaseCase ==");
    let config = BrowserConfig::default().with_mode(DriverMode::Cdp);
    let page_url = url.clone();
    run_browser_test(config, async |sb| base_case_cdp(sb, page_url).await).await?;

    println!("\n== Pure CDP ==");
    pure_cdp(&url).await
}

async fn base_case_cdp(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.cdp_open(&url).await?;
    sb.cdp_wait_for_element_present("h1", Duration::from_secs(10))
        .await?;

    let title = sb.cdp_get_title().await?;
    let heading = sb.cdp_get_text("h1").await?;
    println!("title: {title}");
    println!("heading: {heading}");
    ensure(
        title == "Web Testing Page",
        "the CDP title is the page title",
    )?;
    ensure(
        heading == "Demo Page",
        "the CDP heading is the page heading",
    )?;

    sb.cdp_type("#myTextInput", "typed over CDP").await?;
    sb.cdp_click("#myButton").await?;
    let sentence = sb.cdp_get_text("#pText").await?;
    println!("after the click: {sentence}");
    ensure(
        sentence.contains("Purple"),
        "the click changed the sentence",
    )?;

    let screenshot = ensure_latest_logs_dir()?.join("cdp_screenshot.png");
    sb.cdp_screenshot(&screenshot).await?;
    println!("screenshot: {}", screenshot.display());
    Ok(())
}

async fn pure_cdp(url: &str) -> Result<()> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let outcome = async {
        let page = browser.default_page().await?;
        page.goto(url).await?;
        println!("title: {}", page.title().await?);

        // A locator waits for its element, so there is no sleep and no
        // separate wait.
        page.locator("#myTextInput")
            .fill("typed with pure CDP")
            .await?;
        page.locator("#myButton").click().await?;

        // An expectation retries until it holds or times out.
        page.locator("#pText")
            .expect()
            .to_contain_text("Purple")
            .await?;
        page.expect().to_have_title("Web Testing Page").await?;
        println!("the click changed the sentence");
        Ok(())
    }
    .await;
    let closed = browser.close().await;
    outcome.and(closed)
}
