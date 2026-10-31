//! The MCP servers over streamable HTTP, with a real listener on loopback and
//! no Chrome: what a client sees of the handshake, the token, the `Host` and
//! `Origin` checks, the session cap, and the browser each session owns.
//!
//! The `dev` server starts no browser, so most tests use it. The two tests about
//! browsers serve a host whose "browser" is a stand-in that notes when it was
//! closed. Run with `cargo test --features mcp-server --test mcp_http`.

#![cfg(feature = "mcp-server")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::{Client, RequestBuilder, Response, StatusCode};
use seleniumbase_rs::mcp::{
    Args, Closeable, Ctx, Effect, Host, HttpOptions, HttpServer, Output, Profile, Schema, Settings,
    Started, ToolDef,
};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// How long a test waits for the server before it calls the server stuck.
const PATIENCE: Duration = Duration::from_secs(15);

const TOKEN: &str = "t0ken-for-the-http-tests";

fn loopback_any_port() -> HttpOptions {
    HttpOptions::default()
        .with_listen("127.0.0.1:0".parse().expect("an address"))
        // These tests are about the server; the token policy has its own tests.
        .with_no_token()
}

// ----------------------------------------------------------------------
// A running server
// ----------------------------------------------------------------------

/// A server that is serving, and the way to stop it.
struct Running {
    url: String,
    port: u16,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), SeleniumBaseError>>>,
}

impl Running {
    async fn dev(options: HttpOptions) -> Self {
        let server = HttpServer::bind(Profile::Dev, Settings::new("out"), options)
            .await
            .expect("the dev server binds");
        Self::serve(server)
    }

    fn serve(server: HttpServer) -> Self {
        let (stop, stopped) = oneshot::channel::<()>();
        let url = server.url();
        let port = server.local_addr().port();
        let task = tokio::spawn(server.with_grace(Duration::from_secs(5)).serve(async move {
            let _ = stopped.await;
        }));
        Self {
            url,
            port,
            stop: Some(stop),
            task: Some(task),
        }
    }

    /// Asks the server to stop and returns how `serve` ended.
    async fn stop(&mut self) -> Result<(), SeleniumBaseError> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let task = self.task.take().expect("stopped once");
        tokio::time::timeout(PATIENCE, task)
            .await
            .expect("the server stopped in time")
            .expect("the server task did not panic")
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

// ----------------------------------------------------------------------
// A client
// ----------------------------------------------------------------------

/// An MCP client over streamable HTTP, as small as the tests need.
struct Mcp {
    http: Client,
    url: String,
    token: Option<String>,
    session: Option<String>,
    next_id: u64,
}

impl Mcp {
    fn new(url: &str) -> Self {
        // reqwest is built without a crypto provider of its own, and a client
        // built before one is installed panics, even for plain HTTP.
        seleniumbase_rs::install_tls_provider();
        Self {
            http: Client::builder()
                .timeout(PATIENCE)
                .no_proxy()
                .build()
                .expect("an HTTP client"),
            url: url.to_owned(),
            token: None,
            session: None,
            next_id: 0,
        }
    }

    fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_owned());
        self
    }

    /// A request to the endpoint with the headers MCP asks for.
    fn builder(&self, method: reqwest::Method) -> RequestBuilder {
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

    async fn post(&self, body: &Value) -> Response {
        self.builder(reqwest::Method::POST)
            .json(body)
            .send()
            .await
            .expect("the server answers")
    }

    /// The JSON-RPC message with `id` in an answer, which is either one JSON
    /// document or a stream of server-sent events.
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
    async fn rpc(&mut self, method: &str, params: Value) -> Value {
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

    /// The handshake. Returns the `result` of `initialize`; the session id the
    /// server chose is kept for the calls that follow.
    async fn initialize(&mut self) -> Value {
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
                    "clientInfo": { "name": "mcp_http test", "version": "0" },
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

    async fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let answer = self
            .rpc(
                "tools/call",
                json!({ "name": tool, "arguments": arguments }),
            )
            .await;
        assert!(answer.get("error").is_none(), "{tool}: {answer}");
        answer["result"].clone()
    }

    /// Ends the session, as a client does when it is done.
    async fn delete(&self) -> StatusCode {
        self.builder(reqwest::Method::DELETE)
            .send()
            .await
            .expect("the server answers")
            .status()
    }
}

/// Polls until `condition` holds.
async fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
    let started = tokio::time::Instant::now();
    while !condition() {
        assert!(started.elapsed() < PATIENCE, "never happened: {what}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

// ----------------------------------------------------------------------
// The handshake and the tools
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_dev_server_answers_the_handshake_and_lists_its_tools_over_http() {
    let mut server = Running::dev(loopback_any_port()).await;
    assert!(
        server.url.starts_with("http://127.0.0.1:") && server.url.ends_with("/mcp"),
        "{}",
        server.url
    );
    assert_ne!(
        server.port, 0,
        "port 0 is replaced by the port the system chose"
    );

    let mut client = Mcp::new(&server.url);
    let initialized = client.initialize().await;
    assert_eq!(initialized["serverInfo"]["name"], "seleniumbase-dev");
    assert!(initialized["capabilities"].get("tools").is_some());

    let listed = client.rpc("tools/list", json!({})).await;
    let names: Vec<_> = listed["result"]["tools"]
        .as_array()
        .expect("a list of tools")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert!(names.contains(&"find_api"), "{names:?}");

    let found = client.call("find_api", json!({ "query": "click" })).await;
    assert_ne!(found["isError"], json!(true), "{found}");

    assert!(server.stop().await.is_ok());
}

#[tokio::test]
async fn the_endpoint_is_only_at_its_path() {
    let server = Running::dev(loopback_any_port().with_path("/sbrs/mcp")).await;
    assert!(server.url.ends_with("/sbrs/mcp"), "{}", server.url);
    let base = format!("http://127.0.0.1:{}", server.port);

    let mut client = Mcp::new(&server.url);
    client.initialize().await;

    let other = Mcp::new(&format!("{base}/mcp"));
    let response = other
        .post(&json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }))
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ----------------------------------------------------------------------
// Who may talk to it
// ----------------------------------------------------------------------

#[tokio::test]
async fn a_missing_or_wrong_token_is_401_and_the_right_one_works() {
    let server = Running::dev(loopback_any_port().with_token(TOKEN)).await;
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });

    for client in [
        Mcp::new(&server.url),
        Mcp::new(&server.url).with_token("nope"),
    ] {
        let response = client.post(&ping).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let challenge = response
            .headers()
            .get("www-authenticate")
            .expect("a 401 says how to authenticate")
            .to_str()
            .unwrap()
            .to_owned();
        assert!(challenge.starts_with("Bearer"), "{challenge}");
        let body = response.text().await.unwrap();
        assert!(!body.contains(TOKEN), "{body}");
    }

    let mut client = Mcp::new(&server.url).with_token(TOKEN);
    client.initialize().await;
    let listed = client.rpc("tools/list", json!({})).await;
    assert!(listed["result"]["tools"].is_array(), "{listed}");
}

#[tokio::test]
async fn a_foreign_origin_is_403_even_with_the_token() {
    let server = Running::dev(loopback_any_port().with_token(TOKEN)).await;
    let client = Mcp::new(&server.url).with_token(TOKEN);
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });

    for origin in ["https://evil.test", "null", "http://localhost.evil.test"] {
        let response = client
            .builder(reqwest::Method::POST)
            .header("origin", origin)
            .json(&ping)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{origin}");
    }
    // A page served from the machine itself is not foreign.
    let response = client
        .builder(reqwest::Method::POST)
        .header("origin", "http://localhost:3000")
        .json(&ping)
        .send()
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn an_origin_the_operator_allowed_is_let_in() {
    let server =
        Running::dev(loopback_any_port().with_allow_origin("https://app.example.com")).await;
    let client = Mcp::new(&server.url);
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });

    let allowed = client
        .builder(reqwest::Method::POST)
        .header("origin", "https://app.example.com")
        .json(&ping)
        .send()
        .await
        .unwrap();
    assert_ne!(allowed.status(), StatusCode::FORBIDDEN);
    let other = client
        .builder(reqwest::Method::POST)
        .header("origin", "https://other.example.com")
        .json(&ping)
        .send()
        .await
        .unwrap();
    assert_eq!(other.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_host_header_that_is_not_loopback_is_403() {
    // DNS rebinding: the attacker's name resolves to 127.0.0.1, so the request
    // arrives on the right socket with the wrong `Host`.
    let server = Running::dev(loopback_any_port()).await;
    let client = Mcp::new(&server.url);
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });

    for host in [
        format!("evil.test:{}", server.port),
        "localhost.evil.test".to_owned(),
    ] {
        let response = client
            .builder(reqwest::Method::POST)
            .header("host", &host)
            .json(&ping)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{host}");
    }
    let response = client
        .builder(reqwest::Method::POST)
        .header("host", format!("localhost:{}", server.port))
        .json(&ping)
        .send()
        .await
        .unwrap();
    assert_ne!(
        response.status(),
        StatusCode::FORBIDDEN,
        "localhost is loopback"
    );
}

#[tokio::test]
async fn options_that_fail_validation_bind_nothing() {
    let error = HttpServer::bind(
        Profile::Dev,
        Settings::new("out"),
        HttpOptions::default().with_listen("0.0.0.0:0".parse().unwrap()),
    )
    .await
    .expect_err("a reachable address without a token is refused");
    assert_eq!(error.exit_code(), 2, "{error}");
    assert!(error.to_string().contains("without a token"), "{error}");
}

#[tokio::test]
async fn an_address_that_is_taken_is_an_mcp_error() {
    let first = Running::dev(loopback_any_port()).await;
    let taken = format!("127.0.0.1:{}", first.port).parse().unwrap();
    let error = HttpServer::bind(
        Profile::Dev,
        Settings::new("out"),
        HttpOptions::default().with_listen(taken),
    )
    .await
    .expect_err("the port is in use");
    assert!(matches!(error, SeleniumBaseError::Mcp { .. }), "{error:?}");
    assert!(error.to_string().contains("http"), "{error}");
}

// ----------------------------------------------------------------------
// Sessions
// ----------------------------------------------------------------------

#[tokio::test]
async fn an_initialize_over_the_session_cap_is_503_until_a_session_ends() {
    let server = Running::dev(loopback_any_port().with_max_sessions(2)).await;
    let mut first = Mcp::new(&server.url);
    let mut second = Mcp::new(&server.url);
    first.initialize().await;
    second.initialize().await;

    let third = Mcp::new(&server.url);
    let response = third
        .post(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "third", "version": "0" },
            },
        }))
        .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = response.json().await.expect("a JSON body");
    assert_eq!(body, json!({ "error": "too many sessions (2)" }));

    // A session that is already open is not affected by the cap.
    let listed = first.rpc("tools/list", json!({})).await;
    assert!(listed["result"]["tools"].is_array());

    assert!(first.delete().await.is_success());
    let mut waiting = Mcp::new(&server.url);
    let started = tokio::time::Instant::now();
    loop {
        let response = waiting
            .post(&json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "waiting", "version": "0" },
                },
            }))
            .await;
        if response.status() == StatusCode::OK {
            break;
        }
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(started.elapsed() < PATIENCE, "the slot was never freed");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    waiting.session = None;
}

/// A browser stand-in that notes its number when it is closed.
#[derive(Debug)]
struct Browser {
    number: usize,
    closed: Arc<Mutex<Vec<usize>>>,
}

impl Closeable for Browser {
    async fn close(self) {
        self.closed.lock().unwrap().push(self.number);
    }
}

/// A host whose `start_browser` starts a stand-in numbered 1, 2, ... across all
/// the hosts this function made, and whose `which_browser` says which the
/// session holds.
fn stand_in_host(started: &Arc<AtomicUsize>, closed: &Arc<Mutex<Vec<usize>>>) -> Host<Browser> {
    let (started, closed) = (Arc::clone(started), Arc::clone(closed));
    let start = ToolDef::new(
        "start_browser",
        "Start",
        "Starts the stand-in browser.",
        Effect::Act,
        Schema::new(),
        move |ctx: Ctx<Browser>, _args: Args| {
            let (started, closed) = (Arc::clone(&started), Arc::clone(&closed));
            async move {
                let number = started.fetch_add(1, Ordering::SeqCst) + 1;
                let outcome = ctx
                    .start(async move { Ok(Browser { number, closed }) })
                    .await?;
                let browser = ctx.session().await?.number;
                Ok(Output::Json(json!({
                    "created": outcome == Started::Created,
                    "browser": browser,
                })))
            }
        },
    );
    let which = ToolDef::new(
        "which_browser",
        "Which",
        "Says which stand-in browser this session holds.",
        Effect::Inspect,
        Schema::new(),
        |ctx: Ctx<Browser>, _args: Args| async move {
            Ok(Output::Json(
                json!({ "browser": ctx.session().await?.number }),
            ))
        },
    );
    Host::new(
        "stand-in",
        "A server whose browser is a stand-in.",
        vec![start, which],
        Settings::new("out"),
    )
}

async fn stand_in_server() -> (Running, Arc<Mutex<Vec<usize>>>) {
    stand_in_server_with(loopback_any_port()).await
}

async fn stand_in_server_with(options: HttpOptions) -> (Running, Arc<Mutex<Vec<usize>>>) {
    let started = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(Mutex::new(Vec::new()));
    let (for_hosts, for_closed) = (Arc::clone(&started), Arc::clone(&closed));
    let server = HttpServer::bind_with(options, move || stand_in_host(&for_hosts, &for_closed))
        .await
        .expect("the stand-in server binds");
    assert_eq!(server.name(), "stand-in");
    (Running::serve(server), closed)
}

fn browser_of(result: &Value) -> u64 {
    result["structuredContent"]["browser"]
        .as_u64()
        .unwrap_or_else(|| panic!("no browser in {result}"))
}

#[tokio::test]
async fn every_session_has_its_own_browser_and_closes_it_when_it_ends() {
    let (mut server, closed) = stand_in_server().await;
    let mut a = Mcp::new(&server.url);
    let mut b = Mcp::new(&server.url);
    a.initialize().await;
    b.initialize().await;

    let from_a = a.call("start_browser", json!({})).await;
    let from_b = b.call("start_browser", json!({})).await;
    assert_eq!(
        from_a["structuredContent"]["created"],
        json!(true),
        "{from_a}"
    );
    assert_eq!(
        from_b["structuredContent"]["created"],
        json!(true),
        "the second client does not find the first one's browser: {from_b}"
    );
    let (browser_a, browser_b) = (browser_of(&from_a), browser_of(&from_b));
    assert_ne!(browser_a, browser_b);

    assert!(a.delete().await.is_success());
    eventually("the ended session's browser is closed", || {
        closed
            .lock()
            .unwrap()
            .contains(&usize::try_from(browser_a).unwrap())
    })
    .await;
    assert!(
        !closed
            .lock()
            .unwrap()
            .contains(&usize::try_from(browser_b).unwrap()),
        "the other session's browser stays open"
    );

    let still = b.call("which_browser", json!({})).await;
    assert_eq!(browser_of(&still), browser_b);

    assert!(server.stop().await.is_ok());
    let mut closed = closed.lock().unwrap().clone();
    closed.sort_unstable();
    assert_eq!(closed, vec![1, 2], "each browser was closed once");
}

#[tokio::test]
async fn shutdown_closes_the_browser_of_every_live_session() {
    let (mut server, closed) = stand_in_server().await;
    let mut clients = Vec::new();
    for _ in 0..3 {
        let mut client = Mcp::new(&server.url);
        client.initialize().await;
        client.call("start_browser", json!({})).await;
        clients.push(client);
    }
    assert!(closed.lock().unwrap().is_empty());

    // Connections are still open: the clients have not hung up.
    assert!(
        server.stop().await.is_ok(),
        "a signal ends the server cleanly"
    );

    let mut closed = closed.lock().unwrap().clone();
    closed.sort_unstable();
    assert_eq!(
        closed,
        vec![1, 2, 3],
        "every browser was closed exactly once"
    );
    drop(clients);
}

#[tokio::test]
async fn a_session_that_stays_silent_is_ended_and_its_browser_closed() {
    let options = loopback_any_port().with_idle(Duration::from_millis(400));
    let (server, closed) = stand_in_server_with(options).await;
    let mut client = Mcp::new(&server.url);
    client.initialize().await;
    client.call("start_browser", json!({})).await;
    assert!(closed.lock().unwrap().is_empty());

    eventually("the silent session's browser is closed", || {
        closed.lock().unwrap().as_slice() == [1]
    })
    .await;
}

#[tokio::test]
async fn a_client_of_a_protocol_without_sessions_gets_a_400_that_says_why() {
    let server = Running::dev(loopback_any_port()).await;
    let client = Mcp::new(&server.url);

    let response = client
        .builder(reqwest::Method::POST)
        .header("mcp-protocol-version", "2026-07-28")
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.text().await.unwrap();
    assert!(body.contains("one browser per session"), "{body}");
    assert!(
        body.contains("2025-11-25"),
        "it names a version to use: {body}"
    );

    // A client that falls back to a protocol with sessions, as the 400 asks, is
    // given a session and can use it.
    let mut older = Mcp::new(&server.url);
    let initialized = older.initialize().await;
    assert_ne!(initialized["protocolVersion"], json!("2026-07-28"));
}
