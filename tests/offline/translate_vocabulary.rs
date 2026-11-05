//! The translation dictionary against the real API.
//!
//! A dictionary row that names a method `BaseCase` does not have would
//! translate code that cannot exist, and a scenario action without a row could
//! not be written in another language. Both are checked against the sources.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use seleniumbase_rs::utils::translate::master_dict;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

fn parse(path: &Path) -> syn::File {
    let source = fs::read_to_string(path).expect("readable source");
    syn::parse_file(&source).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every method name in an `impl ... BaseCase` block, trait impls included.
fn basecase_methods() -> BTreeSet<String> {
    fn collect(items: &[syn::Item], out: &mut BTreeSet<String>) {
        for item in items {
            match item {
                syn::Item::Impl(imp) => {
                    let is_basecase = matches!(
                        &*imp.self_ty,
                        syn::Type::Path(path)
                            if path.path.segments.last().is_some_and(|s| s.ident == "BaseCase")
                    );
                    if is_basecase {
                        for member in &imp.items {
                            if let syn::ImplItem::Fn(function) = member {
                                out.insert(function.sig.ident.to_string());
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

    let mut files = Vec::new();
    rust_files(&root().join("src"), &mut files);
    let mut methods = BTreeSet::new();
    for file in files {
        collect(&parse(&file).items, &mut methods);
    }
    methods
}

/// The `"action"` names a JSON scenario step can have: the variants of
/// `ScenarioStep`, in snake case.
fn scenario_actions() -> BTreeSet<String> {
    let file = parse(&root().join("src/api/scenario.rs"));
    let variants = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Enum(e) if e.ident == "ScenarioStep" => Some(e),
            _ => None,
        })
        .expect("ScenarioStep is defined in src/api/scenario.rs");
    variants
        .variants
        .iter()
        .map(|variant| {
            let mut snake = String::new();
            for (i, c) in variant.ident.to_string().chars().enumerate() {
                if c.is_ascii_uppercase() {
                    if i > 0 {
                        snake.push('_');
                    }
                    snake.push(c.to_ascii_lowercase());
                } else {
                    snake.push(c);
                }
            }
            snake
        })
        .collect()
}

#[test]
fn every_dictionary_entry_names_a_method_basecase_really_has() {
    let methods = basecase_methods();
    assert!(
        methods.contains("open") && methods.contains("click"),
        "the scan of src/ found no BaseCase methods; has the layout changed?"
    );
    let missing: Vec<_> = master_dict::entries()
        .iter()
        .map(|entry| entry.en)
        .filter(|name| !methods.contains(*name))
        .collect();
    assert!(
        missing.is_empty(),
        "these dictionary rows are not BaseCase methods: {missing:?}"
    );
}

#[test]
fn every_scenario_action_can_be_written_in_every_language() {
    let actions = scenario_actions();
    assert!(actions.len() > 30, "found only {actions:?}");
    let missing: Vec<_> = actions
        .iter()
        .filter(|action| master_dict::entry(action).is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "scenario actions with no dictionary row (add them to master_dict.rs): {missing:?}"
    );
}
