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
        SeleniumBaseError::browser_launch(
            "chromedriver",
            "binary not found in PATH; install chromedriver or set an explicit path",
        )
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

    let binary = binary.as_ref().display().to_string();
    cmd.spawn().map_err(|e| {
        let err =
            SeleniumBaseError::browser_launch(binary.clone(), format!("failed to spawn: {e}"));
        err.log_in_context("start_detached");
        err
    })
}

/// Kills a child process started with the helpers above.
pub fn kill_process(child: &mut Child) -> Result<(), SeleniumBaseError> {
    let pid = child.id();
    child.kill().map_err(|e| {
        let err =
            SeleniumBaseError::browser_disconnected(format!("failed to kill process {pid}: {e}"));
        err.log_in_context("kill_process");
        err
    })?;
    child.wait().map_err(|e| {
        let err = SeleniumBaseError::browser_disconnected(format!(
            "failed to wait for process {pid} after kill: {e}"
        ));
        err.log_in_context("kill_process");
        err
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
