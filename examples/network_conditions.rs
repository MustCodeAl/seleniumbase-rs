#![allow(deprecated)]
use seleniumbase_rs::{BaseCase, BrowserConfig};
use thirtyfour::extensions::cdp::{ConnectionType, NetworkConditions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    // Slow down the network to simulate a 3G connection.
    let conditions = NetworkConditions {
        offline: false,
        download_throughput: 1_600_000,
        upload_throughput: 768_000,
        latency: 150,
        connection_type: Some(ConnectionType::Cellular3G),
    };
    sb.set_network_conditions(&conditions).await?;

    sb.open("https://seleniumbase.io/demo_page").await?;
    sb.clear_browser_cache().await?;

    sb.quit().await?;
    Ok(())
}
