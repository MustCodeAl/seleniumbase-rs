//! The Rust the Playwright importer writes, run on a real headless Chrome.
//!
//! Every fixture in `tests/fixtures/playwright/` is imported against a
//! loopback shop page, the generated tests are built in a scratch crate that
//! depends on this one, and `cargo test` runs them with `SB_HEADLESS=true`.
//! They must pass; the one fixture whose expectation cannot hold must fail,
//! which shows the converted expectations really check the page.
//!
//! Ignored by default (it launches Chrome and builds a crate). Run it alone:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test playwright_import_chrome -- --ignored --test-threads=1
//! ```

#[path = "support/import_fixtures.rs"]
mod import_fixtures;

use import_fixtures::{
    cargo, module_name, options, printed, read_fixture, results, scratch_dir, serve, write_crate,
    PLACEHOLDER,
};
use seleniumbase_rs::utilities::importer::import_playwright;

const PASSING: &[&str] = &[
    "shop.spec.ts",
    "codegen.js",
    "test_shop.py",
    "codegen_sync.py",
    "async_api.py",
];

fn import_against(base: &str, name: &str) -> (String, String) {
    let source = read_fixture("playwright", name).replace(PLACEHOLDER, base);
    let imported =
        import_playwright(&source, &options(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    (module_name(name), imported.rust)
}

#[test]
#[ignore = "launches a real Chrome and builds a scratch crate"]
fn the_imported_playwright_tests_pass_on_chrome_and_a_failing_one_fails() {
    let base = serve();
    let dir = scratch_dir("playwright");
    let passing: Vec<_> = PASSING
        .iter()
        .map(|name| import_against(&base, name))
        .collect();
    let failing = vec![import_against(&base, "failing.spec.ts")];
    write_crate(
        &dir,
        "playwright-import",
        &[("imported", passing), ("failing", failing)],
    );
    let headless = [("SB_HEADLESS", "true")];

    let run = cargo(
        &dir,
        &["test", "--test", "imported", "--", "--test-threads=1"],
        &headless,
    );
    let text = printed(&run);
    let (passed, failed, ignored) = results(&text);
    assert!(run.status.success(), "the imported tests failed:\n{text}");
    assert_eq!(failed, 0, "{text}");
    // shop.spec.ts has 5 tests that run and one skipped, test_shop.py 5 and
    // one skipped, and each script one.
    assert_eq!((passed, ignored), (13, 2), "{text}");

    let fail = cargo(
        &dir,
        &["test", "--test", "failing", "--", "--test-threads=1"],
        &headless,
    );
    let text = printed(&fail);
    assert!(
        !fail.status.success(),
        "the failing expectation passed:\n{text}"
    );
    assert_eq!(results(&text).1, 1, "{text}");
    assert!(
        text.contains("Not the heading") && text.contains("Welcome to the shop"),
        "the failure says what was expected and what was seen:\n{text}"
    );
}
