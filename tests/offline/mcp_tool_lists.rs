//! The tool lists of the MCP servers, pinned with `insta` snapshots.
//!
//! What a client reads from each server's `tools/list` is a snapshot: the tools
//! in the order they are listed, each with its name, title, description, input
//! schema, output schema and annotations, and whether it is marked as returning
//! untrusted page text. A change to how the lists are put together must leave
//! the snapshots as they are.
//!
//! A change that is meant to alter a tool is reviewed with `cargo insta review`
//! (or accepted with `cargo insta accept`) after
//! `cargo test --features mcp-server --test offline mcp_tool_lists`.
//!
//! The lists are those of a server built with the default [`Settings`]: the
//! tools an operator turns on through the plugin selection (such as
//! `wait_for_email`) are not in them. The `devtools` tools are part of the
//! `cdp`, `driver` and `sb` lists, so those pin them.

#![cfg(feature = "mcp-server")]

use std::collections::BTreeMap;

use seleniumbase_rs::mcp::{cdp, dev, driver, sb, Host, Settings};
use serde_json::{json, Value};

/// `value` with the keys of every object in sorted order.
///
/// Whether `serde_json` keeps the order keys were inserted in depends on which
/// crates are built together, so the snapshots do not depend on it. Arrays,
/// the tool list among them, keep their order.
fn canonical(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(canonical).collect()),
        other => other,
    }
}

/// The server's tool list as a client sees it, with the untrusted marks.
fn listing<S: Send + 'static>(host: &Host<S>) -> Value {
    let untrusted = host.untrusted_tool_names();
    let tools: Vec<Value> = host
        .describe_tools()
        .iter()
        .map(|tool| {
            let name: &str = &tool.name;
            let mut entry = serde_json::to_value(tool).expect("a tool description serializes");
            entry["untrusted"] = json!(untrusted.contains(&name));
            entry
        })
        .collect();
    canonical(json!({
        "server": host.name(),
        "tool_count": tools.len(),
        "tools": tools,
    }))
}

fn render(listing: &Value) -> String {
    let mut text = serde_json::to_string_pretty(listing).expect("a listing renders");
    text.push('\n');
    text
}

/// Compares the server's list with its insta snapshot.
///
/// A mismatch is reviewed with `cargo insta review`, not by editing the
/// snapshot by hand.
fn check<S: Send + 'static>(server: &str, host: &Host<S>) {
    let text = render(&listing(host));
    insta::assert_snapshot!(server, text);
}

#[test]
fn the_cdp_server_lists_the_pinned_tools() {
    check("cdp", &cdp::host(Settings::new("out")));
}

#[test]
fn the_driver_server_lists_the_pinned_tools() {
    check("driver", &driver::host(Settings::new("out")));
}

#[test]
fn the_sb_server_lists_the_pinned_tools() {
    check("sb", &sb::host(Settings::new("out")));
}

#[test]
fn the_dev_server_lists_the_pinned_tools() {
    check("dev", &dev::host(Settings::new("out")));
}
