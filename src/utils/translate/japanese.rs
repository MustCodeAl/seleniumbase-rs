//! Japanese (日本語) method names.

use super::language::Language;
use super::master_dict;

/// `action` in Japanese, or `action` itself if the [dictionary](master_dict)
/// does not know it.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::translate::japanese::translate;
///
/// assert_eq!(translate("click"), "クリック");
/// assert_eq!(translate("custom"), "custom");
/// ```
#[must_use]
pub fn translate(action: &str) -> String {
    master_dict::translate(action, Language::Japanese)
        .unwrap_or(action)
        .to_owned()
}
