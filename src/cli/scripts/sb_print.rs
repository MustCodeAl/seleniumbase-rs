use std::fs;

pub fn print_file(filename: &str) -> std::io::Result<String> {
    fs::read_to_string(filename)
}
