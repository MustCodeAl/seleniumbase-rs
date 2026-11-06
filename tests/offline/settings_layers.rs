//! The settings are layered (`docs/design/settings_v2.md`, step 6): the settings file, the
//! `SB_*` variables and explicit code, each over the one before. These tests pin what the
//! layering changed in what a person sees: the errors name their source, and `check` lists the
//! problems of every source, sorted by the row of the option table.

use std::path::{Path, PathBuf};

use seleniumbase_rs::config::settings::Settings;

/// A settings file `name` with the text `content` in the folder `dir`.
fn file_with(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write the settings file");
    path
}

/// The text of the error of reading the settings file `name` with `content`.
fn refused(name: &str, content: &str) -> String {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = file_with(dir.path(), name, content);
    Settings::from_file(path)
        .expect_err("the file is refused")
        .to_string()
}

#[test]
fn a_refused_file_value_names_the_file_and_the_key_as_the_file_wrote_it() {
    // The key is spelled with a dash and under an alias; the error gives it back as written.
    let error = refused("sbase_config.toml", "Window-Size = \"hunter2\"\n");
    assert!(error.contains("sbase_config.toml"), "{error}");
    assert!(error.contains("Window-Size"), "{error}");
    // A value that may be a secret is not repeated.
    assert!(!error.contains("hunter2"), "{error}");

    // A boolean that is none: the file, the key, then what is wrong with the value.
    let error = refused("sbase_config.toml", "headless = \"maybe\"\n");
    assert!(
        error.contains("sbase_config.toml: headless: cannot parse bool"),
        "{error}"
    );

    // A JSON file is named the same way.
    let error = refused("settings.json", r#"{"dark": "perhaps"}"#);
    assert!(error.contains("settings.json: dark: "), "{error}");
}

#[test]
fn a_refused_table_value_names_the_file_and_the_key_of_its_table() {
    let error = refused("sbase_config.toml", "[run]\ndemo_mode = \"later\"\n");
    assert!(error.contains("sbase_config.toml"), "{error}");
    assert!(error.contains("demo_mode"), "{error}");

    // A key written flat that belongs to a table is named as the file wrote it.
    let error = refused("sbase_config.toml", "demo = \"later\"\n");
    assert!(error.contains("sbase_config.toml"), "{error}");
    assert!(error.contains("demo"), "{error}");
}

#[test]
fn check_lists_the_problems_of_the_file_and_of_the_variables_sorted_by_row() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    // `window_width` is a row after `headless` and before `threads`; `mobile` is after both.
    let path = file_with(
        dir.path(),
        "sbase_config.toml",
        "mobile = \"sometimes\"\nwindow_width = \"wide\"\n",
    );
    let check = Settings::check_with(Some(&path), |name| match name {
        "SB_THREADS" => Some("lots".to_owned()),
        "SB_HEADLESS" => Some("maybe".to_owned()),
        _ => None,
    });
    let order: Vec<String> = check
        .problems
        .iter()
        .map(|problem| {
            let text = problem.error.to_string();
            let key = ["SB_HEADLESS", "window_width", "SB_THREADS", "mobile"]
                .into_iter()
                .find(|key| text.contains(key))
                .unwrap_or("?");
            key.to_owned()
        })
        .collect();
    assert_eq!(
        order,
        ["SB_HEADLESS", "window_width", "SB_THREADS", "mobile"],
        "{:?}",
        check.problems
    );
    // Each source is told apart: the variables by their name, the file by its path.
    let sources: Vec<&str> = check
        .problems
        .iter()
        .map(|problem| problem.source.as_str())
        .collect();
    let file = path.display().to_string();
    assert_eq!(
        sources,
        ["SB_HEADLESS", file.as_str(), "SB_THREADS", file.as_str()]
    );
}

#[test]
fn a_file_with_one_bad_value_still_gives_its_other_values_to_check() {
    let dir = tempfile::tempdir().expect("a temporary folder");
    let path = file_with(
        dir.path(),
        "sbase_config.toml",
        "window_width = \"wide\"\nuser_agent = \"agent-of-the-file\"\n",
    );
    let check = Settings::check_with(Some(&path), |_| None);
    assert_eq!(check.problems.len(), 1, "{:?}", check.problems);
    assert_eq!(
        check.settings.browser.user_agent.as_deref(),
        Some("agent-of-the-file")
    );
}
