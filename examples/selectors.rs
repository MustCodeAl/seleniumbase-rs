use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig {
        mode: DriverMode::Uc,
        ..Default::default()
    })
    .await?;

    sb.open("https://seleniumbase.io/demo_page").await?;

    // CSS selector
    sb.click("#myButton").await?;

    // XPath selector
    let text = sb.get_text("//h1").await?;
    println!("h1 text: {text}");

    // Link text selector
    // sb.click("link=Some Link").await?;

    // Partial link text selector
    // sb.click("partial link=Some").await?;

    sb.quit().await?;
    Ok(())
}
