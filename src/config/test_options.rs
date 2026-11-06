//! The option table: every Python SeleniumBase option (`pytest` options and
//! `SB(...)` keywords) on one line, with the `sbrs test` flags that spell it,
//! the `SB_*` variable that carries it and the [`Settings`] field it lands in.
//!
//! The table is the single source of truth for four consumers:
//!
//! * [`TEST_OPTIONS`], the list `sbrs test` builds its flags from;
//! * the environment reader that [`Settings::load`] runs, which replaces a
//!   hand-written `if let Some(v) = var("SB_..")` chain;
//! * [`Settings::to_browser_config`] and [`Settings::to_launch_options`],
//!   which turn the fields into a browser start-up;
//! * `tests/test_options_table.rs`, which fails when `parity/api.toml` has an
//!   option the table does not spell.
//!
//! A flag written with `_` is the same flag as the one written with `-`
//! (`--no_sandbox` is `--no-sandbox`), and the table lists the `-` form once.
//! The names of `SB(...)` keywords that differ from the flag are the `sb_names`
//! of a row (`is_mobile` for `--mobile`).
//!
//! # Precedence
//!
//! An `sbrs test` flag wins over an `SB_*` variable already in the
//! environment, which wins over the settings file, which wins over the
//! default. A variable that is empty or blank counts as unset.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::config::test_options::{find_option, OptionKind};
//!
//! let dark = find_option("--dark").ok_or("no such option")?;
//! assert_eq!(dark.env, "SB_DARK_MODE");
//! assert_eq!(dark.kind, OptionKind::Switch);
//! assert_eq!(find_option("--dark_mode").map(|row| row.long), Some("dark-mode"));
//! # Ok::<(), &str>(())
//! ```

use crate::browser::options::{LaunchSwitch, WindowSize};
use crate::config::layers::{flag, no_items, nothing, put, Given, Inputs};
use crate::config::text::{self, OptionValue};
use figment::value::{Dict, Value};
use std::path::PathBuf;

use crate::config::settings::{Settings, SettingsProblem};
use crate::config::test_settings::TestData;
use crate::error::SeleniumBaseError;

/// What an option is and how it is read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OptionKind {
    /// A flag that takes no value: `true`/`false` in the variable. Every
    /// switch of `sbrs` accepts `--flag=false` (`bool_value`).
    Switch,
    /// A flag that takes a value of the named kind (`Text`, `Path`, `Seconds`,
    /// ...); the name is the value's placeholder in the help.
    Value(&'static str),
    /// A setting with a variable but no flag of its own (`SB_TIMEOUT`).
    Env(&'static str),
    /// A switch that is shorthand for a fixed value of the row's variable:
    /// `--firefox` is `SB_BROWSER=firefox`. The environment reader skips it.
    Sets(&'static str),
    /// A value with its own check (`--env`, `--variables`).
    Custom(&'static str),
    /// Accepted for the sake of the spelling and ignored, for the reason given.
    NoOp(&'static str),
    /// An action of `sbrs test` itself, with no variable (`--all-scripts`).
    Cli(&'static str),
    /// An option of Python's `SB(...)` that has no flag, for the reason given.
    NotAFlag(&'static str),
}

impl OptionKind {
    /// Whether `sbrs test` has a flag for this kind of option.
    #[must_use]
    pub const fn is_flag(self) -> bool {
        !matches!(self, Self::Env(_) | Self::NotAFlag(_))
    }

    /// Whether the environment reader reads the row's variable.
    #[must_use]
    pub const fn reads_env(self) -> bool {
        matches!(
            self,
            Self::Switch | Self::Value(_) | Self::Env(_) | Self::Custom(_)
        )
    }
}

/// One row of the table. See the [module documentation](self).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptionSpec {
    /// The path of the field in [`Settings`] as it is serialized (`browser.proxy.server`,
    /// `browser.switches.dark-mode`, `run.demo_mode`, `report.html`, `test.pdb`), or `none` when the
    /// row has no field. Several rows may write one field (`browser.window_size`).
    pub field: &'static str,
    /// The canonical flag, without the dashes: `dark-mode`.
    pub long: &'static str,
    /// The other flags that mean the same, without the dashes.
    pub aliases: &'static [&'static str],
    /// The names Python's `SB(...)` knows the option by, when they differ from
    /// the flags.
    pub sb_names: &'static [&'static str],
    /// The environment variable, or an empty string for none.
    pub env: &'static str,
    /// What the option is.
    pub kind: OptionKind,
    /// A one-line help.
    pub help: &'static str,
}

fn normalise(spelling: &str) -> String {
    spelling
        .trim_start_matches('-')
        .replace('_', "-")
        .to_ascii_lowercase()
}

impl OptionSpec {
    /// The help without the variable it ends with.
    ///
    /// The help of a row that has a variable ends with `(SB_X)`, or with `(SB_X=value)` for a
    /// shorthand, so that `--help`, which is made from the same rows, tells where the
    /// variable is. A test checks every row. Listings with a column for the variable use this.
    ///
    /// # Examples
    ///
    /// ```
    /// use seleniumbase_rs::config::test_options::find_option;
    ///
    /// let dark = find_option("--dark").ok_or("no such option")?;
    /// assert_eq!(dark.help, "Force Chrome's dark theme on web pages (SB_DARK_MODE)");
    /// assert_eq!(dark.description(), "Force Chrome's dark theme on web pages");
    /// # Ok::<(), &str>(())
    /// ```
    #[must_use]
    pub fn description(&self) -> &'static str {
        self.help
            .strip_suffix(')')
            .and_then(|rest| rest.rsplit_once(" ("))
            .filter(|(_, variable)| variable.starts_with("SB_"))
            .map_or(self.help, |(text, _)| text)
    }

    /// The flags of the row, the canonical one first, without the dashes.
    pub fn flags(&self) -> impl Iterator<Item = &'static str> + '_ {
        std::iter::once(self.long).chain(self.aliases.iter().copied())
    }

    /// Whether the row has a field in [`Settings`].
    #[must_use]
    pub fn has_field(&self) -> bool {
        self.field != "none"
    }

    /// The key a settings file writes at its top level for the row's setting (`dark_mode`,
    /// `demo`, `timeout_seconds`), or `None` when the setting has only the key of its table
    /// (`[test] pdb`, `[report] html`).
    #[must_use]
    pub fn flat_key(&self) -> Option<&'static str> {
        if !self.has_field()
            || !matches!(
                self.kind,
                OptionKind::Switch | OptionKind::Value(_) | OptionKind::Env(_)
            )
        {
            return None;
        }
        FLAT_KEYS
            .iter()
            .find(|(env, _)| *env == self.env)
            .map(|(_, key)| *key)
    }

    /// Whether `spelling` is a flag of the row (`--dark`, `dark_mode`, ...).
    #[must_use]
    pub fn matches_flag(&self, spelling: &str) -> bool {
        let wanted = normalise(spelling);
        self.flags().any(|flag| normalise(flag) == wanted)
    }

    /// Whether `name` is a keyword of Python's `SB(...)` for this option: the
    /// `sb_names`, or any of the flags.
    #[must_use]
    pub fn matches_sb_name(&self, name: &str) -> bool {
        let wanted = normalise(name);
        self.matches_flag(name) || self.sb_names.iter().any(|sb| normalise(sb) == wanted)
    }
}

/// The row that spells `flag` (`--dark`, `dark_mode`, `-dark`), if any.
#[must_use]
pub fn find_option(flag: &str) -> Option<&'static OptionSpec> {
    TEST_OPTIONS.iter().find(|row| row.matches_flag(flag))
}

/// The keys a settings file writes at its top level (`dark_mode = true`): the variable of the row,
/// then the key. The variable names the row, because several rows may write one field. A row
/// not listed here has no key of its own (`[test] pdb`, `[report] html`).
const FLAT_KEYS: &[(&str, &str)] = &[
    ("SB_BROWSER", "browser"),
    ("SB_HEADLESS", "headless"),
    ("SB_TIMEOUT", "timeout_seconds"),
    ("SB_WEBDRIVER_URL", "webdriver_url"),
    ("SB_CHROME_BIN", "chrome_bin"),
    (
        "SB_REMOTE_WEBDRIVER_TIMEOUT",
        "remote_webdriver_timeout_seconds",
    ),
    ("SB_PROXY", "proxy"),
    ("SB_WINDOW_WIDTH", "window_width"),
    ("SB_WINDOW_HEIGHT", "window_height"),
    ("SB_USER_DATA_DIR", "user_data_dir"),
    ("SB_EXTENSION_DIR", "extension_dir"),
    ("SB_LOCALE", "locale"),
    ("SB_USER_AGENT", "user_agent"),
    ("SB_MODE", "mode"),
    ("SB_AD_BLOCK", "ad_block"),
    ("SB_REUSE_SESSION", "reuse_session"),
    ("SB_REUSE_CLASS_SESSION", "reuse_class_session"),
    ("SB_MOBILE", "mobile"),
    ("SB_THREADS", "threads"),
    ("SB_PROXY_PAC_URL", "proxy_pac_url"),
    ("SB_ESC_END", "esc_end"),
    ("SB_REC_SB_CDP", "rec_sb_cdp"),
    ("SB_START_PAGE", "start_page"),
    ("SB_SETTINGS_FILE", "settings_file"),
    ("SB_HEADLESS1", "headless1"),
    ("SB_LOCALE_CODE", "locale_code"),
    ("SB_SERVER", "server"),
    ("SB_PORT", "port"),
    ("SB_PROTOCOL", "protocol"),
    ("SB_CHROMIUM_ARG", "chromium_arg"),
    ("SB_DARK_MODE", "dark_mode"),
    ("SB_DEVTOOLS", "devtools"),
    ("SB_DISABLE_FEATURES", "disable_features"),
    ("SB_DISABLE_GPU", "disable_gpu"),
    ("SB_DISABLE_WS", "disable_ws"),
    ("SB_GUEST_MODE", "guest_mode"),
    ("SB_INCOGNITO", "incognito"),
    ("SB_HOST_RESOLVER_RULES", "host_resolver_rules"),
    ("SB_NO_SANDBOX", "no_sandbox"),
    ("SB_SWIFTSHADER", "swiftshader"),
    ("SB_ENABLE_3D_APIS", "enable_3d_apis"),
    ("SB_MAXIMIZE", "maximize"),
    ("SB_BLOCK_IMAGES", "block_images"),
    ("SB_DISABLE_JS", "disable_js"),
    ("SB_DISABLE_COOKIES", "disable_cookies"),
    ("SB_DO_NOT_TRACK", "do_not_track"),
    ("SB_EXTERNAL_PDF", "external_pdf"),
    ("SB_ENABLE_SYNC", "enable_sync"),
    ("SB_DISABLE_CSP", "disable_csp"),
    ("SB_USE_AUTO_EXT", "use_auto_ext"),
    ("SB_PROXY_DRIVER", "proxy_driver"),
    ("SB_PAGE_LOAD_STRATEGY", "page_load_strategy"),
    ("SB_WINDOW_SIZE", "window_size"),
    ("SB_WINDOW_POSITION", "window_position"),
    ("SB_DEVICE_METRICS", "device_metrics"),
    ("SB_PROXY_BYPASS_LIST", "proxy_bypass_list"),
    ("SB_DRIVER_VERSION", "driver_version"),
    ("SB_CAP_FILE", "cap_file"),
    ("SB_CAP_STRING", "cap_string"),
    ("SB_EXTENSION_ZIP", "extension_zip"),
    ("SB_FIREFOX_ARG", "firefox_arg"),
    ("SB_FIREFOX_PREF", "firefox_pref"),
    ("SB_XVFB", "xvfb"),
    ("SB_XVFB_METRICS", "xvfb_metrics"),
    ("SB_REMOTE_DEBUG", "remote_debug"),
    ("SB_LOG_CDP_EVENTS", "log_cdp_events"),
    ("SB_UC_CDP_EVENTS", "uc_cdp_events"),
    ("SB_CRUMBS", "crumbs"),
    ("SB_DEMO", "demo"),
    ("SB_SLOW", "slow"),
    ("SB_DEMO_SLEEP", "demo_sleep"),
    ("SB_MESSAGE_DURATION", "message_duration"),
    ("SB_SKIP_JS_WAITS", "skip_js_waits"),
    ("SB_VISUAL_BASELINE", "visual_baseline"),
    ("SB_HIGHLIGHTS", "highlights"),
    ("SB_INTERVAL", "interval"),
    ("SB_VERIFY_DELAY", "verify_delay"),
    ("SB_CHECK_JS", "check_js"),
    ("SB_WAIT_FOR_ANGULARJS", "wait_for_angularjs"),
    ("SB_DISABLE_BEFOREUNLOAD", "disable_beforeunload"),
    ("SB_TIMEOUT_MULTIPLIER", "timeout_multiplier"),
    ("SB_RECORDER", "recorder"),
    ("SB_REC_BEHAVE", "rec_behave"),
    ("SB_REC_PRINT", "rec_print"),
    ("SB_REC_SB_MGR", "rec_sb_mgr"),
];

/// `--env`: one of the 24 environments, in lower case.
fn set_env(settings: &mut Settings, _name: &str, text: &str) -> Result<(), SeleniumBaseError> {
    settings.test.data.env = Some(TestData::parse_env(text)?);
    Ok(())
}

/// `--variables`: a JSON object or a Python dictionary literal.
fn set_variables(
    settings: &mut Settings,
    _name: &str,
    text: &str,
) -> Result<(), SeleniumBaseError> {
    settings.test.data.variables = TestData::parse_variables(text)?;
    Ok(())
}

/// Reads the variable `env` and, when it is set, gives its text to `assign`.
fn read(
    var: &dyn Fn(&str) -> Option<String>,
    problems: &mut Vec<SettingsProblem>,
    env: &str,
    assign: impl FnOnce(&str, &str) -> Result<(), SeleniumBaseError>,
) {
    if let Some(text) = var(env)
        && let Err(error) = assign(env, &text)
    {
        // A value a type refuses (a window size, a driver mode) says what it expected and
        // not where it came from; as an `InvalidConfig` it gets the variable and the flag.
        let error = match error {
            error @ SeleniumBaseError::InvalidConfig(_) => error,
            other => SeleniumBaseError::InvalidConfig(other.to_string()),
        };
        problems.push(SettingsProblem {
            source: env.to_owned(),
            error,
        });
    }
}

macro_rules! kind_of {
    // An on/off setting that is one entry of `browser.switches`: the `LaunchSwitch` and the
    // name it has in the serialized settings.
    (switch_in($v:ident, $k:literal)) => {
        OptionKind::Switch
    };

    (switch) => {
        OptionKind::Switch
    };
    (value($t:ident)) => {
        OptionKind::Value(stringify!($t))
    };
    (env($t:ident)) => {
        OptionKind::Env(stringify!($t))
    };
    (sets($v:literal)) => {
        OptionKind::Sets($v)
    };
    (custom($t:ident, $f:path)) => {
        OptionKind::Custom(stringify!($t))
    };
    (noop($why:literal)) => {
        OptionKind::NoOp($why)
    };
    (cli($why:literal)) => {
        OptionKind::Cli($why)
    };
    (na($why:literal)) => {
        OptionKind::NotAFlag($why)
    };
}

macro_rules! apply_row {
    // The derived settings: several variables write one value, or one writes two, so each has
    // a reader of its own (`config::text`). `SB_WINDOW_HEIGHT` is read with `SB_WINDOW_WIDTH`.
    ($s:ident, $var:ident, $problems:ident, "SB_WINDOW_WIDTH", [$($k:tt)+], $($p:ident).+) => {
        read_window_axes($s, $var, $problems)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_WINDOW_HEIGHT", [$($k:tt)+], $($p:ident).+) => {};
    ($s:ident, $var:ident, $problems:ident, "SB_LOCALE_CODE", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_LOCALE_CODE", browser, text::locale_code)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_XVFB", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_XVFB", browser, text::xvfb)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_XVFB_METRICS", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_XVFB_METRICS", browser, text::xvfb_metrics)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_REMOTE_DEBUG", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_REMOTE_DEBUG", browser, text::remote_debug)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_CHROMIUM_ARG", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_CHROMIUM_ARG", browser, text::chromium_arg)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_DISABLE_FEATURES", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_DISABLE_FEATURES", browser, text::disable_features)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_REUSE_CLASS_SESSION", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_REUSE_CLASS_SESSION", session, text::reuse_class)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_CAP_FILE", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_CAP_FILE", session, text::cap_file)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_CAP_STRING", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_CAP_STRING", session, text::cap_string)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_FIREFOX_ARG", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_FIREFOX_ARG", session, text::firefox_arg)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_FIREFOX_PREF", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_FIREFOX_PREF", session, text::firefox_pref)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_LOG_CDP_EVENTS", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_LOG_CDP_EVENTS", session, text::log_cdp_events)
    };
    ($s:ident, $var:ident, $problems:ident, "SB_UC_CDP_EVENTS", [$($k:tt)+], $($p:ident).+) => {
        derived!($s, $var, $problems, "SB_UC_CDP_EVENTS", session, text::uc_cdp_events)
    };

    ($s:ident, $var:ident, $problems:ident, $env:literal, [switch_in($v:ident, $k:literal)], $($p:ident).+) => {
        read($var, $problems, $env, |_name, text| {
            $s.browser
                .set(LaunchSwitch::$v, crate::config::settings::parse_bool(text)?);
            Ok(())
        })
    };
    ($s:ident, $var:ident, $problems:ident, $env:literal, [switch], $($p:ident).+) => {
        read($var, $problems, $env, |name, text| {
            OptionValue::assign(&mut $s.$($p).+, name, text)
        })
    };
    ($s:ident, $var:ident, $problems:ident, $env:literal, [value($t:ident)], $($p:ident).+) => {
        read($var, $problems, $env, |name, text| {
            OptionValue::assign(&mut $s.$($p).+, name, text)
        })
    };
    ($s:ident, $var:ident, $problems:ident, $env:literal, [env($t:ident)], $($p:ident).+) => {
        read($var, $problems, $env, |name, text| {
            OptionValue::assign(&mut $s.$($p).+, name, text)
        })
    };
    ($s:ident, $var:ident, $problems:ident, $env:literal, [custom($t:ident, $f:path)], $($p:ident).+) => {
        read($var, $problems, $env, |name, text| $f(&mut *$s, name, text))
    };
    ($s:ident, $var:ident, $problems:ident, $env:literal, [sets($v:literal)], $($p:ident).+) => {};
    ($s:ident, $var:ident, $problems:ident, $env:literal, [noop($why:literal)], $($p:ident).+) => {};
    ($s:ident, $var:ident, $problems:ident, $env:literal, [cli($why:literal)], $($p:ident).+) => {};
    ($s:ident, $var:ident, $problems:ident, $env:literal, [na($why:literal)], $($p:ident).+) => {};
}

/// Reads the variable `$env` with the reader `$reader` of one group of the settings.
macro_rules! derived {
    ($s:ident, $var:ident, $problems:ident, $env:literal, $group:ident, $reader:path) => {
        read($var, $problems, $env, |name, text| {
            $reader(&mut $s.$group, name, text)
        })
    };
}

/// The field of a row, as the serialized settings name it: the path the macro is given, and for
/// one entry of `browser.switches` the name of the switch after it.
macro_rules! field_of {
    ([switch_in($v:ident, $k:literal)], $($p:ident).+) => {
        concat!(stringify!($($p).+), ".", $k)
    };
    ([$($k:tt)+], $($p:ident).+) => {
        stringify!($($p).+)
    };
}

macro_rules! option_table {
    ($(
        (
            $($path:ident).+,
            $long:literal,
            [$($alias:literal),* $(,)?],
            [$($sb:literal),* $(,)?],
            $env:tt,
            [$($kind:tt)+],
            $help:literal
        )
    ),+ $(,)?) => {
        /// Every option of the table; see the [module documentation](self).
        pub static TEST_OPTIONS: &[OptionSpec] = &[$(
            OptionSpec {
                field: field_of!([$($kind)+], $($path).+),
                long: $long,
                aliases: &[$($alias),*],
                sb_names: &[$($sb),*],
                env: $env,
                kind: kind_of!($($kind)+),
                help: $help,
            }
        ),+];

        /// Writes every row whose variable `var` reports into `settings`;
        /// a value that does not parse goes to `problems`, in table order.
        #[allow(clippy::too_many_lines)]
        fn apply_rows(
            settings: &mut Settings,
            var: &dyn Fn(&str) -> Option<String>,
            problems: &mut Vec<SettingsProblem>,
        ) {
            $(
                apply_row!(settings, var, problems, $env, [$($kind)+], $($path).+);
            )+
        }
    };
}

/// Reads every option whose variable `var` reports, the variables of one source, into the
/// settings they write (`Variables::dict`); a value that does not parse goes to
/// `Variables::problems`, in table order. The providers `EnvVars` and `SettingsFile` call it.
/// A row writes the field its path names; a derived row has a reader of its own (`config::text`).
///
/// Two settings have two spellings: the window size (`SB_WINDOW_SIZE`, or `SB_WINDOW_WIDTH`
/// and `SB_WINDOW_HEIGHT`) and the language (`SB_LOCALE`, `SB_LOCALE_CODE`). The source that
/// writes one spelling decides: it replaces what a lower source wrote in the other
/// spelling, and within the source the window size and the locale code win, as they always
/// did. A file's `window_size` therefore no longer beats `SB_WINDOW_WIDTH` and
/// `SB_WINDOW_HEIGHT` of the environment.
pub(crate) fn read_variables(var: &dyn Fn(&str) -> Option<String>) -> Variables {
    // The rows run in table order on a scratch `Settings`, so that what one row reads can
    // depend on the rows before it in the same source (a refused value is found here).
    let mut scratch = Settings::default();
    let mut problems = Vec::new();
    apply_rows(&mut scratch, var, &mut problems);
    let refused: Vec<String> = problems
        .iter()
        .map(|problem| problem.source.clone())
        .collect();
    // What the source gives is the paths its rows wrote, with the values the scratch has
    // there. A value that serializes to nothing (a switch that is off, a list that is empty)
    // is written out, so that this source can turn off what a lower one turned on. The
    // variables that write a setting together are not read into a path: the source keeps the
    // text it gave, and the setting is worked out when the sources are merged.
    let mut dict = Dict::new();
    let mut inputs = Inputs::default();
    match Value::serialize(&scratch) {
        Ok(whole) => {
            let size_given = var("SB_WINDOW_SIZE").is_some();
            for row in TEST_OPTIONS {
                let given = row.kind.reads_env() && !row.env.is_empty() && var(row.env).is_some();
                let axis = matches!(row.env, "SB_WINDOW_WIDTH" | "SB_WINDOW_HEIGHT");
                if !given || refused.iter().any(|name| name == row.env) || (axis && size_given) {
                    continue;
                }
                if text::REPLAYED.contains(&row.env) {
                    if let Some(text) = var(row.env) {
                        inputs.give(row.env, Given::Text(text));
                    }
                    continue;
                }
                for (path, absent) in written_by(row) {
                    match whole.find_ref(&path) {
                        Some(value) => put(&mut dict, &path, value.clone()),
                        None => {
                            if let Some(value) = absent.value() {
                                put(&mut dict, &path, value);
                            }
                        }
                    }
                }
            }
        }
        Err(error) => problems.push(SettingsProblem {
            source: "settings".to_owned(),
            error: SeleniumBaseError::InvalidConfig(format!(
                "the settings cannot be layered: {error}"
            )),
        }),
    }
    Variables {
        dict,
        inputs,
        problems,
        settings_file: scratch.settings_file,
    }
}

/// What one source's variables say: the settings they write, as a dictionary at the paths of
/// the serialized [`Settings`], and the values they refuse, in table order.
pub(crate) struct Variables {
    /// The settings written, by path.
    pub(crate) dict: Dict,
    /// The variables of [`text::REPLAYED`] the source gave, as it gave them.
    pub(crate) inputs: Inputs,

    /// The variables whose value is refused; each problem's source is the variable.
    pub(crate) problems: Vec<SettingsProblem>,
    /// The settings file `SB_SETTINGS_FILE` names, which no layer can hold (it names the file
    /// the layers come from).
    pub(crate) settings_file: Option<PathBuf>,
}

/// What a source that gave a variable but wrote nothing at its path means by it.
#[derive(Clone, Copy)]
enum Absent {
    /// Nothing: the setting stays what a lower source says.
    Skip,
    /// The setting is unset (`SB_REMOTE_DEBUG=false`).
    Nothing,
    /// A list with no item (`SB_EXTENSION_DIR` with no folder).
    NoItems,
    /// A switch that is off.
    Off,
}

impl Absent {
    fn value(self) -> Option<Value> {
        match self {
            Self::Skip => None,
            Self::Nothing => Some(nothing()),
            Self::NoItems => Some(no_items()),
            Self::Off => Some(flag(false)),
        }
    }
}

/// The paths of the serialized settings that the row's variable writes, each with what it
/// means when the source gives the variable and the settings have nothing at the path.
///
/// Most rows write the one path their `field` names. The derived rows, where several
/// variables write one value or one writes two, name theirs.
fn written_by(row: &OptionSpec) -> Vec<(String, Absent)> {
    let one = |path: &str, absent| vec![(path.to_owned(), absent)];
    match row.env {
        "SB_WINDOW_WIDTH" => one("browser.window_size.width", Absent::Skip),
        "SB_WINDOW_HEIGHT" => one("browser.window_size.height", Absent::Skip),
        "SB_LOCALE" | "SB_LOCALE_CODE" => one("browser.lang", Absent::Skip),
        "SB_REMOTE_DEBUG" => vec![
            ("browser.debugging_port".to_owned(), Absent::Nothing),
            ("browser.debugging_any_port".to_owned(), Absent::Off),
        ],
        "SB_EXTENSION_DIR" => one("browser.extensions", Absent::NoItems),
        "SB_EXTENSION_ZIP" => one("session.extension_archives", Absent::NoItems),
        "SB_REUSE_CLASS_SESSION" => one("session.reuse_class", Absent::Nothing),
        "SB_CAP_FILE" | "SB_CAP_STRING" => one("session.capabilities", Absent::Skip),
        "SB_FIREFOX_ARG" => one("session.firefox.args", Absent::Skip),
        "SB_FIREFOX_PREF" => one("session.firefox.preferences", Absent::Skip),
        _ => {
            // `TestSettings` flattens its `TestData`: `test.data.env` is `test.env` in the
            // serialized settings.
            let path = row.field.replacen("test.data.", "test.", 1);
            let absent = if matches!(row.kind, OptionKind::Switch) {
                Absent::Off
            } else {
                Absent::Skip
            };
            vec![(path, absent)]
        }
    }
}

/// `SB_WINDOW_WIDTH` and `SB_WINDOW_HEIGHT` together: the window size, unless the same source
/// gives `SB_WINDOW_SIZE`, which wins and is read after them.
///
/// A side the source gives alone is laid over the size already there (a lower source's), and
/// the other side is left at zero when there is none. A size with a side of zero is what
/// [`Settings::validate`] refuses ("both must be set, and above zero"), so a lone side is
/// refused when the settings are checked and not when it is read: the other side may come
/// from a higher source.
fn read_window_axes(
    settings: &mut Settings,
    var: &dyn Fn(&str) -> Option<String>,
    problems: &mut Vec<SettingsProblem>,
) {
    let mut side = |env: &str| -> Option<u32> {
        let mut number: Option<u32> = None;
        read(var, problems, env, |name, text| number.assign(name, text));

        number
    };
    let width = side("SB_WINDOW_WIDTH");
    let height = side("SB_WINDOW_HEIGHT");
    if var("SB_WINDOW_SIZE").is_some() || (width.is_none() && height.is_none()) {
        return;
    }
    let current = settings
        .browser
        .window_size
        .unwrap_or(WindowSize::new(0, 0));
    settings.browser.window_size = Some(WindowSize::new(
        width.unwrap_or(current.width),
        height.unwrap_or(current.height),
    ));
}

/// Hands every row of the table to the macro `$callback`, so that another
/// crate can build its own items (the `sbrs test` flags) from the same rows.
///
/// A row is `(field, "long", ["alias"..], ["sb name"..], "SB_VAR", [kind],
/// "help")`.
#[macro_export]
#[doc(hidden)]
macro_rules! sb_test_options {
    ($callback:ident) => {
        $callback! {
            // Settings that exist since the first release, in the order their
            // variables are read.
            (browser.kind, "browser", [], [], "SB_BROWSER", [value(Browser)], "The browser to run: chrome, edge, firefox, brave, ... (SB_BROWSER)"),
            (browser.headless, "headless", ["headless2"], ["headless", "headless2"], "SB_HEADLESS", [switch], "Run without a visible window (Chrome's new headless mode) (SB_HEADLESS)"),
            (run.default_timeout, "timeout", [], [], "SB_TIMEOUT", [env(Seconds)], "The default wait, in whole seconds (SB_TIMEOUT)"),
            (session.webdriver_url, "webdriver-url", [], [], "SB_WEBDRIVER_URL", [env(Url)], "The WebDriver server to use (SB_WEBDRIVER_URL)"),
            (browser.executable_path, "binary-location", ["bl"], ["binary_location"], "SB_CHROME_BIN", [value(Path)], "The browser binary to run (SB_CHROME_BIN)"),
            (session.remote_webdriver_timeout_seconds, "remote-webdriver-timeout", [], [], "SB_REMOTE_WEBDRIVER_TIMEOUT", [env(Seconds)], "Client timeout for each request to a Remote WebDriver server (SB_REMOTE_WEBDRIVER_TIMEOUT)"),
            (browser.proxy.server, "proxy", ["proxy-server", "proxy-string"], [], "SB_PROXY", [value(Text)], "The proxy server: HOST:PORT or USER:PASS@HOST:PORT (SB_PROXY)"),
            (browser.window_size, "window-width", [], [], "SB_WINDOW_WIDTH", [env(Pixels)], "The browser window's width in pixels (SB_WINDOW_WIDTH)"),
            (browser.window_size, "window-height", [], [], "SB_WINDOW_HEIGHT", [env(Pixels)], "The browser window's height in pixels (SB_WINDOW_HEIGHT)"),
            (browser.user_data_dir, "user-data-dir", [], [], "SB_USER_DATA_DIR", [value(Path)], "The browser profile folder (SB_USER_DATA_DIR)"),
            (browser.extensions, "extension-dir", [], [], "SB_EXTENSION_DIR", [value(Path)], "Unpacked extension folders to load, separated by commas (SB_EXTENSION_DIR)"),
            (browser.lang, "locale", [], [], "SB_LOCALE", [value(Text)], "The browser locale (SB_LOCALE)"),
            (browser.user_agent, "user-agent", ["agent"], [], "SB_USER_AGENT", [value(Text)], "The user agent string (SB_USER_AGENT)"),
            (session.mode, "mode", [], [], "SB_MODE", [value(Mode)], "The driver mode: webdriver, cdp or uc (SB_MODE)"),
            (browser.ad_block, "ad-block", ["adblock", "block-ads"], ["ad_block_on"], "SB_AD_BLOCK", [switch], "Block the requests of ad and tracker hosts (SB_AD_BLOCK)"),
            (session.reuse, "reuse-session", ["rs"], [], "SB_REUSE_SESSION", [switch], "Share browsers among the tests of the run (SB_REUSE_SESSION)"),
            (session.reuse_class, "reuse-class-session", ["rcs"], [], "SB_REUSE_CLASS_SESSION", [switch], "Share a browser among the tests of one module (SB_REUSE_CLASS_SESSION)"),
            (browser.mobile, "mobile", ["mobile-emulator"], ["is_mobile"], "SB_MOBILE", [switch], "Emulate a mobile phone (SB_MOBILE)"),
            (test.threads, "threads", [], [], "SB_THREADS", [env(Count)], "The number of parallel test threads (SB_THREADS)"),
            (browser.proxy.pac_url, "proxy-pac-url", ["pac-url"], [], "SB_PROXY_PAC_URL", [value(Url)], "A proxy auto-config URL (SB_PROXY_PAC_URL)"),
            (browser.switches, "esc-end", ["ee"], [], "SB_ESC_END", [switch_in(EscEnd, "esc-end")], "End the current test when ESC is pressed in the browser (SB_ESC_END)"),
            (browser.switches, "rec-sb-cdp", [], [], "SB_REC_SB_CDP", [switch_in(RecSbCdp, "rec-sb-cdp")], "Write recordings as pure CDP code (SB_REC_SB_CDP)"),
            (fingerprint.file, "fingerprint", [], [], "SB_FINGERPRINT", [value(Path)], "A fingerprint file every browser presents (SB_FINGERPRINT)"),
            (browser.url, "start-page", ["url"], [], "SB_START_PAGE", [value(Url)], "The page every browser opens first (SB_START_PAGE)"),
            (settings_file, "settings-file", ["settings"], [], "SB_SETTINGS_FILE", [value(Path)], "The settings file to read instead of sbase_config.toml (SB_SETTINGS_FILE)"),
            // Which browser: shorthands for a value of SB_BROWSER, SB_HEADLESS and SB_MODE.
            (browser.kind, "chrome", [], [], "SB_BROWSER", [sets("chrome")], "Run Chrome (SB_BROWSER=chrome)"),
            (browser.kind, "edge", [], [], "SB_BROWSER", [sets("edge")], "Run Edge (SB_BROWSER=edge)"),
            (browser.kind, "firefox", [], [], "SB_BROWSER", [sets("firefox")], "Run Firefox (SB_BROWSER=firefox)"),
            (browser.kind, "brave", [], [], "SB_BROWSER", [sets("brave")], "Run Brave (SB_BROWSER=brave)"),
            (browser.kind, "comet", [], [], "SB_BROWSER", [sets("comet")], "Run Comet (SB_BROWSER=comet)"),
            (browser.kind, "opera", [], [], "SB_BROWSER", [sets("opera")], "Run Opera (SB_BROWSER=opera)"),
            (browser.kind, "safari", [], [], "SB_BROWSER", [sets("safari")], "Run Safari (SB_BROWSER=safari)"),
            (browser.kind, "ie", [], [], "SB_BROWSER", [sets("ie")], "Run Internet Explorer (SB_BROWSER=ie)"),
            (browser.kind, "cft", [], [], "SB_BROWSER", [sets("cft")], "Run Chrome for Testing (SB_BROWSER=cft)"),
            (browser.kind, "chs", [], [], "SB_BROWSER", [sets("chs")], "Run chrome-headless-shell (SB_BROWSER=chs)"),
            (browser.kind, "chromium", ["use-chromium"], [], "SB_BROWSER", [sets("chromium")], "Run Chromium (SB_BROWSER=chromium)"),
            (browser.headless, "headed", ["gui"], ["headed"], "SB_HEADLESS", [sets("false")], "Run with a visible window (SB_HEADLESS=false)"),
            (session.mode, "uc", ["undetected", "undetectable"], ["uc", "undetected", "undetectable"], "SB_MODE", [sets("uc")], "Run in undetected-chromedriver mode (SB_MODE=uc)"),
            (browser.switches, "headless1", [], [], "SB_HEADLESS1", [switch_in(Headless1, "headless1")], "Run headless with chrome-headless-shell (Chrome's old headless mode) (SB_HEADLESS1)"),
            (browser.lang, "locale-code", [], [], "SB_LOCALE_CODE", [value(Text)], "The browser language code, such as fr or ar_eg (SB_LOCALE_CODE)"),
            (session.grid.server, "server", [], ["servername"], "SB_SERVER", [value(Text)], "The Selenium Grid server to connect to (SB_SERVER)"),
            (session.grid.port, "port", [], [], "SB_PORT", [value(Port)], "The port of the Selenium Grid server (SB_PORT)"),
            (session.grid.protocol, "protocol", [], [], "SB_PROTOCOL", [value(Text)], "The protocol of the Selenium Grid server: http or https (SB_PROTOCOL)"),
            // Chromium switches.
            (browser.chromium.args, "chromium-arg", [], [], "SB_CHROMIUM_ARG", [value(List)], "Extra Chromium switches, separated by commas (SB_CHROMIUM_ARG)"),
            (browser.switches, "dark-mode", ["dark"], [], "SB_DARK_MODE", [switch_in(DarkMode, "dark-mode")], "Force Chrome's dark theme on web pages (SB_DARK_MODE)"),
            (browser.switches, "devtools", ["open-devtools"], [], "SB_DEVTOOLS", [switch_in(Devtools, "devtools")], "Open the developer tools with each tab (headed only) (SB_DEVTOOLS)"),
            (browser.chromium.disable_features, "disable-features", [], [], "SB_DISABLE_FEATURES", [value(List)], "Chromium features to switch off, separated by commas (SB_DISABLE_FEATURES)"),
            (browser.switches, "disable-gpu", [], [], "SB_DISABLE_GPU", [switch_in(DisableGpu, "disable-gpu")], "Switch the GPU off (SB_DISABLE_GPU)"),
            (browser.switches, "disable-ws", ["dws", "disable-web-security"], [], "SB_DISABLE_WS", [switch_in(DisableWebSecurity, "disable-web-security")], "Switch web security off (SB_DISABLE_WS)"),
            (browser.switches, "guest-mode", ["guest"], [], "SB_GUEST_MODE", [switch_in(Guest, "guest")], "Open the browser as a guest (SB_GUEST_MODE)"),
            (browser.switches, "incognito", ["incognito-mode"], [], "SB_INCOGNITO", [switch_in(Incognito, "incognito")], "Open the browser in an incognito window (SB_INCOGNITO)"),
            (browser.chromium.host_resolver_rules, "host-resolver-rules", [], [], "SB_HOST_RESOLVER_RULES", [value(Text)], "Chromium rules that map host names to addresses (SB_HOST_RESOLVER_RULES)"),
            (browser.switches, "no-sandbox", [], [], "SB_NO_SANDBOX", [switch_in(NoSandbox, "no-sandbox")], "Start Chromium without its sandbox (SB_NO_SANDBOX)"),
            (browser.switches, "swiftshader", [], [], "SB_SWIFTSHADER", [switch_in(Swiftshader, "swiftshader")], "Render with SwiftShader, a software WebGL renderer (SB_SWIFTSHADER)"),
            (browser.switches, "enable-3d-apis", [], [], "SB_ENABLE_3D_APIS", [switch_in(Enable3dApis, "enable-3d-apis")], "Keep the GPU on in a headless session, for WebGL (SB_ENABLE_3D_APIS)"),
            (browser.switches, "maximize", ["maximize-window", "fullscreen"], [], "SB_MAXIMIZE", [switch_in(Maximize, "maximize")], "Maximize the browser window (SB_MAXIMIZE)"),
            // Preferences and launch settings that were implemented before the table.
            (browser.switches, "block-images", [], [], "SB_BLOCK_IMAGES", [switch_in(BlockImages, "block-images")], "Do not load images (SB_BLOCK_IMAGES)"),
            (browser.switches, "disable-js", [], [], "SB_DISABLE_JS", [switch_in(DisableJavaScript, "disable-java-script")], "Turn JavaScript off (SB_DISABLE_JS)"),
            (browser.switches, "disable-cookies", [], [], "SB_DISABLE_COOKIES", [switch_in(DisableCookies, "disable-cookies")], "Refuse every cookie (SB_DISABLE_COOKIES)"),
            (browser.switches, "do-not-track", [], [], "SB_DO_NOT_TRACK", [switch_in(DoNotTrack, "do-not-track")], "Send the DNT: 1 header (SB_DO_NOT_TRACK)"),
            (browser.switches, "external-pdf", [], [], "SB_EXTERNAL_PDF", [switch_in(ExternalPdf, "external-pdf")], "Download PDF files instead of showing them (SB_EXTERNAL_PDF)"),
            (browser.switches, "enable-sync", [], [], "SB_ENABLE_SYNC", [switch_in(EnableSync, "enable-sync")], "Leave Chrome Sync on (SB_ENABLE_SYNC)"),
            (browser.switches, "disable-csp", ["dcsp", "no-csp"], [], "SB_DISABLE_CSP", [switch_in(DisableCsp, "disable-csp")], "Strip the Content Security Policy from every response (SB_DISABLE_CSP)"),
            (browser.switches, "use-auto-ext", ["auto-ext"], [], "SB_USE_AUTO_EXT", [switch_in(AutomationExtension, "automation-extension")], "Keep Chrome's automation extension (SB_USE_AUTO_EXT)"),
            (browser.switches, "proxy-driver", [], [], "SB_PROXY_DRIVER", [switch_in(ProxyForDriverDownload, "proxy-for-driver-download")], "Download the driver through the proxy (SB_PROXY_DRIVER)"),
            (session.page_load_strategy, "page-load-strategy", ["pls"], [], "SB_PAGE_LOAD_STRATEGY", [value(Strategy)], "How long a navigation waits: normal, eager or none (SB_PAGE_LOAD_STRATEGY)"),
            (browser.window_size, "window-size", [], ["size"], "SB_WINDOW_SIZE", [value(Size)], "The window size: WIDTH,HEIGHT (SB_WINDOW_SIZE)"),
            (browser.window_position, "window-position", [], ["position"], "SB_WINDOW_POSITION", [value(Position)], "The window position: X,Y (SB_WINDOW_POSITION)"),
            (browser.device_metrics, "device-metrics", ["metrics"], [], "SB_DEVICE_METRICS", [value(Metrics)], "Emulate a phone screen: WIDTH,HEIGHT,PIXEL_RATIO (SB_DEVICE_METRICS)"),
            (browser.proxy.bypass_list, "proxy-bypass-list", [], [], "SB_PROXY_BYPASS_LIST", [value(List)], "Hosts that skip the proxy (SB_PROXY_BYPASS_LIST)"),
            (session.driver_version, "driver-version", [], [], "SB_DRIVER_VERSION", [value(Version)], "The driver version: a number, latest, stable, beta, ... (SB_DRIVER_VERSION)"),
            (session.capabilities, "cap-file", [], [], "SB_CAP_FILE", [value(Path)], "A JSON file of capabilities to add to the session's (SB_CAP_FILE)"),
            (session.capabilities, "cap-string", [], [], "SB_CAP_STRING", [value(Json)], "A JSON object of capabilities to add to the session's (SB_CAP_STRING)"),
            (session.extension_archives, "extension-zip", ["crx"], [], "SB_EXTENSION_ZIP", [value(List)], "Extension .zip or .crx files to load, separated by commas (SB_EXTENSION_ZIP)"),
            (session.firefox, "firefox-arg", [], [], "SB_FIREFOX_ARG", [value(List)], "Firefox arguments, separated by commas (SB_FIREFOX_ARG)"),
            (session.firefox, "firefox-pref", [], [], "SB_FIREFOX_PREF", [value(List)], "Firefox preferences, NAME:VALUE separated by commas (SB_FIREFOX_PREF)"),
            (browser.virtual_display, "xvfb", [], [], "SB_XVFB", [switch], "Open a virtual display for a headed browser on Linux (SB_XVFB)"),
            (browser.virtual_display, "xvfb-metrics", [], [], "SB_XVFB_METRICS", [value(Size)], "The virtual display size: WIDTH,HEIGHT (SB_XVFB_METRICS)"),
            (browser.debugging_port, "remote-debug", ["remote-debugger"], [], "SB_REMOTE_DEBUG", [switch], "Open Chrome's remote debugger on port 9222 (SB_REMOTE_DEBUG)"),
            (session.cdp_events, "log-cdp-events", ["log-cdp"], [], "SB_LOG_CDP_EVENTS", [switch], "Log the DevTools Protocol events (SB_LOG_CDP_EVENTS)"),
            (session.cdp_events, "uc-cdp-events", ["uc-cdp"], [], "SB_UC_CDP_EVENTS", [switch], "Log the DevTools Protocol events in undetected mode (SB_UC_CDP_EVENTS)"),
            // Run settings and page behaviour.
            (run.demo_mode, "demo", ["demo-mode"], [], "SB_DEMO", [switch], "Highlight each element and pause after an action (SB_DEMO)"),
            (run.slow_mode, "slow", ["slow-mode", "slowmo"], [], "SB_SLOW", [switch], "Pause briefly after each action (SB_SLOW)"),
            (run.demo_sleep, "demo-sleep", [], [], "SB_DEMO_SLEEP", [value(Seconds)], "Seconds to pause after each action in demo mode (SB_DEMO_SLEEP)"),
            (run.message_duration, "message-duration", [], [], "SB_MESSAGE_DURATION", [value(Seconds)], "Seconds a message stays on the page (SB_MESSAGE_DURATION)"),
            (run.skip_js_waits, "skip-js-waits", ["sjw"], [], "SB_SKIP_JS_WAITS", [switch], "Do not wait for the page's JavaScript to settle (SB_SKIP_JS_WAITS)"),
            (run.highlights, "highlights", [], [], "SB_HIGHLIGHTS", [value(Count)], "How many times to flash a highlighted element (SB_HIGHLIGHTS)"),
            (run.interval, "interval", [], [], "SB_INTERVAL", [value(Seconds)], "Seconds between the steps of a tour or presentation (SB_INTERVAL)"),
            (run.verify_delay, "verify-delay", [], [], "SB_VERIFY_DELAY", [value(Seconds)], "Seconds to wait before each MasterQA verification (SB_VERIFY_DELAY)"),
            (run.check_js, "check-js", [], ["js_checking_on"], "SB_CHECK_JS", [switch], "Check each page for JavaScript errors after it opens (SB_CHECK_JS)"),
            (run.wait_for_angularjs, "wait-for-angularjs", ["wfa"], [], "SB_WAIT_FOR_ANGULARJS", [switch], "Wait for AngularJS after opening a page and clicking (SB_WAIT_FOR_ANGULARJS)"),
            (run.disable_beforeunload, "disable-beforeunload", [], [], "SB_DISABLE_BEFOREUNLOAD", [switch], "Remove the beforeunload prompt after each page opens (SB_DISABLE_BEFOREUNLOAD)"),
            (run.reset_visual_baseline, "visual-baseline", [], [], "SB_VISUAL_BASELINE", [switch], "Set the visual baselines again instead of comparing (SB_VISUAL_BASELINE)"),
            (session.crumbs, "crumbs", [], [], "SB_CRUMBS", [switch], "Delete the cookies when a reused browser is handed to the next test (SB_CRUMBS)"),
            (run.timeout_multiplier, "timeout-multiplier", [], [], "SB_TIMEOUT_MULTIPLIER", [value(Float)], "Multiply the default waits by this number (SB_TIMEOUT_MULTIPLIER)"),
            (run.record_sleep, "rec-sleep", ["record-sleep"], [], "SB_REC_SLEEP", [switch], "Keep the sleeps of a recording (SB_REC_SLEEP)"),
            // Test data.
            (test.data.env, "env", [], ["environment"], "SB_ENV", [custom(Env, set_env)], "The environment the tests run against: qa, staging, production, ... (SB_ENV)"),
            (test.data.account, "account", [], [], "SB_ACCOUNT", [value(Text)], "A free account name for the tests (SB_ACCOUNT)"),
            (test.data.data, "data", [], [], "SB_DATA", [value(Text)], "Free test data (SB_DATA)"),
            (test.data.var1, "var1", [], [], "SB_VAR1", [value(Text)], "A free variable for the tests (SB_VAR1)"),
            (test.data.var2, "var2", [], [], "SB_VAR2", [value(Text)], "A free variable for the tests (SB_VAR2)"),
            (test.data.var3, "var3", [], [], "SB_VAR3", [value(Text)], "A free variable for the tests (SB_VAR3)"),
            (test.data.variables, "variables", [], [], "SB_VARIABLES", [custom(Dict, set_variables)], "A dictionary of values: JSON or a Python dictionary literal (SB_VARIABLES)"),
            // Debug modes, run control and evidence.
            (test.pdb, "pdb", [], [], "SB_PDB", [switch], "Stop in a post-mortem prompt when a test fails (SB_PDB)"),
            (test.trace, "trace", [], [], "SB_TRACE", [switch], "Stop in a prompt before each test's body (SB_TRACE)"),
            (test.final_debug, "final-debug", ["fdebug", "final-trace", "ftrace"], [], "SB_FINAL_DEBUG", [switch], "Stop in a prompt after each test, before the browser quits (SB_FINAL_DEBUG)"),
            (test.reruns, "reruns", [], [], "SB_RERUNS", [value(Count)], "Run a failing test again this many times (SB_RERUNS)"),
            (test.reruns_delay, "reruns-delay", [], [], "SB_RERUNS_DELAY", [value(Seconds)], "Seconds to wait between reruns (SB_RERUNS_DELAY)"),
            (test.time_limit, "time-limit", ["timelimit"], [], "SB_TIME_LIMIT", [value(Seconds)], "Fail a test that runs longer than this many seconds (SB_TIME_LIMIT)"),
            (test.runner, "runner", [], [], "SB_TEST_RUNNER", [value(Runner)], "The test runner: cargo or nextest (SB_TEST_RUNNER)"),
            (report.save_screenshot, "screenshot", ["save-screenshot", "ss"], [], "SB_SAVE_SCREENSHOT", [switch], "Save a screenshot of passing tests too (SB_SAVE_SCREENSHOT)"),
            (report.no_screenshot, "no-screenshot", ["ns"], [], "SB_NO_SCREENSHOT", [switch], "Save no screenshot of failing tests (SB_NO_SCREENSHOT)"),
            (report.list_fail_page, "list-fail-page", ["list-fail-pages", "lfp"], [], "SB_LIST_FAIL_PAGE", [switch], "Print the URL of the page a failing test ended on (SB_LIST_FAIL_PAGE)"),
            (report.archive_logs, "archive-logs", [], [], "SB_ARCHIVE_LOGS", [switch], "Keep the old latest_logs folder in archived_logs (SB_ARCHIVE_LOGS)"),
            (report.archive_downloads, "archive-downloads", [], [], "SB_ARCHIVE_DOWNLOADS", [switch], "Keep the old downloaded_files folder in archived_files (SB_ARCHIVE_DOWNLOADS)"),
            (report.dashboard, "dashboard", [], [], "SB_DASHBOARD", [switch], "Write the results dashboard (SB_DASHBOARD)"),
            (report.dash_title, "dash-title", [], [], "SB_DASH_TITLE", [value(Text)], "The dashboard's title (SB_DASH_TITLE)"),
            (report.html, "html", [], [], "SB_HTML", [value(Path)], "Write an HTML report to this file (SB_HTML)"),
            (report.junit_xml, "junit-xml", ["junitxml"], [], "SB_JUNIT_XML", [value(Path)], "Write a JUnit XML report to this file (SB_JUNIT_XML)"),
            (report.allure_dir, "alluredir", ["allure-dir"], [], "SB_ALLURE_DIR", [value(Path)], "Write Allure results to this folder (SB_ALLURE_DIR)"),

            (report.with_db_reporting, "with-db-reporting", [], [], "SB_WITH_DB_REPORTING", [switch], "Record each test's outcome in the results database (SB_WITH_DB_REPORTING)"),
            (report.database_env, "database-env", [], [], "SB_DATABASE_ENV", [value(Text)], "Which results database to use (SB_DATABASE_ENV)"),
            (report.with_s3_logging, "with-s3-logging", [], [], "SB_WITH_S3_LOGGING", [switch], "Upload each test's logs to S3 (SB_WITH_S3_LOGGING)"),
            // The recorder.
            (browser.switches, "rec", ["record", "recorder", "codegen"], ["recorder_ext"], "SB_RECORDER", [switch_in(RecorderMode, "recorder-mode")], "Record the browser actions as a test (SB_RECORDER)"),
            (browser.switches, "rec-behave", ["rec-gherkin"], [], "SB_REC_BEHAVE", [switch_in(RecBehave, "rec-behave")], "Also write the recording as a Gherkin feature (SB_REC_BEHAVE)"),
            (browser.switches, "rec-print", [], [], "SB_REC_PRINT", [switch_in(RecPrint, "rec-print")], "Print the recording when the test ends (SB_REC_PRINT)"),
            (browser.switches, "rec-sb-mgr", [], [], "SB_REC_SB_MGR", [switch_in(RecSbMgr, "rec-sb-mgr")], "Write the recording as a script (SB_REC_SB_MGR)"),
            // Accepted and ignored, or not a flag.
            (none, "all-scripts", [], [], "", [cli("sbrs test runs every example after the tests")], "Run every example after the tests"),
            (none, "with-testing-base", [], [], "", [noop("always on in the sb fixture, as upstream")], "Accepted and ignored"),
            (none, "with-basic-test-info", [], [], "", [noop("always on in the sb fixture, as upstream")], "Accepted and ignored"),
            (none, "with-page-source", [], [], "", [noop("always on in the sb fixture, as upstream")], "Accepted and ignored"),
            (none, "with-screen-shots", [], [], "", [noop("always on in the sb fixture, as upstream")], "Accepted and ignored"),
            (none, "log-path", [], [], "", [noop("deprecated upstream; logs always go to latest_logs/")], "Accepted and ignored"),
            (none, "with-selenium", [], [], "", [noop("deprecated upstream; always on")], "Accepted and ignored"),
            (none, "is-pytest", [], [], "", [noop("an internal pytest flag")], "Accepted and ignored"),
            (none, "uc-sub", ["uc-subprocess"], [], "", [noop("deprecated upstream; undetected mode always uses a subprocess")], "Accepted and ignored"),
            (none, "enable-ws", ["enable-web-security"], [], "", [noop("web security is never switched off unless asked")], "Accepted and ignored"),
            (none, "fast", ["fast-mode"], [], "", [noop("the CDP engine makes no wait to skip")], "Accepted and ignored"),
            (none, "multi-proxy", [], [], "", [noop("each CDP browser answers its own proxy login; WebDriver cannot log in to a proxy")], "Accepted and ignored"),
            (none, "raise-test-failure", [], ["rtf"], "", [na("Sb::run always returns the error; the caller decides")], "Not a flag"),
            (none, "test-mode", [], ["test"], "", [na("Sb::run is SB(test=True); run_browser_test is the other form")], "Not a flag"),
        }
    };
}

sb_test_options!(option_table);

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    #[test]
    fn the_help_of_a_row_ends_with_its_variable() {
        // `--help` is made from these rows, so it tells where the variable is.
        for row in TEST_OPTIONS.iter().filter(|row| !row.env.is_empty()) {
            let tail = match row.kind {
                OptionKind::Sets(value) => format!("({}={value})", row.env),
                _ => format!("({})", row.env),
            };
            assert!(
                row.help.ends_with(&format!(" {tail}")),
                "{}: the help does not end with {tail}: {}",
                row.long,
                row.help
            );
            assert!(!row.description().is_empty(), "{}", row.long);
            assert!(!row.description().contains("(SB_"), "{}", row.long);
        }
        for row in TEST_OPTIONS.iter().filter(|row| row.env.is_empty()) {
            assert_eq!(row.description(), row.help, "{}", row.long);
        }
    }

    use super::*;

    use crate::browser::config::{Browser, DriverMode};
    use std::path::PathBuf;

    fn read_all(pairs: &[(&str, &str)]) -> (Settings, Vec<SettingsProblem>) {
        let var = |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        };
        let Variables { dict, problems, .. } = read_variables(&var);
        let layer = crate::config::layers::Layer::new(figment::Metadata::named("test"), dict);
        let settings = crate::config::layers::extract(&[&layer]).expect("the variables extract");
        (settings, problems)
    }

    #[test]
    fn every_spelling_belongs_to_one_row() {
        let mut seen = BTreeSet::new();
        for row in TEST_OPTIONS {
            for flag in row.flags() {
                assert!(
                    seen.insert(normalise(flag)),
                    "{flag} is spelled by two rows of the table"
                );
            }
        }
    }

    #[test]
    fn a_variable_belongs_to_one_row_unless_the_rows_are_shorthands() {
        let mut seen = BTreeSet::new();
        for row in TEST_OPTIONS.iter().filter(|row| row.kind.reads_env()) {
            assert!(
                row.env.starts_with("SB_"),
                "{}: {} is not an SB_ variable",
                row.long,
                row.env
            );
            assert!(seen.insert(row.env), "{} is read by two rows", row.env);
        }
        for row in TEST_OPTIONS
            .iter()
            .filter(|row| matches!(row.kind, OptionKind::Sets(_)))
        {
            assert!(
                TEST_OPTIONS
                    .iter()
                    .any(|other| other.env == row.env && other.kind.reads_env()),
                "{} sets {}, which no row reads",
                row.long,
                row.env
            );
        }
    }

    #[test]
    fn rows_without_a_variable_are_the_ones_that_do_nothing() {
        for row in TEST_OPTIONS {
            let nothing = matches!(
                row.kind,
                OptionKind::NoOp(_) | OptionKind::Cli(_) | OptionKind::NotAFlag(_)
            );
            assert_eq!(row.env.is_empty(), nothing, "{}", row.long);
            assert_eq!(
                !row.has_field(),
                nothing && row.field == "none",
                "{}",
                row.long
            );
            assert!(!row.help.is_empty(), "{}", row.long);
        }
    }

    #[test]
    fn a_flag_is_found_in_any_spelling() {
        for spelling in ["--dark", "dark", "--dark_mode", "-dark-mode", "--DARK"] {
            assert_eq!(find_option(spelling).map(|row| row.long), Some("dark-mode"));
        }
        assert!(find_option("--nosuch").is_none());
        let metrics = find_option("--metrics").unwrap();
        assert!(metrics.matches_sb_name("device_metrics"));
        let mobile = find_option("--mobile-emulator").unwrap();
        assert!(mobile.matches_sb_name("is_mobile"));
        assert!(!mobile.matches_sb_name("is_desktop"));
    }

    #[test]
    fn a_switch_reads_true_and_false_and_refuses_the_rest() {
        let (settings, problems) = read_all(&[("SB_DARK_MODE", "true"), ("SB_PDB", "1")]);
        assert!(problems.is_empty());
        assert!(settings.browser.has(LaunchSwitch::DarkMode) && settings.test.pdb);

        let (settings, problems) = read_all(&[("SB_DARK_MODE", "off")]);
        assert!(problems.is_empty() && !settings.browser.has(LaunchSwitch::DarkMode));

        let (_, problems) = read_all(&[("SB_DEVTOOLS", "maybe")]);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].source, "SB_DEVTOOLS");
        assert_eq!(
            problems[0].error.to_string(),
            "invalid configuration: cannot parse bool: maybe"
        );
    }

    #[test]
    fn a_value_is_stored_as_text_a_path_a_number_or_seconds() {
        let (settings, problems) = read_all(&[
            ("SB_WINDOW_SIZE", "1280,720"),
            ("SB_USER_DATA_DIR", "profile-dir"),
            ("SB_PORT", "4445"),
            ("SB_HIGHLIGHTS", "3"),
            ("SB_RERUNS", "2"),
            ("SB_RERUNS_DELAY", "0.25"),
            ("SB_TIME_LIMIT", "30"),
            ("SB_TIMEOUT", "7"),
            ("SB_FINGERPRINT", "profile.json"),
            ("SB_DASH_TITLE", "My Run"),
        ]);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            settings.browser.window_size,
            Some(WindowSize::new(1280, 720))
        );
        assert_eq!(
            settings.browser.user_data_dir,
            Some(PathBuf::from("profile-dir"))
        );
        assert_eq!(settings.session.grid.port, Some(4445));
        assert_eq!(settings.run.highlights, Some(3));
        assert_eq!(settings.test.reruns, 2);
        assert!((settings.test.reruns_delay - 0.25).abs() < f64::EPSILON);
        assert_eq!(settings.test.time_limit, Some(30.0));
        assert_eq!(
            settings.run.default_timeout,
            Some(std::time::Duration::from_secs(7))
        );
        assert_eq!(
            settings.fingerprint.file,
            Some(PathBuf::from("profile.json"))
        );
        assert_eq!(settings.report.dash_title.as_deref(), Some("My Run"));
    }

    #[test]
    fn a_number_that_does_not_parse_names_its_variable() {
        let (_, problems) = read_all(&[
            ("SB_PORT", "http"),
            ("SB_TIME_LIMIT", "-1"),
            ("SB_INTERVAL", "soon"),
        ]);
        let sources: Vec<_> = problems.iter().map(|p| p.source.as_str()).collect();
        assert_eq!(sources, ["SB_PORT", "SB_INTERVAL", "SB_TIME_LIMIT"]);
        assert!(problems[0]
            .error
            .to_string()
            .starts_with("invalid configuration: SB_PORT: "));
        assert!(problems[2]
            .error
            .to_string()
            .contains("expected a number of seconds"));
    }

    #[test]
    fn shorthands_are_not_read_from_the_environment() {
        // SB_BROWSER=firefox is the `browser` row's; the `--chrome` and
        // `--firefox` rows only tell the command line what to write.
        let (settings, problems) = read_all(&[("SB_BROWSER", "firefox"), ("SB_MODE", "uc")]);
        assert!(problems.is_empty());
        assert_eq!(settings.browser.kind, Browser::Firefox);
        assert_eq!(settings.session.mode, Some(DriverMode::Uc));
        let chrome = find_option("--chrome").unwrap();
        assert_eq!(chrome.kind, OptionKind::Sets("chrome"));
        assert_eq!(chrome.env, "SB_BROWSER");
        let headed = find_option("--gui").unwrap();
        assert_eq!(
            (headed.env, headed.kind),
            ("SB_HEADLESS", OptionKind::Sets("false"))
        );
    }

    #[test]
    fn test_data_is_checked_when_it_is_read() {
        let (settings, problems) = read_all(&[
            ("SB_ENV", "QA"),
            ("SB_ACCOUNT", "alice"),
            ("SB_DATA", "abc"),
            ("SB_VAR1", "x"),
            ("SB_VARIABLES", "{'k': 1}"),
        ]);
        assert!(problems.is_empty(), "{problems:?}");
        let data = &settings.test.data;
        assert_eq!(data.env(), "qa");
        assert_eq!(data.account.as_deref(), Some("alice"));
        assert_eq!(data.data.as_deref(), Some("abc"));
        assert_eq!(data.var1.as_deref(), Some("x"));
        assert_eq!(data.variables["k"], 1);

        let (settings, problems) = read_all(&[("SB_ENV", "nosuch"), ("SB_VARIABLES", "[1]")]);
        assert_eq!(settings.test.data.env(), "test");
        assert_eq!(problems.len(), 2);
        assert!(problems[0]
            .error
            .to_string()
            .contains("choose from qa, rc, staging"));
        assert!(problems[1]
            .error
            .to_string()
            .contains("Expecting a dictionary"));
    }

    #[test]
    fn rows_with_no_variable_read_nothing() {
        // `--all-scripts` has no variable, and `none` is not a field.
        let (settings, problems) = read_all(&[("", "true"), ("none", "true")]);
        assert!(problems.is_empty());
        assert_eq!(settings, Settings::default());
    }
}
