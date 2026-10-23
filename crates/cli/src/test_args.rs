//! The command line of `sbrs test`.
//!
//! `sbrs test` runs the project's tests through `cargo test` (or `cargo nextest`)
//! and hands every option to them as an `SB_*` variable, which the library reads
//! with [`Settings`]. The options are the rows of the library's option table
//! (`seleniumbase_rs::config::test_options`): this module builds one flag per
//! row from the table itself with a macro, so a row added there is a flag here
//! with no other change. The flags of pytest itself (`-k`, `-m`, `-n`, `-s`,
//! `--co`) and the ones that choose what Cargo runs (`--test`, `--example`,
//! `--release`, `--features`) are written by hand in [`TestArgs`].
//!
//! A switch is `--flag`, which is true, or `--flag=false`; a flag the settings
//! file or the environment turned on can be turned off that way. Every flag is
//! spelled with `-` and also with `_` (`--no_sandbox`, see [`python_spellings`])
//! and under each of Python's aliases.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use seleniumbase_rs::cli::scripts::all_scripts;
use seleniumbase_rs::cli::scripts::run::{self, Runner, TestRun, TestTarget};
use seleniumbase_rs::cli::scripts::run_filter;
use seleniumbase_rs::cli::scripts::run_logs;
use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::config::test_options::find_option;
use seleniumbase_rs::config::test_settings::TestData;
use seleniumbase_rs::SeleniumBaseError;

use crate::failure::ArgumentError;

/// Builds `TestOptions`, one flag for each row of the option table, and
/// `TestOptions::env`, which turns the flags that were given into `SB_*`
/// pairs.
///
/// A macro cannot make up a field name, so each flag takes the next name of the
/// list at the first call (`f000`, `f001`, ...): the list must be as long as
/// the table has flags (a test checks it). `this`, `out` and `sh` are passed
/// along so that every step of the expansion names the same variables.
macro_rules! test_options {
    ($($row:tt),+ $(,)?) => {
        test_options!(@next [this out sh]
            [
                f000 f001 f002 f003 f004 f005 f006 f007 f008 f009 f010 f011
                f012 f013 f014 f015 f016 f017 f018 f019 f020 f021 f022 f023
                f024 f025 f026 f027 f028 f029 f030 f031 f032 f033 f034 f035
                f036 f037 f038 f039 f040 f041 f042 f043 f044 f045 f046 f047
                f048 f049 f050 f051 f052 f053 f054 f055 f056 f057 f058 f059
                f060 f061 f062 f063 f064 f065 f066 f067 f068 f069 f070 f071
                f072 f073 f074 f075 f076 f077 f078 f079 f080 f081 f082 f083
                f084 f085 f086 f087 f088 f089 f090 f091 f092 f093 f094 f095
                f096 f097 f098 f099 f100 f101 f102 f103 f104 f105 f106 f107
                f108 f109 f110 f111 f112 f113 f114 f115 f116 f117 f118 f119
                f120 f121 f122 f123 f124 f125 f126 f127 f128 f129 f130 f131
                f132 f133 f134 f135 f136 f137 f138 f139 f140 f141 f142 f143
                f144 f145 f146 f147 f148 f149 f150 f151 f152 f153 f154 f155
                f156 f157 f158 f159 f160 f161 f162 f163 f164 f165 f166 f167
                f168 f169 f170 f171 f172 f173 f174 f175 f176 f177 f178 f179
            ]
            [] [] $($row)+);
    };

    // No row is left: write the struct and the method.
    (@next [$this:ident $out:ident $sh:ident] $ids:tt [$($fields:tt)*] [$($pushes:tt)*]) => {
        /// The flags that come from the option table.
        #[derive(Debug, usage::Args)]
        #[usage(args_override_self = false)]
        pub(crate) struct TestOptions {
            $($fields)*
        }

        impl TestOptions {
            /// The `SB_*` variables of the flags that were given, in the order of the
            /// table. A switch gives `true` or `false`; a flag that stands for a
            /// value of a variable (`--firefox`) gives that value.
            ///
            /// # Errors
            ///
            /// Returns a message when two such flags choose different values of one
            /// variable (`--firefox --chrome`).
            pub(crate) fn env(&self) -> Result<Vec<(&'static str, String)>, String> {
                let $this = self;
                let mut $out: Vec<(&'static str, String)> = Vec::new();
                let mut $sh: Vec<(&'static str, &'static str, &'static str)> = Vec::new();
                $($pushes)*
                shorthand_conflict(&$sh)?;
                Ok($out)
            }
        }
    };

    // The group a row belongs to, by the path of its field.
    (@next $names:tt $ids:tt $fields:tt $pushes:tt (test.data.$leaf:ident, $($row:tt)*) $($more:tt)*) => {
        test_options!(@alias "Test data" $names $ids $fields $pushes ($($row)*) $($more)*);
    };
    (@next $names:tt $ids:tt $fields:tt $pushes:tt (test.$leaf:ident, $($row:tt)*) $($more:tt)*) => {
        test_options!(@alias "Test run, debugging and reports" $names $ids $fields $pushes ($($row)*) $($more)*);
    };
    (@next $names:tt $ids:tt $fields:tt $pushes:tt ($($path:ident).+, $($row:tt)*) $($more:tt)*) => {
        test_options!(@alias "Browser and page options" $names $ids $fields $pushes ($($row)*) $($more)*);
    };

    // The aliases of a row become `aliases = [..],`, or nothing when it has none: the derive
    // refuses an empty list.
    (@alias $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [], $($row:tt)*) $($more:tt)*) => {
        test_options!(@kind $h $names $ids $fields $pushes ($long, [], $($row)*) $($more)*);
    };
    (@alias $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($a:tt)+], $($row:tt)*) $($more:tt)*) => {
        test_options!(@kind $h $names $ids $fields $pushes
            ($long, [aliases = [$($a)+],], $($row)*) $($more)*);
    };

    // Rows that are no flag of `sbrs test`: `--all-scripts` is written by hand, a variable
    // with no flag and a keyword of `SB(...)` have none.
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ("all-scripts", $($row:tt)*) $($more:tt)*) => {
        test_options!(@next $names $ids $fields $pushes $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [env($t:ident)], $help:tt) $($more:tt)*) => {
        test_options!(@next $names $ids $fields $pushes $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [na($why:tt)], $help:tt) $($more:tt)*) => {
        test_options!(@next $names $ids $fields $pushes $($more)*);
    };

    // A switch: `--flag`, `--flag=true` or `--flag=false`.
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [switch], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* default_missing = "true", require_equals,
                choices("true", "false"), help = $help, help_heading = $h)]]
            [switch $env] $($more)*);
    };
    // A switch that stands for a value of a variable: `--firefox` is `SB_BROWSER=firefox`.
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [sets($v:tt)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* default_missing = "true", require_equals,
                choices("true", "false"), help = $help, help_heading = $h)]]
            [sets $env $v $long] $($more)*);
    };
    // Options that are accepted and ignored: hidden from help and completion.
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [noop($why:tt)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* hide, default_missing = "true", require_equals,
                choices("true", "false"), help = $help, help_heading = $h)]]
            [none] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [cli($why:tt)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* hide, default_missing = "true", require_equals,
                choices("true", "false"), help = $help, help_heading = $h)]]
            [none] $($more)*);
    };

    // Options with a value, by what the value is.
    // The two that take folders, for the shell's completion.
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ("user-data-dir", [$($al:tt)*], $sb:tt, $env:tt, [value(Path)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = "user-data-dir", $($al)* value_name = "DIR",
                value_hint = usage::ValueHint::DirPath, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ("extension-dir", [$($al:tt)*], $sb:tt, $env:tt, [value(Path)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = "extension-dir", $($al)* value_name = "DIR",
                value_hint = usage::ValueHint::DirPath, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };

    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Path)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "PATH",
                value_hint = usage::ValueHint::AnyPath, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Url)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "URL",
                value_hint = usage::ValueHint::Url, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Runner)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "RUNNER",
                choices("cargo", "nextest"), help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Mode)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "MODE",
                choices("webdriver", "cdp", "uc"), help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Strategy)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "STRATEGY",
                choices("normal", "eager", "none"), help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Browser)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "BROWSER",
                choices("chrome", "chromium", "edge", "firefox", "brave", "comet", "opera",
                    "safari", "ie", "cft", "chs"),
                choices_strict = false, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Seconds)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "SECONDS",
                value_hint = usage::ValueHint::Other, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Count)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "COUNT",
                value_hint = usage::ValueHint::Other, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value($t:ident)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "VALUE",
                value_hint = usage::ValueHint::Other, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    // `--env` takes one of Python's environments; the value is checked when the run is planned.
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [custom(Env, $f:path)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "ENV",
                choices("qa", "rc", "staging", "develop", "production", "performance",
                    "replica", "fedramp", "offline", "online", "master", "remote", "legacy",
                    "local", "alpha", "beta", "demo", "gdpr", "main", "test", "gov", "new",
                    "old", "uat"),
                choices_strict = false, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };
    (@kind $h:tt $names:tt $ids:tt $fields:tt $pushes:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [custom($t:ident, $f:path)], $help:tt) $($more:tt)*) => {
        test_options!(@emit $names $ids $fields $pushes
            [#[usage(long = $long, $($al)* value_name = "VALUE",
                value_hint = usage::ValueHint::Other, help = $help, help_heading = $h)]]
            [value $env] $($more)*);
    };

    // Takes the next field name and writes the field and the code that reads it.
    (@emit [$this:ident $out:ident $sh:ident] [$id:ident $($ids:ident)*] [$($f:tt)*] [$($p:tt)*]
        [$($attr:tt)*] [switch $env:tt] $($more:tt)*) => {
        test_options!(@next [$this $out $sh] [$($ids)*]
            [$($f)* $($attr)* $id: Option<bool>,]
            [$($p)* if let Some(on) = $this.$id { $out.push(($env, on.to_string())); }]
            $($more)*);
    };
    (@emit [$this:ident $out:ident $sh:ident] [$id:ident $($ids:ident)*] [$($f:tt)*] [$($p:tt)*]
        [$($attr:tt)*] [sets $env:tt $v:tt $long:tt] $($more:tt)*) => {
        test_options!(@next [$this $out $sh] [$($ids)*]
            [$($f)* $($attr)* $id: Option<bool>,]
            [$($p)* if $this.$id == Some(true) {
                $out.push(($env, ($v).to_owned()));
                $sh.push(($env, $v, $long));
            }]
            $($more)*);
    };
    (@emit [$this:ident $out:ident $sh:ident] [$id:ident $($ids:ident)*] [$($f:tt)*] [$($p:tt)*]
        [$($attr:tt)*] [value $env:tt] $($more:tt)*) => {
        test_options!(@next [$this $out $sh] [$($ids)*]
            [$($f)* $($attr)* $id: Option<String>,]
            [$($p)* if let Some(text) = &$this.$id { $out.push(($env, text.clone())); }]
            $($more)*);
    };
    (@emit [$this:ident $out:ident $sh:ident] [$id:ident $($ids:ident)*] [$($f:tt)*] [$($p:tt)*]
        [$($attr:tt)*] [none] $($more:tt)*) => {
        test_options!(@next [$this $out $sh] [$($ids)*]
            [$($f)* $($attr)* $id: Option<bool>,]
            [$($p)*]
            $($more)*);
    };
}

seleniumbase_rs::sb_test_options!(test_options);

/// Refuses two flags that choose different values of one variable.
pub(crate) fn shorthand_conflict(
    sets: &[(&'static str, &'static str, &'static str)],
) -> Result<(), String> {
    for (at, (env, value, flag)) in sets.iter().enumerate() {
        if let Some((_, _, other)) = sets[..at]
            .iter()
            .find(|(other_env, other_value, _)| other_env == env && other_value != value)
        {
            return Err(format!(
                "--{other} and --{flag} both choose {env}: give only one of them"
            ));
        }
    }
    Ok(())
}

/// The arguments of `sbrs test`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct TestArgs {
    /// Run only the tests whose name contains this text.
    #[usage(value_hint = usage::ValueHint::Other, conflicts("--example"))]
    pub(crate) filter: Option<String>,
    /// Run one integration test file from tests/.
    #[usage(long, value_name = "NAME", value_hint = usage::ValueHint::Other, conflicts("--example"))]
    pub(crate) test: Option<String>,
    /// Run one example from examples/ instead of the tests.
    #[usage(long, value_name = "NAME", value_hint = usage::ValueHint::Other)]
    pub(crate) example: Option<String>,
    /// Build with the release profile.
    #[usage(long, bool_value)]
    pub(crate) release: bool,
    /// Cargo features to enable, separated by commas.
    #[usage(long, delimiter = ',', value_hint = usage::ValueHint::Other)]
    pub(crate) features: Vec<String>,
    /// Select tests by name, as pytest's -k: `login or signup`, `not slow`, `(a or b) and not c`.
    #[usage(short = 'k', value_name = "EXPR", value_hint = usage::ValueHint::Other)]
    pub(crate) keyword: Option<String>,
    /// Select tests by marker, as pytest's -m. A marker is a module name: `-m smoke` runs
    /// the tests in `mod smoke`.
    #[usage(short = 'm', value_name = "EXPR", value_hint = usage::ValueHint::Other)]
    pub(crate) marker: Option<String>,
    /// How many tests run at once, as pytest-xdist's -n: a number or `auto`.
    #[usage(short = 'n', long = "threads", value_name = "N|auto", value_hint = usage::ValueHint::Other)]
    pub(crate) threads: Option<String>,
    /// Show what the tests print as they run (pytest's -s).
    #[usage(short = 's', long = "no-capture", aliases = ["nocapture"], bool_value)]
    pub(crate) no_capture: bool,
    /// List the tests that would run and how many (`collected N items`) without running them.
    #[usage(long = "co", visible_aliases = ["collect-only"], bool_value)]
    pub(crate) collect_only: bool,
    /// After the tests, run every example of the packages with `cargo run --example`.
    #[usage(long = "all-scripts", bool_value)]
    pub(crate) all_scripts: bool,
    #[usage(flatten)]
    pub(crate) options: TestOptions,
    /// Arguments for the test harness, after `--` (for example --exact).
    #[usage(double_dash = "required", value_hint = usage::ValueHint::Other)]
    pub(crate) harness_args: Vec<String>,
}

/// Rewrites the `_` spellings of Python's flags (`--no_sandbox`, `--demo_sleep=2`) to the
/// `-` spelling the command line declares. Only a word that spells a row of the option table
/// is changed, and nothing after a `--`.
pub(crate) fn python_spellings(words: Vec<OsString>) -> Vec<OsString> {
    let mut after_dashes = false;
    words
        .into_iter()
        .map(|word| {
            if after_dashes {
                return word;
            }
            let Some(text) = word.to_str() else {
                return word;
            };
            if text == "--" {
                after_dashes = true;
                return word;
            }
            if !text.starts_with("--") || !text.contains('_') {
                return word;
            }
            let (name, value) = match text.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (text, None),
            };
            if !name.contains('_') || find_option(name).is_none() {
                return word;
            }
            let mut fixed = name.replace('_', "-");
            if let Some(value) = value {
                fixed.push('=');
                fixed.push_str(value);
            }
            OsString::from(fixed)
        })
        .collect()
}

fn argument_error(message: impl Into<String>) -> ArgumentError {
    ArgumentError::new("test", message)
}

/// The text of a configuration error without its kind.
fn plain(error: SeleniumBaseError) -> String {
    match error {
        SeleniumBaseError::InvalidConfig(message) => message,
        other => other.to_string(),
    }
}

/// What a planned `sbrs test` does besides running the tests.
#[derive(Debug)]
pub(crate) struct Plan {
    /// The Cargo command of the tests.
    pub(crate) run: TestRun,
    pub(crate) runner: Runner,
    /// List the tests and exit (`--co`).
    pub(crate) list: bool,
    /// An example is run, so the folders to reset are the working directory's.
    pub(crate) example: bool,
    pub(crate) archive_logs: bool,
    pub(crate) archive_downloads: bool,
    /// Run every example after the tests.
    pub(crate) all_scripts: Option<AllScripts>,
}

/// How `--all-scripts` runs the examples.
#[derive(Debug)]
pub(crate) struct AllScripts {
    pub(crate) release: bool,
    pub(crate) features: Vec<String>,
    pub(crate) env: Vec<(String, String)>,
    pub(crate) timeout: Duration,
}

impl TestArgs {
    /// The value of `--settings-file` (or `--settings`), if it was given.
    pub(crate) fn settings_file(&self) -> Option<String> {
        self.options.env().ok().and_then(|pairs| {
            pairs
                .into_iter()
                .rev()
                .find(|(name, _)| *name == "SB_SETTINGS_FILE")
                .map(|(_, value)| value)
        })
    }

    /// Works out what to run. `forwarded` is the environment the options before the command
    /// ask for; `config` is the settings file named by `-c` or `--settings-file`.
    ///
    /// # Errors
    ///
    /// An [`ArgumentError`] for a value or a combination of options the run cannot use.
    pub(crate) fn plan(
        &self,
        settings: &Settings,
        forwarded: Vec<(String, String)>,
        config: Option<&str>,
    ) -> Result<Plan, ArgumentError> {
        let pairs = self.options.env().map_err(argument_error)?;
        let flag = |name: &str| {
            pairs
                .iter()
                .rev()
                .find(|(var, _)| *var == name)
                .map(|(_, value)| value.as_str())
        };
        let on = |name: &str, fallback: bool| flag(name).map_or(fallback, |value| value == "true");

        let mut exported: Vec<(String, String)> = Vec::new();
        for (name, value) in &pairs {
            let value = match *name {
                "SB_ENV" => TestData::parse_env(value).map_err(|e| argument_error(plain(e)))?,
                "SB_VARIABLES" => {
                    TestData::parse_variables(value).map_err(|e| argument_error(plain(e)))?;
                    value.clone()
                }
                "SB_SETTINGS_FILE" => continue,
                _ => value.clone(),
            };
            exported.push(((*name).to_owned(), value));
        }

        let runner = match flag("SB_TEST_RUNNER").or(settings.test.runner.as_deref()) {
            Some(text) => text.parse::<Runner>().map_err(argument_error)?,
            None => Runner::Cargo,
        };
        let debug = [
            ("SB_PDB", "--pdb", settings.test.pdb),
            ("SB_TRACE", "--trace", settings.test.trace),
            ("SB_FINAL_DEBUG", "--final-debug", settings.test.final_debug),
        ]
        .into_iter()
        .find(|(var, _, fallback)| on(var, *fallback));
        if let (Some((_, name, _)), Runner::Nextest) = (debug, runner) {
            return Err(argument_error(format!(
                "{name} needs the cargo runner: nextest gives tests no standard input"
            )));
        }

        let threads = match self.threads.as_deref() {
            Some("auto") => None,
            Some(text) => Some(
                text.trim()
                    .parse::<usize>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| {
                        argument_error(format!(
                            "-n takes a number of threads or `auto`, not '{text}'"
                        ))
                    })?,
            ),
            None => settings.threads,
        };

        if self.filter.is_some() && self.keyword.is_some() {
            return Err(argument_error("give either FILTER or -k, not both"));
        }
        let name_filter =
            run_filter::translate(self.keyword.as_deref(), self.marker.as_deref(), runner)
                .map_err(|error| argument_error(error.to_string()))?;

        let target = match (&self.test, &self.example) {
            (_, Some(name)) => TestTarget::Example(name.clone()),
            (Some(name), None) => TestTarget::Test(name.clone()),
            (None, None) => TestTarget::All,
        };
        let example = matches!(target, TestTarget::Example(_));
        if example && self.collect_only {
            return Err(argument_error("--co lists tests, and an example has none"));
        }
        if self.all_scripts && !matches!(target, TestTarget::All) {
            return Err(argument_error(
                "--all-scripts runs after all the tests: it cannot be combined with --test or --example",
            ));
        }

        let mut run = TestRun::new()
            .target(target)
            .release(self.release)
            .features(self.features.clone())
            .runner(runner)
            .list(self.collect_only)
            .nocapture(self.no_capture || debug.is_some())
            .name_filter(name_filter)
            .harness_args(self.harness_args.clone());
        if let Some(filter) = &self.filter {
            run = run.filter(filter);
        }
        // A prompt needs the terminal to itself: one test at a time, and its output shown.
        let recording = on("SB_RECORDER", settings.test.recorder)
            || on("SB_REC_PRINT", settings.test.rec_print)
            || on("SB_REC_BEHAVE", settings.test.rec_behave)
            || on("SB_REC_SB_MGR", settings.test.rec_sb_mgr);
        if debug.is_some() {
            run = run.threads(1);
        } else if recording {
            // The Recorder's breakpoint asks a person, so the tests take turns.
            if let Some(warning) = crate::recorder_flags::multi_process_warning(threads) {
                eprint!("{warning}");
            }
            run = run.threads(1);
        } else if let Some(threads) = threads {
            run = run.threads(threads);
        }
        for (name, value) in &forwarded {
            run = run.env(name, value);
        }
        for (name, value) in &exported {
            run = run.env(name, value);
        }
        if let Some(count) = self.threads.as_deref().filter(|text| *text != "auto") {
            run = run.env("SB_THREADS", count.trim());
        }
        if let Some(file) = config.or(flag("SB_SETTINGS_FILE")) {
            // The tests run in their package's folder, so a relative path would move.
            let path = std::path::absolute(file).unwrap_or_else(|_| PathBuf::from(file));
            run = run.env("SB_SETTINGS_FILE", path.display().to_string());
        }

        let time_limit = flag("SB_TIME_LIMIT")
            .and_then(|text| text.trim().parse::<f64>().ok())
            .or(settings.test.time_limit)
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0);
        let all_scripts = self.all_scripts.then(|| {
            let mut env = forwarded.clone();
            env.extend(exported.clone());
            AllScripts {
                release: self.release,
                features: self.features.clone(),
                env,
                timeout: time_limit.map_or(all_scripts::DEFAULT_TIMEOUT, Duration::from_secs_f64),
            }
        });
        Ok(Plan {
            run,
            runner,
            list: self.collect_only,
            example,
            archive_logs: on("SB_ARCHIVE_LOGS", settings.test.archive_logs),
            archive_downloads: on("SB_ARCHIVE_DOWNLOADS", settings.test.archive_downloads),
            all_scripts,
        })
    }
}

impl Plan {
    /// Does what the plan says and returns the exit status for `sbrs`.
    ///
    /// # Errors
    ///
    /// Returns the error of the folder reset or of starting `cargo`.
    pub(crate) fn execute(&self) -> Result<i32, Box<dyn std::error::Error>> {
        let cwd = std::env::current_dir()?;
        let dirs = if self.example {
            vec![cwd]
        } else {
            run_logs::package_dirs(&cwd)
        };
        if self.list {
            return self.list_tests();
        }
        self.reset_folders(&dirs)?;
        let status = self.run.status()?;
        let mut code = status.code().unwrap_or(1);
        if let Some(all) = &self.all_scripts {
            let failed = Self::run_examples(all, &dirs)?;
            if failed && code == 0 {
                code = 1;
            }
        }
        Ok(code)
    }

    /// Empties `latest_logs/` and `downloaded_files/` of each package, or moves them to
    /// the archives, as Python does at the start of a run.
    fn reset_folders(&self, dirs: &[PathBuf]) -> std::io::Result<()> {
        let stamp = run_logs::unix_stamp();
        for dir in dirs {
            if let run_logs::Reset::Archived(to) =
                run_logs::reset_logs(dir, self.archive_logs, stamp)?
            {
                eprintln!("Archived the logs of {} in {}", dir.display(), to.display());
            }
            if let run_logs::Reset::Archived(to) =
                run_logs::reset_downloads(dir, self.archive_downloads, stamp)?
            {
                eprintln!(
                    "Archived the downloads of {} in {}",
                    dir.display(),
                    to.display()
                );
            }
        }
        Ok(())
    }

    /// Runs the examples and prints a line for each; whether any failed.
    fn run_examples(
        all: &AllScripts,
        dirs: &[PathBuf],
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let examples = all_scripts::find_examples(dirs, &all.features)?;
        let mut failed = false;
        for example in &examples {
            let command =
                all_scripts::example_command(example, all.release, &all.features, &all.env);
            let ended = all_scripts::run_with_limit(command, all.timeout);
            println!("{}", all_scripts::report_line(&example.name, &ended));
            failed |= !ended.is_ok();
        }
        println!("{} examples run", examples.len());
        Ok(failed)
    }

    /// `--co`: the names of the tests, then `collected N items`. Nothing runs.
    fn list_tests(&self) -> Result<i32, Box<dyn std::error::Error>> {
        let mut command = self.run.command();
        command.stdout(Stdio::piped()).stderr(Stdio::inherit());
        let output = command.output()?;
        if !output.status.success() {
            return Ok(output.status.code().unwrap_or(1));
        }
        let names = run::list_names(&String::from_utf8_lossy(&output.stdout), self.runner);
        for name in &names {
            println!("{name}");
        }
        println!("\ncollected {} items", names.len());
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use seleniumbase_rs::config::test_options::{OptionKind, TEST_OPTIONS};

    use super::*;
    use crate::{Cli, Commands};

    fn parse(words: &[&str]) -> TestArgs {
        let mut all = vec!["test"];
        all.extend_from_slice(words);
        let words: Vec<&std::ffi::OsStr> = all.iter().map(std::ffi::OsStr::new).collect();
        match Cli::parse_from(&words)
            .unwrap_or_else(|e| panic!("{all:?}: {e:?}"))
            .command
        {
            Commands::Test(args) => args,
            other => panic!("{all:?} parsed as {other:?}"),
        }
    }

    fn env(words: &[&str]) -> Vec<(&'static str, String)> {
        parse(words).options.env().expect("no conflict")
    }

    fn plan(words: &[&str]) -> Result<Plan, ArgumentError> {
        parse(words).plan(&Settings::default(), Vec::new(), None)
    }

    fn args_of(plan: &Plan) -> Vec<String> {
        plan.run
            .command()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn pair(name: &'static str, value: &str) -> (&'static str, String) {
        (name, value.to_owned())
    }

    #[test]
    fn a_switch_is_true_bare_and_false_when_told_so() {
        assert_eq!(env(&["--dark-mode"]), [pair("SB_DARK_MODE", "true")]);
        assert_eq!(env(&["--dark-mode=false"]), [pair("SB_DARK_MODE", "false")]);
        assert_eq!(env(&["--dark"]), [pair("SB_DARK_MODE", "true")]);
        assert!(env(&[]).is_empty());
    }

    #[test]
    fn a_value_option_gives_its_text() {
        assert_eq!(
            env(&["--demo-sleep", "2", "--data=abc", "--var1", "x"]),
            [
                pair("SB_DEMO_SLEEP", "2"),
                pair("SB_DATA", "abc"),
                pair("SB_VAR1", "x")
            ]
        );
    }

    #[test]
    fn a_shorthand_writes_the_value_of_its_variable() {
        assert_eq!(
            env(&["--firefox", "--headed", "--uc"]),
            [
                pair("SB_BROWSER", "firefox"),
                pair("SB_HEADLESS", "false"),
                pair("SB_MODE", "uc")
            ]
        );
        assert_eq!(env(&["--gui"]), [pair("SB_HEADLESS", "false")]);
        assert!(env(&["--firefox=false"]).is_empty());
    }

    #[test]
    fn two_shorthands_for_one_variable_are_refused() {
        let error = parse(&["--firefox", "--chrome"]).options.env().unwrap_err();
        assert_eq!(
            error,
            "--chrome and --firefox both choose SB_BROWSER: give only one of them"
        );
    }

    #[test]
    fn options_that_do_nothing_are_accepted() {
        assert!(env(&["--fast", "--with-selenium", "--is-pytest"]).is_empty());
    }

    #[test]
    fn every_row_of_the_table_is_a_flag_in_each_of_its_spellings() {
        let mut checked = 0;
        for row in TEST_OPTIONS.iter().filter(|row| row.kind.is_flag()) {
            if row.long == "all-scripts" {
                continue;
            }
            for spelling in row.flags() {
                let word = match row.kind {
                    OptionKind::Value(_) | OptionKind::Custom(_) => {
                        let value = match row.long {
                            "runner" => "cargo",
                            "mode" => "cdp",
                            "page-load-strategy" => "eager",
                            "browser" => "chrome",
                            _ => "x",
                        };
                        format!("--{spelling}={value}")
                    }
                    _ => format!("--{spelling}"),
                };
                let args = parse(&[word.as_str()]);
                let pairs = args.options.env().expect("one flag cannot conflict");
                match row.kind {
                    OptionKind::NoOp(_) | OptionKind::Cli(_) => assert!(pairs.is_empty()),
                    _ => assert_eq!(
                        pairs.first().map(|(name, _)| *name),
                        Some(row.env),
                        "{word}"
                    ),
                }
                checked += 1;
            }
        }
        assert!(checked > 150, "only {checked} spellings were checked");
    }

    #[test]
    fn the_list_of_field_names_is_long_enough_for_the_table() {
        // The macro stops with "no rules expected" when the list runs out; this fails first
        // when a row added to the table brings it near the end.
        let flags = TEST_OPTIONS
            .iter()
            .filter(|row| {
                !matches!(row.kind, OptionKind::Env(_) | OptionKind::NotAFlag(_))
                    && row.long != "all-scripts"
            })
            .count();
        assert!(
            flags + 10 <= 180,
            "{flags} flags: lengthen the list of names in test_options!"
        );
    }

    #[test]
    fn python_underscore_spellings_are_rewritten_before_the_parse() {
        let words = |list: &[&str]| -> Vec<OsString> { list.iter().map(OsString::from).collect() };
        assert_eq!(
            python_spellings(words(&[
                "--no_sandbox",
                "--demo_sleep=2",
                "--all_scripts",
                "--nosuch_flag",
                "--",
                "--no_sandbox"
            ])),
            words(&[
                "--no-sandbox",
                "--demo-sleep=2",
                "--all-scripts",
                "--nosuch_flag",
                "--",
                "--no_sandbox"
            ])
        );
        let args = parse(&["--no-sandbox"]);
        assert_eq!(args.options.env().unwrap(), [pair("SB_NO_SANDBOX", "true")]);
    }

    #[test]
    fn the_cargo_command_follows_the_options() {
        let plan = plan(&["login", "--release", "--features", "a,b", "-s", "-n", "3"]).unwrap();
        assert_eq!(
            args_of(&plan),
            [
                "test",
                "--release",
                "--features",
                "a,b",
                "login",
                "--",
                "--test-threads",
                "3",
                "--nocapture"
            ]
        );
        let plan = self::plan(&["-n", "auto"]).unwrap();
        assert_eq!(args_of(&plan), ["test"]);
    }

    #[test]
    fn k_and_m_become_filters_and_skips() {
        let p = plan(&["-k", "login or signup"]).unwrap();
        assert_eq!(args_of(&p), ["test", "login", "signup"]);
        let p = plan(&["-k", "not admin"]).unwrap();
        assert_eq!(args_of(&p), ["test", "--", "--skip", "admin"]);
        let p = plan(&["-m", "smoke"]).unwrap();
        assert_eq!(args_of(&p), ["test", "smoke::"]);
        let p = plan(&["-k", "a and b", "--runner", "nextest"]).unwrap();
        assert_eq!(
            args_of(&p),
            ["nextest", "run", "-E", "test(~a) and test(~b)"]
        );
        let error = plan(&["-k", "a and b"]).unwrap_err();
        assert!(
            error.to_string().contains("needs the nextest runner"),
            "{error}"
        );
        let error = plan(&["x", "-k", "y"]).unwrap_err();
        assert_eq!(error.to_string(), "give either FILTER or -k, not both");
    }

    #[test]
    fn collecting_lists_the_tests() {
        let p = plan(&["--co"]).unwrap();
        assert!(p.list);
        assert_eq!(args_of(&p), ["test", "--", "--list"]);
        assert!(plan(&["--collect-only"]).unwrap().list);
        let error = plan(&["--co", "--example", "x"]).unwrap_err();
        assert!(error.to_string().contains("an example has none"), "{error}");
    }

    #[test]
    fn a_debug_mode_runs_one_test_at_a_time_with_its_output_shown() {
        for flag in ["--pdb", "--trace", "--final-debug", "--ftrace"] {
            let p = plan(&[flag, "-n", "4"]).unwrap();
            assert_eq!(
                args_of(&p),
                ["test", "--", "--test-threads", "1", "--nocapture"],
                "{flag}"
            );
        }
        let error = plan(&["--pdb", "--runner", "nextest"]).unwrap_err();
        assert_eq!(
            error.to_string(),
            "--pdb needs the cargo runner: nextest gives tests no standard input"
        );
        assert!(plan(&["--trace", "--runner=nextest"]).is_err());
    }

    #[test]
    fn the_recorder_runs_the_tests_one_at_a_time() {
        let one = ["test", "--", "--test-threads", "1"];
        for flag in ["--rec", "--rec-print", "--rec-behave", "--rec-sb-mgr"] {
            assert_eq!(args_of(&plan(&[flag]).unwrap()), one, "{flag}");
        }
        // A thread count does not change that.
        assert_eq!(args_of(&plan(&["--rec", "-n", "4"]).unwrap()), one);
        // Nor does the recorder of the settings.
        let mut settings = Settings::default();
        settings.test.recorder = true;
        let p = parse(&[]).plan(&settings, Vec::new(), None).unwrap();
        assert_eq!(args_of(&p), one);
        // Without the recorder the count is used.
        assert_eq!(
            args_of(&plan(&["-n", "4"]).unwrap()),
            ["test", "--", "--test-threads", "4"]
        );
    }

    #[test]
    fn a_debug_mode_of_the_settings_counts_too() {
        let mut settings = Settings::default();
        settings.test.final_debug = true;
        let p = parse(&[]).plan(&settings, Vec::new(), None).unwrap();
        assert_eq!(
            args_of(&p),
            ["test", "--", "--test-threads", "1", "--nocapture"]
        );
        let p = parse(&["--final-debug=false"])
            .plan(&settings, Vec::new(), None)
            .unwrap();
        assert_eq!(args_of(&p), ["test"]);
        settings.test.runner = Some("nextest".into());
        assert!(parse(&[]).plan(&settings, Vec::new(), None).is_err());
    }

    #[test]
    fn a_bad_value_is_refused_before_cargo_runs() {
        let error = plan(&["--env", "nosuch"]).unwrap_err();
        let text = error.to_string();
        assert!(
            text.starts_with(
                "invalid value 'nosuch' for --env (SB_ENV): choose from qa, rc, staging"
            ),
            "{text}"
        );
        assert!(text.ends_with("old, uat"), "{text}");
        assert!(plan(&["--variables", "[1]"]).is_err());
        assert!(plan(&["--variables", "{'k': 1}"]).is_ok());
        assert!(plan(&["--runner", "cargo"]).is_ok());
        assert!(plan(&["-n", "0"]).is_err());
        assert!(plan(&["-n", "many"]).is_err());
        assert!(plan(&["--all-scripts", "--test", "x"]).is_err());
    }

    #[test]
    fn the_environment_has_the_forwarded_options_first_and_the_flags_after() {
        let p = parse(&["--env=QA", "-n", "2", "--variables={'k': 1}"])
            .plan(
                &Settings::default(),
                vec![("SB_HEADLESS".to_owned(), "true".to_owned())],
                Some("conf.toml"),
            )
            .unwrap();
        let envs: Vec<(String, String)> = p
            .run
            .command()
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
            })
            .collect();
        let get = |name: &str| envs.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
        assert_eq!(get("SB_HEADLESS").as_deref(), Some("true"));
        assert_eq!(get("SB_ENV").as_deref(), Some("qa"));
        assert_eq!(get("SB_THREADS").as_deref(), Some("2"));
        assert_eq!(get("SB_VARIABLES").as_deref(), Some("{'k': 1}"));
        let file = get("SB_SETTINGS_FILE").expect("exported");
        assert!(std::path::Path::new(&file).is_absolute() && file.ends_with("conf.toml"));
    }

    #[test]
    fn the_settings_file_flag_is_found() {
        assert_eq!(
            parse(&["--settings-file", "a.toml"])
                .settings_file()
                .as_deref(),
            Some("a.toml")
        );
        assert_eq!(
            parse(&["--settings=b.toml"]).settings_file().as_deref(),
            Some("b.toml")
        );
        assert_eq!(parse(&[]).settings_file(), None);
    }

    #[test]
    fn archiving_comes_from_the_flags_or_the_settings() {
        let p = plan(&["--archive-logs"]).unwrap();
        assert!(p.archive_logs && !p.archive_downloads);
        let p = plan(&["--archive-downloads"]).unwrap_or_else(|_| unreachable!());
        assert!(p.archive_downloads);
        let mut settings = Settings::default();
        settings.test.archive_logs = true;
        let p = parse(&["--archive-logs=false"])
            .plan(&settings, Vec::new(), None)
            .unwrap();
        assert!(!p.archive_logs);
    }
}
