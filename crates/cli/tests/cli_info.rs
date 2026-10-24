//! The information commands and the names Python SeleniumBase also gives them, run as the real
//! binary: `sbrs methods`, and `gui`, `case_plans` with their `PATH` and `-k FILTER`.
//!
//! None of them starts a browser. The terminal apps (`commander`, `caseplans`) take over the
//! terminal in a build with the `tui` feature, so those are only asked for their help here,
//! and, in a build without it, for the hint they print.

#[path = "support/sbrs.rs"]
mod support;

use support::{run, stderr, stdout};

fn empty_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[test]
fn methods_prints_the_common_methods_and_starts_no_browser() {
    let dir = empty_dir();

    let output = run(dir.path(), &[], &["methods"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));
    let listing = stdout(&output);
    for call in [
        "sb.open(url)",
        "sb.type_text(selector, text)",
        "sb.click(selector)",
        "sb.assert_text(text, selector)",
        "sb.wait_for_and_accept_alert()",
        "sb.assert_no_js_errors()",
    ] {
        assert!(listing.contains(call), "no {call} in:\n{listing}");
    }
    // The introduction says the calls are async and where the rest of the API is.
    assert!(listing.contains(".await"), "{listing}");
    assert!(listing.contains("docs/parity.md"), "{listing}");
}

#[test]
fn methods_is_one_row_for_each_method_with_what_it_does() {
    let dir = empty_dir();

    let listing = stdout(&run(dir.path(), &[], &["methods"]));

    let rows: Vec<&str> = listing.lines().filter(|l| l.starts_with("  sb.")).collect();
    assert!(rows.len() >= 40, "{} rows:\n{listing}", rows.len());
    for row in rows {
        let (call, what) = row
            .trim()
            .split_once("  ")
            .unwrap_or_else(|| panic!("a row has no description: {row}"));
        assert!(call.ends_with(')'), "{row}");
        assert!(!what.trim().is_empty(), "{row}");
    }
}

#[test]
fn methods_is_in_the_info_group_of_the_root_help_and_has_an_example() {
    let dir = empty_dir();

    let root = run(dir.path(), &[], &["--help"]);
    assert!(root.status.success(), "{}", stderr(&root));
    assert!(stdout(&root).contains("methods"), "{}", stdout(&root));

    let help = run(dir.path(), &[], &["methods", "--help"]);
    assert!(help.status.success(), "{}", stderr(&help));
    assert!(stdout(&help).contains("sbrs methods"), "{}", stdout(&help));
}

#[test]
fn methods_takes_no_words() {
    let dir = empty_dir();

    let output = run(dir.path(), &[], &["methods", "extra"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
}

#[test]
fn gui_is_the_commander_and_both_take_a_path_and_a_filter() {
    let dir = empty_dir();

    for name in ["commander", "gui"] {
        let help = run(dir.path(), &[], &[name, "--help"]);
        assert!(help.status.success(), "{name}: {}", stderr(&help));
        let text = stdout(&help);
        for word in ["[PATH]", "-k", "FILTER", "`tui` feature"] {
            assert!(text.contains(word), "{name} --help lacks {word}:\n{text}");
        }
    }
}

#[test]
fn the_case_plans_names_are_one_command_with_a_path_and_a_filter() {
    let dir = empty_dir();

    for name in ["caseplans", "case-plans", "case_plans"] {
        let help = run(dir.path(), &[], &[name, "--help"]);
        assert!(help.status.success(), "{name}: {}", stderr(&help));
        let text = stdout(&help);
        for word in ["[PATH]", "-k", "FILTER", "--starter"] {
            assert!(text.contains(word), "{name} --help lacks {word}:\n{text}");
        }
    }
}

#[test]
fn case_plans_with_the_starter_flag_writes_the_checklist_in_any_build() {
    let dir = empty_dir();

    let output = run(dir.path(), &[], &["case_plans", "--starter"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("Generated"), "{}", stdout(&output));
    assert!(dir.path().join("CASE_PLANS.md").is_file());
}

#[test]
fn a_filter_makes_no_sense_with_the_starter_checklist() {
    let dir = empty_dir();

    let output = run(dir.path(), &[], &["caseplans", "--starter", "-k", "login"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(!dir.path().join("CASE_PLANS.md").exists());
}

#[cfg(not(feature = "tui"))]
#[test]
fn without_the_tui_feature_every_name_of_the_terminal_apps_prints_the_hint() {
    let dir = empty_dir();

    for args in [
        &["gui"][..],
        &["gui", "tests", "-k", "login"],
        &["commander", "-k", "login"],
        &["case_plans", "-k", "login"],
        &["caseplans", "tests"],
    ] {
        let output = run(dir.path(), &[], args);

        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("built without the terminal UI"),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(stdout(&output).is_empty(), "{args:?}: {}", stdout(&output));
    }
}
