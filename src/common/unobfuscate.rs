//! The reverse of [`obfuscate`](super::obfuscate::obfuscate): Base64, then
//! ROT13.
//!
//! Anyone can do this to a value that was obfuscated, because nothing secret
//! went into it; see the [`obfuscate` module](super::obfuscate) for why that
//! makes it unfit for passwords and tokens.

use base64::Engine;

use crate::error::SeleniumBaseError;

/// ROT13 again: the same function as
/// [`obfuscate::rot13`](super::obfuscate::rot13), since it is its own inverse.
pub fn rot13(input: &str) -> String {
    crate::common::obfuscate::rot13(input)
}

/// Decodes standard Base64 into UTF-8 text.
///
/// White space around the value is ignored, so a value copied with its
/// trailing newline decodes.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] if `input` is not valid Base64 or does
/// not decode to UTF-8.
///
/// ```
/// use seleniumbase_rs::common::unobfuscate::base64_decode;
///
/// assert_eq!(base64_decode("aGVsbG8=\n").unwrap(), "hello");
/// assert!(base64_decode("not base64!").is_err());
/// ```
pub fn base64_decode(input: &str) -> Result<String, SeleniumBaseError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(input.trim())
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("bad base64: {e}")))?;
    String::from_utf8(bytes)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("invalid utf8: {e}")))
}

/// Undoes [`obfuscate`](super::obfuscate::obfuscate).
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] if `input` is not Base64 text that
/// decodes to UTF-8. Text that was never obfuscated but happens to be valid
/// Base64 decodes to something else without an error: there is no check value.
pub fn unobfuscate(input: &str) -> Result<String, SeleniumBaseError> {
    let decoded = base64_decode(input)?;
    Ok(rot13(&decoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decode_reverses_encode() {
        let encoded = crate::common::obfuscate::base64_encode("hello");
        assert_eq!(base64_decode(&encoded).unwrap(), "hello");
    }

    #[test]
    fn unobfuscate_reverses_obfuscate() {
        let text = "open sesame";
        let encoded = crate::common::obfuscate::obfuscate(text);
        assert_eq!(unobfuscate(&encoded).unwrap(), text);
    }

    #[test]
    fn base64_decode_rejects_invalid_input() {
        assert!(base64_decode("!!!").is_err());
    }

    #[test]
    fn a_value_copied_with_its_newline_still_decodes() {
        assert_eq!(unobfuscate("ZnJ5cmF2aHo=\n").unwrap(), "selenium");
        assert_eq!(unobfuscate("  ZnJ5cmF2aHo=\r\n").unwrap(), "selenium");
    }

    #[test]
    fn bytes_that_are_not_utf8_are_an_error() {
        // Base64 of the single byte 0xFF.
        let error = unobfuscate("/w==").unwrap_err().to_string();
        assert!(error.contains("utf8"), "{error}");
    }
}
