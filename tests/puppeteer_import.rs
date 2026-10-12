//! The Puppeteer importer on whole files, with no browser: every fixture in
//! `tests/fixtures/puppeteer/` (a plain script, a jest-puppeteer test, a Mocha
//! test with chai, a Chrome Recorder export) imports with only its deliberate
//! `TODO`s, and the Rust it writes compiles against this crate in a scratch
//! crate.
//!
//! `tests/puppeteer_import_chrome.rs` runs the same programs on Chrome.

#[path = "support/import_fixtures.rs"]
mod import_fixtures;

use import_fixtures::{
    cargo, module_name, options, printed, read_fixture, scratch_dir, write_crate,
};
use seleniumbase_rs::utilities::importer::{
    import_puppeteer, ImportSeverity, ScriptImport, ScriptLanguage,
};

/// Each fixture, its language, and what its deliberate `TODO`s mention.
const FIXTURES: &[(&str, ScriptLanguage, &[&str])] = &[
    (
        "script.js",
        ScriptLanguage::JavaScript,
        &["an if statement"],
    ),
    (
        "shop.test.ts",
        ScriptLanguage::TypeScript,
        &["setRequestInterception"],
    ),
    ("mocha.test.js", ScriptLanguage::JavaScript, &[]),
    (
        "recording.json",
        ScriptLanguage::RecorderJson,
        &["emulateNetworkConditions"],
    ),
    ("failing.test.js", ScriptLanguage::JavaScript, &[]),
];

fn import(name: &str) -> ScriptImport {
    import_puppeteer(&read_fixture("puppeteer", name), &options(name))
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
        }
        assert!(
            imported.rust.contains("// Imported from Puppeteer code"),
            "{name}"
        );
        syn::parse_file(&imported.rust)
            .unwrap_or_else(|e| panic!("{name}: {e}\n{}", imported.rust));
    }
}

#[test]
fn tests_are_named_from_describe_and_it_and_the_recording_from_its_title() {
    assert_eq!(
        import("shop.test.ts").tests,
        [
            "shop_jest_puppeteer_shows_the_heading",
            "shop_jest_puppeteer_orders_with_the_locator_api_and_element_handles",
            "shop_jest_puppeteer_navigates",
            "shop_jest_puppeteer_keeps_request_interception_as_a_todo",
        ]
    );
    assert_eq!(
        import("mocha.test.js").tests,
        ["shop_mocha_reads_the_page", "shop_mocha_submits_the_order"]
    );
    assert_eq!(import("script.js").tests, ["script"]);
    assert_eq!(import("recording.json").tests, ["recorder_order"]);
}

#[test]
fn the_fixtures_use_every_mapping() {
    let all: String = FIXTURES
        .iter()
        .map(|(name, _, _)| import(name).rust)
        .collect();
    for call in [
        "page.goto(",
        "page.reload()",
        "page.back()",
        "page.forward()",
        "page.wait_for_load()",
        "page.wait_for_function(",
        ".wait_for(State::Visible)",
        ".wait_for(State::Present)",
        "tokio::time::sleep(",
        "page.window().set_bounds(",
        "page.emulation().user_agent(",
        "page.screenshot()",
        "page.title()",
        "page.content()",
        "page.evaluate(",
        ".click()",
        ".type_text(",
        ".focus()",
        ".hover()",
        "SelectBy::Value(",
        "page.keyboard().press(Key::",
        "page.mouse().click(Point::new(",
        "let gift = page.locator(\"#gift\");",
        "support::set_input_files(&upload,",
        ".fill(",
        ".count().await?",
        ".text().await?",
        ".attribute(\"href\")",
        ".is_checked().await?",
        "SelectorBuf::role(\"link\", Some(\"About us\"), true)",
        "SelectorBuf::text(",
        "SelectorBuf::has_text(",
        "page.locator(\"//h1\")",
        ".expect().to_exist()",
        ".to_contain_text(",
        "assert_eq!(",
        "assert_ne!(",
        "assert!(",
        "println!(",
        "support::change(",
        "page.scroll(Scroll::To(",
        "// TODO(puppeteer): ",
    ] {
        assert!(all.contains(call), "no fixture writes {call}");
    }
}

#[test]
fn the_generated_programs_compile() {
    let dir = scratch_dir("puppeteer");
    let passing: Vec<(String, String)> = FIXTURES
        .iter()
        .filter(|(name, _, _)| !name.starts_with("failing"))
        .map(|(name, _, _)| (module_name(name), import(name).rust))
        .collect();
    let failing = vec![(
        module_name("failing.test.js"),
        import("failing.test.js").rust,
    )];
    write_crate(
        &dir,
        "puppeteer-import",
        &[("imported", passing), ("failing", failing)],
    );
    let output = cargo(&dir, &["test", "--no-run", "--tests"], &[]);
    assert!(
        output.status.success(),
        "the generated tests do not compile:\n{}",
        printed(&output)
    );
}
