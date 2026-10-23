//! Downloading Chromium snapshot builds (`sbrs get chromium`).
//!
//! Google builds Chromium for every revision of its source tree and keeps the
//! builds in a public bucket. The newest revision of a platform is named in
//! `LAST_CHANGE`; the build is `chrome-<os>.zip` beside the matching
//! `chromedriver_<os>.zip`. A snapshot can be newer than any Chrome for Testing
//! milestone, so no listing of Chrome for Testing names a chromedriver for it:
//! the driver of the same revision is taken from the bucket instead.
//!
//! A snapshot is unpacked into `downloaded_drivers/chromium/`, replacing an
//! earlier one, and a `REVISION` file there records which. Its chromedriver
//! goes into `downloaded_drivers/chromedriver-<major>.0.<revision>.0/`, a name
//! the launcher already reads (it looks a driver up by the major version of
//! the browser it drives), so a session on the downloaded Chromium finds it
//! without asking the network.
//!
//! The downloads go through [`http_client`], so a proxy for downloads applies.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::browser::chromium_snapshots::{parse_revision, SnapshotPlatform};
//!
//! let platform = SnapshotPlatform::for_system("macos", "aarch64")?;
//! assert_eq!(platform.folder(), "Mac_Arm");
//! assert_eq!(parse_revision("1725796\n")?, 1_725_796);
//! assert!(parse_revision("r1725796").is_err());
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use reqwest::Client;

use crate::browser::binaries::{
    downloaded_executable, find_executable, CHROMIUM_DIR, DOWNLOADS_DIR,
};
use crate::browser::chrome_for_testing::{fetch_bytes, http_client, install_archive};
use crate::browser::config::Browser;
use crate::browser::driver_access::make_runnable;
use crate::browser::driver_version::binary_major;
use crate::error::SeleniumBaseError;

/// Google's bucket of Chromium snapshots.
pub const SNAPSHOT_HOST: &str = "https://storage.googleapis.com/chromium-browser-snapshots";

/// The largest `LAST_CHANGE` file that is read; it holds one number.
const MAX_LISTING_BYTES: u64 = 64;

/// The largest browser archive that is accepted (the Windows build is about
/// 370 MB).
pub const MAX_BROWSER_BYTES: u64 = 1024 * 1024 * 1024;

/// The largest chromedriver archive that is accepted (they are about 16 MB).
pub const MAX_DRIVER_BYTES: u64 = 128 * 1024 * 1024;

/// The largest revision accepted: revisions are about seven digits long, and a
/// bound keeps a typo out of a URL.
pub const MAX_REVISION: u64 = 99_999_999;

/// The names a chromedriver executable goes by.
const CHROMEDRIVER_NAMES: [&str; 2] = ["chromedriver", "chromedriver.exe"];

/// The file in `downloaded_drivers/chromium/` that names its revision.
const REVISION_FILE: &str = "REVISION";

/// How the bucket names the builds of one platform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotPlatform {
    folder: &'static str,
    browser_archive: &'static str,
    driver_archive: &'static str,
}

impl SnapshotPlatform {
    /// The platform `os` and `arch` (as `std::env::consts` spells them) make up.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Unsupported`] for a system the bucket has
    /// no build for.
    pub fn for_system(os: &str, arch: &str) -> Result<Self, SeleniumBaseError> {
        let (folder, browser_archive, driver_archive) = match (os, arch) {
            ("macos", "aarch64") => ("Mac_Arm", "chrome-mac.zip", "chromedriver_mac64.zip"),
            ("macos", "x86_64") => ("Mac", "chrome-mac.zip", "chromedriver_mac64.zip"),
            ("linux", "x86_64") => ("Linux_x64", "chrome-linux.zip", "chromedriver_linux64.zip"),
            ("windows", "x86_64") => ("Win_x64", "chrome-win.zip", "chromedriver_win32.zip"),
            ("windows", "x86") => ("Win", "chrome-win.zip", "chromedriver_win32.zip"),
            (os, arch) => {
                return Err(SeleniumBaseError::Unsupported(format!(
                    "Chromium snapshots are published for macOS, Linux x64 and Windows, not \
                     {os}-{arch}; `sbrs get cft` fetches Chrome for Testing where it has a build"
                )))
            }
        };
        Ok(Self {
            folder,
            browser_archive,
            driver_archive,
        })
    }

    /// The platform this program runs on.
    ///
    /// # Errors
    ///
    /// As [`for_system`](Self::for_system).
    pub fn current() -> Result<Self, SeleniumBaseError> {
        Self::for_system(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// The bucket's folder of this platform, such as `Mac_Arm`.
    #[must_use]
    pub const fn folder(self) -> &'static str {
        self.folder
    }

    /// The URL of the file that names the newest revision, under `host`.
    #[must_use]
    pub fn last_change_url(self, host: &str) -> String {
        format!("{host}/{}/LAST_CHANGE", self.folder)
    }

    /// The URL of the browser archive of `revision`, under `host`.
    #[must_use]
    pub fn browser_url(self, host: &str, revision: u64) -> String {
        format!("{host}/{}/{revision}/{}", self.folder, self.browser_archive)
    }

    /// The URL of the chromedriver archive of `revision`, under `host`.
    #[must_use]
    pub fn driver_url(self, host: &str, revision: u64) -> String {
        format!("{host}/{}/{revision}/{}", self.folder, self.driver_archive)
    }
}

/// Checks that `revision` can name a snapshot: above zero and no more than
/// [`MAX_REVISION`].
///
/// # Errors
///
/// Returns a parse error for a revision outside that range.
pub fn check_revision(revision: u64) -> Result<u64, SeleniumBaseError> {
    if (1..=MAX_REVISION).contains(&revision) {
        Ok(revision)
    } else {
        Err(SeleniumBaseError::parse(
            &revision.to_string(),
            "a Chromium revision such as 1725796",
        ))
    }
}

/// Reads a revision from `text`, as `LAST_CHANGE` holds it or as `--revision`
/// gives it: digits only, with blanks around allowed.
///
/// # Errors
///
/// Returns a parse error for anything else, and for a revision outside the
/// range [`check_revision`] allows.
pub fn parse_revision(text: &str) -> Result<u64, SeleniumBaseError> {
    let trimmed = text.trim();
    let invalid = || SeleniumBaseError::parse(text, "a Chromium revision such as 1725796");
    if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    check_revision(trimmed.parse().map_err(|_| invalid())?)
}

/// The newest snapshot revision for this platform (`LAST_CHANGE`).
///
/// # Errors
///
/// Returns an error for an unsupported platform, a request that fails, or an
/// answer that is not a revision.
pub async fn latest_chromium_revision(client: &Client) -> Result<u64, SeleniumBaseError> {
    latest_revision_at(client, SNAPSHOT_HOST, SnapshotPlatform::current()?).await
}

pub(crate) async fn latest_revision_at(
    client: &Client,
    host: &str,
    platform: SnapshotPlatform,
) -> Result<u64, SeleniumBaseError> {
    let url = platform.last_change_url(host);
    let bytes = fetch_bytes(client, &url, Some(MAX_LISTING_BYTES)).await?;
    parse_revision(&String::from_utf8_lossy(&bytes))
        .map_err(|_| SeleniumBaseError::download(&url, "the answer names no revision"))
}

/// What a snapshot download left on disk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChromiumInstall {
    /// The browser's program.
    pub browser: PathBuf,
    /// The revision it was built from.
    pub revision: u64,
    /// The chromedriver of the same revision, when it could be fetched and the
    /// browser's major version could be read to file it under.
    pub driver: Option<PathBuf>,
}

/// The revision of the Chromium in `downloaded_drivers/`, when one was
/// downloaded.
#[must_use]
pub fn installed_revision(root: &Path) -> Option<u64> {
    let text = fs::read_to_string(root.join(CHROMIUM_DIR).join(REVISION_FILE)).ok()?;
    parse_revision(&text).ok()
}

/// Downloads a Chromium snapshot build into `downloaded_drivers/chromium/`,
/// replacing an earlier one, and returns the browser's program.
///
/// `revision` is the revision to take; the newest one when `None`. A revision
/// that is already there is used without asking the network for it. The
/// chromedriver of the same revision is fetched too (see
/// [`install_chromium_with_driver`] to learn where it went). `proxy` is a proxy
/// for the downloads (`SERVER:PORT` or `USER:PASS@SERVER:PORT`).
///
/// # Errors
///
/// Returns an error for a revision outside the range [`check_revision`]
/// allows, an unsupported platform, a revision the bucket has no build of, or a
/// download that fails or holds no browser.
pub async fn install_chromium(
    revision: Option<u64>,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    Ok(install_chromium_with_driver(revision, proxy).await?.browser)
}

/// [`install_chromium`], with everything it left on disk.
///
/// # Errors
///
/// As [`install_chromium`]; a chromedriver that cannot be fetched is not an
/// error, it is `driver: None` and a warning in the log.
pub async fn install_chromium_with_driver(
    revision: Option<u64>,
    proxy: Option<&str>,
) -> Result<ChromiumInstall, SeleniumBaseError> {
    let client = http_client(proxy)?;
    install_snapshot_in(
        &client,
        SNAPSHOT_HOST,
        SnapshotPlatform::current()?,
        Path::new(DOWNLOADS_DIR),
        revision,
    )
    .await
}

pub(crate) async fn install_snapshot_in(
    client: &Client,
    host: &str,
    platform: SnapshotPlatform,
    root: &Path,
    revision: Option<u64>,
) -> Result<ChromiumInstall, SeleniumBaseError> {
    let revision = match revision {
        Some(revision) => check_revision(revision)?,
        None => latest_revision_at(client, host, platform).await?,
    };
    let dest = root.join(CHROMIUM_DIR);
    let present = (installed_revision(root) == Some(revision))
        .then(|| downloaded_executable(Browser::Chromium, &dest))
        .flatten();
    let browser = match present {
        Some(browser) => browser,
        None => {
            let url = platform.browser_url(host, revision);
            install_archive(client, &url, &dest, Some(MAX_BROWSER_BYTES)).await?;
            let browser = downloaded_executable(Browser::Chromium, &dest)
                .ok_or_else(|| SeleniumBaseError::download(&url, "the archive holds no browser"))?;
            make_runnable(&browser, false)?;
            fs::write(dest.join(REVISION_FILE), revision.to_string())?;
            tracing::info!(revision, path = %browser.display(), "downloaded Chromium");
            browser
        }
    };
    let driver =
        match install_snapshot_driver(client, host, platform, root, revision, &browser).await {
            Ok(driver) => driver,
            Err(error) => {
                tracing::warn!(%error, revision, "no chromedriver was fetched for this Chromium");
                None
            }
        };
    Ok(ChromiumInstall {
        browser,
        revision,
        driver,
    })
}

/// Fetches the chromedriver of `revision` and files it under the major version
/// of `browser`; `None` when that version cannot be read.
async fn install_snapshot_driver(
    client: &Client,
    host: &str,
    platform: SnapshotPlatform,
    root: &Path,
    revision: u64,
    browser: &Path,
) -> Result<Option<PathBuf>, SeleniumBaseError> {
    let Some(major) = binary_major(browser).await else {
        return Ok(None);
    };
    let folder = root.join(format!("chromedriver-{major}.0.{revision}.0"));
    if let Some(found) = find_executable(&folder, &CHROMEDRIVER_NAMES) {
        return Ok(Some(found));
    }
    let url = platform.driver_url(host, revision);
    install_archive(client, &url, &folder, Some(MAX_DRIVER_BYTES)).await?;
    let driver = find_executable(&folder, &CHROMEDRIVER_NAMES)
        .ok_or_else(|| SeleniumBaseError::download(&url, "the archive holds no chromedriver"))?;
    make_runnable(&driver, false)?;
    Ok(Some(driver))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::{Cursor, Write};
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use zip::write::SimpleFileOptions;

    use super::*;
    use crate::browser::binaries::{locate_with, Platform};

    fn linux() -> SnapshotPlatform {
        SnapshotPlatform::for_system("linux", "x86_64").unwrap()
    }

    #[test]
    fn each_system_gets_the_folder_and_archives_the_bucket_publishes_for_it() {
        for (os, arch, folder, browser, driver) in [
            (
                "macos",
                "aarch64",
                "Mac_Arm",
                "chrome-mac.zip",
                "chromedriver_mac64.zip",
            ),
            (
                "macos",
                "x86_64",
                "Mac",
                "chrome-mac.zip",
                "chromedriver_mac64.zip",
            ),
            (
                "linux",
                "x86_64",
                "Linux_x64",
                "chrome-linux.zip",
                "chromedriver_linux64.zip",
            ),
            (
                "windows",
                "x86_64",
                "Win_x64",
                "chrome-win.zip",
                "chromedriver_win32.zip",
            ),
            (
                "windows",
                "x86",
                "Win",
                "chrome-win.zip",
                "chromedriver_win32.zip",
            ),
        ] {
            let platform = SnapshotPlatform::for_system(os, arch).unwrap();
            assert_eq!(platform.folder(), folder, "{os}-{arch}");
            assert_eq!(
                platform.browser_url(SNAPSHOT_HOST, 1_725_796),
                format!("{SNAPSHOT_HOST}/{folder}/1725796/{browser}")
            );
            assert_eq!(
                platform.driver_url(SNAPSHOT_HOST, 1_725_796),
                format!("{SNAPSHOT_HOST}/{folder}/1725796/{driver}")
            );
            assert_eq!(
                platform.last_change_url(SNAPSHOT_HOST),
                format!("{SNAPSHOT_HOST}/{folder}/LAST_CHANGE")
            );
        }
        let error = SnapshotPlatform::for_system("linux", "aarch64").unwrap_err();
        assert!(error.to_string().contains("linux-aarch64"), "{error}");
        assert!(SnapshotPlatform::for_system("freebsd", "x86_64").is_err());
    }

    #[test]
    fn a_revision_is_digits_between_one_and_the_bound() {
        assert_eq!(parse_revision(" 1725796\n").unwrap(), 1_725_796);
        assert_eq!(parse_revision("1").unwrap(), 1);
        assert_eq!(parse_revision("99999999").unwrap(), MAX_REVISION);
        for bad in [
            "",
            "0",
            "-5",
            "r1725796",
            "17.5",
            "100000000",
            "12 34",
            "99999999999999999999",
        ] {
            assert!(parse_revision(bad).is_err(), "{bad:?} was accepted");
        }
        assert!(check_revision(0).is_err());
        assert!(check_revision(MAX_REVISION + 1).is_err());
        assert_eq!(check_revision(1_725_796).unwrap(), 1_725_796);
    }

    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let options = SimpleFileOptions::default().unix_permissions(0o755);
            for (name, body) in entries {
                if name.ends_with('/') {
                    zip.add_directory(*name, options).unwrap();
                } else {
                    zip.start_file(*name, options).unwrap();
                    zip.write_all(body).unwrap();
                }
            }
            zip.finish().unwrap();
        }
        cursor.into_inner()
    }

    /// A bucket on a local port: `files` maps a request path to its body, and
    /// the paths asked for are recorded.
    async fn bucket(files: HashMap<String, Vec<u8>>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        let asked = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&asked);
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let (files, log) = (files.clone(), Arc::clone(&log));
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut chunk = [0_u8; 1024];
                    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                        match stream.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(read) => request.extend_from_slice(&chunk[..read]),
                        }
                    }
                    let text = String::from_utf8_lossy(&request).into_owned();
                    let path = text
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or_default()
                        .to_owned();
                    log.lock().unwrap().push(path.clone());
                    let (status, body) = match files.get(&path) {
                        Some(body) => ("200 OK", body.clone()),
                        None => ("404 Not Found", b"missing".to_vec()),
                    };
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes()).await;
                    let _ = stream.write_all(&body).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        (host, asked)
    }

    fn fake_bucket_files(revision: u64) -> HashMap<String, Vec<u8>> {
        let browser = zip_of(&[
            ("chrome-linux/", b""),
            ("chrome-linux/chrome", b"#!/bin/sh\n"),
            ("chrome-linux/150.0.7500.0/", b""),
        ]);
        let driver = zip_of(&[
            ("chromedriver_linux64/", b""),
            ("chromedriver_linux64/chromedriver", b"#!/bin/sh\n"),
        ]);
        HashMap::from([
            (
                "/Linux_x64/LAST_CHANGE".to_owned(),
                revision.to_string().into_bytes(),
            ),
            (format!("/Linux_x64/{revision}/chrome-linux.zip"), browser),
            (
                format!("/Linux_x64/{revision}/chromedriver_linux64.zip"),
                driver,
            ),
        ])
    }

    fn count(asked: &Mutex<Vec<String>>, needle: &str) -> usize {
        asked
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.contains(needle))
            .count()
    }

    #[tokio::test]
    async fn the_newest_revision_is_read_from_last_change() {
        let (host, asked) = bucket(fake_bucket_files(1_725_798)).await;
        let client = http_client(None).unwrap();
        assert_eq!(
            latest_revision_at(&client, &host, linux()).await.unwrap(),
            1_725_798
        );
        assert_eq!(count(&asked, "LAST_CHANGE"), 1);
        // A bucket with no such platform says which address failed.
        let other = SnapshotPlatform::for_system("macos", "aarch64").unwrap();
        let error = latest_revision_at(&client, &host, other).await.unwrap_err();
        assert!(error.to_string().contains("Mac_Arm"), "{error}");
    }

    #[tokio::test]
    async fn the_snapshot_and_its_driver_are_unpacked_and_found_again() {
        let (host, asked) = bucket(fake_bucket_files(1_725_798)).await;
        let client = http_client(None).unwrap();
        let root = tempfile::tempdir().unwrap();

        let first = install_snapshot_in(&client, &host, linux(), root.path(), None)
            .await
            .unwrap();
        assert_eq!(first.revision, 1_725_798);
        assert_eq!(
            first.browser,
            root.path().join("chromium/chrome-linux/chrome")
        );
        assert!(first.browser.is_file());
        assert_eq!(installed_revision(root.path()), Some(1_725_798));
        // Filed under the browser's major version, which the launcher looks up.
        let driver = first.driver.expect("the driver of the same revision");
        assert_eq!(
            driver,
            root.path()
                .join("chromedriver-150.0.1725798.0/chromedriver_linux64/chromedriver")
        );
        assert!(driver.is_file());
        // The launcher finds that driver by the browser's major version.
        assert_eq!(
            crate::browser::chrome_for_testing::downloaded_for_major(
                root.path(),
                "chromedriver",
                150,
                &CHROMEDRIVER_NAMES
            ),
            Some(driver)
        );

        // The same revision again asks for no archive.
        let archives = count(&asked, ".zip");
        let again = install_snapshot_in(&client, &host, linux(), root.path(), Some(1_725_798))
            .await
            .unwrap();
        assert_eq!(again.browser, first.browser);
        assert_eq!(count(&asked, ".zip"), archives);
    }

    #[tokio::test]
    async fn a_pinned_revision_does_not_ask_for_the_newest() {
        let (host, asked) = bucket(fake_bucket_files(1_700_000)).await;
        let client = http_client(None).unwrap();
        let root = tempfile::tempdir().unwrap();
        let installed = install_snapshot_in(&client, &host, linux(), root.path(), Some(1_700_000))
            .await
            .unwrap();
        assert_eq!(installed.revision, 1_700_000);
        assert_eq!(count(&asked, "LAST_CHANGE"), 0);
    }

    #[tokio::test]
    async fn a_revision_the_bucket_lacks_is_an_error_and_leaves_nothing_behind() {
        let (host, _) = bucket(fake_bucket_files(1_700_000)).await;
        let client = http_client(None).unwrap();
        let root = tempfile::tempdir().unwrap();
        let error = install_snapshot_in(&client, &host, linux(), root.path(), Some(1_600_000))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("1600000"), "{error}");
        assert_eq!(installed_revision(root.path()), None);
        assert!(!root.path().join("chromium").exists());
        // A revision out of range is refused before any request.
        assert!(
            install_snapshot_in(&client, &host, linux(), root.path(), Some(0))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_driver_that_is_missing_does_not_fail_the_browser() {
        let mut files = fake_bucket_files(1_700_000);
        files.remove("/Linux_x64/1700000/chromedriver_linux64.zip");
        let (host, _) = bucket(files).await;
        let client = http_client(None).unwrap();
        let root = tempfile::tempdir().unwrap();
        let installed = install_snapshot_in(&client, &host, linux(), root.path(), Some(1_700_000))
            .await
            .unwrap();
        assert!(installed.browser.is_file());
        assert_eq!(installed.driver, None);
    }

    #[test]
    fn a_downloaded_chromium_is_found_before_the_system_one() {
        let root = tempfile::tempdir().unwrap();
        let find = |installed: &dyn Fn(&Path) -> bool| {
            locate_with(
                Browser::Chromium,
                Some(Platform::Linux),
                &|_| None,
                root.path(),
                installed,
                &|name| (name == "chromium").then(|| PathBuf::from("/opt/chromium")),
            )
        };
        // Nothing downloaded: the system's, then the PATH.
        assert_eq!(
            find(&|path| path == Path::new("/usr/bin/chromium-browser")),
            Some("/usr/bin/chromium-browser".into())
        );
        assert_eq!(find(&|_| false), Some("/opt/chromium".into()));
        // A download comes first.
        let folder = root.path().join("chromium/chrome-linux");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("chrome"), b"x").unwrap();
        assert_eq!(
            find(&|path| path == Path::new("/usr/bin/chromium")),
            Some(folder.join("chrome"))
        );
        // The names a Chromium goes by on each system.
        let none = |_: &str| None::<String>;
        let first = |platform| {
            crate::browser::binaries::install_locations(Browser::Chromium, platform, &none)
                .first()
                .cloned()
        };
        assert_eq!(
            first(Platform::MacOs),
            Some("/Applications/Chromium.app/Contents/MacOS/Chromium".into())
        );
        assert_eq!(first(Platform::Linux), Some("/usr/bin/chromium".into()));
        assert_eq!(first(Platform::Windows), None);
        assert_eq!(
            crate::browser::binaries::path_names(Browser::Chromium),
            ["chromium", "chromium-browser"]
        );
    }
}
