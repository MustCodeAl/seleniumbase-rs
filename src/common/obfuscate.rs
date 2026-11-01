//! Reversible text scrambling. **This is not encryption.**
//!
//! [`obfuscate`] is ROT13 followed by Base64. [`unobfuscate`] is Base64, then
//! ROT13. Neither step has a key: anyone who holds the output can undo it, with
//! [`unobfuscate`] or with any Base64 decoder and a ROT13 table. It keeps a
//! value from being read at a glance, for example a name in a log line or a
//! sample config, and nothing more.
//!
//! Do not use it to protect a password, a token or anything else that must stay
//! secret. For that use
//! [`encrypt_with_passphrase`](super::encryption::encrypt_with_passphrase), which
//! is AES-256-GCM under a key derived from a passphrase and is what `sbrs
//! encrypt` produces.
//!
//! ```
//! use seleniumbase_rs::common::obfuscate::{obfuscate, unobfuscate};
//!
//! let scrambled = obfuscate("selenium");
//! assert_eq!(scrambled, "ZnJ5cmF2aHo=");
//! // Anyone can reverse it, and without a key:
//! assert_eq!(unobfuscate(&scrambled).unwrap(), "selenium");
//! ```

use base64::Engine;

use crate::error::SeleniumBaseError;

/// Rotates each ASCII letter 13 places along the alphabet; every other
/// character, including letters outside ASCII, is left as it is.
///
/// Applying it twice gives the original text. It is a substitution cipher with
/// no key, and offers no secrecy.
///
/// ```
/// use seleniumbase_rs::common::obfuscate::rot13;
///
/// assert_eq!(rot13("Hello, World! 123 é"), "Uryyb, Jbeyq! 123 é");
/// ```
pub fn rot13(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() {
                ((c as u8 - b'a' + 13) % 26 + b'a') as char
            } else if c.is_ascii_uppercase() {
                ((c as u8 - b'A' + 13) % 26 + b'A') as char
            } else {
                c
            }
        })
        .collect()
}

/// Encodes the UTF-8 bytes of `input` as Base64 (the standard alphabet, with
/// `=` padding). This is an encoding, not protection.
///
/// ```
/// use seleniumbase_rs::common::obfuscate::base64_encode;
///
/// assert_eq!(base64_encode("hello"), "aGVsbG8=");
/// ```
pub fn base64_encode(input: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
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
/// use seleniumbase_rs::common::obfuscate::base64_decode;
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

/// Scrambles `input` with ROT13, then Base64. The reverse is [`unobfuscate`].
///
/// **Not encryption:** see the [module documentation](self). The output is
/// deterministic, so equal inputs give equal outputs.
pub fn obfuscate(input: &str) -> String {
    base64_encode(&rot13(input))
}

/// Undoes [`obfuscate`]: Base64, then ROT13.
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
    fn rot13_is_self_inverse() {
        let text = "Hello World! 123";
        assert_eq!(rot13(&rot13(text)), text);
    }

    #[test]
    fn base64_encode_encodes_bytes() {
        let encoded = base64_encode("hello");
        assert_eq!(
            encoded,
            base64::engine::general_purpose::STANDARD.encode("hello")
        );
    }

    #[test]
    fn rot13_matches_the_known_vector_and_leaves_other_characters_alone() {
        assert_eq!(rot13("Hello, World!"), "Uryyb, Jbeyq!");
        assert_eq!(rot13("éü 日本 123 _-"), "éü 日本 123 _-");
        assert_eq!(rot13(""), "");
    }

    #[test]
    fn obfuscate_has_a_fixed_format() {
        // A stored value must stay readable by later versions.
        assert_eq!(obfuscate("selenium"), "ZnJ5cmF2aHo=");
        assert_eq!(obfuscate(""), "");
    }

    #[test]
    fn obfuscate_round_trip() {
        let text = "selenium";
        let encoded = obfuscate(text);
        let decoded = unobfuscate(&encoded).unwrap();
        assert_eq!(decoded, text);
    }

    #[test]
    fn base64_decode_reverses_encode() {
        let encoded = base64_encode("hello");
        assert_eq!(base64_decode(&encoded).unwrap(), "hello");
    }

    #[test]
    fn unobfuscate_reverses_obfuscate() {
        let text = "open sesame";
        let encoded = obfuscate(text);
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
