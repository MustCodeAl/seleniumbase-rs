use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    sb.open("https://seleniumbase.io/demo_page").await?;

    // Pierce through a shadow root using the ::shadow combinator.
    sb.shadow_click("my-app ::shadow button#submit").await?;
    sb.shadow_type("my-app ::shadow input#name", "SeleniumBase")
        .await?;

    let text = sb.shadow_get_text("my-app ::shadow .greeting").await?;
    println!("shadow text: {text}");

    sb.quit().await?;
    Ok(())
}
