//! The Recorder Mode pipeline (`add_parent_frame_switches`, `process`,
//! `to_actions`) against SeleniumBase's own code.
//!
//! Each `tests/fixtures/recorder/NAME.raw.json` holds raw actions as the page
//! script stores them; `NAME.expected.json` is what Python's
//! `__process_recorded_actions` and `generate_sbase_code` made of them
//! (written by `gen_expected.py`, run by hand with the reference SeleniumBase;
//! it is never run here). The Rust pipeline must give the same processed
//! actions and the same calls.

use std::fs;
use std::path::{Path, PathBuf};

use seleniumbase_rs::api::recorder::{
    add_parent_frame_switches, process, to_actions, ActionRecorder, RawAction,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    start_ms: i64,
    #[serde(default)]
    origins: Vec<String>,
    #[serde(default)]
    uc: bool,
    actions: Vec<RawAction>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Call {
    name: String,
    target: Option<String>,
    value: Option<String>,
}

#[derive(Deserialize)]
struct Expected {
    processed: Vec<RawAction>,
    calls: Vec<Call>,
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recorder")
}

fn fixture_names() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixture_dir())
        .expect("the fixture folder")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".raw.json").map(str::to_owned))
        .collect();
    names.sort();
    names
}

fn load(name: &str) -> (Case, Expected) {
    let read = |suffix: &str| {
        let path = fixture_dir().join(format!("{name}.{suffix}.json"));
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    };
    let case = serde_json::from_str(&read("raw")).unwrap_or_else(|e| panic!("{name}.raw: {e}"));
    let expected =
        serde_json::from_str(&read("expected")).unwrap_or_else(|e| panic!("{name}.expected: {e}"));
    (case, expected)
}

/// The pipeline as the harvest runs it for one tab.
fn run(case: Case) -> (Vec<RawAction>, Vec<Call>) {
    let mut tab = case.actions;
    add_parent_frame_switches(&mut tab);
    let processed = process(tab, case.start_ms, &case.origins);
    let calls = to_actions(&processed, case.uc)
        .into_iter()
        .map(|action| Call {
            name: action.name,
            target: action.target,
            value: action.value,
        })
        .collect();
    (processed, calls)
}

#[test]
fn the_fixtures_are_all_there() {
    let names = fixture_names();
    assert!(names.len() >= 10, "{names:?}");
    for name in &names {
        let expected = fixture_dir().join(format!("{name}.expected.json"));
        assert!(
            expected.exists(),
            "{name} has no expected file; run gen_expected.py"
        );
    }
}

#[test]
fn the_rust_pipeline_processes_every_fixture_as_python_does() {
    for name in fixture_names() {
        let (case, expected) = load(&name);
        let (processed, _) = run(case);
        assert_eq!(processed.len(), expected.processed.len(), "{name}: rows");
        for (index, (ours, theirs)) in processed.iter().zip(&expected.processed).enumerate() {
            assert_eq!(ours, theirs, "{name}: row {index}");
        }
    }
}

#[test]
fn the_rust_pipeline_writes_the_calls_python_writes_for_every_fixture() {
    for name in fixture_names() {
        let (case, expected) = load(&name);
        let (_, calls) = run(case);
        assert_eq!(
            calls.len(),
            expected.calls.len(),
            "{name}: calls {calls:#?}"
        );
        for (index, (ours, theirs)) in calls.iter().zip(&expected.calls).enumerate() {
            assert_eq!(ours, theirs, "{name}: call {index}");
        }
    }
}

fn recording(calls: &[(&str, Option<&str>, Option<&str>)]) -> ActionRecorder {
    let mut recorder = ActionRecorder::default();
    for (name, target, value) in calls {
        recorder.record(name, *target, *value);
    }
    recorder
}

#[test]
fn the_default_output_is_one_rstest_test_that_uses_the_sb_fixture() {
    let recorder = recording(&[
        ("goto", Some("https://seleniumbase.io/demo_page"), None),
        ("type_text", Some("#myTextInput"), Some("abc\n")),
        (
            "goto_if_not_url",
            Some("https://seleniumbase.io/demo_page?x=1"),
            None,
        ),
        ("check_if_unchecked", Some("#checkBox1"), None),
        ("assert_element", Some("#myButton"), None),
        ("assert_text", Some("h1"), Some("Demo Page")),
    ]);
    let expected = "\
use rstest::rstest;
use seleniumbase_rs::testing::{sb, Sb};
use seleniumbase_rs::Result;

#[rstest]
#[tokio::test]
async fn test_recording(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.goto(\"https://seleniumbase.io/demo_page\").await?;
        sb.type_text(\"#myTextInput\", \"abc\\n\").await?;
        sb.goto_if_not_url(\"https://seleniumbase.io/demo_page?x=1\").await?;
        sb.check_if_unchecked(\"#checkBox1\").await?;
        sb.assert_element(\"#myButton\").await?;
        sb.assert_text(\"Demo Page\", \"h1\").await?;
        Ok(())
    })
    .await
}
";
    assert_eq!(recorder.to_rstest_test("test_recording", false), expected);
}

#[test]
fn uc_starts_the_test_in_cdp_mode_and_a_password_comes_from_the_environment() {
    let recorder = recording(&[
        ("goto", Some("https://a.test/login"), None),
        ("press_keys", Some("#pw"), Some("{{SB_RECORDED_SECRET}}\n")),
        ("levitate", Some("#a"), None),
    ]);
    let test = recorder.to_rstest_test("my test", true);
    assert!(
        test.contains("use seleniumbase_rs::api::recorder::recorded_secret;\n"),
        "{test}"
    );
    assert!(test.contains("async fn my_test(sb: Sb)"), "{test}");
    assert!(
        test.contains("        sb.activate_cdp_mode().await?;\n        sb.goto("),
        "{test}"
    );
    assert!(
        test.contains(
            "sb.press_keys(\"#pw\", &format!(\"{}{}\", recorded_secret()?, \"\\n\")).await?;"
        ),
        "{test}"
    );
    assert!(
        test.contains("// skipped: unknown action \"levitate\""),
        "{test}"
    );
    // No empty recording starts the browser in CDP mode for nothing.
    let empty = ActionRecorder::default().to_rstest_test("t", true);
    assert!(!empty.contains("activate_cdp_mode"), "{empty}");
}

#[test]
fn the_gherkin_scenario_is_named_and_worded_as_pythons() {
    let recorder = recording(&[
        ("goto", Some("https://a.test/"), None),
        ("click", Some("#go"), None),
        ("assert_text", Some("h1"), Some("Hello")),
    ]);
    assert_eq!(
        recorder.to_gherkin_scenario("RecorderTest", "test_recording", false),
        "Feature: RecorderTest\n\n  Scenario: test_recording\n    \
         Given Open \"https://a.test/\"\n    \
         And Click \"#go\"\n    \
         And Assert text \"Hello\" in \"h1\"\n"
    );
}

#[test]
fn a_raw_recording_becomes_an_action_recorder() {
    let (case, _) = load("basic");
    let recorder =
        ActionRecorder::from_raw_actions(case.actions, case.start_ms, &case.origins, false);
    let names: Vec<&str> = recorder.actions.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names[0], "goto");
    assert!(names.contains(&"assert_exact_text"), "{names:?}");
}
