use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sb = BaseCase::new(BrowserConfig::default()).await?;
    // Add interactions here
    Ok(())
}
