pub fn unobfuscate(text: &str) -> String {
    text.chars().map(|c| (c as u8 ^ 0xAA) as char).collect()
}
