use std::fs;
use std::path::Path;

// BDD Testing Integration
pub fn run_behave_features(feature_dir: &str) {
    let path = Path::new(feature_dir);
    if !path.exists() {
        println!("Error: Feature directory '{}' does not exist.", feature_dir);
        return;
    }
    
    // Read and parse `.feature` files
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.filter_map(Result::ok) {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("feature") {
                println!("Running feature: {:?}", entry.path().file_name().unwrap());
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    for line in content.lines() {
                        if line.trim().starts_with("Feature:") {
                            println!("  {}", line);
                        } else if line.trim().starts_with("Scenario:") {
                            println!("    {}", line);
                        } else if line.trim().starts_with("Given ") || line.trim().starts_with("When ") || line.trim().starts_with("Then ") || line.trim().starts_with("And ") {
                            println!("      {}", line);
                        }
                    }
                }
            }
        }
    }
}
