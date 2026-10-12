//! The program that `ActionRecorder::to_cdp_script` writes compiles against the
//! real `sb_cdp` API.
//!
//! `golden_rust/cdp_script.rs` is the program the generator writes for a
//! recording of every action it supports. A unit test in
//! `src/api/recorder/cdp.rs` keeps the file equal to the generator's output
//! (run it with `SB_UPDATE_GOLDEN=1` to write the file again). Including it
//! here as a module makes the compiler check every call in it: that the method
//! exists on that type, takes those arguments, and that each import is used and
//! exported by `seleniumbase_rs::sb_cdp`. Nothing runs it: it would start a
//! browser.

// The program's `main` is never called.
#![allow(dead_code)]

// `include!`, not `mod`: rustfmt follows a `mod` and would rewrap the generated
// lines, which the generator does not.
mod generated {
    include!("golden_rust/cdp_script.rs");
}

#[test]
fn the_generated_program_is_compiled_into_this_test() {
    let source = include_str!("golden_rust/cdp_script.rs");
    assert!(source.starts_with("use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions"));
    assert!(source.contains("page.locator(\"#go\").click().await?;"));
}
