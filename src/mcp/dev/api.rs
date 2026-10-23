//! `find_api` and `parity_lookup`: where the library's methods are, and how
//! the methods of Python SeleniumBase map onto them.
//!
//! `find_api` searches an index of the public methods of `BaseCase` and of the
//! pure CDP engine (`Browser`, `Page`, `Locator`, `BrowserContext`). The index
//! is generated from the source by a test (`index_gen`), committed as
//! `api_index.json` and embedded, so the answer is the API of this build.
//! `parity_lookup` reads `parity/api.toml`, embedded the same way.

#[cfg(test)]
mod index_gen;

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::NoBrowser;
use crate::error::closest_match;
use crate::mcp::support::json_output;
use crate::mcp::{Args, Ctx, Effect, Output, Prop, Schema, ToolDef, ToolError};

/// The index of public methods, written by `index_gen`.
const INDEX_JSON: &str = include_str!("api_index.json");

/// The manifest of Python names, `parity/api.toml`.
const PARITY_TOML: &str = include_str!("../../../parity/api.toml");

/// Most entries one call returns.
const MAX_LIMIT: usize = 50;

/// One public method in the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ApiEntry {
    /// The method's name, such as `click`.
    pub name: String,
    /// The type it belongs to, such as `BaseCase` or `Page`.
    pub owner: String,
    /// `basecase` (WebDriver) or `cdp` (the pure CDP engine).
    pub engine: String,
    /// The signature as written in the source.
    pub signature: String,
    /// The first paragraph of the doc comment, on one line.
    pub doc: String,
    /// Whether the method is `async`.
    pub is_async: bool,
    /// The source file, relative to the repository.
    pub file: String,
    /// The cargo feature the method needs, if it needs one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
}

/// The embedded index.
fn index() -> &'static [ApiEntry] {
    static INDEX: OnceLock<Vec<ApiEntry>> = OnceLock::new();
    INDEX.get_or_init(|| serde_json::from_str(INDEX_JSON).unwrap_or_default())
}

/// The tools of this file.
pub(super) fn tools() -> Vec<ToolDef<NoBrowser>> {
    vec![
        ToolDef::new(
            "find_api",
            "Find API",
            "Search the public API of this library for a method: BaseCase (the WebDriver engine) \
             and the pure CDP engine (Browser, Page, Locator, BrowserContext). Ranks an exact \
             name first, then names that contain the query, then methods whose name, type or \
             documentation mention every word. Each match has its type, full signature, the \
             first paragraph of its documentation, and whether it is async. The index is \
             the one of this build.",
            Effect::Inspect,
            Schema::new()
                .required(
                    "query",
                    Prop::string(
                        "A method name or words describing it, such as `screenshot`, \
                         `wait for text` or `Page::click`",
                    ),
                )
                .optional(
                    "engine",
                    Prop::choice(
                        &["basecase", "cdp", "any"],
                        "Which engine to search: BaseCase, the pure CDP engine, or both",
                    )
                    .default("any"),
                )
                .optional(
                    "limit",
                    Prop::integer("Most matches to return, 1 to 50")
                        .min(1.0)
                        .default(10),
                ),
            find_api,
        ),
        ToolDef::new(
            "parity_lookup",
            "Parity Lookup",
            "Look up a Python SeleniumBase 4.55.2 name (a BaseCase or sb.cdp method, a Driver() \
             method, a command, an option or an upstream MCP tool) and say what this library has \
             for it: its status (implemented, composed, not-applicable, planned), the Rust path, \
             and notes on any difference in behavior. With no exact match, lists close names.",
            Effect::Inspect,
            Schema::new().required(
                "python_name",
                Prop::string(
                    "The Python name, such as `click`, `self.assert_text`, `sb.cdp.gui_click_x_y` \
                     or `uc_open_with_reconnect`",
                ),
            ),
            parity_lookup,
        ),
    ]
}

// ----------------------------------------------------------------------
// find_api
// ----------------------------------------------------------------------

/// The lowercase words of `text`, split at anything that is not a letter or
/// a digit.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// How well `entry` answers a query of `words`, or `None` when it does not.
///
/// An exact name scores 100, a name that contains the words joined with `_`
/// 80, a name that contains every word 60, and a method whose name, type and
/// documentation together mention every word 40.
fn score(entry: &ApiEntry, words: &[String]) -> Option<u32> {
    let name = entry.name.to_lowercase();
    let joined = words.join("_");
    if name == joined {
        return Some(100);
    }
    if name.contains(&joined) {
        return Some(80);
    }
    if words.iter().all(|word| name.contains(word.as_str())) {
        return Some(60);
    }
    let owner = entry.owner.to_lowercase();
    let doc = entry.doc.to_lowercase();
    words
        .iter()
        .all(|word| {
            name.contains(word.as_str())
                || owner.contains(word.as_str())
                || doc.contains(word.as_str())
        })
        .then_some(40)
}

/// What a search found: every match, best first.
#[derive(Debug)]
pub(super) struct Found<'a> {
    /// Each match with its score, the best score first.
    pub matches: Vec<(u32, &'a ApiEntry)>,
}

/// Searches `index` for `query` in the methods of `engine` (`basecase`, `cdp`
/// or `any`). `Type::name` restricts the search to one type.
pub(super) fn search<'a>(index: &'a [ApiEntry], query: &str, engine: &str) -> Option<Found<'a>> {
    let (owner, name_query) = match query.rsplit_once("::") {
        Some((owner, name)) => (Some(owner.trim().to_lowercase()), name),
        None => (None, query),
    };
    let words = words(name_query);
    if words.is_empty() {
        return None;
    }
    let mut matches: Vec<(u32, &ApiEntry)> = index
        .iter()
        .filter(|entry| engine == "any" || entry.engine == engine)
        .filter(|entry| {
            owner
                .as_ref()
                .is_none_or(|owner| entry.owner.to_lowercase() == *owner)
        })
        .filter_map(|entry| score(entry, &words).map(|score| (score, entry)))
        .collect();
    matches.sort_by(|(a_score, a), (b_score, b)| {
        b_score
            .cmp(a_score)
            .then_with(|| a.engine.cmp(&b.engine))
            .then_with(|| a.owner.cmp(&b.owner))
            .then_with(|| a.name.cmp(&b.name))
    });
    Some(Found { matches })
}

async fn find_api(_ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let query = args.str("query")?;
    let engine = args.choice("engine", &["basecase", "cdp", "any"], "any")?;
    let limit = match args.opt_usize("limit")? {
        None => 10,
        Some(n) if (1..=MAX_LIMIT).contains(&n) => n,
        Some(_) => {
            return Err(ToolError::invalid(
                "limit",
                format!("must be between 1 and {MAX_LIMIT}"),
            ))
        }
    };
    let index = index();
    let Some(found) = search(index, query, engine) else {
        return Err(ToolError::invalid(
            "query",
            "give a method name or some words, such as `screenshot` or `wait for text`",
        ));
    };
    let total = found.matches.len();
    let shown: Vec<&ApiEntry> = found
        .matches
        .iter()
        .take(limit)
        .map(|(_, entry)| *entry)
        .collect();
    let mut result = json!({
        "query": query,
        "engine": engine,
        "index": {
            "entries": index.len(),
            "library_version": env!("CARGO_PKG_VERSION"),
            "covers": "BaseCase (WebDriver) and the pure CDP engine: Browser, Page, Locator, BrowserContext",
        },
        "found": total,
        "returned": shown.len(),
        "matches": shown,
    });
    if total == 0 {
        result["note"] = Value::from(
            "No public method of this version matches. Use fewer or different words, search \
             engine `any`, or try parity_lookup with the Python name.",
        );
    } else if total > shown.len() {
        result["note"] = Value::from(format!(
            "{} more matches were cut by limit; narrow the query or raise the limit.",
            total - shown.len()
        ));
    }
    json_output(result)
}

// ----------------------------------------------------------------------
// parity_lookup
// ----------------------------------------------------------------------

/// The parsed manifest, in file order of its sections.
fn parity() -> &'static toml::Table {
    static PARITY: OnceLock<toml::Table> = OnceLock::new();
    PARITY.get_or_init(|| PARITY_TOML.parse().unwrap_or_default())
}

/// What a section of the manifest lists, in words.
fn surface_meaning(surface: &str) -> &'static str {
    match surface {
        "basecase" => "a method of Python BaseCase and SB() (self.click, sb.click)",
        "cdp" => "a method of Python CDP Mode (sb.cdp.click)",
        "driver" => "a method of Python Driver()",
        "cli" => "a Python SeleniumBase command (sbase, seleniumbase)",
        "options_pytest" => "a pytest option of Python SeleniumBase",
        "options_sb" => "a keyword argument of Python SB()",
        "options_driver" => "a keyword argument of Python Driver()",
        "mcp_cdp" => "a tool of the upstream cdp MCP server",
        "mcp_driver" => "a tool of the upstream driver MCP server",
        "mcp_sb" => "a tool of the upstream sb MCP server",
        _ => "a Python name",
    }
}

/// `raw` as a bare Python name, and the section it is limited to, when the
/// prefix says so: `self.click` and `sb.click()` are `click`; `sb.cdp.click`
/// is `click` in `cdp`; `sbase mkdir` is `mkdir` in `cli`.
fn normalize(raw: &str) -> (String, Option<&'static str>) {
    let mut name = raw.trim();
    name = name.split('(').next().unwrap_or(name).trim();
    name = name.trim_start_matches("await ").trim();
    let mut section = None;
    loop {
        let before = name.len();
        for prefix in ["self.", "sb.", "SB.", "BaseCase.", "sb_cdp."] {
            name = name.strip_prefix(prefix).unwrap_or(name);
        }
        if let Some(rest) = name.strip_prefix("cdp.") {
            name = rest;
            section = Some("cdp");
        } else if let Some(rest) = name.strip_prefix("driver.") {
            name = rest;
            section = Some("driver");
        }
        for prefix in ["sbase ", "seleniumbase ", "sbrs "] {
            if let Some(rest) = name.strip_prefix(prefix) {
                name = rest.trim();
                section = Some("cli");
            }
        }
        if name.len() == before {
            break;
        }
    }
    (name.to_owned(), section)
}

/// One entry of the manifest as the tool shows it.
fn entry_json(surface: &str, name: &str, entry: &toml::Table) -> Value {
    let mut shown = Map::new();
    shown.insert("python_name".into(), Value::from(name));
    shown.insert("section".into(), Value::from(surface));
    shown.insert("is".into(), Value::from(surface_meaning(surface)));
    for key in ["status", "rust", "note", "reason", "divergence"] {
        if let Some(text) = entry.get(key).and_then(toml::Value::as_str) {
            shown.insert(key.into(), Value::from(text));
        }
    }
    Value::Object(shown)
}

/// Every entry of the manifest as `(section, name, entry)`.
fn entries(table: &toml::Table) -> impl Iterator<Item = (&str, &str, &toml::Table)> {
    table.iter().flat_map(|(surface, names)| {
        names
            .as_table()
            .into_iter()
            .flat_map(move |names| names.iter())
            .filter_map(move |(name, entry)| {
                entry
                    .as_table()
                    .map(|entry| (surface.as_str(), name.as_str(), entry))
            })
    })
}

/// The answer for `raw` from `table`: exact matches, close names and the
/// nearest spelling.
pub(super) fn lookup(table: &toml::Table, raw: &str) -> Value {
    let (name, section) = normalize(raw);
    let in_section = |surface: &str| section.is_none_or(|wanted| wanted == surface);
    let exact: Vec<Value> = entries(table)
        .filter(|(surface, entry_name, _)| in_section(surface) && *entry_name == name)
        .map(|(surface, name, entry)| entry_json(surface, name, entry))
        .collect();
    let lower = name.to_lowercase();
    let exact = if exact.is_empty() {
        entries(table)
            .filter(|(surface, entry_name, _)| {
                in_section(surface) && entry_name.to_lowercase() == lower
            })
            .map(|(surface, name, entry)| entry_json(surface, name, entry))
            .collect()
    } else {
        exact
    };
    let mut result = json!({
        "python_name": raw,
        "looked_up": name,
        "python_version": crate::UPSTREAM_SELENIUMBASE_VERSION,
        "found": !exact.is_empty(),
        "matches": exact,
    });
    if result["found"] == Value::Bool(true) {
        return result;
    }
    let close: Vec<Value> = entries(table)
        .filter(|(surface, entry_name, _)| {
            in_section(surface)
                && !lower.is_empty()
                && (entry_name.to_lowercase().contains(&lower)
                    || (entry_name.len() >= 4 && lower.contains(&entry_name.to_lowercase())))
        })
        .take(10)
        .map(|(surface, name, entry)| entry_json(surface, name, entry))
        .collect();
    let nearest = closest_match(
        &name,
        entries(table)
            .filter(|(surface, _, _)| in_section(surface))
            .map(|(_, name, _)| name),
    );
    result["close_matches"] = Value::from(close);
    if let Some(nearest) = nearest {
        result["did_you_mean"] = Value::from(nearest);
    }
    result["note"] = Value::from(
        "No Python SeleniumBase 4.55.2 name matches exactly. The close matches contain the name, \
         or look like a typo of it; find_api searches the Rust methods by name or by what they do.",
    );
    result
}

async fn parity_lookup(_ctx: Ctx<NoBrowser>, args: Args) -> Result<Output, ToolError> {
    let raw = args.str("python_name")?;
    if raw.trim().is_empty() {
        return Err(ToolError::invalid("python_name", "give a Python name"));
    }
    json_output(lookup(parity(), raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, owner: &str, engine: &str, doc: &str) -> ApiEntry {
        ApiEntry {
            name: name.into(),
            owner: owner.into(),
            engine: engine.into(),
            signature: format!("pub async fn {name}(&self)"),
            doc: doc.into(),
            is_async: true,
            file: "src/x.rs".into(),
            feature: None,
        }
    }

    fn sample() -> Vec<ApiEntry> {
        vec![
            entry("click", "BaseCase", "basecase", "Clicks an element."),
            entry("click", "Page", "cdp", "Clicks the first match."),
            entry("double_click", "BaseCase", "basecase", "Clicks twice."),
            entry("save_screenshot", "BaseCase", "basecase", "Saves a PNG."),
            entry(
                "wait_for_text",
                "BaseCase",
                "basecase",
                "Waits until text shows.",
            ),
            entry(
                "assert_text",
                "BaseCase",
                "basecase",
                "Fails unless text is visible.",
            ),
        ]
    }

    fn names(found: &Found<'_>) -> Vec<String> {
        found
            .matches
            .iter()
            .map(|(_, entry)| format!("{}::{}", entry.owner, entry.name))
            .collect()
    }

    #[test]
    fn an_exact_name_ranks_before_a_name_that_contains_it() {
        let index = sample();
        let found = search(&index, "click", "any").unwrap();
        assert_eq!(
            names(&found),
            ["BaseCase::click", "Page::click", "BaseCase::double_click"]
        );
        assert_eq!(found.matches[0].0, 100);
        assert_eq!(found.matches[2].0, 80);
    }

    #[test]
    fn every_word_of_the_query_counts_and_the_doc_is_searched_last() {
        let index = sample();
        let found = search(&index, "text wait", "any").unwrap();
        assert_eq!(names(&found)[0], "BaseCase::wait_for_text");
        assert_eq!(found.matches[0].0, 60);

        // Only the documentation mentions "png"; the name does not.
        let found = search(&index, "png", "any").unwrap();
        assert_eq!(names(&found), ["BaseCase::save_screenshot"]);
        assert_eq!(found.matches[0].0, 40);

        // A word that is nowhere finds nothing, even if another word matches.
        let found = search(&index, "click banana", "any").unwrap();
        assert_eq!(names(&found), Vec::<String>::new());
    }

    #[test]
    fn the_engine_and_the_owner_narrow_the_search() {
        let index = sample();
        let cdp = search(&index, "click", "cdp").unwrap();
        assert_eq!(names(&cdp), ["Page::click"]);
        let basecase = search(&index, "click", "basecase").unwrap();
        assert!(names(&basecase).iter().all(|n| n.starts_with("BaseCase")));
        let page = search(&index, "Page::click", "any").unwrap();
        assert_eq!(names(&page), ["Page::click"]);
    }

    #[test]
    fn a_query_without_words_is_not_a_search() {
        assert!(search(&sample(), "  ::  ", "any").is_none());
        assert!(search(&sample(), "", "any").is_none());
    }

    #[test]
    fn the_embedded_index_is_valid_and_not_empty() {
        let embedded: Result<Vec<ApiEntry>, _> = serde_json::from_str(INDEX_JSON);
        let embedded = embedded.expect("api_index.json parses");
        assert!(
            embedded.len() > 500,
            "the index has only {} entries; run SB_UPDATE_API_INDEX=1 cargo test api_index",
            embedded.len()
        );
        assert!(embedded
            .iter()
            .any(|e| e.owner == "BaseCase" && e.name == "click"));
        assert!(embedded
            .iter()
            .any(|e| e.owner == "Page" && e.engine == "cdp"));
    }

    fn manifest() -> toml::Table {
        r#"
[basecase."click"]
status = "implemented"
rust = "BaseCase::click"

[basecase."assert_text"]
status = "implemented"
rust = "BaseCase::assert_text"

[basecase."uc_open_with_reconnect"]
status = "composed"
rust = "BaseCase::uc_open"
note = "Reconnects after the page loads."
divergence = "The reconnect time is fixed."

[cdp."click"]
status = "implemented"
rust = "Page::click"

[cli."mkdir"]
status = "implemented"
rust = "sbrs mkdir"

[basecase."get_pdf_text"]
status = "planned"
note = "Needs the pdf feature."
"#
        .parse()
        .unwrap()
    }

    #[test]
    fn prefixes_and_calls_are_stripped_from_a_python_name() {
        assert_eq!(normalize("self.click"), ("click".into(), None));
        assert_eq!(normalize("sb.click('#a')"), ("click".into(), None));
        assert_eq!(
            normalize("await sb.assert_text(x)"),
            ("assert_text".into(), None)
        );
        assert_eq!(normalize("sb.cdp.click"), ("click".into(), Some("cdp")));
        assert_eq!(normalize("sbase mkdir"), ("mkdir".into(), Some("cli")));
        assert_eq!(normalize("  click  "), ("click".into(), None));
    }

    #[test]
    fn an_exact_name_gives_the_status_and_the_rust_path_in_every_section() {
        let table = manifest();
        let answer = lookup(&table, "self.click");
        assert_eq!(answer["found"], true);
        let sections: Vec<&str> = answer["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["section"].as_str().unwrap())
            .collect();
        assert_eq!(sections, ["basecase", "cdp"]);
        assert_eq!(answer["matches"][0]["rust"], "BaseCase::click");
        assert_eq!(answer["matches"][0]["status"], "implemented");
        assert_eq!(answer["python_version"], "4.55.2");

        let only_cdp = lookup(&table, "sb.cdp.click");
        assert_eq!(only_cdp["matches"].as_array().unwrap().len(), 1);
        assert_eq!(only_cdp["matches"][0]["rust"], "Page::click");
    }

    #[test]
    fn notes_and_divergence_come_with_the_entry() {
        let answer = lookup(&manifest(), "uc_open_with_reconnect");
        assert_eq!(answer["matches"][0]["status"], "composed");
        assert_eq!(
            answer["matches"][0]["note"],
            "Reconnects after the page loads."
        );
        assert_eq!(
            answer["matches"][0]["divergence"],
            "The reconnect time is fixed."
        );
    }

    #[test]
    fn with_no_exact_name_the_answer_lists_close_ones_and_the_nearest_spelling() {
        let answer = lookup(&manifest(), "asert_text");
        assert_eq!(answer["found"], false);
        assert_eq!(answer["did_you_mean"], "assert_text");

        let answer = lookup(&manifest(), "reconnect");
        assert_eq!(answer["found"], false);
        let close = answer["close_matches"].as_array().unwrap();
        assert_eq!(close[0]["python_name"], "uc_open_with_reconnect");
    }

    #[test]
    fn the_embedded_manifest_knows_click() {
        let answer = lookup(parity(), "click");
        assert_eq!(answer["found"], true, "{answer}");
    }

    #[tokio::test]
    async fn the_tools_reject_empty_and_out_of_range_arguments() {
        let host = super::super::host(crate::mcp::Settings::new("out"));
        let args = |value: Value| value.as_object().cloned().unwrap();

        let error = host
            .call("find_api", args(json!({ "query": "click", "limit": 0 })))
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::InvalidArgument { .. }),
            "{error}"
        );
        let error = host
            .call("find_api", args(json!({ "query": "!!!" })))
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::InvalidArgument { .. }),
            "{error}"
        );
        let error = host
            .call("parity_lookup", args(json!({ "python_name": "  " })))
            .await
            .unwrap_err();
        assert!(
            matches!(error, ToolError::InvalidArgument { .. }),
            "{error}"
        );
    }
}
