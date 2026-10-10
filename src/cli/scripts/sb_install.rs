//! `sbase install`: fetch a chromedriver.

use std::path::PathBuf;

use crate::browser::downloader::download_chrome_driver_for;
use crate::browser::driver_version::binary_major;
use crate::error::SeleniumBaseError;
use crate::stealth::patcher::find_system_chrome;

/// Downloads the chromedriver that matches the installed Chrome into
/// `./downloaded_drivers/` and returns the path of the executable.
///
/// A chromedriver only drives the Chrome of its own major version, so the driver
/// is the newest build of the installed Chrome's milestone, taken from Chrome for
/// Testing. When Chrome is not found, or Chrome for Testing lists no driver for
/// its version, the latest Stable driver is fetched instead. It only fetches
/// `chromedriver`; the CDP engine needs no driver at all. Needs network access.
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
pub async fn install_drivers() -> Result<PathBuf, SeleniumBaseError> {
    let browser = match find_system_chrome() {
        Some(chrome) => binary_major(&chrome).await,
        None => None,
    };
    match download_chrome_driver_for(browser).await {
        Err(error) if browser.is_some() => {
            tracing::warn!(%error, "no chromedriver for the installed Chrome; using the stable one");
            download_chrome_driver_for(None).await
        }
        other => other,
    }
}
