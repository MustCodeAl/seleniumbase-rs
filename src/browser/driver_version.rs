//! Telling whether a chromedriver can drive the installed Chrome.
//!
//! A chromedriver only drives the Chrome of its own major version. Pairing a
//! driver 151 with a Chrome 155 does not crash; every session just fails with
//! "session not created", which looks like a broken driver. The launcher reads
//! both versions with `--version`, and [`fit`] says whether they agree so a
//! stale driver is replaced instead of reused.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::browser::driver_version::{fit, major_version, Fit};
//!
//! let driver = major_version("ChromeDriver 151.0.7922.71 (ef35003457e9-refs/branch-heads)");
//! let browser = major_version("Google Chrome 155.0.8059.40 ");
//! assert_eq!(fit(driver, browser), Fit::Mismatch { driver: 151, browser: 155 });
//! ```

use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

/// How long a `--version` query may take.
///
/// Both programs answer at once. The limit covers a first run on macOS, where
/// the system checks a freshly downloaded binary before it starts.
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

/// Whether a driver and a browser can work together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Fit {
    /// Both have the same major version.
    Matches,
    /// Both versions are known and differ.
    Mismatch {
        /// The driver's major version.
        driver: u32,
        /// The browser's major version.
        browser: u32,
    },
    /// At least one version could not be read, so nothing can be said.
    Unknown,
}

/// The major version in the output of `--version`, such as `155` from
/// `Google Chrome 155.0.8059.40` or `ChromeDriver 155.0.8059.40 (cfaadc5...)`.
///
/// The first word that looks like a version (at least three dot-separated
/// numbers) counts.
#[must_use]
pub fn major_version(output: &str) -> Option<u32> {
    output.split_whitespace().find_map(|word| {
        let mut parts = word.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let rest: Vec<&str> = parts.collect();
        let numeric = |part: &&str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
        (rest.len() >= 2 && rest.iter().all(numeric)).then_some(major)
    })
}

/// Compares the major versions of a driver and a browser.
#[must_use]
pub fn fit(driver: Option<u32>, browser: Option<u32>) -> Fit {
    match (driver, browser) {
        (Some(driver), Some(browser)) if driver == browser => Fit::Matches,
        (Some(driver), Some(browser)) => Fit::Mismatch { driver, browser },
        _ => Fit::Unknown,
    }
}

/// Runs `binary --version` and returns the major version it reports, or `None`
/// when the program does not start, takes longer than ten seconds, or prints
/// no version.
pub async fn binary_major(binary: &Path) -> Option<u32> {
    let output = tokio::time::timeout(
        VERSION_TIMEOUT,
        Command::new(binary)
            .arg("--version")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    major_version(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_major_version_is_read_from_real_output() {
        let cases = [
            ("Google Chrome 155.0.8059.40 \n", Some(155)),
            (
                "ChromeDriver 155.0.8059.40 (cfaadc5a132d-refs/branch-heads/8059@{#936})",
                Some(155),
            ),
            (
                "Chromium 120.0.6099.224 built on Debian 12.5, running on Debian 12.5",
                Some(120),
            ),
            ("Google Chrome for Testing 156.0.8100.0", Some(156)),
            ("Google Chrome", None),
            ("", None),
            ("version 12 of 3.4", None),
            ("1.2", None),
            ("x.155.0.1", None),
        ];
        for (text, expected) in cases {
            assert_eq!(major_version(text), expected, "{text:?}");
        }
    }

    #[test]
    fn fit_needs_both_versions_to_judge() {
        assert_eq!(fit(Some(155), Some(155)), Fit::Matches);
        assert_eq!(
            fit(Some(151), Some(155)),
            Fit::Mismatch {
                driver: 151,
                browser: 155
            }
        );
        assert_eq!(fit(None, Some(155)), Fit::Unknown);
        assert_eq!(fit(Some(155), None), Fit::Unknown);
        assert_eq!(fit(None, None), Fit::Unknown);
    }

    #[cfg(unix)]
    fn stand_in(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        // The first run of a freshly written script can be slow on macOS.
        let _ = std::process::Command::new(&path).arg("--version").output();
        path
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_program_that_prints_a_version_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let driver = stand_in(
            dir.path(),
            "driver",
            "echo 'ChromeDriver 155.0.8059.40 (abc)'",
        );
        assert_eq!(binary_major(&driver).await, Some(155));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_program_with_no_version_or_no_file_gives_none() {
        let dir = tempfile::tempdir().unwrap();
        let silent = stand_in(dir.path(), "silent", "exit 0");
        let noisy = stand_in(dir.path(), "noisy", "echo 'usage: noisy [options]'");
        assert_eq!(binary_major(&silent).await, None);
        assert_eq!(binary_major(&noisy).await, None);
        assert_eq!(binary_major(&dir.path().join("missing")).await, None);
    }
}
