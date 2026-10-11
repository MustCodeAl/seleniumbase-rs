//! Chromium extensions given as a `.zip` or `.crx` file.

use std::fmt;
use std::path::{Path, PathBuf};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::error::SeleniumBaseError;

/// A Chromium extension packed in a `.zip` or `.crx` file.
///
/// Python SeleniumBase takes a comma-separated list of them as
/// `--extension-zip` (also `--crx`). Each file reaches the browser through the
/// `extensions` option of `goog:chromeOptions`, which chromedriver unpacks and
/// loads; Chrome 137 and later also need the
/// `DisableLoadExtensionCommandLineSwitch` feature off, which the launch
/// arguments take care of.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::browser::options::ExtensionArchive;
///
/// let all = ExtensionArchive::parse_list("ublock.crx, tools/helper.zip")?;
/// assert_eq!(all.len(), 2);
/// assert!(ExtensionArchive::new("notes.txt").is_err());
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(try_from = "PathBuf", into = "PathBuf")]
pub struct ExtensionArchive(PathBuf);

impl ExtensionArchive {
    /// An extension archive at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file name does not end in `.zip` or `.crx`. The
    /// file itself is read when a session starts.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, SeleniumBaseError> {
        let path = path.into();
        let packed = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("zip") || extension.eq_ignore_ascii_case("crx")
            });
        if packed {
            Ok(Self(path))
        } else {
            Err(SeleniumBaseError::parse(
                path.display().to_string(),
                "the path of a .zip or .crx extension file",
            ))
        }
    }

    /// Reads a comma-separated list of archive paths, as `--extension-zip`
    /// takes.
    ///
    /// # Errors
    ///
    /// Returns an error for the first entry that is not a `.zip` or `.crx` file.
    pub fn parse_list(text: &str) -> Result<Vec<Self>, SeleniumBaseError> {
        text.split(',')
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(Self::new)
            .collect()
    }

    /// The path of the archive.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// The archive's bytes, base64-encoded for the `extensions` capability.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn encoded(&self) -> Result<String, SeleniumBaseError> {
        let bytes = std::fs::read(&self.0).map_err(|error| {
            SeleniumBaseError::invalid_config(format!(
                "cannot read the extension file {}: {error}",
                self.0.display()
            ))
        })?;
        Ok(STANDARD.encode(bytes))
    }
}

impl TryFrom<PathBuf> for ExtensionArchive {
    type Error = SeleniumBaseError;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Self::new(path)
    }
}

impl From<ExtensionArchive> for PathBuf {
    fn from(archive: ExtensionArchive) -> Self {
        archive.0
    }
}

impl fmt::Display for ExtensionArchive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

/// An extension that ships inside this crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Bundled {
    /// Removes the `Content-Security-Policy` response headers.
    DisableCsp,
    /// The SeleniumBase Recorder.
    Recorder,
}

impl Bundled {
    /// The extension's archive, as SeleniumBase packs it.
    pub(crate) fn archive(self) -> &'static [u8] {
        match self {
            Self::DisableCsp => crate::utils::extensions::get_disable_csp_bytes(),
            Self::Recorder => crate::utils::extensions::get_recorder_bytes(),
        }
    }

    /// The archive, base64-encoded for the `extensions` capability.
    pub(crate) fn encoded(self) -> String {
        STANDARD.encode(self.archive())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use super::*;

    #[test]
    fn only_zip_and_crx_files_are_extensions() {
        for ok in ["a.zip", "dir/b.CRX", "c.Zip"] {
            assert!(ExtensionArchive::new(ok).is_ok(), "{ok}");
        }
        for bad in ["a.txt", "a", "a.zip.bak", "", "zip"] {
            assert!(ExtensionArchive::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_comma_separated_list_is_split_trimmed_and_checked() {
        let list = ExtensionArchive::parse_list(" a.zip ,b.crx,, ").unwrap();
        let paths: Vec<_> = list.iter().map(|a| a.path().to_owned()).collect();
        assert_eq!(paths, [PathBuf::from("a.zip"), PathBuf::from("b.crx")]);
        assert!(ExtensionArchive::parse_list("a.zip,b.exe").is_err());
        assert!(ExtensionArchive::parse_list("").unwrap().is_empty());
    }

    #[test]
    fn the_capability_holds_the_files_bytes_in_base64() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.crx");
        std::fs::write(&path, b"Cr24 not really").unwrap();
        let encoded = ExtensionArchive::new(&path).unwrap().encoded().unwrap();
        assert_eq!(STANDARD.decode(encoded).unwrap(), b"Cr24 not really");
    }

    #[test]
    fn a_missing_file_is_an_error_that_names_it() {
        let error = ExtensionArchive::new("/no/such/ext.zip")
            .unwrap()
            .encoded()
            .unwrap_err();
        assert!(error.to_string().contains("/no/such/ext.zip"), "{error}");
    }

    #[test]
    fn it_is_stored_as_a_plain_path() {
        let archive = ExtensionArchive::new("a/b.zip").unwrap();
        assert_eq!(serde_json::to_string(&archive).unwrap(), r#""a/b.zip""#);
        assert!(serde_json::from_str::<ExtensionArchive>(r#""a/b.txt""#).is_err());
    }

    #[test]
    fn the_bundled_extensions_are_readable_zips_with_a_manifest() {
        for bundled in [Bundled::DisableCsp, Bundled::Recorder] {
            let decoded = STANDARD.decode(bundled.encoded()).unwrap();
            assert_eq!(decoded, bundled.archive());
            let mut zip = zip::ZipArchive::new(Cursor::new(decoded)).unwrap();
            let mut manifest = String::new();
            zip.by_name("manifest.json")
                .unwrap()
                .read_to_string(&mut manifest)
                .unwrap();
            let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
            assert_eq!(manifest["manifest_version"], 3, "{bundled:?}");
        }
    }
}
