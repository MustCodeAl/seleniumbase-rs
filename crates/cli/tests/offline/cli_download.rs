//! `sbrs download server` and the jar `sbrs grid-hub` finds in `downloaded_drivers/`, run as
//! the real binary. Only the `#[ignore]`d test touches the network.

use std::path::Path;
use std::process::{Command, Output};

const SBRS: &str = env!("CARGO_BIN_EXE_sbrs");

/// Runs `sbrs` in `cwd` with no jar in the environment and its Grid records in `cwd`.
fn sbrs(cwd: &Path, args: &[&str]) -> Output {
    Command::new(SBRS)
        .args(args)
        .current_dir(cwd)
        .env("SB_GRID_DIR", cwd.join("grid"))
        .env_remove("SB_SELENIUM_SERVER_JAR")
        .output()
        .expect("sbrs runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn put_jar(cwd: &Path, version: &str) {
    let folder = cwd.join("downloaded_drivers");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(format!("selenium-server-{version}.jar")), b"x").unwrap();
}

#[test]
fn the_help_shows_an_example_and_the_version_option() {
    let cwd = tempfile::tempdir().unwrap();
    let out = sbrs(cwd.path(), &["download", "server", "--help"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let help = text(&out.stdout);
    for word in ["--version", "Examples:", "sbrs download server", "SHA-256"] {
        assert!(help.contains(word), "the help lacks {word}:\n{help}");
    }
}

#[test]
fn a_selenium_three_version_is_refused_before_anything_is_downloaded() {
    let cwd = tempfile::tempdir().unwrap();
    let out = sbrs(cwd.path(), &["download", "server", "--version", "3.141.59"]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("Grid 3 is gone"),
        "{}",
        text(&out.stderr)
    );
    assert!(!cwd.path().join("downloaded_drivers").exists());
}

#[test]
fn a_version_that_could_name_a_path_is_refused() {
    let cwd = tempfile::tempdir().unwrap();
    for version in ["../../evil", "4.51", "latest", "4.1.0/x"] {
        let out = sbrs(cwd.path(), &["download", "server", "--version", version]);
        assert!(!out.status.success(), "{version}");
        assert!(
            text(&out.stderr).contains("a Selenium Server version such as 4.51.0"),
            "{version}: {}",
            text(&out.stderr)
        );
    }
    assert!(!cwd.path().join("downloaded_drivers").exists());
}

#[test]
fn a_jar_that_is_already_there_is_reported_with_its_hash_and_not_downloaded_again() {
    let cwd = tempfile::tempdir().unwrap();
    put_jar(cwd.path(), "4.99.0");
    let out = sbrs(cwd.path(), &["download", "server", "--version", "4.99.0"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let shown = text(&out.stdout);
    let mut lines = shown.lines();
    assert_eq!(
        lines.next(),
        Some("downloaded_drivers/selenium-server-4.99.0.jar")
    );
    // The SHA-256 of the one byte "x".
    let hash = lines.next().expect("a second line");
    assert!(
        hash.starts_with(
            "SHA-256 2d711642b726b04401627ca9fbac32f5c8530fb1903cc4db02258717921a4881:"
        ) && hash.contains("not verified"),
        "{hash}"
    );
    assert!(
        text(&out.stderr).contains("downloaded already"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn grid_hub_runs_the_newest_jar_in_the_downloads_folder_when_none_is_named() {
    let cwd = tempfile::tempdir().unwrap();
    put_jar(cwd.path(), "4.2.0");
    let out = sbrs(
        cwd.path(),
        &[
            "grid-hub",
            "start",
            "--java",
            "/definitely/not/java",
            "--wait",
            "1",
        ],
    );
    // It gets as far as starting Java, which is not there: the jar was found.
    assert!(!out.status.success());
    let error = text(&out.stderr);
    assert!(!error.contains("no Selenium Server jar"), "{error}");
}

#[test]
fn without_a_jar_anywhere_the_error_names_the_download_command() {
    let cwd = tempfile::tempdir().unwrap();
    let out = sbrs(cwd.path(), &["grid-node", "start"]);
    assert!(!out.status.success());
    let error = text(&out.stderr);
    assert!(error.contains("sbrs download server"), "{error}");
    assert!(error.contains("SB_SELENIUM_SERVER_JAR"), "{error}");
}

/// Downloads the real jar (about 56 MB) from GitHub and checks it against the pinned hash.
#[test]
#[ignore = "downloads about 56 MB from github.com"]
fn the_pinned_release_downloads_and_verifies() {
    let cwd = tempfile::tempdir().unwrap();
    let out = sbrs(cwd.path(), &["download", "server"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let shown = text(&out.stdout);
    assert!(
        shown.contains("verified against the checksum pinned"),
        "{shown}"
    );
    let jar = shown.lines().next().expect("a path");
    assert!(cwd.path().join(jar).is_file(), "{jar}");
    // A second run finds it and downloads nothing.
    let again = sbrs(cwd.path(), &["download", "server"]);
    assert!(again.status.success());
    assert!(text(&again.stderr).contains("downloaded already"));
}
