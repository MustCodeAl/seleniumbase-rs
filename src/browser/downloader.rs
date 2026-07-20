use std::fs::{self, File};
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use reqwest::Client;
use zip::ZipArchive;

use crate::error::SeleniumBaseError;

/// Download and extract the latest chromedriver for the current platform.
pub async fn download_chrome_driver() -> Result<PathBuf, SeleniumBaseError> {
    let platform = platform_label()?;
    let client = Client::new();
    let version_info = fetch_version_info(&client).await?;
    let download_url = chromedriver_download_url(&version_info, platform)?;

    let dest_dir = PathBuf::from("downloaded_drivers");
    fs::create_dir_all(&dest_dir).map_err(io_error)?;

    let dest_path = extract_chromedriver(&client, &download_url, &dest_dir).await?;
    make_executable(&dest_path)?;
    Ok(dest_path)
}

fn platform_label() -> Result<&'static str, SeleniumBaseError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("mac-arm64"),
        ("macos", "x86_64") => Ok("mac-x64"),
        ("linux", _) => Ok("linux64"),
        ("windows", "x86_64") => Ok("win64"),
        ("windows", "x86") => Ok("win32"),
        other => Err(SeleniumBaseError::Unsupported(format!(
            "unsupported platform: {}-{}",
            other.0, other.1
        ))),
    }
}

async fn fetch_version_info(client: &Client) -> Result<serde_json::Value, SeleniumBaseError> {
    let url =
        "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json";
    let response = client.get(url).send().await.map_err(io_error)?;
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

async fn extract_chromedriver(
    client: &Client,
    download_url: &str,
    dest_dir: &Path,
) -> Result<PathBuf, SeleniumBaseError> {
    let response = client.get(download_url).send().await.map_err(io_error)?;
    let bytes = response.bytes().await.map_err(io_error)?;
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader).map_err(io_error)?;

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
            let mut outfile = File::create(&dest_path).map_err(io_error)?;
            io::copy(&mut file, &mut outfile).map_err(io_error)?;
            return Ok(dest_path);
        }
    }

    Err(SeleniumBaseError::Unsupported(
        "chromedriver executable not found in ZIP archive".to_owned(),
    ))
}

fn make_executable(path: &Path) -> Result<(), SeleniumBaseError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).map_err(io_error)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).map_err(io_error)?;
    }
    Ok(())
}

fn io_error(err: impl std::fmt::Display) -> SeleniumBaseError {
    SeleniumBaseError::Unsupported(err.to_string())
}
