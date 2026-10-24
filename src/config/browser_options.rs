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

use std::fmt::Write as _;
use std::path::Path;

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};

use crate::config::settings::Settings;
use crate::config::test_options::{find_option, OptionKind, OptionSpec, OptionValue, TEST_OPTIONS};
use crate::error::SeleniumBaseError;

/// The top-level keys of a settings file that hold a table of their own.
const TABLES: [&str; 3] = ["plugins", "fingerprint", "test"];

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

impl OptionValue for Option<ArgList> {
    fn assign(&mut self, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
        *self = Some(ArgList::from_text(text));
        Ok(())
    }
}

/// The key of the settings file that `key` stands for, if it is one: a table of the file
/// (`plugins`, `fingerprint`, `test`) or the field of a row of the option table.
///
/// Dashes and underscores are the same, case does not matter, and a row's flags and
/// `SB(...)` names are accepted (`timeout` is `timeout_seconds`, `dark` is `dark_mode`).
/// The shorthands that stand for a value (`chrome`, `headed`, `uc`) are not keys.
#[must_use]
pub fn settings_key(key: &str) -> Option<&'static str> {
    let wanted = key.trim().replace('-', "_").to_ascii_lowercase();
    if let Some(table) = TABLES.iter().find(|table| **table == wanted) {
        return Some(table);
    }
    let is_key = |row: &&OptionSpec| row.has_field() && !row.field.contains('.');
    if let Some(row) = TEST_OPTIONS
        .iter()
        .filter(is_key)
        .find(|row| row.field == wanted)
    {
        return Some(row.field);
    }
    find_option(key)
        .filter(is_key)
        .filter(|row| {
            matches!(
                row.kind,
                OptionKind::Switch | OptionKind::Value(_) | OptionKind::Env(_)
            )
        })
        .map(|row| row.field)
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

/// The entries of a settings file with each key renamed to the field it stands for.
fn canonical_entries<V>(
    entries: impl IntoIterator<Item = (String, V)>,
    file: &Path,
) -> Result<Vec<(String, V)>, SeleniumBaseError> {
    let mut written: Vec<(&'static str, String)> = Vec::new();
    let mut renamed = Vec::new();
    for (key, value) in entries {
        let name = settings_key(&key).ok_or_else(|| unknown_setting(&key, file))?;
        if let Some((_, first)) = written.iter().find(|(known, _)| *known == name) {
            return Err(SeleniumBaseError::InvalidConfig(format!(
                "\"{first}\" and \"{key}\" are the same setting ({name}) in the settings file"
            )));
        }
        written.push((name, key));
        renamed.push((name.to_owned(), value));
    }
    Ok(renamed)
}

/// Reads the settings file `path` whose text is `content` (TOML for a `.toml` name, JSON
/// otherwise) without checking the values.
///
/// A key may be spelled with dashes or under an alias of its option (see
/// [`settings_key`]); a key that is no setting is an error.
pub(crate) fn parse_settings(path: &Path, content: &str) -> Result<Settings, SeleniumBaseError> {
    if path.extension().is_some_and(|ext| ext == "toml") {
        let failed = |e: &dyn std::fmt::Display| format!("failed to parse TOML settings: {e}");
        let table: toml::Table =
            toml::from_str(content).map_err(|e| SeleniumBaseError::InvalidConfig(failed(&e)))?;
        let table: toml::Table = canonical_entries(table, path)?.into_iter().collect();
        Settings::deserialize(toml::Value::Table(table))
            .map_err(|e| SeleniumBaseError::InvalidConfig(failed(&e)))
    } else {
        let failed = |e: &dyn std::fmt::Display| format!("failed to parse settings: {e}");
        let value: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| SeleniumBaseError::InvalidConfig(failed(&e)))?;
        let serde_json::Value::Object(map) = value else {
            return Err(SeleniumBaseError::InvalidConfig(failed(
                &"the file must hold one object",
            )));
        };
        let map: serde_json::Map<String, serde_json::Value> =
            canonical_entries(map, path)?.into_iter().collect();
        serde_json::from_value(serde_json::Value::Object(map))
            .map_err(|e| SeleniumBaseError::InvalidConfig(failed(&e)))
    }
}

/// The rows `sbrs options` lists: the options of the browser and the page, or with `all` the
/// ones of the test run too. A row that is accepted and ignored, or that is no option of
/// the settings, is left out.
pub fn listed_options(all: bool) -> impl Iterator<Item = &'static OptionSpec> {
    TEST_OPTIONS.iter().filter(move |row| match row.kind {
        OptionKind::Cli(_) | OptionKind::NotAFlag(_) | OptionKind::NoOp(_) => false,
        _ => all || !row.field.starts_with("test."),
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
        OptionKind::Sets(value) => format!("{} = {value}", row.field),
        _ if row.has_field() => row.field.to_owned(),
        _ => "-".to_owned(),
    }
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
    fn every_row_with_a_field_of_its_own_is_a_key() {
        for row in TEST_OPTIONS {
            if row.has_field()
                && !row.field.contains('.')
                && matches!(
                    row.kind,
                    OptionKind::Switch | OptionKind::Value(_) | OptionKind::Env(_)
                )
            {
                assert_eq!(settings_key(row.field), Some(row.field), "{}", row.long);
                for flag in row.flags() {
                    assert_eq!(settings_key(flag), Some(row.field), "--{flag}");
                }
            }
        }
    }

    #[test]
    fn a_file_may_spell_its_keys_with_dashes_and_aliases() {
        let settings = toml_file("block-images = true\ndark = true\ntimeout = 30\n").unwrap();
        assert!(settings.block_images);
        assert!(settings.dark_mode);
        assert_eq!(settings.timeout_seconds, Some(30));
    }

    #[test]
    fn a_json_file_is_read_the_same_way() {
        let settings = parse_settings(
            Path::new("settings.json"),
            r#"{"block-images": true, "pls": "eager"}"#,
        )
        .unwrap();
        assert!(settings.block_images);
        assert_eq!(settings.page_load_strategy.as_deref(), Some("eager"));
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
        assert_eq!(text.chromium_arg.unwrap().items(), ["--a=1", "--b", "--c"]);
        let array = toml_file(
            "chromium_arg = [\"--host-resolver-rules=MAP a b,EXCLUDE c\", \"--mute-audio\"]\n",
        )
        .unwrap();
        assert_eq!(
            array.chromium_arg.unwrap().items(),
            ["--host-resolver-rules=MAP a b,EXCLUDE c", "--mute-audio"]
        );
    }

    #[test]
    fn the_variable_of_chromium_arg_is_split_at_commas() {
        let mut settings = Settings::default();
        settings
            .apply_options(&[("SB_CHROMIUM_ARG", "--a, --b".to_owned())])
            .unwrap();
        assert_eq!(settings.chromium_arg.unwrap().items(), ["--a", "--b"]);
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
