//! Demonstrates launching a browser with a custom anti-detection fingerprint.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example stealth_fingerprint --features playwright
//! ```
//!
//! The example launches Chromium via rustwright with a Windows desktop persona
//! and then prints a few navigator / screen values so you can verify the
//! spoofed values are visible to page JavaScript.

#[cfg(feature = "playwright")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use seleniumbase_rs::browser::playwright::PlaywrightSession;
    use seleniumbase_rs::stealth::fingerprint::Fingerprint;

    let fp = Fingerprint::windows_desktop();
    let session = PlaywrightSession::launch_with_fingerprint(&fp).await?;

    session.goto("about:blank").await?;
    let ua: String = session
        .evaluate("navigator.userAgent")
        .await?
        .as_str()
        .unwrap_or("")
        .to_owned();
    let platform: String = session
        .evaluate("navigator.platform")
        .await?
        .as_str()
        .unwrap_or("")
        .to_owned();
    let width: i64 = session
        .evaluate("screen.width")
        .await?
        .as_i64()
        .unwrap_or(0);
    let height: i64 = session
        .evaluate("screen.height")
        .await?
        .as_i64()
        .unwrap_or(0);

    println!("userAgent: {ua}");
    println!("platform: {platform}");
    println!("screen: {width}x{height}");

    session.close().await?;
    Ok(())
}

#[cfg(not(feature = "playwright"))]
fn main() {
    println!("This example requires the `playwright` feature.");
    println!("Run: cargo run --example stealth_fingerprint --features playwright");
}
