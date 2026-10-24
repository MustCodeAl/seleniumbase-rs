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
    // The stealth crates moved out and were renamed (0.3.0, then codec/mimic).
    ("seleniumbase_rs_wire", "seleniumbase_rs_codec"),
    ("seleniumbase-rs-wire", "seleniumbase-rs-codec"),
    ("seleniumbase_rs_impersonate", "seleniumbase_rs_mimic"),
    ("seleniumbase-rs-impersonate", "seleniumbase-rs-mimic"),
    ("crates/wire", "crates/codec"),
    ("crates/impersonate", "crates/mimic"),
    (
        "features = [\"scrape\"",
        "the seleniumbase-rs-stealth repository",
    ),
    (
        "features = [\"impersonate\"",
        "the seleniumbase-rs-stealth repository",
    ),
    // The Playwright feature is `playwright` (the dependency is still rustwright).
    ("--features rustwright", "--features playwright"),
    ("features = [\"rustwright\"", "features = [\"playwright\"]"),
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
    ("PlaywrightSession::launch_headless", "launch_with_config"),
    ("launch_headless_with_fingerprint", "launch_with_config"),
    ("launch_with_fingerprint", "launch_with_config"),
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
];

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
        if EXEMPT
            .iter()
            .any(|e| rel == *e || (e.ends_with('/') && rel.starts_with(e)))
        {
            continue;
        }
        let text = fs::read_to_string(&page).unwrap_or_default();
        for (number, line) in text.lines().enumerate() {
            for (gone, now) in GONE {
                if names(line, gone) {
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
