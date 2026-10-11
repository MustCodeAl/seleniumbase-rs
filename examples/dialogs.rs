//! The same native dialogs as the `dialog` example, called as `BaseCase`
//! methods, plus the file chooser and save dialogs.
//!
//! `BaseCase` wraps the dialog functions so a test can pause for a human
//! without importing anything else: `show_message`, `show_confirm`,
//! `show_prompt` (which returns a `DialogResult`), `choose_file_dialog` and
//! `save_file_dialog`. None of them touches the browser, so this example builds
//! a `BaseCase` with `BaseCase::without_session` instead of launching Chrome.
//!
//! `show_prompt` cannot collect typed text: the `rfd` crate has no text-entry
//! dialog, so OK returns the default you passed and the result is unconfirmed
//! if the dialog is dismissed any other way.
//!
//! Run it with the `gui` feature, which is off by default because it links the
//! desktop dialog stack (GTK and X11 on Linux):
//!
//! ```bash
//! cargo run --example dialogs --features gui
//! ```
//!
//! It needs a desktop session, because every call blocks until you answer the
//! dialog, so it cannot run in CI or over SSH without a display. Without the
//! feature Cargo refuses to run it and names the missing feature, and
//! `cargo build --examples` skips it.

use seleniumbase_rs::{BaseCase, BrowserConfig};

fn main() {
    let sb = BaseCase::without_session(BrowserConfig::default());

    sb.show_message("Welcome", "Welcome to the SeleniumBase Rust demo!");

    if !sb.show_confirm("Continue?", "Do you want to continue?") {
        println!("You chose not to continue.");
        return;
    }

    let result = sb.show_prompt(
        "Your name",
        "Press OK to be greeted as 'guest'.",
        Some("guest"),
    );
    let name = if result.confirmed {
        result.text.unwrap_or_else(|| "guest".to_owned())
    } else {
        "guest".to_owned()
    };
    sb.show_message("Hello", &format!("Hello, {name}!"));

    // File dialogs return `None` when you cancel.
    match sb.choose_file_dialog("Pick any file") {
        Some(path) => println!("You picked {path}"),
        None => println!("No file picked."),
    }
    match sb.save_file_dialog("Save the report as", "report.txt") {
        Some(path) => println!("You chose to save to {path}"),
        None => println!("Nothing will be saved."),
    }
}
