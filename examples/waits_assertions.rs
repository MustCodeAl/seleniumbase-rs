use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig {
        mode: DriverMode::Uc,
        ..Default::default()
    })
    .await?;

    sb.open("https://seleniumbase.io/demo_page").await?;

    // Explicit waits
    sb.wait_for_element_present("body", 10).await?;
    sb.wait_for_element_visible("h1", 10).await?;

    // Text assertions
    sb.assert_text_visible("Demo Page", "h1").await?;
    sb.assert_text_not_visible("Error", "body").await?;

    // Element assertions
    sb.assert_element_present("#myButton").await?;
    sb.assert_element_visible("#myButton").await?;

    // Title assertions
    sb.assert_title_contains("Demo").await?;

    // Deferred assertions
    sb.deferred_assert_element("#myInput").await?;
    sb.deferred_assert_text("This", "body").await?;
    sb.process_deferred_asserts().await?;

    sb.quit().await?;
    Ok(())
}
