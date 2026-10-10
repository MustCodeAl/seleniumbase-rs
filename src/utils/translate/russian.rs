//! Russian (русский) method names.

use super::language::Language;
use super::master_dict;

/// `action` in Russian, or `action` itself if the [dictionary](master_dict)
/// does not know it.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::translate::russian::translate;
///
/// assert_eq!(translate("click"), "кликнуть");
/// assert_eq!(translate("custom"), "custom");
/// ```
#[must_use]
pub fn translate(action: &str) -> String {
    master_dict::translate(action, Language::Russian)
        .unwrap_or(action)
        .to_owned()
}
