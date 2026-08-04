use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    sb.save_screenshot("demo_page.png").await?;
    sb.save_screenshot_to_logs().await?;
    sb.save_page_source("demo_page.html").await?;
    sb.save_page_source_to_logs().await?;

    println!("Artifacts saved.");
    sb.quit().await?;
    Ok(())
}
