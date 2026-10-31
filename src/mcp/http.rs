//! The MCP servers over streamable HTTP, next to stdio.
//!
//! [`serve_http`] runs one of the servers of this crate as an HTTP endpoint, the
//! way `sbrs-mcp --listen ADDR` does. A client connects with the URL the server
//! prints, for example `http://127.0.0.1:8931/mcp`, and speaks MCP's streamable
//! HTTP transport (POST for requests, GET for the server's stream, DELETE to end
//! a session).
//!
//! Three decisions matter more than the rest:
//!
//! - **One browser per MCP session.** Every `initialize` request makes a new
//!   [`Host`], so two clients never share a page, and a session's browser is
//!   closed when the session ends (`DELETE`, the idle timeout or shutdown).
//!   [`HttpOptions::max_sessions`] caps the browsers; over the cap an
//!   `initialize` is answered with HTTP 503.
//! - **Loopback by default, and checked.** A browser on the same machine can be
//!   made to talk to a local server by DNS rebinding, so the `Host` and `Origin`
//!   headers are validated on every request. Listening on another address needs
//!   a bearer token ([`HttpOptions::token`]); a server that is reachable from the
//!   network without one is refused at start.
//! - **The token never travels in argv.** The command line reads it from
//!   `SB_MCP_TOKEN` or a file, and nothing here prints it.

use std::collections::HashMap;

use std::fmt;
use std::future::Future;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::Response;
use axum::Router;
use futures_util::future::join_all;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, ErrorData, GetPromptRequestParams, GetPromptResponse,
    ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ReadResourceRequestParams, ReadResourceResponse, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tracing::{debug, info, warn};
use url::Url;

use super::{cdp, dev, driver, sb, Closeable, Host, Profile, Settings};
use crate::common::shutdown::{
    cleanup_within, drain_cleanups, spawn_cleanup, timeout_from_env, Shutdown,
};
use crate::error::SeleniumBaseError;

/// The address the server listens on unless told otherwise.
pub const DEFAULT_LISTEN: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8931);

/// The URL path the MCP endpoint is mounted at unless told otherwise.
pub const DEFAULT_PATH: &str = "/mcp";

/// How many MCP sessions, and so browsers, the server holds unless told otherwise.
pub const DEFAULT_MAX_SESSIONS: usize = 4;

/// How long a session may stay silent before the server ends it.
pub const DEFAULT_IDLE: Duration = Duration::from_mins(15);

/// The environment variable `sbrs-mcp` reads the bearer token from.
pub const TOKEN_VAR: &str = "SB_MCP_TOKEN";

/// How the HTTP server listens and whom it lets in.
///
/// The defaults are the safe ones: loopback, a token required, no extra origins.
/// [`validate`](Self::validate) says whether a value may be served at all.
///
/// The token is never written by `Debug`.
#[derive(Clone)]
#[non_exhaustive]
pub struct HttpOptions {
    /// Where to listen. Port 0 picks a free port; see [`HttpServer::local_addr`].
    pub listen: SocketAddr,
    /// The URL path of the endpoint, such as `/mcp`.
    pub path: String,
    /// The bearer token every request must carry, if any. Required when
    /// `listen` is not a loopback address.
    pub token: Option<String>,
    /// Origins, besides the loopback ones, that may send requests, written
    /// `scheme://host[:port]` (or `null`). A request with no `Origin` header is
    /// always let in: only browsers send one.
    pub allow_origins: Vec<String>,
    /// `Host` header values, besides loopback names and the listen address, that
    /// are accepted: a name, optionally with `:port`. A server reached by a
    /// name needs it here.
    pub allow_hosts: Vec<String>,
    /// The most MCP sessions, and so browsers, at one time.
    pub max_sessions: usize,
    /// How long a session may stay silent before it is ended.
    pub idle: Duration,
    /// Whether a loopback listener may run without a bearer token. Off by
    /// default: a token is required on every address, and `sbrs-mcp` makes one
    /// for a loopback listener given none. Only the operator's explicit opt-out
    /// ([`with_no_token`](Self::with_no_token)) turns it on, and never for an
    /// address other machines can reach.
    pub allow_no_token: bool,
}

impl Default for HttpOptions {
    fn default() -> Self {
        Self {
            listen: DEFAULT_LISTEN,
            path: DEFAULT_PATH.to_owned(),
            token: None,
            allow_origins: Vec::new(),
            allow_hosts: Vec::new(),
            max_sessions: DEFAULT_MAX_SESSIONS,
            idle: DEFAULT_IDLE,
            allow_no_token: false,
        }
    }
}

impl fmt::Debug for HttpOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpOptions")
            .field("listen", &self.listen)
            .field("path", &self.path)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .field("allow_origins", &self.allow_origins)
            .field("allow_hosts", &self.allow_hosts)
            .field("max_sessions", &self.max_sessions)
            .field("idle", &self.idle)
            .field("allow_no_token", &self.allow_no_token)
            .finish()
    }
}

impl HttpOptions {
    /// Listens on `listen`.
    #[must_use]
    pub fn with_listen(mut self, listen: SocketAddr) -> Self {
        self.listen = listen;
        self
    }

    /// Mounts the endpoint at `path`.
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Requires `Authorization: Bearer <token>` on every request.
    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Lets requests from `origin` (`scheme://host[:port]`) in.
    #[must_use]
    pub fn with_allow_origin(mut self, origin: impl Into<String>) -> Self {
        self.allow_origins.push(origin.into());
        self
    }

    /// Accepts `host` (a name, optionally with `:port`) in the `Host` header.
    #[must_use]
    pub fn with_allow_host(mut self, host: impl Into<String>) -> Self {
        self.allow_hosts.push(host.into());
        self
    }

    /// Holds at most `max_sessions` sessions.
    #[must_use]
    pub fn with_max_sessions(mut self, max_sessions: usize) -> Self {
        self.max_sessions = max_sessions;
        self
    }

    /// Ends a session after `idle` of silence.
    #[must_use]
    pub fn with_idle(mut self, idle: Duration) -> Self {
        self.idle = idle;
        self
    }

    /// Serves a loopback listener without a bearer token: the operator's
    /// explicit opt-out, for a machine they trust every local process on. An
    /// address other machines can reach still needs a token.
    #[must_use]
    pub fn with_no_token(mut self) -> Self {
        self.allow_no_token = true;
        self
    }

    /// Checks that these options may be served.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] (exit status 2) for a path
    /// that is not an absolute URL path, a session cap of 0, an idle time of 0,
    /// an empty token or one with spaces, an origin or host that does not
    /// parse, a `listen` address that is not loopback while no token is set, and no token at all unless [`with_no_token`](Self::with_no_token) was called.
    pub fn validate(&self) -> Result<(), SeleniumBaseError> {
        if self.max_sessions == 0 {
            return Err(SeleniumBaseError::invalid_config(
                "the MCP session cap (max_sessions) must be at least 1",
            ));
        }
        if self.idle.is_zero() {
            return Err(SeleniumBaseError::invalid_config(
                "the MCP session idle time (idle) must be longer than 0",
            ));
        }
        check_path(&self.path)?;
        if let Some(token) = &self.token {
            check_token(token)?;
        }
        for origin in &self.allow_origins {
            parse_origin(origin).map_err(|reason| {
                SeleniumBaseError::invalid_config(format!(
                    "bad allowed origin {origin:?}: {reason}"
                ))
            })?;
        }
        for host in &self.allow_hosts {
            parse_authority(host).ok_or_else(|| {
                SeleniumBaseError::invalid_config(format!(
                    "bad allowed host {host:?}: expected a name, optionally followed by :port"
                ))
            })?;
        }
        if self.token.is_none() {
            if !is_loopback_ip(self.listen.ip()) {
                return Err(SeleniumBaseError::invalid_config(format!(
                        "refusing to listen on {} without a token: set {TOKEN_VAR} or pass --token-file, \
                                 or listen on a loopback address",
                        self.listen
                    )));
            }
            if !self.allow_no_token {
                return Err(SeleniumBaseError::invalid_config(
                            "refusing to serve without a bearer token: set SB_MCP_TOKEN or pass --token-file, \
                             let sbrs-mcp make one, or pass --no-token for a loopback listener you trust",
                        ));
            }
        }
        Ok(())
    }

    /// What to tell the operator when the server can be reached from other
    /// machines, or `None` for a loopback address.
    #[must_use]
    pub fn exposure_warning(&self) -> Option<String> {
        if is_loopback_ip(self.listen.ip()) {
            return None;
        }
        Some(format!(
                    "listening on {}, so the browser tools are reachable from other machines; \
                     this server speaks plain HTTP, so keep the token secret and put TLS in front of it",
                    self.listen
                ))
    }
}

/// A fresh bearer token: 32 random bytes from the system's CSPRNG, as 64 hex
/// digits. `sbrs-mcp` makes one for a loopback listener that was given none.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Mcp`] if the system gives no random bytes.
pub fn generate_token() -> Result<String, SeleniumBaseError> {
    use ring::rand::SecureRandom;
    let mut bytes = [0u8; 32];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| {
            SeleniumBaseError::mcp("http", "the system gave no random bytes for a token")
        })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Why a request was turned away before it reached the MCP service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// The path is not the endpoint's.
    NotFound,
    /// The `Host` header names somewhere this server is not.
    ForeignHost,
    /// The `Origin` header names a site that may not use this server.
    ForeignOrigin,
    /// The bearer token is missing or wrong.
    Unauthorized,
    /// The client speaks a protocol version without sessions, and a browser
    /// needs one.
    Stateless,
}

/// The checks made on every request, from its headers alone.
struct Gate {
    path: String,
    token_digest: Option<[u8; 32]>,
    hosts: Vec<AllowedHost>,
    origins: Vec<String>,
}

/// A `Host` the server answers to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AllowedHost {
    name: String,
    port: Option<u16>,
}

impl Gate {
    /// The gate for `options`, for a server bound to `bound`.
    ///
    /// Entries of `options` that do not parse are left out: [`HttpOptions::validate`]
    /// refuses them before a server starts.
    fn new(options: &HttpOptions, bound: SocketAddr) -> Self {
        let mut hosts = Vec::new();
        // A wildcard address names no host a client could put in `Host`.
        if !bound.ip().is_unspecified() {
            hosts.push(AllowedHost {
                name: bound.ip().to_canonical().to_string(),
                port: None,
            });
        }
        hosts.extend(
            options
                .allow_hosts
                .iter()
                .filter_map(|h| parse_authority(h)),
        );
        Self {
            path: options.path.clone(),
            token_digest: options.token.as_deref().map(digest),
            hosts,
            origins: options
                .allow_origins
                .iter()
                .filter_map(|origin| parse_origin(origin).ok())
                .map(|origin| origin.normalized)
                .collect(),
        }
    }

    /// Whether a request for `path` with these headers may go on to the MCP
    /// service. The checks run in this order: `Host`, `Origin`, token, path, protocol version. The
    /// two that guard against DNS rebinding come first, so a hostile page learns
    /// nothing, not even whether a token is needed.
    fn check(&self, path: &str, headers: &HeaderMap) -> Result<(), Refusal> {
        let host_ok = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| self.host_allowed(value));
        if !host_ok {
            return Err(Refusal::ForeignHost);
        }
        if let Some(origin) = headers.get(header::ORIGIN) {
            let origin_ok = origin
                .to_str()
                .ok()
                .and_then(|value| parse_origin(value).ok())
                .is_some_and(|origin| origin.loopback || self.origins.contains(&origin.normalized));
            if !origin_ok {
                return Err(Refusal::ForeignOrigin);
            }
        }
        if let Some(expected) = &self.token_digest {
            let token_ok = bearer(headers).is_some_and(|sent| same_digest(&digest(sent), expected));
            if !token_ok {
                return Err(Refusal::Unauthorized);
            }
        }
        if path != self.path {
            return Err(Refusal::NotFound);
        }
        let stateless = headers
            .get(PROTOCOL_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(is_stateless_version);
        if stateless {
            return Err(Refusal::Stateless);
        }
        Ok(())
    }

    /// Whether `Host: value` names this machine or somewhere the operator allowed.
    fn host_allowed(&self, value: &str) -> bool {
        let Some(host) = parse_authority(value) else {
            return false;
        };
        is_loopback_name(&host.name)
            || self.hosts.iter().any(|allowed| {
                allowed.name == host.name && allowed.port.is_none_or(|port| host.port == Some(port))
            })
    }
}

/// An `initialize` request: the one that creates a session.
fn starts_session(method: &Method, headers: &HeaderMap) -> bool {
    *method == Method::POST && !headers.contains_key(SESSION_HEADER)
}

/// The sessions that are alive, each with its `Host`.
struct Registry<S> {
    /// The most sessions at once.
    max: usize,
    /// How long a closing browser may take.
    grace: Duration,
    next: AtomicU64,
    live: Mutex<HashMap<u64, Host<S>>>,
}

impl<S: Closeable> Registry<S> {
    fn new(max: usize, grace: Duration) -> Self {
        Self {
            max,
            grace,
            next: AtomicU64::new(1),
            live: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<u64, Host<S>>> {
        // A panic while the map is held cannot leave it in a bad state.
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// How many sessions are alive.
    fn live(&self) -> usize {
        self.lock().len()
    }

    /// Whether another session would go over the cap.
    fn is_full(&self) -> bool {
        self.live() >= self.max
    }

    /// Records a new session, or `None` when the cap is reached.
    fn admit(&self, host: Host<S>) -> Option<u64> {
        let mut live = self.lock();
        if live.len() >= self.max {
            return None;
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        live.insert(id, host);
        Some(id)
    }

    /// Forgets a session that ended. `None` if shutdown already took it.
    fn release(&self, id: u64) -> Option<Host<S>> {
        self.lock().remove(&id)
    }

    /// Takes every live session, for shutdown.
    fn drain(&self) -> Vec<Host<S>> {
        self.lock().drain().map(|(_, host)| host).collect()
    }
}

/// A live MCP session: its own [`Host`], and the registry slot that goes with it.
struct Session<S: Closeable> {
    host: Host<S>,
    id: u64,
    registry: Arc<Registry<S>>,
}

impl<S: Closeable> Drop for Session<S> {
    /// The session is over (`DELETE`, the idle timeout or shutdown): close its
    /// browser. A drop cannot await, so the close is spawned and the process
    /// waits for it before it exits ([`drain_cleanups`]).
    fn drop(&mut self) {
        let Some(host) = self.registry.release(self.id) else {
            // Shutdown took the session and closes it itself.
            return;
        };
        let grace = self.registry.grace;
        info!(
            server = host.name(),
            session = self.id,
            "an MCP session ended"
        );
        spawn_cleanup(async move {
            cleanup_within(grace, host.shutdown()).await;
        });
    }
}

// The session is its host: every call goes to the host that is only this
// session's.
impl<S: Closeable> ServerHandler for Session<S> {
    fn get_info(&self) -> ServerConfig {
        self.host.get_info()
    }

    fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        self.host.list_tools(request, context)
    }

    fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, ErrorData>> + '_ {
        self.host.list_resources(request, context)
    }

    fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, ErrorData>> + '_ {
        self.host.list_resource_templates(request, context)
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResponse, ErrorData>> + '_ {
        self.host.read_resource(request, context)
    }

    fn list_prompts(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, ErrorData>> + '_ {
        self.host.list_prompts(request, context)
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResponse, ErrorData>> + '_ {
        self.host.get_prompt(request, context)
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, ErrorData>> + '_ {
        self.host.call_tool(request, context)
    }
}

type Closing = Pin<Box<dyn Future<Output = ()> + Send>>;

/// What the server needs once it is running to stop its sessions.
struct Control {
    cancel: Box<dyn Fn() + Send + Sync>,
    close: Box<dyn Fn(Duration) -> Closing + Send + Sync>,
}

/// A bound HTTP listener that has not started serving yet.
///
/// Binding first lets a caller learn the port (`--listen 127.0.0.1:0`) before
/// any request is handled.
pub struct HttpServer {
    listener: TcpListener,
    local: SocketAddr,
    name: &'static str,
    path: String,
    warning: Option<String>,
    router: Router,
    control: Control,
    grace: Duration,
}

impl fmt::Debug for HttpServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpServer")
            .field("server", &self.name)
            .field("local", &self.local)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl HttpServer {
    /// Binds the listener for `profile`, with `settings` for every session's host.
    ///
    /// # Errors
    ///
    /// As [`bind_with`](Self::bind_with).
    pub async fn bind(
        profile: Profile,
        settings: Settings,
        options: HttpOptions,
    ) -> Result<Self, SeleniumBaseError> {
        match profile {
            Profile::Cdp => {
                Self::bind_named(profile.name(), options, move || cdp::host(settings.clone())).await
            }
            Profile::Driver => {
                Self::bind_named(profile.name(), options, move || {
                    driver::host(settings.clone())
                })
                .await
            }
            Profile::Sb => {
                Self::bind_named(profile.name(), options, move || sb::host(settings.clone())).await
            }
            Profile::Dev => {
                Self::bind_named(profile.name(), options, move || dev::host(settings.clone())).await
            }
        }
    }

    /// Binds the listener for the hosts `make_host` builds, one per MCP session.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if the options fail
    /// [`HttpOptions::validate`], and [`SeleniumBaseError::Mcp`] if the address
    /// cannot be bound.
    pub async fn bind_with<S, F>(
        options: HttpOptions,
        make_host: F,
    ) -> Result<Self, SeleniumBaseError>
    where
        S: Closeable,
        F: Fn() -> Host<S> + Send + Sync + 'static,
    {
        let name = make_host().name();
        Self::bind_named(name, options, make_host).await
    }

    async fn bind_named<S, F>(
        name: &'static str,
        options: HttpOptions,
        make_host: F,
    ) -> Result<Self, SeleniumBaseError>
    where
        S: Closeable,
        F: Fn() -> Host<S> + Send + Sync + 'static,
    {
        options.validate()?;
        let failed = |reason: String| SeleniumBaseError::mcp("http", reason);
        let listener = TcpListener::bind(options.listen)
            .await
            .map_err(|e| failed(format!("cannot listen on {}: {e}", options.listen)))?;
        let local = listener
            .local_addr()
            .map_err(|e| failed(format!("cannot read the listen address: {e}")))?;

        let grace = timeout_from_env();
        let registry = Arc::new(Registry::<S>::new(options.max_sessions, grace));
        let mut sessions = LocalSessionManager::default();
        sessions.session_config.keep_alive = Some(options.idle);
        // The checks of `Gate` are the only ones: rmcp's own `Host` and
        // `Origin` lists would have to agree with them to the byte.
        let config = StreamableHttpServerConfig::default()
            .disable_allowed_hosts()
            .disable_allowed_origins();
        let cancellation = config.cancellation_token.clone();

        let for_sessions = Arc::clone(&registry);
        let service = StreamableHttpService::new(
            move || {
                // A new session gets a host of its own, so it has a browser of its own.
                let host = make_host();
                let id = for_sessions.admit(host.clone()).ok_or_else(|| {
                    io::Error::other(format!("too many sessions ({})", for_sessions.max))
                })?;
                Ok(Session {
                    host,
                    id,
                    registry: Arc::clone(&for_sessions),
                })
            },
            Arc::new(sessions),
            config,
        );
        let app = Arc::new(App {
            gate: Gate::new(&options, local),
            registry: Arc::clone(&registry),
            service,
        });
        let control = Control {
            cancel: Box::new(move || cancellation.cancel()),
            close: Box::new(move |grace| {
                let registry = Arc::clone(&registry);
                Box::pin(async move {
                    let hosts = registry.drain();
                    cleanup_within(grace, async {
                        join_all(hosts.iter().map(Host::shutdown)).await;
                    })
                    .await;
                })
            }),
        };
        Ok(Self {
            listener,
            local,
            name,
            path: options.path.clone(),
            warning: options.exposure_warning(),
            router: Router::new().fallback(handle::<S>).with_state(app),
            control,
            grace,
        })
    }

    /// The address the listener is bound to, with the port the system chose for
    /// port 0.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// The URL of the endpoint, such as `http://127.0.0.1:8931/mcp`.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}{}", self.local, self.path)
    }

    /// The name of the server being served (`cdp`, `driver`, `sb`, `dev`).
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// What to tell the operator about how the server is exposed, if anything.
    #[must_use]
    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    /// Gives shutdown `grace` to close the sessions' browsers, instead of
    /// [`timeout_from_env`].
    #[must_use]
    pub fn with_grace(mut self, grace: Duration) -> Self {
        self.grace = grace;
        self
    }

    /// Serves requests until `shutdown` completes, then ends every session and
    /// closes their browsers, waiting at most the grace period.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Mcp`] if the listener fails.
    pub async fn serve<F>(self, shutdown: F) -> Result<(), SeleniumBaseError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let Self {
            listener,
            router,
            control,
            grace,
            name,
            ..
        } = self;
        let (stop, stopped) = oneshot::channel::<()>();
        let serving = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = stopped.await;
        });
        let mut serving = tokio::spawn(async move { serving.await });

        let ended_early = tokio::select! {
            ended = &mut serving => Some(ended),
            () = shutdown => None,
        };
        info!(server = name, "stopping the HTTP server");
        // Ends every session and the streams they hold open, which a graceful
        // stop of the listener would otherwise wait for.
        (control.cancel)();
        let outcome = if let Some(ended) = ended_early {
            listener_outcome(ended)
        } else {
            let _ = stop.send(());
            if let Ok(ended) = tokio::time::timeout(grace, &mut serving).await {
                listener_outcome(ended)
            } else {
                warn!(
                    server = name,
                    "connections did not close in time; dropping them"
                );
                serving.abort();
                Ok(())
            }
        };
        (control.close)(grace).await;
        drain_cleanups(grace).await;
        outcome
    }
}

/// Serves `profile` over streamable HTTP until SIGINT or SIGTERM.
///
/// Once the listener is bound, the one line `sbrs-mcp: listening on
/// http://ADDR/PATH (server NAME)` goes to standard error. A signal ends the
/// server cleanly, with `Ok(())`. Every session's browser is closed first, within
/// the grace period of [`timeout_from_env`].
///
/// Options that fail [`HttpOptions::validate`] are refused before anything is bound.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for options that fail
/// [`HttpOptions::validate`] (nothing is bound then), [`SeleniumBaseError::Mcp`]
/// if the address cannot be bound or the listener fails, and
/// [`SeleniumBaseError::Unsupported`] if the signal handlers cannot be
/// installed.
pub async fn serve_http(
    profile: Profile,
    settings: Settings,
    options: HttpOptions,
) -> Result<(), SeleniumBaseError> {
    options.validate()?;
    // Installed before anything listens: from here a SIGTERM no longer ends the
    // process by itself, so the wait below also listens for it.
    let mut signals = Shutdown::install()?;
    let server = HttpServer::bind(profile, settings, options).await?;
    if let Some(warning) = server.warning() {
        warn!("{warning}");
    }
    // Tests and scripts read the address from this line (`--listen 127.0.0.1:0`).
    eprintln!(
        "sbrs-mcp: listening on {} (server {})",
        server.url(),
        server.name()
    );
    server
        .serve(async move {
            let signal = signals.recv().await;
            info!(%signal, "received a signal");
        })
        .await
}

/// The header that names the protocol version a request speaks.
const PROTOCOL_HEADER: &str = "mcp-protocol-version";

/// The first protocol version without sessions. The service answers a request
/// of that version on its own, with a new handler for it, so a browser one call
/// started would be gone for the next.
const FIRST_STATELESS_VERSION: &str = "2026-07-28";

/// Whether a protocol version header names a version without sessions.
fn is_stateless_version(version: &str) -> bool {
    // Versions are dates, so they sort as text.
    version.starts_with(|c: char| c.is_ascii_digit()) && version >= FIRST_STATELESS_VERSION
}

/// The header that names an MCP session.
const SESSION_HEADER: &str = "mcp-session-id";

/// What every request needs: the checks, the live sessions and the MCP service.
struct App<S: Closeable> {
    gate: Gate,
    registry: Arc<Registry<S>>,
    service: StreamableHttpService<Session<S>, LocalSessionManager>,
}

/// Answers one request: turns it away, or hands it to the MCP service.
async fn handle<S: Closeable>(State(app): State<Arc<App<S>>>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    if let Err(refusal) = app.gate.check(parts.uri.path(), &parts.headers) {
        return refuse(refusal, &parts.method);
    }
    if starts_session(&parts.method, &parts.headers) && app.registry.is_full() {
        warn!(
            max = app.registry.max,
            "refused a new MCP session: too many sessions"
        );
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &format!("too many sessions ({})", app.registry.max),
        );
    }
    app.service
        .handle(Request::from_parts(parts, body))
        .await
        .map(Body::new)
}

/// The answer to a request the gate turned away. It says what was wrong, never
/// what the right value is.
fn refuse(refusal: Refusal, method: &Method) -> Response {
    let (status, message) = match refusal {
        Refusal::NotFound => (StatusCode::NOT_FOUND, "not found"),
        Refusal::ForeignHost => (StatusCode::FORBIDDEN, "forbidden: unexpected Host header"),
        Refusal::ForeignOrigin => (StatusCode::FORBIDDEN, "forbidden: unexpected Origin header"),
        Refusal::Stateless => (
            StatusCode::BAD_REQUEST,
            "unsupported MCP protocol version: this server keeps one browser per session, and \
             protocol versions from 2026-07-28 on have no sessions; send \
             MCP-Protocol-Version: 2025-11-25 or an earlier version",
        ),

        Refusal::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            "unauthorized: send Authorization: Bearer <token>",
        ),
    };
    if refusal == Refusal::NotFound {
        debug!(%method, "an MCP HTTP request for another path");
    } else {
        warn!(?refusal, %method, "refused an MCP HTTP request");
    }
    let mut response = error_response(status, message);
    if refusal == Refusal::Unauthorized {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"sbrs-mcp\""),
        );
    }
    response
}

/// A JSON answer `{"error": message}` with this status.
fn error_response(status: StatusCode, message: &str) -> Response {
    let mut response = Response::new(Body::from(json!({ "error": message }).to_string()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

/// How the task that ran the listener ended.
fn listener_outcome(
    ended: Result<io::Result<()>, tokio::task::JoinError>,
) -> Result<(), SeleniumBaseError> {
    match ended {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(SeleniumBaseError::mcp(
            "http",
            format!("the listener failed: {error}"),
        )),
        Err(error) => Err(SeleniumBaseError::mcp(
            "http",
            format!("the listener task failed: {error}"),
        )),
    }
}

/// Whether `ip` is a loopback address, an IPv4-mapped one included.
fn is_loopback_ip(ip: IpAddr) -> bool {
    ip.to_canonical().is_loopback()
}

/// Whether a host name from a header is this machine: `localhost` or a
/// loopback address. A name that merely ends in one (`localhost.evil.test`) is
/// not.
fn is_loopback_name(name: &str) -> bool {
    name == "localhost" || name.parse::<IpAddr>().is_ok_and(is_loopback_ip)
}

/// A `Host` header value, or an allowed host: a name or an IP, with an optional
/// port. IPv6 addresses are in brackets. `None` for anything else.
fn parse_authority(text: &str) -> Option<AllowedHost> {
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_graphic())
        || text.contains(['/', '\\', '@', '?', '#'])
    {
        return None;
    }
    let (name, port) = if let Some(rest) = text.strip_prefix('[') {
        let (inside, after) = rest.split_once(']')?;
        let ip: Ipv6Addr = inside.parse().ok()?;
        let port = if after.is_empty() {
            None
        } else {
            Some(after.strip_prefix(':')?.parse::<u16>().ok()?)
        };
        (IpAddr::V6(ip).to_canonical().to_string(), port)
    } else {
        let (name, port) = match text.rsplit_once(':') {
            Some((name, port)) => (name, Some(port.parse::<u16>().ok()?)),
            None => (text, None),
        };
        let plain = |byte: u8| byte.is_ascii_alphanumeric() || b"-._".contains(&byte);
        if name.is_empty() || !name.bytes().all(plain) {
            return None;
        }
        (name.to_ascii_lowercase(), port)
    };
    Some(AllowedHost { name, port })
}

/// An `Origin` header value or an allowed origin, in a form that can be compared.
struct Origin {
    /// `scheme://host[:port]` in lower case without a default port, or `null`.
    normalized: String,
    /// Whether it names this machine.
    loopback: bool,
}

/// Reads `scheme://host[:port]` (http or https), or `null`.
fn parse_origin(text: &str) -> Result<Origin, String> {
    if text == "null" {
        return Ok(Origin {
            normalized: "null".to_owned(),
            loopback: false,
        });
    }
    let url = Url::parse(text).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("the scheme must be http or https".to_owned());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err("an origin is scheme://host[:port] with nothing after it".to_owned());
    }
    let loopback = match url.host() {
        Some(url::Host::Domain(name)) => is_loopback_name(name),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => is_loopback_ip(IpAddr::V6(ip)),
        None => return Err("an origin needs a host".to_owned()),
    };
    Ok(Origin {
        normalized: url.origin().ascii_serialization(),
        loopback,
    })
}

/// Checks the URL path of the endpoint.
fn check_path(path: &str) -> Result<(), SeleniumBaseError> {
    let bad = |reason: &str| {
        Err(SeleniumBaseError::invalid_config(format!(
            "the MCP path {path:?} {reason}"
        )))
    };
    if !path.starts_with('/') {
        return bad("must start with /");
    }
    if !path
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte))
    {
        return bad("may only hold letters, digits and - . _ ~ /");
    }
    if path == "/" {
        return Ok(());
    }
    if path.ends_with('/') {
        return bad("must not end with /");
    }
    if path[1..]
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return bad("must not have an empty, . or .. segment");
    }
    Ok(())
}

/// Checks that a token can travel in a header and be written to a file.
fn check_token(token: &str) -> Result<(), SeleniumBaseError> {
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(SeleniumBaseError::invalid_config(
            "the MCP token must be one non-empty word of printable characters, with no spaces",
        ));
    }
    Ok(())
}

/// The SHA-256 of `text`. Tokens are compared by their digests, so the
/// comparison takes the same time whatever the token is and whatever its length.
fn digest(text: &str) -> [u8; 32] {
    let hash = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let mut out = [0_u8; 32];
    out.copy_from_slice(hash.as_ref());
    out
}

/// Whether two digests are equal, looking at every byte whatever they hold.
fn same_digest(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let difference = left
        .iter()
        .zip(right)
        .fold(0_u8, |seen, (a, b)| seen | (a ^ b));
    std::hint::black_box(difference) == 0
}

/// The credential of an `Authorization: Bearer` header.
fn bearer(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then_some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::http::HeaderName;

    const LOCAL: &str = "127.0.0.1:8931";

    fn addr(text: &str) -> SocketAddr {
        text.parse().expect("a socket address")
    }

    fn options() -> HttpOptions {
        // The operator's opt-out: these tests are about the other checks; the
        // token policy has tests of its own.
        HttpOptions::default().with_no_token()
    }

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.append(
                HeaderName::from_bytes(name.as_bytes()).expect("a header name"),
                HeaderValue::from_str(value).expect("a header value"),
            );
        }
        map
    }

    fn gate(options: &HttpOptions) -> Gate {
        Gate::new(options, options.listen)
    }

    /// The gate's answer for a `POST /mcp` with these headers.
    fn check(gate: &Gate, pairs: &[(&str, &str)]) -> Result<(), Refusal> {
        gate.check("/mcp", &headers(pairs))
    }

    #[test]
    fn a_listener_needs_a_token_unless_the_operator_opts_out_on_loopback() {
        let error = HttpOptions::default().validate().unwrap_err().to_string();
        assert!(error.contains("without a bearer token"), "{error}");
        options()
            .with_no_token()
            .validate()
            .expect("the opt-out serves loopback");
        options()
            .with_token("t0ken")
            .validate()
            .expect("a token serves");
        let exposed = options().with_listen(addr("0.0.0.0:8931")).with_no_token();
        let error = exposed.validate().unwrap_err().to_string();
        assert!(
            error.contains("refusing to listen on 0.0.0.0:8931 without a token"),
            "{error}"
        );
    }

    #[test]
    fn a_made_token_is_long_random_hex() {
        let first = generate_token().expect("random bytes");
        let second = generate_token().expect("random bytes");
        assert_eq!(first.len(), 64);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()), "{first}");
        assert_ne!(first, second);
    }

    fn refused(options: &HttpOptions) -> String {
        let error = options.validate().expect_err("these options are refused");
        assert_eq!(error.exit_code(), 2, "an operator mistake exits 2: {error}");
        error.to_string()
    }

    // ------------------------------------------------------------------
    // HttpOptions
    // ------------------------------------------------------------------

    #[test]
    fn the_defaults_are_servable_and_loopback() {
        let defaults = options();
        defaults.validate().expect("the defaults are valid");
        assert_eq!(defaults.listen, addr(LOCAL));
        assert_eq!(defaults.path, "/mcp");
        assert_eq!(defaults.max_sessions, 4);
        assert_eq!(defaults.idle, Duration::from_mins(15));
        assert_eq!(defaults.exposure_warning(), None);
        assert!(
            !HttpOptions::default().allow_no_token,
            "a token is required unless the operator opts out"
        );
    }

    #[test]
    fn an_address_others_can_reach_needs_a_token() {
        for listen in [
            "0.0.0.0:8931",
            "[::]:8931",
            "192.168.1.5:8931",
            "10.0.0.2:0",
        ] {
            let message = refused(&options().with_listen(addr(listen)));
            assert!(
                message.contains("refusing to listen on") && message.contains(listen),
                "{message}"
            );
            assert!(message.contains("token"), "{message}");
            options()
                .with_listen(addr(listen))
                .with_token("s3cret")
                .validate()
                .expect("a token makes it servable");
        }
    }

    #[test]
    fn every_loopback_address_is_servable_with_the_opt_out() {
        for listen in [
            "127.0.0.1:0",
            "127.0.0.2:8931",
            "[::1]:8931",
            "[::ffff:127.0.0.1]:8931",
        ] {
            options()
                .with_listen(addr(listen))
                .validate()
                .unwrap_or_else(|e| panic!("{listen}: {e}"));
        }
    }

    #[test]
    fn a_path_is_an_absolute_plain_url_path() {
        for path in ["/", "/mcp", "/a-b_c.d/e~f"] {
            options()
                .with_path(path)
                .validate()
                .unwrap_or_else(|e| panic!("{path}: {e}"));
        }
        for path in [
            "", "mcp", "/mcp/", "/a//b", "/a/../b", "/a/./b", "/mcp?x=1", "/mcp#top", "/m cp",
            "/mcp\n",
        ] {
            let message = refused(&options().with_path(path));
            assert!(message.contains("path"), "{path:?}: {message}");
        }
    }

    #[test]
    fn a_session_cap_or_an_idle_time_of_zero_is_refused() {
        assert!(refused(&options().with_max_sessions(0)).contains("max"));
        assert!(refused(&options().with_idle(Duration::ZERO)).contains("idle"));
        options()
            .with_max_sessions(1)
            .validate()
            .expect("one is fine");
    }

    #[test]
    fn a_token_is_one_non_empty_word() {
        for token in ["", " ", "a b", "a\tb", "a\nb", "tok\r\n"] {
            assert!(
                refused(&options().with_token(token)).contains("token"),
                "{token:?}"
            );
        }
        options()
            .with_token("s3cret-_.~+/=")
            .validate()
            .expect("a plain token");
    }

    #[test]
    fn an_allowed_origin_is_a_scheme_a_host_and_maybe_a_port() {
        for origin in ["https://app.example.com", "http://localhost:3000", "null"] {
            options()
                .with_allow_origin(origin)
                .validate()
                .unwrap_or_else(|e| panic!("{origin}: {e}"));
        }
        for origin in [
            "",
            "evil.test",
            "ftp://x.test",
            "https://app.example.com/path",
            "https://u:p@app.example.com",
            "https://app.example.com?x=1",
        ] {
            let message = refused(&options().with_allow_origin(origin));
            assert!(message.contains("origin"), "{origin:?}: {message}");
        }
    }

    #[test]
    fn an_allowed_host_is_a_name_with_an_optional_port() {
        for host in ["mcp.example.com", "mcp.example.com:8443", "[::1]:80"] {
            options()
                .with_allow_host(host)
                .validate()
                .unwrap_or_else(|e| panic!("{host}: {e}"));
        }
        for host in ["", "a b", "a/b", "host:notaport", "http://host", "u@host"] {
            let message = refused(&options().with_allow_host(host));
            assert!(message.contains("host"), "{host:?}: {message}");
        }
    }

    #[test]
    fn only_an_address_others_can_reach_gets_an_exposure_warning() {
        for listen in ["0.0.0.0:8931", "192.168.1.5:8931", "[::]:8931"] {
            let warning = options()
                .with_listen(addr(listen))
                .with_token("s3cret")
                .exposure_warning()
                .unwrap_or_else(|| panic!("{listen} is reachable"));
            assert!(warning.contains(listen), "{warning}");
            assert!(!warning.contains("s3cret"), "{warning}");
        }
        for listen in ["127.0.0.1:8931", "[::1]:8931"] {
            assert_eq!(options().with_listen(addr(listen)).exposure_warning(), None);
        }
    }

    #[test]
    fn debug_never_shows_the_token() {
        let shown = format!("{:?}", options().with_token("hunter2-hunter2"));
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("redacted"), "{shown}");
    }

    // ------------------------------------------------------------------
    // The gate
    // ------------------------------------------------------------------

    #[test]
    fn loopback_hosts_are_accepted_on_any_port() {
        let gate = gate(&options());
        for host in [
            "127.0.0.1:8931",
            "127.0.0.1",
            "localhost",
            "LOCALHOST:3000",
            "[::1]:8931",
            "127.0.0.9:80",
        ] {
            assert_eq!(check(&gate, &[("host", host)]), Ok(()), "{host}");
        }
    }

    #[test]
    fn a_host_that_only_looks_like_loopback_is_refused() {
        // The shape of a DNS rebinding attack: the page's own name resolves to
        // the loopback address, so the browser sends that name in `Host`.
        let gate = gate(&options());
        for host in [
            "evil.test",
            "evil.test:8931",
            "localhost.evil.test",
            "127.0.0.1.evil.test",
            "evil.test:127.0.0.1",
            "0.0.0.0:8931",
            "",
            "local host",
        ] {
            assert_eq!(
                check(&gate, &[("host", host)]),
                Err(Refusal::ForeignHost),
                "{host:?}"
            );
        }
    }

    #[test]
    fn a_request_without_a_host_is_refused() {
        assert_eq!(check(&gate(&options()), &[]), Err(Refusal::ForeignHost));
    }

    #[test]
    fn the_listen_address_and_the_allowed_hosts_are_accepted() {
        let options = options()
            .with_listen(addr("192.168.1.5:8931"))
            .with_token("s3cret")
            .with_allow_host("mcp.example.com")
            .with_allow_host("other.example.com:9000");
        let gate = gate(&options);
        let token = ("authorization", "Bearer s3cret");
        for host in [
            "192.168.1.5:8931",
            "mcp.example.com",
            "MCP.example.com:1234",
            "other.example.com:9000",
            "localhost",
        ] {
            assert_eq!(check(&gate, &[("host", host), token]), Ok(()), "{host}");
        }
        for host in ["other.example.com:9001", "other.example.com", "192.168.1.6"] {
            assert_eq!(
                check(&gate, &[("host", host), token]),
                Err(Refusal::ForeignHost),
                "{host}"
            );
        }
    }

    #[test]
    fn a_request_without_an_origin_and_loopback_origins_get_in() {
        let gate = gate(&options());
        let host = ("host", LOCAL);
        assert_eq!(check(&gate, &[host]), Ok(()));
        for origin in [
            "http://localhost:3000",
            "https://127.0.0.1",
            "http://[::1]:5173",
            "http://LOCALHOST",
        ] {
            assert_eq!(
                check(&gate, &[host, ("origin", origin)]),
                Ok(()),
                "{origin}"
            );
        }
    }

    #[test]
    fn a_foreign_origin_is_refused() {
        let gate = gate(&options());
        for origin in [
            "https://evil.test",
            "http://localhost.evil.test",
            "http://127.0.0.1.evil.test",
            "null",
            "file://",
            "not an origin",
            "",
        ] {
            assert_eq!(
                check(&gate, &[("host", LOCAL), ("origin", origin)]),
                Err(Refusal::ForeignOrigin),
                "{origin:?}"
            );
        }
    }

    #[test]
    fn an_allowed_origin_matches_on_scheme_host_and_port() {
        let options = options()
            .with_allow_origin("https://app.example.com")
            .with_allow_origin("http://tool.example.com:8080");
        let gate = gate(&options);
        let ok = |origin| check(&gate, &[("host", LOCAL), ("origin", origin)]);
        assert_eq!(ok("https://app.example.com"), Ok(()));
        assert_eq!(
            ok("HTTPS://APP.EXAMPLE.COM:443"),
            Ok(()),
            "the default port"
        );
        assert_eq!(ok("http://tool.example.com:8080"), Ok(()));
        assert_eq!(ok("http://app.example.com"), Err(Refusal::ForeignOrigin));
        assert_eq!(
            ok("https://app.example.com:8443"),
            Err(Refusal::ForeignOrigin)
        );
        assert_eq!(ok("http://tool.example.com"), Err(Refusal::ForeignOrigin));
    }

    #[test]
    fn the_token_is_a_bearer_credential_and_must_match_exactly() {
        let gate = gate(&options().with_token("s3cret"));
        let host = ("host", LOCAL);
        let with = |value| check(&gate, &[host, ("authorization", value)]);
        assert_eq!(check(&gate, &[host]), Err(Refusal::Unauthorized));
        assert_eq!(with("Bearer s3cret"), Ok(()));
        assert_eq!(
            with("bearer s3cret"),
            Ok(()),
            "the scheme is case-insensitive"
        );
        for value in [
            "Bearer wrong",
            "Bearer s3cre",
            "Bearer s3cret2",
            "Bearer s3cret ",
            "Bearer  s3cret",
            "s3cret",
            "Basic czNjcmV0",
            "Bearer",
            "",
        ] {
            assert_eq!(with(value), Err(Refusal::Unauthorized), "{value:?}");
        }
    }

    #[test]
    fn without_a_token_an_authorization_header_is_ignored() {
        let gate = gate(&options());
        assert_eq!(
            check(&gate, &[("host", LOCAL), ("authorization", "Bearer x")]),
            Ok(())
        );
    }

    #[test]
    fn host_and_origin_are_checked_before_the_token() {
        let gate = gate(&options().with_token("s3cret"));
        assert_eq!(
            check(&gate, &[("host", LOCAL), ("origin", "https://evil.test")]),
            Err(Refusal::ForeignOrigin),
            "a rebinding page learns nothing about the token"
        );
        assert_eq!(
            check(&gate, &[("host", "evil.test")]),
            Err(Refusal::ForeignHost)
        );
    }

    #[test]
    fn only_the_endpoint_path_is_served() {
        let gate = gate(&options().with_path("/sbrs"));
        let host = headers(&[("host", LOCAL)]);
        assert_eq!(gate.check("/sbrs", &host), Ok(()));
        for path in ["/", "/mcp", "/sbrs/", "/sbrs/x", "/SBRS"] {
            assert_eq!(gate.check(path, &host), Err(Refusal::NotFound), "{path}");
        }
    }

    #[test]
    fn a_post_without_a_session_id_starts_a_session() {
        let with_id = headers(&[("mcp-session-id", "abc")]);
        let without = headers(&[]);
        assert!(starts_session(&Method::POST, &without));
        assert!(!starts_session(&Method::POST, &with_id));
        assert!(!starts_session(&Method::GET, &without));
        assert!(!starts_session(&Method::DELETE, &without));
    }

    // ------------------------------------------------------------------
    // The registry
    // ------------------------------------------------------------------

    #[derive(Debug)]
    struct Nothing;

    impl Closeable for Nothing {
        async fn close(self) {}
    }

    fn host() -> Host<Nothing> {
        Host::new("test", "", Vec::new(), Settings::new("out"))
    }

    #[test]
    fn a_protocol_version_without_sessions_is_refused() {
        let gate = gate(&options());
        let with = |version| check(&gate, &[("host", LOCAL), ("mcp-protocol-version", version)]);
        for version in [
            "2025-03-26",
            "2025-06-18",
            "2025-11-25",
            "2026-07-27",
            "latest",
        ] {
            assert_eq!(with(version), Ok(()), "{version}");
        }
        for version in ["2026-07-28", "2026-12-01", "2027-01-01"] {
            assert_eq!(with(version), Err(Refusal::Stateless), "{version}");
        }
        assert_eq!(
            check(&gate, &[("host", LOCAL)]),
            Ok(()),
            "no header is the oldest version"
        );
    }

    #[test]

    fn the_registry_stops_at_the_cap_and_frees_a_slot_when_a_session_ends() {
        let registry = Registry::new(2, Duration::from_secs(1));
        let first = registry.admit(host()).expect("room for one");
        let second = registry.admit(host()).expect("room for two");
        assert_ne!(first, second);
        assert!(registry.is_full());
        assert!(registry.admit(host()).is_none(), "over the cap");
        assert_eq!(registry.live(), 2);

        assert!(registry.release(first).is_some());
        assert!(registry.release(first).is_none(), "released once");
        assert!(!registry.is_full());
        assert!(registry.admit(host()).is_some(), "the slot is free again");
    }

    #[test]
    fn draining_takes_every_session_so_each_ends_up_closed_once() {
        let registry = Registry::new(4, Duration::from_secs(1));
        let ids: Vec<_> = (0..3).map(|_| registry.admit(host()).unwrap()).collect();
        assert_eq!(registry.drain().len(), 3);
        assert_eq!(registry.live(), 0);
        for id in ids {
            assert!(registry.release(id).is_none(), "shutdown already took it");
        }
    }
}
