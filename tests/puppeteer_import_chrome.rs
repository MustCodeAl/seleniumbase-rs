//! The Rust the Puppeteer importer writes, run on a real headless Chrome.
//!
//! Every fixture in `tests/fixtures/puppeteer/` (a plain script, a
//! jest-puppeteer test, a Mocha test, a Chrome Recorder export) is imported
//! against a loopback shop page, built in a scratch crate that depends on this
//! one, and run with `SB_HEADLESS=true`. They must pass; the fixture whose
//! check cannot hold must fail.
//!
//! Ignored by default (it launches Chrome and builds a crate). Run it alone:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test puppeteer_import_chrome -- --ignored --test-threads=1
//! ```

#[path = "support/import_fixtures.rs"]
mod import_fixtures;

use import_fixtures::{
    cargo, module_name, options, printed, read_fixture, results, scratch_dir, serve, write_crate,
    PLACEHOLDER,
};
use seleniumbase_rs::utilities::importer::import_puppeteer;

const PASSING: &[&str] = &[
    "script.js",
    "shop.test.ts",
    "mocha.test.js",
    "recording.json",
];

fn import_against(base: &str, name: &str) -> (String, String) {
    let source = read_fixture("puppeteer", name).replace(PLACEHOLDER, base);
    let imported =
        import_puppeteer(&source, &options(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    (module_name(name), imported.rust)
}

#[test]
#[ignore = "launches a real Chrome and builds a scratch crate"]
fn the_imported_puppeteer_tests_pass_on_chrome_and_a_failing_one_fails() {
    let base = serve();
    let dir = scratch_dir("puppeteer");
    let passing: Vec<_> = PASSING
        .iter()
        .map(|name| import_against(&base, name))
        .collect();
    let failing = vec![import_against(&base, "failing.test.js")];
    write_crate(
        &dir,
        "puppeteer-import",
        &[("imported", passing), ("failing", failing)],
    );
    let headless = [("SB_HEADLESS", "true")];

    let run = cargo(
        &dir,
        &["test", "--test", "imported", "--", "--test-threads=1"],
        &headless,
    );
    let text = printed(&run);
    let (passed, failed, _) = results(&text);
    assert!(run.status.success(), "the imported tests failed:\n{text}");
    assert_eq!(failed, 0, "{text}");
    // The script and the recording are one test each, jest-puppeteer 4, Mocha 2.
    assert_eq!(passed, 8, "{text}");

    let fail = cargo(
        &dir,
        &["test", "--test", "failing", "--", "--test-threads=1"],
        &headless,
    );
    let text = printed(&fail);
    assert!(!fail.status.success(), "the failing check passed:\n{text}");
    assert_eq!(results(&text).1, 1, "{text}");
    assert!(
        text.contains("Not the title") && text.contains("Shop Fixture"),
        "the failure shows both values:\n{text}"
    );
}
