pub fn save_screenshot(data: &[u8], filepath: &str) -> std::io::Result<()> {
    std::fs::write(filepath, data)
}
