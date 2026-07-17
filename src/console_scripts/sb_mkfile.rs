use std::fs;

pub fn create_test_file(file: &str) {
    match fs::write(file, "") {
        Ok(_) => println!("Successfully created test file: {}", file),
        Err(e) => eprintln!("Failed to create file {}: {}", file, e),
    }
}
pub fn create_file(file: &str) {
    match fs::write(file, "") {
        Ok(_) => println!("Successfully created file: {}", file),
        Err(e) => eprintln!("Failed to create file {}: {}", file, e),
    }
}
