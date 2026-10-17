//! Open a page in UC mode with `uc_open_with_reconnect` and capture what it
//! shows.
//!
//! `uc_open_with_reconnect` reconnects the WebDriver session before it
//! navigates, which helps against sites that fingerprint a session that has
//! been attached for a while. The example opens the page, waits for the
//! document to finish loading, then saves a screenshot to `latest_logs/` and
//! prints the title.
//!
//! By default the page is the demo page this example serves from `127.0.0.1`,
//! so the run needs no network and can check the title. To see how a public
//! bot-detection page treats UC mode, pass its URL; `nowsecure.nl` sits behind
//! a Cloudflare challenge:
//!
//! ```bash
//! cargo run --example uc_stealth                           # offline
//! cargo run --example uc_stealth -- https://nowsecure.nl   # a public page
//! ```
//!
//! What a public page decides depends on the site and on your network, and a
//! challenge can keep working after the load event, so the example asserts
//! nothing about a page it does not control: look at the screenshot. For a
//! deterministic check of UC mode itself, run `uc_mode_test`.
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both); a public URL also needs internet access.

mod local_site;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, DriverMode, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let site = Site::serve_demo()?;
    // The first argument, if there is one, is the public page to try.
    let public_url = std::env::args().nth(1);
    let url = public_url.clone().unwrap_or_else(|| site.url(DEMO_PATH));
    let config = BrowserConfig::default().with_mode(DriverMode::Uc);
    run_browser_test(config, async |sb| demo(sb, url, public_url.is_none()).await).await
}

async fn demo(sb: &mut BaseCase, url: String, own_page: bool) -> Result<()> {
    sb.uc_open_with_reconnect(&url).await?;

    // Wait for the document to finish loading instead of guessing how long
    // that takes.
    sb.wait_for_ready_state_complete().await?;
    sb.wait_for_element_visible("body", 10).await?;

    let title = sb.get_title().await?;
    println!("Title: {title}");
    if own_page {
        ensure(title == "Web Testing Page", "the demo page loaded")?;
    }
    let screenshot = sb.save_screenshot("uc_stealth.png").await?;
    println!("Screenshot saved to {}", screenshot.display());
    Ok(())
}
