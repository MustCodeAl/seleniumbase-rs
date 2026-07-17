use std::path::Path;
pub fn get_downloads_folder() -> String {
    "downloads".to_string()
}
pub fn is_downloaded(filename: &str) -> bool {
    Path::new(filename).exists()
}
