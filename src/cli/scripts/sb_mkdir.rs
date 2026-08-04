use std::fs;

pub fn create_test_dir(dir: &str) {
    match fs::create_dir_all(dir) {
        Ok(_) => println!("Successfully created test directory: {}", dir),
        Err(e) => eprintln!("Failed to create directory {}: {}", dir, e),
    }
}
pub fn create_dir(dir: &str) {
    match fs::create_dir_all(dir) {
        Ok(_) => println!("Successfully created directory: {}", dir),
        Err(e) => eprintln!("Failed to create directory {}: {}", dir, e),
    }
}
