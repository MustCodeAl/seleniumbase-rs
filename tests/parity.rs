//! The API parity manifest against the upstream API and this crate's sources.
//!
//! `parity/upstream-api.json` lists the public names of the upstream Python
//! SeleniumBase and seleniumbase-mcp; `parity/api.toml` says what each one maps
//! to here. A failure of this test is a to-do list: read it top to bottom.
//! See `docs/UPSTREAM_SYNC.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const SURFACES: [&str; 10] = [
    "basecase",
    "cdp",
    "driver",
    "cli",
    "options_pytest",
    "options_sb",
    "options_driver",
    "mcp_cdp",
    "mcp_driver",
    "mcp_sb",
];

const STATUSES: [&str; 4] = ["implemented", "composed", "not-applicable", "planned"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

/// The upstream names, by manifest surface.
fn upstream_names() -> BTreeMap<&'static str, BTreeSet<String>> {
    let api: Value = serde_json::from_str(&read("parity/upstream-api.json")).expect("valid JSON");
    let names = |value: &Value| -> BTreeSet<String> {
        value
            .as_array()
            .expect("an array")
            .iter()
            .map(|item| {
                item.get("name")
                    .unwrap_or(item)
                    .as_str()
                    .expect("a name")
                    .to_owned()
            })
            .collect()
    };
    BTreeMap::from([
        ("basecase", names(&api["basecase"])),
        ("cdp", names(&api["cdp"])),
        ("driver", names(&api["driver"])),
        ("cli", names(&api["cli"])),
        ("options_pytest", names(&api["options"]["pytest"])),
        ("options_sb", names(&api["options"]["sb_kwargs"])),
        ("options_driver", names(&api["options"]["driver_kwargs"])),
        ("mcp_cdp", names(&api["mcp"]["cdp"])),
        ("mcp_driver", names(&api["mcp"]["driver"])),
        ("mcp_sb", names(&api["mcp"]["sb"])),
    ])
}

type Manifest = BTreeMap<String, BTreeMap<String, toml::Table>>;

fn manifest() -> Manifest {
    let parsed: toml::Table = read("parity/api.toml")
        .parse()
        .expect("api.toml is valid TOML");
    parsed
        .into_iter()
        .map(|(surface, entries)| {
            let entries = entries
                .as_table()
                .unwrap_or_else(|| panic!("[{surface}] must be a table"))
                .iter()
                .map(|(name, entry)| {
                    let entry = entry
                        .as_table()
                        .unwrap_or_else(|| panic!("[{surface}.{name}] must be a table"));
                    (name.clone(), entry.clone())
                })
                .collect();
            (surface, entries)
        })
        .collect()
}

fn text(entry: &toml::Table, key: &str) -> Option<String> {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
}

// ----------------------------------------------------------------------
// What the Rust sources define
// ----------------------------------------------------------------------

#[derive(Default)]
struct RustItems {
    methods: BTreeSet<(String, String)>,
    fields: BTreeSet<(String, String)>,
}

fn collect(items: &[syn::Item], out: &mut RustItems) {
    for item in items {
        match item {
            syn::Item::Impl(imp) => {
                if let syn::Type::Path(path) = &*imp.self_ty {
                    if let Some(segment) = path.path.segments.last() {
                        for member in &imp.items {
                            if let syn::ImplItem::Fn(function) = member {
                                out.methods.insert((
                                    segment.ident.to_string(),
                                    function.sig.ident.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
            syn::Item::Struct(item) => {
                if let syn::Fields::Named(named) = &item.fields {
                    for field in &named.named {
                        if let Some(name) = &field.ident {
                            out.fields
                                .insert((item.ident.to_string(), name.to_string()));
                        }
                    }
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, inner)) = &module.content {
                    collect(inner, out);
                }
            }
            _ => {}
        }
    }
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot list {}: {e}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

fn rust_items() -> RustItems {
    let mut files = Vec::new();
    rust_files(&root().join("src"), &mut files);
    let mut items = RustItems::default();
    let mut unparsable = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("readable source");
        match syn::parse_file(&source) {
            Ok(parsed) => collect(&parsed.items, &mut items),
            Err(error) => unparsable.push(format!("{}: {error}", file.display())),
        }
    }
    assert!(
        unparsable.is_empty(),
        "these sources could not be parsed, so their items cannot be checked:\n{}",
        unparsable.join("\n")
    );
    items
}

fn cli_source() -> String {
    let mut files = Vec::new();
    rust_files(&root().join("src").join("cli"), &mut files);
    files
        .iter()
        .map(|file| fs::read_to_string(file).expect("readable source"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(feature = "mcp-server")]
fn mcp_tools() -> BTreeMap<&'static str, BTreeSet<String>> {
    use seleniumbase_rs::mcp::{cdp, driver, sb, Settings};
    let names = |names: Vec<&'static str>| names.into_iter().map(str::to_owned).collect();
    BTreeMap::from([
        (
            "cdp",
            names(cdp::host(Settings::new("unused")).tool_names()),
        ),
        (
            "driver",
            names(driver::host(Settings::new("unused")).tool_names()),
        ),
        ("sb", names(sb::host(Settings::new("unused")).tool_names())),
    ])
}

// ----------------------------------------------------------------------
// The checks
// ----------------------------------------------------------------------

#[test]
fn every_upstream_name_is_classified_and_none_is_stale() {
    let upstream = upstream_names();
    let manifest = manifest();
    let mut problems = Vec::new();

    for surface in SURFACES {
        let listed: BTreeSet<String> = manifest
            .get(surface)
            .map(|entries| entries.keys().cloned().collect())
            .unwrap_or_default();
        let wanted = &upstream[surface];
        let missing: Vec<_> = wanted.difference(&listed).collect();
        let stale: Vec<_> = listed.difference(wanted).collect();
        if !missing.is_empty() {
            problems.push(format!(
                "[{surface}] upstream names with no entry in parity/api.toml ({}):\n  {}",
                missing.len(),
                missing
                    .iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !stale.is_empty() {
            problems.push(format!(
                "[{surface}] entries for names upstream no longer has ({}):\n  {}",
                stale.len(),
                stale
                    .iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    for surface in manifest.keys() {
        if !SURFACES.contains(&surface.as_str()) {
            problems.push(format!(
                "parity/api.toml has an unknown section [{surface}]"
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "parity/api.toml is out of step with parity/upstream-api.json.\n\
         Run `python3 -I tools/parity/seed_manifest.py` to add entries for new names,\n\
         delete entries for removed ones, then review what was added.\n\n{}",
        problems.join("\n\n")
    );
}

#[test]
fn every_entry_is_complete() {
    let mut problems = Vec::new();
    for (surface, entries) in manifest() {
        for (name, entry) in entries {
            let status = text(&entry, "status").unwrap_or_default();
            let need = |key: &str, problems: &mut Vec<String>| {
                if text(&entry, key).is_none_or(|value| value.trim().is_empty()) {
                    problems.push(format!(
                        "[{surface}.{name:?}] status {status:?} needs `{key}`"
                    ));
                }
            };
            match status.as_str() {
                "implemented" | "composed" => need("rust", &mut problems),
                "not-applicable" => need("reason", &mut problems),
                "planned" => need("note", &mut problems),
                other => problems.push(format!(
                    "[{surface}.{name:?}] unknown status {other:?}; use one of {STATUSES:?}"
                )),
            }
        }
    }
    assert!(
        problems.is_empty(),
        "incomplete manifest entries:\n{}",
        problems.join("\n")
    );
}

#[test]
fn every_rust_path_in_the_manifest_exists() {
    let items = rust_items();
    let cli = cli_source();
    #[cfg(feature = "mcp-server")]
    let tools = mcp_tools();
    let mut problems = Vec::new();

    for (surface, entries) in manifest() {
        for (name, entry) in entries {
            let status = text(&entry, "status").unwrap_or_default();
            if !matches!(status.as_str(), "implemented" | "composed") {
                continue;
            }
            let Some(path) = text(&entry, "rust") else {
                continue;
            };
            let exists = if path == "clap" {
                true
            } else if let Some(command) = path.strip_prefix("sbase ") {
                let camel: String = command
                    .split('-')
                    .map(|part| {
                        let mut chars = part.chars();
                        chars
                            .next()
                            .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect())
                    })
                    .collect();
                cli.contains(&format!("\"{command}\"")) || cli.contains(&camel)
            } else if let Some(server) = path.strip_prefix("mcp::") {
                #[cfg(feature = "mcp-server")]
                {
                    tools
                        .get(server)
                        .is_some_and(|offered| offered.contains(&name))
                }
                #[cfg(not(feature = "mcp-server"))]
                {
                    let _ = server;
                    true
                }
            } else if let Some((owner, member)) = path.split_once("::") {
                let key = (owner.to_owned(), member.to_owned());
                items.methods.contains(&key) || items.fields.contains(&key)
            } else {
                false
            };
            if !exists {
                problems.push(format!(
                    "[{surface}.{name:?}] `{path}` is not defined in src/"
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "manifest entries point at Rust items that do not exist. Fix the path, or set\n\
         the status to `planned` if the item was removed:\n{}",
        problems.join("\n")
    );
}

#[cfg(feature = "mcp-server")]
#[test]
fn every_mcp_tool_we_offer_from_upstream_is_marked_implemented() {
    let manifest = manifest();
    let tools = mcp_tools();
    let mut problems = Vec::new();
    for (server, offered) in &tools {
        let surface = format!("mcp_{server}");
        let Some(entries) = manifest.get(&surface) else {
            continue;
        };
        for name in offered {
            // Tools this crate adds (such as the stealth tools) have no upstream entry.
            if let Some(entry) = entries.get(name) {
                if text(entry, "status").as_deref() != Some("implemented") {
                    problems.push(format!(
                        "[{surface}.{name:?}] is offered but not marked implemented"
                    ));
                }
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_pinned_upstream_matches_the_extracted_api_and_the_crate_constant() {
    let pinned: toml::Table = read("parity/upstream.toml").parse().expect("valid TOML");
    let api: Value = serde_json::from_str(&read("parity/upstream-api.json")).expect("valid JSON");

    let version = pinned["seleniumbase"]["version"]
        .as_str()
        .expect("a version");
    let commit = pinned["seleniumbase"]["commit"].as_str().expect("a commit");
    let mcp_commit = pinned["seleniumbase_mcp"]["commit"]
        .as_str()
        .expect("a commit");

    assert_eq!(
        api["seleniumbase"]["version"], version,
        "regenerate parity/upstream-api.json"
    );
    assert_eq!(
        api["seleniumbase"]["commit"], commit,
        "regenerate parity/upstream-api.json"
    );
    assert_eq!(
        api["seleniumbase_mcp"]["commit"], mcp_commit,
        "regenerate parity/upstream-api.json"
    );
    assert_eq!(
        seleniumbase_rs::UPSTREAM_SELENIUMBASE_VERSION,
        version,
        "update UPSTREAM_SELENIUMBASE_VERSION in src/lib.rs"
    );
}

#[test]
fn the_rendered_parity_table_is_up_to_date() {
    let fresh = std::env::temp_dir().join(format!("parity-{}.md", std::process::id()));
    let ran = std::process::Command::new("python3")
        .args(["-I", "tools/parity/render_docs.py", "--out"])
        .arg(&fresh)
        .current_dir(root())
        .status();
    let Ok(status) = ran else {
        eprintln!("python3 is not installed; skipping the docs freshness check");
        return;
    };
    assert!(status.success(), "tools/parity/render_docs.py failed");
    let expected = fs::read_to_string(&fresh).expect("rendered table");
    let _ = fs::remove_file(&fresh);
    assert_eq!(
        read("docs/parity.md"),
        expected,
        "docs/parity.md is out of date; run `just parity-docs`"
    );
}

/// Not a check: prints how much of the upstream API is still to do.
#[test]
fn progress_summary() {
    let mut lines = Vec::new();
    for (surface, entries) in manifest() {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for entry in entries.values() {
            *counts
                .entry(text(entry, "status").unwrap_or_default())
                .or_default() += 1;
        }
        lines.push(format!("{surface:<16} {counts:?}"));
    }
    eprintln!("\n{}", lines.join("\n"));
}
