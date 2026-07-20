use std::path::PathBuf;

use crate::browser::downloader::download_chrome_driver;

pub async fn install_drivers() -> Result<PathBuf, crate::error::SeleniumBaseError> {
    download_chrome_driver().await
}
