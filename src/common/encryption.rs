use base64::{engine::general_purpose, Engine as _};
pub fn encrypt_string(text: &str) -> String {
    general_purpose::STANDARD.encode(text)
}
pub fn decrypt_string(text: &str) -> Result<String, base64::DecodeError> {
    let bytes = general_purpose::STANDARD.decode(text)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
