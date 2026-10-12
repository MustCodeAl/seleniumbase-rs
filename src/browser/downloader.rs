use std::fs::{self, File};
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use reqwest::Client;
use zip::ZipArchive;

use crate::browser::driver_access::make_runnable;
use crate::error::SeleniumBaseError;

/// The Chrome for Testing listing of the newest stable build.
const STABLE_LISTING: &str =
    "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json";

/// The Chrome for Testing listing of the newest build of every milestone.
const MILESTONE_LISTING: &str =
    "https://googlechromelabs.github.io/chrome-for-testing/latest-versions-per-milestone-with-downloads.json";

/// Download and extract the latest stable chromedriver for the current platform.
///
/// A chromedriver only drives the Chrome of its own major version, so prefer
/// [`download_chrome_driver_for`] when the installed Chrome is known.
pub async fn download_chrome_driver() -> Result<PathBuf, SeleniumBaseError> {
    download_chrome_driver_for(None).await
}

/// Download and extract the chromedriver for Chrome `major` (the newest build of
/// that milestone), or the latest stable one when `major` is `None`.
///
/// # Errors
///
/// Returns an error when the listing cannot be fetched, when no chromedriver is
/// published for `major` on this platform, or when the archive cannot be read.
pub async fn download_chrome_driver_for(major: Option<u32>) -> Result<PathBuf, SeleniumBaseError> {
    let platform = platform_label()?;
    let client = Client::new();
    let download_url = match major {
        None => {
            let listing = fetch_version_info(&client, STABLE_LISTING).await?;
            chromedriver_download_url(&listing, platform)?
        }
        Some(major) => {
            let listing = fetch_version_info(&client, MILESTONE_LISTING).await?;
            chromedriver_milestone_url(&listing, major, platform)?
        }
    };

    let dest_dir = PathBuf::from("downloaded_drivers");
    fs::create_dir_all(&dest_dir).map_err(io_error)?;

    let dest_path = extract_chromedriver(&client, &download_url, &dest_dir).await?;
    // Executable, and free of the macOS quarantine flag if the archive had one.
    make_runnable(&dest_path, false)?;
    Ok(dest_path)
}

/// The Chrome for Testing name of this machine's platform, such as `mac-arm64`.
pub(crate) fn platform_label() -> Result<&'static str, SeleniumBaseError> {
    label_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// The Chrome for Testing name of the platform `os` and `arch` (as `std::env::consts` spells
/// them) make up.
fn label_for(os: &str, arch: &str) -> Result<&'static str, SeleniumBaseError> {
    match (os, arch) {
        ("macos", "aarch64") => Ok("mac-arm64"),
        ("macos", "x86_64") => Ok("mac-x64"),
        ("linux", "x86_64") => Ok("linux64"),
        // Chrome for Testing has published an arm64 Linux build since Chrome 153.
        ("linux", "aarch64") => Ok("linux-arm64"),
        ("linux", arch) => Err(SeleniumBaseError::Unsupported(format!(
            "unsupported platform: linux-{arch}; Chrome for Testing publishes no build for it, \
             so put a chromedriver for this machine at downloaded_drivers/chromedriver"
        ))),
        ("windows", "x86_64") => Ok("win64"),
        ("windows", "x86") => Ok("win32"),
        (os, arch) => Err(SeleniumBaseError::Unsupported(format!(
            "unsupported platform: {os}-{arch}"
        ))),
    }
}

async fn fetch_version_info(
    client: &Client,
    url: &str,
) -> Result<serde_json::Value, SeleniumBaseError> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| map_download_error(url, e))?;
    if !response.status().is_success() {
        return Err(map_network_error(url, response));
    }
    response.json().await.map_err(io_error)
}

fn chromedriver_download_url(
    version_info: &serde_json::Value,
    platform: &str,
) -> Result<String, SeleniumBaseError> {
    let stable = &version_info["channels"]["Stable"];
    let downloads = stable["downloads"]["chromedriver"]
        .as_array()
        .ok_or_else(|| {
            SeleniumBaseError::Unsupported("invalid chromedriver download metadata".to_owned())
        })?;

    downloads
        .iter()
        .find(|download| download["platform"].as_str() == Some(platform))
        .and_then(|download| download["url"].as_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            SeleniumBaseError::Unsupported(format!(
                "no chromedriver download found for platform: {platform}"
            ))
        })
}

/// The download URL of the chromedriver for Chrome `major`, from the
/// per-milestone listing.
fn chromedriver_milestone_url(
    listing: &serde_json::Value,
    major: u32,
    platform: &str,
) -> Result<String, SeleniumBaseError> {
    let downloads = listing["milestones"][major.to_string().as_str()]["downloads"]["chromedriver"]
        .as_array()
        .ok_or_else(|| {
            SeleniumBaseError::Unsupported(format!(
                "no chromedriver is published for Chrome {major}"
            ))
        })?;

    downloads
        .iter()
        .find(|download| download["platform"].as_str() == Some(platform))
        .and_then(|download| download["url"].as_str())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            SeleniumBaseError::Unsupported(format!(
                "no chromedriver for Chrome {major} is published for platform: {platform}"
            ))
        })
}

async fn extract_chromedriver(
    client: &Client,
    download_url: &str,
    dest_dir: &Path,
) -> Result<PathBuf, SeleniumBaseError> {
    let response = client
        .get(download_url)
        .send()
        .await
        .map_err(|e| map_download_error(download_url, e))?;
    if !response.status().is_success() {
        return Err(map_network_error(download_url, response));
    }
    let bytes = response.bytes().await.map_err(io_error)?;
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| {
        SeleniumBaseError::download(download_url, format!("failed to open ZIP archive: {e}"))
    })?;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(io_error)?;
        let file_name = file
            .enclosed_name()
            .and_then(|path| path.file_name().map(|name| name.to_owned()));

        let Some(file_name) = file_name else {
            continue;
        };

        if file_name == "chromedriver" || file_name == "chromedriver.exe" {
            let dest_path = dest_dir.join(&file_name);
            // Replace the file instead of overwriting it in place: macOS caches
            // a binary's signature per file, and a new binary written into the
            // same file is killed for not matching it.
            match fs::remove_file(&dest_path) {
                Ok(()) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(io_error(err)),
            }
            let mut outfile = File::create(&dest_path).map_err(io_error)?;
            io::copy(&mut file, &mut outfile).map_err(io_error)?;
            return Ok(dest_path);
        }
    }

    Err(SeleniumBaseError::download(
        download_url,
        "chromedriver executable not found in ZIP archive",
    ))
}

fn io_error(err: impl std::fmt::Display) -> SeleniumBaseError {
    SeleniumBaseError::Io(std::io::Error::other(err.to_string()))
}

fn map_download_error(url: impl Into<String>, err: impl std::fmt::Display) -> SeleniumBaseError {
    SeleniumBaseError::download(url, err.to_string())
}

fn map_network_error(url: impl Into<String>, response: reqwest::Response) -> SeleniumBaseError {
    let status = response.status().as_u16();
    SeleniumBaseError::network(url, status, response.status().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing() -> serde_json::Value {
        serde_json::json!({
            "milestones": {
                "155": {
                    "version": "155.0.8059.39",
                    "downloads": {
                        "chromedriver": [
                            { "platform": "linux64", "url": "https://example.test/155/linux64.zip" },
                            { "platform": "mac-arm64", "url": "https://example.test/155/mac-arm64.zip" }
                        ]
                    }
                },
                "140": { "version": "140.0.1.2", "downloads": { "chrome": [] } }
            }
        })
    }

    #[test]
    fn the_milestone_listing_gives_the_driver_for_that_chrome_and_platform() {
        let url = chromedriver_milestone_url(&listing(), 155, "mac-arm64").unwrap();
        assert_eq!(url, "https://example.test/155/mac-arm64.zip");
    }

    #[test]
    fn linux_on_arm_gets_the_arm_build_and_never_the_x86_one() {
        // An x86-64 chromedriver cannot run on an arm64 machine, so a Docker image on an
        // Apple-silicon or Graviton host failed to start its driver.
        assert_eq!(label_for("linux", "aarch64").unwrap(), "linux-arm64");
        assert_eq!(label_for("linux", "x86_64").unwrap(), "linux64");
    }

    #[test]
    fn a_linux_architecture_with_no_build_is_refused_and_named() {
        for arch in ["x86", "riscv64", "s390x"] {
            let error = label_for("linux", arch).unwrap_err();
            assert!(
                error.to_string().contains(&format!("linux-{arch}")),
                "{arch}: {error}"
            );
        }
    }

    #[test]
    fn the_other_systems_keep_their_labels() {
        assert_eq!(label_for("macos", "aarch64").unwrap(), "mac-arm64");
        assert_eq!(label_for("macos", "x86_64").unwrap(), "mac-x64");
        assert_eq!(label_for("windows", "x86_64").unwrap(), "win64");
        assert_eq!(label_for("windows", "x86").unwrap(), "win32");
        assert!(label_for("freebsd", "x86_64").is_err());
    }

    #[test]
    fn a_milestone_or_platform_with_no_driver_is_an_error_that_names_it() {
        let missing_milestone =
            chromedriver_milestone_url(&listing(), 99, "mac-arm64").unwrap_err();
        assert!(
            missing_milestone.to_string().contains("Chrome 99"),
            "{missing_milestone}"
        );
        let no_driver_listed =
            chromedriver_milestone_url(&listing(), 140, "mac-arm64").unwrap_err();
        assert!(
            no_driver_listed.to_string().contains("Chrome 140"),
            "{no_driver_listed}"
        );
        let wrong_platform = chromedriver_milestone_url(&listing(), 155, "win32").unwrap_err();
        assert!(
            wrong_platform.to_string().contains("win32"),
            "{wrong_platform}"
        );
    }
}
