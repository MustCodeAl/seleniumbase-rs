//! Throttle the network to see how a page behaves on a slow connection.
//!
//! `set_network_conditions` asks Chrome to add latency and cap throughput for
//! every request the page makes. The example loads the same page with and
//! without added latency, prints both load times, and fails unless the
//! throttled load was the slower one. It then puts the connection back to
//! normal.
//!
//! Throttling goes through the Chrome DevTools Protocol, so the browser must be
//! started in `DriverMode::Cdp` (or `Uc`); in plain WebDriver mode the call
//! returns an "CDP is unavailable" error. Throughput is in **bytes** per
//! second, and `-1` means "do not limit".
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example network_conditions
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network: the page is served from
//! `127.0.0.1` by `local_site/mod.rs`.

// The crate's API takes thirtyfour's chromedriver `NetworkConditions`, which
// thirtyfour has deprecated in favour of its typed CDP domain.
#![expect(
    deprecated,
    reason = "BaseCase::set_network_conditions takes thirtyfour's NetworkConditions"
)]

mod local_site;

use std::time::{Duration, Instant};

use local_site::{Site, DEMO_PATH};
use seleniumbase_rs::tokio;

use seleniumbase_rs::drivers::{ConnectionType, NetworkConditions};
use seleniumbase_rs::{
    run_browser_test, BaseCase, BrowserConfig, DriverMode, Result, SeleniumBaseError,
};

/// Every request waits this long before its response starts to arrive.
const ADDED_LATENCY_MS: u32 = 400;

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    let config = BrowserConfig::default().with_mode(DriverMode::Cdp);
    run_browser_test(config, async |sb| demo(sb, url).await).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    let normal = timed_load(sb, &url).await?;
    println!("normal connection:    {normal:?}");

    // Roughly Chrome's "Fast 3G" preset, in bytes per second.
    sb.set_network_conditions(&NetworkConditions {
        offline: false,
        latency: ADDED_LATENCY_MS,
        download_throughput: 200_000, // 1.6 Mbit/s
        upload_throughput: 94_000,    // 750 kbit/s
        connection_type: Some(ConnectionType::Cellular3G),
    })
    .await?;
    let slow = timed_load(sb, &url).await?;
    println!("throttled connection: {slow:?}");

    // Back to normal: no latency, no throughput limits.
    sb.set_network_conditions(&NetworkConditions::new()).await?;

    if slow < Duration::from_millis(u64::from(ADDED_LATENCY_MS)) {
        return Err(SeleniumBaseError::AssertionFailed(format!(
            "the throttled load took {slow:?}, less than the {ADDED_LATENCY_MS} ms of added latency"
        )));
    }
    Ok(())
}

/// How long `open` takes to return once the cache is empty.
async fn timed_load(sb: &mut BaseCase, url: &str) -> Result<Duration> {
    sb.clear_browser_cache().await?;
    let started = Instant::now();
    sb.open(url).await?;
    sb.assert_text("Demo Page", "h1").await?;
    Ok(started.elapsed())
}
