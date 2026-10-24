//! The exit status of `sbrs doctor`, run as the real binary in a scratch folder with stand-in
//! programs: no real browser starts and no network is used.
//!
//! The browser is named with `SB_CHROME_BIN`, the chromedriver with `CHROMEDRIVER_PATH`, and
//! `PATH` is a folder of the test's own, so what is installed on the machine running the tests
//! cannot change the answer.

use std::path::Path;
use std::process::{Command, Output, Stdio};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// `sbrs doctor` with `args`, run in `dir`, with `env` set and the host's browser and driver
/// variables removed. `PATH` is an empty folder, and the home and cache folders are in `dir`.
fn doctor(dir: &Path, env: &[(&str, &Path)], args: &[&str]) -> Output {
    let empty = dir.join("empty-path");
    std::fs::create_dir_all(&empty).unwrap();
    let mut command = Command::new(SBASE);
    command
        .arg("doctor")
        .args(args)
        .current_dir(dir)
        .env_remove("SB_CHROME_BIN")
        .env_remove("CHROME_BIN")
        .env_remove("CHROMIUM_BIN")
        .env_remove("CHROMEDRIVER_PATH")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error")
        .env("SB_PATCH_CACHE_DIR", dir.join("patch-cache"))
        .env("PATH", &empty)
        .env("HOME", dir.join("home"))
        .env("XDG_CACHE_HOME", dir.join("home").join(".cache"))
        .env("LOCALAPPDATA", dir.join("home").join("AppData"))
        .stdin(Stdio::null());
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("sbrs runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn a_browser_that_does_not_exist_fails_the_check_with_its_fix() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("no-such-chrome");

    let output = doctor(dir.path(), &[("SB_CHROME_BIN", &missing)], &[]);

    let error = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(
        error.starts_with("error: sbrs doctor found 1 problem\n  - the browser at "),
        "{error}"
    );
    assert!(
        error.contains("no-such-chrome") && error.contains("set SB_CHROME_BIN"),
        "{error}"
    );
    // The report is still whole: the hint is on standard output as before.
    let report = stdout(&output);
    assert!(report.contains("Configured path missing:"), "{report}");
    assert!(report.contains("Chromedriver"), "{report}");
}

#[test]
fn warn_only_prints_the_problems_and_exits_with_0() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("no-such-chrome");

    let output = doctor(dir.path(), &[("SB_CHROME_BIN", &missing)], &["--warn-only"]);

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let report = stdout(&output);
    assert!(report.contains("Problems (--warn-only"), "{report}");
    assert!(
        report.contains("  - the browser at ") && report.contains("no-such-chrome"),
        "{report}"
    );
}

#[cfg(unix)]
mod with_stand_ins {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// An executable script at `path` that prints `version_line`; run once here, so macOS's
    /// first-run check of a new program is over before `sbrs` times it.
    fn stand_in(path: &Path, version_line: &str) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("#!/bin/sh\necho '{version_line}'\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = Command::new(path).arg("--version").output();
        path.to_path_buf()
    }

    #[test]
    fn a_chromedriver_of_another_version_fails_the_check_with_its_fix() {
        let dir = tempfile::tempdir().unwrap();
        let chrome = stand_in(&dir.path().join("chrome"), "Google Chrome 130.0.6723.58");
        let driver = stand_in(
            &dir.path().join("drivers/chromedriver"),
            "ChromeDriver 129.0.6668.58 (abc)",
        );
        let env = [
            ("SB_CHROME_BIN", chrome.as_path()),
            ("CHROMEDRIVER_PATH", &driver),
        ];

        let output = doctor(dir.path(), &env, &[]);

        let error = stderr(&output);
        assert_eq!(output.status.code(), Some(1), "{error}");
        assert!(
            error.starts_with(
                "error: sbrs doctor found 1 problem\n  - chromedriver 129 cannot drive Chrome 130:"
            ),
            "{error}"
        );
        assert!(error.contains("`sbrs install`"), "{error}");
        let report = stdout(&output);
        assert!(
            report.contains("Version: 129, but Chrome is 130"),
            "{report}"
        );

        // --warn-only keeps the old behaviour: the same report, then status 0.
        let warned = doctor(dir.path(), &env, &["--warn-only"]);
        assert_eq!(warned.status.code(), Some(0), "{}", stderr(&warned));
        assert!(
            stdout(&warned).contains("  - chromedriver 129 cannot drive Chrome 130:"),
            "{}",
            stdout(&warned)
        );
    }

    #[test]
    fn a_matching_browser_and_chromedriver_exit_with_0() {
        let dir = tempfile::tempdir().unwrap();
        let chrome = stand_in(&dir.path().join("chrome"), "Google Chrome 130.0.6723.58");
        let driver = stand_in(
            &dir.path().join("drivers/chromedriver"),
            "ChromeDriver 130.0.6723.58 (abc)",
        );
        let env = [
            ("SB_CHROME_BIN", chrome.as_path()),
            ("CHROMEDRIVER_PATH", &driver),
        ];

        let output = doctor(dir.path(), &env, &[]);

        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert_eq!(stderr(&output), "");
        let report = stdout(&output);
        assert!(
            report.contains("Version: 130 (matches Chrome 130)"),
            "{report}"
        );
        assert!(!report.contains("Problems"), "{report}");
    }

    #[test]
    fn a_missing_chromedriver_is_only_a_hint() {
        let dir = tempfile::tempdir().unwrap();
        let chrome = stand_in(&dir.path().join("chrome"), "Google Chrome 130.0.6723.58");

        let output = doctor(dir.path(), &[("SB_CHROME_BIN", &chrome)], &[]);

        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        assert!(
            stdout(&output).contains("sbrs install"),
            "{}",
            stdout(&output)
        );
    }
}
