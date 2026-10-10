//! Reversible text scrambling. **This is not encryption.**
//!
//! [`obfuscate`] is ROT13 followed by Base64. Neither step has a key: anyone
//! who holds the output can undo it, with [`unobfuscate`](super::unobfuscate::unobfuscate)
//! or with any Base64 decoder and a ROT13 table. It keeps a value from being
//! read at a glance, for example a name in a log line or a sample config, and
//! nothing more.
//!
//! Do not use it to protect a password, a token or anything else that must stay
//! secret. For that use
//! [`encrypt_with_passphrase`](super::encryption::encrypt_with_passphrase), which
//! is AES-256-GCM under a key derived from a passphrase and is what `sbase
//! encrypt` produces.
//!
//! ```
//! use seleniumbase_rs::common::{obfuscate::obfuscate, unobfuscate::unobfuscate};
//!
//! let scrambled = obfuscate("selenium");
//! assert_eq!(scrambled, "ZnJ5cmF2aHo=");
//! // Anyone can reverse it, and without a key:
//! assert_eq!(unobfuscate(&scrambled).unwrap(), "selenium");
//! ```

use base64::Engine;

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

/// Scrambles `input` with ROT13, then Base64. The reverse is
/// [`unobfuscate`](super::unobfuscate::unobfuscate).
///
/// **Not encryption:** see the [module documentation](self). The output is
/// deterministic, so equal inputs give equal outputs.
pub fn obfuscate(input: &str) -> String {
    base64_encode(&rot13(input))
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
        let decoded = crate::common::unobfuscate::unobfuscate(&encoded).unwrap();
        assert_eq!(decoded, text);
    }
}
