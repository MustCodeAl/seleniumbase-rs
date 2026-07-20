use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sb = BaseCase::new(BrowserConfig::default()).await?;

    sb.show_message("Welcome", "Welcome to the SeleniumBase Rust demo!");

    if sb.show_confirm("Continue?", "Do you want to continue?") {
        let result = sb.show_prompt("Your name", "What is your name?", Some("guest"));
        let name = result.text.unwrap_or_else(|| "guest".to_string());
        sb.show_message("Hello", &format!("Hello, {name}!"));
    }

    sb.quit().await?;
    Ok(())
}
