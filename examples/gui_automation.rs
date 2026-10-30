//! Type and use shortcuts with the operating system's own keyboard, through
//! the `gui_*` methods of `BaseCase`.
//!
//! Native input is a last resort for things WebDriver cannot reach, such as a
//! native file dialog or a page that distrusts synthetic events. It goes to
//! whichever window has the focus, so the example:
//!
//! 1. opens the browser **with a visible window** (headless Chrome has no
//!    window for the OS to type into),
//! 2. gives the text field focus with WebDriver and checks that it has it,
//!    *before* sending any native key, and
//! 3. reads the field back afterwards, failing if the text did not arrive.
//!
//! It then types a sentence, selects it all with the platform's shortcut, and
//! deletes it. The same family also has `gui_click_x_y`, `gui_click_element`,
//! `gui_hover_x_y` and `gui_drag_and_drop`; they move the real pointer, and
//! `gui_click_element` aims at the element's position within the *page*, which
//! can sit a toolbar's height away from where the window puts it, so prefer
//! WebDriver clicks and keep native input for what WebDriver cannot do.
//!
//! Run it with the `gui` feature, which is off by default because it links the
//! desktop automation stack (enigo, and X11 on Linux):
//!
//! ```bash
//! cargo run --example gui_automation --features gui
//! ```
//!
//! **This takes over your keyboard for a moment.** Leave the browser window in
//! front and do not type while it runs. It needs Chrome and a matching
//! chromedriver on your `PATH`, a desktop session, and on macOS the
//! Accessibility permission for your terminal (System Settings > Privacy &
//! Security > Accessibility). It does not need the network.

mod local_site;

use local_site::Site;
use seleniumbase_rs::{
    run_browser_test, tokio, BaseCase, BrowserConfig, Result, SeleniumBaseError,
};

const PAGE: &str = r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>GUI automation</title></head>
<body>
  <label for="note">Note</label>
  <input id="note" autocomplete="off">
</body>
</html>
"#;

/// The modifier of the "select all" shortcut on this platform.
const MODIFIER: &str = if cfg!(target_os = "macos") {
    "command"
} else {
    "ctrl"
};

#[tokio::main]
async fn main() -> Result<()> {
    let url = Site::serve(&[("/", PAGE)])?.url("/");
    // Native input needs a window to land in.
    let config = BrowserConfig::default().with_headless(false);
    run_browser_test(config, async |sb| demo(sb, url).await).await
}

async fn demo(sb: &mut BaseCase, url: String) -> Result<()> {
    sb.open(&url).await?;

    // Focus with WebDriver, and confirm it before the OS types anything.
    sb.click("#note").await?;
    let focused = sb
        .execute_script("return document.activeElement.id")
        .await?;
    if focused.as_str() != Some("note") {
        return Err(SeleniumBaseError::AssertionFailed(
            "the text field does not have focus, so no native keys were sent".to_owned(),
        ));
    }

    sb.gui_write("Hello from the OS keyboard")?;
    wait_for_value(sb, "Hello from the OS keyboard").await?;
    println!("typed: {}", sb.get_value("#note").await?);

    // Select everything with the platform shortcut, then delete it.
    sb.gui_press_keys(&[MODIFIER, "a"])?;
    sb.gui_press_key("backspace")?;
    wait_for_value(sb, "").await?;
    println!("selected all and deleted it");
    Ok(())
}

/// Native keystrokes reach the page a moment after they are sent, so poll
/// briefly for the field to hold `expected`.
async fn wait_for_value(sb: &mut BaseCase, expected: &str) -> Result<()> {
    for _ in 0..30 {
        if sb.get_value("#note").await? == expected {
            return Ok(());
        }
        sb.sleep(0.1).await;
    }
    Err(SeleniumBaseError::AssertionFailed(format!(
        "the text field never held '{expected}'; was the browser window in front?"
    )))
}
