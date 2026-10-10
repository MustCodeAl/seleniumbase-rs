//! Translating an action name into a language chosen by code or name.

use super::language::Language;
use super::master_dict;

/// Translates `action` into the language named by `lang`.
///
/// `lang` is a two-letter code (`es`) or an English name (`Spanish`), in any
/// case. A name the dictionary does not know, or a language this crate does
/// not support, comes back unchanged, so the result is always usable in place
/// of the input. Use [`translate_to`] when the language is already a
/// [`Language`], and [`master_dict::entry`] to find out whether `action` is
/// known.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::translate::translator::translate_action;
///
/// assert_eq!(translate_action("click", "zh"), "点击");
/// assert_eq!(translate_action("type", "Spanish"), "escribir");
/// assert_eq!(translate_action("click", "xx"), "click");
/// assert_eq!(translate_action("custom", "es"), "custom");
/// ```
#[must_use]
pub fn translate_action(action: &str, lang: &str) -> String {
    match lang.parse::<Language>() {
        Ok(language) => translate_to(action, language),
        Err(_) => action.to_owned(),
    }
}

/// Translates `action` into `language`, or returns it unchanged if the
/// dictionary does not know it.
#[must_use]
pub fn translate_to(action: &str, language: Language) -> String {
    master_dict::translate(action, language)
        .unwrap_or(action)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_in_chinese() {
        assert_eq!(translate_action("click", "zh"), "点击");
    }

    #[test]
    fn type_in_spanish() {
        assert_eq!(translate_action("type", "es"), "escribir");
    }

    #[test]
    fn unknown_action_passes_through() {
        assert_eq!(translate_action("custom", "en"), "custom");
        assert_eq!(translate_action("custom", "ja"), "custom");
    }

    #[test]
    fn default_language_is_english() {
        assert_eq!(translate_action("click", "xx"), "click");
    }

    #[test]
    fn the_language_may_be_a_code_or_a_name_in_any_case() {
        for lang in ["ko", "KO", "Korean", "korean", " ko "] {
            assert_eq!(translate_action("click", lang), "클릭", "{lang:?}");
        }
    }

    #[test]
    fn english_keeps_the_spelling_it_was_given() {
        assert_eq!(translate_action("type", "en"), "type");
        assert_eq!(translate_action("type_text", "en"), "type_text");
    }

    #[test]
    fn a_method_without_a_short_name_translates() {
        assert_eq!(translate_action("double_click", "es"), "doble_clic");
        assert_eq!(
            translate_to("scroll_to_top", Language::French),
            "défiler_vers_haut"
        );
    }

    #[test]
    fn every_language_module_agrees_with_the_dictionary() {
        use crate::utils::translate::{
            chinese, dutch, english, french, italian, japanese, korean, portuguese, russian,
            spanish,
        };

        type Translate = fn(&str) -> String;

        let modules: [(Language, Translate); 10] = [
            (Language::English, english::translate),
            (Language::Chinese, chinese::translate),
            (Language::Dutch, dutch::translate),
            (Language::French, french::translate),
            (Language::Italian, italian::translate),
            (Language::Japanese, japanese::translate),
            (Language::Korean, korean::translate),
            (Language::Portuguese, portuguese::translate),
            (Language::Russian, russian::translate),
            (Language::Spanish, spanish::translate),
        ];
        for (language, translate) in modules {
            for entry in master_dict::entries() {
                assert_eq!(
                    translate(entry.en),
                    entry.get(language),
                    "{} in {language}",
                    entry.en
                );
            }
            assert_eq!(translate("not_an_action"), "not_an_action");
        }
    }
}
