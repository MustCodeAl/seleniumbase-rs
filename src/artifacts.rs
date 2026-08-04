use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::SeleniumBaseError;

pub fn ensure_latest_logs_dir() -> Result<PathBuf, SeleniumBaseError> {
    let path = PathBuf::from("latest_logs");
    fs::create_dir_all(&path).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("failed to create latest_logs directory: {e}"))
    })?;
    Ok(path)
}

pub fn artifact_path(dir: &Path, prefix: &str, extension: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    dir.join(format!("{prefix}_{ts}.{extension}"))
}
