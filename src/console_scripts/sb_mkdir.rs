use std::fs;

pub fn create_test_dir(dir_name: &str) {
    if fs::create_dir_all(dir_name).is_ok() {
        println!("Created test directory: {}", dir_name);
    } else {
        println!("Failed to create test directory");
    }
}
