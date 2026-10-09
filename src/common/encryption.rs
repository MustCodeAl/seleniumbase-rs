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

/// Prefix that marks a token made by [`encrypt_with_passphrase`], so the format
/// can change later without old tokens becoming ambiguous.
const TOKEN_PREFIX: &str = "sbenc1";

/// PBKDF2-HMAC-SHA256 work factor (OWASP's 2023 recommendation).
const PBKDF2_ITERATIONS: u32 = 600_000;

const SALT_LEN: usize = 16;

fn derive_key(
    passphrase: &str,
    salt: &[u8],
    iterations: u32,
) -> Result<[u8; 32], SeleniumBaseError> {
    let iterations = std::num::NonZeroU32::new(iterations).ok_or_else(|| {
        SeleniumBaseError::InvalidConfig("iterations must be non-zero".to_string())
    })?;
    let mut key = [0_u8; 32];
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        iterations,
        salt,
        passphrase.as_bytes(),
        &mut key,
    );
    Ok(key)
}

fn encrypt_with_iterations(
    plaintext: &str,
    passphrase: &str,
    iterations: u32,
) -> Result<String, SeleniumBaseError> {
    if passphrase.is_empty() {
        return Err(SeleniumBaseError::InvalidConfig(
            "the passphrase must not be empty".to_string(),
        ));
    }
    let mut salt = [0_u8; SALT_LEN];
    SystemRandom::new()
        .fill(&mut salt)
        .map_err(|e| SeleniumBaseError::Unsupported(format!("rng failed: {e}")))?;
    let key = derive_key(passphrase, &salt, iterations)?;
    let sealed = aes_encrypt(plaintext, &key)?;
    let salt = base64::engine::general_purpose::STANDARD.encode(salt);
    Ok(format!("{TOKEN_PREFIX}:{iterations}:{salt}:{sealed}"))
}

/// Encrypts `plaintext` with AES-256-GCM under a key derived from `passphrase`.
///
/// The key comes from PBKDF2-HMAC-SHA256 with a fresh random salt, so the same
/// text encrypts differently each time, and a wrong passphrase or a modified
/// token fails to decrypt rather than yielding garbage. The token is
/// `sbenc1:<iterations>:<salt>:<nonce and ciphertext>`, all printable.
///
/// This is not compatible with Python SeleniumBase's `sbase encrypt`, which is
/// a reversible obfuscation with a fixed key.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for an empty passphrase.
pub fn encrypt_with_passphrase(
    plaintext: &str,
    passphrase: &str,
) -> Result<String, SeleniumBaseError> {
    encrypt_with_iterations(plaintext, passphrase, PBKDF2_ITERATIONS)
}

/// Decrypts a token made by [`encrypt_with_passphrase`].
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if the token is malformed, and
/// [`SeleniumBaseError::Unsupported`] if the passphrase is wrong or the token
/// was modified.
pub fn decrypt_with_passphrase(token: &str, passphrase: &str) -> Result<String, SeleniumBaseError> {
    let malformed =
        || SeleniumBaseError::InvalidConfig("not a token made by `sbase encrypt`".to_string());
    let mut parts = token.trim().splitn(4, ':');
    if parts.next() != Some(TOKEN_PREFIX) {
        return Err(malformed());
    }
    let iterations: u32 = parts
        .next()
        .and_then(|n| n.parse().ok())
        .ok_or_else(malformed)?;
    let salt = parts
        .next()
        .and_then(|salt| base64::engine::general_purpose::STANDARD.decode(salt).ok())
        .ok_or_else(malformed)?;
    let sealed = parts.next().ok_or_else(malformed)?;
    // A token is untrusted input: refuse a work factor that would hang the caller.
    if !(1..=10_000_000).contains(&iterations) {
        return Err(malformed());
    }
    let key = derive_key(passphrase, &salt, iterations)?;
    aes_decrypt(sealed, &key)
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

    #[test]
    fn a_passphrase_token_round_trips_and_differs_each_time() {
        let a = encrypt_with_iterations("s3cret pa$$word", "correct horse", 1_000).unwrap();
        let b = encrypt_with_iterations("s3cret pa$$word", "correct horse", 1_000).unwrap();
        assert_ne!(a, b, "a fresh salt and nonce make every token different");
        assert!(a.starts_with("sbenc1:1000:"));
        assert_eq!(
            decrypt_with_passphrase(&a, "correct horse").unwrap(),
            "s3cret pa$$word"
        );
        assert_eq!(
            decrypt_with_passphrase(&b, "correct horse").unwrap(),
            "s3cret pa$$word"
        );
    }

    #[test]
    fn the_wrong_passphrase_is_an_error_not_garbage() {
        let token = encrypt_with_iterations("hello", "right", 1_000).unwrap();
        assert!(decrypt_with_passphrase(&token, "wrong").is_err());
    }

    #[test]
    fn malformed_tokens_are_rejected_without_deriving_a_key() {
        for bad in [
            "",
            "plain text",
            "sbenc1",
            "sbenc1:abc:AAAA:BBBB",
            "sbenc1:1000:!!!:BBBB",
            "other:1000:AAAA:BBBB",
        ] {
            assert!(
                matches!(
                    decrypt_with_passphrase(bad, "x"),
                    Err(SeleniumBaseError::InvalidConfig(_))
                ),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_token_cannot_demand_an_unbounded_work_factor() {
        let hostile = "sbenc1:4000000000:AAAAAAAAAAAAAAAAAAAAAA==:AAAA";
        assert!(matches!(
            decrypt_with_passphrase(hostile, "x"),
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
        assert!(matches!(
            decrypt_with_passphrase("sbenc1:0:AAAAAAAAAAAAAAAAAAAAAA==:AAAA", "x"),
            Err(SeleniumBaseError::InvalidConfig(_))
        ));
    }

    #[test]
    fn an_empty_passphrase_is_refused() {
        assert!(encrypt_with_passphrase("x", "").is_err());
    }

    #[test]
    fn a_modified_token_fails_to_decrypt() {
        let token = encrypt_with_iterations("message", "pass", 1_000).unwrap();
        let (head, sealed) = token.rsplit_once(':').unwrap();
        let mut bytes = sealed.as_bytes().to_vec();
        bytes[10] = if bytes[10] == b'A' { b'B' } else { b'A' };
        let tampered = format!("{head}:{}", String::from_utf8(bytes).unwrap());
        assert!(decrypt_with_passphrase(&tampered, "pass").is_err());
    }
}
