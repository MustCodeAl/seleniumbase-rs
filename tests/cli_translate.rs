//! `sbase translate`, run as the real binary.
//!
//! The command needs no browser: it rewrites the action names of a JSON
//! scenario file with the translation dictionary.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

const SCENARIO: &str = r##"{"name": "login", "steps": [
  {"action": "open", "url": "https://example.com"},
  {"action": "type_text", "css": "#user", "text": "demo"},
  {"action": "click", "css": "button"}
]}
"##;

fn sbase(dir: &Path, args: &[&str]) -> Output {
    Command::new(SBASE)
        .current_dir(dir)
        .args(args)
        .output()
        .expect("sbase starts")
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
    let out = sbase(dir.path(), &["translate", "--help"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let help = text(&out.stdout);
    for word in ["--print", "--overwrite", "--copy", "LANGUAGE", "scenario"] {
        assert!(help.contains(word), "help lacks {word}:\n{help}");
    }
}

#[test]
fn by_default_it_prints_the_translation_and_changes_no_file() {
    let dir = with_scenario();
    let out = sbase(dir.path(), &["translate", "login.json", "es"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        text(&out.stdout),
        SCENARIO
            .replace("\"open\"", "\"abrir\"")
            .replace("\"type_text\"", "\"escribir\"")
            .replace("\"click\"", "\"clic\"")
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("login.json")).unwrap(),
        SCENARIO
    );
}

#[test]
fn a_language_may_be_given_by_name_and_translations_chain() {
    let dir = with_scenario();
    let out = sbase(
        dir.path(),
        &["translate", "login.json", "Japanese", "--overwrite"],
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    let japanese = fs::read_to_string(dir.path().join("login.json")).unwrap();
    assert!(
        japanese.contains("\"開く\"") && japanese.contains("\"クリック\""),
        "{japanese}"
    );

    // From Japanese straight to English restores the original file.
    let back = sbase(dir.path(), &["translate", "login.json", "en", "-o"]);
    assert!(back.status.success(), "{}", text(&back.stderr));
    assert_eq!(
        fs::read_to_string(dir.path().join("login.json")).unwrap(),
        SCENARIO
    );
}

#[test]
fn copy_writes_a_new_file_and_refuses_to_replace_one() {
    let dir = with_scenario();
    let out = sbase(dir.path(), &["translate", "login.json", "fr", "--copy"]);
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

    let again = sbase(dir.path(), &["translate", "login.json", "fr", "-c"]);
    assert!(!again.status.success());
    assert!(
        text(&again.stderr).contains("already exists"),
        "{}",
        text(&again.stderr)
    );
}

#[test]
fn a_bad_language_a_missing_file_and_clashing_options_are_refused() {
    let dir = with_scenario();

    let language = sbase(dir.path(), &["translate", "login.json", "klingon"]);
    assert!(!language.status.success());
    let message = text(&language.stderr);
    assert!(
        message.contains("klingon") && message.contains("zh"),
        "{message}"
    );

    let missing = sbase(dir.path(), &["translate", "none.json", "es"]);
    assert!(!missing.status.success());
    assert!(
        text(&missing.stderr).contains("none.json"),
        "{}",
        text(&missing.stderr)
    );

    let clash = sbase(dir.path(), &["translate", "login.json", "es", "-o", "-c"]);
    assert!(!clash.status.success());
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
    let out = sbase(dir.path(), &["translate", "odd.json", "de"]);
    // German is not a language of the dictionary.
    assert!(!out.status.success());

    let out = sbase(dir.path(), &["translate", "odd.json", "ru"]);
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
    let out = sbase(dir.path(), &["--help"]);
    assert!(out.status.success());
    let help = text(&out.stdout);
    // `--browser` is a value enum, so its choices are shown with `--help`.
    for browser in ["brave", "comet", "opera", "safari", "ie", "cft", "chs"] {
        assert!(help.contains(browser), "--browser lacks {browser}:\n{help}");
    }
}
