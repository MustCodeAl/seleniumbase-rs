//! `sbrs behave-options`: how Python SeleniumBase's behave options map to
//! `sbrs test`.
//!
//! Python's `sbase behave-options` lists the `-D` user-data names of the
//! `behave` runner. Gherkin scenarios here are `rstest-bdd` tests run by
//! `cargo test`, so the same names are flags of `sbrs test`, and the behave
//! flags that select or stop a run have a `sbrs test` form.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::scripts::behave_options::render;
//!
//! let text = render();
//! assert!(text.contains("-D headless"));
//! assert!(text.contains("sbrs test --co"));
//! ```

/// The names Python's `-D name[=value]` takes, each one a flag of
/// `sbrs test` (`-D headless` is `sbrs test --headless`).
pub const USERDATA: [&str; 31] = [
    "browser",
    "headless",
    "demo",
    "slow",
    "rs",
    "crumbs",
    "maximize",
    "dashboard",
    "incognito",
    "guest",
    "dark",
    "uc",
    "pdb",
    "recorder",
    "rec-print",
    "save-screenshot",
    "archive-logs",
    "check-js",
    "start-page",
    "agent",
    "mobile",
    "metrics",
    "ad-block",
    "settings-file",
    "env",
    "data",
    "disable-csp",
    "remote-debug",
    "server",
    "port",
    "proxy",
];

/// The behave options that are not `-D` names, with what replaces each.
pub const OTHER_OPTIONS: [(&str, &str); 4] = [
    ("--dry-run, -d", "sbrs test --co"),
    (
        "--stop",
        "sbrs test --runner nextest (nextest stops at the first failure by default; \
         cargo stops after the first failing test binary)",
    ),
    (
        "-q, --no-snippets",
        "not applicable: rstest-bdd checks every step at compile time",
    ),
    (
        "--tags",
        "scenarios!(dir, tags = \"...\") at compile time, or sbrs test -m with a module name",
    ),
];

/// The text `sbrs behave-options` prints.
#[must_use]
pub fn render() -> String {
    let mut text = String::from(
        "Python SeleniumBase's behave options, as `sbrs test` writes them\n\n\
         -D NAME[=VALUE]   the flag of the same name: -D headless is `sbrs test --headless`,\n\
         \x20                 -D env=qa is `sbrs test --env qa`. The names:\n",
    );
    let mut line = String::from("                   ");
    for name in USERDATA {
        if line.len() + name.len() + 2 > 79 {
            text.push_str(line.trim_end());
            text.push('\n');
            line = String::from("                   ");
        }
        line.push_str(name);
        line.push_str(", ");
    }
    text.push_str(line.trim_end_matches([' ', ',']));
    text.push_str("\n\n");
    for (option, instead) in OTHER_OPTIONS {
        let mut line = format!("{option:<17}");
        for word in instead.split_whitespace() {
            if line.len() + word.len() + 1 > 79 {
                text.push_str(line.trim_end());
                text.push('\n');
                line = " ".repeat(18);
            }
            line.push(' ');
            line.push_str(word);
        }
        text.push_str(&line);
        text.push('\n');
    }
    text.push_str("\nEvery option of Python's pytest plugin is a flag of `sbrs test` as well:\nsee `sbrs test --help`.\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::test_options::find_option;

    #[test]
    fn every_user_data_name_is_a_flag_of_the_option_table() {
        for name in USERDATA {
            let row = find_option(name);
            assert!(row.is_some(), "-D {name} has no flag of that name");
            assert!(row.is_some_and(|row| row.kind.is_flag()), "{name}");
        }
    }

    #[test]
    fn the_text_lists_every_name_and_every_other_option() {
        let text = render();
        for name in USERDATA {
            assert!(text.contains(name), "{name}");
        }
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        for (option, instead) in OTHER_OPTIONS {
            let instead = instead.split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(flat.contains(option) && flat.contains(&instead), "{option}");
        }
        assert!(text.lines().all(|line| line.len() <= 100), "{text}");
    }
}
