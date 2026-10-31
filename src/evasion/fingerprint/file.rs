//! Reading and writing a [`Fingerprint`] as a file: the one file format of every
//! tool, the v2 JSON.

use std::fs;
use std::io::Write;
use std::path::Path;

use super::Fingerprint;
use crate::error::SeleniumBaseError;

/// The most a fingerprint file may hold. A real one is a few kilobytes; the cap
/// keeps a wrong path (a log, a disk image) from being read into memory.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;

impl Fingerprint {
    /// The most bytes [`load`](Self::load) reads from one file.
    pub const MAX_FILE_BYTES: u64 = MAX_FILE_BYTES;

    /// Reads a fingerprint from JSON text.
    ///
    /// The text is read as the builder would build it: a `"random"` setting is
    /// drawn (give a `seed` for a repeatable file), an `"off"` where it means
    /// nothing is an error, and a field this version does not know is an error.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] when the text is not a fingerprint,
    /// with the reason and the place in the text.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::Fingerprint;
    ///
    /// let fp = Fingerprint::from_json(r#"{ "timezone": "Europe/Berlin" }"#)?;
    /// assert_eq!(fp.timezone().as_str(), Some("Europe/Berlin"));
    ///
    /// // A field this version does not know is an error, not a silent loss.
    /// assert!(Fingerprint::from_json(r#"{ "screen_width": 1920 }"#).is_err());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn from_json(text: &str) -> Result<Self, SeleniumBaseError> {
        parse(text).map_err(SeleniumBaseError::InvalidConfig)
    }

    /// The fingerprint as indented JSON: the file format. A `Natural` setting is
    /// left out.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Json`] if the fingerprint cannot be written, which
    /// no fingerprint a builder made can cause.
    pub fn to_json_pretty(&self) -> Result<String, SeleniumBaseError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Reads a fingerprint file.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Io`] when the file cannot be read, and
    /// [`SeleniumBaseError::InvalidConfig`], naming the file, when it is larger
    /// than [`MAX_FILE_BYTES`](Self::MAX_FILE_BYTES) or is not a fingerprint (see
    /// [`from_json`](Self::from_json)).
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SeleniumBaseError> {
        let path = path.as_ref();
        let io = |action: &str, error: std::io::Error| {
            SeleniumBaseError::Io(std::io::Error::new(
                error.kind(),
                format!(
                    "cannot {action} the fingerprint file {}: {error}",
                    path.display()
                ),
            ))
        };
        let size = fs::metadata(path).map_err(|error| io("read", error))?.len();
        if size > MAX_FILE_BYTES {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "the fingerprint file {} is {size} bytes, more than the {MAX_FILE_BYTES} a \
                 fingerprint file may have",
                path.display()
            )));
        }
        let text = fs::read_to_string(path).map_err(|error| io("read", error))?;
        parse(&text).map_err(|reason| {
            SeleniumBaseError::InvalidConfig(format!(
                "the fingerprint file {}: {reason}",
                path.display()
            ))
        })
    }

    /// Writes the fingerprint to `path` as indented JSON, so a reader sees the
    /// old file or the new one and never half of either.
    ///
    /// The text goes to a temporary file in the same folder, which is flushed and
    /// then renamed over `path`. The folder is created when it is missing. The
    /// file is private to its owner (mode `0600` on Unix), because a fingerprint
    /// may hold the login of a proxy.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Io`] when the folder or the file cannot be written; the
    /// file that was there is then left as it was.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::Fingerprint;
    ///
    /// let dir = tempfile::tempdir()?;
    /// let path = dir.path().join("profiles").join("windows.json");
    ///
    /// Fingerprint::windows_desktop().save(&path)?;
    ///
    /// assert_eq!(Fingerprint::load(&path)?, Fingerprint::windows_desktop());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SeleniumBaseError> {
        let path = path.as_ref();
        let mut text = self.to_json_pretty()?;
        text.push('\n');
        let io = |action: &str, error: std::io::Error| {
            SeleniumBaseError::Io(std::io::Error::new(
                error.kind(),
                format!(
                    "cannot {action} the fingerprint file {}: {error}",
                    path.display()
                ),
            ))
        };
        let folder = path
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(folder).map_err(|error| io("create the folder of", error))?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(folder).map_err(|error| io("write", error))?;
        temporary
            .write_all(text.as_bytes())
            .and_then(|()| temporary.flush())
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|error| io("write", error))?;
        temporary
            .persist(path)
            .map_err(|error| io("replace", error.error))?;
        Ok(())
    }
}

/// The fingerprint in `text`, or why it is not one: where the JSON breaks, or which setting
/// holds what, by name and kind of value. The value itself is never in the reason: the text
/// may be a file a tool was only asked to read, and the reason goes back to whoever asked.
fn parse(text: &str) -> Result<Fingerprint, String> {
    serde_json::from_str(text).map_err(|error| {
        // A syntax error says where the JSON breaks and quotes none of the text.
        if !error.is_data() {
            return format!("not a fingerprint: {error}");
        }
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(serde_json::Value::Object(_)) => match misplaced_setting(text) {
                Some((key, value)) => format!(
                    "not a fingerprint: `{key}` (holding {}) is not a setting this version can \
                     read",
                    kind_of(&value)
                ),
                // Every setting reads on its own, so the reason is the builder's, which names
                // settings, not a value of the text.
                None => format!("not a fingerprint: {error}"),
            },
            Ok(other) => format!(
                "not a fingerprint: the text is {}, not an object of settings",
                kind_of(&other)
            ),
            Err(_) => "not a fingerprint: the text is not JSON".to_owned(),
        }
    })
}

/// What kind of JSON value `value` is, in words, without the value.
fn kind_of(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "true or false",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "a list",
        serde_json::Value::Object(_) => "an object",
    }
}

/// The first top-level key of `text` whose value is not valid for it, with the
/// value.
fn misplaced_setting(text: &str) -> Option<(String, serde_json::Value)> {
    let serde_json::Value::Object(entries) = serde_json::from_str(text).ok()? else {
        return None;
    };
    entries.into_iter().find(|(key, value)| {
        let alone = serde_json::json!({ key: value });
        serde_json::from_value::<super::Fields>(alone).is_err()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evasion::fingerprint::Spoof;

    fn presets() -> Vec<(&'static str, Fingerprint)> {
        vec![
            ("windows", Fingerprint::windows_desktop()),
            ("edge", Fingerprint::edge_desktop()),
            ("macos", Fingerprint::macos_desktop()),
            ("linux", Fingerprint::linux_desktop()),
            ("android", Fingerprint::android_mobile()),
            ("ios", Fingerprint::ios_mobile_safari()),
        ]
    }

    #[test]
    fn every_preset_survives_a_save_and_a_load() {
        let dir = tempfile::tempdir().unwrap();

        for (name, preset) in presets() {
            let path = dir.path().join(format!("{name}.json"));

            preset.save(&path).unwrap();

            assert_eq!(Fingerprint::load(&path).unwrap(), preset, "{name}");
        }
    }

    #[test]
    fn a_drawn_identity_survives_a_save_and_a_load_with_its_infinite_battery_times() {
        let dir = tempfile::tempdir().unwrap();
        let drawn = Fingerprint::randomized(crate::evasion::fingerprint::OsType::Macos, 99);
        let path = dir.path().join("drawn.json");

        drawn.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();

        assert_eq!(Fingerprint::load(&path).unwrap(), drawn);
        assert!(text.ends_with("}\n"), "the file ends with a newline");
        assert!(text.contains("\n  \""), "the file is indented");
    }

    #[test]
    fn to_json_pretty_is_what_save_writes() {
        let dir = tempfile::tempdir().unwrap();
        let fp = Fingerprint::linux_desktop();
        let path = dir.path().join("fp.json");

        fp.save(&path).unwrap();

        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("{}\n", fp.to_json_pretty().unwrap())
        );
    }

    #[test]
    fn save_makes_the_folder_it_needs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("fp.json");

        Fingerprint::windows_desktop().save(&path).unwrap();

        assert!(path.is_file());
    }

    #[test]
    fn save_replaces_a_file_and_leaves_no_temporary_one_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fp.json");
        fs::write(&path, "old content").unwrap();

        Fingerprint::macos_desktop().save(&path).unwrap();

        assert_eq!(
            Fingerprint::load(&path).unwrap(),
            Fingerprint::macos_desktop()
        );
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["fp.json"], "no temporary file is left");
    }

    #[cfg(unix)]
    #[test]
    fn a_saved_file_is_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fp.json");

        Fingerprint::windows_desktop().save(&path).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn a_save_that_cannot_write_leaves_the_old_file_as_it_was() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fp.json");
        fs::write(&path, "keep me").unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();

        let result = Fingerprint::windows_desktop().save(&path);

        // Put the folder right again first, so the temporary folder can be removed.
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        // A process that may write anywhere (root) cannot show this.
        if result.is_ok() {
            return;
        }
        assert!(matches!(result, Err(SeleniumBaseError::Io(_))));
        assert_eq!(fs::read_to_string(&path).unwrap(), "keep me");
    }

    #[test]
    fn a_file_that_is_not_there_is_an_io_error_that_names_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.json");

        let error = Fingerprint::load(&path).unwrap_err();

        assert!(matches!(error, SeleniumBaseError::Io(_)), "{error:?}");
        assert!(error.to_string().contains("missing.json"), "{error}");
        assert_eq!(error.exit_code(), 6);
    }

    #[test]
    fn a_file_that_is_not_a_fingerprint_is_an_invalid_config_with_its_name_and_the_setting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.json");
        fs::write(&path, "{\n  \"timezone\": 54321\n}\n").unwrap();

        let error = Fingerprint::load(&path).unwrap_err();

        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
        let message = error.to_string();
        assert!(message.contains("broken.json"), "{message}");
        assert!(
            message.contains("`timezone`"),
            "it names the setting: {message}"
        );
        assert!(!message.contains("54321"), "the value stays out: {message}");
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn a_reason_names_the_setting_and_the_kind_of_value_never_the_value() {
        for (text, wanted) in [
            (r#""hunter2-token""#, "a string, not an object"),
            (r#"[1, "hunter2-token"]"#, "not a fingerprint"),
            (
                r#"{ "timezone": ["hunter2-token"] }"#,
                "`timezone` (holding a list)",
            ),
            (
                r#"{ "hunter2_key": "hunter2-token" }"#,
                "`hunter2_key` (holding a string)",
            ),
        ] {
            let reason = Fingerprint::from_json(text).unwrap_err().to_string();
            assert!(reason.contains(wanted), "{text}: {reason}");
            assert!(!reason.contains("hunter2-token"), "{text}: {reason}");
        }
    }

    #[test]

    fn the_flat_shape_of_the_old_versions_is_refused_by_name() {
        let error = Fingerprint::from_json(r#"{ "screen_width": 1920 }"#).unwrap_err();

        assert!(error.to_string().contains("screen_width"), "{error}");
    }

    #[test]
    fn an_off_where_it_means_nothing_is_refused() {
        let error = Fingerprint::from_json(r#"{ "timezone": "off" }"#).unwrap_err();

        assert!(error.to_string().contains("timezone"), "{error}");
    }

    #[test]
    fn a_file_with_a_random_setting_draws_it_and_a_seed_makes_that_repeatable() {
        let text = r#"{ "timezone": "random", "locale": "random", "seed": 7 }"#;

        let first = Fingerprint::from_json(text).unwrap();
        let second = Fingerprint::from_json(text).unwrap();

        assert!(first.timezone().is_value());
        assert!(first.locale().is_value());
        assert_eq!(first, second);
    }

    #[test]
    fn a_file_above_the_cap_is_refused_before_it_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.json");
        let file = fs::File::create(&path).unwrap();
        file.set_len(Fingerprint::MAX_FILE_BYTES + 1).unwrap();

        let error = Fingerprint::load(&path).unwrap_err();

        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{error:?}"
        );
        assert!(error.to_string().contains("more than"), "{error}");
    }

    #[test]
    fn a_hand_written_file_with_a_few_settings_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small.json");
        fs::write(
            &path,
            r#"{ "timezone": "Asia/Tokyo", "screen": { "width": 1280, "height": 720 }, "geolocation": "off" }"#,
        )
        .unwrap();

        let fp = Fingerprint::load(&path).unwrap();

        assert_eq!(fp.timezone().as_str(), Some("Asia/Tokyo"));
        assert_eq!(fp.screen().value().map(|s| s.width), Some(1280));
        assert_eq!(*fp.geolocation(), Spoof::Off);
    }
}
