use std::fs::File;
use std::io::Write;

pub fn create_test_file(file_name: &str) {
    if let Ok(mut file) = File::create(file_name) {
        let _ = file.write_all(b"use seleniumbase_rs::BaseCase;\n\n#[tokio::test]\nasync fn my_test() {\n    // Test code here\n}\n");
        println!("Created test file: {}", file_name);
    } else {
        println!("Failed to create test file");
    }
}
