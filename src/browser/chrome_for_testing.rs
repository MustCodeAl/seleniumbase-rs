//! Downloading Chrome for Testing: the chromedriver of a chosen version, and the
//! `cft` and `chs` browser builds.
//!
//! [Chrome for Testing] is Google's build of Chrome made for automation. It is
//! published per version, together with the chromedriver of the same version
//! and a headless-only `chrome-headless-shell`, and its JSON listings say which
//! version each release channel and each Chrome major version has.
//!
//! The downloads go through an HTTP client that can use a proxy, which is what
//! Python SeleniumBase's `--proxy-driver` asks for.
//!
//! [Chrome for Testing]: https://googlechromelabs.github.io/chrome-for-testing/

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use reqwest::Client;
use serde_json::Value;

use crate::browser::binaries::{
    downloaded_executable, find_executable, CHROME_FOR_TESTING_DIR, CHROME_HEADLESS_SHELL_DIR,
    DOWNLOADS_DIR,
};
use crate::browser::config::Browser;
use crate::browser::downloader::platform_label;
use crate::browser::driver_access::make_runnable;
use crate::browser::options::{DriverVersion, ReleaseChannel};
use crate::error::SeleniumBaseError;

/// The listing of the newest build of every release channel.
const LAST_KNOWN_GOOD: &str =
    "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json";

/// The listing of the newest build of every Chrome major version.
const MILESTONE_LISTING: &str =
    "https://googlechromelabs.github.io/chrome-for-testing/latest-versions-per-milestone-with-downloads.json";

/// Where every published build lives, by version and platform.
const STORAGE_ROOT: &str = "https://storage.googleapis.com/chrome-for-testing-public";

/// The names a chromedriver executable goes by.
const CHROMEDRIVER_NAMES: [&str; 2] = ["chromedriver", "chromedriver.exe"];

/// A program that Chrome for Testing publishes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Artifact {
    /// The driver.
    Chromedriver,
    /// The browser.
    Chrome,
    /// The headless-only browser.
    ChromeHeadlessShell,
}

impl Artifact {
    /// The name the listings and the archive file names give it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Chromedriver => "chromedriver",
            Self::Chrome => "chrome",
            Self::ChromeHeadlessShell => "chrome-headless-shell",
        }
    }
}

/// One downloadable build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Build {
    /// The full version, such as `126.0.6478.126`.
    pub version: String,
    /// The URL of the archive for this platform.
    pub url: String,
}

/// The URL of the archive of `artifact` at `version` for `platform` (such as
/// `linux64` or `mac-arm64`).
#[must_use]
pub fn archive_url(artifact: Artifact, version: &str, platform: &str) -> String {
    let name = artifact.name();
    format!("{STORAGE_ROOT}/{version}/{platform}/{name}-{platform}.zip")
}

/// A proxy URL from the form Python SeleniumBase takes, `SERVER:PORT` or
/// `USER:PASS@SERVER:PORT`: it is an HTTP proxy unless a scheme says otherwise.
#[must_use]
pub fn proxy_url(spec: &str) -> String {
    if spec.contains("://") {
        spec.to_owned()
    } else {
        format!("http://{spec}")
    }
}

/// An HTTP client for the downloads, going through `proxy` when one is given.
///
/// # Errors
///
/// Returns an error if the proxy is not a URL the client accepts.
pub fn http_client(proxy: Option<&str>) -> Result<Client, SeleniumBaseError> {
    crate::install_tls_provider();
    let mut builder = Client::builder();
    if let Some(proxy) = proxy {
        let url = proxy_url(proxy);
        let proxy = reqwest::Proxy::all(&url).map_err(|error| {
            // The text can hold a password, so the URL is left out.
            SeleniumBaseError::invalid_config(format!(
                "the proxy for downloads is not valid: {error}"
            ))
        })?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|error| SeleniumBaseError::download("(client)", error.to_string()))
}

/// Fetches `url` and reads it as JSON.
async fn fetch_json(client: &Client, url: &str) -> Result<Value, SeleniumBaseError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| SeleniumBaseError::download(url, error.to_string()))?;
    if !response.status().is_success() {
        return Err(SeleniumBaseError::network(
            url,
            response.status().as_u16(),
            response.status().to_string(),
        ));
    }
    response
        .json()
        .await
        .map_err(|error| SeleniumBaseError::download(url, error.to_string()))
}

/// Fetches `url` and returns its bytes, refusing a body of more than
/// `max_bytes` when a limit is given.
pub(crate) async fn fetch_bytes(
    client: &Client,
    url: &str,
    max_bytes: Option<u64>,
) -> Result<Vec<u8>, SeleniumBaseError> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|error| SeleniumBaseError::download(url, error.to_string()))?;
    if !response.status().is_success() {
        return Err(SeleniumBaseError::network(
            url,
            response.status().as_u16(),
            response.status().to_string(),
        ));
    }
    let too_big = |limit: u64| {
        SeleniumBaseError::download(url, format!("the download is larger than {limit} bytes"))
    };
    if let (Some(limit), Some(length)) = (max_bytes, response.content_length()) {
        if length > limit {
            return Err(too_big(limit));
        }
    }
    let mut bytes = Vec::new();
    // Read piece by piece, so a server that sends more than it announced is
    // stopped at the limit instead of filling the memory.
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| SeleniumBaseError::download(url, error.to_string()))?
    {
        bytes.extend_from_slice(&chunk);
        if let Some(limit) = max_bytes {
            if bytes.len() as u64 > limit {
                return Err(too_big(limit));
            }
        }
    }
    Ok(bytes)
}

/// The URL of `artifact` for `platform` in the `downloads` object of a listing.
fn listed_url(downloads: &Value, artifact: Artifact, platform: &str) -> Option<String> {
    downloads
        .get(artifact.name())?
        .as_array()?
        .iter()
        .find(|download| download["platform"].as_str() == Some(platform))
        .and_then(|download| download["url"].as_str())
        .map(str::to_owned)
}

/// The build a release channel has, from the last-known-good listing.
///
/// # Errors
///
/// Returns an error if the listing has no such channel, or no archive of
/// `artifact` for `platform`.
pub fn channel_build(
    listing: &Value,
    channel: ReleaseChannel,
    artifact: Artifact,
    platform: &str,
) -> Result<Build, SeleniumBaseError> {
    let name = channel.listing_name();
    let entry = &listing["channels"][name];
    build_of(entry, artifact, platform, &format!("the {name} channel"))
}

/// The newest build of Chrome `major`, from the per-milestone listing.
///
/// # Errors
///
/// Returns an error if the listing has no such milestone, or no archive of
/// `artifact` for `platform`.
pub fn milestone_build(
    listing: &Value,
    major: u32,
    artifact: Artifact,
    platform: &str,
) -> Result<Build, SeleniumBaseError> {
    let entry = &listing["milestones"][major.to_string().as_str()];
    build_of(entry, artifact, platform, &format!("Chrome {major}"))
}

fn build_of(
    entry: &Value,
    artifact: Artifact,
    platform: &str,
    what: &str,
) -> Result<Build, SeleniumBaseError> {
    let version = entry["version"].as_str().ok_or_else(|| {
        SeleniumBaseError::Unsupported(format!("Chrome for Testing lists no build for {what}"))
    })?;
    let url = listed_url(&entry["downloads"], artifact, platform).ok_or_else(|| {
        SeleniumBaseError::Unsupported(format!(
            "Chrome for Testing has no {} for {what} on {platform}",
            artifact.name()
        ))
    })?;
    Ok(Build {
        version: version.to_owned(),
        url,
    })
}

/// Works out which build `version` means for `artifact`.
///
/// `browser_major` is the major version of the installed browser, which is what
/// [`DriverVersion::MatchBrowser`] follows; without it the stable build is used.
///
/// # Errors
///
/// Returns an error if a listing cannot be fetched or has no such build.
pub async fn resolve_build(
    client: &Client,
    version: &DriverVersion,
    artifact: Artifact,
    platform: &str,
    browser_major: Option<u32>,
) -> Result<Build, SeleniumBaseError> {
    match version {
        DriverVersion::Exact(exact) => Ok(Build {
            version: exact.clone(),
            url: archive_url(artifact, exact, platform),
        }),
        DriverVersion::Channel(channel) => {
            let listing = fetch_json(client, LAST_KNOWN_GOOD).await?;
            channel_build(&listing, *channel, artifact, platform)
        }
        DriverVersion::Milestone(major) => {
            let listing = fetch_json(client, MILESTONE_LISTING).await?;
            milestone_build(&listing, *major, artifact, platform)
        }
        DriverVersion::MatchBrowser => match browser_major {
            Some(major) => {
                let listing = fetch_json(client, MILESTONE_LISTING).await?;
                milestone_build(&listing, major, artifact, platform)
            }
            None => {
                tracing::warn!(
                    "the installed browser's version could not be read; using the stable build"
                );
                let listing = fetch_json(client, LAST_KNOWN_GOOD).await?;
                channel_build(&listing, ReleaseChannel::Stable, artifact, platform)
            }
        },
    }
}

/// The most a gzip-compressed tar archive may unpack to.
const MAX_UNPACKED_TAR_BYTES: u64 = 512 * 1024 * 1024;

/// Unpacks a zip archive, or a gzip-compressed tar archive (geckodriver's form
/// on Linux and macOS), into `dest`, which must not exist yet.
fn unpack(archive: &[u8], dest: &Path) -> Result<(), SeleniumBaseError> {
    if archive.starts_with(&[0x1f, 0x8b]) {
        return unpack_tar_gz(archive, dest);
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(|error| {
        SeleniumBaseError::download("(archive)", format!("not a readable ZIP archive: {error}"))
    })?;
    fs::create_dir_all(dest)?;
    // `extract` refuses entries that climb out of `dest` and keeps the unix
    // permissions and symbolic links, which a browser bundle needs.
    zip.extract(dest).map_err(|error| {
        SeleniumBaseError::download("(archive)", format!("cannot unpack the archive: {error}"))
    })
}

/// Unpacks a gzip-compressed tar archive into `dest`.
///
/// `tar` skips an entry whose path holds `..` and never writes outside `dest`;
/// the unpacked size is capped at [`MAX_UNPACKED_TAR_BYTES`], so a small
/// archive cannot fill the disk.
fn unpack_tar_gz(archive: &[u8], dest: &Path) -> Result<(), SeleniumBaseError> {
    use std::io::Read as _;
    fs::create_dir_all(dest)?;
    let decoder = flate2::read::GzDecoder::new(archive).take(MAX_UNPACKED_TAR_BYTES);
    let mut tar = tar::Archive::new(decoder);
    tar.set_overwrite(true);
    tar.unpack(dest).map_err(|error| {
        SeleniumBaseError::download("(archive)", format!("cannot unpack the archive: {error}"))
    })
}

/// A path next to `path` whose file name has `suffix` added.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

fn remove_dir_if_exists(path: &Path) -> Result<(), SeleniumBaseError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Unpacks `archive` into `dest`, replacing what was there. The old folder goes
/// only after the new one unpacked completely.
pub(crate) fn replace_with_unpacked(archive: &[u8], dest: &Path) -> Result<(), SeleniumBaseError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let partial = sibling(dest, ".partial");
    remove_dir_if_exists(&partial)?;
    if let Err(error) = unpack(archive, &partial) {
        let _ = remove_dir_if_exists(&partial);
        return Err(error);
    }
    remove_dir_if_exists(dest)?;
    fs::rename(&partial, dest)?;
    Ok(())
}

/// Downloads the archive at `url`, at most `max_bytes` of it when a limit is
/// given, and unpacks it into `dest`.
pub(crate) async fn install_archive(
    client: &Client,
    url: &str,
    dest: &Path,
    max_bytes: Option<u64>,
) -> Result<(), SeleniumBaseError> {
    let bytes = fetch_bytes(client, url, max_bytes).await?;
    let dest = dest.to_owned();
    tokio::task::spawn_blocking(move || replace_with_unpacked(&bytes, &dest))
        .await
        .map_err(|error| SeleniumBaseError::download("(unpack)", error.to_string()))?
}

/// A chromedriver of Chrome `major` already in `root`, if any: a folder called
/// `chromedriver-<major>.<...>` that holds the executable.
fn downloaded_chromedriver_for_major(root: &Path, major: u32) -> Option<PathBuf> {
    downloaded_for_major(root, "chromedriver", major, &CHROMEDRIVER_NAMES)
}

/// A driver of browser `major` already in `root`, if any: the newest folder
/// called `<stem>-<major>.<...>` that holds an executable called one of `names`.
pub(crate) fn downloaded_for_major(
    root: &Path,
    stem: &str,
    major: u32,
    names: &[&str],
) -> Option<PathBuf> {
    let prefix = format!("{stem}-{major}.");
    let mut folders: Vec<PathBuf> = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .map(|entry| entry.path())
        .collect();
    folders.sort();
    folders
        .iter()
        .rev()
        .find_map(|folder| find_executable(folder, names))
}

/// Makes sure the chromedriver `version` means is in `downloaded_drivers/`, and
/// returns its path.
///
/// Each build has its own folder, `downloaded_drivers/chromedriver-<version>/`,
/// so drivers of different versions live side by side. A driver of the wanted
/// major version that is already there is used without asking the network.
/// `browser_major` is what [`DriverVersion::MatchBrowser`] follows. `proxy` is
/// a proxy for the downloads (`SERVER:PORT` or `USER:PASS@SERVER:PORT`).
///
/// # Errors
///
/// Returns an error if the build cannot be found or downloaded, or its archive
/// holds no chromedriver.
pub async fn install_chromedriver(
    version: &DriverVersion,
    browser_major: Option<u32>,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    install_chromedriver_in(Path::new(DOWNLOADS_DIR), version, browser_major, proxy).await
}

pub(crate) async fn install_chromedriver_in(
    root: &Path,
    version: &DriverVersion,
    browser_major: Option<u32>,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    let known_major = match version {
        DriverVersion::Milestone(major) => Some(*major),
        DriverVersion::MatchBrowser => browser_major,
        _ => None,
    };
    if let Some(found) =
        known_major.and_then(|major| downloaded_chromedriver_for_major(root, major))
    {
        return Ok(found);
    }
    if let DriverVersion::Exact(exact) = version {
        if let Some(found) = find_executable(
            &root.join(format!("chromedriver-{exact}")),
            &CHROMEDRIVER_NAMES,
        ) {
            return Ok(found);
        }
    }

    let client = http_client(proxy)?;
    let build = resolve_build(
        &client,
        version,
        Artifact::Chromedriver,
        platform_label()?,
        browser_major,
    )
    .await?;
    let folder = root.join(format!("chromedriver-{}", build.version));
    if let Some(found) = find_executable(&folder, &CHROMEDRIVER_NAMES) {
        return Ok(found);
    }
    install_archive(&client, &build.url, &folder, None).await?;
    let executable = find_executable(&folder, &CHROMEDRIVER_NAMES).ok_or_else(|| {
        SeleniumBaseError::download(&build.url, "the archive holds no chromedriver")
    })?;
    make_runnable(&executable, false)?;
    Ok(executable)
}

/// Downloads Chrome for Testing (`--cft`) into `downloaded_drivers/cft/`,
/// replacing an earlier download, and returns the browser's program.
///
/// # Errors
///
/// Returns an error if `version` is [`DriverVersion::MatchBrowser`], which names
/// no build, if the build cannot be found or downloaded, or its archive holds no
/// browser.
pub async fn install_chrome_for_testing(
    version: &DriverVersion,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    install_browser_in(
        Path::new(DOWNLOADS_DIR),
        Browser::ChromeForTesting,
        version,
        proxy,
    )
    .await
}

/// Downloads `chrome-headless-shell` (`--chs`) into `downloaded_drivers/chs/`,
/// replacing an earlier download, and returns its program. Errors as
/// [`install_chrome_for_testing`] does.
pub async fn install_chrome_headless_shell(
    version: &DriverVersion,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    install_browser_in(
        Path::new(DOWNLOADS_DIR),
        Browser::ChromeHeadlessShell,
        version,
        proxy,
    )
    .await
}

pub(crate) async fn install_browser_in(
    root: &Path,
    browser: Browser,
    version: &DriverVersion,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    let (artifact, folder) = match browser {
        Browser::ChromeForTesting => (Artifact::Chrome, CHROME_FOR_TESTING_DIR),
        Browser::ChromeHeadlessShell => (Artifact::ChromeHeadlessShell, CHROME_HEADLESS_SHELL_DIR),
        other => {
            return Err(SeleniumBaseError::Unsupported(format!(
                "Chrome for Testing does not publish {other}"
            )))
        }
    };
    if *version == DriverVersion::MatchBrowser {
        return Err(SeleniumBaseError::invalid_config(
            "a browser download needs a channel, a major version or a full version, not `browser`",
        ));
    }
    let client = http_client(proxy)?;
    let build = resolve_build(&client, version, artifact, platform_label()?, None).await?;
    let dest = root.join(folder);
    install_archive(&client, &build.url, &dest, None).await?;
    let executable = downloaded_executable(browser, &dest).ok_or_else(|| {
        SeleniumBaseError::download(&build.url, "the archive holds no browser program")
    })?;
    make_runnable(&executable, false)?;
    tracing::info!(version = %build.version, path = %executable.display(), "downloaded {browser}");
    Ok(executable)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;
    use zip::write::SimpleFileOptions;

    use super::*;

    fn last_known_good() -> Value {
        json!({
            "channels": {
                "Stable": {
                    "channel": "Stable",
                    "version": "126.0.6478.126",
                    "downloads": {
                        "chromedriver": [
                            {"platform": "linux64", "url": "https://x.test/126/chromedriver-linux64.zip"},
                            {"platform": "mac-arm64", "url": "https://x.test/126/chromedriver-mac-arm64.zip"}
                        ],
                        "chrome": [
                            {"platform": "linux64", "url": "https://x.test/126/chrome-linux64.zip"}
                        ]
                    }
                },
                "Beta": { "version": "127.0.6533.5", "downloads": { "chrome": [] } }
            }
        })
    }

    fn milestones() -> Value {
        json!({
            "milestones": {
                "114": {
                    "version": "114.0.5735.90",
                    "downloads": {
                        "chromedriver": [
                            {"platform": "linux64", "url": "https://x.test/114/chromedriver-linux64.zip"}
                        ]
                    }
                }
            }
        })
    }

    #[test]
    fn an_exact_version_has_the_published_archive_url() {
        assert_eq!(
            archive_url(Artifact::Chromedriver, "126.0.6478.126", "mac-arm64"),
            "https://storage.googleapis.com/chrome-for-testing-public/126.0.6478.126/mac-arm64/chromedriver-mac-arm64.zip"
        );
        assert_eq!(
            archive_url(Artifact::Chrome, "126.0.6478.126", "linux64"),
            "https://storage.googleapis.com/chrome-for-testing-public/126.0.6478.126/linux64/chrome-linux64.zip"
        );
        assert_eq!(
            archive_url(Artifact::ChromeHeadlessShell, "1.2.3.4", "win64"),
            "https://storage.googleapis.com/chrome-for-testing-public/1.2.3.4/win64/chrome-headless-shell-win64.zip"
        );
    }

    #[test]
    fn a_channel_gives_its_version_and_the_platforms_archive() {
        let build = channel_build(
            &last_known_good(),
            ReleaseChannel::Stable,
            Artifact::Chromedriver,
            "mac-arm64",
        )
        .unwrap();
        assert_eq!(build.version, "126.0.6478.126");
        assert_eq!(build.url, "https://x.test/126/chromedriver-mac-arm64.zip");
    }

    #[test]
    fn a_missing_channel_artifact_or_platform_is_an_error_that_says_which() {
        let listing = last_known_good();
        let canary = channel_build(
            &listing,
            ReleaseChannel::Canary,
            Artifact::Chrome,
            "linux64",
        )
        .unwrap_err();
        assert!(canary.to_string().contains("Canary"), "{canary}");
        let empty =
            channel_build(&listing, ReleaseChannel::Beta, Artifact::Chrome, "linux64").unwrap_err();
        assert!(empty.to_string().contains("Beta"), "{empty}");
        let platform =
            channel_build(&listing, ReleaseChannel::Stable, Artifact::Chrome, "win32").unwrap_err();
        assert!(platform.to_string().contains("win32"), "{platform}");
        let shell = channel_build(
            &listing,
            ReleaseChannel::Stable,
            Artifact::ChromeHeadlessShell,
            "linux64",
        )
        .unwrap_err();
        assert!(
            shell.to_string().contains("chrome-headless-shell"),
            "{shell}"
        );
    }

    #[test]
    fn a_milestone_gives_its_newest_build() {
        let build = milestone_build(&milestones(), 114, Artifact::Chromedriver, "linux64").unwrap();
        assert_eq!(build.version, "114.0.5735.90");
        assert!(
            milestone_build(&milestones(), 99, Artifact::Chromedriver, "linux64")
                .unwrap_err()
                .to_string()
                .contains("Chrome 99")
        );
    }

    #[test]
    fn a_proxy_is_an_http_proxy_unless_a_scheme_says_otherwise() {
        assert_eq!(proxy_url("host:8080"), "http://host:8080");
        assert_eq!(proxy_url("u:p@host:8080"), "http://u:p@host:8080");
        assert_eq!(proxy_url("socks5://host:1080"), "socks5://host:1080");
    }

    #[test]
    fn a_proxy_the_client_cannot_use_is_an_error_that_hides_the_password() {
        let error = http_client(Some("http://user:hunter2@[bad")).unwrap_err();
        assert!(!error.to_string().contains("hunter2"), "{error}");
        assert!(http_client(None).is_ok());
        assert!(http_client(Some("127.0.0.1:3128")).is_ok());
    }

    fn zip_of(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data, mode) in entries {
            let options = SimpleFileOptions::default().unix_permissions(*mode);
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn unpacking_keeps_the_executable_bit_and_finds_the_driver() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("chromedriver-1.2.3.4");
        let archive = zip_of(&[
            ("chromedriver-linux64/chromedriver", b"#!/bin/sh\n", 0o755),
            (
                "chromedriver-linux64/LICENSE.chromedriver",
                b"license",
                0o644,
            ),
        ]);
        replace_with_unpacked(&archive, &dest).unwrap();

        let found = find_executable(&dest, &CHROMEDRIVER_NAMES).unwrap();
        assert_eq!(found, dest.join("chromedriver-linux64/chromedriver"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&found).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "{mode:o}");
        }
    }

    #[test]
    fn a_new_unpack_replaces_the_old_folder_and_a_bad_archive_leaves_it_alone() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("cft");
        replace_with_unpacked(&zip_of(&[("old.txt", b"old", 0o644)]), &dest).unwrap();
        replace_with_unpacked(&zip_of(&[("new.txt", b"new", 0o644)]), &dest).unwrap();
        assert!(!dest.join("old.txt").exists());
        assert!(dest.join("new.txt").is_file());
        assert!(!sibling(&dest, ".partial").exists());

        assert!(replace_with_unpacked(b"not a zip", &dest).is_err());
        assert!(dest.join("new.txt").is_file(), "the good copy survived");
        assert!(!sibling(&dest, ".partial").exists());
    }

    #[test]
    fn an_entry_that_climbs_out_of_the_folder_is_not_written_outside_it() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("inner");
        let archive = zip_of(&[("../escaped.txt", b"x", 0o644)]);
        let _ = replace_with_unpacked(&archive, &dest);
        assert!(!dir.path().join("escaped.txt").exists());
    }

    #[test]
    fn a_driver_of_the_wanted_major_on_disk_is_found_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        for version in ["126.0.1.1", "126.0.2.2", "127.0.0.1"] {
            let folder = dir.path().join(format!("chromedriver-{version}"));
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("chromedriver"), b"x").unwrap();
        }
        // A folder with no driver in it is passed over.
        fs::create_dir_all(dir.path().join("chromedriver-126.9.9.9")).unwrap();

        let found = downloaded_chromedriver_for_major(dir.path(), 126).unwrap();
        assert_eq!(
            found,
            dir.path().join("chromedriver-126.0.2.2/chromedriver")
        );
        assert_eq!(downloaded_chromedriver_for_major(dir.path(), 125), None);
        assert_eq!(
            downloaded_chromedriver_for_major(&dir.path().join("none"), 126),
            None
        );
    }

    #[tokio::test]
    async fn a_driver_that_is_already_downloaded_needs_no_network() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("chromedriver-126.0.1.1");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("chromedriver"), b"x").unwrap();
        let expected = folder.join("chromedriver");

        // None of these may reach the network: an unroutable proxy would fail.
        let dead_proxy = Some("127.0.0.1:1");
        for (version, major) in [
            (DriverVersion::Milestone(126), None),
            (DriverVersion::MatchBrowser, Some(126)),
            (DriverVersion::Exact("126.0.1.1".into()), None),
        ] {
            let found = install_chromedriver_in(dir.path(), &version, major, dead_proxy)
                .await
                .unwrap();
            assert_eq!(found, expected, "{version}");
        }
    }

    #[tokio::test]
    async fn a_browser_download_needs_a_build_to_name() {
        let dir = tempfile::tempdir().unwrap();
        let error = install_browser_in(
            dir.path(),
            Browser::ChromeForTesting,
            &DriverVersion::MatchBrowser,
            None,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("browser"), "{error}");
        let error = install_browser_in(
            dir.path(),
            Browser::Brave,
            &DriverVersion::Channel(ReleaseChannel::Stable),
            None,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("brave"), "{error}");
    }

    /// A listing fetched through a proxy: the proxy sees the whole URL and the
    /// credentials, and its answer is what the client reads.
    #[tokio::test]
    async fn downloads_go_through_the_proxy_with_its_credentials() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let proxy = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut seen = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !seen.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).await.unwrap();
                assert!(read > 0, "the client hung up before sending a request");
                seen.extend_from_slice(&chunk[..read]);
            }
            let body = r#"{"channels":{"Stable":{"version":"9.9.9.9","downloads":{"chromedriver":[{"platform":"linux64","url":"https://x.test/d.zip"}]}}}}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&seen).into_owned()
        });

        let client = http_client(Some(&format!("user:s3cret@127.0.0.1:{port}"))).unwrap();
        let listing = fetch_json(&client, "http://listing.invalid/versions.json")
            .await
            .unwrap();
        let build = channel_build(
            &listing,
            ReleaseChannel::Stable,
            Artifact::Chromedriver,
            "linux64",
        )
        .unwrap();
        assert_eq!(build.version, "9.9.9.9");

        let request = proxy.await.unwrap();
        assert!(
            request.starts_with("GET http://listing.invalid/versions.json HTTP/1.1"),
            "{request}"
        );
        // Basic credentials for `user:s3cret`.
        assert!(
            request.contains("dXNlcjpzM2NyZXQ="),
            "no Proxy-Authorization in:\n{request}"
        );
    }
}
