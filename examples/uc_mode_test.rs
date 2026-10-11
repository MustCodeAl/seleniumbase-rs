//! Check that UC (undetected) mode hides the usual automation markers.
//!
//! Plain WebDriver leaves several tells for a page to find: `navigator.webdriver`
//! is `true`, headless Chrome puts `HeadlessChrome` in its user agent, and
//! chromedriver injects `cdc_` globals into every page. UC mode removes them.
//! The example opens the same local page in both modes, reads the three
//! markers from page JavaScript, prints them side by side, and fails if UC mode
//! still shows any of them.
//!
//! This is a check against pages you control. How a particular third-party
//! site judges your browser is a different question: see `uc_stealth` for a
//! run against a public bot-detection page.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example uc_mode_test
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbase doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

mod local_site;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode, Result};

/// The automation markers a page can read.
#[derive(Debug)]
struct Markers {
    /// `navigator.webdriver === true`.
    webdriver_flag: bool,
    /// `HeadlessChrome` appears in `navigator.userAgent`.
    headless_user_agent: bool,
    /// How many `cdc_` globals chromedriver injected into `window`.
    cdc_globals: u64,
}

impl Markers {
    fn none_found(&self) -> bool {
        !self.webdriver_flag && !self.headless_user_agent && self.cdc_globals == 0
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);

    let plain = probe(BrowserConfig::default(), &url).await?;
    println!("WebDriver mode: {plain:?}");

    let config = BrowserConfig::default().with_mode(DriverMode::Uc);
    let undetected = probe(config, &url).await?;
    println!("UC mode:        {undetected:?}");

    ensure(
        undetected.none_found(),
        "UC mode shows no automation markers",
    )
}

/// Starts a browser with `config`, reads the markers, and closes the browser.
async fn probe(config: BrowserConfig, url: &str) -> Result<Markers> {
    let mut sb = BaseCase::new(config).await?;
    let markers = read_markers(&mut sb, url).await;
    let closed = sb.quit().await;
    let markers = markers?;
    closed?;
    Ok(markers)
}

async fn read_markers(sb: &mut BaseCase, url: &str) -> Result<Markers> {
    sb.open(url).await?;
    let found = sb
        .execute_script(
            "return {
                webdriver: navigator.webdriver === true,
                headlessUa: navigator.userAgent.includes('HeadlessChrome'),
                cdcGlobals: Object.keys(window).filter(k => /^\\$?cdc_/.test(k)).length
            };",
        )
        .await?;
    Ok(Markers {
        webdriver_flag: found["webdriver"].as_bool().unwrap_or(true),
        headless_user_agent: found["headlessUa"].as_bool().unwrap_or(true),
        cdc_globals: found["cdcGlobals"].as_u64().unwrap_or(u64::MAX),
    })
}
