use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default();
    let mut sb = BaseCase::new(config).await?;

    sb.open("https://example.com").await?;

    // Window introspection
    let (x, y, width, height) = sb.get_window_rect().await?;
    println!("Window: x={x}, y={y}, width={width}, height={height}");

    // Page artifacts
    let screenshot = sb.save_screenshot("example.png").await?;
    println!("Screenshot saved to {}", screenshot.display());
    let source = sb.save_page_source("example.html").await?;
    println!("Page source saved to {}", source.display());

    // On-page messaging
    sb.post_success_message("All good!").await?;
    sb.wait(1).await;

    // Referral helper
    let referral = sb.generate_referral("https://example.com", "rust_docs")?;
    println!("Referral URL: {referral}");

    // Console log capture
    sb.console_log_script().await?;
    sb.console_log_string("Manual log entry").await?;

    // Multi-series chart
    sb.create_bar_chart("Sample Chart").await?;
    sb.add_data_point("Jan", 100).await?;
    sb.add_data_point("Feb", 150).await?;
    sb.add_series_to_chart("Revenue", &[("Jan".into(), 100), ("Feb".into(), 150)])
        .await?;
    let chart_path = sb.display_chart().await?;
    println!("Chart displayed at {}", chart_path.display());

    sb.quit().await?;
    Ok(())
}
