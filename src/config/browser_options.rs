//! What the settings file, the `SB_*` variables and the `sbrs` flags share beyond the
//! option table of [`test_options`](crate::config::test_options): the names a settings file
//! may use, the list type of `chromium_arg`, and the listing `sbrs options` prints.
//!
//! The option table has one row for each option, and each row names the [`Settings`]
//! field, the flag (with its aliases), the `SB_*` variable and the help. This module adds
//! nothing to the table. It reads the table to answer three more questions:
//!
//! * which top-level keys of a settings file are settings ([`settings_key`]): a key may be
//!   written with dashes or underscores and under any alias of its row (`dark`, `dark-mode`
//!   and `dark_mode` are one setting), and a key that is none of these is an error that
//!   names `sbrs options`, so a typo is not silently ignored;
//! * how `chromium_arg` is written ([`ArgList`]): a comma-separated string, as Python's
//!   `--chromium-arg` takes it, or an array, whose items are not split at commas;
//! * what the options are, as text or a Markdown table ([`render_options`],
//!   [`render_options_markdown`]).
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::config::browser_options::settings_key;
//!
//! assert_eq!(settings_key("dark"), Some("dark_mode"));
//! assert_eq!(settings_key("block-images"), Some("block_images"));
//! assert_eq!(settings_key("blok_images"), None);
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::settings::Settings;
use crate::config::test_options::{find_option, OptionKind, OptionSpec, TEST_OPTIONS};
use crate::error::SeleniumBaseError;

/// The top-level keys of a settings file that hold a table of their own.
const TABLES: [&str; 5] = ["plugins", "fingerprint", "test", "report", "run"];

/// A list of Chromium switches, written in a settings file either as a comma-separated
/// string or as an array.
///
/// A string (and the variable `SB_CHROMIUM_ARG`, and the flag `--chromium-arg`) is split at
/// the commas, as Python SeleniumBase splits it. The items of an array are kept as they are,
/// so a switch whose value holds a comma needs the array form.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::config::browser_options::ArgList;
///
/// let text = ArgList::from_text("--mute-audio, lang=fr");
/// assert_eq!(text.items(), ["--mute-audio", "lang=fr"]);
///
/// let array: ArgList = serde_json::from_str(r#"["--host-resolver-rules=MAP a b,EXCLUDE c"]"#)?;
/// assert_eq!(array.items(), ["--host-resolver-rules=MAP a b,EXCLUDE c"]);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ArgList(Vec<String>);

impl ArgList {
    /// The items of `text`, split at the commas, trimmed, without the empty ones.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        Self(
            text.split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect(),
        )
    }

    /// The items, in the order they were written.
    #[must_use]
    pub fn items(&self) -> &[String] {
        &self.0
    }

    /// Whether the list has no item.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for ArgList {
    fn from(text: &str) -> Self {
        Self::from_text(text)
    }
}

impl From<Vec<String>> for ArgList {
    fn from(items: Vec<String>) -> Self {
        Self(items)
    }
}

impl<'de> Deserialize<'de> for ArgList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Written {
            Text(String),
            Items(Vec<String>),
        }
        Ok(match Written::deserialize(deserializer)? {
            Written::Text(text) => Self::from_text(&text),
            Written::Items(items) => Self(
                items
                    .into_iter()
                    .map(|item| item.trim().to_owned())
                    .filter(|item| !item.is_empty())
                    .collect(),
            ),
        })
    }
}

/// The key of the settings file that `key` stands for, if it is one: a table of the file
/// (`plugins`, `fingerprint`, `test`, `report`) or the field of a row of the option table.
///
/// Dashes and underscores are the same, case does not matter, and a row's flags are accepted
/// (`timeout` is `timeout_seconds`, `dark` is `dark_mode`); the names of Python's `SB(...)` are not.
/// The shorthands that stand for a value (`chrome`, `headed`, `uc`) are not keys.
#[must_use]
pub fn settings_key(key: &str) -> Option<&'static str> {
    let wanted = key.trim().replace('-', "_").to_ascii_lowercase();
    if let Some(table) = TABLES.iter().find(|table| **table == wanted) {
        return Some(table);
    }
    if let Some(name) = TEST_OPTIONS
        .iter()
        .filter_map(OptionSpec::flat_key)
        .find(|name| *name == wanted)
    {
        return Some(name);
    }
    find_option(key)
        .filter(|row| {
            matches!(
                row.kind,
                OptionKind::Switch | OptionKind::Value(_) | OptionKind::Env(_)
            )
        })
        .and_then(OptionSpec::flat_key)
}

/// The path in `Settings` of the setting that the settings-file key `key` (a name that
/// [`settings_key`] returned) stands for: `run.demo_mode` for `demo`, the key itself when the
/// setting sits at the top level.
fn row_of(key: &'static str) -> Option<&'static OptionSpec> {
    TEST_OPTIONS.iter().find(|row| row.flat_key() == Some(key))
}

/// Whether the settings file gives the row's setting as a flat key that is read as the text of
/// its variable (the browser and the session) and not by serde (the run group).
fn is_read_as_text(row: &OptionSpec) -> bool {
    !row.field.starts_with("run.")
}

/// The four recorder switches, which a `[test]` table still holds.
const RECORDER_VARIABLES: [&str; 4] = [
    "SB_RECORDER",
    "SB_REC_BEHAVE",
    "SB_REC_PRINT",
    "SB_REC_SB_MGR",
];

/// Whether the row is an option of the test run: `sbrs options` lists it with `--all` only.
fn is_test_run_row(row: &OptionSpec) -> bool {
    row.field.starts_with("test.")
        || row.field.starts_with("report.")
        || row.field == "run.record_sleep"
        || RECORDER_VARIABLES.contains(&row.env)
}

/// The recorder switch that the key `key` of the `[test]` table stands for, if any.
fn recorder_row(key: &str) -> Option<&'static OptionSpec> {
    let wanted = key.trim().replace('-', "_").to_ascii_lowercase();
    TEST_OPTIONS.iter().find(|row| {
        RECORDER_VARIABLES.contains(&row.env)
            && row.flat_key().is_some()
            && (row.flat_key() == Some(wanted.as_str()) || row.matches_flag(key))
    })
}

/// The message for a key of the settings file `file` that is no setting.
fn unknown_setting(key: &str, file: &Path) -> SeleniumBaseError {
    let name = file.file_name().map_or_else(
        || file.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    SeleniumBaseError::InvalidConfig(format!(
        "unknown setting \"{key}\" in {name}; `sbrs options` lists the browser options"
    ))
}

/// The message for a key of the table `table` (`test` or `report`) of the settings file `file`
/// that is no setting of it.
fn unknown_in_table(key: &str, table: &str, file: &Path) -> SeleniumBaseError {
    let name = file.file_name().map_or_else(
        || file.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    SeleniumBaseError::InvalidConfig(format!(
        "unknown setting \"{key}\" in the [{table}] table of {name}; \
         `sbrs options --all` lists the options of the test run"
    ))
}

/// The message for two spellings of one setting in one settings file.
fn same_setting(first: &str, second: &str, setting: &str) -> SeleniumBaseError {
    SeleniumBaseError::InvalidConfig(format!(
        "\"{first}\" and \"{second}\" are the same setting ({setting}) in the settings file"
    ))
}

/// The group and the key that `key` stands for inside the table `table` (`test` or `report`) of
/// a settings file, if it is a setting of it.
///
/// A key is the last part of the field of a row (`dash_title`) or a flag of the row
/// (`dash-title`, `ftrace`), as the top-level keys are. `[test]` also reads the keys it held
/// before the report moved to `[report]` (`html`, `dashboard`, `databases`, ...), and they
/// sort into the group they belong to now.
fn table_key(table: &str, key: &str) -> Option<(&'static str, &'static str)> {
    let wanted = key.trim().replace('-', "_").to_ascii_lowercase();
    // The databases by environment are a table of their own, with no row.
    if wanted == "databases" {
        return Some(("report", "databases"));
    }
    if table == "report" && wanted == "db" {
        return Some(("report", "db"));
    }
    TEST_OPTIONS
        .iter()
        .filter(|row| {
            row.has_field()
                && matches!(
                    row.kind,
                    OptionKind::Switch
                        | OptionKind::Value(_)
                        | OptionKind::Env(_)
                        | OptionKind::Custom(_)
                )
        })
        .find_map(|row| {
            let (group, rest) = row.field.split_once('.')?;
            let name = rest.strip_prefix("data.").unwrap_or(rest);
            let belongs = group == table
                || (table == "test" && (group == "report" || row.field == "run.record_sleep"));
            (belongs && (name == wanted || row.matches_flag(key))).then_some((group, name))
        })
}

/// The entries of a settings file with each key renamed to the field it stands for.
fn canonical_entries(
    entries: Map<String, Value>,
    file: &Path,
) -> Result<FileEntries, SeleniumBaseError> {
    let mut seen: Vec<(String, String)> = Vec::new();
    let mut top = Map::new();
    let mut text: Vec<TextEntry> = Vec::new();
    let mut groups: BTreeMap<&'static str, Map<String, Value>> = BTreeMap::new();
    for (key, value) in entries {
        let name = settings_key(&key).ok_or_else(|| unknown_setting(&key, file))?;
        if name == "settings_file" {
            // The setting names the file, so a file cannot hold it: it would do nothing.
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "\"{key}\" names the settings file, so a settings file cannot set it; \
                 use `--settings-file` or `SB_SETTINGS_FILE`"
            )));
        }
        // The setting as the messages name it: a table, the path of a setting of the run
        // group, or the key of a setting the file writes flat.
        let row = row_of(name);
        let path = match row {
            Some(row) if row.field.starts_with("run.") || row.field.starts_with("test.") => {
                row.field
            }
            _ => name,
        };
        if let Some((_, first)) = seen.iter().find(|(known, _)| known == path) {
            return Err(same_setting(first, &key, path));
        }
        seen.push((path.to_owned(), key.clone()));
        match row {
            // A setting read as the text of its variable: `dark_mode`, `window_size`.
            Some(row) if is_read_as_text(row) => text.push((row, key, value)),
            // A setting of a group, written flat: `demo` is `run.demo_mode`.
            Some(row) => {
                if let Some((group, field)) = row.field.split_once('.') {
                    groups
                        .entry(group)
                        .or_default()
                        .insert(field.to_owned(), value);
                }
            }
            None => {
                top.insert(name.to_owned(), value);
            }
        }
    }

    // The keys of `[test]`, `[report]` and `[run]` are checked as the top-level ones are, and
    // each goes to the group that holds its setting now: `[test] html` is `[report] html`.
    for table in ["test", "report", "run"] {
        match top.remove(table) {
            Some(Value::Object(entries)) => {
                for (key, value) in entries {
                    let written_as = format!("[{table}] {key}");
                    // The recorder switches were `[test]` settings; they are switches of the
                    // browser now.
                    if table == "test"
                        && let Some(row) = recorder_row(&key)
                    {
                        let name = row.flat_key().unwrap_or(row.env);
                        if let Some((_, first)) = seen.iter().find(|(known, _)| known == name) {
                            return Err(same_setting(first, &written_as, name));
                        }
                        seen.push((name.to_owned(), written_as.clone()));
                        text.push((row, written_as, value));
                        continue;
                    }
                    let (group, canonical) = table_key(table, &key)
                        .ok_or_else(|| unknown_in_table(&key, table, file))?;
                    let path = format!("{group}.{canonical}");
                    if let Some((_, first)) = seen.iter().find(|(known, _)| *known == path) {
                        return Err(same_setting(first, &written_as, &path));
                    }
                    seen.push((path, written_as));
                    groups
                        .entry(group)
                        .or_default()
                        .insert(canonical.to_owned(), value);
                }
            }
            // Not a table: the reader of the value says so.
            Some(other) => {
                top.insert(table.to_owned(), other);
            }
            None => {}
        }
    }
    for (group, fields) in groups {
        if !fields.is_empty() {
            top.insert(group.to_owned(), Value::Object(fields));
        }
    }
    Ok(FileEntries { groups: top, text })
}

/// A setting a file gives as a flat key that is read as the text of its variable: the row,
/// the key as the file wrote it, and the value.
type TextEntry = (&'static OptionSpec, String, Value);

/// What a settings file holds, sorted by how each part is read.
struct FileEntries {
    /// The tables and the settings of a group, which serde reads as they are written.
    groups: Map<String, Value>,
    /// The settings read through the readers of the option table, as the variables are.
    text: Vec<TextEntry>,
}

/// Reads the flat settings of a file through the readers of the option table, as the
/// variables are read, so that a value is refused in one place. The error names the key as
/// the file wrote it and does not repeat the value, which may be a secret.
fn read_text_settings(
    settings: &mut Settings,
    entries: Vec<TextEntry>,
) -> Result<(), SeleniumBaseError> {
    let refused =
        |key: &str, message: &str| SeleniumBaseError::InvalidConfig(format!("{key}: {message}"));
    let mut texts: BTreeMap<&'static str, String> = BTreeMap::new();
    let mut keys: BTreeMap<&'static str, String> = BTreeMap::new();
    for (row, key, value) in entries {
        // A null is an unset setting, as it was for the optional fields.
        if value.is_null() {
            continue;
        }

        // The array form of `chromium_arg` keeps an item that holds a comma whole.
        if row.env == "SB_CHROMIUM_ARG" && value.is_array() {
            let items: ArgList =
                serde_json::from_value(value).map_err(|error| refused(&key, &error.to_string()))?;
            crate::config::text::add_chromium_items(&mut settings.browser, items.items());
            continue;
        }
        let text = crate::config::text::text_of(&value)
            .ok_or_else(|| refused(&key, "expected a text, a number or a boolean"))?;
        texts.insert(row.env, text);
        keys.insert(row.env, key);
    }
    let mut problems = Vec::new();
    crate::config::test_options::apply_env(
        settings,
        &|name| texts.get(name).cloned(),
        &mut problems,
    );
    match problems.into_iter().next() {
        None => Ok(()),
        Some(problem) => {
            let value = texts
                .get(problem.source.as_str())
                .cloned()
                .unwrap_or_default();
            let key = keys
                .get(problem.source.as_str())
                .cloned()
                .unwrap_or(problem.source);
            let mut message = problem.error.to_string();
            if !value.is_empty() {
                message = message.replace(value.as_str(), "…");
            }
            Err(refused(&key, &message))
        }
    }
}

/// Reads the settings file `path` whose text is `content` (TOML for a `.toml` name, JSON
/// otherwise) without checking the values.
///
/// A key may be spelled with dashes or under an alias of its option (see
/// [`settings_key`]); a key that is no setting is an error.
pub(crate) fn parse_settings(path: &Path, content: &str) -> Result<Settings, SeleniumBaseError> {
    let (what, parsed) = if path.extension().is_some_and(|ext| ext == "toml") {
        let table = toml::from_str::<toml::Table>(content).map_err(|error| error.to_string());
        let value = table.and_then(|table| serde_json::to_value(table).map_err(|e| e.to_string()));
        ("failed to parse TOML settings", value)
    } else {
        let value = serde_json::from_str::<Value>(content).map_err(|error| error.to_string());
        ("failed to parse settings", value)
    };
    let failed = |error: &dyn std::fmt::Display| {
        SeleniumBaseError::InvalidConfig(format!("{what}: {error}"))
    };
    let Value::Object(entries) = parsed.map_err(|error| failed(&error))? else {
        return Err(failed(&"the file must hold one object"));
    };
    let FileEntries { groups, text } = canonical_entries(entries, path)?;
    let mut settings: Settings =
        serde_json::from_value(Value::Object(groups)).map_err(|error| failed(&error))?;
    read_text_settings(&mut settings, text).map_err(|error| failed(&error))?;
    Ok(settings)
}

/// The rows `sbrs options` lists: the options of the browser and the page, or with `all` the
/// ones of the test run too. A row that is accepted and ignored, or that is no option of
/// the settings, is left out.
pub fn listed_options(all: bool) -> impl Iterator<Item = &'static OptionSpec> {
    TEST_OPTIONS.iter().filter(move |row| match row.kind {
        OptionKind::Cli(_) | OptionKind::NotAFlag(_) | OptionKind::NoOp(_) => false,
        _ => all || !is_test_run_row(row),
    })
}

/// The browsers or setups an option does anything for.
///
/// `all` means every browser; `grid` an option of a Selenium Grid session, `linux` one that
/// needs a virtual display, which only Linux has.
#[must_use]
pub fn applies_to(row: &OptionSpec) -> &'static str {
    match row.long {
        "dark-mode"
        | "guest-mode"
        | "devtools"
        | "disable-features"
        | "disable-gpu"
        | "disable-ws"
        | "host-resolver-rules"
        | "chromium-arg"
        | "swiftshader"
        | "enable-3d-apis"
        | "remote-debug"
        | "headless1"
        | "mobile"
        | "device-metrics"
        | "log-cdp-events"
        | "uc-cdp-events"
        | "use-auto-ext"
        | "disable-csp"
        | "ad-block"
        | "no-sandbox" => "chromium",
        "firefox-arg" | "firefox-pref" => "firefox",
        "server" | "port" | "protocol" => "grid",
        "xvfb" | "xvfb-metrics" => "linux",
        _ => "all",
    }
}

/// The flags of a row as the listing writes them: the canonical flag, the value it takes
/// and the aliases; `-` for an option that has no flag.
pub(crate) fn flag_cell(row: &OptionSpec) -> String {
    if !row.kind.is_flag() {
        return "-".to_owned();
    }
    let mut cell = format!("--{}", row.long);
    if let OptionKind::Value(hint) | OptionKind::Custom(hint) = row.kind {
        let _ = write!(cell, " <{}>", hint.to_ascii_uppercase());
    }
    for alias in row.aliases {
        let _ = write!(cell, ", --{alias}");
    }
    cell
}

/// The settings key of a row as the listing writes it: the field, or for a shorthand the
/// field and the value it sets.
fn key_cell(row: &OptionSpec) -> String {
    match row.kind {
        OptionKind::Sets(value) => format!("{} = {value}", sets_key(row)),
        _ if row.has_field() => row.flat_key().unwrap_or(row.field).to_owned(),
        _ => "-".to_owned(),
    }
}

/// The key of the setting a shorthand sets: the key of the row that reads the same variable
/// (`browser` for `--firefox`).
fn sets_key(row: &OptionSpec) -> &'static str {
    TEST_OPTIONS
        .iter()
        .find_map(|other| (other.env == row.env).then(|| other.flat_key()).flatten())
        .unwrap_or(row.field)
}

fn variable_cell(row: &OptionSpec) -> &'static str {
    if row.env.is_empty() {
        "-"
    } else {
        row.env
    }
}

/// The options as a text table: the flag, the `SB_*` variable, the key of the settings
/// file, what it applies to and the help. `all` adds the options of the test run.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::config::browser_options::render_options;
///
/// let text = render_options(false);
/// assert!(text.contains("--block-images"));
/// assert!(text.contains("SB_BLOCK_IMAGES"));
/// assert!(!text.contains("SB_PDB"));
/// ```
#[must_use]
pub fn render_options(all: bool) -> String {
    let header = ["FLAG", "VARIABLE", "SETTINGS KEY", "APPLIES TO", "HELP"];
    let rows: Vec<[String; 5]> = listed_options(all)
        .map(|row| {
            [
                flag_cell(row),
                variable_cell(row).to_owned(),
                key_cell(row),
                applies_to(row).to_owned(),
                row.description().to_owned(),
            ]
        })
        .collect();
    let mut widths = header.map(str::len);
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.len());
        }
    }
    let mut out = String::new();
    let mut line = |cells: [&str; 5]| {
        let last = cells.len() - 1;
        for (at, (cell, width)) in cells.iter().zip(widths).enumerate() {
            if at == last {
                out.push_str(cell);
            } else {
                let _ = write!(out, "{cell:<width$}  ");
            }
        }
        out.push('\n');
    };
    line(header);
    for row in &rows {
        line([&row[0], &row[1], &row[2], &row[3], &row[4]]);
    }
    out
}

/// The options as a Markdown table, for the documentation. `all` adds the options of the
/// test run.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::config::browser_options::render_options_markdown;
///
/// let text = render_options_markdown(false);
/// assert!(text.starts_with("| Flag | Variable | Settings key | Applies to | What it does |"));
/// assert!(text.contains("`SB_DARK_MODE`"));
/// ```
#[must_use]
pub fn render_options_markdown(all: bool) -> String {
    let mut out = String::from(
        "| Flag | Variable | Settings key | Applies to | What it does |\n\
         |---|---|---|---|---|\n",
    );
    for row in listed_options(all) {
        let code = |text: &str| {
            if text == "-" {
                "-".to_owned()
            } else {
                format!("`{text}`")
            }
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            code(&flag_cell(row)),
            code(variable_cell(row)),
            code(&key_cell(row)),
            applies_to(row),
            row.description().replace('|', "\\|"),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::browser::options::{LaunchSwitch, PageLoadStrategy};

    #[test]
    fn the_recorder_switches_and_the_threads_are_read_flat_or_in_the_test_table() {
        let flat = toml_file("recorder = true\nrec_print = true\nthreads = 4\n").unwrap();
        let table = toml_file("[test]\nrecorder = true\nrec-print = true\nthreads = 4\n").unwrap();
        assert!(flat.browser.has(LaunchSwitch::RecorderMode));
        assert!(flat.browser.has(LaunchSwitch::RecPrint));
        assert!(!flat.browser.has(LaunchSwitch::RecBehave));
        assert_eq!(flat.test.threads, Some(4));
        assert_eq!(flat, table);
        // Written twice, in the table and flat, it is one setting twice.
        let error = message(toml_file("threads = 4\n[test]\nthreads = 2\n").unwrap_err());
        assert!(error.contains("are the same setting"), "{error}");
        let error = message(toml_file("recorder = true\n[test]\nrec = true\n").unwrap_err());
        assert!(error.contains("are the same setting (recorder)"), "{error}");
    }

    #[test]
    fn a_value_the_file_gives_is_read_like_the_variable_and_not_repeated_when_refused() {
        let error = message(toml_file("window_size = \"hunter2\"\n").unwrap_err());
        assert!(error.contains("window_size"), "{error}");
        assert!(!error.contains("hunter2"), "{error}");
        // A number, a boolean and a null are texts too; a table is not.
        let settings = toml_file("port = 4445\nmobile = true\nuser_data_dir = \"\"\n").unwrap();
        assert_eq!(settings.session.grid.port, Some(4445));
        assert!(settings.browser.mobile);
        assert!(toml_file("port = [1]\n").is_err());
        let json = parse_settings(Path::new("settings.json"), r#"{"locale": null}"#).unwrap();
        assert_eq!(json.browser.lang, None);
    }

    fn toml_file(content: &str) -> Result<Settings, SeleniumBaseError> {
        parse_settings(Path::new("sbase_config.toml"), content)
    }

    fn message(error: SeleniumBaseError) -> String {
        match error {
            SeleniumBaseError::InvalidConfig(message) => message,
            other => other.to_string(),
        }
    }

    #[test]
    fn a_key_is_found_by_field_dashes_and_alias() {
        assert_eq!(settings_key("dark_mode"), Some("dark_mode"));
        assert_eq!(settings_key("dark-mode"), Some("dark_mode"));
        assert_eq!(settings_key("dark"), Some("dark_mode"));
        assert_eq!(settings_key("DCSP"), Some("disable_csp"));
        assert_eq!(settings_key("timeout"), Some("timeout_seconds"));
        assert_eq!(settings_key("bl"), Some("chrome_bin"));
        assert_eq!(settings_key("plugins"), Some("plugins"));
        assert_eq!(settings_key("test"), Some("test"));
    }

    #[test]
    fn a_shorthand_and_a_typo_are_not_keys() {
        for key in ["chrome", "headed", "uc", "blok_images", "pdb", ""] {
            assert_eq!(settings_key(key), None, "{key}");
        }
    }

    #[test]
    fn every_row_with_a_flat_key_is_a_key() {
        let mut keys = 0;
        for row in TEST_OPTIONS {
            let Some(key) = row.flat_key() else {
                continue;
            };
            assert_eq!(settings_key(key), Some(key), "{}", row.long);
            for flag in row.flags() {
                assert_eq!(settings_key(flag), Some(key), "--{flag}");
            }
            keys += 1;
        }
        assert!(keys > 80, "only {keys} rows have a flat key");
        // A shorthand has none, and a setting of the test run has its table only.
        assert!(find_option("--chrome")
            .and_then(OptionSpec::flat_key)
            .is_none());
        assert!(find_option("--pdb")
            .and_then(OptionSpec::flat_key)
            .is_none());
    }

    #[test]
    fn a_file_may_spell_its_keys_with_dashes_and_aliases() {
        let settings = toml_file("block-images = true\ndark = true\ntimeout = 30\n").unwrap();
        assert!(settings.browser.has(LaunchSwitch::BlockImages));
        assert!(settings.browser.has(LaunchSwitch::DarkMode));
        assert_eq!(
            settings.run.default_timeout,
            Some(std::time::Duration::from_secs(30))
        );
    }

    #[test]
    fn a_json_file_is_read_the_same_way() {
        let settings = parse_settings(
            Path::new("settings.json"),
            r#"{"block-images": true, "pls": "eager"}"#,
        )
        .unwrap();
        assert!(settings.browser.has(LaunchSwitch::BlockImages));
        assert_eq!(
            settings.session.page_load_strategy,
            Some(PageLoadStrategy::Eager)
        );
    }

    #[test]
    fn an_unknown_key_is_refused_and_names_the_listing() {
        let error = message(toml_file("blok_images = true\n").unwrap_err());
        assert_eq!(
            error,
            "unknown setting \"blok_images\" in sbase_config.toml; \
             `sbrs options` lists the browser options"
        );
        let error = message(
            parse_settings(Path::new("/tmp/x/settings.json"), r#"{"nope": 1}"#).unwrap_err(),
        );
        assert!(
            error.contains("unknown setting \"nope\" in settings.json"),
            "{error}"
        );
    }

    #[test]
    fn a_setting_written_twice_under_two_names_is_refused() {
        let error = message(toml_file("dark = true\ndark_mode = false\n").unwrap_err());
        assert!(
            error.contains("are the same setting (dark_mode)"),
            "{error}"
        );
    }

    #[test]
    fn a_json_file_that_is_not_an_object_is_refused() {
        let error = message(parse_settings(Path::new("settings.json"), "[1]").unwrap_err());
        assert!(error.contains("one object"), "{error}");
    }

    #[test]
    fn the_tables_of_the_file_are_kept() {
        let settings =
            toml_file("[test]\npdb = true\n\n[fingerprint]\nfile = \"f.json\"\n").unwrap();
        assert!(settings.test.pdb);
        assert_eq!(
            settings.fingerprint.file.as_deref(),
            Some(Path::new("f.json"))
        );
    }

    #[test]
    fn chromium_arg_is_a_string_split_at_commas_or_an_array_that_is_not() {
        let text = toml_file("chromium_arg = \"--a=1, --b,--c\"\n").unwrap();
        assert_eq!(text.browser.chromium.args, ["--a=1", "--b", "--c"]);
        let array = toml_file(
            "chromium_arg = [\"--host-resolver-rules=MAP a b,EXCLUDE c\", \"--mute-audio\"]\n",
        )
        .unwrap();
        assert_eq!(
            array.browser.chromium.args,
            ["--host-resolver-rules=MAP a b,EXCLUDE c", "--mute-audio"]
        );
    }

    #[test]
    fn the_variable_of_chromium_arg_is_split_at_commas() {
        let mut settings = Settings::default();
        settings
            .apply_options(&[("SB_CHROMIUM_ARG", "--a, --b".to_owned())])
            .unwrap();
        assert_eq!(settings.browser.chromium.args, ["--a", "--b"]);
    }

    #[test]
    fn the_listing_has_a_line_for_each_browser_option() {
        let text = render_options(false);
        assert_eq!(text.lines().count(), listed_options(false).count() + 1);
        assert!(text.lines().next().unwrap().starts_with("FLAG"));
        assert!(text.contains("--dark-mode, --dark"));
        assert!(text.contains("--metrics") || text.contains("--device-metrics <METRICS>"));
        assert!(text.contains("browser = firefox"));
        for row in listed_options(false) {
            assert!(!row.field.starts_with("test."), "{}", row.long);
        }
        assert!(!text.contains("SB_PDB"));
        assert!(render_options(true).contains("SB_PDB"));
    }

    #[test]
    fn the_markdown_listing_is_a_table_of_the_same_rows() {
        let text = render_options_markdown(false);
        assert_eq!(text.lines().count(), listed_options(false).count() + 2);
        assert!(text.contains("| `--block-images` | `SB_BLOCK_IMAGES` | `block_images` | all |"));
        for line in text.lines() {
            assert!(line.starts_with('|') && line.ends_with('|'), "{line}");
        }
    }
}
