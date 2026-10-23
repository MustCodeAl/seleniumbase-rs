//! Tools that read a page the way a model needs it: a snapshot with element
//! refs, readable text, and the links, tables and forms as data.
//!
//! - `page_snapshot` lists the page as an accessibility tree (or, with
//!   `mode: "outline"`, as the tag tree). Interactive nodes get refs such as
//!   `e5`; any selector argument of any tool may then be written `ref=e5`,
//!   which the host checks against the page and replaces (see
//!   [`refs`](super::refs)).
//! - `read_page_text` gives the main content as Markdown.
//! - `extract_links`, `extract_tables` and `extract_forms` return data.
//!
//! All of them only look: they leave nothing behind in the page, they cap
//! what they return and say when they cut it, and their output is marked
//! untrusted because it is the page's own text. The tools are generic over
//! [`DevSession`], so the `cdp`, `driver` and `sb` servers share them; the
//! page side is `devtools.js`.

use std::fmt::Write as _;

use serde_json::{json, Map, Value};

use super::devtools::{bounded, page_op, tagged, DevSession};
use super::host::Ctx;
use super::refs::{RefEntry, RefFuture, RefProbe, RefTable};
use super::support::nonempty;
use super::{Args, Effect, Output, Prop, Schema, ToolDef, ToolError};
use crate::utils::shadow::split_shadow_selector;

/// The most characters `read_page_text` may be asked for.
const MAX_TEXT_CHARS: usize = 200_000;

/// The fewest characters `read_page_text` may be asked for.
const MIN_TEXT_CHARS: usize = 1_000;

fn scope_prop() -> Prop {
    Prop::string("Only look inside the first element this selector (or ref=eN) matches")
}

/// The page tools, for a server whose session is `S`.
#[expect(
    clippy::too_many_lines,
    reason = "the tool table reads best as one list"
)]
pub(super) fn tools<S: DevSession>() -> Vec<ToolDef<S>> {
    vec![
        ToolDef::new(
            "page_snapshot",
            "Page Snapshot",
            "Read the page as a compact accessibility tree: landmarks, headings, links, buttons \
             and form controls with their roles, names, state and values. Interactive nodes \
             carry a ref such as [ref=e5]; pass \"ref=e5\" as the selector of click_element, \
             type_text and the other tools. Call it again after the page changes: refs belong to \
             the last snapshot. mode \"outline\" shows the tag tree instead, for pages with poor \
             semantics. Passwords and card numbers are never printed.",
            Effect::Observe,
            Schema::new()
                .optional("scope", scope_prop())
                .optional(
                    "mode",
                    Prop::choice(
                        &["accessibility", "outline"],
                        "The accessibility tree, or the DOM tag tree",
                    )
                    .default("accessibility"),
                )
                .optional(
                    "depth",
                    Prop::integer("Deepest nesting to list, 1 to 30")
                        .min(1.0)
                        .default(12),
                )
                .optional(
                    "max_nodes",
                    Prop::integer("Most nodes to list, 1 to 2000")
                        .min(1.0)
                        .default(400),
                )
                .optional(
                    "include_hidden",
                    Prop::boolean("Also list nodes that are not visible").default(false),
                ),
            page_snapshot::<S>,
        )
        .with_output_schema(snapshot_schema())
        .untrusted(),
        ToolDef::new(
            "read_page_text",
            "Read Page Text",
            "Read the readable text of the page as Markdown: the main content (main, article, or \
             the densest block), headings as #, links as [text](url), lists and tables. Scripts, \
             styles and hidden nodes are skipped. Cut at max_chars on a block boundary and says \
             how much more there was.",
            Effect::Observe,
            Schema::new()
                .optional("scope", scope_prop())
                .optional(
                    "format",
                    Prop::choice(&["markdown", "text"], "Markdown, or plain text")
                        .default("markdown"),
                )
                .optional(
                    "main_only",
                    Prop::boolean("Only the main content, without navigation and footers")
                        .default(true),
                )
                .optional(
                    "max_chars",
                    Prop::integer("Most characters to return, 1000 to 200000")
                        .min(1000.0)
                        .default(20_000),
                ),
            read_page_text::<S>,
        )
        .with_output_schema(text_schema())
        .untrusted(),
        ToolDef::new(
            "extract_links",
            "Extract Links",
            "List the visible links of the page with their text, absolute URL, rel, target and \
             the landmark they sit in. Duplicates are collapsed with a count; javascript: links \
             are flagged. The first 100 links carry a ref.",
            Effect::Observe,
            Schema::new()
                .optional("scope", scope_prop())
                .optional(
                    "same_origin",
                    Prop::boolean("Only links to the page's own origin").default(false),
                )
                .optional(
                    "contains",
                    Prop::string("Only links whose text or URL contains this, in any case"),
                )
                .optional(
                    "limit",
                    Prop::integer("Most links to return, 1 to 2000")
                        .min(1.0)
                        .default(200),
                ),
            extract_links::<S>,
        )
        .with_output_schema(links_schema())
        .untrusted(),
        ToolDef::new(
            "extract_tables",
            "Extract Tables",
            "Read the visible tables (HTML tables and role=table or grid) as data: caption, \
             headers and rows, with colspan and rowspan expanded. Each table also carries a \
             selector and a ref.",
            Effect::Observe,
            Schema::new()
                .optional("scope", scope_prop())
                .optional(
                    "limit",
                    Prop::integer("Most tables to return, 1 to 100")
                        .min(1.0)
                        .default(20),
                )
                .optional(
                    "max_rows",
                    Prop::integer("Most rows per table, 1 to 5000")
                        .min(1.0)
                        .default(200),
                ),
            extract_tables::<S>,
        )
        .with_output_schema(tables_schema())
        .untrusted(),
        ToolDef::new(
            "extract_forms",
            "Extract Forms",
            "Read the forms of the page as data: action, method and each field with its name, \
             type, label, required flag, value, checked state and options, plus the submit \
             button. Fields and forms carry refs. Password and card values are left out.",
            Effect::Observe,
            Schema::new().optional("scope", scope_prop()),
            extract_forms::<S>,
        )
        .with_output_schema(forms_schema())
        .untrusted(),
    ]
}

/// Looks in the page for the element of a ref: how many elements its
/// selector finds, and the role and name of the first.
pub(super) fn verify<S: DevSession>(ctx: Ctx<S>, entry: RefEntry) -> RefFuture {
    Box::pin(async move {
        let mut session = ctx.session().await?;
        let parts: Vec<Value> = split_shadow_selector(&entry.selector)
            .iter()
            .map(|fragment| tagged(fragment))
            .collect();
        if parts.len() > 1 && session.engine() == "cdp" {
            return Ok(RefProbe {
                count: 0,
                role: String::new(),
                name: String::new(),
                url: String::new(),
                unsupported: Some(
                    "it is inside a shadow root, which the Pure CDP engine cannot select; the \
                     driver and sb servers can"
                        .to_owned(),
                ),
            });
        }
        let answer = page_op(&mut *session, "verify_ref", &json!({ "parts": parts })).await?;
        Ok(RefProbe {
            count: answer["count"]
                .as_u64()
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or(0),
            role: text_of(&answer, "role"),
            name: text_of(&answer, "name"),
            url: text_of(&answer, "url"),
            unsupported: None,
        })
    })
}

// ----------------------------------------------------------------------
// Handlers
// ----------------------------------------------------------------------

/// Runs one page operation. A scope the page rejects is the caller's mistake.
async fn run<S: DevSession>(ctx: &Ctx<S>, op: &str, args: &Value) -> Result<Value, ToolError> {
    let mut session = ctx.session().await?;
    let answer = page_op(&mut *session, op, args).await?;
    drop(session);
    match answer.get("error").and_then(Value::as_str) {
        Some(error) => Err(ToolError::invalid("scope", error)),
        None => Ok(answer),
    }
}

async fn page_snapshot<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let mode = args.choice("mode", &["accessibility", "outline"], "accessibility")?;
    let depth = bounded(&args, "depth", 12, 30)?;
    let max_nodes = bounded(&args, "max_nodes", 400, 2000)?;
    let include_hidden = args.bool_or("include_hidden", false)?;
    let scope = nonempty(&args, "scope")?.map(tagged);
    let outline = mode == "outline";
    let page = run(
        &ctx,
        if outline { "outline" } else { "snapshot" },
        &json!({
            "scope": scope,
            "depth": depth,
            "max_nodes": max_nodes,
            "include_hidden": include_hidden,
        }),
    )
    .await?;
    Ok(snapshot_output(&ctx, &page, outline))
}

async fn read_page_text<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let format = args.choice("format", &["markdown", "text"], "markdown")?;
    let main_only = args.bool_or("main_only", true)?;
    let max_chars = match args.opt_usize("max_chars")? {
        None => 20_000,
        Some(n) if (MIN_TEXT_CHARS..=MAX_TEXT_CHARS).contains(&n) => n,
        Some(_) => {
            return Err(ToolError::invalid(
                "max_chars",
                format!("must be between {MIN_TEXT_CHARS} and {MAX_TEXT_CHARS}"),
            ))
        }
    };
    let scope = nonempty(&args, "scope")?.map(tagged);
    let page = run(
        &ctx,
        "text",
        &json!({
            "scope": scope,
            "format": format,
            "main_only": main_only,
            "max_chars": max_chars,
        }),
    )
    .await?;
    let text = text_of(&page, "text");
    ctx.refs().set_page_text(text.clone());
    let data = json!({
        "url": page["url"],
        "title": page["title"],
        "source": page["source"],
        "format": page["format"],
        "chars": page["chars"],
        "total_chars": page["total_chars"],
        "truncated": page["truncated"],
    });
    Ok(Output::Structured { text, data })
}

async fn extract_links<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let limit = bounded(&args, "limit", 200, 2000)?;
    let scope = nonempty(&args, "scope")?.map(tagged);
    let same_origin = args.bool_or("same_origin", false)?;
    let contains = nonempty(&args, "contains")?;
    let mut page = run(
        &ctx,
        "links",
        &json!({
            "scope": scope,
            "same_origin": same_origin,
            "contains": contains,
            "limit": limit,
        }),
    )
    .await?;
    register_refs(&ctx, &mut page, false);
    Ok(Output::Structured {
        text: links_text(&page),
        data: page,
    })
}

async fn extract_tables<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let limit = bounded(&args, "limit", 20, 100)?;
    let max_rows = bounded(&args, "max_rows", 200, 5000)?;
    let scope = nonempty(&args, "scope")?.map(tagged);
    let mut page = run(
        &ctx,
        "tables",
        &json!({ "scope": scope, "limit": limit, "max_rows": max_rows }),
    )
    .await?;
    register_refs(&ctx, &mut page, true);
    Ok(Output::Structured {
        text: tables_text(&page),
        data: page,
    })
}

async fn extract_forms<S: DevSession>(ctx: Ctx<S>, args: Args) -> Result<Output, ToolError> {
    let scope = nonempty(&args, "scope")?.map(tagged);
    let mut page = run(&ctx, "forms", &json!({ "scope": scope })).await?;
    register_refs(&ctx, &mut page, true);
    Ok(Output::Structured {
        text: forms_text(&page),
        data: page,
    })
}

/// Gives the elements an extract tool found refs in the table of the last
/// snapshot, after the ones it has.
fn register_refs<S: Send + 'static>(ctx: &Ctx<S>, page: &mut Value, keep_selector: bool) {
    let url = text_of(page, "url");
    assign_refs(&mut ctx.refs(), &url, page, keep_selector);
}

/// Walks `value` for objects the page script gave a `selector`, `ref_role`
/// and `ref_name`: each gets the next ref and loses the two helper fields (and
/// the selector, unless `keep_selector`).
fn assign_refs(table: &mut RefTable, url: &str, value: &mut Value, keep_selector: bool) {
    match value {
        Value::Array(items) => {
            for item in items {
                assign_refs(table, url, item, keep_selector);
            }
        }
        Value::Object(map) => {
            let own = match (map.get("selector"), map.get("ref_role")) {
                (Some(Value::String(selector)), Some(Value::String(role))) => {
                    let name = map
                        .get("ref_name")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    Some((selector.clone(), role.clone(), name.to_owned()))
                }
                _ => None,
            };
            let registered = own.is_some();
            if let Some((selector, role, name)) = own {
                let id = table.push(url, selector, role, name);
                map.insert("ref".to_owned(), Value::String(id));
            }
            // The page listed parents before children and fields before the
            // submit button; the helper fields come out only afterwards, so
            // removing them does not reorder the rest.
            for child in map.values_mut() {
                assign_refs(table, url, child, keep_selector);
            }
            map.remove("ref_role");
            map.remove("ref_name");
            if registered && !keep_selector {
                map.remove("selector");
            }
        }
        _ => {}
    }
}

// ----------------------------------------------------------------------
// The snapshot
// ----------------------------------------------------------------------

fn text_of(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// `text` as a JSON string, which is unambiguous on one line.
fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| format!("\"{text}\""))
}

fn str_field<'a>(node: &'a Map<String, Value>, key: &str) -> &'a str {
    node.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn flag(node: &Map<String, Value>, key: &str) -> Option<bool> {
    node.get(key).and_then(Value::as_bool)
}

/// One node of the accessibility tree as a line.
fn accessibility_line(node: &Map<String, Value>) -> String {
    let mut line = format!("- {}", str_field(node, "role"));
    let name = str_field(node, "name");
    if !name.is_empty() {
        let _ = write!(line, " {}", quoted(name));
    }
    if let Some(id) = node.get("ref").and_then(Value::as_str) {
        let _ = write!(line, " [ref={id}]");
    }
    if let Some(level) = node.get("level").and_then(Value::as_u64) {
        let _ = write!(line, " [level={level}]");
    }
    for key in ["required", "readonly", "selected", "pressed"] {
        if flag(node, key) == Some(true) {
            let _ = write!(line, " [{key}]");
        }
    }
    match node.get("checked") {
        Some(Value::Bool(true)) => line.push_str(" [checked]"),
        Some(Value::String(state)) => {
            let _ = write!(line, " [checked={state}]");
        }
        _ => {}
    }
    match flag(node, "expanded") {
        Some(true) => line.push_str(" [expanded]"),
        Some(false) => line.push_str(" [collapsed]"),
        None => {}
    }
    if let Some(current) = node.get("current").and_then(Value::as_str) {
        let _ = write!(line, " [current={current}]");
    }
    if flag(node, "enabled") == Some(false) {
        line.push_str(" [disabled]");
    }
    if flag(node, "visible") == Some(false) {
        line.push_str(" [hidden]");
    }
    if flag(node, "shadow") == Some(true) {
        line.push_str(" [shadow]");
    }
    for key in ["value", "href", "src"] {
        if let Some(text) = node.get(key).and_then(Value::as_str) {
            let _ = write!(line, " {key}={}", quoted(text));
        }
    }
    line
}

/// One node of the tag tree as a line.
fn outline_line(node: &Map<String, Value>) -> String {
    let mut line = format!("- {}", str_field(node, "tag"));
    if let Some(id) = node.get("id").and_then(Value::as_str) {
        let _ = write!(line, "#{id}");
    }
    if let Some(classes) = node.get("cls").and_then(Value::as_str) {
        let _ = write!(line, ".{classes}");
    }
    if let Some(id) = node.get("ref").and_then(Value::as_str) {
        let _ = write!(line, " [ref={id}]");
    }
    let _ = write!(
        line,
        " children={} text={}",
        node.get("children").and_then(Value::as_u64).unwrap_or(0),
        node.get("text_len").and_then(Value::as_u64).unwrap_or(0)
    );
    let role = str_field(node, "role");
    if !role.is_empty() && role != "generic" {
        let _ = write!(line, " ({role}");
        let name = str_field(node, "name");
        if !name.is_empty() {
            let _ = write!(line, " {}", quoted(name));
        }
        line.push(')');
    }
    if flag(node, "visible") == Some(false) {
        line.push_str(" [hidden]");
    }
    line
}

/// The whole snapshot as text, `number` being its snapshot number.
fn render_snapshot(
    title: &str,
    url: &str,
    number: u64,
    outline: bool,
    nodes: &[Map<String, Value>],
    more: u64,
    note: Option<&str>,
) -> String {
    let mut out = String::new();
    let _ = write!(out, "- page {} url={url} snapshot={number}", quoted(title));
    if outline {
        out.push_str(" mode=outline");
    }
    for node in nodes {
        let depth = node
            .get("d")
            .and_then(Value::as_u64)
            .and_then(|depth| usize::try_from(depth).ok())
            .unwrap_or(0);
        out.push('\n');
        out.push_str(&"  ".repeat(depth + 1));
        out.push_str(&if outline {
            outline_line(node)
        } else {
            accessibility_line(node)
        });
    }
    if more > 0 {
        let _ = write!(
            out,
            "\n  - ... {more} more nodes (raise max_nodes or pass scope)"
        );
    }
    if let Some(note) = note {
        let _ = write!(out, "\n  - note: {note}");
    }
    out
}

/// What a client reads about a node that has a ref.
fn node_data(node: &Map<String, Value>) -> Value {
    let mut data = Map::new();
    for key in [
        "ref", "role", "name", "selector", "visible", "enabled", "value", "checked", "shadow",
    ] {
        if let Some(value) = node.get(key) {
            data.insert(key.to_owned(), value.clone());
        }
    }
    data.insert("frame".to_owned(), Value::Null);
    Value::Object(data)
}

/// Numbers the refs of a snapshot, stores them for the next call, and draws
/// the snapshot.
fn snapshot_output<S: Send + 'static>(ctx: &Ctx<S>, page: &Value, outline: bool) -> Output {
    let mut nodes: Vec<Map<String, Value>> = page["nodes"]
        .as_array()
        .map(|list| list.iter().filter_map(|node| node.as_object().cloned()).collect())
        .unwrap_or_default();
    let mut entries = Vec::new();
    for node in &mut nodes {
        let selector = str_field(node, "selector").to_owned();
        if selector.is_empty() {
            continue;
        }
        let id = format!("e{}", entries.len() + 1);
        entries.push(RefEntry {
            id: id.clone(),
            selector,
            role: str_field(node, "role").to_owned(),
            name: str_field(node, "name").to_owned(),
        });
        node.insert("ref".to_owned(), Value::String(id));
    }
    let url = text_of(page, "url");
    let title = text_of(page, "title");
    let more = page["more"].as_u64().unwrap_or(0);
    let note = page["note"].as_str();
    let mut number = 0;
    let text = ctx.refs().replace(url.clone(), entries, |snapshot| {
        number = snapshot;
        render_snapshot(&title, &url, snapshot, outline, &nodes, more, note)
    });
    let listed: Vec<Value> = nodes
        .iter()
        .filter(|node| node.contains_key("ref"))
        .map(node_data)
        .collect();
    let data = json!({
        "url": url,
        "title": title,
        "mode": if outline { "outline" } else { "accessibility" },
        "snapshot": number,
        "truncated": page["truncated"].as_bool().unwrap_or(false),
        "more": more,
        "nodes": listed,
    });
    Output::Structured { text, data }
}

// ----------------------------------------------------------------------
// Text for the extract tools
// ----------------------------------------------------------------------

fn items<'a>(page: &'a Value, key: &str) -> &'a [Value] {
    page.get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn ref_suffix(value: &Value) -> String {
    value
        .get("ref")
        .and_then(Value::as_str)
        .map_or_else(String::new, |id| format!(" [ref={id}]"))
}

fn links_text(page: &Value) -> String {
    let links = items(page, "links");
    let total = page["total"].as_u64().unwrap_or(0);
    let mut out = format!("{total} links");
    if page["truncated"].as_bool().unwrap_or(false) {
        let _ = write!(
            out,
            " (showing {}; raise limit or narrow with scope, same_origin or contains)",
            links.len()
        );
    }
    for link in links {
        let _ = write!(
            out,
            "\n- {} {}{}",
            quoted(&text_of(link, "text")),
            text_of(link, "href"),
            ref_suffix(link)
        );
        let region = text_of(link, "region");
        if !region.is_empty() {
            let _ = write!(out, " in {region}");
        }
        if link["kind"] == "script" {
            out.push_str(" (script)");
        }
        if let Some(count) = link["count"].as_u64() {
            let _ = write!(out, " x{count}");
        }
    }
    out
}

/// A row of cells as a Markdown table row.
fn table_row(cells: &[Value], width: usize) -> String {
    let cells: Vec<String> = (0..width)
        .map(|column| {
            cells
                .get(column)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .replace('|', "\\|")
        })
        .collect();
    format!("| {} |", cells.join(" | "))
}

fn tables_text(page: &Value) -> String {
    let tables = items(page, "tables");
    if tables.is_empty() {
        return "no visible tables".to_owned();
    }
    let mut blocks = Vec::new();
    for (index, table) in tables.iter().enumerate() {
        let headers = items(table, "headers");
        let rows = items(table, "rows");
        let mut block = format!("Table {}", index + 1);
        let caption = text_of(table, "caption");
        if !caption.is_empty() {
            let _ = write!(block, " {}", quoted(&caption));
        }
        let _ = write!(
            block,
            "{}: {} rows",
            ref_suffix(table),
            table["row_count"].as_u64().unwrap_or(0)
        );
        if table["truncated"].as_bool().unwrap_or(false) {
            let _ = write!(block, " (showing {}; raise max_rows)", rows.len());
        }
        let width = rows
            .iter()
            .filter_map(Value::as_array)
            .map(Vec::len)
            .chain(std::iter::once(headers.len()))
            .max()
            .unwrap_or(0);
        if width > 0 {
            let head: &[Value] = headers;
            let _ = write!(block, "\n{}", table_row(head, width));
            let _ = write!(block, "\n|{}", " --- |".repeat(width));
            for row in rows {
                let cells = row.as_array().map_or(&[][..], Vec::as_slice);
                let _ = write!(block, "\n{}", table_row(cells, width));
            }
        }
        blocks.push(block);
    }
    let more = page["more_tables"].as_u64().unwrap_or(0);
    if more > 0 {
        blocks.push(format!("... {more} more tables (raise limit or pass scope)"));
    }
    blocks.join("\n\n")
}

fn forms_text(page: &Value) -> String {
    let forms = items(page, "forms");
    if forms.is_empty() {
        return "no forms".to_owned();
    }
    let mut out = Vec::new();
    for form in forms {
        let mut block = format!(
            "form{} {} {}",
            ref_suffix(form),
            text_of(form, "method").to_uppercase(),
            quoted(&text_of(form, "action"))
        );
        for field in items(form, "fields") {
            let _ = write!(
                block,
                "\n  - {} ({}) {}{}",
                text_of(field, "name"),
                text_of(field, "type"),
                quoted(&text_of(field, "label")),
                ref_suffix(field)
            );
            if field["required"].as_bool().unwrap_or(false) {
                block.push_str(" [required]");
            }
            if field["checked"].as_bool() == Some(true) {
                block.push_str(" [checked]");
            }
            if let Some(value) = field["value"].as_str() {
                let _ = write!(block, " value={}", quoted(value));
            }
            let options = items(field, "options");
            if !options.is_empty() {
                let labels: Vec<String> = options
                    .iter()
                    .map(|option| {
                        let label = text_of(option, "label");
                        if option["selected"].as_bool() == Some(true) {
                            format!("{label}*")
                        } else {
                            label
                        }
                    })
                    .collect();
                let _ = write!(block, " options: {}", labels.join(", "));
            }
        }
        if let Some(submit) = form.get("submit") {
            let _ = write!(
                block,
                "\n  submit: {}{}",
                quoted(&text_of(submit, "label")),
                ref_suffix(submit)
            );
        }
        out.push(block);
    }
    out.join("\n\n")
}

// ----------------------------------------------------------------------
// Output schemas
// ----------------------------------------------------------------------

fn snapshot_schema() -> Value {
    json!({
        "type": "object",
        "required": ["url", "title", "snapshot", "truncated", "nodes"],
        "properties": {
            "url": { "type": "string" },
            "title": { "type": "string" },
            "mode": { "type": "string", "enum": ["accessibility", "outline"] },
            "snapshot": { "type": "integer", "minimum": 1 },
            "truncated": { "type": "boolean" },
            "more": { "type": "integer", "minimum": 0 },
            "nodes": {
                "type": "array",
                "description": "The nodes that have refs.",
                "items": {
                    "type": "object",
                    "required": ["ref", "role", "name", "selector", "visible", "enabled", "frame"],
                    "properties": {
                        "ref": { "type": "string" },
                        "role": { "type": "string" },
                        "name": { "type": "string" },
                        "selector": { "type": "string" },
                        "visible": { "type": "boolean" },
                        "enabled": { "type": "boolean" },
                        "value": { "type": "string" },
                        "checked": { "type": ["boolean", "string"] },
                        "shadow": { "type": "boolean" },
                        "frame": { "type": "null" }
                    }
                }
            }
        }
    })
}

fn text_schema() -> Value {
    json!({
        "type": "object",
        "required": ["url", "title", "source", "format", "chars", "total_chars", "truncated"],
        "properties": {
            "url": { "type": "string" },
            "title": { "type": "string" },
            "source": { "type": "string" },
            "format": { "type": "string", "enum": ["markdown", "text"] },
            "chars": { "type": "integer", "minimum": 0 },
            "total_chars": { "type": "integer", "minimum": 0 },
            "truncated": { "type": "boolean" }
        }
    })
}

fn links_schema() -> Value {
    json!({
        "type": "object",
        "required": ["links", "total", "truncated"],
        "properties": {
            "links": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["text", "href", "region"],
                    "properties": {
                        "text": { "type": "string" },
                        "href": { "type": "string" },
                        "region": { "type": "string" },
                        "rel": { "type": "string" },
                        "target": { "type": "string" },
                        "kind": { "type": "string", "enum": ["script"] },
                        "count": { "type": "integer", "minimum": 2 },
                        "ref": { "type": "string" }
                    }
                }
            },
            "total": { "type": "integer", "minimum": 0 },
            "truncated": { "type": "boolean" }
        }
    })
}

fn tables_schema() -> Value {
    json!({
        "type": "object",
        "required": ["tables"],
        "properties": {
            "tables": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["headers", "rows", "row_count", "truncated"],
                    "properties": {
                        "caption": { "type": "string" },
                        "selector": { "type": "string" },
                        "ref": { "type": "string" },
                        "headers": { "type": "array", "items": { "type": "string" } },
                        "rows": {
                            "type": "array",
                            "items": { "type": "array", "items": { "type": "string" } }
                        },
                        "row_count": { "type": "integer", "minimum": 0 },
                        "truncated": { "type": "boolean" }
                    }
                }
            },
            "more_tables": { "type": "integer", "minimum": 0 }
        }
    })
}

fn forms_schema() -> Value {
    json!({
        "type": "object",
        "required": ["forms"],
        "properties": {
            "forms": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["action", "method", "fields"],
                    "properties": {
                        "ref": { "type": "string" },
                        "selector": { "type": "string" },
                        "action": { "type": "string" },
                        "method": { "type": "string" },
                        "fields": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["name", "type", "label", "required"],
                                "properties": {
                                    "ref": { "type": "string" },
                                    "name": { "type": "string" },
                                    "type": { "type": "string" },
                                    "label": { "type": "string" },
                                    "required": { "type": "boolean" },
                                    "value": { "type": "string" },
                                    "checked": { "type": "boolean" },
                                    "options": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "required": ["value", "label", "selected"],
                                            "properties": {
                                                "value": { "type": "string" },
                                                "label": { "type": "string" },
                                                "selected": { "type": "boolean" }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        "submit": {
                            "type": "object",
                            "required": ["label"],
                            "properties": {
                                "ref": { "type": "string" },
                                "label": { "type": "string" }
                            }
                        }
                    }
                }
            },
            "more_forms": { "type": "integer", "minimum": 0 }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(value: &Value) -> Map<String, Value> {
        value.as_object().cloned().expect("an object")
    }

    #[test]
    fn an_accessibility_line_shows_the_role_name_state_and_value() {
        let node = object(&json!({
            "role": "textbox", "name": "Email", "ref": "e2", "required": true,
            "value": "a@b.c", "visible": true, "enabled": true,
        }));
        assert_eq!(
            accessibility_line(&node),
            r#"- textbox "Email" [ref=e2] [required] value="a@b.c""#
        );

        let button = object(&json!({
            "role": "button", "name": "Sign in", "ref": "e5", "enabled": false, "visible": true,
        }));
        assert_eq!(
            accessibility_line(&button),
            r#"- button "Sign in" [ref=e5] [disabled]"#
        );

        let heading = object(&json!({ "role": "heading", "name": "Sign in", "level": 1 }));
        assert_eq!(accessibility_line(&heading), r#"- heading "Sign in" [level=1]"#);

        let mixed = object(&json!({ "role": "checkbox", "name": "All", "checked": "mixed" }));
        assert_eq!(accessibility_line(&mixed), r#"- checkbox "All" [checked=mixed]"#);
    }

    #[test]
    fn a_snapshot_is_drawn_with_the_page_line_indents_and_the_cut_note() {
        let nodes: Vec<_> = [
            json!({ "d": 0, "role": "main", "name": "" }),
            json!({ "d": 1, "role": "heading", "name": "Sign in", "level": 1 }),
            json!({ "d": 1, "role": "form", "name": "Login", "ref": "e1" }),
            json!({ "d": 2, "role": "checkbox", "name": "Remember me", "ref": "e4", "checked": true }),
        ]
        .iter()
        .map(object)
        .collect();

        let text = render_snapshot("Sign in", "https://example.test/login", 4, false, &nodes, 132, None);

        assert_eq!(
            text,
            "- page \"Sign in\" url=https://example.test/login snapshot=4\n\
             \x20 - main\n\
             \x20   - heading \"Sign in\" [level=1]\n\
             \x20   - form \"Login\" [ref=e1]\n\
             \x20     - checkbox \"Remember me\" [ref=e4] [checked]\n\
             \x20 - ... 132 more nodes (raise max_nodes or pass scope)"
        );
    }

    #[test]
    fn an_outline_line_shows_the_tag_counts_and_the_role_when_there_is_one() {
        let node = object(&json!({
            "tag": "button", "id": "buy", "cls": "btn.primary", "ref": "e3", "children": 1,
            "text_len": 7, "role": "button", "name": "Buy now",
        }));
        assert_eq!(
            outline_line(&node),
            r#"- button#buy.btn.primary [ref=e3] children=1 text=7 (button "Buy now")"#
        );
        let plain = object(&json!({ "tag": "div", "children": 3, "text_len": 20, "role": "generic" }));
        assert_eq!(outline_line(&plain), "- div children=3 text=20");
    }

    #[test]
    fn refs_found_by_an_extract_tool_continue_the_numbering() {
        let mut table = RefTable::default();
        table.replace(
            "https://a.test/".to_owned(),
            vec![RefEntry {
                id: "e1".to_owned(),
                selector: "#a".to_owned(),
                role: "button".to_owned(),
                name: "A".to_owned(),
            }],
            |n| format!("snapshot {n}"),
        );
        let mut page = json!({
            "links": [
                { "text": "Docs", "href": "https://a.test/d", "selector": "a.docs",
                  "ref_role": "link", "ref_name": "Docs" },
                { "text": "No ref", "href": "https://a.test/n" },
            ],
        });

        assign_refs(&mut table, "https://a.test/", &mut page, false);

        assert_eq!(page["links"][0]["ref"], "e2");
        assert!(page["links"][0].get("selector").is_none(), "links drop the selector");
        assert!(page["links"][0].get("ref_role").is_none());
        assert!(page["links"][1].get("ref").is_none());
        assert_eq!(table.get("e2").unwrap().selector, "a.docs");
        assert_eq!(table.get("e2").unwrap().role, "link");
    }

    #[test]
    fn every_output_schema_is_an_object_whose_required_fields_exist() {
        for schema in [
            snapshot_schema(),
            text_schema(),
            links_schema(),
            tables_schema(),
            forms_schema(),
        ] {
            assert_eq!(schema["type"], "object");
            let properties = schema["properties"].as_object().expect("properties");
            for required in schema["required"].as_array().expect("required") {
                assert!(
                    properties.contains_key(required.as_str().unwrap()),
                    "{required} is required but not described"
                );
            }
        }
    }

    #[test]
    fn a_table_is_drawn_as_markdown_with_escaped_bars() {
        let page = json!({
            "tables": [{
                "caption": "Prices", "ref": "e3", "row_count": 2, "truncated": false,
                "headers": ["Item", "Cost"],
                "rows": [["a|b", "1"], ["c", "2"]],
            }],
        });
        assert_eq!(
            tables_text(&page),
            "Table 1 \"Prices\" [ref=e3]: 2 rows\n| Item | Cost |\n| --- | --- |\n| a\\|b | 1 |\n| c | 2 |"
        );
    }
}
