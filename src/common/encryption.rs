use base64::Engine;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};

use crate::error::SeleniumBaseError;

pub fn xor_obfuscate(plaintext: &str, key: &str) -> String {
    let bytes: Vec<u8> = plaintext
        .bytes()
        .zip(key.bytes().cycle())
        .map(|(a, b)| a ^ b)
        .collect();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

pub fn xor_deobfuscate(ciphertext: &str, key: &str) -> Result<String, SeleniumBaseError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(ciphertext)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("bad base64: {e}")))?;
    let plain: Vec<u8> = bytes
        .iter()
        .zip(key.bytes().cycle())
        .map(|(&a, b)| a ^ b)
        .collect();
    String::from_utf8(plain)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("invalid utf8: {e}")))
}

pub fn aes_encrypt(plaintext: &str, key: &[u8]) -> Result<String, SeleniumBaseError> {
    let key: &[u8; 32] = key
        .try_into()
        .map_err(|_| SeleniumBaseError::InvalidConfig("AES key must be 32 bytes".to_string()))?;
    let unbound = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|e| SeleniumBaseError::Unsupported(format!("AES key setup failed: {e}")))?;
    let key = LessSafeKey::new(unbound);
    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes)
        .map_err(|e| SeleniumBaseError::Unsupported(format!("rng failed: {e}")))?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut in_out = plaintext.as_bytes().to_vec();
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
        .map_err(|e| SeleniumBaseError::Unsupported(format!("encryption failed: {e}")))?;
    let mut output = nonce_bytes.to_vec();
    output.extend_from_slice(&in_out);
    Ok(base64::engine::general_purpose::STANDARD.encode(&output))
}

pub fn aes_decrypt(ciphertext: &str, key: &[u8]) -> Result<String, SeleniumBaseError> {
    let key: &[u8; 32] = key
        .try_into()
        .map_err(|_| SeleniumBaseError::InvalidConfig("AES key must be 32 bytes".to_string()))?;
    let data = base64::engine::general_purpose::STANDARD
        .decode(ciphertext)
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("bad base64: {e}")))?;
    if data.len() < 12 {
        return Err(SeleniumBaseError::InvalidConfig(
            "ciphertext too short".to_string(),
        ));
    }
    let (nonce_bytes, cipher) = data.split_at(12);
    let nonce_bytes: [u8; 12] = nonce_bytes.try_into().unwrap();
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let unbound = UnboundKey::new(&AES_256_GCM, key)
        .map_err(|e| SeleniumBaseError::Unsupported(format!("AES key setup failed: {e}")))?;
    let key = LessSafeKey::new(unbound);
    let mut in_out = cipher.to_vec();
    let plain = key
        .open_in_place(nonce, Aad::empty(), &mut in_out)
        .map_err(|e| SeleniumBaseError::Unsupported(format!("decryption failed: {e}")))?;
    String::from_utf8(plain.to_vec())
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("invalid utf8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_round_trip() {
        let key = "secret";
        let text = "hello world";
        let encoded = xor_obfuscate(text, key);
        assert_ne!(encoded, text);
        let decoded = xor_deobfuscate(&encoded, key).unwrap();
        assert_eq!(decoded, text);
    }

    #[test]
    fn xor_deobfuscate_rejects_bad_base64() {
        let result = xor_deobfuscate("not-base64!!!", "key");
        assert!(result.is_err());
    }

    #[test]
    fn aes_round_trip() {
        let key = b"0123456789abcdef0123456789abcdef";
        let text = "selenium base secret";
        let encrypted = aes_encrypt(text, key).unwrap();
        assert_ne!(encrypted, text);
        let decrypted = aes_decrypt(&encrypted, key).unwrap();
        assert_eq!(decrypted, text);
    }

    #[test]
    fn aes_requires_32_byte_key() {
        let result = aes_encrypt("x", b"short");
        assert!(matches!(result, Err(SeleniumBaseError::InvalidConfig(_))));
    }

    #[test]
    fn aes_decrypt_rejects_tampered_data() {
        let key = b"0123456789abcdef0123456789abcdef";
        let encrypted = aes_encrypt("message", key).unwrap();
        let mut tampered = encrypted.into_bytes();
        tampered[15] = tampered[15].wrapping_add(1);
        let result = aes_decrypt(std::str::from_utf8(&tampered).unwrap(), key);
        assert!(result.is_err());
    }
}
