use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    sb.start_qa_session("Login flow manual verification");

    sb.open("https://seleniumbase.io/demo_page").await?;
    sb.manual_verify("The demo page loaded without errors");
    sb.manual_verify("The heading is visible and readable");

    let steps = sb.qa_session().map(|s| s.steps.clone()).unwrap_or_default();
    let report = sb.save_qa_report(&steps, "Login flow manual verification", None::<&str>)?;
    println!("Report written to: {}", report.display());

    sb.quit().await?;
    Ok(())
}
