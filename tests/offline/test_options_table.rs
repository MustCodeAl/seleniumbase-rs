//! The option table (`config::test_options`) against the manifest of Python
//! options (`parity/api.toml`): an option the manifest lists and the table does
//! not spell would be a flag `sbrs test` cannot take and a variable no run reads.

use seleniumbase_rs::config::test_options::{find_option, OptionKind, TEST_OPTIONS};

/// Rows of the table that have no entry in `options_pytest` or `options_sb`: the pytest and
/// plugin options (`--pdb`, `--reruns`, `--html`, `--junit-xml`, `--alluredir`), the options only `sbrs` has (`--runner`) and the
/// settings that have a variable but no Python option (`SB_TIMEOUT`).
const NOT_IN_THE_MANIFEST: [&str; 16] = [
    "pdb",
    "trace",
    "reruns",
    "reruns-delay",
    "runner",
    "threads",
    "timeout",
    "webdriver-url",
    "window-width",
    "window-height",
    "remote-webdriver-timeout",
    "fingerprint",
    "mode",
    "html",
    "junit-xml",
    "alluredir",
];

fn manifest() -> toml::Table {
    let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/parity/api.toml"));
    toml::from_str(text).expect("parity/api.toml is TOML")
}

fn entries(manifest: &toml::Table, section: &str) -> Vec<(String, String)> {
    let table = manifest
        .get(section)
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| panic!("parity/api.toml has no [{section}] table"));
    table
        .iter()
        .map(|(key, entry)| {
            let status = entry
                .get("status")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_owned();
            (key.clone(), status)
        })
        .collect()
}

#[test]
fn every_options_pytest_key_has_a_spelling_in_the_table() {
    let manifest = manifest();
    let pytest = entries(&manifest, "options_pytest");
    assert!(
        pytest.len() > 200,
        "the manifest lists {} options",
        pytest.len()
    );
    let missing: Vec<_> = pytest
        .iter()
        .filter(|(key, _)| find_option(key).is_none())
        .map(|(key, _)| key.as_str())
        .collect();
    assert!(missing.is_empty(), "no row spells {missing:?}");
}

#[test]
fn every_options_sb_key_has_a_row_with_a_field_or_a_reason() {
    let manifest = manifest();
    let sb = entries(&manifest, "options_sb");
    assert!(sb.len() > 100, "the manifest lists {} keywords", sb.len());
    let missing: Vec<_> = sb
        .iter()
        .filter(|(key, _)| !TEST_OPTIONS.iter().any(|row| row.matches_sb_name(key)))
        .map(|(key, _)| key.as_str())
        .collect();
    assert!(missing.is_empty(), "no row names {missing:?}");
}

#[test]
fn an_option_the_manifest_calls_not_applicable_is_a_variable_or_says_why_it_does_nothing() {
    let manifest = manifest();
    for (key, status) in entries(&manifest, "options_pytest") {
        if status != "not-applicable" {
            continue;
        }
        let row = find_option(&key).unwrap_or_else(|| panic!("no row spells {key}"));
        match row.kind {
            OptionKind::NoOp(why) | OptionKind::Cli(why) | OptionKind::NotAFlag(why) => {
                assert!(!why.is_empty(), "{key} gives no reason");
            }
            kind => assert!(
                kind.reads_env() && row.env.starts_with("SB_"),
                "{key} is neither read from a variable nor a documented no-op: {kind:?}"
            ),
        }
    }
}

#[test]
fn every_row_is_an_option_of_the_manifest_or_one_of_the_known_extras() {
    let manifest = manifest();
    let pytest = entries(&manifest, "options_pytest");
    let sb = entries(&manifest, "options_sb");
    for row in TEST_OPTIONS {
        let in_manifest = pytest.iter().any(|(key, _)| row.matches_flag(key))
            || sb.iter().any(|(key, _)| row.matches_sb_name(key));
        assert!(
            in_manifest || NOT_IN_THE_MANIFEST.contains(&row.long),
            "{} is in neither the manifest nor the list of extras",
            row.long
        );
    }
}

#[test]
fn the_variables_of_the_first_release_are_still_read() {
    for env in [
        "SB_BROWSER",
        "SB_HEADLESS",
        "SB_MODE",
        "SB_USER_AGENT",
        "SB_LOCALE",
        "SB_AD_BLOCK",
        "SB_PROXY",
        "SB_PROXY_PAC_URL",
        "SB_USER_DATA_DIR",
        "SB_EXTENSION_DIR",
        "SB_REUSE_SESSION",
        "SB_REUSE_CLASS_SESSION",
        "SB_MOBILE",
        "SB_THREADS",
        "SB_ESC_END",
        "SB_REC_SB_CDP",
        "SB_FINGERPRINT",
        "SB_START_PAGE",
        "SB_WEBDRIVER_URL",
        "SB_CHROME_BIN",
        "SB_TIMEOUT",
        "SB_REMOTE_WEBDRIVER_TIMEOUT",
        "SB_WINDOW_WIDTH",
        "SB_WINDOW_HEIGHT",
        "SB_SETTINGS_FILE",
    ] {
        assert!(
            TEST_OPTIONS
                .iter()
                .any(|row| row.env == env && row.kind.reads_env()),
            "no row reads {env}"
        );
    }
}
