//! Where the crate keeps the browser drivers it manages, and how it finds one.
//!
//! Python SeleniumBase installs drivers into a `drivers` folder next to the
//! package (`sbase get chromedriver`). The Rust counterpart is
//! [`DRIVERS_DIR`], a folder in the current directory that `sbase install`
//! fills and the local driver launcher reads. [`lookup_chromedriver`] answers
//! "which chromedriver would be used?" the same way everywhere.
//!
//! The lookup order is:
//!
//! 1. the file named by the `CHROMEDRIVER_PATH` environment variable, which
//!    wins when set, so a wrong value is reported instead of silently skipped;
//! 2. the driver in [`DRIVERS_DIR`];
//! 3. the first `chromedriver` on `PATH`.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::resources::assets::{chromedriver_path, DRIVERS_DIR};
//!
//! assert!(chromedriver_path().starts_with(DRIVERS_DIR));
//! ```

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The folder, relative to the working directory, that holds downloaded
/// drivers.
pub const DRIVERS_DIR: &str = "downloaded_drivers";

/// The environment variable that names a chromedriver binary explicitly.
pub const CHROMEDRIVER_PATH_VAR: &str = "CHROMEDRIVER_PATH";

/// The folder downloaded drivers are kept in.
#[must_use]
pub fn drivers_dir() -> PathBuf {
    PathBuf::from(DRIVERS_DIR)
}

/// The file name of the chromedriver binary on this platform.
#[must_use]
pub const fn chromedriver_file_name() -> &'static str {
    if cfg!(windows) {
        "chromedriver.exe"
    } else {
        "chromedriver"
    }
}

/// Where a downloaded chromedriver is kept, whether or not it exists yet.
#[must_use]
pub fn chromedriver_path() -> PathBuf {
    drivers_dir().join(chromedriver_file_name())
}

/// The result of looking for a chromedriver.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DriverLookup {
    /// A chromedriver binary was found here.
    Found(PathBuf),
    /// `CHROMEDRIVER_PATH` is set but names no file.
    MissingConfigured(PathBuf),
    /// There is no chromedriver in any of the places searched.
    NotFound,
}

impl DriverLookup {
    /// The path of the binary, if one was found.
    #[must_use]
    pub fn found(&self) -> Option<&Path> {
        match self {
            Self::Found(path) => Some(path),
            _ => None,
        }
    }
}

/// Looks for a chromedriver using the process environment and the current
/// directory; see the [module documentation](self) for the order.
#[must_use]
pub fn lookup_chromedriver() -> DriverLookup {
    let cwd = std::env::current_dir().unwrap_or_default();
    lookup_chromedriver_in(
        std::env::var_os(CHROMEDRIVER_PATH_VAR).as_deref(),
        &cwd.join(drivers_dir()),
        std::env::var_os("PATH").as_deref(),
        &cwd,
    )
}

/// Looks for a chromedriver in the given places, without reading the
/// environment.
///
/// `configured` is the value of `CHROMEDRIVER_PATH` (an empty value counts as
/// unset), `drivers_dir` the folder of downloaded drivers, `path_var` a
/// `PATH`-style list of folders, and `cwd` what a relative `PATH` entry is
/// relative to.
#[must_use]
pub fn lookup_chromedriver_in(
    configured: Option<&OsStr>,
    drivers_dir: &Path,
    path_var: Option<&OsStr>,
    cwd: &Path,
) -> DriverLookup {
    if let Some(configured) = configured.filter(|value| !value.is_empty()) {
        let path = PathBuf::from(configured);
        return if path.is_file() {
            DriverLookup::Found(path)
        } else {
            DriverLookup::MissingConfigured(path)
        };
    }
    let downloaded = drivers_dir.join(chromedriver_file_name());
    if downloaded.is_file() {
        return DriverLookup::Found(downloaded);
    }
    path_var
        .and_then(|paths| which::which_in("chromedriver", Some(paths), cwd).ok())
        .map_or(DriverLookup::NotFound, DriverLookup::Found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn the_downloaded_driver_lives_in_the_drivers_dir() {
        assert_eq!(
            chromedriver_path(),
            Path::new(DRIVERS_DIR).join(chromedriver_file_name())
        );
        assert_eq!(drivers_dir(), Path::new("downloaded_drivers"));
    }

    #[test]
    fn the_file_name_matches_the_platform() {
        if cfg!(windows) {
            assert_eq!(chromedriver_file_name(), "chromedriver.exe");
        } else {
            assert_eq!(chromedriver_file_name(), "chromedriver");
        }
    }

    #[test]
    fn nothing_anywhere_is_not_found() {
        let empty = tempfile::tempdir().unwrap();
        let found = lookup_chromedriver_in(
            None,
            empty.path(),
            Some(empty.path().as_os_str()),
            empty.path(),
        );
        assert_eq!(found, DriverLookup::NotFound);
        assert_eq!(found.found(), None);
    }

    #[test]
    fn a_downloaded_driver_is_found() {
        let drivers = tempfile::tempdir().unwrap();
        let driver = touch(drivers.path(), chromedriver_file_name());

        let found = lookup_chromedriver_in(None, drivers.path(), None, drivers.path());

        assert_eq!(found, DriverLookup::Found(driver.clone()));
        assert_eq!(found.found(), Some(driver.as_path()));
    }

    #[cfg(unix)]
    #[test]
    fn a_driver_on_the_path_is_found_when_none_was_downloaded() {
        let drivers = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let driver = touch(bin.path(), "chromedriver");

        let found = lookup_chromedriver_in(
            None,
            drivers.path(),
            Some(bin.path().as_os_str()),
            drivers.path(),
        );

        assert_eq!(found, DriverLookup::Found(driver));
    }

    #[cfg(unix)]
    #[test]
    fn a_downloaded_driver_beats_one_on_the_path() {
        let drivers = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let downloaded = touch(drivers.path(), "chromedriver");
        touch(bin.path(), "chromedriver");

        let found = lookup_chromedriver_in(
            None,
            drivers.path(),
            Some(bin.path().as_os_str()),
            drivers.path(),
        );

        assert_eq!(found, DriverLookup::Found(downloaded));
    }

    #[test]
    fn the_configured_path_beats_everything_else() {
        let drivers = tempfile::tempdir().unwrap();
        touch(drivers.path(), chromedriver_file_name());
        let elsewhere = tempfile::tempdir().unwrap();
        let chosen = touch(elsewhere.path(), "my-chromedriver");

        let found = lookup_chromedriver_in(
            Some(chosen.as_os_str()),
            drivers.path(),
            None,
            drivers.path(),
        );

        assert_eq!(found, DriverLookup::Found(chosen));
    }

    #[test]
    fn a_configured_path_that_names_nothing_is_reported_not_skipped() {
        let drivers = tempfile::tempdir().unwrap();
        touch(drivers.path(), chromedriver_file_name());
        let missing = drivers.path().join("no-such-driver");

        let found = lookup_chromedriver_in(
            Some(missing.as_os_str()),
            drivers.path(),
            None,
            drivers.path(),
        );

        assert_eq!(found, DriverLookup::MissingConfigured(missing));
    }

    #[test]
    fn an_empty_configured_path_counts_as_unset() {
        let drivers = tempfile::tempdir().unwrap();
        let driver = touch(drivers.path(), chromedriver_file_name());

        let found =
            lookup_chromedriver_in(Some(OsStr::new("")), drivers.path(), None, drivers.path());

        assert_eq!(found, DriverLookup::Found(driver));
    }
}
