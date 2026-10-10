//! Korean (한국어) method names.

use super::language::Language;
use super::master_dict;

/// `action` in Korean, or `action` itself if the [dictionary](master_dict)
/// does not know it.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::translate::korean::translate;
///
/// assert_eq!(translate("click"), "클릭");
/// assert_eq!(translate("custom"), "custom");
/// ```
#[must_use]
pub fn translate(action: &str) -> String {
    master_dict::translate(action, Language::Korean)
        .unwrap_or(action)
        .to_owned()
}
