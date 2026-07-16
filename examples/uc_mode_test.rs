use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = BrowserConfig::default();
    config.mode = DriverMode::Uc;
    let mut sb = BaseCase::new(config).await?;
    sb.open("https://nowsecure.nl").await?;
    sb.quit().await?;
    Ok(())
}
