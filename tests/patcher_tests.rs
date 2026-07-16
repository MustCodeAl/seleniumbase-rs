use seleniumbase_rs::patcher::patch_chromedriver;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_patch_chromedriver() {
    let mut temp_file = NamedTempFile::new().unwrap();

    // Write some dummy matching strings
    let original_content = b"Some random binary stuff window.cdc_1234567890123456789012_Array = window.Array; more stuff '$cdc_1234567890123456789012_'; and {window.cdc_blah;} done.";
    temp_file.write_all(original_content).unwrap();

    let path = temp_file.path().to_path_buf();

    // Run the patcher
    let result = patch_chromedriver(&path);
    assert!(result.is_ok());

    let patched_content = fs::read(&path).unwrap();
    assert_eq!(
        original_content.len(),
        patched_content.len(),
        "Length must remain identical for binary patch"
    );

    let patched_str = String::from_utf8_lossy(&patched_content);
    assert!(!patched_str.contains("window.cdc_1234567890123456789012_Array = window.Array;"));
    assert!(!patched_str.contains("'$cdc_1234567890123456789012_';"));
}
