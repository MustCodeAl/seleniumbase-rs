//! A chromedriver that [mise](https://mise.jdx.dev) manages.
//!
//! `mise which chromedriver` prints the path of the chromedriver mise would run
//! in the current directory: the one a `[tools]` entry in a `mise.toml` above it
//! names, or one of the user's global tools. The launcher asks for it after the
//! driver in `downloaded_drivers/`, and before it downloads one.
//!
//! The question must cost almost nothing when there is no answer. No `mise` on
//! the `PATH`, a non-zero exit, output that is not a path to a program, and a
//! `mise` that does not answer in time are all "no driver here", with no error:
//! the launcher goes on to its other ways. The answer is only read, never
//! changed: nothing in this module patches or repairs the driver, and a driver
//! mise installed stays as mise left it.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::browser::driver_version::{binary_major, fit, Fit};

/// The program that is run for mise, found on the `PATH`.
pub(crate) const MISE: &str = "mise";

/// How long `mise which chromedriver` may take.
///
/// It answers in a few milliseconds. The limit covers a mise that loads a slow
/// config, and keeps one that hangs from holding up a launch for longer.
pub(crate) const MISE_TIMEOUT: Duration = Duration::from_secs(5);

/// The chromedriver the program `mise` names for the current directory, if
/// there is one.
///
/// `mise` is a program name or a path, run with an argument list and never
/// through a shell; a test puts a stand-in in a folder of its own and passes its
/// path, so the `PATH` of the process is never touched.
pub(crate) async fn which_chromedriver(mise: &OsStr, timeout: Duration) -> Option<PathBuf> {
    let run = Command::new(mise)
        .args(["which", "chromedriver"])
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .output();
    let output = match tokio::time::timeout(timeout, run).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            tracing::debug!(%error, "mise could not be run; no chromedriver from it");
            return None;
        }
        Err(_elapsed) => {
            tracing::debug!(?timeout, "mise did not say which chromedriver in time");
            return None;
        }
    };
    if !output.status.success() {
        tracing::debug!(status = %output.status, "mise has no chromedriver here");
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(text.trim_end_matches(['\r', '\n']));
    (path.is_absolute() && is_runnable(&path)).then_some(path)
}

/// [`which_chromedriver`], when the driver can drive Chrome of major version
/// `browser`, or when either version cannot be read.
pub(crate) async fn fitting_with(
    mise: &OsStr,
    timeout: Duration,
    browser: Option<u32>,
) -> Option<PathBuf> {
    let driver = which_chromedriver(mise, timeout).await?;
    let Some(browser) = browser else {
        return Some(driver);
    };
    match fit(binary_major(&driver).await, Some(browser)) {
        Fit::Mismatch {
            driver: found,
            browser,
        } => {
            tracing::warn!(
                found,
                browser,
                path = %driver.display(),
                "the chromedriver mise manages is for Chrome {found}, not Chrome {browser}; passing it over"
            );
            None
        }
        Fit::Matches | Fit::Unknown => Some(driver),
    }
}

/// Whether `path` is a file the system will run.
fn is_runnable(path: &Path) -> bool {
    // `metadata` follows symbolic links: mise names a version folder such as
    // `150` that links to the build it installed.
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// A stand-in for a program: a shell script with `body`, run once with
    /// `--warm` first, since macOS delays the first run of a freshly written file.
    #[cfg(unix)]
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join(name);
        std::fs::write(
            &path,
            format!("#!/bin/sh\n[ \"$1\" = \"--warm\" ] && exit 0\n{body}\n"),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::process::Command::new(&path).arg("--warm").output();
        path
    }

    /// A chromedriver stand-in that reports `version`.
    #[cfg(unix)]
    fn driver(dir: &Path, version: &str) -> PathBuf {
        script(
            dir,
            "chromedriver",
            &format!("echo 'ChromeDriver {version} (stand-in)'"),
        )
    }

    /// A mise stand-in that prints `path` and exits with `status`.
    #[cfg(unix)]
    fn mise_printing(dir: &Path, path: &Path, status: i32) -> PathBuf {
        script(
            dir,
            "mise",
            &format!("echo '{}'\nexit {status}", path.display()),
        )
    }

    const WAIT: Duration = Duration::from_secs(10);

    #[cfg(unix)]
    #[tokio::test]
    async fn the_driver_mise_names_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let driver = driver(dir.path(), "155.0.8059.40");
        let mise = mise_printing(dir.path(), &driver, 0);
        assert_eq!(
            which_chromedriver(mise.as_os_str(), WAIT).await,
            Some(driver)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn mise_is_asked_which_chromedriver_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let driver = driver(dir.path(), "155.0.8059.40");
        let record = dir.path().join("arguments");
        let mise = script(
            dir.path(),
            "mise",
            &format!(
                "printf '%s\\n' \"$@\" > '{}'\necho '{}'",
                record.display(),
                driver.display()
            ),
        );
        assert!(which_chromedriver(mise.as_os_str(), WAIT).await.is_some());
        assert_eq!(
            std::fs::read_to_string(record).unwrap(),
            "which\nchromedriver\n"
        );
    }

    #[tokio::test]
    async fn no_mise_means_no_driver_and_costs_almost_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-mise");
        let started = Instant::now();
        assert_eq!(which_chromedriver(missing.as_os_str(), WAIT).await, None);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "took {:?}",
            started.elapsed()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_mise_that_fails_gives_no_driver_even_when_it_printed_a_path() {
        let dir = tempfile::tempdir().unwrap();
        let driver = driver(dir.path(), "155.0.8059.40");
        let mise = mise_printing(dir.path(), &driver, 1);
        assert_eq!(which_chromedriver(mise.as_os_str(), WAIT).await, None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_path_that_does_not_exist_is_not_used() {
        let dir = tempfile::tempdir().unwrap();
        let mise = mise_printing(dir.path(), &dir.path().join("gone/chromedriver"), 0);
        assert_eq!(which_chromedriver(mise.as_os_str(), WAIT).await, None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_file_that_cannot_run_and_a_folder_are_not_used() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain");
        std::fs::write(&plain, b"not a program").unwrap();
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        let mise = mise_printing(dir.path(), &plain, 0);
        assert_eq!(which_chromedriver(mise.as_os_str(), WAIT).await, None);

        let folder = dir.path().join("folder");
        std::fs::create_dir(&folder).unwrap();
        let mise = mise_printing(dir.path(), &folder, 0);
        assert_eq!(which_chromedriver(mise.as_os_str(), WAIT).await, None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_answer_that_is_empty_or_not_a_full_path_is_not_used() {
        let dir = tempfile::tempdir().unwrap();
        let driver = driver(dir.path(), "155.0.8059.40");
        let silent = script(dir.path(), "silent", "exit 0");
        assert_eq!(which_chromedriver(silent.as_os_str(), WAIT).await, None);

        // A name, as it would be looked up on the PATH, names no file here.
        let relative = script(dir.path(), "relative", "echo chromedriver");
        assert_eq!(which_chromedriver(relative.as_os_str(), WAIT).await, None);

        // The line ends with CR LF on Windows; the path is what comes before.
        let crlf = script(
            dir.path(),
            "crlf",
            &format!("printf '%s\\r\\n' '{}'", driver.display()),
        );
        assert_eq!(
            which_chromedriver(crlf.as_os_str(), WAIT).await,
            Some(driver)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_mise_that_hangs_is_given_up_on() {
        let dir = tempfile::tempdir().unwrap();
        let mise = script(dir.path(), "mise", "exec sleep 30");
        let started = Instant::now();
        let found = which_chromedriver(mise.as_os_str(), Duration::from_millis(400)).await;
        assert_eq!(found, None);
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "took {:?}",
            started.elapsed()
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_driver_for_another_chrome_is_passed_over() {
        let dir = tempfile::tempdir().unwrap();
        let driver = driver(dir.path(), "151.0.7922.71");
        let mise = mise_printing(dir.path(), &driver, 0);
        // The same major, or a browser whose version is not known, is used.
        for browser in [Some(151), None] {
            assert_eq!(
                fitting_with(mise.as_os_str(), WAIT, browser).await,
                Some(driver.clone()),
                "{browser:?}"
            );
        }
        assert_eq!(fitting_with(mise.as_os_str(), WAIT, Some(155)).await, None);
    }
}
