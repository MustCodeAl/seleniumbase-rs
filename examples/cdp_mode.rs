use std::path::Path;

use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig {
        mode: DriverMode::Cdp,
        ..Default::default()
    })
    .await?;

    sb.cdp_open("https://seleniumbase.io").await?;
    let title = sb.cdp_get_title().await?;
    let heading = sb.cdp_get_text("h1").await?;
    println!("title: {title}");
    println!("heading: {heading}");

    sb.cdp_click("a[href='/demo_page']").await.ok();
    sb.cdp_screenshot(Path::new("cdp_screenshot.png"))
        .await
        .ok();

    sb.quit().await?;
    Ok(())
}
