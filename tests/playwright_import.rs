//! The Playwright importer on whole files, with no browser: every fixture in
//! `tests/fixtures/playwright/` imports with only its deliberate `TODO`s, the
//! fixtures together use every mapping, and the Rust the importer writes for
//! them compiles against this crate in a scratch crate.
//!
//! `tests/playwright_import_chrome.rs` runs the same programs on Chrome.

#[path = "support/import_fixtures.rs"]
mod import_fixtures;

use import_fixtures::{
    cargo, module_name, options, printed, read_fixture, scratch_dir, write_crate,
};
use seleniumbase_rs::utilities::importer::{
    import_playwright, ImportSeverity, ScriptImport, ScriptLanguage,
};

/// Each fixture, its language, and what its deliberate `TODO`s mention.
const FIXTURES: &[(&str, ScriptLanguage, &[&str])] = &[
    ("shop.spec.ts", ScriptLanguage::TypeScript, &["page.route"]),
    ("codegen.js", ScriptLanguage::JavaScript, &[]),
    ("test_shop.py", ScriptLanguage::Python, &["expect_popup"]),
    ("codegen_sync.py", ScriptLanguage::Python, &[]),
    ("async_api.py", ScriptLanguage::Python, &[]),
    ("failing.spec.ts", ScriptLanguage::TypeScript, &[]),
];

fn import(name: &str) -> ScriptImport {
    import_playwright(&read_fixture("playwright", name), &options(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn every_fixture_imports_with_only_its_deliberate_todos() {
    for (name, language, todos) in FIXTURES {
        let imported = import(name);
        assert_eq!(imported.language, *language, "{name}");
        assert_eq!(
            imported.diagnostics.len(),
            todos.len(),
            "{name}: {:#?}\n{}",
            imported.diagnostics,
            imported.rust
        );
        for (diagnostic, expected) in imported.diagnostics.iter().zip(todos.iter()) {
            assert_eq!(diagnostic.severity, ImportSeverity::Warning, "{name}");
            assert!(
                diagnostic.message.contains(expected),
                "{name}: {diagnostic:?}"
            );
            assert!(
                imported
                    .rust
                    .contains(&format!("//     line {}: ", diagnostic.line)),
                "{name}: the TODO names line {}",
                diagnostic.line
            );
        }
        assert_eq!(imported.is_complete(), todos.is_empty(), "{name}");
        syn::parse_file(&imported.rust)
            .unwrap_or_else(|e| panic!("{name}: {e}\n{}", imported.rust));
    }
}

#[test]
fn the_tests_are_named_from_their_titles_and_scripts_from_the_file() {
    assert_eq!(
        import("shop.spec.ts").tests,
        [
            "shop_has_a_title_and_a_heading",
            "shop_fills_in_the_order_form",
            "shop_reads_lists_links_and_attributes",
            "shop_navigates_waits_and_uses_the_keyboard_and_mouse",
            "shop_is_skipped",
            "shop_keeps_what_it_cannot_convert_as_a_todo",
        ]
    );
    assert_eq!(import("codegen.js").tests, ["codegen"]);
    assert_eq!(import("codegen_sync.py").tests, ["codegen_sync"]);
    assert_eq!(import("async_api.py").tests, ["async_api"]);
    let pytest = import("test_shop.py");
    assert_eq!(pytest.tests.len(), 6);
    assert!(pytest.rust.contains("#[ignore = \"kept skipped\"]"));
    assert!(import("shop.spec.ts")
        .rust
        .contains("#[ignore = \"skipped with test.skip in the source\"]"));
}

#[test]
fn the_fixtures_use_every_mapping() {
    let all: String = FIXTURES
        .iter()
        .map(|(name, _, _)| import(name).rust)
        .collect();
    for call in [
        // The page.
        ".goto(",
        "page.reload()",
        "page.back()",
        "page.forward()",
        "page.expect().to_contain_url(",
        "page.wait_for_load()",
        "tokio::time::sleep(",
        ".wait_for(State::Visible)",
        "page.window().set_bounds(",
        "page.screenshot()",
        "page.title()",
        "page.url()",
        "page.evaluate(",
        "page.keyboard().type_text(",
        "page.keyboard().press(Key::",
        "page.mouse().click(Point::new(",
        // Locators.
        "SelectorBuf::role(",
        "SelectorBuf::text(",
        "SelectorBuf::label(",
        "SelectorBuf::has_text(",
        "[placeholder*=",
        "[data-testid=",
        "[alt*=",
        "[title*=",
        "[id=",
        ".first()",
        ".last()",
        ".nth(",
        // Actions.
        ".click()",
        ".double_click()",
        ".right_click()",
        ".fill(",
        ".type_text(",
        ".press(Key::",
        ".check()",
        ".uncheck()",
        ".set_checked(true)",
        "support::select_option(",
        "SelectBy::Text(",
        "SelectBy::Index(",
        "SelectBy::Value(",
        ".hover()",
        ".focus()",
        ".clear()",
        "document.activeElement.blur()",
        "support::set_input_files(",
        // Reads.
        ".text().await?",
        "support::field_value(",
        ".attribute(",
        ".is_visible().await?",
        ".is_checked().await?",
        ".count().await?",
        // Expectations.
        ".to_be_visible()",
        ".to_be_hidden()",
        ".to_have_text(",
        ".to_contain_text(",
        "support::expect_field_value(",
        ".to_have_attribute(",
        ".to_be_checked()",
        ".not().to_be_checked()",
        "support::expect_enabled(",
        ".to_have_count(",
        "page.expect().to_have_title(",
        "page.expect().to_contain_title(",
        "page.expect().to_have_url(",
        "assert_eq!(",
        "assert!(",
        "println!(",
        "// TODO(playwright): ",
    ] {
        assert!(all.contains(call), "no fixture writes {call}");
    }
}

/// The generated programs compile against this crate: the passing ones in
/// one test target, the one that must fail in another.
#[test]
fn the_generated_programs_compile() {
    let dir = scratch_dir("playwright");
    let passing: Vec<(String, String)> = FIXTURES
        .iter()
        .filter(|(name, _, _)| !name.starts_with("failing"))
        .map(|(name, _, _)| (module_name(name), import(name).rust))
        .collect();
    let failing = vec![(
        module_name("failing.spec.ts"),
        import("failing.spec.ts").rust,
    )];
    write_crate(
        &dir,
        "playwright-import",
        &[("imported", passing), ("failing", failing)],
    );
    let output = cargo(&dir, &["test", "--no-run", "--tests"], &[]);
    assert!(
        output.status.success(),
        "the generated tests do not compile:\n{}",
        printed(&output)
    );
}
