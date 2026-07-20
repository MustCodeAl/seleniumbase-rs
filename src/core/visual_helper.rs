use std::path::{Path, PathBuf};

/// Return the path where a visual baseline screenshot is stored.
pub fn baseline_path(workspace: &Path, name: &str) -> PathBuf {
    let dir = workspace.join("visual_baseline");
    std::fs::create_dir_all(&dir).ok();
    dir.join(format!("{}.png", name))
}

/// Return the path where a visual diff screenshot is stored.
pub fn diff_path(workspace: &Path, name: &str) -> PathBuf {
    workspace.join(format!("{}_diff.png", name))
}
