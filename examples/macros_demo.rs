use seleniumbase_rs::{sb_assert_title, sb_click, sb_open, sb_quit, BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    sb_open!(sb, "https://example.com", "open example.com");
    sb_assert_title!(sb, "Example Domain", "title mismatch");
    sb_click!(sb, "a", "click more info link");
    sb_quit!(sb, "close browser");

    Ok(())
}
