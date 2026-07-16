use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default();
    let mut sb = BaseCase::new(config).await?;
    sb.open("https://google.com/ncr").await?;
    sb.post_message("SeleniumBase Rust demo", 2).await?;
    sb.highlight("textarea[name='q'], input[name='q']").await?;
    println!("Loaded: {}", sb.get_title().await?);
    let screenshot = sb.save_screenshot_to_logs().await?;
    let source = sb.save_page_source_to_logs().await?;
    println!("Saved screenshot: {}", screenshot.display());
    println!("Saved source: {}", source.display());
    sb.quit().await?;
    Ok(())
}
