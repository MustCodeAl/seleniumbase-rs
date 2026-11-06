//! The settings file as a figment provider.
//!
//! [`SettingsFile`] reads the file (TOML for a `.toml` name, JSON otherwise) into a [`Layer`]:
//! the settings it gives, as a dictionary at the paths of the serialized
//! [`Settings`](crate::Settings). It does what the option table says about the keys of a file
//! (`browser_options`): a key may be spelled with dashes or under an alias, the keys of the
//! `[test]`, `[report]` and `[run]` tables are checked against the table, a key that is no
//! setting and a setting written twice are errors raised before any value is read.
//!
//! Then each part is read the way its group is read:
//!
//! * the flat keys of the browser and the session (`dark_mode = true`, `window_size =
//!   "800,600"`) are read as the text of their variable, by the same readers as the `SB_*`
//!   variables, so that a value is refused in one place;
//! * the tables and the settings of the other groups (`[run]`, `[test]`, `[report]`,
//!   `[plugins]`, `[fingerprint]`) are handed on as written, and figment's extraction reads
//!   them with serde;
//! * the nested tables `[browser]` and `[session]`, with the tables and maps inside them, are the
//!   settings as `Settings` serializes them: they are handed on as written, beside the flat
//!   keys, and a setting that a flat key and a nested table both give is written twice.
//!
//! A value that is refused names the file and the key as the file wrote it
//! (`sbase_config.toml: headless: cannot parse bool: …`), and never repeats the value, which may
//! be a secret.
//!
//! The type is crate-private: no figment type is part of the API of the crate.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use figment::value::{Dict, Map, Value};
use figment::{Metadata, Profile, Provider};

use crate::config::browser_options::{canonical_entries, ArgList, FileEntries};
use crate::config::env_vars::row_of;
use crate::config::layers::{describe, extract, merge_into, Given, Layer, Refused};
use crate::config::settings::{Settings, SettingsProblem};
use crate::config::test_options::{read_variables, OptionSpec, Variables, TEST_OPTIONS};
use crate::config::text::text_of;
use crate::error::SeleniumBaseError;

/// What a settings file says.
pub(crate) struct SettingsFile {
    path: PathBuf,
    /// How a failure to read the file is introduced: "failed to parse TOML settings" or
    /// "failed to parse settings".
    what: &'static str,
    layer: Layer,
    /// The values the file gives that are refused, each with its row of the option table.
    refused: Vec<Refused>,
}

impl SettingsFile {
    /// Reads the settings file `path`.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] when the file cannot be read, is not TOML (JSON for
    /// a name that does not end in `.toml`), holds a key that is no setting, or names a setting
    /// twice. A value that is refused is not an error here: it is kept (see
    /// [`refused`](Self::refused)), so that [`Settings::check`](crate::Settings::check) can list
    /// every one.
    pub(crate) fn read(path: &Path) -> Result<Self, SeleniumBaseError> {
        let content = fs::read_to_string(path).map_err(|e| {
            SeleniumBaseError::InvalidConfig(format!("failed to read settings: {e}"))
        })?;
        Self::parse(path, &content)
    }

    /// The settings file `path` whose text is `content`; see [`read`](Self::read).
    pub(crate) fn parse(path: &Path, content: &str) -> Result<Self, SeleniumBaseError> {
        let toml = path.extension().is_some_and(|ext| ext == "toml");
        let what = if toml {
            "failed to parse TOML settings"
        } else {
            "failed to parse settings"
        };
        let failed = |error: &dyn std::fmt::Display| {
            SeleniumBaseError::InvalidConfig(format!("{what}: {error}"))
        };
        let parsed = if toml {
            toml::from_str::<toml::Table>(content)
                .map_err(|error| error.to_string())
                .and_then(|table| serde_json::to_value(table).map_err(|e| e.to_string()))
        } else {
            serde_json::from_str::<serde_json::Value>(content).map_err(|error| error.to_string())
        };
        let serde_json::Value::Object(entries) = parsed.map_err(|error| failed(&error))? else {
            return Err(failed(&"the file must hold one object"));
        };
        let FileEntries {
            groups,
            text,
            written,
        } = canonical_entries(entries, path)?;

        let name = file_name(path);
        let mut refused = Vec::new();
        let mut refuse = |row: usize, key: &str, message: &str| {
            refused.push(Refused {
                row,
                problem: SettingsProblem {
                    source: path.display().to_string(),
                    error: SeleniumBaseError::InvalidConfig(format!(
                        "{what}: {name}: {key}: {message}"
                    )),
                },
            });
        };

        // The tables and the settings of a group are the file's own values; figment's
        // extraction reads them with serde, as `check` finds out value by value.
        let mut dict = match Value::serialize(serde_json::Value::Object(groups))
            .map_err(|error| failed(&error))?
        {
            Value::Dict(_, dict) => dict,
            _ => Dict::new(),
        };

        // The flat keys of the browser and the session are the text of their variable.
        let mut texts: BTreeMap<&'static str, String> = BTreeMap::new();
        let mut keys: BTreeMap<&'static str, String> = BTreeMap::new();
        let mut chromium_items = None;
        for (row, key, value) in text {
            // A null is an unset setting.
            if value.is_null() {
                continue;
            }
            let at = row_of(row.env);
            // The array form of `chromium_arg` keeps an item that holds a comma whole.
            if row.env == "SB_CHROMIUM_ARG" && value.is_array() {
                match serde_json::from_value::<ArgList>(value) {
                    Ok(items) => chromium_items = Some(items),
                    Err(error) => refuse(at, &key, &error.to_string()),
                }
                continue;
            }
            match text_of(&value) {
                Some(text) => {
                    texts.insert(row.env, text);
                    keys.insert(row.env, key);
                }
                None => refuse(at, &key, "expected a text, a number or a boolean"),
            }
        }
        let Variables {
            dict: read,
            mut inputs,
            problems,
            ..
        } = read_variables(&|variable| texts.get(variable).cloned());
        merge_into(&mut dict, read);
        for problem in problems {
            let variable = problem.source.as_str();
            let key = keys.get(variable).map_or(variable, String::as_str);
            let mut message = inner_message(&problem.error);
            if let Some(rest) = message.strip_prefix(&format!("{variable}: ")) {
                message = rest.to_owned();
            }
            // The value may be a secret: the message does not repeat it.
            if let Some(value) = texts
                .get(variable)
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            {
                message = without_value(&message, value);
            }
            refuse(row_of(variable), key, &message);
        }
        if let Some(items) = chromium_items {
            // The array form keeps an item that holds a comma whole; the switches and the
            // features it names are worked out with the other sources' (`text::replay`).
            inputs.give("SB_CHROMIUM_ARG", Given::Items(items.items().to_vec()));
        }

        let metadata = Metadata::named(name).interpolater(move |_: &Profile, keys: &[&str]| {
            let path = keys.join(".");
            written.get(&path).cloned().unwrap_or(path)
        });
        Ok(Self {
            path: path.to_path_buf(),
            what,
            layer: Layer::new(metadata, dict).with_inputs(inputs),
            refused,
        })
    }

    /// The settings of the file alone, over the defaults.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] for the first value that is refused, naming the file
    /// and the key as the file wrote it.
    pub(crate) fn settings(&self) -> Result<Settings, SeleniumBaseError> {
        if let Some(first) = self.refused.first() {
            return Err(SeleniumBaseError::InvalidConfig(inner_message(
                &first.problem.error,
            )));
        }
        extract(&[&self.layer]).map_err(|error| {
            SeleniumBaseError::InvalidConfig(format!("{}: {}", self.what, self.refusal(&error)))
        })
    }

    /// Every value of the file that is refused, in the order of the option table: the flat
    /// values the readers refuse, then the values of the tables that serde refuses. Each one
    /// that serde refuses is left out of the layer and the rest is read again, so that every
    /// problem is found, and not only the first.
    pub(crate) fn check(&mut self) -> Vec<Refused> {
        let mut problems = std::mem::take(&mut self.refused);
        // A value in each table of the file is the most that can be refused, so the loop ends;
        // the bound is the guard against a key that cannot be left out.
        for _ in 0..TEST_OPTIONS.len() * 2 {
            let Err(error) = extract(&[&self.layer]) else {
                break;
            };
            let path = error.path.join(".");
            problems.push(Refused {
                row: TEST_OPTIONS
                    .iter()
                    .position(|row| row_holds(row, &path))
                    .unwrap_or(usize::MAX),
                problem: SettingsProblem {
                    source: self.path.display().to_string(),
                    error: SeleniumBaseError::InvalidConfig(format!(
                        "{}: {}",
                        self.what,
                        self.refusal(&error)
                    )),
                },
            });
            if !self.layer.forget(&error.path) {
                break;
            }
        }
        problems.sort_by_key(|refused| refused.row);
        problems
    }

    /// What a figment error says of a value of the file that serde refused (the file, the key as
    /// the file wrote it, what is wrong), without the value itself when it is a string: it may be
    /// a secret, and serde's own text quotes it (`invalid type: string "…", expected a boolean`).
    fn refusal(&self, error: &figment::Error) -> String {
        let text = describe(error);
        match self.layer.text_at(&error.path) {
            Some(value) if !value.is_empty() => without_value(&text, value),
            _ => text,
        }
    }

    /// The layer of the settings the file gives.
    pub(crate) fn into_layer(self) -> Layer {
        self.layer
    }
}

impl Provider for SettingsFile {
    fn metadata(&self) -> Metadata {
        self.layer.metadata()
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        match self.refused.first() {
            Some(refused) => Err(figment::Error::from(inner_message(&refused.problem.error))),
            None => self.layer.data(),
        }
    }
}

/// `message` without the secret `value`, which a reader or serde may have quoted in it.
///
/// Only the value is hidden: the value between quotes (`"v"`, `'v'` or `` `v` ``, however the
/// reader quotes it), and the value that ends the message (`cannot parse bool: v`). A value of
/// three characters or more is hidden wherever it stands alone, as a word of the message. A
/// shorter value is never searched for in the words of the message: a secret of one letter is
/// also a letter of "cannot parse", and the message must stay readable.
fn without_value(message: &str, value: &str) -> String {
    if value.is_empty() {
        return message.to_owned();
    }
    let mut hidden = message.to_owned();
    for quote in ['"', '\'', '`'] {
        hidden = hidden.replace(
            &format!("{quote}{value}{quote}"),
            &format!("{quote}…{quote}"),
        );
    }
    if let Some(head) = hidden.strip_suffix(value)
        && head.ends_with(": ")
    {
        hidden = format!("{head}…");
    }
    if value.chars().count() >= 3 {
        hidden = hide_words(&hidden, value);
    }
    hidden
}

/// `text` with each occurrence of `value` that is not part of a longer word replaced by `…`.
fn hide_words(text: &str, value: &str) -> String {
    let inside_word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    let mut hidden = String::with_capacity(text.len());
    let mut copied = 0;
    for (at, found) in text.match_indices(value) {
        let end = at + found.len();
        if inside_word(text[..at].chars().next_back()) || inside_word(text[end..].chars().next()) {
            continue;
        }
        hidden.push_str(&text[copied..at]);
        hidden.push('…');
        copied = end;
    }
    hidden.push_str(&text[copied..]);
    hidden
}

/// Whether the setting at `path` of the serialized settings is the field of `row` or one inside
/// it (`browser.proxy.server` for the row of `browser.proxy`), so that the problems of a nested
/// table are sorted by the row of the option table too.
fn row_holds(row: &OptionSpec, path: &str) -> bool {
    let field = row.field.replacen("test.data.", "test.", 1);
    row.has_field()
        && path
            .strip_prefix(&field)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

/// The name of the settings file as the messages give it: the file name without its folder.
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The text of an `InvalidConfig` error without the words that the error type puts before it.
fn inner_message(error: &SeleniumBaseError) -> String {
    match error {
        SeleniumBaseError::InvalidConfig(text) => text.clone(),
        other => other.to_string(),
    }
}
