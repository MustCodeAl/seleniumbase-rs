//! A test binary that reaches the library only through the `sb` fixture, as a
//! user's scenario does, still has the built-in steps.
//!
//! The steps register through `inventory` from object files of the library,
//! and nothing in this file names them: launching the fixture's case is what
//! keeps them linked (`steps::link`). It is a binary of its own because the
//! other test file names the `steps` module and would link it by itself.
#![cfg(feature = "bdd")]

use rstest_bdd::{find_step, StepKeyword};

#[test]
fn a_binary_that_only_uses_the_fixture_finds_the_builtin_steps() {
    // The way a scenario function reaches the library.
    std::hint::black_box(seleniumbase_rs::testing::bdd::sb::default);

    for keyword in [StepKeyword::Given, StepKeyword::When, StepKeyword::Then] {
        for text in [
            "Open \"https://example.com\"",
            "User clicks 'button#go'",
            "Assert text \"Welcome\" in \"h1\"",
            "Process deferred asserts",
            "Switch to default content",
            "Save screenshot to logs",
        ] {
            assert!(
                find_step(keyword, text.into()).is_some(),
                "{keyword:?} {text}"
            );
        }
    }
}
