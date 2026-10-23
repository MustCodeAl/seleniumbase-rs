//! The page tools in a real Chrome, on both engines.
//!
//! `tests/mcp_pagetools.rs` checks what the server makes of the page script's
//! answers; these tests run the script itself against the fixture
//! `tests/fixtures/mcp/page_snapshot.html`: navigation links, a form with a
//! password and a hidden field, a table with a colspan and a rowspan, an
//! iframe and a shadow host. The same assertions run on the Pure CDP engine
//! and, through chromedriver, on the WebDriver engine (the `sb` server). They
//! need Chrome, so they are ignored by default; run one binary at a time:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test mcp_pagetools_chrome -- --ignored
//! ```

#![cfg(feature = "mcp-server")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use seleniumbase_rs::mcp::cdp;
use seleniumbase_rs::mcp::{sb, Host, Output, Settings, ToolError};
use serde_json::{json, Map, Value};

const PAGE: &str = include_str!("fixtures/mcp/page_snapshot.html");

const FRAME: &str = r##"<!doctype html>
<html><body><input id="card" placeholder="Card"></body></html>"##;

/// Serves the fixture and its frame on a loopback port; anything else is 404.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]);
            let path = request.split_whitespace().nth(1).unwrap_or("/");
            let (status, body) = match path {
                "/" => ("200 OK", PAGE),
                "/frame" => ("200 OK", FRAME),
                _ => ("404 Not Found", "not here"),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{address}")
}

fn arguments(v: Value) -> Map<String, Value> {
    v.as_object()
        .cloned()
        .expect("tool arguments are an object")
}

async fn call<S: Send + 'static>(
    host: &Host<S>,
    tool: &str,
    v: Value,
) -> Result<Output, ToolError> {
    host.call(tool, arguments(v)).await
}

/// The text and the structured content of a page tool.
async fn both<S: Send + 'static>(host: &Host<S>, tool: &str, v: Value) -> (String, Value) {
    match call(host, tool, v).await {
        Ok(Output::Structured { text, data }) => (text, data),
        other => panic!("{tool} should answer with text and data, got {other:?}"),
    }
}

/// The ref of the node called `name` with role `role` in a snapshot's data.
fn ref_of(data: &Value, role: &str, name: &str) -> String {
    data["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["role"] == role && node["name"] == name)
        .unwrap_or_else(|| panic!("no {role} {name:?} with a ref in {data}"))["ref"]
        .as_str()
        .expect("a ref")
        .to_owned()
}

/// What every engine must show on the fixture.
async fn the_page_is_read_the_same_on_every_engine<S: Send + 'static>(host: &Host<S>, base: &str) {
    // The snapshot: roles, names, state, values; no secrets.
    let (snapshot, data) = both(host, "page_snapshot", json!({})).await;
    for expected in [
        "- page \"Snapshot Fixture\"",
        "- navigation \"Main menu\"",
        "- heading \"Sign in\" [level=1]",
        "- form \"Login\" [ref=e",
        "- textbox \"Email\" [ref=e",
        "[required] value=\"a@b.c\"",
        "- checkbox \"Remember me\" [ref=e",
        "[checked]",
        "- combobox \"Plan\" [ref=e",
        "value=\"Free\"",
        "- button \"Sign in\" [ref=e",
        "[disabled]",
        "- iframe \"Payment\" [ref=e",
        "- table \"Prices\"\n",
        "- contentinfo",
        "- button \"Inside the shadow\" [ref=e",
        "[shadow]",
    ] {
        assert!(
            snapshot.contains(expected),
            "{expected:?} is missing from:\n{snapshot}"
        );
    }
    assert!(
        !snapshot.contains("s3cret-pass") && !snapshot.contains("tok123"),
        "a password or hidden value leaked:\n{snapshot}"
    );
    assert_eq!(data["snapshot"], 1);
    assert_eq!(data["truncated"], false, "{data}");
    ref_of(&data, "textbox", "Email");

    // A bounded snapshot says what it left out; a scoped one is the form only.
    let (short, short_data) = both(host, "page_snapshot", json!({ "max_nodes": 3 })).await;
    assert!(
        short.contains(" more nodes (raise max_nodes or pass scope)"),
        "{short}"
    );
    assert_eq!(short_data["truncated"], true);
    let (scoped, _) = both(host, "page_snapshot", json!({ "scope": "#login" })).await;
    assert!(
        scoped.contains("textbox \"Email\"") && !scoped.contains("navigation"),
        "{scoped}"
    );

    // The tag tree.
    let (outline, _) = both(host, "page_snapshot", json!({ "mode": "outline" })).await;
    assert!(outline.contains("- form#login [ref=e"), "{outline}");
    assert!(outline.contains(" children="), "{outline}");

    // Readable text: the main content, as Markdown, without the secrets.
    let (text, text_data) = both(host, "read_page_text", json!({})).await;
    for expected in [
        "# Sign in",
        &format!("[readable]({base}/docs)"),
        "`mugs`",
        "| Item | Cost | Cost |",
        "| Mug | Small | 4 |",
    ] {
        assert!(
            text.contains(expected),
            "{expected:?} is missing from:\n{text}"
        );
    }
    assert!(
        !text.contains("Footer text") && !text.contains("s3cret-pass"),
        "only the main content, and no secrets:\n{text}"
    );
    assert_eq!(text_data["source"], "main");
    let (cut, cut_data) = both(host, "read_page_text", json!({ "max_chars": 1000 })).await;
    assert!(cut.len() <= 1100, "{}", cut.len());
    assert_eq!(cut_data["truncated"], false, "the fixture is short");

    // Links: absolute, deduplicated, flagged, filtered.
    let (_, links) = both(host, "extract_links", json!({})).await;
    let list = links["links"].as_array().expect("links");
    let docs = list
        .iter()
        .find(|link| link["text"] == "Docs")
        .expect("the Docs link");
    assert_eq!(docs["href"], format!("{base}/docs"));
    assert_eq!(docs["count"], 2, "the two Docs links are one entry");
    assert_eq!(docs["region"], "navigation");
    assert!(list.iter().any(|link| link["kind"] == "script"), "{links}");
    let (_, same) = both(host, "extract_links", json!({ "same_origin": true })).await;
    assert!(
        same["links"]
            .as_array()
            .unwrap()
            .iter()
            .all(
                |link| link["href"].as_str().unwrap().starts_with(base) || link["kind"] == "script"
            ),
        "{same}"
    );
    let (_, blog) = both(host, "extract_links", json!({ "contains": "BLOG" })).await;
    assert_eq!(blog["links"].as_array().unwrap().len(), 1, "{blog}");

    // Tables with the spans expanded.
    let (table_text, tables) = both(host, "extract_tables", json!({})).await;
    let table = &tables["tables"][0];
    assert_eq!(table["caption"], "Prices");
    assert_eq!(table["headers"], json!(["Item", "Cost", "Cost"]));
    assert_eq!(
        table["rows"],
        json!([["Mug", "Small", "4"], ["Mug", "Large", "6"]])
    );
    assert_eq!(table["row_count"], 2);
    assert!(table_text.contains("| Mug | Large | 6 |"), "{table_text}");

    // Forms: fields, no password value, the hidden value, the submit button.
    let (_, forms) = both(host, "extract_forms", json!({})).await;
    let form = &forms["forms"][0];
    assert_eq!(form["action"], format!("{base}/login"));
    assert_eq!(form["method"], "post");
    let fields = form["fields"].as_array().unwrap();
    let field = |name: &str| {
        fields
            .iter()
            .find(|field| field["name"] == name)
            .unwrap_or_else(|| panic!("no field {name} in {form}"))
    };
    assert_eq!(field("email")["value"], "a@b.c");
    assert_eq!(field("email")["required"], true);
    assert!(field("password").get("value").is_none(), "{form}");
    assert_eq!(field("remember")["checked"], true);
    assert_eq!(field("plan")["options"][0]["selected"], true);
    assert_eq!(field("csrf")["value"], "tok123");
    assert_eq!(form["submit"]["label"], "Sign in");
    assert!(!forms.to_string().contains("s3cret-pass"));
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_page_tools_read_a_real_page_on_the_pure_cdp_engine() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let host = cdp::host(Settings::new(dir.path()));
    call(
        &host,
        "start_browser",
        json!({ "headless": true, "url": base }),
    )
    .await
    .expect("Chrome starts");

    the_page_is_read_the_same_on_every_engine(&host, &base).await;

    // A ref is a selector argument: typing into it changes the field.
    let (_, data) = both(&host, "page_snapshot", json!({})).await;
    let email = ref_of(&data, "textbox", "Email");
    call(
        &host,
        "type_text",
        json!({ "selector": format!("ref={email}"), "text": "x@y.z" }),
    )
    .await
    .expect("typing into a ref works");
    let (after, _) = both(&host, "page_snapshot", json!({})).await;
    assert!(
        after.contains("x@y.z"),
        "the typed text is in the field:\n{after}"
    );

    // A ref to an element inside a shadow root cannot be selected on this engine.
    let shadow = ref_of(&data, "button", "Inside the shadow");
    let error = call(
        &host,
        "click_element",
        json!({ "selector": format!("ref={shadow}"), "timeout": 0.1 }),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(error.contains("inside a shadow root"), "{error}");

    // A ref from before the page changed is refused, and a made-up one too.
    let stale = call(
        &host,
        "click_element",
        json!({ "selector": "ref=e999", "timeout": 0.1 }),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(stale.contains("unknown ref e999"), "{stale}");
    call(&host, "close_browser", json!({})).await.ok();
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn the_page_tools_read_a_real_page_on_the_webdriver_engine() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let host = sb::host(Settings::new(dir.path()));
    call(
        &host,
        "start_browser",
        json!({ "headless": true, "uc": false }),
    )
    .await
    .expect("Chrome starts");
    call(&host, "open_url", json!({ "url": base }))
        .await
        .expect("the fixture opens");

    the_page_is_read_the_same_on_every_engine(&host, &base).await;

    // A ref is a selector argument here too.
    let (_, data) = both(&host, "page_snapshot", json!({})).await;
    let email = ref_of(&data, "textbox", "Email");
    call(
        &host,
        "type_text",
        json!({ "selector": format!("ref={email}"), "text": "x@y.z" }),
    )
    .await
    .expect("typing into a ref works");
    let (after, _) = both(&host, "page_snapshot", json!({})).await;
    assert!(
        after.contains("x@y.z"),
        "the typed text is in the field:\n{after}"
    );
    call(&host, "close_browser", json!({})).await.ok();
}
