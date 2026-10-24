//! Cookies and `localStorage`: set them, read them back, and move them through
//! a JSON file.
//!
//! The example sets a cookie and reads it back, saves the cookie jar to a JSON
//! file, deletes the cookie, restores it from the file, and then does the same
//! kind of round trip with a `localStorage` item. Every value is checked, so a
//! run that finishes has proved the calls worked.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example cookies_storage
//! ```
//!
//! It needs Chrome and a matching chromedriver on your `PATH` (`sbrs doctor`
//! checks both) and does not need the network. Cookies and `localStorage` need
//! a real origin, so the page is served over HTTP from `127.0.0.1` by
//! `local_site/mod.rs`; a `data:` or `about:blank` page would refuse both.
//!
//! `BaseCase::get_cookies` (every cookie as JSON) goes through the Chrome
//! DevTools Protocol, so it needs `DriverMode::Cdp`; `get_cookie` by name works
//! in every mode.

mod local_site;

use local_site::{ensure, Site, DEMO_PATH};
use seleniumbase_rs::{run_browser_test, BaseCase, BrowserConfig, CookieExpiry, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve_demo()?.url(DEMO_PATH);
    // `run_browser_test` closes the browser even if a check below fails.
    run_browser_test(BrowserConfig::default(), async |sb| demo(sb, url).await).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    // ---- Cookies ----
    sb.add_cookie(&serde_json::json!({ "name": "session", "value": "abc123" }))
        .await?;
    let cookie = sb.get_cookie("session").await?;
    ensure(cookie.value == "abc123", "the cookie keeps its value")?;
    println!("cookie: {}={}", cookie.name, cookie.value);

    // Save the jar, delete the cookie, and bring it back from the file.
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("cookies.json");
    sb.save_cookies_to_path(&file).await?;
    sb.delete_cookie("session").await?;
    ensure(
        sb.get_cookie("session").await.is_err(),
        "the deleted cookie is gone",
    )?;
    // `Saved` keeps the expiry the cookie was saved with; `load_cookies(name)`
    // would make it a session cookie, as Python's does.
    sb.load_cookies_from_path(&file, CookieExpiry::Saved)
        .await?;
    ensure(
        sb.get_cookie("session").await?.value == "abc123",
        "the cookie is restored from the file",
    )?;
    println!("cookie restored from {}", file.display());

    // ---- Local storage ----
    sb.set_local_storage_item("username", "demo_user").await?;
    let username = sb.get_local_storage_item("username").await?;
    ensure(
        username.as_str() == Some("demo_user"),
        "the stored value reads back",
    )?;
    println!("username from local storage: {username}");

    sb.remove_local_storage_item("username").await?;
    ensure(
        sb.get_local_storage_item("username").await?.is_null(),
        "a removed item reads back as null",
    )?;

    sb.set_local_storage_item("theme", "dark").await?;
    sb.clear_local_storage().await?;
    ensure(
        sb.get_local_storage_item("theme").await?.is_null(),
        "clearing storage removes every item",
    )?;
    println!("local storage cleared");
    Ok(())
}
