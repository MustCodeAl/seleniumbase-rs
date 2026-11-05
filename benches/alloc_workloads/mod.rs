//! The work both allocation benches measure: `alloc` (divan) counts the allocations of each call,
//! `alloc_sites` (dhat) says where the bytes come from. Nothing here needs Chrome or the network:
//! the pure CDP engine runs on its mocked browser, the importer reads the golden Python files,
//! and the MCP tool calls are handled in process.

use std::path::Path;

use seleniumbase_rs::mcp::cdp::{self, Cdp};
use seleniumbase_rs::mcp::dev::{self, NoBrowser};
use seleniumbase_rs::mcp::{Host, Settings, ToolError};
use seleniumbase_rs::sb_cdp::{Browser, MockCtrl, Page};
use seleniumbase_rs::tokio::runtime::Runtime;
use seleniumbase_rs::utilities::python_importer::{import_python, ImportOptions, PythonSource};
use serde_json::{json, Map, Value};

/// A runtime on this thread, so the allocations of a call are not spread over workers.
pub fn runtime() -> Runtime {
    seleniumbase_rs::tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a Tokio runtime")
}

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

/// A page on the mocked browser. It answers the page-script calls of `evaluate` and of a
/// locator (a count of one, a centre point, visible); every other command gets `{}`.
pub async fn mocked_page() -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    mock.on("Runtime.evaluate", |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        Ok(if expression.contains("__sbcdp.center(") {
            value(json!({ "x": 120.0, "y": 80.0, "covered": false }))
        } else if expression.contains(".some(__sbcdp.visible)") {
            value(json!(true))
        } else if expression.contains("__sbcdp.search(") || expression.contains("resolve(") {
            value(json!(1))
        } else {
            value(json!(2))
        })
    });
    let page = browser.default_page().await.expect("the mocked page");
    (page, mock)
}

/// `Page::evaluate` of a small expression.
pub async fn evaluate(page: &Page) -> Value {
    page.evaluate("1 + 1").await.expect("evaluate")
}

/// `Locator::count` of a CSS selector.
pub async fn count(page: &Page) -> usize {
    page.locator("#go").count().await.expect("count")
}

/// `Locator::click`: find, scroll, centre, then the mouse events.
pub async fn click(page: &Page) {
    page.locator("#go").click().await.expect("click");
}

/// The Python examples of `tests/golden/import_python`, by name.
pub fn golden_sources() -> Vec<(String, String)> {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/import_python");
    let mut sources: Vec<(String, String)> = std::fs::read_dir(&folder)
        .expect("the golden folder")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "py"))
        .map(|path| {
            let name = path
                .file_stem()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            let source = std::fs::read_to_string(&path).expect("a Python file");
            (name, source)
        })
        .collect();
    sources.sort();
    sources
}

/// Converts every golden file; returns the length of the Rust written.
pub fn import_all(sources: &[(String, String)]) -> usize {
    sources
        .iter()
        .map(|(name, source)| {
            let options = ImportOptions {
                source: PythonSource::Auto,
                test_name: name.clone(),
            };
            import_python(source, &options).rust.len()
        })
        .sum()
}

/// The server with no browser, writing nowhere it matters.
pub fn dev_host() -> Host<NoBrowser> {
    dev::host(Settings::new(std::env::temp_dir().join("sbrs-alloc-bench")))
}

/// The Pure CDP server, its session already running on a mocked browser.
pub async fn cdp_host() -> (Host<Cdp>, MockCtrl) {
    let host = cdp::host(Settings::new(std::env::temp_dir().join("sbrs-alloc-bench")));
    let (browser, mock) = Browser::new_mocked();
    mock.on("Runtime.evaluate", |_| Ok(value(json!(2))));
    host.ctx()
        .start(async { Ok::<_, ToolError>(Cdp::attach(browser).await?) })
        .await
        .expect("the mocked session starts");
    (host, mock)
}

/// The arguments of a tool call, from a JSON object.
pub fn arguments(v: Value) -> Map<String, Value> {
    v.as_object()
        .cloned()
        .expect("tool arguments are an object")
}
