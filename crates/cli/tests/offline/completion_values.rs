//! What `sbrs` offers at a value position.
//!
//! Every shell's completion script calls the hidden `sbrs __complete_word__` command and
//! acts on its answer, so these tests read that answer and need no shell. A line of
//! `\u{1}files`, `\u{1}dirs`, `\u{1}executables` or `\u{1}extensions\t...` tells the script to
//! complete file names; a value with no line at all offers nothing. The tests that run the
//! scripts in real shells are in `completion_shells.rs`.

use std::process::Command;

/// The lines `sbrs` answers for the command line `line`, with the cursor at its end.
fn complete(line: &str) -> Vec<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_sbrs"))
        .args(["__complete_word__", "--shell", "bash", "--line", line])
        .output()
        .expect("run sbrs");
    assert!(
        output.status.success(),
        "{line:?}: {:?}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn a_selector_or_a_text_is_not_a_file_name() {
    // Before the value hints these fell back to file names, which is wrong for every one of
    // them and offered the files of the folder the person happened to be in.
    for line in [
        "sbrs get-text --css ",
        "sbrs assert-element --css ",
        "sbrs wait-for-text --css h1 --text ",
        "sbrs get-attribute --css a --attribute ",
        "sbrs get-property --css a --property ",
        "sbrs assert-attribute --css a --attribute href --value ",
        "sbrs select-option --css select --value ",
        "sbrs set-local-storage-item --key k --value ",
        "sbrs get-local-storage-item --key ",
        "sbrs hover-and-click --hover-css ",
        "sbrs hover-and-click --hover-css a --click-css ",
        "sbrs drag-and-drop --source-css ",
        "sbrs drag-and-drop --source-css a --target-css ",
        "sbrs cdp --cmd ",
        "sbrs cdp --cmd Browser.getVersion --params ",
        "sbrs choose-file ",
        "sbrs smoke https://example.com --title-contains ",
        "sbrs deferred --assert ",
        "sbrs encrypt ",
        "sbrs --user-agent ",
        "sbrs --proxy ",
    ] {
        assert_eq!(complete(line), Vec::<String>::new(), "{line:?}");
    }
}

#[test]
fn a_url_is_not_a_file_name() {
    for line in [
        "sbrs open ",
        "sbrs smoke ",
        "sbrs download-file ",
        "sbrs print-pdf ",
        "sbrs pdf-text ",
        "sbrs assert-pdf-text ",
        "sbrs mkfile x.rs --url ",
        "sbrs mkrec x.rs --url ",
        "sbrs --start-page ",
        "sbrs --url ",
        "sbrs get-title --url ",
        "sbrs --webdriver ",
        "sbrs --proxy-pac-url ",
    ] {
        assert_eq!(complete(line), Vec::<String>::new(), "{line:?}");
    }
}

#[test]
fn a_file_is_completed_by_the_kind_the_command_takes() {
    for (line, answer) in [
        ("sbrs mkrec ", "\u{1}extensions\trs"),
        ("sbrs replay ", "\u{1}extensions\tjson\tyaml"),
        ("sbrs mkfile ", "\u{1}extensions\trs"),
        ("sbrs import-python ", "\u{1}extensions\tpy"),
        ("sbrs import-python x.py --output ", "\u{1}extensions\trs"),
        ("sbrs screenshot --path ", "\u{1}extensions\tpng"),
        ("sbrs save-cookies --file ", "\u{1}extensions\tjson"),
        ("sbrs load-cookies --file ", "\u{1}extensions\tjson"),
        ("sbrs translate ", "\u{1}extensions\tjson"),
        ("sbrs --config ", "\u{1}extensions\ttoml\tjson"),
        ("sbrs print ", "\u{1}files"),
        ("sbrs mkpres ", "\u{1}extensions\trs"),
        ("sbrs mkchart ", "\u{1}extensions\trs"),
        ("sbrs convert ", "\u{1}extensions\tpy"),
        ("sbrs --proxy-list ", "\u{1}files"),
        ("sbrs choose-file input ", "\u{1}files"),
        ("sbrs mkdir ", "\u{1}dirs"),
        ("sbrs --user-data-dir ", "\u{1}dirs"),
        ("sbrs get-title --extension-dir ", "\u{1}dirs"),
        ("sbrs patch-chromedriver --path ", "\u{1}executables"),
        ("sbrs patch-chrome --path ", "\u{1}executables"),
        ("sbrs devtools --browser-path ", "\u{1}executables"),
    ] {
        assert_eq!(complete(line), [answer], "{line:?}");
    }
}

#[test]
fn install_offers_the_drivers_and_then_the_versions() {
    assert_eq!(
        complete("sbrs install "),
        [
            "cft",
            "chromedriver",
            "chromium",
            "chs",
            "edgedriver",
            "geckodriver",
            "uc_driver"
        ]
    );
    // `get` is the same command.
    assert_eq!(
        complete("sbrs get "),
        [
            "cft",
            "chromedriver",
            "chromium",
            "chs",
            "edgedriver",
            "geckodriver",
            "uc_driver"
        ]
    );
    assert_eq!(complete("sbrs install gecko"), ["geckodriver"]);
    assert_eq!(
        complete("sbrs install chromedriver "),
        ["beta", "browser", "canary", "dev", "keep", "latest", "mlatest", "previous", "stable"]
    );
    assert_eq!(complete("sbrs install chromedriver sta"), ["stable"]);
}

#[test]
fn translate_offers_the_language_flags_and_the_actions_after_the_file() {
    let flags = complete("sbrs translate scenario.json --");
    for flag in [
        "--en",
        "--zh",
        "--es",
        "--ja",
        "--print",
        "--overwrite",
        "--copy",
    ] {
        assert!(flags.iter().any(|word| word == flag), "{flag}: {flags:?}");
    }
    assert!(
        flags.iter().all(|word| !word.starts_with('\u{1}')),
        "no file names: {flags:?}"
    );
}

#[test]
fn a_global_flag_completes_after_the_command() {
    assert_eq!(
        complete("sbrs get-title --hea"),
        ["--headed", "--headless", "--headless1"]
    );
}

/// The built-in plugins are not in the spec: which ones a build has is known when the word is
/// asked for (`runtime_values.rs`), so a Tab at a plugin name lists the plugins of this binary.
#[test]
fn a_plugin_name_is_offered_from_the_plugins_of_the_build() {
    for line in [
        "sbrs --plugin ",
        "sbrs get-title --plugin ",
        "sbrs plugin info ",
    ] {
        let words = complete(line);
        for plugin in ["shield", "crawler"] {
            assert!(words.iter().any(|word| word == plugin), "{line:?}: {words:?}");
        }
    }
    assert_eq!(complete("sbrs plugin info sh"), ["shield"]);
    assert_eq!(complete("sbrs --plugin cra"), ["crawler"]);
}

/// `sbrs plugin run NAME` runs the task of a plugin, so only the plugins that offer one are
/// worth a Tab there.
#[test]
fn plugin_run_offers_the_plugins_that_have_a_task() {
    let words = complete("sbrs plugin run ");
    assert!(words.iter().any(|word| word == "crawler"), "{words:?}");
    assert!(!words.iter().any(|word| word == "shield"), "{words:?}");
    assert_eq!(complete("sbrs plugin run cra"), ["crawler"]);
}

/// The preset names are the library's own list, offered when the word is asked for.
#[test]
fn a_fingerprint_preset_is_offered_from_the_library_list() {
    assert_eq!(
        complete("sbrs fingerprint export --preset "),
        ["windows", "edge", "macos", "linux", "android", "ios"]
    );
}

/// The plugin tasks are commands of `sbrs plugin` that no declaration holds: a catalog of
/// runtime commands (usage-dynamic) adds them to the help and to the completion.
#[test]
fn the_plugins_with_a_task_are_commands_of_the_plugin_command() {
    let words = complete("sbrs plugin ");
    for command in ["list", "info", "run", "crawler"] {
        assert!(words.iter().any(|word| word == command), "{command}: {words:?}");
    }
    assert!(!words.iter().any(|word| word == "shield"), "{words:?}");
    assert_eq!(complete("sbrs plugin cra"), ["crawler"]);
    let flags = complete("sbrs plugin crawler --");
    for flag in ["--input", "--out"] {
        assert!(flags.iter().any(|word| word == flag), "{flag}: {flags:?}");
    }
}

