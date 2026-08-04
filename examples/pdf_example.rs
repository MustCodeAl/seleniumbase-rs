use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    sb.save_as_pdf("demo_page.pdf").await?;
    let text = sb.get_pdf_text("demo_page.pdf")?;
    println!("Extracted text length: {}", text.len());

    sb.assert_pdf_text("demo_page.pdf", "SeleniumBase")?;

    sb.quit().await?;
    Ok(())
}
