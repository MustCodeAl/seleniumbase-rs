//! `sbase install` (also `sbase get`): fetch a driver.
//!
//! Python SeleniumBase's `sbase get` and `sbase install` take a driver name and
//! a version; so does this one, for chromedriver, geckodriver and msedgedriver.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use crate::browser::binaries::locate;
use crate::browser::chrome_for_testing::install_chromedriver;
use crate::browser::config::Browser;
use crate::browser::downloader::download_chrome_driver_for;
use crate::browser::driver_install::{install_geckodriver, install_msedgedriver};
use crate::browser::driver_version::binary_major;
use crate::browser::options::DriverVersion;
use crate::error::SeleniumBaseError;
use crate::stealth::patcher::find_system_chrome;

/// A driver `sbase install` can fetch.
///
/// The names Python SeleniumBase takes are read by [`FromStr`]: `chromedriver`,
/// `geckodriver` (also `firefoxdriver`), and `edgedriver` (also
/// `msedgedriver`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriverName {
    /// chromedriver, for Chrome.
    Chromedriver,
    /// geckodriver, for Firefox.
    Geckodriver,
    /// msedgedriver, for Microsoft Edge.
    Msedgedriver,
}

impl fmt::Display for DriverName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Chromedriver => "chromedriver",
            Self::Geckodriver => "geckodriver",
            Self::Msedgedriver => "msedgedriver",
        })
    }
}

impl FromStr for DriverName {
    type Err = SeleniumBaseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "chromedriver" => Ok(Self::Chromedriver),
            "geckodriver" | "firefoxdriver" => Ok(Self::Geckodriver),
            "edgedriver" | "msedgedriver" => Ok(Self::Msedgedriver),
            _ => Err(SeleniumBaseError::parse(
                text,
                "a driver: chromedriver, geckodriver (firefoxdriver), or edgedriver (msedgedriver)",
            )),
        }
    }
}

/// The major version of the browser at `program`, if it can be read.
async fn major_of(program: Option<PathBuf>) -> Option<u32> {
    match program {
        Some(program) => binary_major(&program).await,
        None => None,
    }
}

/// Downloads driver `name` into `./downloaded_drivers/` and returns the path of
/// the executable.
///
/// `version` is what Python's `sbase get NAME VERSION` takes. Without one,
/// chromedriver and msedgedriver are the drivers for the installed Chrome and
/// Edge (the latest stable ones when the browser is not found), and geckodriver
/// is the newest release. With one: `latest`, a major version (`155`) or a full
/// build for chromedriver and msedgedriver, and `latest` or a version such as
/// `0.36.0` for geckodriver. Needs network access unless that driver is in
/// `downloaded_drivers/` already.
///
/// # Errors
///
/// Fails on a version that names no driver, an unsupported platform, or a
/// listing or download that fails.
pub async fn install_driver(
    name: DriverName,
    version: Option<&str>,
) -> Result<PathBuf, SeleniumBaseError> {
    match name {
        DriverName::Chromedriver => match version {
            None => install_drivers().await,
            Some(version) => {
                let version: DriverVersion = version.parse()?;
                let chrome = major_of(find_system_chrome()).await;
                install_chromedriver(&version, chrome, None).await
            }
        },
        DriverName::Geckodriver => install_geckodriver(version, None).await,
        DriverName::Msedgedriver => {
            let version: DriverVersion = match version {
                None => DriverVersion::MatchBrowser,
                Some(version) => version.parse()?,
            };
            let edge = major_of(locate(Browser::Edge)).await;
            install_msedgedriver(&version, edge, None).await
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_driver_names_are_read() {
        for (text, name) in [
            ("chromedriver", DriverName::Chromedriver),
            ("geckodriver", DriverName::Geckodriver),
            ("FirefoxDriver", DriverName::Geckodriver),
            ("edgedriver", DriverName::Msedgedriver),
            (" msedgedriver ", DriverName::Msedgedriver),
        ] {
            assert_eq!(text.parse::<DriverName>().unwrap(), name, "{text}");
        }
        let error = "operadriver".parse::<DriverName>().unwrap_err();
        let text = error.to_string();
        assert!(
            text.contains("geckodriver") && text.contains("edgedriver"),
            "{text}"
        );
        assert_eq!(DriverName::Msedgedriver.to_string(), "msedgedriver");
    }

    #[tokio::test]
    async fn a_version_that_names_no_driver_is_refused_before_any_download() {
        for (name, version) in [
            (DriverName::Geckodriver, "v1"),
            (DriverName::Msedgedriver, "v155"),
            (DriverName::Chromedriver, "1.2.3"),
        ] {
            let error = install_driver(name, Some(version)).await.unwrap_err();
            assert!(error.to_string().contains(version), "{name}: {error}");
        }
    }
}
