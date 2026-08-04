use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    // Move the mouse to screen coordinates and click.
    sb.gui_hover_x_y(500, 400)?;
    sb.gui_click_x_y(500, 400)?;

    // Type a string using the OS keyboard.
    sb.gui_write("Hello from SeleniumBase Rust!")?;

    // Press a key combination.
    sb.gui_press_keys(&["command", "a"])?;

    sb.quit().await?;
    Ok(())
}
