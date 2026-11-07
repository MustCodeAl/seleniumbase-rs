//! The environment variables the command lines read, in one list.
//!
//! Most of them are rows of the option table
//! ([`test_options`](crate::config::test_options)): the table names each option's flag and
//! variable, [`Settings::load`] reads the variable, and `sbrs` makes the flag from the same
//! row. This module adds the few variables that are not rows ([`EXTRA_VARIABLES`]: the
//! plugin names, the proxy list, the log level, the `sbrs-mcp` server, ...), the helpers a
//! command line uses to fill a flag it was not given from its variable ([`text`], [`parse`],
//! [`list`]), and the listing that `sbrs options --env` prints ([`variables`],
//! [`render_variables`], [`render_variables_markdown`]), so the list is made from the
//! tables and not written again by hand.
//!
//! # Precedence
//!
//! A flag wins over its variable, which wins over the settings file, which wins over the
//! default. A variable is read trimmed, a blank one counts as unset, and one that cannot be used
//! is an error that names the variable and the flag it stands for.
//! `SB_PLUGINS` keeps an empty or blank value as an answer (no plugin), and the variables of one
//! tool that name a key, a file or a folder (`SB_ENCRYPTION_KEY`, `SB_REC_FILE`, `SB_OCR_MODELS`,
//! `SB_GRID_DIR`) are not trimmed.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::config::env_vars::{render_variables, variables};
//!
//! let all = variables();
//! assert!(all.iter().any(|variable| variable.name == "SB_DARK_MODE"));
//! assert!(all.iter().any(|variable| variable.name == "SB_MCP_LISTEN"));
//! assert!(render_variables().contains("SB_PROXY_LIST"));
//! ```

use std::fmt::{Display, Write as _};
use std::str::FromStr;

use crate::config::browser_options::flag_cell;
use crate::config::layers::{no_items, put, Layer, Refused};
use crate::config::plugins::{PluginSelection, PLUGINS_ENV};
use crate::config::settings::{named, Settings, SettingsProblem};
use crate::config::test_options::{read_variables, Variables};
use crate::config::test_options::{OptionKind, OptionSpec, TEST_OPTIONS};
use crate::error::SeleniumBaseError;
use figment::value::{Dict, Map, Value};
use figment::{Metadata, Profile, Provider};

/// A variable that is not a row of the option table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvVar {
    /// The variable.
    pub name: &'static str,
    /// The programs that read it.
    pub read_by: &'static str,
    /// The flags that carry the same value, each as the program, the command path and the
    /// flag: `sbrs --proxy-list`, `sbrs grid-hub --jar`, `sbrs-mcp --listen`. None for a
    /// variable with no flag.
    pub flags: &'static [&'static str],
    /// What it is when the variable is not set.
    pub default: &'static str,
    /// What it sets.
    pub help: &'static str,
    /// A value that the program refuses with status 2, naming the variable, or `None` when
    /// any text is taken (a path, a name, a free list) or a wrong value only warns.
    pub refused: Option<&'static str>,
}

/// The flags of a variable that is a row of the option table but has one that the table does
/// not spell: the root's own `--webdriver`, `--cdp`, and the flags of commands that are not
/// `sbrs test`. Each is the program, the command path and the flag, then the variable.
pub const TABLE_FLAGS: &[(&str, &str)] = &[
    ("sbrs --webdriver", "SB_WEBDRIVER_URL"),
    ("sbrs --cdp", "SB_MODE"),
    ("sbrs --config", "SB_SETTINGS_FILE"),
    ("sbrs test --threads", "SB_THREADS"),
    ("sbrs prompt --browser-path", "SB_CHROME_BIN"),
    ("sbrs report finish --dashboard", "SB_DASHBOARD"),
    ("sbrs report finish --dash-title", "SB_DASH_TITLE"),
    ("sbrs report finish --html", "SB_HTML"),
    ("sbrs report finish --junit-xml", "SB_JUNIT_XML"),
    ("sbrs report finish --archive-logs", "SB_ARCHIVE_LOGS"),
];

/// The variables that are not rows of the option table.
pub const EXTRA_VARIABLES: &[EnvVar] = &[
    EnvVar {
        name: "SB_PLUGINS",
        read_by: "sbrs, sbrs-mcp, tests",
        flags: &["sbrs --plugin", "sbrs-mcp --plugin"],
        default: "-",
        help: "Built-in plugins to turn on, separated by commas; replaces the names the settings file enables",
        refused: None,
    },
    EnvVar {
        name: "SB_PROXY_LIST",
        read_by: "sbrs",
        flags: &["sbrs --proxy-list"],
        default: "-",
        help: "A file of proxies to choose from; without a proxy a random one is used",
        refused: None,
    },
    EnvVar {
        name: "SB_LOG_LEVEL",
        read_by: "sbrs, sbrs-mcp, tests",
        flags: &[],
        default: "RUST_LOG, else info",
        help: "The tracing filter, such as debug or seleniumbase_rs=debug,info",
        refused: None,
    },
    EnvVar {
        name: "SB_LOG_FORMAT",
        read_by: "sbrs, sbrs-mcp, tests",
        flags: &[],
        default: "pretty",
        help: "The log format: pretty or json (a wrong value is reported and the default used)",
        refused: None,
    },
    EnvVar {
        name: "SB_SHUTDOWN_TIMEOUT_SECS",
        read_by: "sbrs, sbrs-mcp",
        flags: &[],
        default: "30",
        help: "Seconds to wait for the browser to close when the program is stopped",
        refused: None,
    },
    EnvVar {
        name: "SB_CHROMEDRIVER_PORT",
        read_by: "sbrs, tests",
        flags: &[],
        default: "0",
        help: "The port of the chromedriver that is started; 0 picks a free one",
        refused: None,
    },
    EnvVar {
        name: "SB_IMPLICIT_WAIT_SECS",
        read_by: "sbrs, tests",
        flags: &[],
        default: "30",
        help: "The implicit wait for element queries, in seconds",
        refused: None,
    },
    EnvVar {
        name: "SB_PATCH_CACHE_DIR",
        read_by: "sbrs, tests",
        flags: &["sbrs patch-chrome --cache-dir"],
        default: "the user cache folder",
        help: "The folder that holds patched browser binaries",
        refused: None,
    },
    EnvVar {
        name: "SB_ENCRYPTION_KEY",
        read_by: "sbrs",
        flags: &[],
        default: "asked for",
        help: "The passphrase of `sbrs encrypt` and `sbrs decrypt`; never an argument, so it stays out of the process list",
        refused: None,
    },
    EnvVar {
        name: "SB_SELENIUM_SERVER_JAR",
        read_by: "sbrs",
        flags: &["sbrs grid-hub --jar", "sbrs grid-node --jar"],
        default: "the newest jar in downloaded_drivers/",
        help: "The Selenium Server jar that `grid-hub` and `grid-node` run",
        refused: None,
    },
    EnvVar {
        name: "SB_DRIVER_PATH_DIR",
        read_by: "sbrs",
        flags: &["sbrs install --path"],
        default: "/usr/local/bin",
        help: "The folder `sbrs install --path` also copies the driver into",
        refused: None,
    },
    EnvVar {
        name: "SB_REPORT_ROOT",
        read_by: "sbrs, tests",
        flags: &[],
        default: "the current folder",
        help: "The folder the files of a test run go under, and the run `sbrs report finish` ends",
        refused: None,
    },
    EnvVar {
        name: "SB_REPORT_DB",
        read_by: "sbrs, tests",
        flags: &["sbrs report history --db"],
        default: "reports/results.db",
        help: "The results database",
        refused: None,
    },
    EnvVar {
        name: "SB_RUN_ID",
        read_by: "sbrs, tests",
        flags: &[],
        default: "made for each `sbrs test`",
        help: "The id of the test run the processes belong to; `sbrs test` sets it",
        refused: None,
    },
    EnvVar {
        name: "SB_MCP_SERVER",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp --server"],
        default: "sb",
        help: "Which toolset the MCP server serves: cdp, driver, sb or dev",
        refused: Some("nosuch"),
    },
    EnvVar {
        name: "SB_MCP_TRANSPORT",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp --transport"],
        default: "stdio, or http when a listen address is given",
        help: "How the MCP server talks to its client: stdio or http",
        refused: Some("udp"),
    },
    EnvVar {
        name: "SB_MCP_LISTEN",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp --listen"],
        default: "127.0.0.1:8931 for http",
        help: "The IP address and port of the HTTP endpoint; setting it serves over HTTP",
        refused: Some("not-an-address"),
    },
    EnvVar {
        name: "SB_MCP_PATH",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp --path"],
        default: "/mcp",
        help: "The URL path of the HTTP endpoint",
        refused: None,
    },
    EnvVar {
        name: "SB_MCP_TOKEN",
        read_by: "sbrs-mcp",
        flags: &[],
        default: "-",
        help: "The bearer token the HTTP endpoint asks for; an address that is not loopback needs one",
        refused: None,
    },
    EnvVar {
        name: "SB_MCP_TOKEN_FILE",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp --token-file"],
        default: "-",
        help: "A file whose first line is the bearer token; it wins over SB_MCP_TOKEN",
        refused: None,
    },
    EnvVar {
        name: "SB_MCP_NO_TOKEN",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp --no-token"],
        default: "false",
        help: "Serve HTTP on a loopback address without a bearer token (true or false); without it a loopback listener given no token gets a random one",
        refused: None,
    },
    EnvVar {
        name: "SB_MCP_ALLOW_ORIGIN",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp --allow-origin"],
        default: "-",
        help: "Origins (scheme://host[:port]) of browser pages that may use the HTTP endpoint, separated by commas",
        refused: None,
    },
    EnvVar {
        name: "SB_MCP_ALLOW_HOST",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp --allow-host"],
        default: "-",
        help: "Host names accepted in the Host header besides localhost, separated by commas",
        refused: None,
    },
    EnvVar {
        name: "SB_MCP_MAX_SESSIONS",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp --max-sessions"],
        default: "4",
        help: "The most MCP sessions, and so browsers, the HTTP server holds at once",
        refused: Some("many"),
    },
    EnvVar {
        name: "SB_MCP_OUTPUT_DIR",
        read_by: "sbrs-mcp",
        flags: &["sbrs-mcp install --output-dir"],
        default: "./mcp_output",
        help: "The folder the tools of the MCP server write their files to",
        refused: None,
    },
    EnvVar {
        name: "SB_REC_FILE",
        read_by: "tests",
        flags: &[],
        default: "the name of the test",
        help: "The file stem a recording is written to, when the test names none",
        refused: None,
    },
    EnvVar {
        name: "SB_REC_KEEP_SECRETS",
        read_by: "tests",
        flags: &[],
        default: "false",
        help: "Record typed passwords as typed, not as a placeholder read from the environment when the recording is replayed (true or 1)",
        refused: None,
    },
    EnvVar {
        name: "SB_S3_BUCKET",
        read_by: "tests",
        flags: &[],
        default: "-",
        help: "The S3 bucket that --with-s3-logging uploads the logs of each test to",
        refused: None,
    },
    EnvVar {
        name: "SB_S3_REGION",
        read_by: "tests",
        flags: &[],
        default: "AWS_REGION, else us-east-1",
        help: "The region of the S3 bucket",
        refused: None,
    },
    EnvVar {
        name: "SB_S3_PREFIX",
        read_by: "tests",
        flags: &[],
        default: "-",
        help: "A key prefix in the S3 bucket, such as logs/",
        refused: None,
    },
    EnvVar {
        name: "SB_OCR_MODELS",
        read_by: "tests",
        flags: &[],
        default: "downloaded into the cache folder",
        help: "A folder that holds the OCR models, instead of downloading them (feature ocr)",
        refused: None,
    },
    EnvVar {
        name: "SB_GRID_DIR",
        read_by: "sbrs",
        flags: &[],
        default: "seleniumbase-rs/grid in the state folder of the user",
        help: "The folder the Selenium Grid records and logs go to",
        refused: None,
    },

];

/// One line of the listing: a variable, who reads it, its flag, its default and what it sets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variable {
    /// The variable.
    pub name: &'static str,
    /// The programs that read it.
    pub read_by: &'static str,
    /// The flags that set the same thing, or `-`.
    pub flag: String,
    /// What it is when the variable is not set, or `-` for nothing.
    pub default: String,
    /// What it sets.
    pub help: &'static str,
}

/// Whether the row's variable is read from the environment into a field of [`Settings`].
fn is_variable(row: &OptionSpec) -> bool {
    !row.env.is_empty() && row.kind.reads_env()
}

/// The text a row's variable has when it is not set, from `Settings::default()`.
fn default_of(row: &OptionSpec, settings: &serde_json::Value) -> String {
    match row.env {
        "SB_WEBDRIVER_URL" => return "http://localhost:4444".to_owned(),
        "SB_TIMEOUT" => return "10".to_owned(),
        "SB_CHROME_BIN" => return "the browser found on the machine".to_owned(),
        "SB_BROWSER" => return crate::browser::config::Browser::default().to_string(),
        // The command decides when no mode is asked for (`sbrs devtools` uses the DevTools).
        "SB_MODE" => return "-".to_owned(),
        _ => {}
    }
    let mut at = settings;
    for part in row.field.split('.') {
        match at.get(part) {
            Some(next) => at = next,
            // A setting that is off writes nothing (`skip_serializing_if`), so a switch that the
            // settings do not show is off.
            None if matches!(row.kind, OptionKind::Switch) => return "false".to_owned(),
            None => return "-".to_owned(),
        }
    }
    match at {
        serde_json::Value::Bool(on) => on.to_string(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::String(text) if !text.is_empty() => text.clone(),
        _ => "-".to_owned(),
    }
}

/// The flags of a row's variable: the row's own flag, the flags of the shorthand rows that
/// set the same variable (`--firefox` for `SB_BROWSER`), and the spellings of `TABLE_FLAGS`.
fn flags_of(row: &OptionSpec) -> String {
    let mut flags = Vec::new();
    if row.kind.is_flag() {
        flags.push(flag_cell(row));
    }
    for (spelling, variable) in TABLE_FLAGS {
        if *variable == row.env {
            flags.push(flag_text(spelling));
        }
    }
    for other in TEST_OPTIONS {
        if matches!(other.kind, OptionKind::Sets(_)) && other.env == row.env {
            flags.push(format!("--{}", other.long));
        }
    }
    if flags.is_empty() {
        "-".to_owned()
    } else {
        flags.join(", ")
    }
}

/// Every variable the command lines read, sorted by name: the rows of the option table that
/// have one, then [`EXTRA_VARIABLES`].
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::config::env_vars::variables;
///
/// let all = variables();
/// let browser = all.iter().find(|variable| variable.name == "SB_BROWSER").unwrap();
/// assert_eq!(browser.default, "chrome");
/// assert!(browser.flag.contains("--firefox"));
/// ```
#[must_use]
pub fn variables() -> Vec<Variable> {
    let settings = serde_json::to_value(Settings::default()).unwrap_or_default();
    let mut all: Vec<Variable> = TEST_OPTIONS
        .iter()
        .filter(|row| is_variable(row))
        .map(|row| Variable {
            name: row.env,
            read_by: "sbrs, tests",
            flag: flags_of(row),
            default: default_of(row, &settings),
            help: row.description(),
        })
        .collect();
    all.extend(EXTRA_VARIABLES.iter().map(|extra| Variable {
        name: extra.name,
        read_by: extra.read_by,
        flag: if extra.flags.is_empty() {
            "-".to_owned()
        } else {
            extra
                .flags
                .iter()
                .map(|flag| flag_text(flag))
                .collect::<Vec<_>>()
                .join(", ")
        },
        default: extra.default.to_owned(),
        help: extra.help,
    }));
    all.sort_by_key(|variable| variable.name);
    all
}

/// A registered flag without the program: `sbrs grid-hub --jar` is `grid-hub --jar`, and a
/// flag of the root is the flag alone. `sbrs-mcp` flags keep the program.
fn flag_text(flag: &str) -> String {
    match flag.strip_prefix("sbrs ") {
        Some(rest) => rest.to_owned(),
        None => flag.to_owned(),
    }
}

/// The variable that the flag `flag` of the command `command` stands for, if it has one.
///
/// `command` is the program and the command path (`sbrs`, `sbrs report finish`, `sbrs-mcp`)
/// and `flag` a spelling of the flag with or without its dashes. The root and `sbrs test`
/// take their flags from the option table, so any spelling of a row finds it; the other
/// commands have the flags listed in [`EXTRA_VARIABLES`] and [`TABLE_FLAGS`].
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::config::env_vars::flag_variable;
///
/// assert_eq!(flag_variable("sbrs", "--dark"), Some("SB_DARK_MODE"));
/// assert_eq!(flag_variable("sbrs", "webdriver"), Some("SB_WEBDRIVER_URL"));
/// assert_eq!(flag_variable("sbrs-mcp", "--listen"), Some("SB_MCP_LISTEN"));
/// assert_eq!(flag_variable("sbrs open", "--url"), None);
/// ```
#[must_use]
pub fn flag_variable(command: &str, flag: &str) -> Option<&'static str> {
    let dashed = format!("--{}", flag.trim_start_matches('-'));
    let spelled = format!("{command} {dashed}");
    if let Some((_, variable)) = TABLE_FLAGS.iter().find(|(known, _)| *known == spelled) {
        return Some(variable);
    }
    if let Some(extra) = EXTRA_VARIABLES
        .iter()
        .find(|extra| extra.flags.contains(&spelled.as_str()))
    {
        return Some(extra.name);
    }
    // The root and `sbrs test` build their flags from the table.
    if command == "sbrs" || command == "sbrs test" {
        return crate::config::test_options::find_option(flag)
            .filter(|row| is_variable(row) || matches!(row.kind, OptionKind::Sets(_)))
            .map(|row| row.env);
    }
    None
}

/// Whether `name` is a variable of the listing.
#[must_use]
pub fn is_known(name: &str) -> bool {
    EXTRA_VARIABLES.iter().any(|extra| extra.name == name)
        || TEST_OPTIONS.iter().any(|row| row.env == name)
}

/// The value of the variable `name`, trimmed; `None` when it is not set, empty or blank.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::config::env_vars::text;
///
/// assert_eq!(text("SB_SURELY_NOT_SET_IN_THIS_ENVIRONMENT"), None);
/// ```
#[must_use]
pub fn text(name: &str) -> Option<String> {
    EnvVars::process().text(name)
}

/// The value of the variable `name` read as a `T`; `None` when it is not set.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] naming the variable, the flag it stands for and the
/// value, when the value is not a `T`.
pub fn parse<T>(name: &str) -> Result<Option<T>, SeleniumBaseError>
where
    T: FromStr,
    T::Err: Display,
{
    EnvVars::process().parse(name)
}

/// The value of the switch variable `name`: `Some(true)` for `true`, `1`, `yes` or `on`,
/// `Some(false)` for `false`, `0`, `no` or `off` (in any letter case), and `None` when it is
/// not set, empty or blank.
///
/// A variable that turns something on is read with this, so a value that says no never turns it
/// on by accident: `SB_NO_SANDBOX=0` keeps Chrome's sandbox.
///
/// # Errors
///
/// [`SeleniumBaseError::InvalidConfig`] naming the variable and the value, when the value is
/// none of those.
pub fn switch(name: &str) -> Result<Option<bool>, SeleniumBaseError> {
    EnvVars::process().switch(name)
}

/// The items of the variable `name`, separated by commas and trimmed; empty when it is not
/// set.
#[must_use]
pub fn list(name: &str) -> Vec<String> {
    text(name)
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The error for a value of `name` that cannot be used: it names the variable, the flag the
/// variable stands for and the value.
#[must_use]
pub fn refusal(name: &str, value: &str, why: &str) -> SeleniumBaseError {
    let flag = EXTRA_VARIABLES
        .iter()
        .find(|extra| extra.name == name)
        .and_then(|extra| extra.flags.first())
        .and_then(|flag| flag.rsplit(' ').next());
    let stands_for = flag.map_or_else(String::new, |flag| format!(" (the variable of {flag})"));
    SeleniumBaseError::InvalidConfig(format!("{name}{stands_for}: cannot use '{value}': {why}"))
}

/// The value of the process variable `name`, as the environment has it.
fn process_variable(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Looks a variable up by name: the process environment, or the variables a program hands in.
type Lookup<'a> = Box<dyn Fn(&str) -> Option<String> + 'a>;

/// The `SB_*` variables of one source, and the one rule they are read by: a value that is empty
/// or blank is not set, and any other is trimmed (`SB_PLUGINS` keeps an empty value as an answer).
///
/// The source is the process environment ([`process`](Self::process)), the variables a program
/// hands to [`Settings::check_with`](crate::config::settings::Settings::check_with), or the
/// pairs a command line asked for ([`pairs`](Self::pairs)). `EnvVars` is a figment provider:
/// laid over the settings file, each variable it gives wins over the file's value for the same
/// setting, and the settings it does not give are the file's. Every variable of the option
/// table is read here and nowhere else; so are the few that other code reads
/// ([`text`], [`switch`], [`parse`]).
///
/// The type is crate-private: no figment type is part of the API of the crate.
pub(crate) struct EnvVars<'a> {
    /// What an error calls the source: "environment variable" or "command line".
    name: &'static str,
    lookup: Lookup<'a>,
}

/// What the variables of one source gave, read once.
pub(crate) struct Read {
    /// The settings the variables wrote, by path.
    pub(crate) layer: Layer,
    /// The variables whose value is refused, with the row of each.
    pub(crate) refused: Vec<Refused>,
    /// Whether `SB_PLUGINS` was given, so that the options of the plugins it leaves out go.
    pub(crate) plugins_named: bool,
    /// The settings file `SB_SETTINGS_FILE` names.
    pub(crate) settings_file: Option<std::path::PathBuf>,
}

impl<'a> EnvVars<'a> {
    /// The variables `lookup` reports, called `name` in an error.
    pub(crate) fn new(name: &'static str, lookup: impl Fn(&str) -> Option<String> + 'a) -> Self {
        Self {
            name,
            lookup: Box::new(lookup),
        }
    }

    /// The process environment.
    pub(crate) fn process() -> EnvVars<'static> {
        EnvVars::new("environment variable", process_variable)
    }

    /// The pairs of a command line (`("SB_HEADLESS", "true")`): for a variable given twice, the
    /// last pair wins.
    pub(crate) fn pairs(pairs: &'a [(&str, String)]) -> EnvVars<'a> {
        EnvVars::new("command line", move |name: &str| {
            pairs
                .iter()
                .rev()
                .find(|(variable, _)| *variable == name)
                .map(|(_, value)| value.clone())
        })
    }

    /// The value of the variable `name`, trimmed; `None` when it is not set, empty or blank.
    pub(crate) fn text(&self, name: &str) -> Option<String> {
        (self.lookup)(name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }

    /// The value of the switch variable `name`; see [`switch`].
    pub(crate) fn switch(&self, name: &str) -> Result<Option<bool>, SeleniumBaseError> {
        self.text(name)
            .map(|value| {
                crate::config::settings::parse_bool(&value).map_err(|_| {
                    refusal(
                        name,
                        &value,
                        "expected true, 1, yes, on, false, 0, no or off",
                    )
                })
            })
            .transpose()
    }

    /// The value of the variable `name` read as a `T`; `None` when it is not set; see [`parse`].
    pub(crate) fn parse<T>(&self, name: &str) -> Result<Option<T>, SeleniumBaseError>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.text(name)
            .map(|value| {
                value
                    .parse()
                    .map_err(|why: T::Err| refusal(name, &value, &why.to_string()))
            })
            .transpose()
    }

    /// Reads every variable of the option table, and `SB_PLUGINS`.
    pub(crate) fn read(&self) -> Read {
        let Variables {
            mut dict,
            inputs,
            problems,
            settings_file,
        } = read_variables(&*self.lookup);
        // The variable names the plugins; the options of the file stay for the ones it names.
        let plugins_named = match (self.lookup)(PLUGINS_ENV) {
            Some(list) => {
                let mut selection = PluginSelection::new();
                selection.set_enabled_from_list(&list);
                let names = Value::serialize(&selection)
                    .ok()
                    .and_then(|value| value.find_ref("enable").cloned())
                    .unwrap_or_else(no_items);
                put(&mut dict, "plugins.enable", names);
                true
            }
            None => false,
        };
        // A refusal says which variable it came from, and which flag stands for it.
        let refused = problems
            .into_iter()
            .map(|problem| Refused {
                row: row_of(&problem.source),
                problem: SettingsProblem {
                    source: problem.source.clone(),
                    error: named(problem),
                },
            })
            .collect();
        Read {
            layer: Layer::new(self.metadata(), dict).with_inputs(inputs),
            refused,
            plugins_named,
            settings_file,
        }
    }
}

impl Provider for EnvVars<'_> {
    fn metadata(&self) -> Metadata {
        Metadata::named(self.name)
            .interpolater(|_profile: &Profile, keys: &[&str]| variable_of(&keys.join(".")))
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        let read = self.read();
        match read.refused.first() {
            Some(refused) => Err(figment::Error::from(refused.problem.error.to_string())),
            None => read.layer.data(),
        }
    }
}

/// The index of the row of the option table that reads `variable`; the rows run in this order.
pub(crate) fn row_of(variable: &str) -> usize {
    TEST_OPTIONS
        .iter()
        .position(|row| row.env == variable)
        .unwrap_or(usize::MAX)
}

/// The variable that writes the setting at `path` of the serialized settings, or the path.
fn variable_of(path: &str) -> String {
    TEST_OPTIONS
        .iter()
        .filter(|row| row.has_field() && !row.env.is_empty())
        .find(|row| {
            let field = row.field.replacen("test.data.", "test.", 1);
            field == path
                || path
                    .strip_prefix(field.as_str())
                    .is_some_and(|rest| rest.starts_with('.'))
        })
        .map_or_else(|| path.to_owned(), |row| row.env.to_owned())
}

/// The table: the cells of each line in columns as wide as the longest cell.
fn columns(rows: &[Variable]) -> String {
    let header = ["VARIABLE", "READ BY", "FLAG", "DEFAULT", "WHAT IT SETS"];
    let cells: Vec<[&str; 5]> = rows
        .iter()
        .map(|row| {
            [
                row.name,
                row.read_by,
                row.flag.as_str(),
                row.default.as_str(),
                row.help,
            ]
        })
        .collect();
    let mut widths = header.map(str::len);
    for line in &cells {
        for (width, cell) in widths.iter_mut().zip(line) {
            *width = (*width).max(cell.len());
        }
    }
    let mut out = String::new();
    for line in std::iter::once(header).chain(cells.iter().copied()) {
        for (at, (cell, width)) in line.iter().zip(widths).enumerate() {
            if at + 1 == line.len() {
                out.push_str(cell);
            } else {
                let _ = write!(out, "{cell:<width$}  ");
            }
        }
        out.push('\n');
    }
    out
}

/// The variables as a text table: the variable, who reads it, its flag, its default and what it
/// sets.
#[must_use]
pub fn render_variables() -> String {
    columns(&variables())
}

/// The variables as a Markdown table, for the documentation.
#[must_use]
pub fn render_variables_markdown() -> String {
    let mut out = String::from(
        "| Variable | Read by | Flag | Default | What it sets |\n|---|---|---|---|---|\n",
    );
    let code = |text: &str| {
        if text == "-" {
            "-".to_owned()
        } else {
            format!("`{text}`")
        }
    };
    for row in variables() {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} | {} |",
            row.name,
            row.read_by,
            code(&row.flag),
            row.default.replace('|', "\\|"),
            row.help.replace('|', "\\|"),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The source that gives the one variable `name` the text `value`.
    fn gives<'a>(name: &'a str, value: &'a str) -> EnvVars<'a> {
        EnvVars::new("test", move |asked: &str| {
            (asked == name).then(|| value.to_owned())
        })
    }

    #[test]
    fn only_a_true_value_turns_the_sandbox_off() {
        for text in ["true", "TRUE", "1", "yes", "On"] {
            let source = gives("SB_NO_SANDBOX", text);
            assert_eq!(
                source.switch("SB_NO_SANDBOX").unwrap(),
                Some(true),
                "{text}"
            );
        }
        for text in ["false", "0", "no", "OFF"] {
            let source = gives("SB_NO_SANDBOX", text);
            assert_eq!(
                source.switch("SB_NO_SANDBOX").unwrap(),
                Some(false),
                "{text}"
            );
        }
    }

    #[test]
    fn a_blank_value_is_unset_and_a_value_that_is_no_switch_is_refused() {
        for text in ["", "   "] {
            let source = gives("SB_NO_SANDBOX", text);
            assert_eq!(source.switch("SB_NO_SANDBOX").unwrap(), None, "{text:?}");
            assert_eq!(source.text("SB_NO_SANDBOX"), None, "{text:?}");
        }
        let error = gives("SB_NO_SANDBOX", "maybe")
            .switch("SB_NO_SANDBOX")
            .unwrap_err()
            .to_string();
        assert!(error.contains("SB_NO_SANDBOX"), "{error}");
    }

    #[test]
    fn the_process_environment_and_a_lookup_are_read_by_the_same_rule() {
        let source = gives("SB_CHROME_BIN", "  /opt/chrome  ");
        assert_eq!(source.text("SB_CHROME_BIN").as_deref(), Some("/opt/chrome"));
        let read = source.read();
        assert!(read.refused.is_empty());
        assert!(!read.plugins_named);
        assert!(
            !read.layer.is_empty(),
            "the variable is a setting of the layer"
        );
        // `SB_PLUGINS` is the exception to the blank rule: empty names no plugin.
        assert!(gives("SB_PLUGINS", "").read().plugins_named);
    }

    #[test]
    fn a_figment_made_of_a_source_with_a_refused_variable_fails_naming_it() {
        let source = gives("SB_HEADLESS", "maybe");
        let error = figment::Figment::new()
            .merge(source)
            .extract::<Settings>()
            .unwrap_err();
        assert!(error.to_string().contains("SB_HEADLESS"), "{error}");
    }

    #[test]
    fn a_variable_is_listed_once() {
        let all = variables();
        let names: BTreeSet<_> = all.iter().map(|variable| variable.name).collect();
        assert_eq!(names.len(), all.len(), "a variable is listed twice");
    }

    #[test]
    fn the_extra_variables_are_not_rows_of_the_table() {
        for extra in EXTRA_VARIABLES {
            assert!(extra.name.starts_with("SB_"), "{}", extra.name);
            assert!(
                !TEST_OPTIONS.iter().any(|row| row.env == extra.name),
                "{} is a row of the option table",
                extra.name
            );
            assert!(!extra.help.is_empty(), "{}", extra.name);
            for flag in extra.flags {
                assert!(
                    flag.starts_with("sbrs ") || flag.starts_with("sbrs-mcp "),
                    "{flag} does not start with its program"
                );
                assert!(flag.contains(" --"), "{flag} names no flag");
            }
        }
    }

    #[test]
    fn the_flags_of_the_table_variables_name_rows() {
        for (spelling, variable) in TABLE_FLAGS {
            assert!(
                TEST_OPTIONS.iter().any(|row| row.env == *variable),
                "{spelling}: {variable} is no row"
            );
        }
    }

    #[test]
    fn every_flag_is_found_by_the_command_and_the_flag() {
        for extra in EXTRA_VARIABLES {
            for flag in extra.flags {
                let (command, name) = flag.rsplit_once(' ').expect("a flag");
                assert_eq!(flag_variable(command, name), Some(extra.name), "{flag}");
            }
        }
        assert_eq!(flag_variable("sbrs", "--firefox"), Some("SB_BROWSER"));
        assert_eq!(flag_variable("sbrs", "dark_mode"), Some("SB_DARK_MODE"));
        assert_eq!(flag_variable("sbrs", "--nosuch"), None);
        assert_eq!(
            flag_variable("sbrs report finish", "--html"),
            Some("SB_HTML")
        );
    }

    #[test]
    fn defaults_come_from_the_settings() {
        let all = variables();
        let default = |name: &str| {
            all.iter()
                .find(|variable| variable.name == name)
                .map(|variable| variable.default.as_str())
        };
        assert_eq!(default("SB_BROWSER"), Some("chrome"));
        assert_eq!(default("SB_HEADLESS"), Some("false"));
        assert_eq!(default("SB_WEBDRIVER_URL"), Some("http://localhost:4444"));
        assert_eq!(default("SB_START_PAGE"), Some("-"));
    }

    #[test]
    fn a_blank_variable_is_unset_and_a_list_is_split_at_commas() {
        // Only names no other test sets, so the tests may run side by side.
        assert_eq!(text("SB_ENV_VARS_TEST_UNSET"), None);
        assert!(list("SB_ENV_VARS_TEST_UNSET").is_empty());
        assert_eq!(parse::<u16>("SB_ENV_VARS_TEST_UNSET").unwrap(), None);
    }

    #[test]
    fn a_refusal_names_the_variable_and_the_flag() {
        let message = refusal("SB_MCP_MAX_SESSIONS", "many", "invalid digit").to_string();
        assert!(message.contains("SB_MCP_MAX_SESSIONS"), "{message}");
        assert!(message.contains("--max-sessions"), "{message}");
        assert!(message.contains("'many'"), "{message}");
    }

    #[test]
    fn the_listings_have_a_line_for_every_variable() {
        let text = render_variables();
        let markdown = render_variables_markdown();
        for variable in variables() {
            assert!(text.contains(variable.name), "{}", variable.name);
            assert!(markdown.contains(variable.name), "{}", variable.name);
        }
        assert!(markdown.starts_with("| Variable | Read by | Flag | Default |"));
    }
}
