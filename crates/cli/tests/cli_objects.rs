//! `sbrs extract-objects`, `inject-objects`, `objectify` and `revert-objects`, run as the real
//! binary on test files in a scratch folder.
//!
//! The files they write are the golden files of `tests/cli_objects_compile/`: that folder is a
//! test target of the library, so the injected test and its `page_objects.rs` are compiled
//! against the real `BaseCase` API. None of the commands starts a browser or touches the network.

use std::path::Path;
use std::process::{Command, Output, Stdio};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// The test the goldens start from.
const ORIGINAL: &str = include_str!("../../../tests/cli_objects_compile/original.rs.golden");
/// What `inject-objects` makes of it.
const INJECTED: &str = include_str!("../../../tests/cli_objects_compile/main.rs");
/// What `inject-objects -c` makes of it.
const COMMENTED: &str = include_str!("../../../tests/cli_objects_compile/commented.rs.golden");
/// What `extract-objects` writes for it.
const OBJECTS: &str = include_str!("../../../tests/cli_objects_compile/page_objects.rs");

/// The exit status of `SeleniumBaseError::InvalidConfig`.
const INVALID: i32 = 2;
/// The exit status of a file that cannot be read (`SeleniumBaseError::Io`).
const IO_FAILURE: i32 = 6;

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(SBASE)
        .args(args)
        .current_dir(dir)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error")
        .stdin(Stdio::null())
        .output()
        .expect("sbrs runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Runs a command that must succeed and returns its standard output.
fn succeeds(dir: &Path, args: &[&str]) -> String {
    let output = run(dir, args);
    assert!(
        output.status.success(),
        "sbrs {args:?} failed: {}",
        stderr(&output)
    );
    stdout(&output)
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// A scratch folder with the original test in it as `login.rs`.
fn folder_with_the_test() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("login.rs"), ORIGINAL).expect("write the test");
    dir
}

#[test]
fn extract_objects_writes_page_objects_rs_and_leaves_the_test_alone() {
    let dir = folder_with_the_test();

    let said = succeeds(dir.path(), &["extract-objects", "login.rs"]);

    assert_eq!(read(dir.path(), "page_objects.rs"), OBJECTS);
    assert_eq!(read(dir.path(), "login.rs"), ORIGINAL);
    assert_eq!(said, "Wrote 10 page objects to page_objects.rs.\n");
}

#[test]
fn extract_objects_runs_again_without_changing_the_file() {
    let dir = folder_with_the_test();
    succeeds(dir.path(), &["extract-objects", "login.rs"]);

    let said = succeeds(dir.path(), &["extract-objects", "login.rs"]);

    assert_eq!(read(dir.path(), "page_objects.rs"), OBJECTS);
    assert_eq!(
        said,
        "page_objects.rs already has the selectors of login.rs.\n"
    );
}

#[test]
fn extract_objects_in_a_folder_writes_next_to_the_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(dir.path().join("tests")).expect("tests folder");
    std::fs::write(dir.path().join("tests/login.rs"), ORIGINAL).expect("write the test");

    let said = succeeds(dir.path(), &["extract-objects", "tests/login.rs"]);

    assert_eq!(read(dir.path(), "tests/page_objects.rs"), OBJECTS);
    assert!(!dir.path().join("page_objects.rs").exists());
    let path = format!("tests{}page_objects.rs", std::path::MAIN_SEPARATOR);
    assert_eq!(said, format!("Wrote 10 page objects to {path}.\n"));
}

#[test]
fn a_file_without_selectors_makes_no_page_objects_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("empty.rs"), "fn main() {}\n").expect("write the test");

    let said = succeeds(dir.path(), &["extract-objects", "empty.rs"]);

    assert_eq!(said, "No selectors found in empty.rs.\n");
    assert!(!dir.path().join("page_objects.rs").exists());
}

#[test]
fn inject_objects_replaces_the_selectors() {
    let dir = folder_with_the_test();
    succeeds(dir.path(), &["extract-objects", "login.rs"]);

    let said = succeeds(dir.path(), &["inject-objects", "login.rs"]);

    assert_eq!(read(dir.path(), "login.rs"), INJECTED);
    assert_eq!(
        said,
        "Replaced 10 selectors in login.rs with page objects.\n"
    );
    assert_eq!(read(dir.path(), "page_objects.rs"), OBJECTS);
}

#[test]
fn inject_objects_with_comments_appends_the_selectors() {
    for flag in ["-c", "--comments"] {
        let dir = folder_with_the_test();
        succeeds(dir.path(), &["extract-objects", "login.rs"]);

        succeeds(dir.path(), &["inject-objects", "login.rs", flag]);

        assert_eq!(read(dir.path(), "login.rs"), COMMENTED, "{flag}");
    }
}

#[test]
fn the_flag_may_come_before_the_file() {
    let dir = folder_with_the_test();
    succeeds(dir.path(), &["extract-objects", "login.rs"]);

    succeeds(dir.path(), &["inject-objects", "-c", "login.rs"]);

    assert_eq!(read(dir.path(), "login.rs"), COMMENTED);
}

#[test]
fn inject_objects_twice_changes_nothing_the_second_time() {
    let dir = folder_with_the_test();
    succeeds(dir.path(), &["extract-objects", "login.rs"]);
    succeeds(dir.path(), &["inject-objects", "login.rs"]);

    let said = succeeds(dir.path(), &["inject-objects", "login.rs"]);

    assert_eq!(read(dir.path(), "login.rs"), INJECTED);
    assert_eq!(said, "No selector of login.rs has a page object.\n");
}

#[test]
fn revert_objects_gives_the_original_back_byte_for_byte() {
    let dir = folder_with_the_test();
    succeeds(dir.path(), &["extract-objects", "login.rs"]);
    succeeds(dir.path(), &["inject-objects", "login.rs"]);
    assert_eq!(read(dir.path(), "login.rs"), INJECTED);

    let said = succeeds(dir.path(), &["revert-objects", "login.rs"]);

    assert_eq!(read(dir.path(), "login.rs"), ORIGINAL);
    assert_eq!(said, "Restored 10 selectors in login.rs.\n");
    assert_eq!(
        read(dir.path(), "page_objects.rs"),
        OBJECTS,
        "revert keeps the page objects file"
    );
}

#[test]
fn revert_objects_with_comments_takes_the_comments_off() {
    let dir = folder_with_the_test();
    succeeds(dir.path(), &["extract-objects", "login.rs"]);
    succeeds(dir.path(), &["inject-objects", "login.rs", "-c"]);
    assert_eq!(read(dir.path(), "login.rs"), COMMENTED);

    succeeds(dir.path(), &["revert-objects", "login.rs", "-c"]);

    assert_eq!(read(dir.path(), "login.rs"), ORIGINAL);
}

#[test]
fn revert_objects_without_the_flag_keeps_the_comments() {
    let dir = folder_with_the_test();
    succeeds(dir.path(), &["extract-objects", "login.rs"]);
    succeeds(dir.path(), &["inject-objects", "login.rs", "-c"]);

    succeeds(dir.path(), &["revert-objects", "login.rs"]);

    let text = read(dir.path(), "login.rs");
    assert!(!text.contains("page_objects"), "{text}");
    assert!(text.contains("sb.click(\"button#send\").await?; // \"button#send\"\n"));
}

#[test]
fn revert_objects_on_a_file_without_page_objects_changes_nothing() {
    let dir = folder_with_the_test();
    succeeds(dir.path(), &["extract-objects", "login.rs"]);

    let said = succeeds(dir.path(), &["revert-objects", "login.rs"]);

    assert_eq!(read(dir.path(), "login.rs"), ORIGINAL);
    assert_eq!(said, "login.rs uses no page object.\n");
}

#[test]
fn objectify_is_extract_and_inject() {
    let dir = folder_with_the_test();

    let said = succeeds(dir.path(), &["objectify", "login.rs"]);

    assert_eq!(read(dir.path(), "page_objects.rs"), OBJECTS);
    assert_eq!(read(dir.path(), "login.rs"), INJECTED);
    assert_eq!(
        said,
        "Wrote 10 page objects to page_objects.rs.\n\
         Replaced 10 selectors in login.rs with page objects.\n"
    );
    assert!(
        !dir.path().join("page_object.rs").exists(),
        "the starter file of the old objectify is gone"
    );
}

#[test]
fn objectify_with_comments_and_then_revert_with_comments() {
    let dir = folder_with_the_test();

    succeeds(dir.path(), &["objectify", "login.rs", "-c"]);
    assert_eq!(read(dir.path(), "login.rs"), COMMENTED);

    succeeds(dir.path(), &["revert-objects", "login.rs", "-c"]);
    assert_eq!(read(dir.path(), "login.rs"), ORIGINAL);
}

#[test]
fn the_underscore_spellings_are_the_same_commands() {
    let dir = folder_with_the_test();

    succeeds(dir.path(), &["extract_objects", "login.rs"]);
    assert_eq!(read(dir.path(), "page_objects.rs"), OBJECTS);

    succeeds(dir.path(), &["inject_objects", "login.rs"]);
    assert_eq!(read(dir.path(), "login.rs"), INJECTED);

    succeeds(dir.path(), &["revert_objects", "login.rs"]);
    assert_eq!(read(dir.path(), "login.rs"), ORIGINAL);
}

#[test]
fn a_file_with_two_pages_gets_a_module_for_each() {
    let dir = tempfile::tempdir().expect("temp dir");
    let test = "async fn flow(sb: &mut BaseCase) -> Result<(), E> {\n\
                \x20   sb.open(\"https://example.com/one\").await?;\n\
                \x20   sb.click(\"#go\").await?;\n\
                \x20   sb.open(\"https://example.com/two\").await?;\n\
                \x20   sb.click(\"#go\").await?;\n\
                \x20   sb.click(\"#back\").await?;\n\
                }\n";
    std::fs::write(dir.path().join("flow.rs"), test).expect("write the test");

    let said = succeeds(dir.path(), &["objectify", "flow.rs"]);

    assert!(said.starts_with("Wrote 3 page objects"), "{said}");
    let objects = read(dir.path(), "page_objects.rs");
    assert!(objects.contains("/// https://example.com/one\npub mod example_com_one {\n    pub const GO: &str = \"#go\";\n}\n"), "{objects}");
    assert!(objects.contains("/// https://example.com/two\npub mod example_com_two {\n    pub const GO: &str = \"#go\";\n    pub const BACK: &str = \"#back\";\n}\n"), "{objects}");
    let flow = read(dir.path(), "flow.rs");
    assert!(
        flow.starts_with("mod page_objects;\nasync fn flow"),
        "{flow}"
    );
    assert!(
        flow.contains("sb.click(page_objects::example_com_one::GO)"),
        "{flow}"
    );
    assert!(
        flow.contains("sb.click(page_objects::example_com_two::BACK)"),
        "{flow}"
    );

    succeeds(dir.path(), &["revert-objects", "flow.rs"]);
    assert_eq!(read(dir.path(), "flow.rs"), test);
}

#[test]
fn the_page_objects_file_keeps_what_it_has_when_another_test_is_extracted() {
    let dir = folder_with_the_test();
    succeeds(dir.path(), &["extract-objects", "login.rs"]);
    std::fs::write(
        dir.path().join("other.rs"),
        "fn t() {\n    sb.open(\"https://example.com/form\");\n    sb.click(\"#name\");\n    sb.click(\"#extra\");\n}\n",
    )
    .expect("write the test");

    let said = succeeds(dir.path(), &["extract-objects", "other.rs"]);

    assert_eq!(said, "Wrote 1 page object to page_objects.rs.\n");
    let objects = read(dir.path(), "page_objects.rs");
    assert!(
        objects.contains("    pub const H2: &str = \"h2\";\n"),
        "{objects}"
    );
    assert!(
        objects.contains("    pub const EXTRA: &str = \"#extra\";\n"),
        "{objects}"
    );
}

#[test]
fn a_file_that_is_not_there_fails_with_the_io_status() {
    let dir = tempfile::tempdir().expect("temp dir");

    for command in [
        "extract-objects",
        "inject-objects",
        "objectify",
        "revert-objects",
    ] {
        let output = run(dir.path(), &[command, "missing.rs"]);

        assert_eq!(output.status.code(), Some(IO_FAILURE), "{command}");
        assert!(
            stderr(&output).contains("missing.rs"),
            "{command}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn a_file_that_is_not_a_rust_file_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("login.py"), "self.click('#a')\n").expect("write the file");

    for command in [
        "extract-objects",
        "inject-objects",
        "objectify",
        "revert-objects",
    ] {
        let output = run(dir.path(), &[command, "login.py"]);

        assert_eq!(output.status.code(), Some(INVALID), "{command}");
        assert!(
            stderr(&output).contains("is not a Rust file"),
            "{command}: {}",
            stderr(&output)
        );
    }
    assert!(!dir.path().join("page_objects.rs").exists());
}

#[test]
fn inject_and_revert_without_a_page_objects_file_say_to_extract_first() {
    let dir = folder_with_the_test();

    for command in ["inject-objects", "revert-objects"] {
        let output = run(dir.path(), &[command, "login.rs"]);

        assert_eq!(output.status.code(), Some(INVALID), "{command}");
        assert!(
            stderr(&output).contains("sbrs extract-objects login.rs"),
            "{command}: {}",
            stderr(&output)
        );
    }
    assert_eq!(read(dir.path(), "login.rs"), ORIGINAL);
}

#[test]
fn a_page_objects_file_that_sbrs_did_not_write_is_not_overwritten() {
    let dir = folder_with_the_test();
    std::fs::write(dir.path().join("page_objects.rs"), "pub fn mine() {}\n").expect("write it");

    let output = run(dir.path(), &["extract-objects", "login.rs"]);

    assert_eq!(output.status.code(), Some(INVALID));
    assert!(
        stderr(&output).contains("page_objects.rs cannot be read"),
        "{}",
        stderr(&output)
    );
    assert_eq!(read(dir.path(), "page_objects.rs"), "pub fn mine() {}\n");
}

#[test]
fn the_commands_need_a_file_and_extract_takes_no_flag() {
    let dir = folder_with_the_test();

    for args in [
        &["extract-objects"][..],
        &["inject-objects"],
        &["objectify"],
        &["revert-objects"],
        &["extract-objects", "login.rs", "-c"],
        &["inject-objects", "login.rs", "--nonsense"],
    ] {
        let output = run(dir.path(), args);

        assert!(!output.status.success(), "{args:?} must fail");
        assert_eq!(
            output.status.code(),
            Some(INVALID),
            "{args:?}: {}",
            stderr(&output)
        );
    }
    assert!(!dir.path().join("page_objects.rs").exists());
    assert!(!dir.path().join("page_object.rs").exists());
}

#[test]
fn the_help_describes_each_command() {
    let dir = tempfile::tempdir().expect("temp dir");

    for (command, words) in [
        ("extract-objects", &["page_objects.rs", "FILE"][..]),
        ("inject-objects", &["page_objects::", "-c, --comments"]),
        (
            "objectify",
            &["extract-objects", "inject-objects", "-c, --comments"],
        ),
        ("revert-objects", &["byte for byte", "-c, --comments"]),
    ] {
        let help = succeeds(dir.path(), &[command, "--help"]);

        for word in words {
            assert!(
                help.contains(word),
                "{command} --help lacks {word:?}:\n{help}"
            );
        }
    }
}

#[test]
fn the_injected_test_imports_from_seleniumbase_rs_only() {
    for line in INJECTED.lines().filter(|line| line.starts_with("use ")) {
        assert!(line.starts_with("use seleniumbase_rs::"), "{line}");
    }
}
