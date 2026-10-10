//! Browser extensions that ship inside the crate, and the one SeleniumBase
//! writes for authenticated proxies.
//!
//! The Chromium archives are SeleniumBase's own, embedded unchanged:
//!
//! | Accessor | Extension (manifest v3) | What it does |
//! |---|---|---|
//! | [`get_ad_block_bytes`] | Simple Ad Blocker 1.0.0 | Blocks requests to `doubleclick.net` and `googleadservices.com` |
//! | [`get_disable_csp_bytes`] | Disable CSP 1.0.0 | Removes the `Content-Security-Policy` response headers |
//! | [`get_recorder_bytes`] | SeleniumBase Recorder 1.0.0 | Saves the page's actions to `sessionStorage` with CSS selectors |
//! | [`get_sbase_ext_bytes`] | SBase Extension 1.0.0 | Does nothing; for testing extension loading |
//! | [`get_firefox_addon_bytes`] | A signed Firefox add-on (`.xpi`) | Does nothing; for testing |
//!
//! Chrome loads an extension from a folder (`--load-extension=<folder>`), not
//! from an archive, so unpack one first with [`extract_extension`].
//!
//! Nothing in this crate builds the archives. They are binary files checked in
//! under `src/utils/extensions/assets/`, byte for byte the ones in Python
//! SeleniumBase's `seleniumbase/extensions/`; the source of each is inside it,
//! as plain JavaScript and JSON. Replace one by copying a new archive over
//! the old, and run this module's tests, which check that every archive is a
//! readable zip, that its `manifest.json` parses and that every file the
//! manifest names is in the archive.
//!
//! [`write_proxy_auth_extension`] and [`get_proxy_auth_extension`] are
//! different: they generate a small extension each time, because it has to
//! carry the proxy's credentials.

use std::fs::{self, File};
use std::io::{self, Cursor, Read};
use std::path::Path;

use crate::error::SeleniumBaseError;

pub mod ad_block;
pub mod disable_csp;
pub mod proxy_auth;
pub mod recorder;
pub mod sbase_ext;

pub use ad_block::get_ad_block_bytes;
pub use disable_csp::get_disable_csp_bytes;
pub use proxy_auth::{
    get_proxy_auth_extension, proxy_auth_background_js, proxy_auth_manifest,
    write_proxy_auth_extension,
};
pub use recorder::get_recorder_bytes;
pub use sbase_ext::{get_firefox_addon_bytes, get_sbase_ext_bytes};

/// The most an unpacked extension may take up: far more than any real one,
/// and little enough that an archive built to fill the disk is refused.
const MAX_UNPACKED_BYTES: u64 = 64 * 1024 * 1024;

/// Unpacks an extension archive into `dest`, creating the folder if needed.
///
/// The folder can then be given to Chrome with `--load-extension`. Entries
/// that only macOS's archiver adds (`__MACOSX/` and `.DS_Store`) are skipped.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if `archive` is not a zip file,
/// if an entry's path climbs out of `dest` with `..`, or if the archive
/// unpacks to more than 64 MiB; [`SeleniumBaseError::Io`] if a file cannot be
/// written. Files already unpacked when an error occurs are left in place. An
/// entry with an absolute path is unpacked inside `dest`, as if the path were
/// relative.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::extensions::{extract_extension, get_ad_block_bytes};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let dir = std::env::temp_dir().join(format!("sb-doc-ad-block-{}", std::process::id()));
/// extract_extension(get_ad_block_bytes(), &dir)?;
/// assert!(dir.join("manifest.json").is_file());
/// # std::fs::remove_dir_all(&dir)?;
/// # Ok(())
/// # }
/// ```
pub fn extract_extension(archive: &[u8], dest: &Path) -> Result<(), SeleniumBaseError> {
    extract_with_limit(archive, dest, MAX_UNPACKED_BYTES)
}

fn extract_with_limit(archive: &[u8], dest: &Path, limit: u64) -> Result<(), SeleniumBaseError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).map_err(|error| {
        SeleniumBaseError::InvalidConfig(format!("the extension is not a readable zip: {error}"))
    })?;
    fs::create_dir_all(dest)?;
    let mut written: u64 = 0;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!("entry {index} of the extension: {error}"))
        })?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "the extension has an entry that leaves its folder: {}",
                entry.name().map_or_else(
                    |error| format!("<unreadable name: {error}>"),
                    std::borrow::Cow::into_owned,
                )
            )));
        };
        if is_archiver_litter(&relative) {
            continue;
        }
        let target = dest.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        // Read at most one byte past what is left, so a lying size header
        // cannot get past the limit.
        let room = limit - written;
        let copied = io::copy(
            &mut (&mut entry).take(room + 1),
            &mut File::create(&target)?,
        )?;
        written += copied;
        if written > limit {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "the extension unpacks to more than {limit} bytes"
            )));
        }
    }
    Ok(())
}

/// Whether `path` is something macOS's archiver adds to a zip.
fn is_archiver_litter(path: &Path) -> bool {
    path.components()
        .next()
        .is_some_and(|first| first.as_os_str() == "__MACOSX")
        || path.file_name().is_some_and(|name| name == ".DS_Store")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::io::Write;

    use serde_json::Value;
    use zip::write::SimpleFileOptions;

    use super::*;

    /// Every embedded archive, with the name used in the failure message.
    fn assets() -> [(&'static str, &'static [u8]); 5] {
        [
            ("ad_block.zip", get_ad_block_bytes()),
            ("disable_csp.zip", get_disable_csp_bytes()),
            ("recorder.zip", get_recorder_bytes()),
            ("sbase_ext.zip", get_sbase_ext_bytes()),
            ("firefox_addon.xpi", get_firefox_addon_bytes()),
        ]
    }

    fn names_in(archive: &[u8]) -> BTreeSet<String> {
        let zip = zip::ZipArchive::new(Cursor::new(archive)).expect("a readable zip");
        zip.file_names()
            .map(|name| name.expect("a readable entry name").into_owned())
            .collect()
    }

    fn manifest_of(archive: &[u8]) -> Value {
        let mut zip = zip::ZipArchive::new(Cursor::new(archive)).expect("a readable zip");
        let mut text = String::new();
        zip.by_name("manifest.json")
            .expect("manifest.json at the top of the archive")
            .read_to_string(&mut text)
            .expect("manifest.json is UTF-8");
        serde_json::from_str(&text).expect("manifest.json is JSON")
    }

    /// The files in the archive that a manifest points at.
    fn files_named_by(manifest: &Value) -> Vec<String> {
        let mut files = Vec::new();
        let mut add = |value: &Value| {
            if let Some(path) = value.as_str() {
                files.push(path.to_owned());
            }
        };
        add(&manifest["background"]["service_worker"]);
        for script in manifest["background"]["scripts"]
            .as_array()
            .into_iter()
            .flatten()
        {
            add(script);
        }
        for content in manifest["content_scripts"].as_array().into_iter().flatten() {
            for key in ["js", "css"] {
                for file in content[key].as_array().into_iter().flatten() {
                    add(file);
                }
            }
        }
        for rules in manifest["declarative_net_request"]["rule_resources"]
            .as_array()
            .into_iter()
            .flatten()
        {
            add(&rules["path"]);
        }
        add(&manifest["options_page"]);
        for (_, icon) in manifest["icons"].as_object().into_iter().flatten() {
            add(icon);
        }
        for action in ["action", "browser_action"] {
            add(&manifest[action]["default_icon"]);
            for (_, icon) in manifest[action]["default_icon"]
                .as_object()
                .into_iter()
                .flatten()
            {
                add(icon);
            }
        }
        if let Some(locale) = manifest["default_locale"].as_str() {
            files.push(format!("_locales/{locale}/messages.json"));
        }
        files
    }

    #[test]
    fn every_embedded_archive_is_a_zip_with_a_manifest_that_parses() {
        for (name, bytes) in assets() {
            let manifest = manifest_of(bytes);
            for key in ["manifest_version", "name", "version"] {
                assert!(
                    !manifest[key].is_null(),
                    "{name}: manifest.json lacks {key}"
                );
            }
        }
    }

    #[test]
    fn embedded_manifests_have_the_versions_and_permissions_we_ship() {
        let strings = |value: &Value| -> Vec<String> {
            value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        };
        // (archive, manifest version, extension version, permissions)
        let expected: [(&str, u64, &str, &[&str]); 5] = [
            ("ad_block.zip", 3, "1.0.0", &["declarativeNetRequest"]),
            (
                "disable_csp.zip",
                3,
                "1.0.0",
                &[
                    "declarativeNetRequest",
                    "declarativeNetRequestWithHostAccess",
                    "storage",
                    "activeTab",
                ],
            ),
            ("recorder.zip", 3, "1.0.0", &["notifications"]),
            ("sbase_ext.zip", 3, "1.0.0", &["notifications"]),
            ("firefox_addon.xpi", 3, "0.1", &[]),
        ];
        for ((name, bytes), (expected_name, manifest_version, version, permissions)) in
            assets().into_iter().zip(expected)
        {
            assert_eq!(name, expected_name);
            let manifest = manifest_of(bytes);
            assert_eq!(manifest["manifest_version"], manifest_version, "{name}");
            assert_eq!(manifest["version"], version, "{name}");
            assert_eq!(strings(&manifest["permissions"]), permissions, "{name}");
        }
    }

    #[test]
    fn every_file_a_manifest_names_is_in_its_archive() {
        for (name, bytes) in assets() {
            let present = names_in(bytes);
            let manifest = manifest_of(bytes);
            let named = files_named_by(&manifest);
            assert!(!named.is_empty(), "{name}: the manifest names no files");
            for file in named {
                assert!(
                    present.contains(&file),
                    "{name}: manifest names {file}, not in the archive"
                );
            }
        }
    }

    #[test]
    fn the_firefox_addon_still_carries_its_mozilla_signature() {
        let present = names_in(get_firefox_addon_bytes());
        for file in ["META-INF/mozilla.rsa", "META-INF/manifest.mf", "inject.js"] {
            assert!(present.contains(file), "{file} is missing: {present:?}");
        }
    }

    #[test]
    fn extracting_an_embedded_archive_leaves_a_loadable_folder() {
        for (name, bytes) in assets() {
            let dir = tempfile::tempdir().expect("a temporary folder");
            extract_extension(bytes, dir.path()).expect(name);
            let manifest = manifest_of(bytes);
            assert_eq!(
                manifest,
                serde_json::from_slice::<Value>(
                    &fs::read(dir.path().join("manifest.json")).expect("manifest.json")
                )
                .expect("manifest.json parses"),
                "{name}"
            );
            for file in files_named_by(&manifest) {
                assert!(
                    dir.path().join(&file).is_file(),
                    "{name}: {file} was not unpacked"
                );
            }
            assert!(
                !dir.path().join("__MACOSX").exists(),
                "{name}: macOS litter unpacked"
            );
        }
    }

    /// A zip holding the given `(name, contents)` entries.
    fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, contents) in entries {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .expect("start an entry");
            writer.write_all(contents).expect("write an entry");
        }
        writer.finish().expect("finish the zip").into_inner()
    }

    #[test]
    fn an_entry_that_climbs_out_of_the_folder_is_refused() {
        let archive = zip_of(&[("ok.txt", b"fine"), ("../escaped.txt", b"nope")]);
        let outer = tempfile::tempdir().expect("a temporary folder");
        let dest = outer.path().join("extension");
        let error = extract_extension(&archive, &dest).expect_err("path traversal is refused");
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
        assert!(!outer.path().join("escaped.txt").exists());
    }

    #[test]
    fn an_absolute_entry_is_unpacked_inside_the_folder() {
        let archive = zip_of(&[("/tmp/sb-absolute-entry.txt", b"inside")]);
        let dest = tempfile::tempdir().expect("a temporary folder");
        extract_extension(&archive, dest.path()).expect("unpacks");
        assert!(dest.path().join("tmp/sb-absolute-entry.txt").is_file());
        assert!(!Path::new("/tmp/sb-absolute-entry.txt").exists());
    }

    #[test]
    fn an_archive_that_unpacks_too_large_is_refused_whatever_its_headers_say() {
        let archive = zip_of(&[("big.bin", &[0_u8; 4096])]);
        let dest = tempfile::tempdir().expect("a temporary folder");
        let error = extract_with_limit(&archive, dest.path(), 1000).expect_err("over the limit");
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
        // The limit counts what is written, so an archive at the limit passes.
        extract_with_limit(&archive, dest.path(), 4096).expect("exactly the limit");
    }

    #[test]
    fn something_that_is_not_a_zip_is_refused() {
        let dest = tempfile::tempdir().expect("a temporary folder");
        let error =
            extract_extension(b"this is not a zip file", dest.path()).expect_err("not an archive");
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
    }

    #[test]
    fn files_only_macos_adds_are_skipped() {
        let archive = zip_of(&[
            ("manifest.json", b"{}"),
            ("__MACOSX/._manifest.json", b"junk"),
            ("images/.DS_Store", b"junk"),
        ]);
        let dest = tempfile::tempdir().expect("a temporary folder");
        extract_extension(&archive, dest.path()).expect("unpacks");
        assert!(dest.path().join("manifest.json").is_file());
        assert!(!dest.path().join("__MACOSX").exists());
        assert!(!dest.path().join("images/.DS_Store").exists());
    }
}
