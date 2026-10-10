//! The starter files that `sbase mkrec` and `sbase objectify` write, compiled
//! against the real `BaseCase` API.
//!
//! The templates are Rust source kept in `src/cli/scripts/templates/`. Including
//! them here as modules means that renaming or re-typing a `BaseCase` method
//! they call breaks this build, instead of silently breaking the files that
//! users generate.

#![allow(dead_code)]

/// `sbase mkrec`: a `#[tokio::main]` program.
mod starter_recording {
    include!("../src/cli/scripts/templates/starter_recording.tpl");

    /// The template's `main` is private to this module; this names it.
    pub fn entry_point() {
        let _ = main;
    }
}

/// `sbase objectify`: a page object wrapping a `BaseCase`.
mod page_object {
    include!("../src/cli/scripts/templates/page_object.tpl");

    /// Uses the page object the way its owner would.
    pub async fn usage(
        sb: &mut seleniumbase_rs::BaseCase,
    ) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
        LoginPage::new(sb).open().await
    }
}

#[test]
fn the_templates_compile_against_the_real_api() {
    // The check is that this file builds: the templates are included above as
    // modules, so a renamed or re-typed `BaseCase` method they call fails the
    // build. Nothing is run, because a `BaseCase` needs a live browser session
    // and the page object's `open` would drive it. Naming the entry points
    // ties the test to them.
    let _ = starter_recording::entry_point;
    let _ = page_object::usage;
}
