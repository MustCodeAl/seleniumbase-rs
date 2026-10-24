//! The real `sbrs-mcp --listen` as a child process, and a small MCP client over
//! streamable HTTP, for the tests that talk to it.
//!
//! Included by `mcp_http.rs` and `mcp_http_chrome.rs` with `#[path]`.

#![allow(dead_code, reason = "each test file uses part of this module")]

use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::time::timeout;

/// How long a test waits for the server before it calls the server stuck.
pub const STEP: Duration = Duration::from_secs(60);

const READY: &str = "sbrs-mcp: listening on ";

fn command(args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sbrs-mcp"));
    command
        .args(args)
        // A token in the environment of the test run must not reach a server
        // that a test starts without one.
        .env_remove("SB_MCP_TOKEN")
        .env("SB_SHUTDOWN_TIMEOUT_SECS", "5")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    for (name, value) in env {
        command.env(name, value);
    }
    command
}

/// A server that listens and has printed its address.
pub struct Server {
    pub child: Child,
    /// The endpoint, such as `http://127.0.0.1:41523/mcp`.
    pub url: String,
    pub port: u16,
    /// The line it printed when it started listening.
    pub ready: String,
    /// Everything it wrote to standard error before that line.
    pub before_ready: Vec<String>,
    /// Everything it has written to standard error so far.
    pub log: Arc<Mutex<String>>,
}

impl Server {
    /// Starts `sbrs-mcp` with `args` and waits for the line that says where it listens.
    pub async fn start(args: &[&str], env: &[(&str, &str)]) -> Self {
        let mut child = command(args, env)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start sbrs-mcp");
        let mut lines = BufReader::new(child.stderr.take().expect("stderr")).lines();
        let mut before_ready = Vec::new();
        let ready = loop {
            let line = timeout(STEP, lines.next_line())
                .await
                .expect("the server printed its address in time")
                .expect("standard error is readable")
                .unwrap_or_else(|| {
                    panic!(
                        "the server ended before it listened:\n{}",
                        before_ready.join("\n")
                    )
                });
            if line.starts_with(READY) {
                break line;
            }
            before_ready.push(line);
        };

        // Keep reading, or the pipe fills and the server blocks on a log line.
        let log = Arc::new(Mutex::new(before_ready.join("\n")));
        let drain = Arc::clone(&log);
        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                let mut log = drain.lock().unwrap();
                log.push('\n');
                log.push_str(&line);
            }
        });

        let url = ready[READY.len()..]
            .split(' ')
            .next()
            .expect("an address in the line")
            .to_owned();
        let authority = url
            .strip_prefix("http://")
            .and_then(|rest| rest.split('/').next())
            .unwrap_or_else(|| panic!("not an http URL: {ready}"));
        let port = authority
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse().ok())
            .unwrap_or_else(|| panic!("no port in: {ready}"));
        Self {
            child,
            url,
            port,
            ready,
            before_ready,
            log,
        }
    }

    pub fn log(&self) -> String {
        self.log.lock().unwrap().clone()
    }

    /// Sends the signal the way an orchestrator would.
    #[cfg(unix)]
    pub fn signal(&self, name: &str) {
        let pid = self.child.id().expect("the server is running");
        let status = std::process::Command::new("kill")
            .arg(format!("-{name}"))
            .arg(pid.to_string())
            .status()
            .expect("run kill");
        assert!(status.success(), "kill -{name} {pid} failed");
    }

    /// Waits for the server to exit and returns its exit code, `None` if a
    /// signal killed it.
    pub async fn exit_code(&mut self) -> Option<i32> {
        timeout(Duration::from_secs(30), self.child.wait())
            .await
            .expect("the server stopped within 30 seconds")
            .unwrap()
            .code()
    }
}

/// Runs `sbrs-mcp` with `args` to its end and returns its exit code and what it
/// wrote to standard error.
pub async fn run_to_exit(args: &[&str], env: &[(&str, &str)]) -> (Option<i32>, String) {
    let output = timeout(
        STEP,
        command(args, env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .expect("the command ended in time")
    .expect("run sbrs-mcp");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// An MCP client over streamable HTTP, as small as the tests need.
pub struct Mcp {
    http: Client,
    pub url: String,
    token: Option<String>,
    session: Option<String>,
    next_id: u64,
}

impl Mcp {
    pub fn new(url: &str) -> Self {
        // reqwest is built without a crypto provider of its own, and a client
        // built before one is installed panics, even for plain HTTP.
        seleniumbase_rs::install_tls_provider();
        Self {
            http: Client::builder()
                .timeout(STEP)
                .no_proxy()
                .build()
                .expect("an HTTP client"),
            url: url.to_owned(),
            token: None,
            session: None,
            next_id: 0,
        }
    }

    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_owned());
        self
    }

    /// A request to the endpoint with the headers MCP asks for.
    pub fn builder(&self, method: reqwest::Method) -> RequestBuilder {
        let mut request = self
            .http
            .request(method, &self.url)
            .header("accept", "application/json, text/event-stream");
        if let Some(token) = &self.token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        if let Some(session) = &self.session {
            request = request.header("mcp-session-id", session);
        }
        request
    }

    pub async fn post(&self, body: &Value) -> Response {
        self.builder(reqwest::Method::POST)
            .json(body)
            .send()
            .await
            .expect("the server answers")
    }

    /// The JSON-RPC message with `id` in an answer: one JSON document or a
    /// stream of server-sent events.
    async fn message(response: Response, id: u64) -> Value {
        let streamed = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|kind| kind.starts_with("text/event-stream"));
        let text = response.text().await.expect("the answer has a body");
        if !streamed {
            return serde_json::from_str(&text).expect("the answer is JSON");
        }
        text.lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .map(str::trim)
            .filter(|data| !data.is_empty())
            .filter_map(|data| serde_json::from_str::<Value>(data).ok())
            .find(|message| message["id"] == json!(id))
            .unwrap_or_else(|| panic!("no answer with id {id} in: {text}"))
    }

    /// Sends a request and returns the whole JSON-RPC answer.
    pub async fn rpc(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let response = self
            .post(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await;
        assert!(
            response.status().is_success(),
            "{method} was answered with {}",
            response.status()
        );
        Self::message(response, id).await
    }

    /// The handshake. Returns the `result` of `initialize`.
    pub async fn initialize(&mut self) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let response = self
            .post(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "sbrs-mcp http test", "version": "0" },
                },
            }))
            .await;
        assert_eq!(response.status(), StatusCode::OK, "initialize");
        self.session = Some(
            response
                .headers()
                .get("mcp-session-id")
                .expect("the server names the session")
                .to_str()
                .expect("a plain session id")
                .to_owned(),
        );
        let answer = Self::message(response, id).await;
        let accepted = self
            .post(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .await;
        assert_eq!(accepted.status(), StatusCode::ACCEPTED, "initialized");
        answer["result"].clone()
    }

    /// Calls a tool and returns the `result` of the call.
    pub async fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let answer = self
            .rpc(
                "tools/call",
                json!({ "name": tool, "arguments": arguments }),
            )
            .await;
        assert!(answer.get("error").is_none(), "{tool}: {answer}");
        answer["result"].clone()
    }

    /// The text of a tool result.
    pub async fn text(&mut self, tool: &str, arguments: Value) -> String {
        let result = self.call(tool, arguments).await;
        assert_ne!(result["isError"], json!(true), "{tool}: {result}");
        result["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{tool} sent no text: {result}"))
            .to_owned()
    }
}
