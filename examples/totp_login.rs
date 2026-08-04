use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    // Example secret; load from environment in real tests.
    let secret = std::env::var("TOTP_SECRET").unwrap_or_else(|_| "JBSWY3DPEHPK3PXP".into());

    sb.open("https://seleniumbase.io/demo_page").await?;

    let code = sb.get_totp_code(&secret)?;
    println!("Generated TOTP: {code}");

    sb.quit().await?;
    Ok(())
}
