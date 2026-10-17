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

use std::ffi::OsStr;
use std::io::Read as _;
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

/// The major version of the program at `binary`, read from files only, without
/// starting it; `None` when the files do not tell.
///
/// The `Info.plist` of the macOS app bundle the program is in comes first
/// (`Google Chrome.app/Contents/MacOS/Google Chrome` has its version in
/// `Google Chrome.app/Contents/Info.plist`), then the version folder beside the
/// program that [`binary_major`] also reads. Reading a file takes well under a
/// millisecond, where starting Chrome to ask it takes tens of milliseconds, so
/// a shell prompt (`sbrs prompt`) asks this first.
#[must_use]
pub fn major_without_running(binary: &Path) -> Option<u32> {
    major_from_bundle(binary).or_else(|| major_from_install_dir(binary))
}

/// The most of an `Info.plist` that is read; Chrome's is about 20 KB.
const MAX_PLIST_BYTES: u64 = 1024 * 1024;

/// The major version in the `Info.plist` of the macOS app bundle whose
/// `Contents/MacOS/` folder holds `binary`.
fn major_from_bundle(binary: &Path) -> Option<u32> {
    let macos = binary.parent()?;
    let contents = macos.parent()?;
    if macos.file_name() != Some(OsStr::new("MacOS"))
        || contents.file_name() != Some(OsStr::new("Contents"))
    {
        return None;
    }
    let mut plist = String::new();
    std::fs::File::open(contents.join("Info.plist"))
        .ok()?
        .take(MAX_PLIST_BYTES)
        .read_to_string(&mut plist)
        .ok()?;
    major_version(plist_string(&plist, "CFBundleShortVersionString")?)
}

/// The `<string>` that follows `<key>key</key>` in an XML property list.
fn plist_string<'a>(plist: &'a str, key: &str) -> Option<&'a str> {
    let marker = format!("<key>{key}</key>");
    let after = &plist[plist.find(&marker)? + marker.len()..];
    let value = after.trim_start().strip_prefix("<string>")?;
    Some(value[..value.find("</string>")?].trim())
}

/// The major version `binary --version` prints, `None` when it prints none.
///
/// # Errors
///
/// When the program cannot be started, or has not ended within `timeout`
/// ([`std::io::ErrorKind::TimedOut`]; the program is then killed).
pub async fn version_flag_major(binary: &Path, timeout: Duration) -> std::io::Result<Option<u32>> {
    let run = Command::new(binary)
        .arg("--version")
        .kill_on_drop(true)
        .output();
    let output = tokio::time::timeout(timeout, run)
        .await
        .map_err(|_elapsed| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("no answer within {timeout:?}"),
            )
        })??;
    Ok(major_version(&String::from_utf8_lossy(&output.stdout)))
}

/// The major version of the program at `binary`, or `None` when it cannot be
/// told.
///
/// The folder the program sits in is looked at first: Chrome and Edge on
/// Windows keep their files in a folder named after the version beside the
/// program (`Application\154.0.8037.98\`), and there `chrome.exe --version`
/// prints nothing. The newest such folder counts, as in Python SeleniumBase.
/// Otherwise `binary --version` is run, and `None` comes back when it does not
/// start, takes longer than ten seconds, or prints no version.
pub async fn binary_major(binary: &Path) -> Option<u32> {
    if cfg!(windows) {
        if let Some(major) = windows_product_major(binary).await {
            return Some(major);
        }
    }
    if let Some(major) = major_from_install_dir(binary) {
        return Some(major);
    }
    version_flag_major(binary, VERSION_TIMEOUT)
        .await
        .ok()
        .flatten()
}

/// The product version Windows records in the program file itself: the browser
/// that really starts. While an update waits for a restart, its folder is
/// already there but `chrome.exe` is still the old one. Read through
/// PowerShell, given the path in a variable so it is never parsed as code.
async fn windows_product_major(binary: &Path) -> Option<u32> {
    let output = tokio::time::timeout(
        VERSION_TIMEOUT,
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-Item -LiteralPath $env:SB_VERSION_OF).VersionInfo.ProductVersion",
            ])
            .env("SB_VERSION_OF", binary)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    major_version(&String::from_utf8_lossy(&output.stdout))
}

/// The major version of the newest folder beside `binary` whose name is a
/// whole version such as `154.0.8037.98`. When an update waits for a restart
/// (`new_chrome.exe` beside `chrome.exe`), the newest folder is the update's,
/// so the one before it counts.
fn major_from_install_dir(binary: &Path) -> Option<u32> {
    let mut versions: Vec<[u32; 4]> = std::fs::read_dir(binary.parent()?)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().to_str().and_then(version_parts))
        .collect();
    versions.sort_unstable();
    let update_waiting = binary
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| binary.with_file_name(format!("new_{name}")).exists());
    let newest = versions.len().checked_sub(1)?;
    let index = if update_waiting {
        newest.saturating_sub(1)
    } else {
        newest
    };
    versions.get(index).map(|parts| parts[0])
}

/// The four numbers of a folder name like `154.0.8037.98`, and nothing else.
fn version_parts(name: &str) -> Option<[u32; 4]> {
    let mut parts = [0_u32; 4];
    let mut count = 0;
    for part in name.split('.') {
        if count == parts.len() || part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        parts[count] = part.parse().ok()?;
        count += 1;
    }
    (count == parts.len()).then_some(parts)
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

    #[test]
    fn only_whole_four_part_versions_name_an_install_folder() {
        assert_eq!(version_parts("154.0.8037.98"), Some([154, 0, 8037, 98]));
        for name in [
            "154.0.8037",
            "154.0.8037.98.1",
            "v154.0.8037.98",
            "msedgedriver-155.0.4283.45",
            "Locales",
            "1..2.3",
        ] {
            assert_eq!(version_parts(name), None, "{name}");
        }
    }

    /// Chrome on Windows: `chrome.exe --version` prints nothing, and the
    /// version is the name of the folder beside it.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_silent_browser_is_known_by_the_newest_version_folder_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let chrome = stand_in(dir.path(), "chrome", "exit 0");
        for folder in ["153.0.7999.12", "154.0.8037.98", "Locales", "SetupMetrics"] {
            std::fs::create_dir(dir.path().join(folder)).unwrap();
        }
        assert_eq!(binary_major(&chrome).await, Some(154));
    }

    /// Chrome on Windows with an update waiting: the 155 folder and
    /// `new_chrome.exe` are there, but `chrome.exe` is still 154.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_waiting_update_does_not_count_until_it_replaces_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let chrome = stand_in(dir.path(), "chrome", "exit 0");
        for folder in ["154.0.8037.98", "155.0.8059.40"] {
            std::fs::create_dir(dir.path().join(folder)).unwrap();
        }
        std::fs::write(dir.path().join("new_chrome"), b"").unwrap();
        assert_eq!(binary_major(&chrome).await, Some(154));
        std::fs::remove_file(dir.path().join("new_chrome")).unwrap();
        assert_eq!(binary_major(&chrome).await, Some(155));
    }

    /// The start of Chrome's own `Info.plist`, with the build number
    /// (`CFBundleVersion`) before the version so the wrong key would show.
    const CHROME_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>Google Chrome</string>
	<key>CFBundleVersion</key>
	<string>8059.40</string>
	<key>CFBundleShortVersionString</key>
	<string>155.0.8059.40</string>
</dict>
</plist>
"#;

    /// `<dir>/<app>/Contents/MacOS/<name>`, a file that is no program, with
    /// `plist` as the bundle's `Info.plist` when given.
    fn app_bundle(dir: &Path, app: &str, name: &str, plist: Option<&str>) -> std::path::PathBuf {
        let contents = dir.join(app).join("Contents");
        std::fs::create_dir_all(contents.join("MacOS")).unwrap();
        let binary = contents.join("MacOS").join(name);
        std::fs::write(&binary, b"not a program").unwrap();
        if let Some(plist) = plist {
            std::fs::write(contents.join("Info.plist"), plist).unwrap();
        }
        binary
    }

    #[test]
    fn a_macos_app_is_known_by_its_info_plist_without_running_it() {
        let dir = tempfile::tempdir().unwrap();
        let chrome = app_bundle(
            dir.path(),
            "Google Chrome.app",
            "Google Chrome",
            Some(CHROME_PLIST),
        );
        assert_eq!(major_without_running(&chrome), Some(155));

        // Spacing between the key and its value does not matter.
        let spaced = CHROME_PLIST.replace("</key>\n\t<string>155", "</key>\n\n   <string> 155");
        let chromium = app_bundle(dir.path(), "Chromium.app", "Chromium", Some(&spaced));
        assert_eq!(major_without_running(&chromium), Some(155));
    }

    #[test]
    fn files_that_do_not_tell_give_none() {
        let dir = tempfile::tempdir().unwrap();
        // A bundle without an Info.plist, and one without the version key.
        let bare = app_bundle(dir.path(), "Bare.app", "Bare", None);
        assert_eq!(major_without_running(&bare), None);
        let keyless = CHROME_PLIST.replace("CFBundleShortVersionString", "CFBundleName");
        let unnamed = app_bundle(dir.path(), "Unnamed.app", "Unnamed", Some(&keyless));
        assert_eq!(major_without_running(&unnamed), None);
        // A plist next to a program that is not in a bundle's MacOS folder.
        let loose = dir.path().join("loose");
        std::fs::create_dir_all(&loose).unwrap();
        std::fs::write(loose.join("Info.plist"), CHROME_PLIST).unwrap();
        std::fs::write(loose.join("chromedriver"), b"x").unwrap();
        assert_eq!(major_without_running(&loose.join("chromedriver")), None);
        // Nothing there at all.
        assert_eq!(major_without_running(&dir.path().join("missing")), None);
    }

    #[test]
    fn the_version_folder_beside_a_program_is_read_without_running_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("chrome.exe"), b"x").unwrap();
        std::fs::create_dir(dir.path().join("154.0.8037.98")).unwrap();
        assert_eq!(
            major_without_running(&dir.path().join("chrome.exe")),
            Some(154)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_version_flag_is_read_with_a_time_limit() {
        let dir = tempfile::tempdir().unwrap();
        let limit = Duration::from_secs(5);
        let driver = stand_in(
            dir.path(),
            "driver",
            "echo 'ChromeDriver 155.0.8059.40 (abc)'",
        );
        assert_eq!(version_flag_major(&driver, limit).await.unwrap(), Some(155));
        let silent = stand_in(dir.path(), "silent", "exit 0");
        assert_eq!(version_flag_major(&silent, limit).await.unwrap(), None);
        assert!(version_flag_major(&dir.path().join("missing"), limit)
            .await
            .is_err());

        // A program that hangs is given up on, not waited for.
        let slow = stand_in(
            dir.path(),
            "slow",
            r#"if [ -e "$0.hang" ]; then exec sleep 30; fi"#,
        );
        std::fs::write(dir.path().join("slow.hang"), b"").unwrap();
        let started = std::time::Instant::now();
        let error = version_flag_major(&slow, Duration::from_millis(200))
            .await
            .expect_err("it hangs");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(10));
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
