//! The MCP servers the way an AI client uses them.
//!
//! The `seleniumbase-mcp` binary is spawned, spoken to in JSON-RPC over its stdin
//! and stdout (the `initialize` handshake, `tools/list`, `tools/call`), and drives
//! a real headless Chrome against a loopback page. Nothing here calls the library
//! API: `tests/mcp_cdp_chrome.rs` and its neighbours do that. What these tests add
//! is the wire: that stdout carries only JSON-RPC, that results and errors have the
//! shape a client reads, and that the browser is gone when the server is.
//!
//! The tests that start a browser are ignored by default. Run them one at a time:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test mcp_stdio_chrome -- --ignored --test-threads=1
//! ```
//!
//! The `sb` and `driver` servers need chromedriver; the crate finds or downloads
//! one that matches Chrome. The `cdp` test also compiles the Rust test the server
//! exports, in a scratch crate beside the target directory.

#![cfg(feature = "mcp-server")]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

#[path = "support/scratch_crate.rs"]
mod scratch_crate;

/// How long one request may wait for its answer. Starting a browser is the
/// slow one; the WebDriver servers may also download a driver first.
const STEP: Duration = Duration::from_secs(300);

const PAGE: &str = r##"<!doctype html>
<html lang="en"><head><title>Stdio Fixture</title></head>
<body>
<h1>Shop</h1>
<h3>Skipped a level</h3>
<form id="order" onsubmit="document.title='submitted:'+document.getElementById('email').value;return false;">
  <label for="email">Email</label><input id="email" name="email" type="email">
  <input id="coupon" name="coupon">
  <button id="submit" type="submit" data-testid="place-order">Place order</button>
</form>
<span id="dup">a</span><span id="dup">b</span>
<img src="/logo.png">
<a href="/about">About us</a>
<script src="/missing.js"></script>
<script>setTimeout(function () { throw new Error("boom from the fixture"); }, 0);</script>
</body></html>"##;

const ABOUT: &str = r##"<!doctype html>
<html lang="en"><head><title>About</title></head><body><h1>About us</h1></body></html>"##;

/// Serves the fixture on a loopback port, a thread to each connection; any
/// other path is a 404. Returns the base URL, with a trailing slash.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let (status, body) = match path {
                    "/" => ("200 OK", PAGE),
                    "/about" => ("200 OK", ABOUT),
                    _ => ("404 Not Found", "not here"),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nSet-Cookie: sid=abc123; Path=/\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    format!("http://{address}/")
}

/// What `tools/call` answered: the text blocks joined, whether the call failed,
/// and the structured content of a failure report.
struct Reply {
    tool: String,
    is_error: bool,
    text: String,
    structured: Value,
}

impl Reply {
    fn new(tool: &str, result: &Value) -> Self {
        let content = result["content"]
            .as_array()
            .unwrap_or_else(|| panic!("{tool}: the result has no content: {result}"));
        let text = content
            .iter()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            tool: tool.to_owned(),
            is_error: result["isError"] == json!(true),
            text,
            structured: result
                .get("structuredContent")
                .cloned()
                .unwrap_or(Value::Null),
        }
    }

    /// The reply of a call that must have worked.
    fn ok(self) -> Self {
        assert!(!self.is_error, "{} failed: {}", self.tool, self.text);
        self
    }

    /// The reply of a call that must have failed.
    fn failed(self) -> Self {
        assert!(
            self.is_error,
            "{} should have failed but said: {}",
            self.tool, self.text
        );
        self
    }

    /// The text of the reply read as JSON.
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or_else(|error| {
            panic!(
                "{} did not answer with JSON ({error}): {}",
                self.tool, self.text
            )
        })
    }
}

/// An MCP client on a spawned `seleniumbase-mcp`.
struct Client {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    log: PathBuf,
}

impl Client {
    /// Starts `seleniumbase-mcp --server <server>`, with its files going to
    /// `dir/out` and its log to `dir/server.log`.
    fn spawn(server: &str, dir: &Path) -> Self {
        std::fs::create_dir_all(dir).expect("create the server's folder");
        let log = dir.join("server.log");
        let mut child = Command::new(env!("CARGO_BIN_EXE_seleniumbase-mcp"))
            .args(["--server", server])
            .env("SB_MCP_OUTPUT_DIR", dir.join("out"))
            .env("SB_SHUTDOWN_TIMEOUT_SECS", "20")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(
                std::fs::File::create(&log).expect("create the server log"),
            ))
            .kill_on_drop(true)
            .spawn()
            .expect("start seleniumbase-mcp");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
            log,
        }
    }

    fn pid(&self) -> u32 {
        self.child.id().expect("the server is running")
    }

    /// What the server wrote to its standard error so far.
    fn stderr(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    async fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin is still open");
        let mut line = message.to_string();
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .await
            .expect("write to the server");
        stdin.flush().await.expect("flush the server's stdin");
    }

    /// The next line of the server's stdout, which must be a JSON-RPC message:
    /// a stray log line would break every client.
    async fn read_message(&mut self) -> Value {
        let mut line = String::new();
        let read = timeout(STEP, self.stdout.read_line(&mut line))
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "the server did not answer within {STEP:?}\n{}",
                    self.stderr()
                )
            })
            .expect("read the server's stdout");
        assert!(read > 0, "the server closed its stdout\n{}", self.stderr());
        let message: Value = serde_json::from_str(line.trim()).unwrap_or_else(|error| {
            panic!("stdout must carry JSON-RPC only; got {line:?} ({error})")
        });
        assert_eq!(message["jsonrpc"], "2.0", "not a JSON-RPC message: {line}");
        message
    }

    /// Sends a request and waits for its answer: the `result`, or the `error`
    /// object of a protocol failure. Messages the server starts itself are
    /// skipped.
    async fn request(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await;
        loop {
            let message = self.read_message().await;
            if message.get("method").is_some() || message["id"] != json!(id) {
                continue;
            }
            return match message.get("error") {
                Some(error) => Err(error.clone()),
                None => Ok(message["result"].clone()),
            };
        }
    }

    async fn notify(&mut self, method: &str) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method }))
            .await;
    }

    /// The `initialize` handshake of a client with no capabilities.
    async fn initialize(&mut self) -> Value {
        let result = self
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "mcp_stdio_chrome", "version": "0" },
                }),
            )
            .await
            .unwrap_or_else(|error| panic!("initialize failed: {error}"));
        self.notify("notifications/initialized").await;
        result
    }

    async fn tools(&mut self) -> Vec<Value> {
        let result = self
            .request("tools/list", json!({}))
            .await
            .unwrap_or_else(|error| panic!("tools/list failed: {error}"));
        assert!(
            result.get("nextCursor").is_none_or(Value::is_null),
            "the tool list is paged: {result}"
        );
        result["tools"].as_array().expect("a tools array").clone()
    }

    /// Calls a tool and returns what it answered, failed or not.
    async fn call(&mut self, tool: &str, arguments: Value) -> Reply {
        let result = self
            .request(
                "tools/call",
                json!({ "name": tool, "arguments": arguments }),
            )
            .await
            .unwrap_or_else(|error| panic!("{tool}: a protocol error: {error}"));
        Reply::new(tool, &result)
    }

    /// The text of a tool that must work.
    async fn text(&mut self, tool: &str, arguments: Value) -> String {
        self.call(tool, arguments).await.ok().text
    }

    /// The JSON answer of a tool that must work.
    async fn json(&mut self, tool: &str, arguments: Value) -> Value {
        self.call(tool, arguments).await.ok().json()
    }

    /// Closes the server's stdin, as a client that went away does, and waits
    /// for the exit status. `None` means a signal ended the process.
    async fn hang_up(&mut self) -> Option<i32> {
        drop(self.stdin.take());
        timeout(Duration::from_secs(90), self.child.wait())
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "the server kept running after its client left\n{}",
                    self.stderr()
                )
            })
            .expect("wait for the server")
            .code()
    }
}

// ----------------------------------------------------------------------
// Processes
// ----------------------------------------------------------------------

/// Every process below `pid`: for a server, its browser and its driver.
///
/// Windows has no `pgrep`, so there no process is listed and the check that
/// the browser ends with its server is skipped (on Windows nothing yet ends a
/// browser whose server is killed; see `docs/platforms.md`).
fn descendants(pid: u32) -> Vec<u32> {
    if !cfg!(unix) {
        return Vec::new();
    }
    let mut found = Vec::new();
    let mut queue = vec![pid];
    while let Some(parent) = queue.pop() {
        let output = std::process::Command::new("pgrep")
            .args(["-P", &parent.to_string()])
            .output()
            .expect("run pgrep");
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Ok(child) = line.trim().parse::<u32>() {
                found.push(child);
                queue.push(child);
            }
        }
    }
    found
}

fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Waits for every one of `pids` to be gone and returns those that are not.
fn survivors(pids: &[u32]) -> Vec<u32> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let left: Vec<u32> = pids.iter().copied().filter(|pid| alive(*pid)).collect();
        if left.is_empty() || Instant::now() > deadline {
            return left;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

// ----------------------------------------------------------------------
// Without a browser
// ----------------------------------------------------------------------

/// The names of the tools a server lists.
fn names(tools: &[Value]) -> Vec<&str> {
    tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("a tool name"))
        .collect()
}

#[tokio::test]
async fn every_server_speaks_mcp_over_stdio_without_a_browser() {
    let dir = tempfile::tempdir().unwrap();
    for (server, name) in [
        ("cdp", "seleniumbase-cdp"),
        ("driver", "seleniumbase-driver"),
        ("sb", "seleniumbase-sb"),
    ] {
        let mut mcp = Client::spawn(server, &dir.path().join(server));

        let init = mcp.initialize().await;
        assert_eq!(init["serverInfo"]["name"], name, "{init}");
        assert!(init["protocolVersion"].is_string(), "{init}");
        assert!(init["capabilities"]["tools"].is_object(), "{init}");
        assert!(
            init["instructions"]
                .as_str()
                .is_some_and(|text| text.contains("start_browser")),
            "{server}: {init}"
        );

        // Every tool is described the way a client needs to call it.
        let tools = mcp.tools().await;
        let listed = names(&tools);
        let mut sorted = listed.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), listed.len(), "{server}: duplicate tool names");
        for tool in &tools {
            let tool_name = tool["name"].as_str().unwrap();
            assert!(
                tool["description"]
                    .as_str()
                    .is_some_and(|text| text.len() > 10),
                "{server}/{tool_name} has no description"
            );
            assert_eq!(
                tool["inputSchema"]["type"], "object",
                "{server}/{tool_name} has no object schema: {tool}"
            );
        }
        for needed in [
            "start_browser",
            "close_browser",
            "open_url",
            "click_element",
            "type_text",
        ] {
            assert!(
                listed.contains(&needed),
                "{server} lacks {needed}: {listed:?}"
            );
        }
        match server {
            "driver" => assert_eq!(listed.len(), 26, "the Driver() toolset: {listed:?}"),
            "sb" => assert!(
                listed.len() > 88,
                "the SB() toolset and ours: {}",
                listed.len()
            ),
            _ => {}
        }
        if server != "driver" {
            for needed in [
                "inspect_page",
                "suggest_selectors",
                "diagnose_selector",
                "get_console_messages",
                "get_failed_requests",
                "audit_page",
                "capture_debug_bundle",
                "export_session_as_test",
                "diagnose_environment",
            ] {
                assert!(listed.contains(&needed), "{server} lacks {needed}");
            }
        }

        // A server with no browser says so, as a failure report a model can act on.
        let nothing = mcp
            .call("click_element", json!({ "selector": "#anything" }))
            .await
            .failed();
        assert_eq!(
            nothing.structured["kind"], "no_browser",
            "{server}: {}",
            nothing.text
        );
        assert!(nothing.text.contains("start_browser"), "{}", nothing.text);

        // A missing argument names the argument.
        let missing = mcp.call("type_text", json!({})).await.failed();
        assert_eq!(
            missing.structured["kind"], "invalid_argument",
            "{}",
            missing.text
        );
        assert!(missing.text.contains("selector"), "{}", missing.text);

        // A mistyped tool is a protocol error that names the likely one.
        let typo = mcp
            .request(
                "tools/call",
                json!({ "name": "clck_element", "arguments": {} }),
            )
            .await
            .expect_err("an unknown tool is a protocol error");
        assert!(
            typo["message"]
                .as_str()
                .is_some_and(|text| text.contains("did you mean click_element")),
            "{typo}"
        );

        // `ping` and an unknown method are answered, not ignored.
        assert!(mcp.request("ping", json!({})).await.is_ok());
        let unknown = mcp
            .request("no/such/method", json!({}))
            .await
            .expect_err("an unknown method is an error");
        assert_eq!(unknown["code"], -32601, "{unknown}");

        if server != "driver" {
            let environment = mcp.json("diagnose_environment", json!({})).await;
            assert!(environment.is_object(), "{environment}");
            assert_eq!(environment["browser_session"], "none", "{environment}");
        }
        if server == "sb" {
            let presets = mcp.json("list_fingerprint_presets", json!({})).await;
            assert!(presets.to_string().contains("macos"), "{presets}");
            let built = mcp
                .json("build_fingerprint", json!({ "preset": "macos" }))
                .await;
            assert!(built.is_object(), "{built}");
        }

        assert_eq!(mcp.hang_up().await, Some(0), "{server}: {}", mcp.stderr());
        let log = mcp.stderr();
        assert!(!log.contains("panicked"), "{server} panicked:\n{log}");
    }
}

// ----------------------------------------------------------------------
// With a browser
// ----------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Engine {
    Cdp,
    WebDriver,
}

/// Evaluates `javascript` in the page, whichever server this is: the Pure CDP
/// server takes an expression, the WebDriver ones a function body.
async fn evaluate(mcp: &mut Client, engine: Engine, javascript: &str) -> Reply {
    match engine {
        Engine::Cdp => {
            mcp.call("run_javascript", json!({ "expression": javascript }))
                .await
        }
        Engine::WebDriver => {
            mcp.call(
                "execute_script",
                json!({ "script": format!("return ({javascript});") }),
            )
            .await
        }
    }
    .ok()
}

async fn title(mcp: &mut Client) -> String {
    mcp.json("get_page_info", json!({})).await["title"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// A model reads the page: its outline, selectors that work, and why one does not.
async fn read_the_page(mcp: &mut Client) {
    let outline = mcp.json("inspect_page", json!({})).await;
    assert_eq!(outline["title"], "Stdio Fixture", "{outline}");
    assert_eq!(outline["headings"][0]["text"], "Shop", "{outline}");
    let controls = outline["interactive"].as_array().expect("controls");
    let submit = controls
        .iter()
        .find(|control| control["name"] == "Place order")
        .unwrap_or_else(|| panic!("no Place order control in {outline}"));
    assert_eq!(submit["role"], "button");
    assert_eq!(submit["selector"], "[data-testid=\"place-order\"]");
    let email = controls
        .iter()
        .find(|control| control["selector"] == "#email")
        .unwrap_or_else(|| panic!("no #email control in {outline}"));
    assert_eq!(email["name"], "Email", "the label names the input");

    let suggested = mcp
        .json("suggest_selectors", json!({ "text": "Place order" }))
        .await;
    let best = &suggested["elements"][0]["selectors"][0];
    assert_eq!(
        best["selector"], "[data-testid=\"place-order\"]",
        "{suggested}"
    );
    assert_eq!(best["unique"], true);
    assert_eq!(best["finds_element"], true);
    let nothing = mcp
        .json("suggest_selectors", json!({ "text": "no such thing" }))
        .await;
    assert!(nothing["advice"].is_string(), "{nothing}");

    let typo = mcp
        .json("diagnose_selector", json!({ "selector": "#submitt" }))
        .await;
    assert_eq!(typo["near_misses"][0]["try"], "#submit", "{typo}");
    let broken = mcp
        .json("diagnose_selector", json!({ "selector": "div[" }))
        .await;
    assert!(broken["syntax_error"].is_string(), "{broken}");
    let fine = mcp
        .json("diagnose_selector", json!({ "selector": "#submit" }))
        .await;
    assert!(
        fine["verdict"]
            .as_str()
            .is_some_and(|text| text.starts_with("The selector works")),
        "{fine}"
    );

    let audit = mcp.json("audit_page", json!({})).await;
    let rules = audit["by_rule"].as_object().expect("findings by rule");
    for rule in [
        "image-alt",
        "duplicate-id",
        "form-control-label",
        "heading-order",
    ] {
        assert!(rules.contains_key(rule), "{rule} missing from {audit}");
    }
}

/// A model fills in the form and sees the page react.
async fn fill_in_the_form(mcp: &mut Client) {
    mcp.text(
        "type_text",
        json!({ "selector": "#email", "text": "buyer@example.test" }),
    )
    .await;
    mcp.text("click_element", json!({ "selector": "#submit" }))
        .await;
    assert_eq!(title(mcp).await, "submitted:buyer@example.test");
}

/// A failed call comes back as a report, and the server carries on.
async fn failures_are_reports(mcp: &mut Client, engine: Engine, base: &str) {
    // The Pure CDP engine says the element is not there; the WebDriver one, which waits for
    // it with the driver's own clock, says the wait ran out.
    let kind = match engine {
        Engine::Cdp => "element_not_found",
        Engine::WebDriver => "wait_timeout",
    };
    let gone = mcp
        .call(
            "click_element",
            json!({ "selector": "#nope", "timeout": 0.5 }),
        )
        .await
        .failed();
    assert_eq!(gone.structured["kind"], kind, "{}", gone.text);
    assert_eq!(
        gone.structured["page"]["title"], "submitted:buyer@example.test",
        "{}",
        gone.text
    );
    assert_eq!(
        gone.structured["page"]["selector"]["matches"], 0,
        "{}",
        gone.text
    );
    assert!(
        gone.text.starts_with("click_element failed:"),
        "{}",
        gone.text
    );
    assert!(gone.text.contains("next steps:"), "{}", gone.text);

    let bad = mcp
        .call(
            "click_element",
            json!({ "selector": "div[", "timeout": 0.5 }),
        )
        .await
        .failed();
    assert!(
        bad.text.contains("div["),
        "the report names the selector: {}",
        bad.text
    );
    assert!(
        bad.structured["kind"]
            .as_str()
            .is_some_and(|kind| !kind.is_empty()),
        "{}",
        bad.text
    );

    let wrong_type = mcp
        .call("click_element", json!({ "selector": 7 }))
        .await
        .failed();
    assert_eq!(
        wrong_type.structured["kind"], "invalid_argument",
        "{}",
        wrong_type.text
    );

    let refused = mcp
        .call(
            "capture_debug_bundle",
            json!({ "name": "../../escape", "calls": 0 }),
        )
        .await
        .failed();
    assert_eq!(
        refused.structured["kind"], "invalid_argument",
        "{}",
        refused.text
    );

    // Still alive, still on the page.
    assert_eq!(title(mcp).await, "submitted:buyer@example.test", "{base}");
}

/// The console and the failed requests, as an engine reports them.
async fn console_and_network(mcp: &mut Client, engine: Engine) {
    let started = mcp
        .json("get_console_messages", json!({ "start": true }))
        .await;
    assert_eq!(started["recording"], true, "{started}");
    evaluate(mcp, engine, "console.error('saved in the cart')").await;
    let messages = mcp
        .json("get_console_messages", json!({ "level": "error" }))
        .await;
    assert_eq!(
        messages["entries"][0]["text"], "saved in the cart",
        "{messages}"
    );

    let watching = mcp
        .json("get_failed_requests", json!({ "watch": true }))
        .await;
    assert!(
        watching["from_performance_timeline"]["failed"]
            .to_string()
            .contains("/missing.js"),
        "{watching}"
    );
    if engine == Engine::Cdp {
        // Only this engine records a request that failed before the call.
        let errors = mcp.json("get_console_messages", json!({})).await;
        assert!(
            errors["page_errors"]
                .to_string()
                .contains("boom from the fixture"),
            "{errors}"
        );
        mcp.text("manage_history", json!({ "action": "reload" }))
            .await;
        tokio::time::sleep(Duration::from_millis(700)).await;
        let recorded = mcp.json("get_failed_requests", json!({})).await;
        let failures = recorded["recorded"]["failures"].to_string();
        assert!(
            failures.contains("/missing.js") && failures.contains("404"),
            "{recorded}"
        );
    }
}

/// The session leaves a bundle behind and becomes a Rust test.
async fn bundle_and_export(mcp: &mut Client, out: &Path, base: &str) -> (String, PathBuf) {
    let saved = mcp
        .json("capture_debug_bundle", json!({ "name": "stdio" }))
        .await;
    let folder = PathBuf::from(saved["folder"].as_str().expect("a folder"));
    assert!(folder.starts_with(out.join("debug_bundles")), "{saved}");
    assert_eq!(saved["problems"], json!([]), "{saved}");
    let png = std::fs::read(folder.join("screenshot.png")).expect("a screenshot");
    assert!(png.starts_with(b"\x89PNG"), "a real screenshot");
    let source = std::fs::read_to_string(folder.join("page.html")).expect("the page source");
    assert!(
        source.contains("Stdio Fixture") || source.contains("submitted:"),
        "{source}"
    );
    let bundle = std::fs::read_to_string(folder.join("bundle.json")).expect("the bundle");
    let bundle: Value = serde_json::from_str(&bundle).expect("the bundle is JSON");
    // The page's own title may repeat what was typed (this fixture's form puts it there,
    // and the bundle records the title); the log of tool calls must not.
    let calls = bundle["tool_calls"].to_string();
    assert!(
        !calls.contains("buyer@example.test"),
        "typed text leaked into the logged tool calls: {calls}"
    );
    assert!(
        calls.contains("[redacted, 18 characters]"),
        "the typed text is logged by its length: {calls}"
    );
    assert_eq!(
        bundle["cookie_names"],
        json!(["sid"]),
        "the page's cookie is named, and only named: {bundle}"
    );
    assert!(
        !bundle.to_string().contains("abc123"),
        "a cookie's value leaked into the bundle"
    );
    assert!(
        bundle["tool_calls"]["recent"]
            .as_array()
            .is_some_and(|calls| !calls.is_empty()),
        "{bundle}"
    );

    // A label cannot take the folder out of the output directory.
    let hostile = mcp
        .json("capture_debug_bundle", json!({ "name": "../../escape" }))
        .await;
    let hostile_folder = PathBuf::from(hostile["folder"].as_str().expect("a folder"));
    assert!(
        hostile_folder.starts_with(out.join("debug_bundles")),
        "{hostile}"
    );
    assert!(
        !hostile_folder.to_string_lossy().contains(".."),
        "{hostile}"
    );

    let exported = mcp
        .json(
            "export_session_as_test",
            json!({ "test_name": "stdio_session" }),
        )
        .await;
    let code = exported["code"]
        .as_str()
        .expect("the test's code")
        .to_owned();
    let path = PathBuf::from(exported["path"].as_str().expect("a path"));
    assert_eq!(path, out.join("exported_tests").join("stdio_session.rs"));
    assert_eq!(std::fs::read_to_string(&path).expect("the file"), code);
    assert!(
        code.contains("#[tokio::test]\nasync fn stdio_session()"),
        "{code}"
    );
    assert!(
        code.contains(&format!("sb.open(\"{base}\").await?;")),
        "{code}"
    );
    assert!(
        code.contains("sb.type_text(\"#email\", \"buyer@example.test\").await?;"),
        "{code}"
    );
    assert!(code.contains("sb.click(\"#submit\").await?;"), "{code}");
    // What the test cannot repeat is said so, not dropped.
    assert!(
        exported["skipped"].to_string().contains("javascript")
            || exported["skipped"].to_string().contains("script"),
        "{exported}"
    );
    let missing_name = mcp
        .call(
            "export_session_as_test",
            json!({ "test_name": "../escape" }),
        )
        .await
        .failed();
    assert_eq!(missing_name.structured["kind"], "invalid_argument");
    (code, path)
}

/// Compiles `exported` in a scratch crate that depends on this one by path.
fn compile_in_a_scratch_crate(exported: &Path, scratch: &Path) {
    scratch_crate::create(scratch, "exported-session-check");
    std::fs::copy(exported, scratch.join("tests").join("exported.rs")).unwrap();
    let output = scratch_crate::cargo(scratch)
        .args(["test", "--no-run"])
        .output()
        .expect("run cargo");
    assert!(
        output.status.success(),
        "the exported test does not compile:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_cdp_server_runs_a_whole_session_over_stdio() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let base = serve();
    let mut mcp = Client::spawn("cdp", dir.path());
    mcp.initialize().await;

    let started = mcp
        .text("start_browser", json!({ "headless": true, "url": base }))
        .await;
    assert!(
        started.starts_with("Started Pure CDP Mode browser"),
        "{started}"
    );
    assert_eq!(
        mcp.text("start_browser", json!({ "headless": true })).await,
        "A browser session is already running."
    );
    mcp.text("open_url", json!({ "url": base })).await;
    let info = mcp.json("get_page_info", json!({})).await;
    assert_eq!(info["running"], true, "{info}");
    assert_eq!(info["title"], "Stdio Fixture", "{info}");
    assert!(
        info["user_agent"]
            .as_str()
            .is_some_and(|ua| ua.contains("Chrome")),
        "{info}"
    );

    read_the_page(&mut mcp).await;
    fill_in_the_form(&mut mcp).await;
    failures_are_reports(&mut mcp, Engine::Cdp, &base).await;
    console_and_network(&mut mcp, Engine::Cdp).await;
    let (_code, path) = bundle_and_export(&mut mcp, &out, &base).await;

    assert_eq!(
        mcp.text("close_browser", json!({})).await,
        "The browser session was closed."
    );
    assert_eq!(
        mcp.json("get_page_info", json!({})).await["running"],
        false,
        "the session is gone after close_browser"
    );
    assert_eq!(mcp.hang_up().await, Some(0), "{}", mcp.stderr());
    assert!(!mcp.stderr().contains("panicked"), "{}", mcp.stderr());

    // The test the model was handed builds.
    compile_in_a_scratch_crate(&path, &dir.path().join("scratch"));
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_client_that_leaves_takes_the_pure_cdp_browser_with_it() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let mut mcp = Client::spawn("cdp", dir.path());
    mcp.initialize().await;
    mcp.text("start_browser", json!({ "headless": true, "url": base }))
        .await;
    assert_eq!(title(&mut mcp).await, "Stdio Fixture");

    let family = descendants(mcp.pid());
    if cfg!(unix) {
        assert!(!family.is_empty(), "the server has no browser below it");
    }
    assert_eq!(mcp.hang_up().await, Some(0), "{}", mcp.stderr());
    assert_eq!(
        survivors(&family),
        Vec::<u32>::new(),
        "the browser outlived its server"
    );
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn the_sb_server_runs_a_whole_session_over_stdio() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let base = serve();
    let mut mcp = Client::spawn("sb", dir.path());
    mcp.initialize().await;

    let started = mcp
        .text("start_browser", json!({ "headless": true, "uc": false }))
        .await;
    assert!(
        started.starts_with("Started WebDriver session"),
        "{started}"
    );
    mcp.text("open_url", json!({ "url": base })).await;
    let info = mcp.json("get_page_info", json!({})).await;
    assert_eq!(info["title"], "Stdio Fixture", "{info}");
    assert_eq!(mcp.text("get_title", json!({})).await, "Stdio Fixture");
    assert_eq!(
        mcp.text("get_text", json!({ "selector": "h1" })).await,
        "Shop"
    );
    // The cookie list works in a session with no DevTools connection, as it does with one.
    let cookies = mcp.json("get_cookies", json!({})).await;
    assert_eq!(cookies["cookies"][0]["name"], "sid", "{cookies}");
    assert_eq!(cookies["cookies"][0]["value"], "abc123", "{cookies}");

    read_the_page(&mut mcp).await;
    fill_in_the_form(&mut mcp).await;
    failures_are_reports(&mut mcp, Engine::WebDriver, &base).await;
    console_and_network(&mut mcp, Engine::WebDriver).await;
    let (_code, _path) = bundle_and_export(&mut mcp, &out, &base).await;

    assert_eq!(
        mcp.text("close_browser", json!({})).await,
        "Browser closed."
    );
    assert_eq!(mcp.hang_up().await, Some(0), "{}", mcp.stderr());
    assert!(!mcp.stderr().contains("panicked"), "{}", mcp.stderr());
}

#[tokio::test]
#[ignore = "launches a real Chrome through chromedriver"]
async fn the_driver_server_runs_a_session_in_undetected_mode_and_leaves_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let mut mcp = Client::spawn("driver", dir.path());
    mcp.initialize().await;

    // What a client does first: the defaults, which turn undetected mode on.
    let started = mcp.text("start_browser", json!({ "headless": true })).await;
    assert!(
        started.starts_with("Started WebDriver session"),
        "{started}"
    );
    mcp.text("open_url", json!({ "url": base })).await;
    assert_eq!(mcp.text("get_title", json!({})).await, "Stdio Fixture");
    assert_eq!(mcp.text("get_current_url", json!({})).await, base);
    assert_eq!(
        mcp.text("get_text", json!({ "selector": "#order label" }))
            .await,
        "Email"
    );
    assert_eq!(
        mcp.call("find_elements_count", json!({ "selector": "span" }))
            .await
            .ok()
            .text
            .trim(),
        "2"
    );
    assert_eq!(
        mcp.call("is_element_visible", json!({ "selector": "#submit" }))
            .await
            .ok()
            .text
            .trim(),
        "true"
    );
    mcp.text(
        "type_text",
        json!({ "selector": "#email", "text": "buyer@example.test" }),
    )
    .await;
    mcp.text("click_element", json!({ "selector": "#submit" }))
        .await;
    mcp.text(
        "assert_text",
        json!({ "text": "Place order", "selector": "#order" }),
    )
    .await;
    assert_eq!(
        mcp.text("get_title", json!({})).await,
        "submitted:buyer@example.test"
    );

    mcp.text("save_screenshot", json!({ "filename": "shot.png" }))
        .await;
    let png = std::fs::read(dir.path().join("out").join("shot.png")).expect("the screenshot");
    assert!(png.starts_with(b"\x89PNG"));

    // A failure carries the page with it, and the server goes on.
    let gone = mcp
        .call(
            "click_element",
            json!({ "selector": "#nope", "timeout": 0.5 }),
        )
        .await
        .failed();
    assert_eq!(gone.structured["kind"], "wait_timeout", "{}", gone.text);
    assert_eq!(
        gone.structured["page"]["title"], "submitted:buyer@example.test",
        "{}",
        gone.text
    );
    let absent = mcp
        .call(
            "assert_element",
            json!({ "selector": "#nope", "timeout": 0.5 }),
        )
        .await
        .failed();
    assert!(absent.text.contains("#nope"), "{}", absent.text);
    assert_eq!(
        mcp.text("get_title", json!({})).await,
        "submitted:buyer@example.test"
    );

    // The client leaves without close_browser: the browser and the driver go too.
    let family = descendants(mcp.pid());
    if cfg!(unix) {
        assert!(
            family.len() >= 2,
            "expected chromedriver and Chrome below the server: {family:?}"
        );
    }
    assert_eq!(mcp.hang_up().await, Some(0), "{}", mcp.stderr());
    assert_eq!(
        survivors(&family),
        Vec::<u32>::new(),
        "the browser or its driver outlived the server"
    );
    assert!(!mcp.stderr().contains("panicked"), "{}", mcp.stderr());
}
