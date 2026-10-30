//! `sbrs translate`, run as the real binary.
//!
//! The command needs no browser: it rewrites the action names of a JSON
//! scenario file with the translation dictionary.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

const SCENARIO: &str = r##"{"name": "login", "steps": [
  {"action": "open", "url": "https://example.com"},
  {"action": "type_text", "css": "#user", "text": "demo"},
  {"action": "click", "css": "button"}
]}
"##;

/// `SCENARIO` with the names of its actions in Spanish.
const SPANISH: &str = r##"{"name": "login", "steps": [
  {"action": "abrir", "url": "https://example.com"},
  {"action": "escribir", "css": "#user", "text": "demo"},
  {"action": "clic", "css": "button"}
]}
"##;

fn sbrs(dir: &Path, args: &[&str]) -> Output {
    Command::new(SBASE)
        .current_dir(dir)
        .args(args)
        .output()
        .expect("sbrs starts")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn with_scenario() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("login.json"), SCENARIO).unwrap();
    dir
}

#[test]
fn the_command_is_listed_and_describes_its_options() {
    let dir = tempfile::tempdir().unwrap();
    let out = sbrs(dir.path(), &["translate", "--help"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let help = text(&out.stdout);
    for word in [
        "--print",
        "--overwrite",
        "--copy",
        "--line-numbers",
        "--zh",
        "--es",
        "scenario",
        "Examples",
    ] {
        assert!(help.contains(word), "help lacks {word}:\n{help}");
    }
}

#[test]
fn by_default_it_prints_the_translation_and_changes_no_file() {
    let dir = with_scenario();
    let out = sbrs(dir.path(), &["translate", "login.json", "--es"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), SPANISH);
    assert_eq!(
        fs::read_to_string(dir.path().join("login.json")).unwrap(),
        SCENARIO
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn every_spelling_of_a_language_flag_gives_the_same_translation() {
    let dir = with_scenario();
    for flag in ["--es", "--spanish", "--Spanish"] {
        let out = sbrs(dir.path(), &["translate", "login.json", flag]);
        assert!(out.status.success(), "{flag}: {}", text(&out.stderr));
        assert_eq!(text(&out.stdout), SPANISH, "{flag}");
    }
    // The flag may come before the file, and `-p` asks for what is the default.
    for args in [
        &["translate", "--es", "login.json"][..],
        &["translate", "login.json", "--es", "-p"][..],
        &["translate", "login.json", "--es", "--print"][..],
    ] {
        let out = sbrs(dir.path(), args);
        assert!(out.status.success(), "{args:?}: {}", text(&out.stderr));
        assert_eq!(text(&out.stdout), SPANISH, "{args:?}");
    }
}

#[test]
fn n_puts_the_line_number_in_front_of_every_printed_line() {
    let dir = with_scenario();
    for flag in ["-n", "--line-numbers"] {
        let out = sbrs(dir.path(), &["translate", "login.json", "--es", flag]);
        assert!(out.status.success(), "{flag}: {}", text(&out.stderr));
        let printed = text(&out.stdout);
        let expected: String = SPANISH
            .lines()
            .enumerate()
            .map(|(index, line)| format!("{} {line}\n", index + 1))
            .collect();
        assert_eq!(printed, expected, "{flag}");
        assert!(printed.starts_with("1 {\"name\": \"login\""), "{printed}");
    }
    // The file is not numbered: only the printed lines are.
    let copy = sbrs(dir.path(), &["translate", "login.json", "--es", "-c", "-n"]);
    assert!(copy.status.success(), "{}", text(&copy.stderr));
    assert_eq!(
        fs::read_to_string(dir.path().join("login_es.json")).unwrap(),
        SPANISH
    );
}

#[test]
fn overwrite_replaces_the_file_and_translating_back_restores_it() {
    let dir = with_scenario();
    let out = sbrs(
        dir.path(),
        &["translate", "login.json", "--japanese", "--overwrite"],
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    // An overwrite alone says what it wrote and does not print the scenario.
    assert!(
        text(&out.stdout).contains("Wrote login.json"),
        "{}",
        text(&out.stdout)
    );
    assert!(
        !text(&out.stdout).contains("\"steps\""),
        "{}",
        text(&out.stdout)
    );
    let japanese = fs::read_to_string(dir.path().join("login.json")).unwrap();
    assert!(
        japanese.contains("\"開く\"") && japanese.contains("\"クリック\""),
        "{japanese}"
    );

    // From Japanese straight to English restores the original file.
    let back = sbrs(dir.path(), &["translate", "login.json", "--en", "-o"]);
    assert!(back.status.success(), "{}", text(&back.stderr));
    assert_eq!(
        fs::read_to_string(dir.path().join("login.json")).unwrap(),
        SCENARIO
    );
}

#[test]
fn copy_writes_a_new_file_named_with_the_language_code_and_refuses_to_replace_one() {
    let dir = with_scenario();
    let out = sbrs(dir.path(), &["translate", "login.json", "--fr", "--copy"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(
        text(&out.stdout).contains("login_fr.json"),
        "{}",
        text(&out.stdout)
    );
    let copy = fs::read_to_string(dir.path().join("login_fr.json")).unwrap();
    assert!(
        copy.contains("\"ouvrir\"") && copy.contains("\"cliquer\""),
        "{copy}"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("login.json")).unwrap(),
        SCENARIO
    );

    let again = sbrs(dir.path(), &["translate", "login.json", "--fr", "-c"]);
    assert!(!again.status.success());
    assert!(
        text(&again.stderr).contains("already exists"),
        "{}",
        text(&again.stderr)
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("login_fr.json")).unwrap(),
        copy
    );

    // Python names the copy after the language code, whichever way the flag is spelled.
    let chinese = sbrs(dir.path(), &["translate", "login.json", "--zh", "-c"]);
    assert!(chinese.status.success(), "{}", text(&chinese.stderr));
    assert!(dir.path().join("login_zh.json").is_file());
    let named = sbrs(dir.path(), &["translate", "login.json", "--Dutch", "-c"]);
    assert!(named.status.success(), "{}", text(&named.stderr));
    assert!(dir.path().join("login_nl.json").is_file());
}

#[test]
fn print_may_go_with_one_of_the_other_actions() {
    let dir = with_scenario();
    let copy = sbrs(dir.path(), &["translate", "login.json", "--es", "-p", "-c"]);
    assert!(copy.status.success(), "{}", text(&copy.stderr));
    let printed = text(&copy.stdout);
    assert!(
        printed.starts_with(SPANISH),
        "-p -c prints the scenario:\n{printed}"
    );
    assert!(printed.contains("Wrote login_es.json"), "{printed}");
    assert_eq!(
        fs::read_to_string(dir.path().join("login_es.json")).unwrap(),
        SPANISH
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("login.json")).unwrap(),
        SCENARIO
    );

    let overwrite = sbrs(dir.path(), &["translate", "login.json", "--es", "-o", "-p"]);
    assert!(overwrite.status.success(), "{}", text(&overwrite.stderr));
    assert!(text(&overwrite.stdout).starts_with(SPANISH));
    assert_eq!(
        fs::read_to_string(dir.path().join("login.json")).unwrap(),
        SPANISH
    );
}

#[test]
fn a_missing_a_second_a_positional_or_an_unknown_language_is_refused_with_status_2() {
    let dir = with_scenario();
    for args in [
        // No language at all: usage's missing-group error.
        &["translate", "login.json"][..],
        // Two languages.
        &["translate", "login.json", "--es", "--fr"][..],
        // The old positional language.
        &["translate", "login.json", "es"][..],
        &["translate", "login.json", "klingon"][..],
        // German is not a language of the dictionary.
        &["translate", "login.json", "--de"][..],
        &["translate", "login.json", "--german"][..],
    ] {
        let out = sbrs(dir.path(), args);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{args:?}: {}",
            text(&out.stderr)
        );
        assert!(!text(&out.stderr).is_empty(), "{args:?} says nothing");
        assert!(
            text(&out.stdout).is_empty(),
            "{args:?}: {}",
            text(&out.stdout)
        );
    }
    let no_language = sbrs(dir.path(), &["translate", "login.json"]);
    assert!(
        text(&no_language.stderr)
            .to_lowercase()
            .contains("language"),
        "{}",
        text(&no_language.stderr)
    );
    // Nothing was written or changed.
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("login.json")).unwrap(),
        SCENARIO
    );
}

#[test]
fn a_missing_file_and_clashing_actions_are_refused() {
    let dir = with_scenario();

    let missing = sbrs(dir.path(), &["translate", "none.json", "--es"]);
    assert!(!missing.status.success());
    assert!(
        text(&missing.stderr).contains("none.json"),
        "{}",
        text(&missing.stderr)
    );

    let clash = sbrs(dir.path(), &["translate", "login.json", "--es", "-o", "-c"]);
    assert_eq!(clash.status.code(), Some(2), "{}", text(&clash.stderr));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("login.json")).unwrap(),
        SCENARIO
    );
}

#[test]
fn an_action_the_dictionary_does_not_know_is_kept_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("odd.json"),
        r#"{"name": "n", "steps": [{"action": "teleport"}, {"action": "open", "url": "u"}]}"#,
    )
    .unwrap();
    let out = sbrs(dir.path(), &["translate", "odd.json", "--ru"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let printed = text(&out.stdout);
    assert!(
        printed.contains("\"teleport\"") && printed.contains("\"открыть\""),
        "{printed}"
    );
    assert!(
        text(&out.stderr).contains("teleport"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn the_extra_browsers_can_be_chosen() {
    let dir = tempfile::tempdir().unwrap();
    let out = sbrs(dir.path(), &["--help"]);
    assert!(out.status.success());
    let help = text(&out.stdout);
    // `--browser` is a value enum, so its choices are shown with `--help`.
    for browser in ["brave", "comet", "opera", "safari", "ie", "cft", "chs"] {
        assert!(help.contains(browser), "--browser lacks {browser}:\n{help}");
    }
}
