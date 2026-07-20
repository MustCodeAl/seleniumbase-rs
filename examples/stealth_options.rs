use std::collections::HashMap;

use seleniumbase_rs::stealth::{dprocess::find_chromedriver, options::StealthOptions};
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut headers = HashMap::new();
    headers.insert("Accept-Language".to_owned(), "en-US,en;q=0.9".to_owned());

    let opts = StealthOptions {
        uc: true,
        headless: true,
        user_agent: Some(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/126.0.0.0 Safari/537.36"
                .to_owned(),
        ),
        window_size: Some("1920,1080".to_owned()),
        extra_headers: headers,
        ..Default::default()
    };

    let config = BrowserConfig {
        headless: opts.headless,
        user_agent: opts.user_agent.clone(),
        mode: DriverMode::Uc,
        ..Default::default()
    };

    println!("chromedriver path: {:?}", find_chromedriver());
    println!("launch args: {:?}", opts.args());

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://nowsecure.nl").await?;
    sb.sleep(5.0).await;
    sb.save_screenshot("stealth_options.png").await?;
    sb.quit().await?;
    Ok(())
}
