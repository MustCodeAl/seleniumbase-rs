use std::fs;
use std::path::Path;

pub fn print_file(file: &str) {
    let path = Path::new(file);
    if !path.exists() {
        println!("Error: File '{}' does not exist.", file);
        return;
    }
    match fs::read_to_string(path) {
        Ok(contents) => {
            println!("{}", contents);
        }
        Err(e) => {
            println!("Error reading file '{}': {}", file, e);
        }
    }
}
