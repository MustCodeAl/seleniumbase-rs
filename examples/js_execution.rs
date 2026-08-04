use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    let title = sb
        .execute_script("return document.title")
        .await?
        .to_string();
    println!("Page title from JS: {title}");

    sb.execute_script("document.body.style.backgroundColor = 'yellow'")
        .await?;
    sb.sleep(1.0).await;

    sb.quit().await?;
    Ok(())
}
