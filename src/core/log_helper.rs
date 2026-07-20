use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Return the latest logs directory, creating it if necessary.
pub fn latest_logs_dir(workspace: &Path) -> std::io::Result<PathBuf> {
    let dir = workspace.join("latest_logs");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Write a log line to the given file inside the workspace logs directory.
pub fn write_log(workspace: &Path, name: &str, content: &str) -> std::io::Result<()> {
    let dir = latest_logs_dir(workspace)?;
    let path = dir.join(name);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", content)?;
    Ok(())
}

/// Return an ISO-8601 style timestamp string.
pub fn timestamp() -> String {
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S%.3f")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latest_logs_dir() {
        let tmp = std::env::temp_dir().join("sb_log_helper_test");
        let _ = fs::remove_dir_all(&tmp);
        let dir = latest_logs_dir(&tmp).unwrap();
        assert!(dir.exists());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_write_log() {
        let tmp = std::env::temp_dir().join("sb_write_log_test");
        let _ = fs::remove_dir_all(&tmp);
        write_log(&tmp, "test.log", "hello").unwrap();
        let path = tmp.join("latest_logs").join("test.log");
        assert!(path.exists());
        let data = fs::read_to_string(path).unwrap();
        assert!(data.contains("hello"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
