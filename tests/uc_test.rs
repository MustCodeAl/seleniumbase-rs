use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::test]
#[ignore] // Ignoring because it requires a local selenium server to be running
async fn test_uc_stealth() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = BrowserConfig::default();
    config.mode = DriverMode::Uc;
    // ensure we are using the local selenium server
    config.webdriver_url = "http://localhost:4444".to_string();

    let mut sb = BaseCase::new(config).await?;

    // We can use a service like https://bot.sannysoft.com/
    sb.open("https://bot.sannysoft.com/").await?;
    sb.wait_for_element_visible("table", 10).await?;

    // Let's dump the page source to check if it says we are a bot
    let source = sb.get_page_source().await?;
    println!("Source length: {}", source.len());

    sb.quit().await?;
    Ok(())
}
