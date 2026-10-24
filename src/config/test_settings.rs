//! The settings of a test run that are not about the browser: test data, debug
//! modes, reruns, evidence and reports, and what the recorder writes.
//!
//! [`TestSettings`] is the `[test]` table of the settings file and the
//! `SB_*` variables of the options in [`test_options`](super::test_options).
//! [`TestData`] is the part a test reads: `sb.env()`, `sb.data()`, `sb.var1()`
//! and `sb.variables()` in Python SeleniumBase.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::config::test_settings::{TestData, TestSettings};
//!
//! let settings: TestSettings = toml::from_str(
//!     r#"
//!     env = "qa"
//!     reruns = 2
//!     reruns_delay = 0.5
//!
//!     [databases]
//!     qa = "results-qa.db"
//!     "#,
//! )?;
//! assert_eq!(settings.data.env(), "qa");
//! assert_eq!(settings.reruns, 2);
//!
//! let variables = TestData::parse_variables("{'KEY1': 'VALUE', 'KEY2': 123}")?;
//! assert_eq!(variables["KEY2"], 123);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::SeleniumBaseError;

/// The values Python SeleniumBase accepts for `--env`; `test` is the default.
pub const VALID_ENVS: [&str; 24] = [
    "qa",
    "rc",
    "staging",
    "develop",
    "production",
    "performance",
    "replica",
    "fedramp",
    "offline",
    "online",
    "master",
    "remote",
    "legacy",
    "local",
    "alpha",
    "beta",
    "demo",
    "gdpr",
    "main",
    "test",
    "gov",
    "new",
    "old",
    "uat",
];

/// The data a run hands to its tests (`--env`, `--account`, `--data`,
/// `--var1`, `--var2`, `--var3` and `--variables`).
///
/// A test reads it from the `BaseCase`; the library gives it no meaning.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TestData {
    /// The environment the tests run against (`SB_ENV`, one of
    /// [`VALID_ENVS`], lower case). `None` is `test`; see [`env`](Self::env).
    pub env: Option<String>,
    /// A free account name (`SB_ACCOUNT`).
    pub account: Option<String>,
    /// Free test data (`SB_DATA`).
    pub data: Option<String>,
    /// A free variable (`SB_VAR1`).
    pub var1: Option<String>,
    /// A free variable (`SB_VAR2`).
    pub var2: Option<String>,
    /// A free variable (`SB_VAR3`).
    pub var3: Option<String>,
    /// A dictionary of values (`SB_VARIABLES`): a JSON object, or the
    /// Python dictionary literal Python SeleniumBase accepts. See
    /// [`parse_variables`](Self::parse_variables).
    pub variables: Map<String, Value>,
}

impl TestData {
    /// The environment (`sb.env` and `sb.environment` in Python): `test`
    /// when none was given.
    #[must_use]
    pub fn env(&self) -> &str {
        self.env.as_deref().unwrap_or("test")
    }

    /// Checks `text` as an `--env` value and returns it in lower case.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] naming the 24 choices when
    /// `text` is not one of [`VALID_ENVS`] (any letter case).
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::config::test_settings::TestData;
    ///
    /// assert_eq!(TestData::parse_env("QA")?, "qa");
    /// assert!(TestData::parse_env("nosuch").is_err());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn parse_env(text: &str) -> Result<String, SeleniumBaseError> {
        let env = text.trim().to_ascii_lowercase();
        if VALID_ENVS.contains(&env.as_str()) {
            Ok(env)
        } else {
            Err(SeleniumBaseError::InvalidConfig(format!(
                "invalid value '{}' for --env (SB_ENV): choose from {}",
                text.trim(),
                VALID_ENVS.join(", ")
            )))
        }
    }

    /// Reads the text of `--variables` (`SB_VARIABLES`): a JSON object, or
    /// the Python dictionary literal that Python SeleniumBase reads with
    /// `ast.literal_eval` (single-quoted strings, `True`, `False`, `None` and
    /// trailing commas).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] when the text is not a
    /// dictionary.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::config::test_settings::TestData;
    ///
    /// assert_eq!(TestData::parse_variables(r#"{"k": 1}"#)?["k"], 1);
    /// assert_eq!(TestData::parse_variables("{'k': 1}")?["k"], 1);
    /// assert!(TestData::parse_variables("[1, 2]").is_err());
    /// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
    /// ```
    pub fn parse_variables(text: &str) -> Result<Map<String, Value>, SeleniumBaseError> {
        let refusal = || {
            SeleniumBaseError::InvalidConfig(
                "Expecting a dictionary for \"variables\"!\n\
                 Eg. --variables='{\"KEY1\":\"VALUE\",\"KEY2\":123}'"
                    .to_owned(),
            )
        };
        let text = text.trim();
        let value = serde_json::from_str::<Value>(text)
            .or_else(|_| serde_json::from_str::<Value>(&python_literal_to_json(text)))
            .map_err(|_| refusal())?;
        match value {
            Value::Object(map) => Ok(map),
            _ => Err(refusal()),
        }
    }
}

/// The `[test]` table of the settings file: the settings of a run that are
/// not about the browser.
///
/// Every field has an `SB_*` variable and a flag of `sbrs test`; the table of
/// them is [`TEST_OPTIONS`](super::test_options::TEST_OPTIONS).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TestSettings {
    /// The data tests read (`--env`, `--account`, `--data`, `--var1..3`,
    /// `--variables`); the keys sit directly in the `[test]` table.
    #[serde(flatten)]
    pub data: TestData,
    /// Stop in a post-mortem prompt when a test fails (`SB_PDB`, `--pdb`).
    pub pdb: bool,
    /// Stop in a prompt before a test's body (`SB_TRACE`, `--trace`).
    pub trace: bool,
    /// Stop in a prompt after every test, before the browser quits
    /// (`SB_FINAL_DEBUG`, `--final-debug`, `--ftrace`).
    pub final_debug: bool,
    /// Run a failing test again this many times (`SB_RERUNS`, `--reruns`).
    pub reruns: u32,
    /// Seconds to wait between reruns (`SB_RERUNS_DELAY`, `--reruns-delay`).
    pub reruns_delay: f64,
    /// Fail a test that runs longer than this many seconds
    /// (`SB_TIME_LIMIT`, `--time-limit`).
    pub time_limit: Option<f64>,
    /// Save a screenshot of a passing test too (`SB_SAVE_SCREENSHOT`,
    /// `--screenshot`).
    pub save_screenshot: bool,
    /// Save no screenshot of a failing test (`SB_NO_SCREENSHOT`,
    /// `--no-screenshot`); it wins over `save_screenshot`.
    pub no_screenshot: bool,
    /// Print `=> Fail Page: <url>` after a failing test (`SB_LIST_FAIL_PAGE`).
    pub list_fail_page: bool,
    /// Keep the old `latest_logs/` in `archived_logs/` (`SB_ARCHIVE_LOGS`).
    pub archive_logs: bool,
    /// Keep the old `downloaded_files/` in `archived_files/`
    /// (`SB_ARCHIVE_DOWNLOADS`).
    pub archive_downloads: bool,
    /// Write the dashboard (`SB_DASHBOARD`).
    pub dashboard: bool,
    /// The dashboard's title (`SB_DASH_TITLE`).
    pub dash_title: Option<String>,
    /// Write an HTML report to this file (`SB_HTML`, `--html`).
    pub html: Option<PathBuf>,
    /// Write a JUnit XML report to this file (`SB_JUNIT_XML`, `--junit-xml`).
    pub junit_xml: Option<PathBuf>,
    /// Write Allure results to this folder (`SB_ALLURE_DIR`, `--alluredir`).
    pub allure_dir: Option<PathBuf>,

    /// Record each outcome in the results database (`SB_WITH_DB_REPORTING`).
    pub with_db_reporting: bool,
    /// Which database of `databases` to use (`SB_DATABASE_ENV`).
    pub database_env: Option<String>,
    /// Upload the logs of each test to S3 (`SB_WITH_S3_LOGGING`).
    pub with_s3_logging: bool,
    /// Record the browser actions (`SB_RECORDER`, `--rec`).
    pub recorder: bool,
    /// Also write a Gherkin feature (`SB_REC_BEHAVE`).
    pub rec_behave: bool,
    /// Print the recording when the test ends (`SB_REC_PRINT`).
    pub rec_print: bool,
    /// Write the recording as a script (`SB_REC_SB_MGR`).
    pub rec_sb_mgr: bool,
    /// Keep the recorded sleeps (`SB_REC_SLEEP`).
    pub rec_sleep: bool,
    /// Which test runner `sbrs test` uses: `cargo` or `nextest`
    /// (`SB_TEST_RUNNER`, `--runner`).
    pub runner: Option<String>,
    /// Result databases by environment name (`[test.databases]`).
    pub databases: BTreeMap<String, PathBuf>,
}

/// Python dictionary literal text as JSON text: single-quoted strings become
/// double-quoted, `True`, `False` and `None` become `true`, `false` and
/// `null`, and a comma before a closing bracket is dropped. Anything else is
/// left for the JSON reader to refuse.
fn python_literal_to_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                out.push('"');
                while let Some(d) = chars.next() {
                    match d {
                        '\\' => match chars.next() {
                            Some('\'') => out.push('\''),
                            Some(escaped) => {
                                out.push('\\');
                                out.push(escaped);
                            }
                            None => out.push('\\'),
                        },
                        '"' => out.push_str("\\\""),
                        '\'' => break,
                        other => out.push(other),
                    }
                }
                out.push('"');
            }
            '"' => {
                out.push('"');
                while let Some(d) = chars.next() {
                    out.push(d);
                    if d == '\\' {
                        if let Some(escaped) = chars.next() {
                            out.push(escaped);
                        }
                    } else if d == '"' {
                        break;
                    }
                }
            }
            c if c.is_ascii_alphabetic() => {
                let mut word = String::from(c);
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphanumeric() || next == '_' {
                        word.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push_str(match word.as_str() {
                    "True" => "true",
                    "False" => "false",
                    "None" => "null",
                    other => other,
                });
            }
            ',' => {
                let mut look = chars.clone();
                if !matches!(look.find(|c| !c.is_whitespace()), Some('}' | ']')) {
                    out.push(',');
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_twenty_four_environments_and_test_is_the_default() {
        assert_eq!(VALID_ENVS.len(), 24);
        assert!(VALID_ENVS.contains(&"test"));
        assert_eq!(TestData::default().env(), "test");
        let qa = TestData {
            env: Some("qa".to_owned()),
            ..TestData::default()
        };
        assert_eq!(qa.env(), "qa");
    }

    #[test]
    fn an_environment_is_lower_cased_and_checked() {
        assert_eq!(TestData::parse_env(" Staging ").unwrap(), "staging");
        let error = TestData::parse_env("nosuch").unwrap_err().to_string();
        assert!(
            error.contains("invalid value 'nosuch' for --env (SB_ENV): choose from qa, rc,"),
            "{error}"
        );
        assert!(error.ends_with("new, old, uat"), "{error}");
    }

    #[test]
    fn variables_are_read_from_json_and_from_a_python_literal() {
        for text in [
            r#"{"k": 1}"#,
            "{'k': 1}",
            "{'k': 1,}",
            " { 'k' : 1 } ",
            "{\"k\": 1}",
        ] {
            let variables = TestData::parse_variables(text).unwrap();
            assert_eq!(variables["k"], 1, "{text}");
        }
    }

    #[test]
    fn python_values_become_json_values() {
        let variables = TestData::parse_variables(
            "{'on': True, 'off': False, 'none': None, 'list': [1, 'a',], 'nested': {'x': 1.5}}",
        )
        .unwrap();
        assert_eq!(variables["on"], true);
        assert_eq!(variables["off"], false);
        assert!(variables["none"].is_null());
        assert_eq!(variables["list"], serde_json::json!([1, "a"]));
        assert_eq!(variables["nested"]["x"], 1.5);
    }

    #[test]
    fn quotes_inside_strings_survive() {
        let variables = TestData::parse_variables(
            r#"{'a': "it's", 'b': 'say "hi"', 'c': 'it\'s', 'd': "x,}"}"#,
        )
        .unwrap();
        assert_eq!(variables["a"], "it's");
        assert_eq!(variables["b"], "say \"hi\"");
        assert_eq!(variables["c"], "it's");
        assert_eq!(variables["d"], "x,}");
    }

    #[test]
    fn anything_but_a_dictionary_is_refused_with_the_python_hint() {
        for text in ["", "[1, 2]", "42", "abc", "{'k': }", "{'k': 1", "'text'"] {
            let error = TestData::parse_variables(text).unwrap_err().to_string();
            assert!(
                error.contains("Expecting a dictionary for \"variables\"!"),
                "{text}: {error}"
            );
            assert!(error.contains("Eg. --variables='{\"KEY1\":\"VALUE\",\"KEY2\":123}'"));
        }
    }

    #[test]
    fn the_defaults_change_nothing() {
        let settings = TestSettings::default();
        assert!(!settings.pdb && !settings.trace && !settings.final_debug);
        assert_eq!(settings.reruns, 0);
        assert!(settings.time_limit.is_none() && settings.runner.is_none());
        assert!(settings.databases.is_empty());
        assert_eq!(settings.data, TestData::default());
    }

    #[test]
    fn the_test_table_of_a_settings_file_is_read() {
        let settings: TestSettings = toml::from_str(
            r#"
            env = "qa"
            var1 = "x"
            pdb = true
            reruns = 3
            reruns_delay = 0.25
            time_limit = 30
            runner = "nextest"

            [variables]
            k = 1

            [databases]
            qa = "results-qa.db"
            "#,
        )
        .unwrap();
        assert_eq!(settings.data.env(), "qa");
        assert_eq!(settings.data.var1.as_deref(), Some("x"));
        assert_eq!(settings.data.variables["k"], 1);
        assert!(settings.pdb);
        assert_eq!((settings.reruns, settings.reruns_delay), (3, 0.25));
        assert_eq!(settings.time_limit, Some(30.0));
        assert_eq!(settings.runner.as_deref(), Some("nextest"));
        assert_eq!(settings.databases["qa"], PathBuf::from("results-qa.db"));
    }
}
