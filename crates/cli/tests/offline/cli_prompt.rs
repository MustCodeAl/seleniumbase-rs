//! `sbrs prompt`, run as the real binary in a scratch folder with stand-in
//! programs: no real browser starts and no network is used.
//!
//! The browser is named with `--browser-path` (or `SB_CHROME_BIN`), the
//! chromedriver with `CHROMEDRIVER_PATH` or a `downloaded_drivers/` folder,
//! and `PATH` is a folder of the test's own, so what is installed on the
//! machine running the tests cannot change the answer.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// `sbrs prompt` with `args`, run in `dir`, with `env` set and the host's
/// browser and driver variables removed. `PATH` is an empty folder, and the
/// home and cache folders are in `dir`, so no real cache is read or written.
fn prompt(dir: &Path, env: &[(&str, &Path)], args: &[&str]) -> Output {
    let empty = dir.join("empty-path");
    std::fs::create_dir_all(&empty).unwrap();
    let mut command = Command::new(SBASE);
    command
        .arg("prompt")
        .args(args)
        .current_dir(dir)
        .env_remove("SB_CHROME_BIN")
        .env_remove("CHROME_BIN")
        .env_remove("CHROMIUM_BIN")
        .env_remove("CHROMEDRIVER_PATH")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error")
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
fn no_browser_prints_nothing_and_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("no-such-chrome");
    let missing_arg = missing.to_str().unwrap();

    let quiet = prompt(dir.path(), &[], &["--browser-path", missing_arg]);
    assert_eq!(quiet.status.code(), Some(0), "{}", stderr(&quiet));
    assert_eq!(stdout(&quiet), "");
    assert_eq!(stderr(&quiet), "");

    // --verbose explains, on standard error only.
    let verbose = prompt(
        dir.path(),
        &[],
        &["--browser-path", missing_arg, "--verbose"],
    );
    assert_eq!(verbose.status.code(), Some(0), "{}", stderr(&verbose));
    assert_eq!(stdout(&verbose), "");
    let why = stderr(&verbose);
    assert!(
        why.contains("--browser-path") && why.contains(missing_arg),
        "{why}"
    );

    // A wrong SB_CHROME_BIN is not replaced by the Chrome installed here.
    let configured = prompt(dir.path(), &[("SB_CHROME_BIN", &missing)], &[]);
    assert_eq!(configured.status.code(), Some(0), "{}", stderr(&configured));
    assert_eq!(stdout(&configured), "");
}

#[test]
fn a_wrong_format_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let output = prompt(dir.path(), &[], &["--format", "{browser} {version}"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("{version}"), "{}", stderr(&output));
}

/// `<dir>/Google Chrome.app/Contents/MacOS/Google Chrome` with its version in
/// the bundle's Info.plist. The program records it if it is ever started.
fn bundled_chrome(dir: &Path, version: &str) -> PathBuf {
    let contents = dir.join("Google Chrome.app").join("Contents");
    std::fs::create_dir_all(contents.join("MacOS")).unwrap();
    std::fs::write(
        contents.join("Info.plist"),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n\
             \t<key>CFBundleShortVersionString</key>\n\t<string>{version}</string>\n\
             </dict>\n</plist>\n"
        ),
    )
    .unwrap();
    let binary = contents.join("MacOS").join("Google Chrome");
    std::fs::write(&binary, "#!/bin/sh\ntouch \"$0.ran\"\n").unwrap();
    binary
}

/// An executable script at `path` that prints `version_line` and records
/// every run in `<path>.runs`; run once here, so macOS's first-run check of a
/// new program is over before anything is timed or counted.
#[cfg(unix)]
fn stand_in(path: &Path, version_line: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!("#!/bin/sh\necho run >> \"$0.runs\"\necho '{version_line}'\n"),
    )
    .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let _ = Command::new(path).arg("--version").output();
    path.to_path_buf()
}

#[cfg(unix)]
fn runs(program: &Path) -> usize {
    let mut log = program.as_os_str().to_owned();
    log.push(".runs");
    std::fs::read_to_string(PathBuf::from(log))
        .map(|text| text.lines().count())
        .unwrap_or(0)
}

#[cfg(unix)]
#[test]
fn the_line_says_whether_the_driver_matches_the_browser() {
    let dir = tempfile::tempdir().unwrap();
    let chrome = bundled_chrome(dir.path(), "155.0.8059.40");
    let chrome_arg = chrome.to_str().unwrap();
    let matching = stand_in(
        &dir.path().join("drivers/155/chromedriver"),
        "ChromeDriver 155.0.8059.40 (abc)",
    );
    let older = stand_in(
        &dir.path().join("drivers/151/chromedriver"),
        "ChromeDriver 151.0.7922.71 (abc)",
    );

    // The chromedriver CHROMEDRIVER_PATH names (none: no driver anywhere), the
    // options after --browser-path, and the line.
    let cases: [(Option<&Path>, &[&str], &str); 5] = [
        (Some(&matching), &[], "chrome 155 ✓driver\n"),
        (Some(&matching), &["--ascii"], "chrome 155 +driver\n"),
        (Some(&older), &[], "chrome 155 ✗driver\n"),
        (
            Some(&older),
            &["--format", "[{browser}:{major}:{driver_major}{mark}] "],
            "[chrome:155:151✗] \n",
        ),
        (None, &["--ascii"], "chrome 155 -driver\n"),
    ];
    for (driver, extra, expected) in cases {
        let env: Vec<(&str, &Path)> = driver
            .map(|driver| ("CHROMEDRIVER_PATH", driver))
            .into_iter()
            .collect();
        let mut args = vec!["--browser-path", chrome_arg, "--no-cache"];
        args.extend_from_slice(extra);
        let output = prompt(dir.path(), &env, &args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {}",
            stderr(&output)
        );
        assert_eq!(stdout(&output), expected, "{args:?} {env:?}");
    }

    // SB_CHROME_BIN names the browser when --browser-path does not.
    let output = prompt(
        dir.path(),
        &[("SB_CHROME_BIN", &chrome), ("CHROMEDRIVER_PATH", &matching)],
        &["--no-cache"],
    );
    assert_eq!(
        stdout(&output),
        "chrome 155 ✓driver\n",
        "{}",
        stderr(&output)
    );

    let mut ran = chrome.into_os_string();
    ran.push(".ran");
    assert!(!Path::new(&ran).exists(), "the browser was started");
}

#[cfg(unix)]
#[test]
fn a_driver_downloaded_into_the_folder_counts() {
    let dir = tempfile::tempdir().unwrap();
    let chrome = bundled_chrome(dir.path(), "155.0.8059.40");
    stand_in(
        &dir.path().join("downloaded_drivers/chromedriver"),
        "ChromeDriver 155.0.8059.40 (abc)",
    );
    let output = prompt(
        dir.path(),
        &[],
        &["--browser-path", chrome.to_str().unwrap(), "--no-cache"],
    );
    assert_eq!(
        stdout(&output),
        "chrome 155 ✓driver\n",
        "{}",
        stderr(&output)
    );
}

#[cfg(unix)]
#[test]
fn the_driver_runs_once_and_its_version_is_cached_for_the_next_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let chrome = bundled_chrome(dir.path(), "155.0.8059.40");
    let chrome_arg = chrome.to_str().unwrap();
    let driver = stand_in(
        &dir.path().join("drivers/chromedriver"),
        "ChromeDriver 155.0.8059.40 (abc)",
    );
    let warmed = runs(&driver);
    let env: &[(&str, &Path)] = &[("CHROMEDRIVER_PATH", &driver)];

    for _ in 0..3 {
        let output = prompt(dir.path(), env, &["--browser-path", chrome_arg]);
        assert_eq!(
            stdout(&output),
            "chrome 155 ✓driver\n",
            "{}",
            stderr(&output)
        );
    }
    assert_eq!(
        runs(&driver),
        warmed + 1,
        "the cache answered after the first run"
    );

    // --no-cache asks the driver again.
    let output = prompt(
        dir.path(),
        env,
        &["--browser-path", chrome_arg, "--no-cache"],
    );
    assert_eq!(stdout(&output), "chrome 155 ✓driver\n");
    assert_eq!(runs(&driver), warmed + 2);

    // A new download (another size) is noticed at once.
    std::fs::write(
        &driver,
        "#!/bin/sh\necho run >> \"$0.runs\"\necho 'ChromeDriver 156.0.8100.0 (newer build)'\n",
    )
    .unwrap();
    let _ = Command::new(&driver).arg("--version").output();
    let output = prompt(
        dir.path(),
        env,
        &["--browser-path", chrome_arg, "--verbose"],
    );
    assert_eq!(
        stdout(&output),
        "chrome 155 ✗driver\n",
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("cache"), "{}", stderr(&output));
}
