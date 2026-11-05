//! The library package carries no command-line front end.
//!
//! `sbrs` and `sbrs-mcp` live in the workspace crate `crates/cli`
//! (`seleniumbase-rs-cli`), so that someone who depends on `seleniumbase-rs` does
//! not compile a command-line parser, completion generators or the binaries'
//! own dependencies. These tests read the manifests as text and fail when a
//! binary or one of those dependencies comes back into the library package.

use toml::Table;

const LIBRARY: &str = include_str!("../../Cargo.toml");
const CLI: &str = include_str!("../../crates/cli/Cargo.toml");
const PLAYWRIGHT: &str = include_str!("../../crates/playwright/Cargo.toml");

/// Crates only the command line may use. A library user must not build them.
///
/// `usage-rs` is the parser of `crates/cli` (its sources import it as `usage`), `usage_lib`
/// the Markdown renderer its tests use; the clap crates are what the parser replaced.
const COMMAND_LINE_ONLY: [&str; 8] = [
    "usage",
    "usage-rs",
    "usage_lib",
    "usage-lib",
    "clap",
    "clap_complete",
    "clap_complete_fig",
    "clap_usage",
];

fn parse(manifest: &str) -> Table {
    manifest.parse().expect("the manifest parses")
}

fn table_names(manifest: &Table, section: &str) -> Vec<String> {
    manifest
        .get(section)
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().cloned().collect())
        .unwrap_or_default()
}

fn bin_names(manifest: &Table) -> Vec<String> {
    manifest
        .get("bin")
        .and_then(toml::Value::as_array)
        .map(|bins| {
            bins.iter()
                .filter_map(|bin| bin.get("name").and_then(toml::Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_library_declares_no_binary() {
    let library = parse(LIBRARY);

    assert_eq!(bin_names(&library), Vec::<String>::new());
}

#[test]
fn the_library_depends_on_no_command_line_crate() {
    let library = parse(LIBRARY);

    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let names = table_names(&library, section);
        for banned in COMMAND_LINE_ONLY {
            assert!(
                !names.iter().any(|name| name == banned),
                "[{section}] of the library names {banned}; it belongs to crates/cli"
            );
        }
    }
}

#[test]
fn the_command_line_crate_is_a_member_of_the_workspace() {
    let library = parse(LIBRARY);
    let members: Vec<&str> = library
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
        .map(|members| members.iter().filter_map(toml::Value::as_str).collect())
        .unwrap_or_default();

    assert!(members.contains(&"crates/cli"), "members: {members:?}");
}

#[test]
fn the_command_line_crate_builds_both_binaries_from_the_library() {
    let cli = parse(CLI);
    let name = cli
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str);

    assert_eq!(name, Some("seleniumbase-rs-cli"));
    assert_eq!(bin_names(&cli), ["sbrs", "sbrs-mcp"]);
    let dependencies = table_names(&cli, "dependencies");
    for needed in ["seleniumbase-rs", "usage-rs"] {
        assert!(
            dependencies.iter().any(|name| name == needed),
            "crates/cli does not depend on {needed}: {dependencies:?}"
        );
    }
}

#[test]
fn the_library_links_no_playwright_engine() {
    let library = parse(LIBRARY);

    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let names = table_names(&library, section);
        for engine in ["rustwright", "rustwright-core"] {
            assert!(
                !names.iter().any(|name| name == engine),
                "[{section}] of the library names {engine}; the Playwright engine belongs to \
                 crates/playwright"
            );
        }
    }
    assert!(
        !table_names(&library, "features")
            .iter()
            .any(|feature| feature == "playwright"),
        "the library has no `playwright` feature; the bridge is the crate crates/playwright"
    );
}

#[test]
fn the_playwright_bridge_is_a_workspace_member_that_is_not_built_by_default() {
    let library = parse(LIBRARY);
    let list = |key: &str| -> Vec<&str> {
        library
            .get("workspace")
            .and_then(|workspace| workspace.get(key))
            .and_then(toml::Value::as_array)
            .map(|members| members.iter().filter_map(toml::Value::as_str).collect())
            .unwrap_or_default()
    };

    assert!(
        list("members").contains(&"crates/playwright"),
        "members: {:?}",
        list("members")
    );
    assert!(
        !list("default-members").contains(&"crates/playwright"),
        "a plain `cargo build` must not compile the Playwright engine"
    );
}

#[test]
fn the_playwright_bridge_depends_on_the_library_and_the_engine() {
    let bridge = parse(PLAYWRIGHT);
    let name = bridge
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str);

    assert_eq!(name, Some("seleniumbase-rs-playwright"));
    let dependencies = table_names(&bridge, "dependencies");
    for needed in ["seleniumbase-rs", "rustwright"] {
        assert!(
            dependencies.iter().any(|name| name == needed),
            "crates/playwright does not depend on {needed}: {dependencies:?}"
        );
    }
    // The bridge is reached through the library's `sb_cdp` module, not a second copy of it.
    let library_path = bridge
        .get("dependencies")
        .and_then(|dependencies| dependencies.get("seleniumbase-rs"))
        .and_then(|dependency| dependency.get("path"))
        .and_then(toml::Value::as_str);
    assert_eq!(library_path, Some("../.."));
}

#[test]
fn the_command_line_is_parsed_by_usage_not_clap() {
    let cli = parse(CLI);

    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        let names = table_names(&cli, section);
        for gone in ["clap", "clap_complete", "clap_complete_fig", "clap_usage"] {
            assert!(
                !names.iter().any(|name| name == gone),
                "[{section}] of crates/cli names {gone}; the parser is usage"
            );
        }
    }
}
