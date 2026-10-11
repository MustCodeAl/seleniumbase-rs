//! Native message, confirmation and prompt dialogs from the `dialog` module.
//!
//! The dialogs are the operating system's own, so a script can pause for a
//! human during a run: to read a message, to approve the next step, or to
//! confirm a manual check. They are free functions and need no browser.
//!
//! One limit to know about: the underlying `rfd` crate has no text-entry
//! dialog. `show_prompt` shows an information dialog and, if you press OK,
//! returns the default you passed; it returns `None` if the dialog is closed
//! some other way. To collect typed input, read the terminal or use a browser
//! form instead.
//!
//! Run it with the `gui` feature, which is off by default because it links the
//! desktop dialog stack (GTK and X11 on Linux):
//!
//! ```bash
//! cargo run --example dialog --features gui
//! ```
//!
//! It needs a desktop session, because every call blocks until you answer the
//! dialog, so it cannot run in CI or over SSH without a display. Without the
//! feature Cargo refuses to run it and names the missing feature, and
//! `cargo build --examples` skips it.

use seleniumbase_rs::api::dialog::{show_confirm, show_message, show_prompt};

fn main() {
    show_message("Welcome", "Welcome to SeleniumBase for Rust!");

    if !show_confirm("Continue?", "Do you want to continue?") {
        println!("You chose not to continue.");
        return;
    }

    // With a default, OK returns it; with `None` there is nothing to return.
    let name = show_prompt(
        "Your name",
        "Press OK to be greeted as 'guest'.",
        Some("guest"),
    )
    .unwrap_or_else(|| "guest".to_owned());
    show_message("Hello", &format!("Hello, {name}!"));
}
