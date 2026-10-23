//! The starter file that `sbrs objectify` writes, compiled
//! against the real `BaseCase` API.
//!
//! The templates are Rust source kept in `src/cli/scripts/templates/`. Including
//! them here as modules means that renaming or re-typing a `BaseCase` method
//! they call breaks this build, instead of silently breaking the files that
//! users generate.

#![allow(dead_code)]

/// `sbrs objectify`: a page object wrapping a `BaseCase`.
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

    let _ = page_object::usage;
}
