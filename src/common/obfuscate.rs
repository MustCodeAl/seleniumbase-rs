use base64::Engine;

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

pub fn base64_encode(input: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(input.as_bytes())
}

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
    fn obfuscate_round_trip() {
        let text = "selenium";
        let encoded = obfuscate(text);
        let decoded = crate::common::unobfuscate::unobfuscate(&encoded).unwrap();
        assert_eq!(decoded, text);
    }
}
