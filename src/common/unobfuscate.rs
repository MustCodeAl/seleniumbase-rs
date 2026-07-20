use base64::Engine;

use crate::error::SeleniumBaseError;

pub fn rot13(input: &str) -> String {
    crate::common::obfuscate::rot13(input)
}

pub fn base64_decode(input: &str) -> Result<String, SeleniumBaseError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("bad base64: {e}")))?;
    String::from_utf8(bytes)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("invalid utf8: {e}")))
}

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
}
