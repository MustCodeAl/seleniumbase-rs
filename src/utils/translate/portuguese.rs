//! Portuguese (português) method names.

use super::language::Language;
use super::master_dict;

/// `action` in Portuguese, or `action` itself if the [dictionary](master_dict)
/// does not know it.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::translate::portuguese::translate;
///
/// assert_eq!(translate("click"), "clicar");
/// assert_eq!(translate("custom"), "custom");
/// ```
#[must_use]
pub fn translate(action: &str) -> String {
    master_dict::translate(action, Language::Portuguese)
        .unwrap_or(action)
        .to_owned()
}
