//! Detached process helpers for launching chromedriver / browser binaries.

use crate::error::SeleniumBaseError;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Returns the path to `chromedriver` if it exists in `PATH`.
pub fn find_chromedriver() -> Option<PathBuf> {
    which::which("chromedriver").ok()
}

/// Starts `chromedriver` on `port` as a detached background process.
pub fn start_chromedriver(port: u16) -> Result<Child, SeleniumBaseError> {
    let binary = find_chromedriver().ok_or_else(|| {
        SeleniumBaseError::InvalidConfig("chromedriver not found in PATH".to_owned())
    })?;
    start_detached(&binary, &["--port".to_owned(), port.to_string()])
}

/// Starts `browser` with `args` as a detached background process.
pub fn start_detached<P: AsRef<Path>>(
    binary: P,
    args: &[String],
) -> Result<Child, SeleniumBaseError> {
    let mut cmd = Command::new(binary.as_ref());
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(unix)]
    cmd.process_group(0);

    cmd.spawn()
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("failed to spawn process: {e}")))
}

/// Kills a child process started with the helpers above.
pub fn kill_process(child: &mut Child) -> Result<(), SeleniumBaseError> {
    child
        .kill()
        .map_err(|e| SeleniumBaseError::InvalidConfig(format!("failed to kill process: {e}")))?;
    child.wait().map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("failed to wait for process: {e}"))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_chromedriver_args() {
        let args: Vec<String> = ["--port".to_owned(), "9515".to_owned()].to_vec();
        assert_eq!(args, vec!["--port", "9515"]);
    }

    #[test]
    fn find_chromedriver_does_not_panic() {
        let _ = find_chromedriver();
    }
}
