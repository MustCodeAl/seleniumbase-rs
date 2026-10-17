//! The library package carries no command-line front end.
//!
//! `sbrs` and `sbrs-mcp` live in the workspace crate `crates/cli`
//! (`seleniumbase-rs-cli`), so that someone who depends on `seleniumbase-rs` does
//! not compile a command-line parser, completion generators or the binaries'
//! own dependencies. These tests read the manifests as text and fail when a
//! binary or one of those dependencies comes back into the library package.

use toml::Table;

const LIBRARY: &str = include_str!("../Cargo.toml");
const CLI: &str = include_str!("../crates/cli/Cargo.toml");

/// Crates only the command line may use. A library user must not build them.
const COMMAND_LINE_ONLY: [&str; 4] = ["clap", "clap_complete", "clap_complete_fig", "clap_usage"];

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
    for needed in ["seleniumbase-rs", "clap", "clap_complete"] {
        assert!(
            dependencies.iter().any(|name| name == needed),
            "crates/cli does not depend on {needed}: {dependencies:?}"
        );
    }
}
