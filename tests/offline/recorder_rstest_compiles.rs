//! The test that `ActionRecorder::to_rstest_test` writes compiles against the
//! real `sb` fixture and `BaseCase` API (feature `testing`).
//!
//! `golden_rust/rstest_recording.rs` is the test the generator writes for the
//! recordings in `tests/fixtures/recorder`, run through the Recorder Mode
//! pipeline, plus a typed password. A test here keeps the file equal to the
//! generator's output (run it with `SB_UPDATE_GOLDEN=1` to write the file
//! again). The doctest `RstestRecordingCompiles` in `src/api/recorder.rs` compiles the
//! file as a user's crate would: the compiler checks that each method exists on
//! `BaseCase`, takes those arguments, and that the imports exist. It is `no_run`: the
//! test would start a browser.

#![cfg(feature = "testing")]

use std::fs;
use std::path::Path;

use seleniumbase_rs::api::recorder::{ActionRecorder, RawAction, RecordedAction};

fn recording() -> ActionRecorder {
    let mut actions: Vec<RecordedAction> = Vec::new();
    for name in [
        "basic",
        "checkbox",
        "search",
        "api_actions",
        "canvas_hover_drag",
        "js_enter",
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/recorder")
            .join(format!("{name}.raw.json"));
        let text = fs::read_to_string(&path).expect("a recorder fixture");
        let case: serde_json::Value = serde_json::from_str(&text).expect("fixture JSON");
        let raw: Vec<RawAction> = serde_json::from_value(case["actions"].clone()).expect("actions");
        let start = case["start_ms"].as_i64().expect("start_ms");
        actions.extend(ActionRecorder::from_raw_actions(raw, start, &[], false).actions);
    }
    actions.push(RecordedAction::new(
        "type_text",
        Some("#password"),
        Some("{{SB_RECORDED_SECRET}}\n"),
    ));
    ActionRecorder { actions }
}

#[test]
fn the_golden_file_is_what_the_generator_writes() {
    let generated = recording().to_rstest_test("test_recording", false);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden_rust/rstest_recording.rs");
    if std::env::var_os("SB_UPDATE_GOLDEN").is_some() {
        fs::write(&path, &generated).expect("write the golden file");
    }
    let on_disk = fs::read_to_string(&path).expect("the golden file");
    assert_eq!(
        on_disk,
        generated,
        "run with SB_UPDATE_GOLDEN=1 to write {} again",
        path.display()
    );
    assert!(generated.contains("use seleniumbase_rs::testing::{rstest, sb, Sb};"));
    assert!(!generated.contains("use rstest::"), "{generated}");
    assert!(generated.contains("sb.run(async |sb| {"));
    assert!(generated.contains("recorded_secret()?"));
}
