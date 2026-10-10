//! `sbase install`: fetch a chromedriver.

use std::path::PathBuf;

use crate::browser::downloader::download_chrome_driver;

/// Downloads the chromedriver that Chrome for Testing lists for its Stable
/// channel into `./downloaded_drivers/` and returns the path of the executable.
///
/// The driver is the latest Stable one, not one chosen for the Chrome that is
/// installed: if that Chrome is older or on another channel, the driver may
/// refuse to start it. It also only fetches `chromedriver`; the CDP engine
/// needs no driver at all. Needs network access.
///
/// # Errors
///
/// Fails if the platform is unsupported, the network request fails or the
/// archive cannot be unpacked.
///
/// ```no_run
/// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let driver = seleniumbase_rs::cli::scripts::sb_install::install_drivers().await?;
/// println!("chromedriver is at {}", driver.display());
/// # Ok(())
/// # }
/// ```
pub async fn install_drivers() -> Result<PathBuf, crate::error::SeleniumBaseError> {
    download_chrome_driver().await
}
