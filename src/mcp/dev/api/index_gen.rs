//! Writes and checks `api_index.json`, the index `find_api` searches.
//!
//! The index is generated from the source: every `pub` method of an inherent
//! `impl` of `BaseCase` (anywhere in `src/`) and of the pure CDP engine's
//! `Browser`, `Page`, `Locator` and `BrowserContext` (in `src/sb_cdp/`), with
//! the signature as written, the first paragraph of the doc comment and the
//! `async`, `deprecated` and feature facts.
//!
//! `the_committed_api_index_matches_the_source` fails when the committed file
//! is stale; `SB_UPDATE_API_INDEX=1 cargo test --features mcp-server api_index`
//! rewrites it.

use std::fs;
use std::path::{Path, PathBuf};

use proc_macro2::LineColumn;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ImplItem, Item, Lit, Meta, Type, Visibility};

use super::ApiEntry;

/// The types of the pure CDP engine whose methods are indexed.
const CDP_OWNERS: [&str; 4] = ["Browser", "Page", "Locator", "BrowserContext"];

/// The longest a doc paragraph is kept, in characters.
const MAX_DOC: usize = 600;

/// The text of one source file, with the offset of each line, so that a span
/// (line and column) can be turned into a slice.
struct Source<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> Source<'a> {
    fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
        Self { text, line_starts }
    }

    /// The byte offset of `at`; columns count characters.
    fn offset(&self, at: LineColumn) -> usize {
        let line_start = self.line_starts[at.line - 1];
        let rest = &self.text[line_start..];
        let column = rest
            .char_indices()
            .nth(at.column)
            .map_or(rest.len(), |(index, _)| index);
        line_start + column
    }

    fn slice(&self, start: LineColumn, end: LineColumn) -> &'a str {
        &self.text[self.offset(start)..self.offset(end)]
    }
}

/// `#[cfg(test)]`.
fn is_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && matches!(&attr.meta, Meta::List(list) if list.tokens.to_string() == "test")
    })
}

/// The feature named by `#[cfg(feature = "...")]`, if there is such an attribute.
fn cfg_feature(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("cfg") {
            return None;
        }
        let Meta::List(list) = &attr.meta else {
            return None;
        };
        let tokens = list.tokens.to_string();
        let name = tokens.strip_prefix("feature = \"")?.strip_suffix('"')?;
        Some(name.to_owned())
    })
}

/// The first paragraph of the doc comment, on one line.
fn first_paragraph(attrs: &[Attribute]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        let Meta::NameValue(pair) = &attr.meta else {
            continue;
        };
        let Expr::Lit(literal) = &pair.value else {
            continue;
        };
        let Lit::Str(text) = &literal.lit else {
            continue;
        };
        for line in text.value().split('\n') {
            lines.push(line.strip_prefix(' ').unwrap_or(line).trim_end().to_owned());
        }
    }
    let paragraph: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .skip_while(|line| line.trim().is_empty())
        .take_while(|line| !line.trim().is_empty())
        .collect();
    let mut text = paragraph.join(" ");
    if text.chars().count() > MAX_DOC {
        text = text.chars().take(MAX_DOC).collect::<String>() + "…";
    }
    text
}

/// A signature on one line: the whitespace runs of a wrapped one are single
/// spaces, and no space is left inside the parentheses.
fn one_line(signature: &str) -> String {
    signature
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("( ", "(")
        .replace(", )", ")")
        .replace(" )", ")")
}

fn deprecated(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("deprecated"))
}

fn owner_of(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

fn collect(
    items: &[Item],
    source: &Source<'_>,
    file: &str,
    engine: &str,
    owners: &[&str],
    out: &mut Vec<ApiEntry>,
) {
    for item in items {
        match item {
            Item::Impl(imp) if imp.trait_.is_none() && !is_cfg_test(&imp.attrs) => {
                let Some(owner) = owner_of(&imp.self_ty) else {
                    continue;
                };
                if !owners.contains(&owner.as_str()) {
                    continue;
                }
                let impl_feature = cfg_feature(&imp.attrs);
                for member in &imp.items {
                    let ImplItem::Fn(method) = member else {
                        continue;
                    };
                    if !matches!(method.vis, Visibility::Public(_)) || is_cfg_test(&method.attrs) {
                        continue;
                    }
                    // A where clause is not part of the signature's tokens.
                    let end = method
                        .sig
                        .generics
                        .where_clause
                        .as_ref()
                        .map_or_else(|| method.sig.span().end(), |clause| clause.span().end());
                    let signature = one_line(source.slice(method.vis.span().start(), end));
                    out.push(ApiEntry {
                        name: method.sig.ident.to_string(),
                        owner: owner.clone(),
                        engine: engine.to_owned(),
                        signature,
                        doc: first_paragraph(&method.attrs),
                        is_async: method.sig.asyncness.is_some(),
                        deprecated: deprecated(&method.attrs),
                        file: file.to_owned(),
                        feature: cfg_feature(&method.attrs).or_else(|| impl_feature.clone()),
                    });
                }
            }
            Item::Mod(module) if !is_cfg_test(&module.attrs) => {
                if let Some((_, inner)) = &module.content {
                    collect(inner, source, file, engine, owners, out);
                }
            }
            _ => {}
        }
    }
}

/// The indexed methods of one source file.
fn entries_of(text: &str, file: &str, engine: &str, owners: &[&str]) -> Vec<ApiEntry> {
    let parsed = syn::parse_file(text).unwrap_or_else(|error| panic!("{file}: {error}"));
    let source = Source::new(text);
    let mut out = Vec::new();
    collect(&parsed.items, &source, file, engine, owners, &mut out);
    out
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("cannot list {}: {error}", dir.display()))
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

/// The index of the whole source tree, in a fixed order.
fn fresh_entries() -> Vec<ApiEntry> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    let mut entries = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(root)
            .expect("under the manifest directory")
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&path).expect("readable source");
        if relative.starts_with("src/sb_cdp/") {
            entries.extend(entries_of(&text, &relative, "cdp", &CDP_OWNERS));
        }
        entries.extend(entries_of(&text, &relative, "basecase", &["BaseCase"]));
    }
    entries.sort_by(|a, b| {
        (&a.engine, &a.owner, &a.name, &a.file).cmp(&(&b.engine, &b.owner, &b.name, &b.file))
    });
    entries
}

/// The file text: one entry per line, so a change shows as a small diff.
fn render(entries: &[ApiEntry]) -> String {
    let mut text = String::from("[\n");
    for (position, entry) in entries.iter().enumerate() {
        text.push_str(&serde_json::to_string(entry).expect("an entry serializes"));
        text.push_str(if position + 1 < entries.len() {
            ",\n"
        } else {
            "\n"
        });
    }
    text.push_str("]\n");
    text
}

#[test]
fn the_committed_api_index_matches_the_source() {
    let fresh = render(&fresh_entries());
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mcp/dev/api_index.json");
    if std::env::var_os("SB_UPDATE_API_INDEX").is_some() {
        fs::write(&path, &fresh).expect("api_index.json is writable");
        return;
    }
    let committed = fs::read_to_string(&path).expect("api_index.json exists");
    assert!(
        committed == fresh,
        "src/mcp/dev/api_index.json is out of date with the source. Regenerate it with:\n    \
         SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indexed(source: &str, owners: &[&str]) -> Vec<ApiEntry> {
        entries_of(source, "src/x.rs", "basecase", owners)
    }

    #[test]
    fn only_public_methods_of_inherent_impls_of_the_owners_are_indexed() {
        let source = r"
impl BaseCase {
    /// Opens a page.
    pub async fn open(&mut self, url: &str) -> Result<()> { Ok(()) }
    fn private(&self) {}
    pub(crate) fn crate_only(&self) {}
}
impl Other { pub fn elsewhere(&self) {} }
impl Trait for BaseCase { fn from_trait(&self) {} }
#[cfg(test)]
mod tests { impl BaseCase { pub fn only_in_tests(&self) {} } }
mod nested { impl BaseCase { pub fn in_a_module(&self) {} } }
";
        let names: Vec<_> = indexed(source, &["BaseCase"])
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(names, ["open", "in_a_module"]);
    }

    #[test]
    fn the_signature_is_the_source_text_on_one_line() {
        let source = r"
impl BaseCase {
    /// Types text.
    pub async fn type_text(
        &mut self,
        selector: &str,
        text: impl Into<String>,
    ) -> Result<(), SeleniumBaseError> { Ok(()) }
    pub const fn short<'a, T: Clone>(x: &'a T) -> Option<&'a T> where T: Sized { None }
}
";
        let entries = indexed(source, &["BaseCase"]);
        assert_eq!(
            entries[0].signature,
            "pub async fn type_text(&mut self, selector: &str, text: impl Into<String>) \
             -> Result<(), SeleniumBaseError>"
        );
        assert!(entries[0].is_async);
        assert_eq!(
            entries[1].signature,
            "pub const fn short<'a, T: Clone>(x: &'a T) -> Option<&'a T> where T: Sized"
        );
        assert!(!entries[1].is_async);
    }

    #[test]
    fn the_doc_is_the_first_paragraph_joined_on_one_line() {
        let source = r"
impl BaseCase {
    /// Clicks the element
    /// that matches.
    ///
    /// Waits first.
    pub fn click(&self) {}
    pub fn undocumented(&self) {}
}
";
        let entries = indexed(source, &["BaseCase"]);
        assert_eq!(entries[0].doc, "Clicks the element that matches.");
        assert_eq!(entries[1].doc, "");
    }

    #[test]
    fn deprecation_and_features_are_recorded() {
        let source = r#"
impl BaseCase {
    #[deprecated(note = "use open")]
    pub fn goto(&self) {}
    #[cfg(feature = "pdf")]
    pub fn pdf_text(&self) {}
}
#[cfg(feature = "gui")]
impl BaseCase {
    pub fn gui_click(&self) {}
    #[cfg(feature = "ocr")]
    pub fn read_screen(&self) {}
}
"#;
        let entries = indexed(source, &["BaseCase"]);
        let by_name = |name: &str| entries.iter().find(|e| e.name == name).unwrap();
        assert!(by_name("goto").deprecated);
        assert_eq!(by_name("goto").feature, None);
        assert!(!by_name("pdf_text").deprecated);
        assert_eq!(by_name("pdf_text").feature.as_deref(), Some("pdf"));
        assert_eq!(by_name("gui_click").feature.as_deref(), Some("gui"));
        assert_eq!(by_name("read_screen").feature.as_deref(), Some("ocr"));
    }

    #[test]
    fn the_owners_are_matched_by_the_last_path_segment() {
        let source = "impl crate::sb_cdp::Page { pub fn goto(&self) {} }\n\
                      impl Locator { pub fn text(&self) {} }\n\
                      impl Identity { pub fn seed(&self) {} }";
        let names: Vec<_> = indexed(source, &CDP_OWNERS)
            .into_iter()
            .map(|e| format!("{}::{}", e.owner, e.name))
            .collect();
        assert_eq!(names, ["Page::goto", "Locator::text"]);
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        let source = "impl BaseCase {\n    /// Ünïcode.\n    pub fn café(&self, é: u8) {}\n}\n";
        let entries = indexed(source, &["BaseCase"]);
        assert_eq!(entries[0].signature, "pub fn café(&self, é: u8)");
    }
}
