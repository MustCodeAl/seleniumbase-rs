//! Browser language codes.

use std::fmt;
use std::str::FromStr;

use crate::error::SeleniumBaseError;

/// A browser language such as `fr`, `fr-FR` or `zh-Hans-CN`, in the form
/// Chrome expects.
///
/// Python SeleniumBase writes its locale codes like `ar_eg` (`--locale-code`,
/// `help_docs/locale_codes.md`); Chrome wants `ar-EG`. Reading one with
/// [`FromStr`] accepts either separator and any letter case, and
/// [`Display`](fmt::Display) writes Chrome's form: the language in lower case,
/// a four-letter script with a capital first letter, and a region in upper
/// case or as three digits.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::LocaleCode;
///
/// let locale: LocaleCode = "ar_eg".parse()?;
/// assert_eq!(locale.to_string(), "ar-EG");
/// assert_eq!(LocaleCode::normalise("zh_hans_cn"), "zh-Hans-CN");
/// assert!("english".parse::<LocaleCode>().is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LocaleCode(String);

impl LocaleCode {
    /// The code in Chrome's form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `text` as a Chrome language code when it is one, and `text` as it is
    /// when it is not, so an unusual value a caller chose reaches the browser
    /// unchanged.
    #[must_use]
    pub fn normalise(text: &str) -> String {
        text.parse::<Self>()
            .map_or_else(|_| text.to_owned(), |code| code.0)
    }
}

impl fmt::Display for LocaleCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for LocaleCode {
    type Err = SeleniumBaseError;

    /// Reads a language of two or three letters, then any of a four-letter
    /// script, and a region of two letters or three digits, separated by `-`
    /// or `_`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            SeleniumBaseError::parse(
                text,
                "a language code such as fr, fr-FR, ar_eg or zh-Hans-CN",
            )
        };
        let mut parts = text.trim().split(['-', '_']);
        let language = parts.next().unwrap_or_default();
        if !(2..=3).contains(&language.len()) || !language.bytes().all(|b| b.is_ascii_alphabetic())
        {
            return Err(invalid());
        }
        let mut code = language.to_ascii_lowercase();
        for part in parts {
            let alphabetic = part.bytes().all(|b| b.is_ascii_alphabetic());
            let numeric = part.bytes().all(|b| b.is_ascii_digit());
            code.push('-');
            match part.len() {
                4 if alphabetic => {
                    code.push_str(&part[..1].to_ascii_uppercase());
                    code.push_str(&part[1..].to_ascii_lowercase());
                }
                2 if alphabetic => code.push_str(&part.to_ascii_uppercase()),
                3 if numeric => code.push_str(part),
                _ => return Err(invalid()),
            }
        }
        Ok(Self(code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pythons_codes_become_chromes() {
        for (python, chrome) in [
            ("ar_eg", "ar-EG"),
            ("fr", "fr"),
            ("FR", "fr"),
            ("pt_BR", "pt-BR"),
            ("zh-hans-cn", "zh-Hans-CN"),
            ("es-419", "es-419"),
            (" en-us ", "en-US"),
            ("fil", "fil"),
        ] {
            assert_eq!(python.parse::<LocaleCode>().unwrap().as_str(), chrome);
            assert_eq!(LocaleCode::normalise(python), chrome);
        }
    }

    #[test]
    fn what_is_not_a_language_code_is_refused_and_normalise_leaves_it() {
        for text in [
            "", "e", "english", "fr--FR", "fr-F", "fr_FR_x", "1a", "fr-12",
        ] {
            assert!(text.parse::<LocaleCode>().is_err(), "{text:?} was accepted");
            assert_eq!(LocaleCode::normalise(text), text);
        }
    }

    #[test]
    fn display_round_trips() {
        let code: LocaleCode = "zh_Hant_TW".parse().unwrap();
        assert_eq!(code.to_string().parse::<LocaleCode>().unwrap(), code);
    }
}
