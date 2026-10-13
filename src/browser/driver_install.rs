//! Downloading geckodriver, the driver of Firefox, and msedgedriver, the driver
//! of Microsoft Edge.
//!
//! Python SeleniumBase fetches both itself (`sbrs get geckodriver`,
//! `sbrs get edgedriver`, and its launcher when a session needs one), and so
//! does this module:
//!
//! - geckodriver comes from its GitHub releases: the newest one, or a version
//!   named like `0.36.0` or `v0.36.0`. One geckodriver drives a range of Firefox
//!   versions, so it is not matched to the installed Firefox.
//! - msedgedriver comes from Microsoft's download host. Like chromedriver it
//!   only drives the Edge of its own major version, so the default is the newest
//!   build for the installed Edge, which the host names in
//!   `LATEST_RELEASE_<major>_<OS>`.
//!
//! Each download is unpacked into a folder of its own under
//! `downloaded_drivers/` (`geckodriver-v0.37.1/`, `msedgedriver-155.0.4283.45/`),
//! so versions live side by side. An archive over [`MAX_ARCHIVE_BYTES`] is
//! refused, an entry that would land outside its folder is not written, and
//! the executable is made runnable (the execute bit, and on macOS no quarantine
//! flag; see [`make_runnable`]).
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::browser::driver_install::{edgedriver_url, geckodriver_tag, geckodriver_url};
//!
//! let tag = geckodriver_tag("0.36.0")?;
//! assert_eq!(
//!     geckodriver_url(&tag, "linux64.tar.gz"),
//!     "https://github.com/mozilla/geckodriver/releases/download/v0.36.0/geckodriver-v0.36.0-linux64.tar.gz"
//! );
//! assert_eq!(
//!     edgedriver_url("155.0.4283.45", "mac64_m1"),
//!     "https://msedgedriver.microsoft.com/155.0.4283.45/edgedriver_mac64_m1.zip"
//! );
//! # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use reqwest::Client;

use crate::browser::binaries::{find_executable, DOWNLOADS_DIR};
use crate::browser::chrome_for_testing::{
    downloaded_for_major, fetch_bytes, http_client, install_archive,
};
use crate::browser::driver_access::make_runnable;
use crate::browser::options::DriverVersion;
use crate::error::SeleniumBaseError;

/// Where geckodriver is released.
pub const GECKODRIVER_RELEASES: &str = "https://github.com/mozilla/geckodriver/releases";

/// Microsoft's download host for msedgedriver.
pub const EDGEDRIVER_HOST: &str = "https://msedgedriver.microsoft.com";

/// The largest driver archive that is accepted. Both drivers are far smaller
/// (geckodriver about 3 MB, msedgedriver about 10 MB).
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;

/// The largest `LATEST_*` file that is read; it holds one version.
const MAX_LISTING_BYTES: u64 = 1024;

/// The names a geckodriver executable goes by.
pub(crate) const GECKODRIVER_NAMES: [&str; 2] = ["geckodriver", "geckodriver.exe"];

/// The names an msedgedriver executable goes by.
pub(crate) const MSEDGEDRIVER_NAMES: [&str; 2] = ["msedgedriver", "msedgedriver.exe"];

/// The end of the name of geckodriver's release archive for `os` and `arch` (as
/// `std::env::consts` spells them), such as `macos-aarch64.tar.gz`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Unsupported`] for a system geckodriver publishes
/// no build for.
pub fn geckodriver_asset(os: &str, arch: &str) -> Result<&'static str, SeleniumBaseError> {
    match (os, arch) {
        ("macos", "aarch64") => Ok("macos-aarch64.tar.gz"),
        ("macos", "x86_64") => Ok("macos.tar.gz"),
        ("linux", "x86_64") => Ok("linux64.tar.gz"),
        ("linux", "aarch64") => Ok("linux-aarch64.tar.gz"),
        ("linux", "x86") => Ok("linux32.tar.gz"),
        ("windows", "x86_64") => Ok("win64.zip"),
        ("windows", "x86") => Ok("win32.zip"),
        ("windows", "aarch64") => Ok("win-aarch64.zip"),
        (os, arch) => Err(SeleniumBaseError::Unsupported(format!(
            "geckodriver publishes no build for {os}-{arch}; put one in {DOWNLOADS_DIR}/ or on the PATH"
        ))),
    }
}

/// The release tag of geckodriver `version`: `0.36.0` and `v0.36.0` both give
/// `v0.36.0`.
///
/// # Errors
///
/// Returns a parse error for anything but two or three dot-separated numbers,
/// with or without a leading `v`.
pub fn geckodriver_tag(version: &str) -> Result<String, SeleniumBaseError> {
    let trimmed = version.trim();
    let bare = trimmed.strip_prefix(['v', 'V']).unwrap_or(trimmed);
    let parts: Vec<&str> = bare.split('.').collect();
    let numeric = (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    if numeric {
        Ok(format!("v{bare}"))
    } else {
        Err(SeleniumBaseError::parse(
            version,
            "a geckodriver version such as 0.36.0 or v0.36.0, or latest",
        ))
    }
}

/// The URL of geckodriver's archive `asset` (see [`geckodriver_asset`]) in the
/// release `tag` (see [`geckodriver_tag`]).
#[must_use]
pub fn geckodriver_url(tag: &str, asset: &str) -> String {
    format!("{GECKODRIVER_RELEASES}/download/{tag}/geckodriver-{tag}-{asset}")
}

/// The tag at the end of the address `/releases/latest` sends to:
/// `.../releases/tag/v0.37.1` gives `v0.37.1`.
fn tag_from_release_url(url: &str) -> Option<String> {
    let (_, tag) = url.trim_end_matches('/').rsplit_once("/releases/tag/")?;
    geckodriver_tag(tag).ok()
}

/// The name Microsoft gives the platform of `os` and `arch` in its msedgedriver
/// archives, such as `mac64_m1` in `edgedriver_mac64_m1.zip`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Unsupported`] for a system Microsoft publishes
/// no msedgedriver for.
pub fn edgedriver_platform(os: &str, arch: &str) -> Result<&'static str, SeleniumBaseError> {
    match (os, arch) {
        ("macos", "aarch64") => Ok("mac64_m1"),
        ("macos", "x86_64") => Ok("mac64"),
        ("linux", "x86_64") => Ok("linux64"),
        ("windows", "x86_64") => Ok("win64"),
        ("windows", "x86") => Ok("win32"),
        ("windows", "aarch64") => Ok("arm64"),
        (os, arch) => Err(SeleniumBaseError::Unsupported(format!(
            "Microsoft publishes no msedgedriver for {os}-{arch}; put one in {DOWNLOADS_DIR}/ or on the PATH"
        ))),
    }
}

/// The URL of the msedgedriver archive of build `version` for `platform` (see
/// [`edgedriver_platform`]).
#[must_use]
pub fn edgedriver_url(version: &str, platform: &str) -> String {
    format!("{EDGEDRIVER_HOST}/{version}/edgedriver_{platform}.zip")
}

/// The file on Microsoft's host that names the build `version` means, or `None`
/// when `version` is a build already.
///
/// A major version (or the installed Edge's, `edge_major`, for
/// [`DriverVersion::MatchBrowser`]) is read from `LATEST_RELEASE_<major>_<OS>`,
/// a channel from `LATEST_<CHANNEL>`; without a known Edge the stable build is
/// used. `os` is spelled as `std::env::consts::OS` spells it.
#[must_use]
pub fn edgedriver_listing_url(
    version: &DriverVersion,
    edge_major: Option<u32>,
    os: &str,
) -> Option<String> {
    let os = match os {
        "macos" => "MACOS",
        "windows" => "WINDOWS",
        _ => "LINUX",
    };
    let release = |major: u32| format!("{EDGEDRIVER_HOST}/LATEST_RELEASE_{major}_{os}");
    match version {
        DriverVersion::Exact(_) => None,
        DriverVersion::Milestone(major) => Some(release(*major)),
        DriverVersion::MatchBrowser => {
            Some(edge_major.map_or_else(|| format!("{EDGEDRIVER_HOST}/LATEST_STABLE"), release))
        }
        DriverVersion::Channel(channel) => Some(format!(
            "{EDGEDRIVER_HOST}/LATEST_{}",
            channel.listing_name().to_ascii_uppercase()
        )),
    }
}

/// Whether `text` is a full build number: four dot-separated numbers.
fn is_build(text: &str) -> bool {
    matches!(DriverVersion::from_str(text), Ok(DriverVersion::Exact(_)))
}

/// The build a `LATEST_*` file of Microsoft's host names.
///
/// The files are UTF-16 with a byte order mark and end in `\r\n`; UTF-8 is read
/// too.
///
/// # Errors
///
/// Returns a download error when the first line is not a full build number.
pub fn read_release_text(bytes: &[u8]) -> Result<String, SeleniumBaseError> {
    fn utf16(bytes: &[u8], word: fn([u8; 2]) -> u16) -> String {
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().copied().map(word).collect();
        String::from_utf16_lossy(&units)
    }
    let text = match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, u16::from_le_bytes),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, u16::from_be_bytes),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    };
    let first = text.lines().next().unwrap_or_default().trim();
    if is_build(first) {
        Ok(first.to_owned())
    } else {
        let shown: String = first.chars().take(40).collect();
        Err(SeleniumBaseError::download(
            EDGEDRIVER_HOST,
            format!("the msedgedriver listing names no build: {shown:?}"),
        ))
    }
}

/// A version of a driver's folder name, as numbers, for picking the newest:
/// `geckodriver-v0.37.1` gives `[0, 37, 1]`.
fn folder_version(name: &str, stem: &str) -> Option<Vec<u32>> {
    let rest = name.strip_prefix(stem)?.strip_prefix('-')?;
    let rest = rest.strip_prefix('v').unwrap_or(rest);
    rest.split('.').map(|part| part.parse().ok()).collect()
}

/// The driver `program` (named by `stem` and `names`) that `root` holds: a file
/// right in it, else the one in the newest `<stem>-<version>/` folder.
pub(crate) fn downloaded_driver(root: &Path, stem: &str, names: &[&str]) -> Option<PathBuf> {
    if let Some(flat) = names
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
    {
        return Some(flat);
    }
    let mut folders: Vec<(Vec<u32>, PathBuf)> = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let version = folder_version(entry.file_name().to_str()?, stem)?;
            Some((version, entry.path()))
        })
        .collect();
    folders.sort();
    folders
        .iter()
        .rev()
        .find_map(|(_, folder)| find_executable(folder, names))
}

/// Downloads the archive at `url` into `folder` and returns the executable in
/// it, made runnable.
async fn install_driver_archive(
    client: &Client,
    url: &str,
    folder: &Path,
    names: &[&str],
    what: &str,
) -> Result<PathBuf, SeleniumBaseError> {
    install_archive(client, url, folder, Some(MAX_ARCHIVE_BYTES)).await?;
    let executable = find_executable(folder, names)
        .ok_or_else(|| SeleniumBaseError::download(url, format!("the archive holds no {what}")))?;
    make_runnable(&executable, false)?;
    tracing::info!(path = %executable.display(), "downloaded {what}");
    Ok(executable)
}

/// The tag of the newest geckodriver release.
///
/// `/releases/latest` redirects to the page of that release, so the address the
/// client ends at names it. Unlike GitHub's API this has no rate limit for
/// clients that are not signed in.
async fn latest_geckodriver_tag(client: &Client) -> Result<String, SeleniumBaseError> {
    let url = format!("{GECKODRIVER_RELEASES}/latest");
    let response = client
        .head(&url)
        .send()
        .await
        .map_err(|error| SeleniumBaseError::download(&url, error.to_string()))?;
    if !response.status().is_success() {
        return Err(SeleniumBaseError::network(
            &url,
            response.status().as_u16(),
            response.status().to_string(),
        ));
    }
    tag_from_release_url(response.url().as_str()).ok_or_else(|| {
        SeleniumBaseError::download(
            &url,
            format!("the newest release is not named in {}", response.url()),
        )
    })
}

/// Makes sure geckodriver `version` (`0.36.0`, `v0.36.0`; the newest release
/// when `None` or `latest`) is in `downloaded_drivers/`, and returns its path.
///
/// A pinned version already there is used without asking the network. `proxy`
/// is a proxy for the downloads (`SERVER:PORT` or `USER:PASS@SERVER:PORT`).
///
/// # Errors
///
/// Returns an error for a version that is not one, a system with no geckodriver
/// build, or a download that fails or holds no geckodriver.
pub async fn install_geckodriver(
    version: Option<&str>,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    install_geckodriver_in(Path::new(DOWNLOADS_DIR), version, proxy).await
}

pub(crate) async fn install_geckodriver_in(
    root: &Path,
    version: Option<&str>,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    let pinned = match version.map(str::trim) {
        None | Some("") => None,
        Some(latest) if latest.eq_ignore_ascii_case("latest") => None,
        Some(version) => Some(geckodriver_tag(version)?),
    };
    let asset = geckodriver_asset(std::env::consts::OS, std::env::consts::ARCH)?;
    let in_folder =
        |tag: &str| find_executable(&root.join(format!("geckodriver-{tag}")), &GECKODRIVER_NAMES);
    if let Some(found) = pinned.as_deref().and_then(in_folder) {
        return Ok(found);
    }
    let client = http_client(proxy)?;
    let tag = match pinned {
        Some(tag) => tag,
        None => latest_geckodriver_tag(&client).await?,
    };
    if let Some(found) = in_folder(&tag) {
        return Ok(found);
    }
    install_driver_archive(
        &client,
        &geckodriver_url(&tag, asset),
        &root.join(format!("geckodriver-{tag}")),
        &GECKODRIVER_NAMES,
        "geckodriver",
    )
    .await
}

/// Makes sure the msedgedriver `version` means is in `downloaded_drivers/`, and
/// returns its path.
///
/// `edge_major` is the major version of the installed Edge, which
/// [`DriverVersion::MatchBrowser`] follows; without it the stable build is
/// used. A driver of the wanted major version (or the exact build) that is
/// already there is used without asking the network. `proxy` is a proxy for the
/// downloads (`SERVER:PORT` or `USER:PASS@SERVER:PORT`).
///
/// # Errors
///
/// Returns an error for an exact version that is not a build number, a system
/// with no msedgedriver build, or a listing or download that fails or holds no
/// msedgedriver.
pub async fn install_msedgedriver(
    version: &DriverVersion,
    edge_major: Option<u32>,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    install_msedgedriver_in(Path::new(DOWNLOADS_DIR), version, edge_major, proxy).await
}

pub(crate) async fn install_msedgedriver_in(
    root: &Path,
    version: &DriverVersion,
    edge_major: Option<u32>,
    proxy: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    let platform = edgedriver_platform(std::env::consts::OS, std::env::consts::ARCH)?;
    let known_major = match version {
        DriverVersion::Milestone(major) => Some(*major),
        DriverVersion::MatchBrowser => edge_major,
        _ => None,
    };
    if let Some(found) = known_major
        .and_then(|major| downloaded_for_major(root, "msedgedriver", major, &MSEDGEDRIVER_NAMES))
    {
        return Ok(found);
    }
    let in_folder = |build: &str| {
        find_executable(
            &root.join(format!("msedgedriver-{build}")),
            &MSEDGEDRIVER_NAMES,
        )
    };
    if let DriverVersion::Exact(exact) = version {
        // The build becomes part of a path and a URL, so it must be a number.
        if !is_build(exact) {
            return Err(SeleniumBaseError::parse(
                exact.as_str(),
                "an msedgedriver build such as 155.0.4283.45",
            ));
        }
        if let Some(found) = in_folder(exact) {
            return Ok(found);
        }
    }
    let client = http_client(proxy)?;
    let build = match edgedriver_listing_url(version, edge_major, std::env::consts::OS) {
        None => version.to_string(),
        Some(listing) => {
            read_release_text(&fetch_bytes(&client, &listing, Some(MAX_LISTING_BYTES)).await?)?
        }
    };
    if let Some(found) = in_folder(&build) {
        return Ok(found);
    }
    install_driver_archive(
        &client,
        &edgedriver_url(&build, platform),
        &root.join(format!("msedgedriver-{build}")),
        &MSEDGEDRIVER_NAMES,
        "msedgedriver",
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::chrome_for_testing::replace_with_unpacked;
    use crate::browser::options::ReleaseChannel;

    #[test]
    fn each_system_gets_the_geckodriver_archive_published_for_it() {
        for (os, arch, asset) in [
            ("macos", "aarch64", "macos-aarch64.tar.gz"),
            ("macos", "x86_64", "macos.tar.gz"),
            ("linux", "x86_64", "linux64.tar.gz"),
            ("linux", "aarch64", "linux-aarch64.tar.gz"),
            ("linux", "x86", "linux32.tar.gz"),
            ("windows", "x86_64", "win64.zip"),
            ("windows", "x86", "win32.zip"),
            ("windows", "aarch64", "win-aarch64.zip"),
        ] {
            assert_eq!(geckodriver_asset(os, arch).unwrap(), asset, "{os}-{arch}");
        }
        let error = geckodriver_asset("freebsd", "x86_64").unwrap_err();
        assert!(error.to_string().contains("freebsd-x86_64"), "{error}");
    }

    #[test]
    fn a_geckodriver_version_becomes_its_release_tag() {
        assert_eq!(geckodriver_tag("0.36.0").unwrap(), "v0.36.0");
        assert_eq!(geckodriver_tag(" v0.37.1 ").unwrap(), "v0.37.1");
        assert_eq!(geckodriver_tag("V0.34").unwrap(), "v0.34");
        for bad in [
            "",
            "latest",
            "v",
            "0",
            "0.36.0.1",
            "0.x.1",
            "../0.36.0",
            "0.36.0/../x",
        ] {
            assert!(geckodriver_tag(bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            geckodriver_url("v0.37.1", "macos-aarch64.tar.gz"),
            "https://github.com/mozilla/geckodriver/releases/download/v0.37.1/geckodriver-v0.37.1-macos-aarch64.tar.gz"
        );
    }

    #[test]
    fn the_newest_release_is_read_from_where_latest_redirects() {
        assert_eq!(
            tag_from_release_url("https://github.com/mozilla/geckodriver/releases/tag/v0.37.1")
                .as_deref(),
            Some("v0.37.1")
        );
        assert_eq!(
            tag_from_release_url("https://github.com/mozilla/geckodriver/releases/latest"),
            None
        );
        assert_eq!(
            tag_from_release_url("https://github.com/mozilla/geckodriver/releases/tag/nightly"),
            None
        );
    }

    #[test]
    fn each_system_gets_the_edgedriver_archive_published_for_it() {
        for (os, arch, platform) in [
            ("macos", "aarch64", "mac64_m1"),
            ("macos", "x86_64", "mac64"),
            ("linux", "x86_64", "linux64"),
            ("windows", "x86_64", "win64"),
            ("windows", "x86", "win32"),
            ("windows", "aarch64", "arm64"),
        ] {
            assert_eq!(
                edgedriver_platform(os, arch).unwrap(),
                platform,
                "{os}-{arch}"
            );
        }
        assert!(edgedriver_platform("linux", "aarch64").is_err());
        assert_eq!(
            edgedriver_url("155.0.4283.45", "linux64"),
            "https://msedgedriver.microsoft.com/155.0.4283.45/edgedriver_linux64.zip"
        );
    }

    #[test]
    fn the_edgedriver_listing_follows_the_installed_edge_and_the_version_asked_for() {
        let host = EDGEDRIVER_HOST;
        assert_eq!(
            edgedriver_listing_url(&DriverVersion::MatchBrowser, Some(155), "macos"),
            Some(format!("{host}/LATEST_RELEASE_155_MACOS"))
        );
        assert_eq!(
            edgedriver_listing_url(&DriverVersion::MatchBrowser, None, "linux"),
            Some(format!("{host}/LATEST_STABLE"))
        );
        assert_eq!(
            edgedriver_listing_url(&DriverVersion::Milestone(140), Some(155), "windows"),
            Some(format!("{host}/LATEST_RELEASE_140_WINDOWS"))
        );
        assert_eq!(
            edgedriver_listing_url(&DriverVersion::Channel(ReleaseChannel::Beta), None, "linux"),
            Some(format!("{host}/LATEST_BETA"))
        );
        assert_eq!(
            edgedriver_listing_url(&DriverVersion::Exact("155.0.1.2".into()), Some(9), "macos"),
            None
        );
    }

    #[test]
    fn the_listing_is_read_in_utf16_as_microsoft_serves_it_and_in_utf8() {
        // The bytes LATEST_STABLE held on 2026-10-12.
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain(
                "155.0.4283.45\r\n"
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes),
            )
            .collect();
        assert_eq!(read_release_text(&utf16).unwrap(), "155.0.4283.45");
        let big_endian: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain("140.0.1.2".encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert_eq!(read_release_text(&big_endian).unwrap(), "140.0.1.2");
        assert_eq!(
            read_release_text(b"\xEF\xBB\xBF1.2.3.4\n").unwrap(),
            "1.2.3.4"
        );
        assert_eq!(read_release_text(b"1.2.3.4").unwrap(), "1.2.3.4");
        for bad in [
            &b""[..],
            b"<html>not found</html>",
            b"../../1.2.3.4",
            b"1.2.3",
        ] {
            assert!(read_release_text(bad).is_err(), "{bad:?}");
        }
    }

    fn tar_gz_of(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        for (name, data, mode) in entries {
            let mut header = tar::Header::new_gnu();
            // Written by hand, since `set_path` refuses a name that climbs out.
            let field = &mut header.as_old_mut().name;
            field[..name.len()].copy_from_slice(name.as_bytes());
            header.set_size(data.len() as u64);
            header.set_mode(*mode);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            builder.append(&header, *data).unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn a_tar_gz_unpacks_with_its_driver_and_nothing_outside_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("geckodriver-v0.37.1");
        let archive = tar_gz_of(&[
            ("geckodriver", b"#!/bin/sh\n", 0o755),
            ("../escaped.txt", b"x", 0o644),
        ]);
        replace_with_unpacked(&archive, &dest).unwrap();

        let found = find_executable(&dest, &GECKODRIVER_NAMES).unwrap();
        assert_eq!(found, dest.join("geckodriver"));
        assert!(!dir.path().join("escaped.txt").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&found).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "{mode:o}");
        }
        assert!(replace_with_unpacked(&[0x1f, 0x8b, 0, 1, 2], &dest).is_err());
        assert!(dest.join("geckodriver").is_file(), "the good copy survived");
    }

    #[test]
    fn a_driver_in_the_folder_wins_over_the_newest_versioned_one() {
        let dir = tempfile::tempdir().unwrap();
        for version in ["v0.9.0", "v0.36.0", "v0.37.1"] {
            let folder = dir.path().join(format!("geckodriver-{version}"));
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join("geckodriver"), b"x").unwrap();
        }
        // A folder with no driver in it, or a name that is no version, is passed over.
        fs::create_dir_all(dir.path().join("geckodriver-v9.9.9")).unwrap();
        fs::create_dir_all(dir.path().join("geckodriver-old")).unwrap();
        assert_eq!(
            downloaded_driver(dir.path(), "geckodriver", &GECKODRIVER_NAMES),
            Some(dir.path().join("geckodriver-v0.37.1/geckodriver"))
        );
        fs::write(dir.path().join("geckodriver"), b"x").unwrap();
        assert_eq!(
            downloaded_driver(dir.path(), "geckodriver", &GECKODRIVER_NAMES),
            Some(dir.path().join("geckodriver"))
        );
        assert_eq!(
            downloaded_driver(dir.path(), "msedgedriver", &MSEDGEDRIVER_NAMES),
            None
        );
    }

    #[tokio::test]
    async fn a_driver_that_is_already_downloaded_needs_no_network() {
        let dir = tempfile::tempdir().unwrap();
        let edge = dir.path().join("msedgedriver-155.0.4283.45");
        fs::create_dir_all(&edge).unwrap();
        fs::write(edge.join("msedgedriver"), b"x").unwrap();
        let gecko = dir.path().join("geckodriver-v0.36.0");
        fs::create_dir_all(&gecko).unwrap();
        fs::write(gecko.join("geckodriver"), b"x").unwrap();

        // None of these may reach the network: an unroutable proxy would fail.
        let dead_proxy = Some("127.0.0.1:1");
        if edgedriver_platform(std::env::consts::OS, std::env::consts::ARCH).is_ok() {
            for (version, major) in [
                (DriverVersion::Milestone(155), None),
                (DriverVersion::MatchBrowser, Some(155)),
                (DriverVersion::Exact("155.0.4283.45".into()), None),
            ] {
                let found = install_msedgedriver_in(dir.path(), &version, major, dead_proxy)
                    .await
                    .unwrap();
                assert_eq!(found, edge.join("msedgedriver"), "{version}");
            }
        }
        if geckodriver_asset(std::env::consts::OS, std::env::consts::ARCH).is_ok() {
            for pinned in ["0.36.0", "v0.36.0"] {
                let found = install_geckodriver_in(dir.path(), Some(pinned), dead_proxy)
                    .await
                    .unwrap();
                assert_eq!(found, gecko.join("geckodriver"), "{pinned}");
            }
        }
    }

    #[tokio::test]
    async fn a_build_that_is_not_a_number_never_reaches_a_path_or_the_network() {
        if edgedriver_platform(std::env::consts::OS, std::env::consts::ARCH).is_err() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let error = install_msedgedriver_in(
            dir.path(),
            &DriverVersion::Exact("../../etc".into()),
            None,
            Some("127.0.0.1:1"),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("../../etc"), "{error}");
    }

    /// A server on localhost that answers every request with `body`, and tells
    /// what it was asked.
    async fn serve_once(
        body: Vec<u8>,
        announce: Option<usize>,
    ) -> (String, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut seen = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !seen.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                seen.extend_from_slice(&chunk[..read]);
            }
            let length = announce.map_or(String::new(), |n| format!("Content-Length: {n}\r\n"));
            let head = format!("HTTP/1.1 200 OK\r\n{length}Connection: close\r\n\r\n");
            let _ = stream.write_all(head.as_bytes()).await;
            let _ = stream.write_all(&body).await;
            String::from_utf8_lossy(&seen).into_owned()
        });
        (url, task)
    }

    #[tokio::test]
    async fn a_download_over_the_limit_is_refused() {
        let client = http_client(None).unwrap();
        // Announced too big.
        let (url, task) = serve_once(vec![b'x'; 10], Some(10)).await;
        let error = fetch_bytes(&client, &url, Some(4)).await.unwrap_err();
        assert!(error.to_string().contains("larger than 4 bytes"), "{error}");
        let _ = task.await;
        // Not announced, and too big once read.
        let (url, task) = serve_once(vec![b'x'; 10], None).await;
        let error = fetch_bytes(&client, &url, Some(4)).await.unwrap_err();
        assert!(error.to_string().contains("larger than 4 bytes"), "{error}");
        let _ = task.await;
        // Within the limit.
        let (url, task) = serve_once(b"1.2.3.4".to_vec(), Some(7)).await;
        assert_eq!(
            fetch_bytes(&client, &url, Some(1024)).await.unwrap(),
            b"1.2.3.4"
        );
        let request = task.await.unwrap();
        assert!(request.starts_with("GET /file HTTP/1.1"), "{request}");
    }

    #[test]
    fn the_folder_version_is_read_as_numbers() {
        assert_eq!(
            folder_version("geckodriver-v0.37.1", "geckodriver"),
            Some(vec![0, 37, 1])
        );
        assert_eq!(
            folder_version("msedgedriver-155.0.4283.45", "msedgedriver"),
            Some(vec![155, 0, 4283, 45])
        );
        assert_eq!(folder_version("geckodriver-old", "geckodriver"), None);
        assert_eq!(folder_version("chromedriver-1.2.3.4", "geckodriver"), None);
    }
}
