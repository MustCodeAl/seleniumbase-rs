//! The documentation names only what exists.
//!
//! The docs, the READMEs and the files written for coding assistants are read by
//! people and models who copy from them. This test fails when one of them names an
//! API, a crate, a feature or an install line that the crate no longer has. The
//! change logs and the design records are exempt: they describe the past on
//! purpose.
//!
//! When something is removed or renamed, add its old spelling here.

use std::fs;
use std::path::{Path, PathBuf};

/// Spellings that no current page may contain, each with what replaced it.
const GONE: &[(&str, &str)] = &[
    // Fingerprint API v1 (0.5.0 replaced it with one Spoof per setting).
    ("MaskingMode", "Spoof<T> per setting"),
    ("NoiseMode", "CanvasNoise / AudioNoise in StealthPolicy"),
    ("PopupMode", "the geolocation setting (Value or Off)"),
    ("CanvasNoiseMode", "CanvasNoise"),
    ("fills_defaults", "the Spoof state of each setting"),
    ("with_flags(", "policy(..) / with_policy(..)"),
    (".flags(", "policy(..)"),
    ("canvas_seed(", "seed(..) and StealthPolicy canvas noise"),
    // Pure CDP pages (0.5.0: new_page() is blank, new_page_at(url) opens one).
    ("new_page(Some(", "new_page_at(url)"),
    ("new_page(None", "new_page()"),
    // Settings (0.5.0 split load).
    ("Settings::load(None", "Settings::load() / load_from(path)"),
    // The stealth crates moved out and were renamed (0.3.0, then codec/mimic, then codec/wire).
    ("seleniumbase_rs_mimic", "seleniumbase_rs_wire"),
    ("seleniumbase-rs-mimic", "seleniumbase-rs-wire"),
    ("seleniumbase_rs_impersonate", "seleniumbase_rs_wire"),
    ("seleniumbase-rs-impersonate", "seleniumbase-rs-wire"),
    ("crates/mimic", "crates/wire"),
    ("crates/impersonate", "crates/wire"),
    (
        "features = [\"scrape\"",
        "the seleniumbase-rs-stealth repository",
    ),
    (
        "features = [\"impersonate\"",
        "the seleniumbase-rs-stealth repository",
    ),
    // The library's `stealth` module is `evasion` (the facade crate keeps its name).
    ("seleniumbase_rs::stealth", "seleniumbase_rs::evasion"),
    (
        "seleniumbase_rs_stealth::stealth",
        "seleniumbase_rs_stealth::evasion",
    ),
    ("crate::stealth", "crate::evasion"),
    ("src/stealth", "src/evasion"),
    // The library has no Playwright engine any more: the bridge is the crate
    // `seleniumbase-rs-playwright` (`crates/playwright`), which re-exports rustwright.
    (
        "--features rustwright",
        "the crate seleniumbase-rs-playwright",
    ),
    (
        "--features playwright",
        "the crate seleniumbase-rs-playwright",
    ),
    (
        "--features \"playwright",
        "the crate seleniumbase-rs-playwright",
    ),
    (
        "features = [\"rustwright\"",
        "the crate seleniumbase-rs-playwright",
    ),
    (
        "features = [\"playwright\"",
        "the crate seleniumbase-rs-playwright",
    ),
    (
        "activate_playwright_mode",
        "seleniumbase_rs_playwright::PlaywrightSidecar",
    ),
    (
        "playwright_session()",
        "seleniumbase_rs_playwright::PlaywrightSidecar",
    ),
    (
        "browser::playwright",
        "seleniumbase_rs_playwright::PlaywrightSidecar",
    ),
    (
        "api::playwright",
        "seleniumbase_rs_playwright::PlaywrightSidecar",
    ),
    (
        "launch_playwright_headless",
        "Browser::launch, then PlaywrightSidecar::attach",
    ),
    (
        "StealthyPlaywright",
        "Browser::launch, then PlaywrightSidecar::attach(&page)",
    ),
    // The bridge's session and launcher were replaced by the sidecar (the design note
    // docs/design/playwright_basecase.md): `PlaywrightSession` is `PlaywrightSidecar` and
    // `StealthyPlaywright` is `Browser::launch`, then `PlaywrightSidecar::attach`.
    (
        "playwright_mut(",
        "Browser::new_page, then PlaywrightSidecar::attach",
    ),
    // The crate is not on crates.io: it is installed from git.
    (
        "seleniumbase-rs = { version",
        "seleniumbase-rs = { git = \"https://github.com/MustCodeAl/seleniumbase-rs\" }",
    ),
    // The binaries are the package seleniumbase-rs-cli since 0.5.0.
    (
        "cargo install seleniumbase-rs --",
        "cargo install ... seleniumbase-rs-cli",
    ),
    (
        "--bin sbrs --features mcp-server",
        "the seleniumbase-rs-cli package",
    ),
    // The deprecated items of 0.5.x, deleted in 0.6.0.
    (
        "common::decorators",
        "utilities::retry::{RetryPolicy, RateLimiter}",
    ),
    ("common::exceptions", "SeleniumBaseError"),
    ("retry_on_exception_async", "RetryPolicy::retry"),
    ("rate_limited", "utilities::retry::RateLimiter"),
    ("StealthFlags", "StealthPolicy"),
    ("mask_all", "StealthPolicy::max"),
    ("all_custom", "StealthPolicy::max"),
    ("cdp_overrides", "safe_cdp_overrides"),
    ("profile_payloads::Flags", "profile_payloads::ProfileFlags"),
    (
        "profile_payloads::ProxyConfig",
        "profile_payloads::ProfileProxy",
    ),
    ("PlaywrightSession", "PlaywrightSidecar"),
    (".with_page(", "PlaywrightSidecar::run"),
    (
        "launch_headless_with_fingerprint",
        "Browser::launch with LaunchOptionsBuilder::fingerprint",
    ),
    (
        "launch_with_fingerprint",
        "Browser::launch with LaunchOptionsBuilder::fingerprint",
    ),
    (
        "run_browser_test_boxed",
        "run_browser_test(.., async |sb| ..)",
    ),
    ("BrowserTestFuture", "an async closure"),
    (
        "with_fast_mode",
        "nothing: Python reads --fast only in CDP mode",
    ),
    ("pause(tiny: bool)", "pause_for(PauseKind)"),
    ("RunSettings::pause", "RunSettings::pause_for"),
    ("message_seconds", "RunSettings::message_duration"),
    ("DEFAULT_MESSAGE_SECONDS", "DEFAULT_MESSAGE_DURATION"),
    ("cdp_type_text", "cdp_insert_text"),
    ("cdp_click_element", "cdp_click"),
    ("save_saved_cookies", "save_cookies"),
    ("load_saved_cookies", "load_cookies"),
    ("qa_report_markdown", "MasterQaSession::to_markdown"),
    ("Settings::from_toml", "Settings::from_file"),
    ("Locator::click_at", "click_with_offset(x, y, false)"),
    ("assert_visible!", "sb_assert_visible!"),
    // Third-party crates the docs import through `seleniumbase_rs` (the "Imports and
    // dependencies" section of docs/tutorials/getting_started.md). The
    // pages that must still name them are in `ALLOWED`.
    ("use thirtyfour", "seleniumbase_rs::drivers"),
    ("thirtyfour::prelude::By", "seleniumbase_rs::drivers::By"),
    // The WebDriver engine hands out `Element` and takes selector strings, not `By`
    // (docs/design/element_type.md). `drivers::WebElement` stays for the escape hatch of
    // `Element::as_webdriver_element`, which the pages may name.
    ("use seleniumbase_rs::drivers::By", "a selector string"),
    ("drivers::{By, WebElement}", "seleniumbase_rs::Element"),
    ("Selector::to_by", "a selector string, or SelectorBuf"),
    ("Selector::auto_by", "a selector string, or SelectorBuf"),
    // The cookie is one type of ours, the same in both engines (docs/design/cookie_type.md).
    ("drivers::Cookie", "seleniumbase_rs::Cookie"),
    ("thirtyfour::cookie::Cookie", "seleniumbase_rs::Cookie"),
    ("get_shadow_root", "find_element(\"host::shadow inner\")"),
    ("BaseCase::locator", "BaseCase::find_element"),
    // `Key` has one meaning: the pure CDP engine's, at the crate root. thirtyfour's is not
    // re-exported (docs/design/element_type.md).
    ("drivers::Key", "seleniumbase_rs::Key"),
    ("DesiredCapabilities, Key", "seleniumbase_rs::Key"),
    (
        "use rstest::",
        "seleniumbase_rs::testing::{fixture, rstest}",
    ),
    ("use rstest_bdd::", "seleniumbase_rs::testing::bdd"),
    ("use rstest_bdd_macros", "seleniumbase_rs::testing::bdd"),
    ("use serde_json", "seleniumbase_rs::json"),
    ("serde_json::json!(", "seleniumbase_rs::json::json!"),
    ("use tokio", "seleniumbase_rs::tokio"),
    (
        "use async_trait",
        "seleniumbase_rs::plugins::observer::async_trait",
    ),
];

/// Pages that may keep a spelling of `GONE` because they cannot import it from
/// `seleniumbase_rs`, each with its reason (the table "The lines that stay" in
/// `docs/tutorials/getting_started.md` says the same):
///
/// - the plugin pages describe crates that depend on `seleniumbase-rs-plugin`, never on
///   this crate, so `serde_json`, `tokio` and `async_trait` come from their own manifest;
/// - the stealth pages are the API of another repository (`seleniumbase-rs-stealth`);
/// - the contributor pages show the library's own code, which depends on all of them.
const ALLOWED: &[(&str, &[&str])] = &[
    (
        "use serde_json",
        &[
            "docs/plugins/",
            "docs/stealth/api/",
            "docs/scraping/stealthy/",
            "docs/scraping/advanced-stealthy/",
            "docs/stealth/advanced/mitm-proxy.md",
            "docs/stealth/advanced/wire-formats.md",
            "docs/contributing/",
            "docs/DEVELOPER_GUIDE.md",
            "docs/CONTRIBUTING.md",
            "CONTRIBUTING.md",
        ],
    ),
    ("serde_json::json!(", &["docs/scraping/stealthy/"]),
    ("use async_trait", &["docs/plugins/"]),
    ("use tokio", &["docs/plugins/", "docs/ecosystem/"]),
    (
        "use rstest::",
        &[
            "docs/contributing/",
            "docs/DEVELOPER_GUIDE.md",
            "docs/CONTRIBUTING.md",
            "CONTRIBUTING.md",
        ],
    ),
];

/// Whether the page `rel` may keep the spelling `gone`.
fn allowed(rel: &str, gone: &str) -> bool {
    ALLOWED
        .iter()
        .any(|(spelling, pages)| *spelling == gone && pages.iter().any(|p| rel.starts_with(p)))
}

/// Files and folders that describe the past on purpose.
const EXEMPT: &[&str] = &[
    "CHANGELOG.md",
    "docs/changelog.md",
    "docs/design/",
    "docs/adr/",
];

/// Whether `line` names `gone`.
///
/// A name made of identifier characters must not be part of a longer
/// identifier: `cdp_overrides` is gone, `safe_cdp_overrides` is not. An entry
/// that starts or ends with punctuation, such as `with_flags(`, is plain text.
fn names(line: &str, gone: &str) -> bool {
    let is_ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let starts_ident = gone.chars().next().is_some_and(is_ident);
    let ends_ident = gone.chars().next_back().is_some_and(is_ident);
    line.match_indices(gone).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + gone.len()..].chars().next();
        !(starts_ident && before.is_some_and(is_ident))
            && !(ends_ident && after.is_some_and(is_ident))
    })
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn markdown_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if !matches!(
                name.as_ref(),
                "target" | "node_modules" | "book" | ".git" | ".claude"
            ) {
                markdown_under(&path, out);
            }
        } else if name.ends_with(".md") || name.ends_with(".mdx") {
            out.push(path);
        }
    }
}

fn pages() -> Vec<PathBuf> {
    let root = root();
    let mut pages = Vec::new();
    for top in [
        "README.md",
        "AGENTS.md",
        "CLAUDE.md",
        "CONTRIBUTING.md",
        "COPILOT.md",
        "ROADMAP.md",
    ] {
        let path = root.join(top);
        if path.exists() {
            pages.push(path);
        }
    }
    for dir in ["docs", "examples", "contrib", "crates"] {
        markdown_under(&root.join(dir), &mut pages);
    }
    pages
}

#[test]
fn the_docs_name_no_removed_api_crate_feature_or_install_line() {
    let root = root();
    let mut found = Vec::new();
    for page in pages() {
        let rel = page
            .strip_prefix(&root)
            .unwrap_or(&page)
            .to_string_lossy()
            .replace('\\', "/");
        if is_exempt(&rel) {
            continue;
        }
        let text = fs::read_to_string(&page).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            for (gone, now) in GONE {
                if names(line, gone) && !allowed(&rel, gone) {
                    found.push(format!(
                        "{rel}:{}: `{gone}` is gone; now: {now}",
                        number + 1
                    ));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "outdated names in the docs:\n{}",
        found.join("\n")
    );
}

/// Whether the page `rel` is one of the files that describe the past on purpose.
fn is_exempt(rel: &str) -> bool {
    EXEMPT
        .iter()
        .any(|e| rel == *e || (e.ends_with('/') && rel.starts_with(e)))
}

/// Whether `line` holds an import group `drivers::{...}` that lists `item` by name.
///
/// An entry of `GONE` is one spelling, so it cannot see `drivers::{Key, Cookie}`.
fn drivers_group_lists(line: &str, item: &str) -> bool {
    const OPEN: &str = "drivers::{";
    line.match_indices(OPEN).any(|(at, _)| {
        let rest = &line[at + OPEN.len()..];
        let group = rest.split('}').next().unwrap_or(rest);
        group.split(',').any(|name| name.trim() == item)
    })
}

#[test]
fn a_grouped_import_from_drivers_is_read_name_by_name() {
    let listed = "use seleniumbase_rs::drivers::{DesiredCapabilities, Key, Cookie, ...};";
    assert!(drivers_group_lists(listed, "Cookie"));
    assert!(drivers_group_lists("`drivers::{Cookie}`", "Cookie"));
    assert!(!drivers_group_lists(
        "use seleniumbase_rs::drivers::{DesiredCapabilities, Key};",
        "Cookie"
    ));
    assert!(!drivers_group_lists("drivers::{CookieJar}", "Cookie"));
    assert!(!drivers_group_lists(
        "use seleniumbase_rs::{Cookie, SameSite};",
        "Cookie"
    ));
}

#[test]
fn no_page_imports_the_removed_drivers_cookie_in_a_group() {
    let root = root();
    let mut found = Vec::new();
    for page in pages() {
        let rel = page
            .strip_prefix(&root)
            .unwrap_or(&page)
            .to_string_lossy()
            .replace('\\', "/");
        if is_exempt(&rel) {
            continue;
        }
        let text = fs::read_to_string(&page).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            if drivers_group_lists(line, "Cookie") {
                found.push(format!(
                    "{rel}:{}: `drivers::{{.., Cookie, ..}}` is gone; now: seleniumbase_rs::Cookie",
                    number + 1
                ));
            }
        }
    }
    assert!(
        found.is_empty(),
        "outdated names in the docs:\n{}",
        found.join("\n")
    );
}

#[test]
fn a_name_is_not_found_inside_a_longer_identifier() {
    assert!(names("`cdp_overrides` is gone", "cdp_overrides"));
    assert!(!names("use safe_cdp_overrides;", "cdp_overrides"));
    assert!(!names("RunSettings::pause_for(kind)", "RunSettings::pause"));
    assert!(names("RunSettings::pause(true)", "RunSettings::pause"));
    assert!(names("builder.with_flags(x)", "with_flags("));
    assert!(names("sb.assert_visible!(a)", "assert_visible!"));
    assert!(!names("sb_assert_visible!(a)", "assert_visible!"));
}

#[test]
fn a_page_keeps_a_third_party_import_only_where_it_must() {
    // A plugin crate has no `seleniumbase_rs` to import from; a tutorial has.
    assert!(allowed("docs/plugins/api/handles.md", "use serde_json"));
    assert!(!allowed("docs/tutorials/cdp_mode.md", "use serde_json"));
    // Nothing allows a `use thirtyfour` anywhere.
    assert!(!allowed("docs/plugins/overview.md", "use thirtyfour"));
    // The import is what is gone; the type's name in prose is not.
    assert!(names("use thirtyfour::By;", "use thirtyfour"));
    assert!(!names(
        "`thirtyfour::WebElement` is the type",
        "use thirtyfour"
    ));
    assert!(names("use rstest::rstest;", "use rstest::"));
    assert!(!names("use rstest_bdd_macros::scenario;", "use rstest::"));
    assert!(names(
        "use rstest_bdd_macros::scenario;",
        "use rstest_bdd_macros"
    ));
    assert!(names("use rstest_bdd::skip;", "use rstest_bdd::"));
}

#[test]
fn the_list_covers_the_pages_people_read() {
    let pages = pages();
    assert!(pages.iter().any(|p| p.ends_with("README.md")));
    assert!(pages
        .iter()
        .any(|p| p.to_string_lossy().contains("docs/tutorials")));
    assert!(pages.len() > 50, "only {} pages found", pages.len());
}

/// The quick start of the README runs against https://seleniumbase.io/simple/login, whose
/// "Sign in" control is the link `#log-in`: the page has no `<button>`, so a quick start that
/// clicked `locator("button")` stopped with `ElementNotFound` on a new user's first run.
#[test]
fn the_readme_quick_start_clicks_the_sign_in_link_the_page_has() {
    let readme = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"))
        .expect("README.md");
    assert!(
        readme.contains("page.locator(\"#log-in\").click()"),
        "the quick start clicks #log-in"
    );
    assert!(
        !readme.contains("page.locator(\"button\")"),
        "the login page has no <button>"
    );
}
