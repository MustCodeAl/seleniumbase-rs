use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    // Cookies
    sb.add_cookie("session", "abc123").await?;
    let value = sb.get_cookie("session").await?;
    println!("session cookie: {value:?}");

    // Local storage
    sb.set_local_storage_item("username", "demo_user").await?;
    let username = sb.get_local_storage_item("username").await?;
    println!("username from local storage: {username:?}");

    sb.clear_local_storage().await?;

    sb.quit().await?;
    Ok(())
}
