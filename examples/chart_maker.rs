use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    sb.create_pie_chart("Browser Market Share").await?;
    sb.add_data_point("Chrome", 60).await?;
    sb.add_data_point("Firefox", 25).await?;
    sb.add_data_point("Safari", 15).await?;
    sb.save_chart("market_share.html").await?;

    println!("Chart saved to market_share.html");
    sb.quit().await?;
    Ok(())
}
