use seleniumbase_rs::artifacts::artifact_path;
use std::path::Path;

#[test]
fn artifact_path_uses_prefix_and_extension() {
    let path = artifact_path(Path::new("latest_logs"), "screenshot", "png");
    let output = path.to_string_lossy();
    assert!(output.contains("latest_logs"));
    assert!(output.contains("screenshot_"));
    assert!(output.ends_with(".png"));
}
