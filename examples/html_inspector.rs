use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    sb.open("https://seleniumbase.io/demo_page").await?;
    let inspection = sb.inspect_html().await?;

    if inspection.is_clean() {
        println!("No HTML inspector issues found.");
    } else {
        for issue in &inspection.issues {
            println!("[{}] {}", issue.rule, issue.message);
        }
    }

    sb.quit().await?;
    Ok(())
}
