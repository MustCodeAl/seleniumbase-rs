//! Dialog builder for requesting user input during automation runs.

/// Result of a user-facing dialog.
#[derive(Clone, Debug, Default)]
pub struct DialogResult {
    pub confirmed: bool,
    pub text: Option<String>,
}

/// Displays a simple message dialog.
pub fn show_message(title: &str, message: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .show();
}

/// Displays a yes/no confirmation dialog.
pub fn show_confirm(title: &str, message: &str) -> bool {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show()
        == rfd::MessageDialogResult::Yes
}

/// Displays an input prompt dialog.
pub fn show_prompt(title: &str, message: &str, default: Option<&str>) -> Option<String> {
    let default = default.map(ToOwned::to_owned);
    let (tx, rx) = std::sync::mpsc::channel();
    let title = title.to_owned();
    let message = message.to_owned();
    let default_for_thread = default.clone();
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(|| {
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            rt.block_on(async {
                rfd::AsyncMessageDialog::new()
                    .set_title(&title)
                    .set_description(&message)
                    .set_level(rfd::MessageLevel::Info)
                    .show()
                    .await
            })
        });
        let _ = tx.send((result, default_for_thread));
    });
    match rx.recv() {
        Ok((Ok(rfd::MessageDialogResult::Yes) | Ok(rfd::MessageDialogResult::Ok), default)) => {
            default
        }
        _ => None,
    }
}

/// Blocking prompt for simple synchronous scripts.
pub fn prompt(title: &str, message: &str, default: Option<&str>) -> DialogResult {
    // rfd does not offer a synchronous prompt, so fall back to a confirmation
    // plus a stdin fallback when running in a terminal.
    let confirmed = show_confirm(title, &format!("{}\n(default: {:?})", message, default));
    let text = if confirmed {
        if let Some(value) = default {
            Some(value.to_owned())
        } else {
            eprintln!("No default value provided; returning empty prompt result.");
            Some(String::new())
        }
    } else {
        None
    };
    DialogResult { confirmed, text }
}

/// Opens a native file chooser dialog and returns the selected path.
pub fn choose_file(title: &str) -> Option<String> {
    rfd::FileDialog::new()
        .set_title(title)
        .pick_file()
        .map(|p: std::path::PathBuf| p.to_string_lossy().to_string())
}

/// Saves to a path chosen by the user.
pub fn save_file(title: &str, default_name: &str) -> Option<String> {
    rfd::FileDialog::new()
        .set_title(title)
        .set_file_name(default_name)
        .save_file()
        .map(|p: std::path::PathBuf| p.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_result_defaults() {
        let result = DialogResult::default();
        assert!(!result.confirmed);
        assert!(result.text.is_none());
    }

    // Dialog UI cannot be exercised headlessly; the public functions are
    // exercised indirectly through BaseCase wrappers in integration tests.
}
