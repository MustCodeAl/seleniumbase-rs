//! The browser options as flags of `sbrs` itself: `sbrs --dark --metrics 360,640,2 open URL`.
//!
//! The flags are the rows of the library's option table
//! (`seleniumbase_rs::config::test_options`), the same table `sbrs test` builds its
//! flags from, so a row added there is a flag here with no other change. Only the rows that
//! describe the browser and the page are made flags here, and each one is `global`, so it
//! is accepted before and after the command. The few options the root declares by hand
//! (`--browser`, `--headless`, `--headed`, `--proxy`, ...) are left out of this list and
//! keep their own fields in [`SettingsOverrides`](seleniumbase_rs::config::settings::SettingsOverrides).
//!
//! A flag the command line gives becomes the `SB_*` variable of its row with the text of
//! its value ([`BrowserFlags::options`]); the library reads it as it reads the
//! environment (`Settings::apply_options`), so a flag, a variable and a key of the settings
//! file are checked by the same code. A switch is `--flag`, which is true, or
//! `--flag=false`, so a flag the settings file turned on can be turned off.

use seleniumbase_rs::config::browser_options::{render_options, render_options_markdown};
use seleniumbase_rs::config::env_vars::{render_variables, render_variables_markdown};

use crate::test_args::shorthand_conflict;

/// The options the root `Cli` declares by hand, one rule for each: they are not made flags
/// again from the table. Everything else passes to `$keep`.
macro_rules! declared_at_the_root {
    ("browser", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("headless", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("headed", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("uc", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    // (`--mode` is not declared by hand: it is made from its row.)
    ("locale", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("user-agent", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("ad-block", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("reuse-session", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("reuse-class-session", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("mobile", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("proxy", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("proxy-pac-url", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("user-data-dir", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("extension-dir", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("esc-end", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("rec-sb-cdp", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("fingerprint", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("start-page", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ("settings-file", $skip:tt, $keep:tt) => { declared_at_the_root!(@go $skip); };
    ($other:tt, $skip:tt, $keep:tt) => { declared_at_the_root!(@go $keep); };
    (@go [$($tokens:tt)*]) => { $($tokens)* };
}

/// Builds `BrowserFlags`, one global flag for each browser row of the option table, and
/// `BrowserFlags::options`, which turns the flags that were given into `SB_*` pairs.
///
/// The same technique as `test_options!` in `test_args.rs`: a macro cannot make up a field
/// name, so each flag takes the next name of the list at the first call; the list must be as
/// long as the table has flags (a test checks it).
macro_rules! browser_flags {
    ($($row:tt),+ $(,)?) => {
        browser_flags!(@next [this out sh]
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
            ]
            [] [] $($row)+);
    };

    // No row is left: write the struct and the method.
    (@next [$this:ident $out:ident $sh:ident] $ids:tt [$($fields:tt)*] [$($pushes:tt)*]) => {
        // The flags that come from the option table. (A plain comment: a doc comment on a
        // struct that derives `Args` would be help text.)
        #[derive(Debug, Default, usage::Args)]
        #[usage(args_override_self = false)]
        pub(crate) struct BrowserFlags {
            $($fields)*
        }

        impl BrowserFlags {
            /// The `SB_*` variables of the flags that were given, in the order of the table.
            /// A switch gives `true` or `false`; a flag that stands for a value of a
            /// variable (`--firefox`) gives that value.
            ///
            /// # Errors
            ///
            /// Returns a message when two such flags choose different values of one
            /// variable (`--firefox --chrome`).
            pub(crate) fn options(&self) -> Result<Vec<(&'static str, String)>, String> {
                let $this = self;
                let mut $out: Vec<(&'static str, String)> = Vec::new();
                let mut $sh: Vec<(&'static str, &'static str, &'static str)> = Vec::new();
                $($pushes)*
                shorthand_conflict(&$sh)?;
                Ok($out)
            }
        }
    };

    // The options of the test run and the rows with no variable are not browser options.
    (@next $n:tt $i:tt $f:tt $p:tt (test.$($r:tt)*) $($more:tt)*) => {
        browser_flags!(@next $n $i $f $p $($more)*);
    };
    (@next $n:tt $i:tt $f:tt $p:tt (none, $($r:tt)*) $($more:tt)*) => {
        browser_flags!(@next $n $i $f $p $($more)*);
    };
    // The options the root declares by hand.
    (@next $n:tt $i:tt $f:tt $p:tt ($($path:ident).+, $long:tt, $($r:tt)*) $($more:tt)*) => {
        declared_at_the_root!($long,
            [browser_flags!(@next $n $i $f $p $($more)*);],
            [browser_flags!(@alias $n $i $f $p ($long, $($r)*) $($more)*);]);
    };

    // The aliases of a row become `aliases = [..],`, or nothing when it has none: the derive
    // refuses an empty list.
    (@alias $n:tt $i:tt $f:tt $p:tt ($long:tt, [], $($r:tt)*) $($more:tt)*) => {
        browser_flags!(@kind $n $i $f $p ($long, [], $($r)*) $($more)*);
    };
    (@alias $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($a:tt)+], $($r:tt)*) $($more:tt)*) => {
        browser_flags!(@kind $n $i $f $p ($long, [aliases = [$($a)+],], $($r)*) $($more)*);
    };

    // Rows that are no flag of the root: a variable with no flag, an option that is ignored
    // and one of the test run.
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [env($t:ident)], $help:tt) $($more:tt)*) => {
        browser_flags!(@next $n $i $f $p $($more)*);
    };
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [noop($why:tt)], $help:tt) $($more:tt)*) => {
        browser_flags!(@next $n $i $f $p $($more)*);
    };
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [cli($why:tt)], $help:tt) $($more:tt)*) => {
        browser_flags!(@next $n $i $f $p $($more)*);
    };
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [na($why:tt)], $help:tt) $($more:tt)*) => {
        browser_flags!(@next $n $i $f $p $($more)*);
    };
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [custom($t:ident, $fun:path)], $help:tt) $($more:tt)*) => {
        browser_flags!(@next $n $i $f $p $($more)*);
    };

    // A switch: `--flag`, `--flag=true` or `--flag=false`.
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [switch], $help:tt) $($more:tt)*) => {
        browser_flags!(@emit $n $i $f $p
            [#[usage(long = $long, $($al)* global, default_missing = "true", require_equals,
                choices("true", "false"), help = $help)]]
            [switch $env] $($more)*);
    };
    // A switch that stands for a value of a variable: `--firefox` is `SB_BROWSER=firefox`.
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [sets($v:tt)], $help:tt) $($more:tt)*) => {
        browser_flags!(@emit $n $i $f $p
            [#[usage(long = $long, $($al)* global, default_missing = "true", require_equals,
                choices("true", "false"), help = $help)]]
            [sets $env $v $long] $($more)*);
    };

    // Options with a value, by what the value is.
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Path)], $help:tt) $($more:tt)*) => {
        browser_flags!(@emit $n $i $f $p
            [#[usage(long = $long, $($al)* global, value_name = "PATH",
                value_hint = usage::ValueHint::AnyPath, help = $help)]]
            [value $env] $($more)*);
    };
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Url)], $help:tt) $($more:tt)*) => {
        browser_flags!(@emit $n $i $f $p
            [#[usage(long = $long, $($al)* global, value_name = "URL",
                value_hint = usage::ValueHint::Url, help = $help)]]
            [value $env] $($more)*);
    };
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Strategy)], $help:tt) $($more:tt)*) => {
        browser_flags!(@emit $n $i $f $p
            [#[usage(long = $long, $($al)* global, value_name = "STRATEGY",
                choices("normal", "eager", "none"), help = $help)]]
            [value $env] $($more)*);
    };
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Seconds)], $help:tt) $($more:tt)*) => {
        browser_flags!(@emit $n $i $f $p
            [#[usage(long = $long, $($al)* global, value_name = "SECONDS",
                value_hint = usage::ValueHint::Other, help = $help)]]
            [value $env] $($more)*);
    };
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value(Count)], $help:tt) $($more:tt)*) => {
        browser_flags!(@emit $n $i $f $p
            [#[usage(long = $long, $($al)* global, value_name = "COUNT",
                value_hint = usage::ValueHint::Other, help = $help)]]
            [value $env] $($more)*);
    };
    (@kind $n:tt $i:tt $f:tt $p:tt ($long:tt, [$($al:tt)*], $sb:tt, $env:tt, [value($t:ident)], $help:tt) $($more:tt)*) => {
        browser_flags!(@emit $n $i $f $p
            [#[usage(long = $long, $($al)* global, value_name = "VALUE",
                value_hint = usage::ValueHint::Other, help = $help)]]
            [value $env] $($more)*);
    };

    // Takes the next field name and writes the field and the code that reads it.
    (@emit [$this:ident $out:ident $sh:ident] [$id:ident $($ids:ident)*] [$($f:tt)*] [$($p:tt)*]
        [$($attr:tt)*] [switch $env:tt] $($more:tt)*) => {
        browser_flags!(@next [$this $out $sh] [$($ids)*]
            [$($f)* $($attr)* $id: Option<bool>,]
            [$($p)* if let Some(on) = $this.$id { $out.push(($env, on.to_string())); }]
            $($more)*);
    };
    (@emit [$this:ident $out:ident $sh:ident] [$id:ident $($ids:ident)*] [$($f:tt)*] [$($p:tt)*]
        [$($attr:tt)*] [sets $env:tt $v:tt $long:tt] $($more:tt)*) => {
        browser_flags!(@next [$this $out $sh] [$($ids)*]
            [$($f)* $($attr)* $id: Option<bool>,]
            [$($p)* if $this.$id == Some(true) {
                $out.push(($env, ($v).to_owned()));
                $sh.push(($env, $v, $long));
            }]
            $($more)*);
    };
    (@emit [$this:ident $out:ident $sh:ident] [$id:ident $($ids:ident)*] [$($f:tt)*] [$($p:tt)*]
        [$($attr:tt)*] [value $env:tt] $($more:tt)*) => {
        browser_flags!(@next [$this $out $sh] [$($ids)*]
            [$($f)* $($attr)* $id: Option<String>,]
            [$($p)* if let Some(text) = &$this.$id { $out.push(($env, text.clone())); }]
            $($more)*);
    };
}

seleniumbase_rs::sb_test_options!(browser_flags);

// The options of `sbrs options`.
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub(crate) struct OptionsArgs {
    /// Print a Markdown table, for the documentation.
    #[usage(long, bool_value)]
    pub(crate) markdown: bool,
    /// Also list the options of the test run (`sbrs test`).
    #[usage(long, bool_value)]
    pub(crate) all: bool,
    /// List the environment variables `sbrs` and `sbrs-mcp` read instead of the options.
    ///
    /// One line for each variable: who reads it, the flag that sets the same thing, what it
    /// is when it is not set, and what it sets. `--markdown` prints the table for the
    /// documentation. The table is made from the option table and the list of the other
    /// variables, so it names every variable the command lines read.
    #[usage(long, bool_value)]
    pub(crate) env: bool,
}

/// The text `sbrs options` prints.
pub(crate) fn options_listing(args: &OptionsArgs) -> String {
    match (args.env, args.markdown) {
        (true, true) => render_variables_markdown(),
        (true, false) => render_variables(),
        (false, true) => render_options_markdown(args.all),
        (false, false) => render_options(args.all),
    }
}

#[cfg(test)]
mod tests {
    use seleniumbase_rs::config::browser_options::listed_options;
    use seleniumbase_rs::config::test_options::{OptionKind, TEST_OPTIONS};

    use super::*;
    use std::collections::BTreeSet;

    use seleniumbase_rs::config::env_vars::{
        flag_variable, is_known, EXTRA_VARIABLES, TABLE_FLAGS,
    };

    /// A flag of the usage spec: the program and command it belongs to, its long spellings
    /// and its help.
    struct SpecFlag {
        path: String,
        longs: Vec<String>,
        help: String,
    }

    /// Every flag of every command of the usage spec of `sbrs`.
    fn spec_flags() -> Vec<SpecFlag> {
        fn walk(path: &str, command: &usage_lib::SpecCommand, out: &mut Vec<SpecFlag>) {
            for flag in &command.flags {
                out.push(SpecFlag {
                    path: path.to_owned(),
                    longs: flag.long.clone(),
                    help: flag.help.clone().unwrap_or_default(),
                });
            }
            for sub in command.subcommands.values() {
                walk(&format!("{path} {}", sub.name), sub, out);
            }
        }
        let spec: usage_lib::Spec = crate::Cli::to_kdl().parse().expect("the usage spec parses");
        let mut out = Vec::new();
        walk(&spec.bin, &spec.cmd, &mut out);
        out
    }

    fn is_builtin(flag: &SpecFlag) -> bool {
        flag.longs
            .iter()
            .any(|long| long == "help" || long == "version")
    }

    /// The variable of a flag is named in its help, so `sbrs --help` and the reference page
    /// say where to set it once for a machine or a CI job.
    #[test]
    fn a_flag_that_stands_for_a_variable_names_it_in_its_help() {
        let mut named = 0;
        for flag in spec_flags().iter().filter(|flag| !is_builtin(flag)) {
            let variables: BTreeSet<&str> = flag
                .longs
                .iter()
                .filter_map(|long| flag_variable(&flag.path, long))
                .collect();
            assert!(
                variables.len() <= 1,
                "`{} --{}` stands for {variables:?}",
                flag.path,
                flag.longs.join("/--")
            );
            for variable in variables {
                assert!(
                    flag.help.contains(variable),
                    "`{} --{}` is set by {variable}, but its help does not say so: {}",
                    flag.path,
                    flag.longs.join("/--"),
                    flag.help
                );
                named += 1;
            }
        }
        assert!(named > 150, "only {named} flags have a variable");
    }

    /// Every global flag of `sbrs` is a setting, except the options of a plugin, which are a
    /// table of the settings file.
    #[test]
    fn every_global_flag_has_a_variable_but_plugin_opt() {
        for flag in spec_flags()
            .iter()
            .filter(|flag| flag.path == "sbrs" && !is_builtin(flag))
        {
            if flag.longs == ["plugin-opt"] {
                continue;
            }
            assert!(
                flag.longs
                    .iter()
                    .any(|long| flag_variable("sbrs", long).is_some()),
                "--{} has no variable",
                flag.longs.join("/--")
            );
        }
    }

    /// A variable that a help text names is one the programs read: a stale name would send a
    /// person to a variable that does nothing.
    #[test]
    fn a_variable_named_in_a_help_text_is_one_the_programs_read() {
        for flag in spec_flags() {
            let words = flag
                .help
                .split(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'));
            for word in words.filter(|word| word.starts_with("SB_") && word.len() > 3) {
                assert!(
                    is_known(word),
                    "`{} --{}` names {word}, which is not in the list of variables",
                    flag.path,
                    flag.longs.join("/--")
                );
            }
        }
    }

    /// The registry points at flags that exist.
    #[test]
    fn the_flags_of_the_registry_are_flags_of_sbrs() {
        let flags = spec_flags();
        let exists = |spelled: &str| {
            let (path, long) = spelled.rsplit_once(' ').expect("a command and a flag");
            let long = long.trim_start_matches('-');
            flags
                .iter()
                .any(|flag| flag.path == path && flag.longs.iter().any(|known| known == long))
        };
        for extra in EXTRA_VARIABLES {
            for spelled in extra
                .flags
                .iter()
                .filter(|spelled| spelled.starts_with("sbrs "))
            {
                // `sbrs report history` is built with the feature `turso` only.
                if spelled.contains("report history") && !cfg!(feature = "turso") {
                    continue;
                }
                assert!(exists(spelled), "{spelled} ({}) is no flag", extra.name);
            }
        }
        for (spelled, variable) in TABLE_FLAGS {
            assert!(exists(spelled), "{spelled} ({variable}) is no flag");
        }
    }

    #[test]
    fn options_env_lists_the_variables_and_not_the_options() {
        let text = options_listing(&OptionsArgs {
            markdown: false,
            all: false,
            env: true,
        });
        assert!(text.starts_with("VARIABLE"), "{text}");
        for name in [
            "SB_BROWSER",
            "SB_PROXY_LIST",
            "SB_MCP_LISTEN",
            "SB_LOG_LEVEL",
        ] {
            assert!(text.contains(name), "{name} is missing:\n{text}");
        }
        let markdown = options_listing(&OptionsArgs {
            markdown: true,
            all: false,
            env: true,
        });
        assert!(markdown.starts_with("| Variable |"), "{markdown}");
    }

    /// The rows the root declares by hand, by canonical flag.
    const BY_HAND: [&str; 19] = [
        "browser",
        "headless",
        "headed",
        "uc",
        "locale",
        "user-agent",
        "ad-block",
        "reuse-session",
        "reuse-class-session",
        "mobile",
        "proxy",
        "proxy-pac-url",
        "user-data-dir",
        "extension-dir",
        "esc-end",
        "rec-sb-cdp",
        "fingerprint",
        "start-page",
        "settings-file",
    ];

    #[test]
    fn no_flag_is_given_no_variable_is_exported() {
        assert_eq!(BrowserFlags::default().options().unwrap(), []);
    }

    #[test]
    fn the_table_has_fewer_browser_flags_than_the_macro_has_field_names() {
        let flags = BrowserFlags::default();
        assert!(flags.options().is_ok());
        // One field per flag; a row left out would not be a field. The count of the fields is
        // the count of rows the table has for the browser, less the ones declared by hand.
        let expected = TEST_OPTIONS
            .iter()
            .filter(|row| !row.field.starts_with("test.") && row.has_field())
            .filter(|row| {
                matches!(
                    row.kind,
                    OptionKind::Switch | OptionKind::Value(_) | OptionKind::Sets(_)
                )
            })
            .filter(|row| !BY_HAND.contains(&row.long))
            .count();
        assert!(expected > 60, "{expected}");
        assert!(
            expected <= 108,
            "the macro has 108 field names, the table needs {expected}"
        );
    }

    #[test]
    fn the_listing_has_a_line_for_each_option() {
        let text = options_listing(&OptionsArgs {
            markdown: false,
            all: false,
            env: false,
        });
        assert_eq!(text.lines().count(), listed_options(false).count() + 1);
        let markdown = options_listing(&OptionsArgs {
            markdown: true,
            all: true,
            env: false,
        });
        assert!(markdown.starts_with("| Flag |"));
    }
}
