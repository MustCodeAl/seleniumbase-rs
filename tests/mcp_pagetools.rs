//! The page tools (`page_snapshot`, `read_page_text`, `extract_*`) and the
//! refs they hand out, against a scripted browser, with no Chrome.
//!
//! The page side of the tools is one script; these tests answer it by the
//! operation it is asked for and check what the server makes of the answer:
//! the text a model reads, the structured content, the refs, and the
//! checks a ref goes through before a tool uses it.
//! `tests/mcp_pagetools_chrome.rs` runs the script itself in a real Chrome.
//! Run them with `cargo test --features "mcp-server test-util" --test mcp_pagetools`.

#![cfg(all(feature = "mcp-server", feature = "test-util"))]

use std::path::Path;

use seleniumbase_rs::mcp::cdp::{self, Cdp};
use seleniumbase_rs::mcp::{driver, sb, Host, Output, Settings, ToolError};
use seleniumbase_rs::sb_cdp::{Browser, MockCtrl};
use serde_json::{json, Map, Value};

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

async fn running(output_dir: &Path) -> (Host<Cdp>, MockCtrl) {
    let host = cdp::host(Settings::new(output_dir));
    let (browser, mock) = Browser::new_mocked();
    host.ctx()
        .start(async { Ok::<_, ToolError>(Cdp::attach(browser).await?) })
        .await
        .expect("the mocked session starts");
    (host, mock)
}

fn arguments(v: Value) -> Map<String, Value> {
    v.as_object()
        .cloned()
        .expect("tool arguments are an object")
}

async fn call(host: &Host<Cdp>, tool: &str, v: Value) -> Result<Output, ToolError> {
    host.call(tool, arguments(v)).await
}

/// The text and the structured content of a tool that answers with both.
async fn both(host: &Host<Cdp>, tool: &str, v: Value) -> (String, Value) {
    match call(host, tool, v).await {
        Ok(Output::Structured { text, data }) => (text, data),
        other => panic!("{tool} should answer with text and data, got {other:?}"),
    }
}

/// The page-script operation an expression runs, if it runs one.
fn operation(expression: &str) -> Option<String> {
    if !expression.contains("unknown developer-tools operation") {
        return None;
    }
    let start = expression.rfind("})(\"")? + 4;
    let end = start + expression[start..].find('"')?;
    Some(expression[start..end].to_owned())
}

/// Answers the page script with `answer(op)`; every other evaluation is null.
fn script_page(mock: &MockCtrl, answer: impl Fn(&str) -> Value + Send + 'static) {
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(value(
            operation(expression).map_or(Value::Null, |op| answer(&op)),
        ))
    });
}

/// The expressions the browser was asked to evaluate, oldest first.
fn evaluated(mock: &MockCtrl) -> Vec<String> {
    mock.calls_to("Runtime.evaluate")
        .iter()
        .filter_map(|call| call.params["expression"].as_str().map(str::to_owned))
        .collect()
}

/// The page-script operations the browser was asked to run, oldest first.
fn ops(mock: &MockCtrl) -> Vec<String> {
    evaluated(mock)
        .into_iter()
        .filter(|expression| operation(expression).is_some())
        .collect()
}

fn login_page() -> Value {
    json!({
        "url": "https://example.test/login",
        "title": "Sign in",
        "ready_state": "complete",
        "mode": "accessibility",
        "more": 0,
        "truncated": false,
        "nodes": [
            { "d": 0, "role": "main", "name": "", "tag": "main", "visible": true, "enabled": true },
            { "d": 1, "role": "heading", "name": "Sign in", "tag": "h1", "visible": true,
              "enabled": true, "level": 1 },
            { "d": 1, "role": "form", "name": "Login", "tag": "form", "visible": true,
              "enabled": true, "selector": "form#login" },
            { "d": 2, "role": "textbox", "name": "Email", "tag": "input", "visible": true,
              "enabled": true, "selector": "#email", "required": true, "value": "a@b.c" },
            { "d": 2, "role": "textbox", "name": "Password", "tag": "input", "visible": true,
              "enabled": true, "selector": "#password", "required": true },
            { "d": 2, "role": "checkbox", "name": "Remember me", "tag": "input", "visible": true,
              "enabled": true, "selector": "#remember", "checked": true },
            { "d": 2, "role": "button", "name": "Sign in", "tag": "button", "visible": true,
              "enabled": false, "selector": "#go" },
        ],
    })
}

/// A script that shows the login page and says each ref's element is as it was.
fn login_script(mock: &MockCtrl) {
    script_page(mock, |op| match op {
        "snapshot" => login_page(),
        "verify_ref" => json!({
            "count": 1, "role": "textbox", "name": "Email", "url": "https://example.test/login",
        }),
        _ => Value::Null,
    });
}

const LOGIN_TEXT: &str = "\
- page \"Sign in\" url=https://example.test/login snapshot=1
  - main
    - heading \"Sign in\" [level=1]
    - form \"Login\" [ref=e1]
      - textbox \"Email\" [ref=e2] [required] value=\"a@b.c\"
      - textbox \"Password\" [ref=e3] [required]
      - checkbox \"Remember me\" [ref=e4] [checked]
      - button \"Sign in\" [ref=e5] [disabled]";

// ----------------------------------------------------------------------
// A tiny JSON Schema check: type, required, properties, items, enum, minimum
// ----------------------------------------------------------------------

fn has_type(kind: &str, value: &Value) -> bool {
    match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "number" => value.is_number(),
        "null" => value.is_null(),
        other => panic!("the check does not know the type {other}"),
    }
}

fn conforms(schema: &Value, value: &Value, path: &str) -> Result<(), String> {
    if let Some(kind) = schema.get("type") {
        let kinds: Vec<&str> = match kind {
            Value::String(kind) => vec![kind.as_str()],
            Value::Array(kinds) => kinds.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        if !kinds.iter().any(|kind| has_type(kind, value)) {
            return Err(format!("{path}: expected {kinds:?}, got {value}"));
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        if !allowed.contains(value) {
            return Err(format!("{path}: {value} is not one of {allowed:?}"));
        }
    }
    if let (Some(minimum), Some(number)) = (
        schema.get("minimum").and_then(Value::as_f64),
        value.as_f64(),
    ) {
        if number < minimum {
            return Err(format!("{path}: {number} is below {minimum}"));
        }
    }
    if let Some(object) = value.as_object() {
        for required in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !object.contains_key(required) {
                return Err(format!("{path}: missing required field {required}"));
            }
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (name, inner) in properties {
                if let Some(found) = object.get(name) {
                    conforms(inner, found, &format!("{path}.{name}"))?;
                }
            }
        }
    }
    if let (Some(items), Some(list)) = (schema.get("items"), value.as_array()) {
        for (index, item) in list.iter().enumerate() {
            conforms(items, item, &format!("{path}[{index}]"))?;
        }
    }
    Ok(())
}

fn schema_of<S: Send + 'static>(host: &Host<S>, tool: &str) -> Value {
    let described = host
        .describe_tools()
        .into_iter()
        .find(|described| described.name == tool)
        .unwrap_or_else(|| panic!("{tool} is not offered"));
    Value::Object(
        described
            .output_schema
            .map(|schema| (*schema).clone())
            .unwrap_or_else(|| panic!("{tool} declares no output schema")),
    )
}

// ----------------------------------------------------------------------
// page_snapshot
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_snapshot_is_a_compact_tree_with_refs_and_structured_content() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    login_script(&mock);

    let (text, data) = both(&host, "page_snapshot", json!({})).await;

    assert_eq!(text, LOGIN_TEXT);
    assert_eq!(data["url"], "https://example.test/login");
    assert_eq!(data["snapshot"], 1);
    assert_eq!(data["truncated"], false);
    let nodes = data["nodes"].as_array().expect("the nodes with refs");
    assert_eq!(nodes.len(), 5, "only the nodes with refs: {data}");
    assert_eq!(nodes[0]["ref"], "e1");
    assert_eq!(nodes[0]["selector"], "form#login");
    assert_eq!(nodes[3]["checked"], true);
    assert_eq!(nodes[4]["enabled"], false);
    assert!(nodes[1]["frame"].is_null());
    conforms(&schema_of(&host, "page_snapshot"), &data, "page_snapshot").unwrap();
}

#[tokio::test]
async fn the_bounds_and_the_scope_reach_the_page_script() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    login_script(&mock);

    both(
        &host,
        "page_snapshot",
        json!({ "depth": 5, "max_nodes": 150, "include_hidden": true, "scope": "#app" }),
    )
    .await;
    both(&host, "page_snapshot", json!({ "mode": "outline" })).await;

    let expressions = ops(&mock);
    assert!(
        expressions[0].contains("\"max_nodes\":150")
            && expressions[0].contains("\"depth\":5")
            && expressions[0].contains("\"include_hidden\":true")
            && expressions[0].contains("\"value\":\"#app\""),
        "{}",
        expressions[0]
    );
    assert_eq!(operation(&expressions[1]).as_deref(), Some("outline"));
}

#[tokio::test]
async fn out_of_range_bounds_and_a_scope_that_matches_nothing_are_argument_errors() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(
        &mock,
        |_| json!({ "error": "the scope selector matched nothing" }),
    );

    for (tool, arguments, name) in [
        ("page_snapshot", json!({ "depth": 31 }), "depth"),
        ("page_snapshot", json!({ "max_nodes": 0 }), "max_nodes"),
        ("page_snapshot", json!({ "mode": "tree" }), "mode"),
        ("read_page_text", json!({ "max_chars": 10 }), "max_chars"),
        ("extract_links", json!({ "limit": 2001 }), "limit"),
        ("page_snapshot", json!({ "scope": "#none" }), "scope"),
    ] {
        let error = call(&host, tool, arguments).await.unwrap_err();
        assert!(
            matches!(&error, ToolError::InvalidArgument { name: found, .. } if found == name),
            "{tool} {name}: {error}"
        );
    }
}

#[tokio::test]
async fn a_snapshot_marks_what_cuts_it_short() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, |_| {
        json!({
            "url": "https://a.test/", "title": "Big", "more": 132, "truncated": true,
            "nodes": [{ "d": 0, "role": "main", "name": "" }],
        })
    });

    let (text, data) = both(&host, "page_snapshot", json!({})).await;

    assert!(
        text.ends_with("\n  - ... 132 more nodes (raise max_nodes or pass scope)"),
        "{text}"
    );
    assert_eq!(data["truncated"], true);
    assert_eq!(data["more"], 132);
}

// ----------------------------------------------------------------------
// Refs in tool arguments
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_ref_in_a_selector_argument_reaches_the_tool_as_the_selector() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    login_script(&mock);
    both(&host, "page_snapshot", json!({})).await;
    let before = evaluated(&mock).len();

    // Whether the click succeeds does not matter here: what the tool looked for does.
    let _ = call(
        &host,
        "click_element",
        json!({ "selector": "ref=e2", "timeout": 0.05 }),
    )
    .await;

    let after = evaluated(&mock).split_off(before);
    assert_eq!(
        operation(&after[0]).as_deref(),
        Some("verify_ref"),
        "the element is checked first: {after:?}"
    );
    assert!(
        after
            .iter()
            .skip(1)
            .any(|expression| expression.contains("#email")),
        "the tool looked for the stored selector: {after:?}"
    );
    assert!(
        after
            .iter()
            .all(|expression| !expression.contains("ref=e2")),
        "the tool never saw the ref: {after:?}"
    );
}

#[tokio::test]
async fn a_ref_whose_element_changed_is_refused_with_what_it_is_now() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    login_script(&mock);
    both(&host, "page_snapshot", json!({})).await;
    script_page(&mock, |op| match op {
        "verify_ref" => json!({
            "count": 1, "role": "button", "name": "Save", "url": "https://example.test/login",
        }),
        _ => Value::Null,
    });

    let error = call(&host, "click_element", json!({ "selector": "ref=e2" }))
        .await
        .unwrap_err();

    assert!(
        matches!(&error, ToolError::InvalidArgument { name, .. } if name == "selector"),
        "{error}"
    );
    assert!(
        error.to_string().contains(
            "ref e2 is stale: the element is now button \"Save\", it was textbox \"Email\"; \
             call page_snapshot again"
        ),
        "{error}"
    );
}

#[tokio::test]
async fn a_ref_whose_element_is_gone_and_a_moved_page_are_told_apart() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    login_script(&mock);
    both(&host, "page_snapshot", json!({})).await;
    script_page(&mock, |op| match op {
        "verify_ref" => json!({ "count": 0, "url": "https://example.test/home" }),
        _ => Value::Null,
    });

    let error = call(
        &host,
        "type_text",
        json!({ "selector": "ref=e3", "text": "x" }),
    )
    .await
    .unwrap_err()
    .to_string();

    assert!(
        error
            .contains("ref e3 is stale: no element matches any more (it was textbox \"Password\")"),
        "{error}"
    );
    assert!(
        error.contains(
            "the page is now https://example.test/home, the snapshot was of \
             https://example.test/login"
        ),
        "{error}"
    );
}

#[tokio::test]
async fn an_unknown_ref_names_the_refs_there_are() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    login_script(&mock);

    let none = call(&host, "click_element", json!({ "selector": "ref=e1" }))
        .await
        .unwrap_err()
        .to_string();
    both(&host, "page_snapshot", json!({})).await;
    let unknown = call(&host, "click_element", json!({ "selector": "ref=e9" }))
        .await
        .unwrap_err()
        .to_string();

    assert!(
        none.contains("unknown ref e1; no page_snapshot has been taken yet"),
        "{none}"
    );
    assert!(
        unknown.contains("unknown ref e9; the last snapshot (1) has e1..e5"),
        "{unknown}"
    );
}

#[tokio::test]
async fn every_selector_argument_may_be_a_ref_and_a_new_snapshot_replaces_the_refs() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, |op| match op {
        "snapshot" => login_page(),
        "verify_ref" => json!({
            "count": 1, "role": "form", "name": "Login", "url": "https://example.test/login",
        }),
        _ => Value::Null,
    });
    both(&host, "page_snapshot", json!({})).await;

    // `scope` of the snapshot tool itself is a selector argument too.
    both(&host, "page_snapshot", json!({ "scope": "ref=e1" })).await;
    let scoped = ops(&mock);
    let last = scoped.last().expect("the second snapshot ran");
    assert_eq!(operation(last).as_deref(), Some("snapshot"));
    assert!(last.contains("\"value\":\"form#login\""), "{last}");

    // The second snapshot is number 2; its refs are the ones that count now.
    let error = call(&host, "click_element", json!({ "selector": "ref=e9" }))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("the last snapshot (2) has e1..e5"),
        "{error}"
    );
}

#[tokio::test]
async fn a_ref_inside_a_shadow_root_is_refused_on_the_pure_cdp_engine() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, |_| {
        json!({
            "url": "https://a.test/", "title": "Shadow", "more": 0, "truncated": false,
            "nodes": [{ "d": 0, "role": "button", "name": "Go", "shadow": true,
                        "selector": "my-app ::shadow button.go", "visible": true, "enabled": true }],
        })
    });
    let (text, _) = both(&host, "page_snapshot", json!({})).await;
    assert!(text.contains("button \"Go\" [ref=e1] [shadow]"), "{text}");

    let error = call(&host, "click_element", json!({ "selector": "ref=e1" }))
        .await
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("ref e1 cannot be used: it is inside a shadow root"),
        "{error}"
    );
}

// ----------------------------------------------------------------------
// read_page_text and the extract tools
// ----------------------------------------------------------------------

#[tokio::test]
async fn page_text_is_the_text_and_the_facts_about_it() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, |op| match op {
        "text" => json!({
            "text": "# Mugs\n\nA [mug](https://a.test/mug).\n\n... [cut: 120 more characters]",
            "chars": 40, "total_chars": 160, "truncated": true, "cut_chars": 120,
            "source": "main", "format": "markdown", "url": "https://a.test/", "title": "Mugs",
        }),
        _ => Value::Null,
    });

    let (text, data) = both(&host, "read_page_text", json!({ "max_chars": 1000 })).await;

    assert!(
        text.starts_with("# Mugs\n\nA [mug](https://a.test/mug)."),
        "{text}"
    );
    assert!(text.ends_with("... [cut: 120 more characters]"), "{text}");
    assert_eq!(data["source"], "main");
    assert_eq!(data["total_chars"], 160);
    assert!(data.get("text").is_none(), "the text is not sent twice");
    conforms(&schema_of(&host, "read_page_text"), &data, "read_page_text").unwrap();
    let expressions = ops(&mock);
    assert!(
        expressions[0].contains("\"max_chars\":1000"),
        "{}",
        expressions[0]
    );
}

#[tokio::test]
async fn links_are_listed_with_refs_and_without_their_selectors() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, |op| match op {
        "links" => json!({
            "url": "https://example.test/", "title": "Home", "total": 2, "truncated": false,
            "links": [
                { "text": "Docs", "href": "https://example.test/docs", "region": "navigation",
                  "selector": "a.docs", "ref_role": "link", "ref_name": "Docs" },
                { "text": "Home", "href": "https://example.test/", "region": "banner", "count": 2 },
            ],
        }),
        _ => Value::Null,
    });

    let (text, data) = both(&host, "extract_links", json!({ "same_origin": true })).await;

    assert_eq!(
        text,
        "2 links\n\
         - \"Docs\" https://example.test/docs [ref=e1] in navigation\n\
         - \"Home\" https://example.test/ in banner x2"
    );
    assert_eq!(data["links"][0]["ref"], "e1");
    assert!(data["links"][0].get("selector").is_none());
    assert!(data["links"][1].get("ref").is_none());
    conforms(&schema_of(&host, "extract_links"), &data, "extract_links").unwrap();
    let error = call(&host, "click_element", json!({ "selector": "ref=e1" }))
        .await
        .map(|_| ())
        .unwrap_err();
    assert!(
        !error.to_string().contains("unknown ref"),
        "the link's ref is in the table: {error}"
    );
}

#[tokio::test]
async fn tables_and_forms_are_data_with_refs_that_continue_the_numbering() {
    let dir = tempfile::tempdir().unwrap();
    let (host, mock) = running(dir.path()).await;
    script_page(&mock, |op| match op {
        "tables" => json!({
            "url": "https://example.test/", "title": "Shop", "more_tables": 0,
            "tables": [{
                "caption": "Prices", "headers": ["Item", "Cost"],
                "rows": [["Mug", "4"], ["Cup", "3"]], "row_count": 2, "truncated": false,
                "selector": "table#prices", "ref_role": "table", "ref_name": "Prices",
            }],
        }),
        "forms" => json!({
            "url": "https://example.test/", "title": "Shop", "more_forms": 0,
            "forms": [{
                "action": "https://example.test/login", "method": "post",
                "selector": "form#login", "ref_role": "form", "ref_name": "Login",
                "fields": [
                    { "name": "email", "type": "email", "label": "Email", "required": true,
                      "value": "a@b.c", "selector": "#email", "ref_role": "textbox",
                      "ref_name": "Email" },
                    { "name": "plan", "type": "select", "label": "Plan", "required": false,
                      "options": [{ "value": "a", "label": "Free", "selected": true },
                                  { "value": "b", "label": "Pro", "selected": false }] },
                ],
                "submit": { "label": "Sign in", "selector": "#go", "ref_role": "button",
                            "ref_name": "Sign in" },
            }],
        }),
        _ => Value::Null,
    });

    let (table_text, tables) = both(&host, "extract_tables", json!({})).await;
    let (form_text, forms) = both(&host, "extract_forms", json!({})).await;

    assert_eq!(
        table_text,
        "Table 1 \"Prices\" [ref=e1]: 2 rows\n| Item | Cost |\n| --- | --- |\n| Mug | 4 |\n| Cup | 3 |"
    );
    assert_eq!(tables["tables"][0]["selector"], "table#prices");
    assert_eq!(forms["forms"][0]["ref"], "e2");
    assert_eq!(forms["forms"][0]["fields"][0]["ref"], "e3");
    assert_eq!(forms["forms"][0]["submit"]["ref"], "e4");
    assert!(forms["forms"][0]["fields"][1].get("ref").is_none());
    assert_eq!(
        form_text,
        "form [ref=e2] POST \"https://example.test/login\"\n\
         \x20 - email (email) \"Email\" [ref=e3] [required] value=\"a@b.c\"\n\
         \x20 - plan (select) \"Plan\" options: Free*, Pro\n\
         \x20 submit: \"Sign in\" [ref=e4]"
    );
    conforms(
        &schema_of(&host, "extract_tables"),
        &tables,
        "extract_tables",
    )
    .unwrap();
    conforms(&schema_of(&host, "extract_forms"), &forms, "extract_forms").unwrap();
}

// ----------------------------------------------------------------------
// Declarations
// ----------------------------------------------------------------------

#[test]
fn every_declared_output_schema_is_an_object_whose_required_fields_exist() {
    let dir = tempfile::tempdir().unwrap();
    let settings = || Settings::new(dir.path());
    let mut declared = 0;
    for tools in [
        cdp::host(settings()).describe_tools(),
        driver::host(settings()).describe_tools(),
        sb::host(settings()).describe_tools(),
    ] {
        for tool in tools {
            let Some(schema) = tool.output_schema else {
                continue;
            };
            declared += 1;
            assert_eq!(schema.get("type"), Some(&json!("object")), "{}", tool.name);
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .unwrap_or_else(|| panic!("{} has no properties", tool.name));
            for required in schema
                .get("required")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                assert!(
                    properties.contains_key(required),
                    "{}: {required} is required but not described",
                    tool.name
                );
            }
        }
    }
    assert_eq!(declared, 15, "five page tools on each of three servers");
}

#[test]
fn the_page_tools_are_on_every_browser_server_and_marked_untrusted() {
    let dir = tempfile::tempdir().unwrap();
    let settings = || Settings::new(dir.path());
    let page_tools = [
        "page_snapshot",
        "read_page_text",
        "extract_links",
        "extract_tables",
        "extract_forms",
    ];

    let cdp = cdp::host(settings());
    let driver = driver::host(settings());
    let sb = sb::host(settings());
    for tool in page_tools {
        assert!(
            cdp.has_tool(tool) && driver.has_tool(tool) && sb.has_tool(tool),
            "{tool}"
        );
        for untrusted in [
            cdp.untrusted_tool_names(),
            driver.untrusted_tool_names(),
            sb.untrusted_tool_names(),
        ] {
            assert!(untrusted.contains(&tool), "{tool} is not untrusted");
        }
    }
    assert!(!seleniumbase_rs::mcp::dev::host(settings()).has_tool("page_snapshot"));
}
