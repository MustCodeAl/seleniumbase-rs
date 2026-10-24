//! The shape of the plugin features in `Cargo.toml` (ADR 005, "Core surface").
//!
//! Two dependencies on the plugins repository, `seleniumbase-rs-plugin` and
//! `seleniumbase-rs-plugins-builtin`, must name one source. Cargo treats two
//! git specs that differ in URL, branch, tag or revision as two sources, so a
//! mismatch builds two copies of the `seleniumbase-rs-plugin` crate and the
//! `Plugin` trait a built-in implements is then not the one this crate calls.
//! These tests read the manifest and the lockfile and fail on such a mismatch.
//! They need no feature and no browser.

use std::path::Path;

use toml::{Table, Value};

/// The commit of the plugins repository the manifest pins its dependencies to.
/// Read from the manifest, so moving the pin is an edit of the two dependency
/// lines and of the lockfile (`cargo update -p seleniumbase-rs-plugin`) only.
fn pinned_rev(manifest: &Table) -> String {
    let rev = source_spec(&manifest["dependencies"][PLUGINS_REPO_DEPS[0]])[3]
        .clone()
        .expect("the plugin API dependency is pinned to a `rev`");
    assert!(
        rev.len() == 40 && rev.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "`rev` is a full commit hash, not a branch or a short hash: {rev}"
    );
    rev
}

/// The two dependencies on the plugins repository.
const PLUGINS_REPO_DEPS: [&str; 2] = ["seleniumbase-rs-plugin", "seleniumbase-rs-plugins-builtin"];

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read_table(file: &str) -> Table {
    let text = std::fs::read_to_string(manifest_dir().join(file))
        .unwrap_or_else(|error| panic!("cannot read {file}: {error}"));
    text.parse::<Table>()
        .unwrap_or_else(|error| panic!("cannot parse {file}: {error}"))
}

/// What makes a git dependency one source: URL, branch, tag and revision.
fn source_spec(dependency: &Value) -> [Option<String>; 4] {
    let key = |name: &str| {
        dependency
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    [key("git"), key("branch"), key("tag"), key("rev")]
}

/// `Ok` when every dependency of `manifest` named in `names` is a git
/// dependency with the same source spec, otherwise what differs.
fn one_source(manifest: &Table, names: &[&str]) -> Result<(), String> {
    let dependencies = manifest
        .get("dependencies")
        .and_then(Value::as_table)
        .ok_or("no [dependencies] table")?;
    let mut first: Option<(&str, [Option<String>; 4])> = None;
    for name in names {
        let dependency = dependencies
            .get(*name)
            .ok_or_else(|| format!("`{name}` is not a dependency"))?;
        let spec = source_spec(dependency);
        if spec[0].is_none() {
            return Err(format!("`{name}` is not a git dependency"));
        }
        match &first {
            None => first = Some((name, spec)),
            Some((other, expected)) if *expected != spec => {
                return Err(format!(
                    "`{other}` is {expected:?} but `{name}` is {spec:?}: two plugin API crates \
                     would be built"
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn manifest_with(dependencies: &str) -> Table {
    format!("[dependencies]\n{dependencies}")
        .parse()
        .expect("a test manifest")
}

#[test]
fn the_plugins_repo_dependencies_use_one_source_spec() {
    let manifest = read_table("Cargo.toml");

    one_source(&manifest, &PLUGINS_REPO_DEPS).unwrap();

    for name in PLUGINS_REPO_DEPS {
        let dependency = &manifest["dependencies"][name];
        assert_eq!(
            source_spec(dependency)[3].as_deref(),
            Some(pinned_rev(&manifest).as_str()),
            "`{name}` is pinned to the same commit of the plugins repository"
        );
        assert_eq!(
            dependency.get("optional").and_then(Value::as_bool),
            Some(true),
            "`{name}` is optional: a build without plugins does not fetch it"
        );
    }
}

#[test]
fn a_differing_revision_url_or_branch_is_caught() {
    let same = manifest_with(
        r#"
a = { git = "https://example.test/repo", rev = "abc" }
b = { version = "0.1", git = "https://example.test/repo", rev = "abc" }
"#,
    );
    one_source(&same, &["a", "b"]).unwrap();

    for different in [
        r#"b = { git = "https://example.test/repo", rev = "def" }"#,
        r#"b = { git = "https://example.test/repo" }"#,
        r#"b = { git = "https://example.test/other", rev = "abc" }"#,
        r#"b = { git = "https://example.test/repo", branch = "main", rev = "abc" }"#,
        r#"b = "0.1""#,
    ] {
        let manifest = manifest_with(&format!(
            "a = {{ git = \"https://example.test/repo\", rev = \"abc\" }}\n{different}\n"
        ));
        let error = one_source(&manifest, &["a", "b"]).unwrap_err();
        assert!(error.contains('b'), "{different}: {error}");
    }
    assert!(one_source(&same, &["a", "missing"])
        .unwrap_err()
        .contains("missing"));
}

#[test]
fn the_lockfile_resolves_both_to_one_package_source() {
    let rev = pinned_rev(&read_table("Cargo.toml"));
    let lock = read_table("Cargo.lock");
    let packages = lock["package"].as_array().expect("[[package]] entries");
    let sources = |name: &str| -> Vec<String> {
        packages
            .iter()
            .filter(|package| package["name"].as_str() == Some(name))
            .filter_map(|package| package["source"].as_str().map(str::to_owned))
            .collect()
    };

    let api = sources("seleniumbase-rs-plugin");
    assert_eq!(api.len(), 1, "one plugin API crate in the tree: {api:?}");
    assert!(
        api[0].ends_with(&format!("#{rev}")),
        "the lockfile pins {rev}: {}",
        api[0]
    );
    let builtin = sources("seleniumbase-rs-plugins-builtin");
    assert_eq!(builtin, api, "the aggregator comes from the same source");
}

/// The feature a name stands for, as its list of entries.
fn feature(manifest: &Table, name: &str) -> Vec<String> {
    manifest["features"]
        .get(name)
        .unwrap_or_else(|| panic!("no feature `{name}`"))
        .as_array()
        .unwrap_or_else(|| panic!("feature `{name}` is not a list"))
        .iter()
        .map(|entry| entry.as_str().expect("a string entry").to_owned())
        .collect()
}

#[test]
fn each_builtin_plugin_has_a_feature_that_turns_on_exactly_its_own_crate() {
    let manifest = read_table("Cargo.toml");

    for plugin in ["shield", "block-resources", "crx", "fetcher", "extractor"] {
        assert_eq!(
            feature(&manifest, &format!("plugin-{plugin}")),
            [
                "plugins".to_owned(),
                "dep:seleniumbase-rs-plugins-builtin".to_owned(),
                format!("seleniumbase-rs-plugins-builtin/{plugin}"),
            ],
            "plugin-{plugin}"
        );
    }
    assert_eq!(
        feature(&manifest, "plugins-builtin"),
        [
            "plugin-shield",
            "plugin-block-resources",
            "plugin-crx",
            "plugin-fetcher",
            "plugin-extractor"
        ],
        "plugins-builtin is all of them"
    );
    assert_eq!(
        feature(&manifest, "plugins"),
        ["dep:seleniumbase-rs-plugin"],
        "plugins alone brings the API and no built-in plugin"
    );
}
