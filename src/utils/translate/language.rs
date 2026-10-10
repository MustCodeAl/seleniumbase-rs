//! The languages test method names can be translated into.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// A language the translator supports.
///
/// Parsed from a two-letter code (`es`) or the English name (`Spanish`),
/// ignoring case and surrounding white space.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::translate::Language;
///
/// let language: Language = "es".parse()?;
/// assert_eq!(language, Language::Spanish);
/// assert_eq!(language.code(), "es");
/// assert_eq!("JAPANESE".parse::<Language>()?, Language::Japanese);
/// # Ok::<(), seleniumbase_rs::utils::translate::ParseLanguageError>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Language {
    /// English, the language of the method names themselves.
    English,
    /// Chinese (中文).
    Chinese,
    /// Dutch (Nederlands).
    Dutch,
    /// French (français).
    French,
    /// Italian (italiano).
    Italian,
    /// Japanese (日本語).
    Japanese,
    /// Korean (한국어).
    Korean,
    /// Portuguese (português).
    Portuguese,
    /// Russian (русский).
    Russian,
    /// Spanish (español).
    Spanish,
}

impl Language {
    /// Every supported language, English first.
    pub const ALL: [Self; 10] = [
        Self::English,
        Self::Chinese,
        Self::Dutch,
        Self::French,
        Self::Italian,
        Self::Japanese,
        Self::Korean,
        Self::Portuguese,
        Self::Russian,
        Self::Spanish,
    ];

    /// The two-letter code: `en`, `zh`, `nl`, `fr`, `it`, `ja`, `ko`, `pt`,
    /// `ru` or `es`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Chinese => "zh",
            Self::Dutch => "nl",
            Self::French => "fr",
            Self::Italian => "it",
            Self::Japanese => "ja",
            Self::Korean => "ko",
            Self::Portuguese => "pt",
            Self::Russian => "ru",
            Self::Spanish => "es",
        }
    }

    /// The English name of the language.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Chinese => "Chinese",
            Self::Dutch => "Dutch",
            Self::French => "French",
            Self::Italian => "Italian",
            Self::Japanese => "Japanese",
            Self::Korean => "Korean",
            Self::Portuguese => "Portuguese",
            Self::Russian => "Russian",
            Self::Spanish => "Spanish",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The text is not the code or name of a supported language.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseLanguageError {
    input: String,
}

impl fmt::Display for ParseLanguageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let codes: Vec<&str> = Language::ALL.iter().map(|l| l.code()).collect();
        write!(
            f,
            "unknown language {:?}; use one of {} (or the English name, such as Spanish)",
            self.input,
            codes.join(", ")
        )
    }
}

impl Error for ParseLanguageError {}

impl FromStr for Language {
    type Err = ParseLanguageError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let wanted = text.trim();
        Self::ALL
            .into_iter()
            .find(|language| {
                wanted.eq_ignore_ascii_case(language.code())
                    || wanted.eq_ignore_ascii_case(language.name())
            })
            .ok_or_else(|| ParseLanguageError {
                input: text.to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_names_both_parse_in_any_case() {
        for language in Language::ALL {
            assert_eq!(language.code().parse::<Language>().unwrap(), language);
            assert_eq!(language.name().parse::<Language>().unwrap(), language);
            assert_eq!(
                language.name().to_uppercase().parse::<Language>().unwrap(),
                language
            );
            assert_eq!(
                format!("  {}\n", language.code())
                    .parse::<Language>()
                    .unwrap(),
                language
            );
        }
    }

    #[test]
    fn codes_and_names_are_distinct() {
        let mut codes: Vec<_> = Language::ALL.iter().map(|l| l.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), Language::ALL.len());
        let mut names: Vec<_> = Language::ALL.iter().map(|l| l.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Language::ALL.len());
    }

    #[test]
    fn an_unknown_language_lists_the_choices() {
        let error = "klingon".parse::<Language>().unwrap_err().to_string();
        assert!(error.contains("klingon"), "{error}");
        assert!(
            error.contains("en, zh, nl, fr, it, ja, ko, pt, ru, es"),
            "{error}"
        );
        assert!("".parse::<Language>().is_err());
        assert!("e".parse::<Language>().is_err(), "a prefix is not enough");
    }

    #[test]
    fn languages_display_as_their_english_name() {
        assert_eq!(Language::Korean.to_string(), "Korean");
    }

    #[test]
    fn english_comes_first() {
        assert_eq!(Language::ALL[0], Language::English);
    }
}
