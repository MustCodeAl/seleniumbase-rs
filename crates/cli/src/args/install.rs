//! `sbrs install` (also `sbrs get`): the request its arguments make, and what it prints.
//!
//! The declarations themselves (`DriverArg`, the version words offered to the shell and the
//! `Install` command) stay in `sbrs.rs`, where the parser is built; this file holds the logic
//! that needs a test of its own.

use seleniumbase_rs::cli::scripts::sb_install::{DriverName, InstallRequest, Installed};

use crate::failure::ArgumentError;
use crate::DriverArg;

/// The request the arguments of `sbrs install` make.
///
/// # Errors
///
/// An [`ArgumentError`] when the arguments ask for what the program cannot do: a `--revision`
/// for anything but chromium, a revision no snapshot has, a `--path` for a browser. Nothing has
/// been downloaded by then.
pub(crate) fn request(
    driver: DriverArg,
    version: Option<String>,
    path: bool,
    revision: Option<u64>,
) -> Result<InstallRequest, ArgumentError> {
    let mut request = InstallRequest::new(DriverName::from(driver)).with_copy_to_path(path);
    request.version = version;
    request.revision = revision;
    request
        .validate()
        .map_err(|error| ArgumentError::new("install", error.to_string()))?;
    Ok(request)
}

/// The lines `sbrs install` prints once the download is done.
pub(crate) fn report(name: DriverName, installed: &Installed) -> String {
    let mut lines = vec![match &installed.version {
        Some(version) => format!("{name} {version} installed at {}", installed.path.display()),
        None => format!("{name} installed at {}", installed.path.display()),
    }];
    if let Some(driver) = &installed.driver {
        lines.push(format!(
            "chromedriver of the same revision at {}",
            driver.display()
        ));
    }
    if let Some(copied) = &installed.copied_to {
        lines.push(format!("copied to {}", copied.display()));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn the_arguments_make_the_request() {
        let made = request(DriverArg::UcDriver, Some("stable".into()), true, None).unwrap();
        assert_eq!(made.name, DriverName::UcDriver);
        assert_eq!(made.version.as_deref(), Some("stable"));
        assert!(made.copy_to_path);
        assert_eq!(made.revision, None);

        let made = request(DriverArg::Chromium, None, false, Some(1_725_796)).unwrap();
        assert_eq!(made.name, DriverName::Chromium);
        assert_eq!(made.revision, Some(1_725_796));
    }

    #[test]
    fn what_a_program_cannot_do_is_refused_before_any_download() {
        for (driver, path, revision, said) in [
            (
                DriverArg::Chromedriver,
                false,
                Some(1),
                "--revision applies to chromium only",
            ),
            (
                DriverArg::Geckodriver,
                false,
                Some(1),
                "--revision applies to chromium only",
            ),
            (DriverArg::Chromium, false, Some(0), "Chromium revision"),
            (
                DriverArg::Cft,
                true,
                None,
                "--path copies drivers, and cft is a browser",
            ),
            (
                DriverArg::Chs,
                true,
                None,
                "--path copies drivers, and chs is a browser",
            ),
            (
                DriverArg::Chromium,
                true,
                None,
                "--path copies drivers, and chromium is a browser",
            ),
        ] {
            let error = request(driver, None, path, revision).unwrap_err();
            assert!(error.to_string().contains(said), "{driver:?}: {error}");
        }
    }

    #[test]
    fn the_report_names_the_version_the_driver_and_the_copy() {
        let installed = Installed {
            path: PathBuf::from("downloaded_drivers/chromedriver-141.0.7390.76/chromedriver"),
            version: Some("141.0.7390.76".into()),
            driver: None,
            copied_to: Some(PathBuf::from("/usr/local/bin/chromedriver")),
        };
        assert_eq!(
            report(DriverName::Chromedriver, &installed),
            "chromedriver 141.0.7390.76 installed at \
             downloaded_drivers/chromedriver-141.0.7390.76/chromedriver\n\
             copied to /usr/local/bin/chromedriver"
        );
        let browser = Installed {
            path: PathBuf::from("downloaded_drivers/chromium/chrome-linux/chrome"),
            version: Some("r1725796".into()),
            driver: Some(PathBuf::from(
                "downloaded_drivers/chromedriver-150.0.1725796.0/c",
            )),
            copied_to: None,
        };
        let text = report(DriverName::Chromium, &browser);
        assert!(
            text.starts_with("chromium r1725796 installed at "),
            "{text}"
        );
        assert!(
            text.contains("\nchromedriver of the same revision at "),
            "{text}"
        );
        let plain = Installed {
            path: PathBuf::from("downloaded_drivers/cft/chrome"),
            version: None,
            driver: None,
            copied_to: None,
        };
        assert_eq!(
            report(DriverName::Cft, &plain),
            "cft installed at downloaded_drivers/cft/chrome"
        );
    }
}
