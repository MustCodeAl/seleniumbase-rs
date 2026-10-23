//! `sbrs install` (also `sbrs get`), run as the real binary.
//!
//! Nothing here reaches the network: a request that contradicts itself is refused before any
//! download, and what would be fetched is put in `downloaded_drivers/` by the test first, as an
//! earlier `sbrs install` would have left it. The downloads themselves are the `#[ignore]`d
//! tests at the end (run with `cargo test -p seleniumbase-rs-cli --test cli_install -- --ignored`
//! and a network).

#[path = "support/sbrs.rs"]
mod sbrs;

use std::fs;
use std::path::Path;
use std::process::Output;

use sbrs::{run, stderr, stdout};

/// Runs `sbrs args...` in `dir`, with `env` set.
fn sbrs_in(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    run(dir, env, args)
}

/// Puts an executable-looking file at `path`, with its folders.
fn place(path: &Path, body: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

#[test]
fn the_help_names_every_program_the_version_words_and_both_flags() {
    let dir = tempfile::tempdir().unwrap();
    for command in ["install", "get"] {
        let output = sbrs_in(dir.path(), &[], &[command, "--help"]);
        assert!(output.status.success(), "{}", stderr(&output));
        let help = stdout(&output);
        for word in [
            "chromedriver",
            "uc_driver",
            "geckodriver",
            "edgedriver",
            "cft",
            "chs",
            "chromium",
            "--path",
            "-p",
            "--revision",
            "SB_DRIVER_PATH_DIR",
        ] {
            assert!(help.contains(word), "{command} help lacks {word}:\n{help}");
        }
    }
}

#[test]
fn a_program_is_required_as_it_is_in_python() {
    let dir = tempfile::tempdir().unwrap();
    for command in ["install", "get"] {
        let output = sbrs_in(dir.path(), &[], &[command]);
        let error = stderr(&output);
        assert_eq!(output.status.code(), Some(2), "{error}");
        assert!(error.contains("Usage: sbrs "), "{error}");
        assert!(error.contains("DRIVER"), "{error}");
    }
    assert!(!dir.path().join("downloaded_drivers").exists());
}

#[test]
fn the_names_python_takes_are_accepted() {
    // Each of these is refused for what it asks, not for its name: a name the parser did not
    // know would say so in its own words.
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "uc_driver",
        "uc-driver",
        "ucdriver",
        "firefoxdriver",
        "msedgedriver",
    ] {
        let output = sbrs_in(dir.path(), &[], &["install", name, "--revision", "5"]);
        let error = stderr(&output);
        assert_eq!(output.status.code(), Some(2), "{name}: {error}");
        assert!(
            error.contains("--revision applies to chromium only"),
            "{name}: {error}"
        );
    }
    let output = sbrs_in(dir.path(), &[], &["install", "operadriver"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("operadriver"));
}

#[test]
fn a_request_that_contradicts_itself_is_refused_before_any_download() {
    let dir = tempfile::tempdir().unwrap();
    for (args, said) in [
        (
            &["install", "chromedriver", "--revision", "1725796"][..],
            "--revision applies to chromium only",
        ),
        (
            &["install", "chromium", "--revision", "0"],
            "Chromium revision",
        ),
        (
            &["install", "cft", "--path"],
            "--path copies drivers, and cft is a browser",
        ),
        (
            &["install", "chs", "-p"],
            "--path copies drivers, and chs is a browser",
        ),
        (
            &["get", "chromium", "--path"],
            "--path copies drivers, and chromium is a browser",
        ),
    ] {
        let output = sbrs_in(dir.path(), &[], args);
        let error = stderr(&output);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {error}");
        assert!(error.contains(said), "{args:?}: {error}");
        assert!(stdout(&output).is_empty(), "{args:?}: {}", stdout(&output));
    }
    assert!(!dir.path().join("downloaded_drivers").exists());
}

#[test]
fn chromium_takes_a_revision_not_a_version_and_a_bad_version_names_the_word() {
    let dir = tempfile::tempdir().unwrap();
    let output = sbrs_in(dir.path(), &[], &["install", "chromium", "150"]);
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("--revision"), "{error}");
    assert!(error.contains("150"), "{error}");

    let output = sbrs_in(dir.path(), &[], &["install", "chromedriver", "latest-2"]);
    assert!(!output.status.success());
    let error = stderr(&output);
    for word in ["previous", "mlatest", "keep"] {
        assert!(error.contains(word), "{word} missing from: {error}");
    }
    assert!(!dir.path().join("downloaded_drivers").exists());
}

#[test]
fn a_build_that_is_already_downloaded_is_reported_and_copied_with_path() {
    let dir = tempfile::tempdir().unwrap();
    let drivers = tempfile::tempdir().unwrap();
    let driver = dir
        .path()
        .join("downloaded_drivers/chromedriver-150.0.7500.0/chromedriver");
    place(&driver, b"a driver");
    let env = [("SB_DRIVER_PATH_DIR", drivers.path().to_str().unwrap())];

    let output = sbrs_in(
        dir.path(),
        &env,
        &["install", "chromedriver", "150.0.7500.0"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.starts_with("chromedriver 150.0.7500.0 installed at "),
        "{text}"
    );
    assert!(!text.contains("copied to"), "{text}");
    assert!(!drivers.path().join("chromedriver").exists());

    let output = sbrs_in(
        dir.path(),
        &env,
        &["get", "chromedriver", "150.0.7500.0", "-p"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    let copied = drivers.path().join("chromedriver");
    assert!(
        text.lines()
            .any(|line| line == format!("copied to {}", copied.display())),
        "{text}"
    );
    assert_eq!(fs::read(&copied).unwrap(), b"a driver");
}

#[cfg(unix)]
#[test]
fn path_never_writes_through_a_symbolic_link() {
    let dir = tempfile::tempdir().unwrap();
    let drivers = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    place(
        &dir.path()
            .join("downloaded_drivers/chromedriver-150.0.7500.0/chromedriver"),
        b"a driver",
    );
    let precious = elsewhere.path().join("precious");
    fs::write(&precious, b"keep me").unwrap();
    let link = drivers.path().join("chromedriver");
    std::os::unix::fs::symlink(&precious, &link).unwrap();
    let env = [("SB_DRIVER_PATH_DIR", drivers.path().to_str().unwrap())];

    let output = sbrs_in(
        dir.path(),
        &env,
        &["install", "chromedriver", "150.0.7500.0", "--path"],
    );
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("symbolic link"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fs::read(&precious).unwrap(), b"keep me");
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn a_path_folder_that_does_not_exist_is_refused_before_any_download() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("no-such-folder");
    let env = [("SB_DRIVER_PATH_DIR", missing.to_str().unwrap())];
    place(
        &dir.path()
            .join("downloaded_drivers/chromedriver-150.0.7500.0/chromedriver"),
        b"a driver",
    );
    let output = sbrs_in(
        dir.path(),
        &env,
        &["install", "chromedriver", "150.0.7500.0", "--path"],
    );
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(error.contains("is not a folder"), "{error}");
    assert!(error.contains("SB_DRIVER_PATH_DIR"), "{error}");
}

#[test]
fn keep_reuses_the_browser_that_was_downloaded() {
    let dir = tempfile::tempdir().unwrap();
    place(
        &dir.path()
            .join("downloaded_drivers/chromium/chrome-linux/chrome"),
        b"a browser",
    );
    place(
        &dir.path().join("downloaded_drivers/chromium/REVISION"),
        b"1725796",
    );
    place(
        &dir.path()
            .join("downloaded_drivers/cft/chrome-linux64/chrome"),
        b"cft",
    );

    let output = sbrs_in(dir.path(), &[], &["install", "chromium", "keep"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.starts_with("chromium r1725796 installed at "),
        "{text}"
    );
    assert!(text.contains("chrome-linux"), "{text}");

    let output = sbrs_in(dir.path(), &[], &["install", "cft", "keep"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.starts_with("cft installed at "), "{text}");
    assert!(text.contains("chrome-linux64"), "{text}");
}

#[test]
fn keep_uses_the_geckodriver_already_downloaded() {
    let dir = tempfile::tempdir().unwrap();
    place(
        &dir.path()
            .join("downloaded_drivers/geckodriver-v0.37.1/geckodriver"),
        b"gecko",
    );
    let output = sbrs_in(dir.path(), &[], &["get", "geckodriver", "keep"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.starts_with("geckodriver v0.37.1 installed at "),
        "{text}"
    );
}

/// Downloads of what `sbrs install` fetches, which need a network and a good deal of disk.
mod network {
    use super::*;

    #[test]
    #[ignore = "downloads chromedriver and patches it"]
    fn uc_driver_is_a_patched_copy_of_the_chromedriver() {
        let dir = tempfile::tempdir().unwrap();
        let output = sbrs_in(dir.path(), &[], &["install", "uc_driver", "stable"]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains("uc_driver"), "{text}");
        let folder = fs::read_dir(dir.path().join("downloaded_drivers"))
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("uc_driver-")
            })
            .expect("a uc_driver folder");
        assert!(folder.path().join("uc_driver").is_file());
    }

    #[test]
    #[ignore = "downloads chrome-headless-shell"]
    fn chs_is_unpacked_where_a_session_looks() {
        let dir = tempfile::tempdir().unwrap();
        let output = sbrs_in(dir.path(), &[], &["install", "chs", "stable"]);
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(dir.path().join("downloaded_drivers/chs").is_dir());
    }

    #[test]
    #[ignore = "downloads Chromium, about 200 MB"]
    fn a_chromium_snapshot_and_its_driver_are_unpacked() {
        let dir = tempfile::tempdir().unwrap();
        let output = sbrs_in(dir.path(), &[], &["install", "chromium"]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.starts_with("chromium r"), "{text}");
        assert!(dir
            .path()
            .join("downloaded_drivers/chromium/REVISION")
            .is_file());
    }
}
