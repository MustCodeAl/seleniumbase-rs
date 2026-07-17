use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default().with_mode(DriverMode::Uc);
    let mut sb = BaseCase::new(config).await?;

    // Cloudflare turnstile test page
    sb.open("https://nowsecure.nl").await?;
    sb.sleep(5.0).await;

    sb.save_screenshot("uc_stealth.png").await?;

    sb.quit().await?;
    Ok(())
}
