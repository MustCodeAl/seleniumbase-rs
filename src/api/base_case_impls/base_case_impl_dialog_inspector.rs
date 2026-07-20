// Dialog builder and HTML Inspector wrappers for BaseCase.

use crate::api::dialog::{choose_file, save_file, show_confirm, show_message, show_prompt, DialogResult};
use crate::api::html_inspector::{HtmlInspection, HtmlInspector};

impl BaseCase {
    /// Displays a native message dialog.
    pub fn show_message(&self, title: &str, message: &str) {
        show_message(title, message);
    }

    /// Displays a native yes/no confirmation dialog and returns the choice.
    pub fn show_confirm(&self, title: &str, message: &str) -> bool {
        show_confirm(title, message)
    }

    /// Displays a native prompt dialog and returns the user's input.
    pub fn show_prompt(&self, title: &str, message: &str, default: Option<&str>) -> DialogResult {
        let text = show_prompt(title, message, default);
        DialogResult {
            confirmed: text.is_some(),
            text,
        }
    }

    /// Opens a native file chooser dialog.
    pub fn choose_file_dialog(&self, title: &str) -> Option<String> {
        choose_file(title)
    }

    /// Opens a native save-file dialog.
    pub fn save_file_dialog(&self, title: &str, default_name: &str) -> Option<String> {
        save_file(title, default_name)
    }

    /// Inspects the current page source for common HTML issues.
    pub async fn inspect_html(&self) -> Result<HtmlInspection, SeleniumBaseError> {
        let source = self.get_page_source().await?;
        HtmlInspector::inspect(&source)
    }

    /// Asserts that the current page source has no HTML inspector issues.
    pub async fn assert_no_html_issues(&self) -> Result<(), SeleniumBaseError> {
        let inspection = self.inspect_html().await?;
        if !inspection.is_clean() {
            let messages: Vec<String> = inspection
                .issues
                .iter()
                .map(|i| format!("[{}] {}", i.rule, i.message))
                .collect();
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "HTML inspector found {} issue(s): {}",
                messages.len(),
                messages.join("; ")
            )));
        }
        Ok(())
    }
}
