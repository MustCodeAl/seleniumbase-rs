//! The tool lists of the MCP servers, pinned to checked-in snapshots.
//!
//! What a client reads from each server's `tools/list` is written to
//! `tests/golden/mcp_tools/<server>.json`: the tools in the order they are
//! listed, each with its name, title, description, input schema, output schema
//! and annotations, and whether it is marked as returning untrusted page text.
//! The servers build these lists from many tool definitions, so a change to how
//! they are put together (moving a group of tools to another module, say) must
//! leave the snapshots as they are.
//!
//! A change that is meant to alter a tool rewrites them:
//! `UPDATE_GOLDEN=1 cargo test --features mcp-server --test mcp_tool_lists`.
//!
//! The lists are those of a server built with the default [`Settings`]: the
//! tools an operator turns on through the plugin selection (such as
//! `wait_for_email`) are not in them. The `devtools` tools are part of the
//! `cdp`, `driver` and `sb` lists, so those pin them.

#![cfg(feature = "mcp-server")]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use seleniumbase_rs::mcp::{cdp, dev, driver, sb, Host, Settings};
use serde_json::{json, Value};

fn snapshot_path(server: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/mcp_tools")
        .join(format!("{server}.json"))
}

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

fn names(listing: &Value) -> Vec<String> {
    listing["tools"]
        .as_array()
        .expect("a listing has a tool array")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap_or("?").to_owned())
        .collect()
}

/// Compares the server's list with its snapshot, or rewrites the snapshot when
/// `UPDATE_GOLDEN` is set.
fn check<S: Send + 'static>(server: &str, host: &Host<S>) {
    let fresh = listing(host);
    let text = render(&fresh);
    let path = snapshot_path(server);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().expect("the snapshot has a folder"))
            .expect("the snapshot folder is writable");
        fs::write(&path, &text).expect("the snapshot is writable");
        return;
    }
    let committed = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{} cannot be read ({error}); write it with UPDATE_GOLDEN=1 cargo test --features mcp-server --test mcp_tool_lists",
            path.display()
        )
    });
    if committed == text {
        return;
    }

    let pinned: Value = serde_json::from_str(&committed).expect("the snapshot is JSON");
    assert_eq!(
        names(&pinned),
        names(&fresh),
        "the {server} server lists other tools, or in another order, than {}",
        path.display()
    );
    let (old, new) = (pinned["tools"].as_array(), fresh["tools"].as_array());
    for (old, new) in old.into_iter().flatten().zip(new.into_iter().flatten()) {
        assert_eq!(
            old,
            new,
            "the {server} tool {} differs from {}",
            old["name"],
            path.display()
        );
    }
    assert_eq!(
        pinned,
        fresh,
        "the {server} server differs from {}",
        path.display()
    );
    panic!(
        "{} holds the same tools as the {server} server but is written differently; \
         rewrite it with UPDATE_GOLDEN=1",
        path.display()
    );
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
