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
    // The binaries are the package seleniumbase-rs-cli since 0.5.0.
    (
        "cargo install seleniumbase-rs --",
        "cargo install ... seleniumbase-rs-cli",
    ),
    (
        "--bin sbrs --features mcp-server",
        "the seleniumbase-rs-cli package",
    ),
];

/// Files and folders that describe the past on purpose.
const EXEMPT: &[&str] = &[
    "CHANGELOG.md",
    "docs/changelog.md",
    "docs/design/",
    "docs/adr/",
];

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
                if line.contains(gone) {
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
fn the_list_covers_the_pages_people_read() {
    let pages = pages();
    assert!(pages.iter().any(|p| p.ends_with("README.md")));
    assert!(pages
        .iter()
        .any(|p| p.to_string_lossy().contains("docs/tutorials")));
    assert!(pages.len() > 50, "only {} pages found", pages.len());
}
