//! Enforces that the scrape domain layer stays pure: no runtime, no I/O, no clock reads, and only the allowed `sb_cdp` types.
//!
//! The check is syntactic. Each file under `src/scrape/domain` is parsed with `syn`, and
//! every path, `use` tree, attribute list and macro body is examined. It sees the names a
//! file writes, so it cannot judge code reached only through a type alias or a dependency.
//! The self-tests feed the checker an impure source and a clean one, so the check is shown
//! to reject what it should and accept what it should.

use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};
use syn::{ItemExternCrate, ItemUse, Macro, MetaList, UseTree};

/// Top-level crates that the domain layer may not name.
const FORBIDDEN_CRATES: &[&str] = &["tokio"];

/// Standard-library paths that perform I/O or read the clock.
const FORBIDDEN_STD: &[&[&str]] = &[
    &["std", "fs"],
    &["std", "net"],
    &["std", "process"],
    &["std", "time", "SystemTime"],
    &["std", "time", "*"],
    &["std", "*"],
];

/// Clock reads, however the type is reached.
const FORBIDDEN_CLOCK: &[&[&str]] = &[&["Instant", "now"], &["SystemTime", "now"]];

/// The only `sb_cdp` items the domain layer may name. `sync` is allowed only as `sync::locked`.
const ALLOWED_SB_CDP: &[&str] = &["Proxy", "Cookie", "SameSite", "Captcha"];
const ALLOWED_SB_CDP_SYNC: &str = "locked";

/// Roots and clock types whose renaming would hide a forbidden path.
const FORBIDDEN_RENAME_ROOTS: &[&str] =
    &["std", "tokio", "core", "alloc", "crate", "self", "super"];
const FORBIDDEN_RENAME_TARGETS: &[&str] = &["Instant", "SystemTime"];

/// Parses one source file and returns a description of every rule it breaks.
fn violations(source: &str) -> Result<Vec<String>, syn::Error> {
    let file = syn::parse_file(source)?;
    let mut checker = Checker::default();
    checker.visit_file(&file);
    Ok(checker.found)
}

#[derive(Default)]
struct Checker {
    found: Vec<String>,
}

impl Checker {
    fn check_path(&mut self, segments: &[String]) {
        check_segments(segments, &mut self.found);
    }

    fn check_tokens(&mut self, tokens: &impl ToString) {
        for run in token_runs(&tokens.to_string()) {
            self.check_path(&run);
        }
    }

    fn check_rename(&mut self, source: &[String]) {
        let last = source.last().map_or("", String::as_str);
        let root_rename = source.len() == 1 && FORBIDDEN_RENAME_ROOTS.contains(&last);
        if root_rename || FORBIDDEN_RENAME_TARGETS.contains(&last) {
            self.found.push(format!(
                "renames `{}`, which would hide a forbidden path",
                source.join("::")
            ));
        }
    }
}

impl<'ast> Visit<'ast> for Checker {
    fn visit_path(&mut self, node: &'ast syn::Path) {
        self.check_path(&path_segments(node));
        visit::visit_path(self, node);
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        let mut paths = Vec::new();
        let mut renames = Vec::new();
        flatten_use(&node.tree, &mut Vec::new(), &mut paths, &mut renames);
        for path in &paths {
            self.check_path(path);
        }
        for source in &renames {
            self.check_rename(source);
        }
        visit::visit_item_use(self, node);
    }

    fn visit_item_extern_crate(&mut self, node: &'ast ItemExternCrate) {
        self.check_path(&[node.ident.to_string()]);
        visit::visit_item_extern_crate(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        self.check_path(&path_segments(&node.path));
        self.check_tokens(&node.tokens);
    }

    fn visit_meta_list(&mut self, node: &'ast MetaList) {
        self.check_tokens(&node.tokens);
        visit::visit_meta_list(self, node);
    }
}

fn path_segments(path: &syn::Path) -> Vec<String> {
    path.segments.iter().map(|s| s.ident.to_string()).collect()
}

/// Expands a `use` tree into every full path it imports (a glob ends in `*`), and records
/// the source path of each rename separately.
fn flatten_use(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    paths: &mut Vec<Vec<String>>,
    renames: &mut Vec<Vec<String>>,
) {
    match tree {
        UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            flatten_use(&p.tree, prefix, paths, renames);
            prefix.pop();
        }
        UseTree::Name(n) => {
            let mut full = prefix.clone();
            full.push(n.ident.to_string());
            paths.push(full);
        }
        UseTree::Rename(r) => {
            let mut full = prefix.clone();
            full.push(r.ident.to_string());
            paths.push(full.clone());
            renames.push(full);
        }
        UseTree::Glob(_) => {
            let mut full = prefix.clone();
            full.push("*".to_owned());
            paths.push(full);
        }
        UseTree::Group(g) => {
            for item in &g.items {
                flatten_use(item, prefix, paths, renames);
            }
        }
    }
}

/// Applies the path rules to one sequence of segments.
fn check_segments(segments: &[String], found: &mut Vec<String>) {
    let text = segments.join("::");
    if segments
        .iter()
        .any(|s| FORBIDDEN_CRATES.contains(&s.as_str()))
    {
        found.push(format!("names a forbidden crate: `{text}`"));
    }
    for banned in FORBIDDEN_STD.iter().chain(FORBIDDEN_CLOCK) {
        if contains_run(segments, banned) {
            found.push(format!("names `{}`: `{text}`", banned.join("::")));
        }
    }
    for (index, segment) in segments.iter().enumerate() {
        if segment != "sb_cdp" {
            continue;
        }
        if !sb_cdp_path_allowed(&segments[index + 1..]) {
            found.push(format!(
                "reaches beyond the allowed `sb_cdp` types: `{text}`"
            ));
        }
    }
}

/// `rest` is what follows `sb_cdp`. Only an allowed type, or `sync::locked`, may follow.
fn sb_cdp_path_allowed(rest: &[String]) -> bool {
    match rest {
        [first, ..] => {
            ALLOWED_SB_CDP.contains(&first.as_str())
                || (first == "sync" && rest.get(1).map(String::as_str) == Some(ALLOWED_SB_CDP_SYNC))
        }
        [] => false,
    }
}

/// True when `needle` occurs as consecutive segments of `haystack`.
fn contains_run(haystack: &[String], needle: &[&str]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window.iter().zip(needle).all(|(a, b)| a.as_str() == *b))
}

/// One lexical token of a rendered token stream.
enum Tok {
    Ident(String),
    Sep,
    Other,
}

/// Lexes the text of a token stream (as `TokenStream::to_string` renders it), skipping
/// string literals so that a path written inside a string is never read as code.
fn lex(text: &str) -> Vec<Tok> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i += 1;
            out.push(Tok::Other);
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Tok::Ident(chars[start..i].iter().collect()));
        } else if c == ':' && chars.get(i + 1) == Some(&':') {
            out.push(Tok::Sep);
            i += 2;
        } else {
            out.push(Tok::Other);
            i += 1;
        }
    }
    out
}

/// The `::`-joined identifier runs in a token stream, such as `tokio::spawn` in a macro body.
fn token_runs(text: &str) -> Vec<Vec<String>> {
    let tokens = lex(text);
    let mut runs = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let Tok::Ident(first) = &tokens[i] else {
            i += 1;
            continue;
        };
        let mut run = vec![first.clone()];
        i += 1;
        while i + 1 < tokens.len() && matches!(tokens[i], Tok::Sep) {
            let Tok::Ident(next) = &tokens[i + 1] else {
                break;
            };
            run.push(next.clone());
            i += 2;
        }
        runs.push(run);
    }
    runs
}

/// Every `.rs` file under `dir`, recursively, in a stable order.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = fs::read_dir(&current)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", current.display()));
        for entry in entries {
            let path = entry.expect("readable directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn scrape_domain_layer_is_pure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let domain = root.join("src/scrape/domain");
    let files = rust_files(&domain);
    assert!(
        !files.is_empty(),
        "no .rs files under {}: the check would pass without looking at anything",
        domain.display()
    );

    let mut problems = Vec::new();
    for file in &files {
        let source = fs::read_to_string(file)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", file.display()));
        let found =
            violations(&source).unwrap_or_else(|e| panic!("cannot parse {}: {e}", file.display()));
        let shown = file.strip_prefix(root).unwrap_or(file.as_path());
        for violation in found {
            problems.push(format!("{}: {violation}", shown.display()));
        }
    }
    assert!(
        problems.is_empty(),
        "impure code in the scrape domain layer:\n{}",
        problems.join("\n")
    );
}

/// An impure module that breaks every rule at least once. Each entry of `EXPECTED` must be
/// reported; if the checker stops seeing one of these, the purity test above proves nothing.
const IMPURE: &str = r#"
use std::fs::File;
use std::net::TcpStream;
use std::process::Command;
use std::time::{Duration, SystemTime};
use std::time::Instant as Clock;
use tokio::sync::Mutex;
use crate::sb_cdp::Browser;
use crate::sb_cdp::sync::unlocked;
use seleniumbase_rs::sb_cdp::Page;
use crate::sb_cdp::{self, Proxy};

fn tick() -> u64 {
    let _a = std::time::Instant::now();
    let _b = SystemTime::now();
    let _c = std::fs::read("x");
    println!("{:?}", tokio::spawn(async {}));
    0
}
"#;

const EXPECTED: &[&str] = &[
    "names a forbidden crate",
    "names `std::fs`",
    "names `std::net`",
    "names `std::process`",
    "names `std::time::SystemTime`",
    "names `Instant::now`",
    "names `SystemTime::now`",
    "crate::sb_cdp::Browser",
    "crate::sb_cdp::sync::unlocked",
    "seleniumbase_rs::sb_cdp::Page",
    "crate::sb_cdp::self",
    "renames `std::time::Instant`",
];

#[test]
fn checker_rejects_an_impure_module() {
    let found = violations(IMPURE).expect("the impure fixture parses");
    for want in EXPECTED {
        assert!(
            found.iter().any(|v| v.contains(*want)),
            "the checker did not report `{want}`; it reported:\n{}",
            found.join("\n")
        );
    }
}

/// A module that uses the allowed `sb_cdp` items, mentions forbidden paths only in strings and
/// doc comments, and imports `Duration`. It must pass.
const CLEAN: &str = r#"
//! Mentions tokio and std::fs in documentation only.
use std::time::Duration;
use crate::sb_cdp::{Captcha, Cookie, Proxy, SameSite};
use crate::sb_cdp::sync::locked;

/// Refers to `Instant::now` in documentation only.
pub fn parts(p: &Proxy, c: &Cookie, s: SameSite, d: Duration) -> Option<Captcha> {
    let _ = Captcha::ALL;
    let _ = locked;
    println!("tokio::spawn and std::fs::read appear in a string only");
    let _ = (p, c, s, d);
    None
}
"#;

#[test]
fn checker_accepts_a_clean_module() {
    let found = violations(CLEAN).expect("the clean fixture parses");
    assert!(
        found.is_empty(),
        "the checker rejected a clean module:\n{}",
        found.join("\n")
    );
}
