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
        "sbrs record ",
        "sbrs mkfile x.rs --url ",
        "sbrs mkrec --file x.rs --url ",
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
        ("sbrs export-recording ", "\u{1}extensions\tjson\tyaml"),
        ("sbrs replay ", "\u{1}extensions\tjson\tyaml"),
        ("sbrs mkfile ", "\u{1}extensions\trs"),
        ("sbrs import-python ", "\u{1}extensions\tpy"),
        ("sbrs import-python x.py --output ", "\u{1}extensions\trs"),
        ("sbrs screenshot --path ", "\u{1}extensions\tpng"),
        ("sbrs save-cookies --file ", "\u{1}extensions\tjson"),
        ("sbrs load-cookies --file ", "\u{1}extensions\tjson"),
        ("sbrs translate ", "\u{1}extensions\tjson"),
        ("sbrs --config ", "\u{1}extensions\ttoml\tjson"),
        ("sbrs print --file ", "\u{1}files"),
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
fn translate_offers_the_language_codes_after_the_file() {
    let languages = complete("sbrs translate scenario.json ");
    for code in ["en", "zh", "es", "ja"] {
        assert!(languages.iter().any(|word| word == code), "{languages:?}");
    }
    assert!(
        languages.iter().all(|word| !word.starts_with('\u{1}')),
        "no file names: {languages:?}"
    );
}

#[test]
fn a_global_flag_completes_after_the_command() {
    assert_eq!(
        complete("sbrs get-title --hea"),
        ["--headed", "--headless", "--headless1"]
    );
}
