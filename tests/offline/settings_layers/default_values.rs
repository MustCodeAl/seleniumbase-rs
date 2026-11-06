//! A value that equals its default is a value (`docs/design/settings_v2.md`, step 6): a
//! source that gives a setting its default value still wins over a lower source that gave it
//! another, whatever the kind of the setting and whether the serializer leaves the default
//! out (an unset option, a list with no item).

use seleniumbase_rs::config::settings::{Settings, SettingsOverrides};

use super::file_with;

/// The settings of the file `content`, with the variables `vars` over it and the pairs
/// `pairs` over those, as `sbrs` lays its flags.
fn stacked(content: &str, vars: &[(&str, &str)], pairs: &[(&'static str, &str)]) -> Settings {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = file_with(dir.path(), "sbase_config.toml", content);
    let check = Settings::check_with(Some(&path), |name| {
        vars.iter()
            .find(|(variable, _)| *variable == name)
            .map(|(_, value)| (*value).to_owned())
    });
    assert!(check.problems.is_empty(), "{:?}", check.problems);
    let mut settings = check.settings;
    SettingsOverrides {
        options: pairs
            .iter()
            .map(|(name, value)| (*name, (*value).to_owned()))
            .collect(),
    }
    .try_apply(&mut settings)
    .expect("the pairs are valid");
    settings
}

/// The JSON text of the setting at the dotted `path` of the serialized settings, or `-` for a
/// setting that serializes to nothing (an unset option, an empty list, a switch that is off).
fn shown(settings: &Settings, path: &str) -> String {
    let whole = serde_json::to_value(settings).expect("the settings serialize");
    let mut at = &whole;
    for part in path.split('.') {
        match at.get(part) {
            Some(inner) => at = inner,
            None => return "-".to_owned(),
        }
    }
    at.to_string()
}

#[test]
fn a_higher_source_that_gives_the_default_of_a_setting_still_wins() {
    // (the path of the setting, a file that gives it a value other than its default, the
    // variable that gives it, and the text of the default value)
    let pins: [(&str, &str, &'static str, &str); 17] = [
        // A number of the run group, and numbers of the test group.
        (
            "run.timeout_multiplier",
            "timeout_multiplier = 3\n",
            "SB_TIMEOUT_MULTIPLIER",
            "1",
        ),
        (
            "run.highlights",
            "[run]\nhighlights = 4\n",
            "SB_HIGHLIGHTS",
            "0",
        ),
        ("run.demo_sleep", "demo_sleep = 5\n", "SB_DEMO_SLEEP", "0"),
        ("test.reruns", "[test]\nreruns = 2\n", "SB_RERUNS", "0"),
        (
            "test.reruns_delay",
            "[test]\nreruns_delay = 1.5\n",
            "SB_RERUNS_DELAY",
            "0",
        ),
        (
            "test.time_limit",
            "[test]\ntime_limit = 9\n",
            "SB_TIME_LIMIT",
            "0",
        ),
        // A text, an enum and an option of the test group and the session.
        (
            "test.runner",
            "[test]\nrunner = \"nextest\"\n",
            "SB_TEST_RUNNER",
            "cargo",
        ),
        (
            "session.grid.protocol",
            "protocol = \"https\"\n",
            "SB_PROTOCOL",
            "http",
        ),
        ("session.grid.port", "port = 4445\n", "SB_PORT", "4444"),
        (
            "session.remote_webdriver_timeout_seconds",
            "remote_webdriver_timeout_seconds = 9\n",
            "SB_REMOTE_WEBDRIVER_TIMEOUT",
            "0",
        ),
        (
            "browser.kind",
            "browser = \"firefox\"\n",
            "SB_BROWSER",
            "chrome",
        ),
        ("session.mode", "mode = \"cdp\"\n", "SB_MODE", "webdriver"),
        (
            "session.page_load_strategy",
            "page_load_strategy = \"eager\"\n",
            "SB_PAGE_LOAD_STRATEGY",
            "normal",
        ),
        // A list with no item: the list of the higher source replaces the lower one's.
        (
            "browser.proxy.bypass_list",
            "proxy_bypass_list = \"a.test,b.test\"\n",
            "SB_PROXY_BYPASS_LIST",
            ",",
        ),
        (
            "browser.extensions",
            "extension_dir = \"/a,/b\"\n",
            "SB_EXTENSION_DIR",
            ",",
        ),
        // A switch of the report group and of the test group.
        (
            "report.dashboard",
            "[report]\ndashboard = true\n",
            "SB_DASHBOARD",
            "false",
        ),
        ("test.pdb", "[test]\npdb = true\n", "SB_PDB", "false"),
    ];
    let mut wrong = Vec::new();
    for (path, file, variable, default) in pins {
        // What the setting shows when the default is the only source, and when the file is.
        let by_default = shown(&stacked("", &[(variable, default)], &[]), path);
        let by_file = shown(&stacked(file, &[], &[]), path);
        if by_file == by_default {
            wrong.push(format!("{path}: the file does not differ from the default"));
        }
        let over_by_variable = shown(&stacked(file, &[(variable, default)], &[]), path);
        let over_by_pair = shown(&stacked(file, &[], &[(variable, default)]), path);
        for (source, seen) in [("a variable", over_by_variable), ("a pair", over_by_pair)] {
            if seen != by_default {
                wrong.push(format!(
                    "{path}: {source} gives {default:?} over the file, and the setting shows \
                     {seen} instead of {by_default} (the file alone shows {by_file})"
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn the_default_a_pair_gives_wins_over_a_variable_that_gave_another_value() {
    // The layers are the file, the variables, then the pairs: each one over the one before.
    let settings = stacked(
        "proxy_bypass_list = \"a.test\"\nport = 4445\n",
        &[("SB_PROXY_BYPASS_LIST", "b.test"), ("SB_PORT", "4446")],
        &[("SB_PROXY_BYPASS_LIST", ","), ("SB_PORT", "4444")],
    );
    assert_eq!(shown(&settings, "browser.proxy.bypass_list"), "-");
    assert_eq!(shown(&settings, "session.grid.port"), "4444");
}
