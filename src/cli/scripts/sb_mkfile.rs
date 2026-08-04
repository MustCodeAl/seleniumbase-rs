use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn create_test_file(filename: &str) {
    let path = Path::new(filename);
    if path.exists() {
        println!("File already exists: {}", filename);
        return;
    }

    match File::create(path) {
        Ok(mut file) => {
            let content = "use seleniumbase_rs::{BaseCase, BrowserConfig};\n\n#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n    let mut sb = BaseCase::new(BrowserConfig::default()).await?;\n    sb.open(\"https://github.com/MustCodeAl\").await?;\n    sb.assert_title(\"MustCodeAl\").await?;\n    Ok(())\n}\n";
            if let Err(e) = file.write_all(content.as_bytes()) {
                eprintln!("Failed to write to file {}: {}", filename, e);
            } else {
                println!("Successfully created test file: {}", filename);
            }
        }
        Err(e) => eprintln!("Failed to create file {}: {}", filename, e),
    }
}
