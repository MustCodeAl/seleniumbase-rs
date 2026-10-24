//! The language flags of Python's generators: `--en`, `--zh`, `--nl`, ...
//!
//! Python reads each language as `--zh` or `--chinese` in any case. The group has
//! one member for the code, shown in the help, and two hidden ones for the
//! English name in lower case and with a capital: `--zh`, `--chinese`, `--Chinese`.
//! (A member of an `ArgGroup` has no aliases, hence three members per language.)
//! `mkfile` uses the group as an optional one; the generators of later commands
//! (`mkpres`, `mkchart`, `translate`) use the same type.

use seleniumbase_rs::utils::translate::Language;

/// Declares [`LanguageFlag`]: for each language its three members (code, lower
/// case name, capitalised name), the [`Language`] and the help of the code.
macro_rules! language_flags {
    ($(
        ($code:ident, $lower:ident, $title:ident) => $language:ident,
        $short:tt, $long:tt, $capital:tt, $help:tt;
    )*) => {
        /// The language of the test's strings and first comment, as Python's flags give it.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, usage::ArgGroup)]
        #[usage(name = "language")]
        pub(crate) enum LanguageFlag {
            $(
                #[usage(long = $short, help = $help)]
                $code,
                #[usage(long = $long, hide)]
                $lower,
                #[usage(long = $capital, hide)]
                $title,
            )*
        }

        impl LanguageFlag {
            /// The language the flag asks for.
            pub(crate) const fn language(self) -> Language {
                match self {
                    $( Self::$code | Self::$lower | Self::$title => Language::$language, )*
                }
            }
        }
    };
}

language_flags! {
    (En, EnglishLower, EnglishTitle) => English, "en", "english", "English", "English strings";
    (Zh, ChineseLower, ChineseTitle) => Chinese, "zh", "chinese", "Chinese", "Chinese strings";
    (Nl, DutchLower, DutchTitle) => Dutch, "nl", "dutch", "Dutch", "Dutch strings";
    (Fr, FrenchLower, FrenchTitle) => French, "fr", "french", "French", "French strings";
    (It, ItalianLower, ItalianTitle) => Italian, "it", "italian", "Italian", "Italian strings";
    (Ja, JapaneseLower, JapaneseTitle) => Japanese, "ja", "japanese", "Japanese", "Japanese strings";
    (Ko, KoreanLower, KoreanTitle) => Korean, "ko", "korean", "Korean", "Korean strings";
    (Pt, PortugueseLower, PortugueseTitle) => Portuguese, "pt", "portuguese", "Portuguese", "Portuguese strings";
    (Ru, RussianLower, RussianTitle) => Russian, "ru", "russian", "Russian", "Russian strings";
    (Es, SpanishLower, SpanishTitle) => Spanish, "es", "spanish", "Spanish", "Spanish strings";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_member_of_a_language_names_that_language() {
        for (members, language) in [
            (
                [
                    LanguageFlag::En,
                    LanguageFlag::EnglishLower,
                    LanguageFlag::EnglishTitle,
                ],
                Language::English,
            ),
            (
                [
                    LanguageFlag::Zh,
                    LanguageFlag::ChineseLower,
                    LanguageFlag::ChineseTitle,
                ],
                Language::Chinese,
            ),
            (
                [
                    LanguageFlag::Es,
                    LanguageFlag::SpanishLower,
                    LanguageFlag::SpanishTitle,
                ],
                Language::Spanish,
            ),
        ] {
            for member in members {
                assert_eq!(member.language(), language, "{member:?}");
            }
        }
    }
}
