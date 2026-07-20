use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

#[tokio::main]
async fn main() -> Result<()> {
    run_browser_test(BrowserConfig::default(), |sb| {
        Box::pin(async move {
            sb.open("https://example.com").await?;
            sb.assert_title("Example Domain").await
        })
    })
    .await
}
