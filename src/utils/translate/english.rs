//! English: the language of the method names themselves.

use super::language::Language;
use super::master_dict;

/// `action` in English: the name itself.
///
/// A name the [dictionary](master_dict) does not know comes back unchanged,
/// and so does a known one, so this is the identity.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::translate::english::translate;
///
/// assert_eq!(translate("click"), "click");
/// assert_eq!(translate("type"), "type");
/// ```
#[must_use]
pub fn translate(action: &str) -> String {
    master_dict::translate(action, Language::English)
        .unwrap_or(action)
        .to_owned()
}
