use seleniumbase_rs::api::dialog::{show_confirm, show_message, show_prompt};

fn main() {
    show_message("Welcome", "Welcome to SeleniumBase for Rust!");

    if show_confirm("Continue?", "Do you want to continue?") {
        let name = show_prompt("Your name", "What is your name?", Some("guest"))
            .unwrap_or_else(|| "guest".into());
        show_message("Hello", &format!("Hello, {name}!"));
    }
}
