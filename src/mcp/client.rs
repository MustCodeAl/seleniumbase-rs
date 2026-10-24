//! An MCP client: call the tools of any MCP server, such as the ones of this crate, from a
//! Rust program, a script or the command line (`sbrs-mcp call`).
//!
//! A [`Target`] says where the server is: a child process that speaks MCP on its standard
//! input and output, or a streamable HTTP endpoint with an optional bearer token.
//! [`McpClient::connect`] performs the handshake, and the client then lists and calls tools,
//! lists and reads resources and lists and renders prompts.
//!
//! ```no_run
//! # async fn run() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! use std::path::PathBuf;
//! use std::time::Duration;
//!
//! use seleniumbase_rs::mcp::client::{McpClient, Target};
//!
//! let target = Target::Spawn {
//!     program: PathBuf::from("sbrs-mcp"),
//!     args: vec!["--server".into(), "dev".into()],
//!     env: Vec::new(),
//! };
//! let client = McpClient::connect(target, Duration::from_secs(60)).await?;
//! let outcome = client
//!     .call("find_api", serde_json::json!({ "query": "click" }).as_object().cloned().unwrap_or_default(), None)
//!     .await?;
//! println!("{}", outcome.text);
//! client.close().await;
//! # Ok(())
//! # }
//! ```
//!
//! Errors. A transport that breaks, a handshake that fails (a wrong bearer token, a server that
//! is not an MCP server) and a call that takes longer than the timeout are
//! [`SeleniumBaseError::Mcp`]. A server that refuses the request itself, because the tool, the
//! resource or the prompt does not exist or its arguments are wrong (JSON-RPC `invalid_params`
//! or `resource_not_found`), is [`SeleniumBaseError::InvalidConfig`] with the server's own text,
//! which names the nearest tool when there is one. A tool that ran and failed is **not** an
//! error: it is a [`CallOutcome`] with [`is_error`](CallOutcome::is_error) set, carrying the
//! failure report the server wrote.

use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use base64::Engine;
use rmcp::handler::client::ClientHandler;
use rmcp::model::{ClientConfig, ProgressNotificationParam};
use rmcp::service::{NotificationContext, RoleClient, RunningService, ServiceError};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{IntoTransport, StreamableHttpClientTransport, TokioChildProcess};
use rmcp::ServiceExt;
use serde::Serialize;
use serde_json::{json, Map, Value};
use tokio::process::Command;

use crate::error::SeleniumBaseError;

/// The protocol version the client asks for. The servers of this crate answer with the one
/// they support when it differs.
const PROTOCOL_VERSION: &str = "2025-06-18";

/// How long [`McpClient::close`] waits for the transport to wind down.
const CLOSE_GRACE: Duration = Duration::from_secs(10);

/// Where an MCP server is.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Target {
    /// A streamable HTTP endpoint, such as `http://127.0.0.1:8931/mcp`.
    Http {
        /// The endpoint URL. `https://` needs a TLS provider in the process; the library
        /// installs its own before it connects.
        url: String,
        /// The bearer token the server asks for, without the `Bearer ` prefix.
        bearer: Option<String>,
    },
    /// A program that serves MCP on its standard input and output. It is started with
    /// `args`, inherits the environment and the standard error of this process, and is
    /// ended when the client closes.
    Spawn {
        /// The program to run.
        program: PathBuf,
        /// Its arguments.
        args: Vec<String>,
        /// Variables to set for it, besides the ones it inherits.
        env: Vec<(String, String)>,
    },
}

impl Target {
    /// A short name of the target for error messages: the URL, or the program with its
    /// arguments.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Http { url, .. } => url.clone(),
            Self::Spawn { program, args, .. } => {
                let mut label = program.display().to_string();
                for arg in args {
                    label.push(' ');
                    label.push_str(arg);
                }
                label
            }
        }
    }
}

/// What the server said about itself in the handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ServerSummary {
    /// The server's name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// A display title, when it has one.
    pub title: Option<String>,
    /// The protocol version the two sides agreed on.
    pub protocol_version: String,
    /// The server's instructions for the model, when it has some.
    pub instructions: Option<String>,
    /// Whether the server offers tools.
    pub tools: bool,
    /// Whether the server offers resources.
    pub resources: bool,
    /// Whether the server offers prompts.
    pub prompts: bool,
}

/// One tool of a server.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolSummary {
    /// The name to call.
    pub name: String,
    /// A display title, when the server gives one.
    pub title: Option<String>,
    /// What the tool does.
    pub description: Option<String>,
    /// The JSON Schema of its arguments.
    pub input_schema: Value,
    /// The JSON Schema of its structured result, when it declares one.
    pub output_schema: Option<Value>,
}

/// The result of a tool call that ran.
#[derive(Debug, Clone, PartialEq)]
pub struct CallOutcome {
    /// Whether the tool reported a failure (`isError`). The text and the structured content
    /// then hold the server's failure report.
    pub is_error: bool,
    /// The text blocks of the result, joined by a line break. Other kinds of block are
    /// described in one line each, such as `[audio audio/wav, 120 bytes]`.
    pub text: String,
    /// The structured result (`structuredContent`), when the tool sends one.
    pub structured: Option<Value>,
    /// The images of the result: the MIME type and the decoded bytes.
    pub images: Vec<(String, Vec<u8>)>,
}

/// A progress report of a running tool.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientProgress {
    /// How far the tool is. It only grows.
    pub current: f64,
    /// The total, when the tool knows it.
    pub total: Option<f64>,
    /// What the tool is doing, when it says.
    pub message: Option<String>,
}

/// What [`McpClient::call`] calls with each progress report.
pub type ProgressCallback = Box<dyn FnMut(ClientProgress) + Send>;

/// A resource, or a template of resource URIs, of a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceSummary {
    /// The URI, or the URI template (RFC 6570) when [`template`](Self::template) is set.
    pub uri: String,
    /// The resource's name.
    pub name: String,
    /// A display title, when the server gives one.
    pub title: Option<String>,
    /// What the resource holds.
    pub description: Option<String>,
    /// Its MIME type, when known.
    pub mime_type: Option<String>,
    /// Whether `uri` is a template to fill in, not a URI to read.
    pub template: bool,
}

/// One piece of what [`McpClient::read_resource`] returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceContent {
    /// The URI of this piece.
    pub uri: String,
    /// Its MIME type, when known.
    pub mime_type: Option<String>,
    /// The text, for a text resource.
    pub text: Option<String>,
    /// The decoded bytes, for a binary resource.
    pub blob: Option<Vec<u8>>,
}

/// One argument of a prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptArgSummary {
    /// The argument's name.
    pub name: String,
    /// What it means.
    pub description: Option<String>,
    /// Whether the prompt cannot be rendered without it.
    pub required: bool,
}

/// A prompt of a server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptSummary {
    /// The name to render it by.
    pub name: String,
    /// A display title, when the server gives one.
    pub title: Option<String>,
    /// What the prompt is for.
    pub description: Option<String>,
    /// Its arguments.
    pub arguments: Vec<PromptArgSummary>,
}

/// A prompt rendered with its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptOutcome {
    /// What the prompt is for, when the server says.
    pub description: Option<String>,
    /// The messages: the role (`user` or `assistant`) and the text.
    pub messages: Vec<(String, String)>,
}

/// The slot the progress callback of the call in flight sits in.
type ProgressSlot = Arc<Mutex<Option<ProgressCallback>>>;

/// The client side of the handshake: names this program and hands progress reports to the call
/// that waits for them.
#[derive(Clone)]
struct Handler {
    info: ClientConfig,
    progress: ProgressSlot,
}

impl ClientHandler for Handler {
    fn get_info(&self) -> ClientConfig {
        self.info.clone()
    }

    fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        let report = to_value(&params).map(|value| ClientProgress {
            current: value["progress"].as_f64().unwrap_or_default(),
            total: value["total"].as_f64(),
            message: value["message"].as_str().map(str::to_owned),
        });
        if let Some(report) = report {
            let mut slot = self.progress.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(callback) = slot.as_mut() {
                callback(report);
            }
        }
        std::future::ready(())
    }
}

/// An MCP client over one connection. See the [module](self) for what it returns on failure.
///
/// Requests may overlap, but only one [`call`](Self::call) at a time can report progress: a
/// callback is the one of the call that set it last.
pub struct McpClient {
    service: RunningService<RoleClient, Handler>,
    label: String,
    timeout: Duration,
    progress: ProgressSlot,
}

impl fmt::Debug for McpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpClient")
            .field("target", &self.label)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

fn to_value<T: Serialize>(value: &T) -> Option<Value> {
    serde_json::to_value(value).ok()
}

fn text_of(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Why the handshake failed, in words: rmcp's own text names its transport types.
fn connect_failure(error: &dyn fmt::Display) -> String {
    let text = error.to_string();
    if text.contains("Auth required") {
        return "cannot connect: the server asks for a bearer token, and none was given or the \
                one given was not accepted"
            .to_owned();
    }
    format!("cannot connect: {text}")
}

fn client_info() -> ClientConfig {
    let info = json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": {},
        "clientInfo": { "name": "sbrs-mcp", "version": env!("CARGO_PKG_VERSION") },
    });
    // The fields are public API of the protocol; a mismatch is a bug of this module.
    serde_json::from_value(info).unwrap_or_default()
}

impl McpClient {
    /// Connects to `target` and performs the MCP handshake, giving up after `timeout`.
    /// `timeout` is also how long each later request may take.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Mcp`] if the program cannot be started, the endpoint cannot be
    /// reached or refuses the token, or the handshake does not finish in time.
    pub async fn connect(target: Target, timeout: Duration) -> Result<Self, SeleniumBaseError> {
        let label = target.label();
        match target {
            Target::Http { url, bearer } => {
                // reqwest is built without a TLS provider of its own, and a client built
                // before one is installed panics, even for plain HTTP.
                crate::install_tls_provider();
                let mut config = StreamableHttpClientTransportConfig::with_uri(url);
                if let Some(token) = bearer {
                    config = config.auth_header(token);
                }
                let transport = StreamableHttpClientTransport::from_config(config);
                Self::connect_with(&label, transport, timeout).await
            }
            Target::Spawn { program, args, env } => {
                let mut command = Command::new(&program);
                command.args(&args).envs(env);
                let transport = TokioChildProcess::new(command).map_err(|error| {
                    SeleniumBaseError::mcp(
                        &label,
                        format!("cannot start {}: {error}", program.display()),
                    )
                })?;
                Self::connect_with(&label, transport, timeout).await
            }
        }
    }

    /// [`connect`](Self::connect) over a transport the caller made: any pair of async
    /// streams, such as [`tokio::io::duplex`] (the tests of this crate), or an rmcp transport.
    /// `label` names the server in error messages.
    ///
    /// # Errors
    ///
    /// As [`connect`](Self::connect).
    pub async fn connect_with<T, E, A>(
        label: &str,
        transport: T,
        timeout: Duration,
    ) -> Result<Self, SeleniumBaseError>
    where
        T: IntoTransport<RoleClient, E, A>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let progress: ProgressSlot = Arc::new(Mutex::new(None));
        let handler = Handler {
            info: client_info(),
            progress: Arc::clone(&progress),
        };
        let service = tokio::time::timeout(timeout, handler.serve(transport))
            .await
            .map_err(|_elapsed| {
                SeleniumBaseError::mcp(
                    label,
                    format!("no answer to the handshake within {} s", timeout.as_secs()),
                )
            })?
            .map_err(|error| SeleniumBaseError::mcp(label, connect_failure(&error)))?;
        Ok(Self {
            service,
            label: label.to_owned(),
            timeout,
            progress,
        })
    }

    /// What the server said about itself in the handshake.
    #[must_use]
    pub fn server_info(&self) -> ServerSummary {
        let info = self
            .service
            .peer_info()
            .and_then(|info| to_value(&*info))
            .unwrap_or(Value::Null);
        let offers = |capability: &str| !info["capabilities"][capability].is_null();
        ServerSummary {
            name: text_of(&info["serverInfo"], "name").unwrap_or_default(),
            version: text_of(&info["serverInfo"], "version").unwrap_or_default(),
            title: text_of(&info["serverInfo"], "title"),
            protocol_version: text_of(&info, "protocolVersion").unwrap_or_default(),
            instructions: text_of(&info, "instructions"),
            tools: offers("tools"),
            resources: offers("resources"),
            prompts: offers("prompts"),
        }
    }

    /// Runs one request with the timeout and turns a failure into the error this module
    /// documents.
    async fn request<T>(
        &self,
        what: &str,
        request: impl Future<Output = Result<T, ServiceError>>,
    ) -> Result<T, SeleniumBaseError> {
        match tokio::time::timeout(self.timeout, request).await {
            Err(_) => Err(SeleniumBaseError::mcp(
                &self.label,
                format!("{what} took longer than {} s", self.timeout.as_secs()),
            )),
            Ok(Ok(answer)) => Ok(answer),
            Ok(Err(error)) => Err(failure(&self.label, what, &error)),
        }
    }

    /// Every tool of the server; follows `nextCursor` until the list is complete.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Mcp`] on a transport or protocol failure.
    pub async fn list_tools(&self) -> Result<Vec<ToolSummary>, SeleniumBaseError> {
        let tools = self
            .request("tools/list", self.service.list_all_tools())
            .await?;
        Ok(tools
            .iter()
            .filter_map(to_value)
            .map(|tool| ToolSummary {
                name: text_of(&tool, "name").unwrap_or_default(),
                title: text_of(&tool, "title"),
                description: text_of(&tool, "description"),
                input_schema: tool["inputSchema"].clone(),
                output_schema: tool.get("outputSchema").filter(|v| !v.is_null()).cloned(),
            })
            .collect())
    }

    /// Calls a tool. `on_progress` receives the progress reports the tool sends while it runs
    /// (a server sends them to the client that asked, which this client always does).
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] if the server refuses the call (no such tool,
    /// arguments that do not fit), with the server's text; [`SeleniumBaseError::Mcp`] on a
    /// transport or protocol failure or a timeout. A tool that fails is a [`CallOutcome`].
    pub async fn call(
        &self,
        tool: &str,
        arguments: Map<String, Value>,
        on_progress: Option<ProgressCallback>,
    ) -> Result<CallOutcome, SeleniumBaseError> {
        let params = serde_json::from_value(json!({ "name": tool, "arguments": arguments }))
            .map_err(|error| SeleniumBaseError::mcp(&self.label, error.to_string()))?;
        self.set_progress(on_progress);
        let answer = self
            .request("tools/call", self.service.call_tool(params))
            .await;
        self.set_progress(None);
        let result = to_value(&answer?).unwrap_or(Value::Null);
        Ok(outcome(&result))
    }

    fn set_progress(&self, callback: Option<ProgressCallback>) {
        *self.progress.lock().unwrap_or_else(PoisonError::into_inner) = callback;
    }

    /// Every resource and resource template of the server.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Mcp`] on a transport or protocol failure, including a server that
    /// has no resources.
    pub async fn list_resources(&self) -> Result<Vec<ResourceSummary>, SeleniumBaseError> {
        let fixed = self
            .request("resources/list", self.service.list_all_resources())
            .await?;
        let templates = self
            .request(
                "resources/templates/list",
                self.service.list_all_resource_templates(),
            )
            .await?;
        let summarize = |item: Value, template: bool| ResourceSummary {
            uri: text_of(&item, if template { "uriTemplate" } else { "uri" }).unwrap_or_default(),
            name: text_of(&item, "name").unwrap_or_default(),
            title: text_of(&item, "title"),
            description: text_of(&item, "description"),
            mime_type: text_of(&item, "mimeType"),
            template,
        };
        Ok(fixed
            .iter()
            .filter_map(to_value)
            .map(|item| summarize(item, false))
            .chain(
                templates
                    .iter()
                    .filter_map(to_value)
                    .map(|item| summarize(item, true)),
            )
            .collect())
    }

    /// Reads the resource at `uri`.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] if the server does not know the URI or refuses it;
    /// [`SeleniumBaseError::Mcp`] on a transport or protocol failure.
    pub async fn read_resource(
        &self,
        uri: &str,
    ) -> Result<Vec<ResourceContent>, SeleniumBaseError> {
        let params = serde_json::from_value(json!({ "uri": uri }))
            .map_err(|error| SeleniumBaseError::mcp(&self.label, error.to_string()))?;
        let answer = self
            .request("resources/read", self.service.read_resource(params))
            .await?;
        let answer = to_value(&answer).unwrap_or(Value::Null);
        Ok(answer["contents"]
            .as_array()
            .map(|contents| {
                contents
                    .iter()
                    .map(|item| ResourceContent {
                        uri: text_of(item, "uri").unwrap_or_default(),
                        mime_type: text_of(item, "mimeType"),
                        text: text_of(item, "text"),
                        blob: text_of(item, "blob").and_then(|b64| decode(&b64)),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Every prompt of the server.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Mcp`] on a transport or protocol failure, including a server that
    /// has no prompts.
    pub async fn list_prompts(&self) -> Result<Vec<PromptSummary>, SeleniumBaseError> {
        let prompts = self
            .request("prompts/list", self.service.list_all_prompts())
            .await?;
        Ok(prompts
            .iter()
            .filter_map(to_value)
            .map(|prompt| PromptSummary {
                name: text_of(&prompt, "name").unwrap_or_default(),
                title: text_of(&prompt, "title"),
                description: text_of(&prompt, "description"),
                arguments: prompt["arguments"]
                    .as_array()
                    .map(|arguments| {
                        arguments
                            .iter()
                            .map(|argument| PromptArgSummary {
                                name: text_of(argument, "name").unwrap_or_default(),
                                description: text_of(argument, "description"),
                                required: argument["required"].as_bool().unwrap_or(false),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect())
    }

    /// Renders the prompt `name` with `arguments`.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::InvalidConfig`] if there is no such prompt or an argument is
    /// missing; [`SeleniumBaseError::Mcp`] on a transport or protocol failure.
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<PromptOutcome, SeleniumBaseError> {
        let params = serde_json::from_value(json!({ "name": name, "arguments": arguments }))
            .map_err(|error| SeleniumBaseError::mcp(&self.label, error.to_string()))?;
        let answer = self
            .request("prompts/get", self.service.get_prompt(params))
            .await?;
        let answer = to_value(&answer).unwrap_or(Value::Null);
        Ok(PromptOutcome {
            description: text_of(&answer, "description"),
            messages: answer["messages"]
                .as_array()
                .map(|messages| {
                    messages
                        .iter()
                        .map(|message| {
                            (
                                text_of(message, "role").unwrap_or_default(),
                                block_text(&message["content"]),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Ends the session: tells the server (an HTTP session is deleted, which closes the
    /// browser the server made for it) and ends a child process. Waits at most ten seconds.
    pub async fn close(mut self) {
        // A transport that is already gone has nothing left to close.
        drop(tokio::time::timeout(CLOSE_GRACE, self.service.close()).await);
    }
}

/// The error for a failed request: a request the server refused is the caller's mistake.
fn failure(label: &str, what: &str, error: &ServiceError) -> SeleniumBaseError {
    if let ServiceError::McpError(data) = error {
        let refused = to_value(data).map_or(0, |value| value["code"].as_i64().unwrap_or(0));
        // -32602 is `invalid_params`; -32002 is `resource_not_found`.
        if refused == -32602 || refused == -32002 {
            return SeleniumBaseError::invalid_config(data.message.to_string());
        }
    }
    SeleniumBaseError::mcp(label, format!("{what}: {error}"))
}

fn decode(base64: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD
        .decode(base64)
        .ok()
}

/// The text of one content block, or a line that says what kind of block it was.
fn block_text(block: &Value) -> String {
    match block["type"].as_str() {
        Some("text") => text_of(block, "text").unwrap_or_default(),
        Some("image") => format!("[image {}]", text_of(block, "mimeType").unwrap_or_default()),
        Some("audio") => format!(
            "[audio {}, {} bytes]",
            text_of(block, "mimeType").unwrap_or_default(),
            text_of(block, "data")
                .and_then(|b64| decode(&b64))
                .map_or(0, |bytes| bytes.len())
        ),
        Some("resource") => block["resource"]["text"].as_str().map_or_else(
            || {
                format!(
                    "[resource {}]",
                    block["resource"]["uri"].as_str().unwrap_or("")
                )
            },
            str::to_owned,
        ),
        Some("resource_link") => {
            format!("[resource {}]", text_of(block, "uri").unwrap_or_default())
        }
        _ => block.to_string(),
    }
}

/// A `CallToolResult`, as the JSON the protocol puts on the wire, read into a [`CallOutcome`].
fn outcome(result: &Value) -> CallOutcome {
    let blocks = result["content"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let images = blocks
        .iter()
        .filter(|block| block["type"] == "image")
        .filter_map(|block| {
            let bytes = decode(block["data"].as_str()?)?;
            Some((text_of(block, "mimeType").unwrap_or_default(), bytes))
        })
        .collect();
    CallOutcome {
        is_error: result["isError"].as_bool().unwrap_or(false),
        text: blocks.iter().map(block_text).collect::<Vec<_>>().join("\n"),
        structured: result
            .get("structuredContent")
            .filter(|value| !value.is_null())
            .cloned(),
        images,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_is_named_by_its_url_or_its_command() {
        let http = Target::Http {
            url: "http://127.0.0.1:1/mcp".to_owned(),
            bearer: Some("secret".to_owned()),
        };
        assert_eq!(http.label(), "http://127.0.0.1:1/mcp");
        let spawn = Target::Spawn {
            program: PathBuf::from("/bin/server"),
            args: vec!["--server".to_owned(), "dev".to_owned()],
            env: Vec::new(),
        };
        assert_eq!(spawn.label(), "/bin/server --server dev");
    }

    #[test]
    fn a_result_is_read_into_text_structure_and_images() {
        let result = json!({
            "content": [
                { "type": "text", "text": "one" },
                { "type": "image", "data": "AAEC/w==", "mimeType": "image/png" },
                { "type": "text", "text": "two" },
            ],
            "structuredContent": { "n": 1 },
            "isError": true,
        });

        let read = outcome(&result);

        assert!(read.is_error);
        assert_eq!(read.text, "one\n[image image/png]\ntwo");
        assert_eq!(read.structured, Some(json!({ "n": 1 })));
        assert_eq!(
            read.images,
            vec![("image/png".to_owned(), vec![0, 1, 2, 255])]
        );
    }

    #[test]
    fn a_result_without_flags_is_a_success_with_no_structure() {
        let read = outcome(&json!({ "content": [{ "type": "text", "text": "done" }] }));

        assert!(!read.is_error);
        assert_eq!(read.text, "done");
        assert_eq!(read.structured, None);
        assert_eq!(read.images.len(), 0);
    }
}
