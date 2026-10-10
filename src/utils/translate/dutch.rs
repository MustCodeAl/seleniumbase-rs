//! Dutch (Nederlands) method names.

use super::language::Language;
use super::master_dict;

/// `action` in Dutch, or `action` itself if the [dictionary](master_dict)
/// does not know it.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::translate::dutch::translate;
///
/// assert_eq!(translate("click"), "klikken");
/// assert_eq!(translate("custom"), "custom");
/// ```
#[must_use]
pub fn translate(action: &str) -> String {
    master_dict::translate(action, Language::Dutch)
        .unwrap_or(action)
        .to_owned()
}
