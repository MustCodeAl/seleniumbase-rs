use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig {
        mode: DriverMode::Uc,
        ..BrowserConfig::default()
    };
    let mut sb = BaseCase::new(config).await?;
    sb.open("https://nowsecure.nl").await?;
    sb.quit().await?;
    Ok(())
}
