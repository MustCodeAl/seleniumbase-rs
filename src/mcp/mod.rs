//! Model Context Protocol servers for browser automation and for developing
//! with this library.
//!
//! Three servers expose SeleniumBase to an MCP client, each over one shared
//! browser session:
//!
//! - [`cdp`]: the Pure CDP engine ([`sb_cdp`](crate::sb_cdp)), which drives
//!   Chrome directly without a WebDriver.
//! - [`driver`]: the `Driver()` toolset over WebDriver.
//! - [`sb`]: the broader `SB()` toolset over WebDriver, with the stealth tools.
//!
//! A fourth, [`dev`], starts no browser. It helps with the library, the `sbrs`
//! command line and with diagnosing problems; load it next to one of the
//! three.
//!
//! A server is a [`Host`]: a list of [`ToolDef`]s plus the session they share.
//! Tool failures are returned to the model as error results it can act on,
//! not as protocol errors: a [`FailureReport`] says what failed, what was
//! tried, what the page looked like, and which tools to try next.
//!
//! ```no_run
//! # async fn run() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! use seleniumbase_rs::mcp;
//!
//! mcp::serve(mcp::Profile::Cdp).await?;
//! # Ok(())
//! # }
//! ```

mod devtools;
#[cfg(feature = "plugin-imap")]
mod email;
mod export;
mod failure;
mod fingerprint;

mod host;
mod http;
mod journal;
mod pagetools;

mod progress;
mod prompts;
mod refs;
mod resources;
mod schema;
mod support;
mod tool;
mod webdriver;

pub mod cdp;
#[cfg(feature = "mcp-client")]
pub mod client;

pub mod dev;
pub mod driver;
pub mod sb;
mod stealth;

pub use failure::FailureReport;
pub use host::{
    Closeable, Ctx, Host, Session, Settings, Started, OUTPUT_DIR_VAR, UNTRUSTED_META_KEY,
    UNTRUSTED_NOTICE,
};
pub use http::{
    generate_token, serve_http, HttpOptions, HttpServer, DEFAULT_IDLE, DEFAULT_LISTEN,
    DEFAULT_MAX_SESSIONS, DEFAULT_PATH, TOKEN_VAR,
};
pub use progress::{Progress, ProgressSink, ProgressUpdate};
pub use prompts::{PromptArg, PromptDef, Render};
pub use resources::{
    ResourceBody, ResourceDef, ResourceError, ResourceFuture, JOURNAL_URI, MAX_OUTPUT_BYTES,
    OUTPUT_TEMPLATE, PAGE_SNAPSHOT_URI, PAGE_TEXT_URI, SESSION_URI,
};
pub use schema::{Prop, Schema};
pub use tool::{Args, Effect, Output, ToolDef, ToolError, ToolFuture};

use std::time::Duration;

use rmcp::serve_server;
use rmcp::service::{QuitReason, RoleServer};
use rmcp::transport::io::stdio;
use rmcp::transport::IntoTransport;
use tracing::{info, warn};

use crate::common::shutdown::{
    cleanup_within, drain_cleanups, timeout_from_env, Outcome, Shutdown,
};
use crate::error::SeleniumBaseError;

/// Which server to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Profile {
    /// The Pure CDP engine: no WebDriver.
    Cdp,
    /// WebDriver with the `Driver()` toolset.
    Driver,
    /// WebDriver with the broader `SB()` toolset and the stealth tools.
    Sb,
    /// No browser: tools for developing with the library, using `sbrs` and
    /// diagnosing problems. Load it next to one of the others.
    Dev,
}

impl Profile {
    /// Every profile, in the order they are documented.
    pub const ALL: [Self; 4] = [Self::Cdp, Self::Driver, Self::Sb, Self::Dev];

    /// The name used on the command line.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Cdp => "cdp",
            Self::Driver => "driver",
            Self::Sb => "sb",
            Self::Dev => "dev",
        }
    }
}

impl std::str::FromStr for Profile {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.name() == name)
            .ok_or_else(|| {
                let known: Vec<_> = Self::ALL.iter().map(|profile| profile.name()).collect();
                format!(
                    "unknown server {name:?}; expected one of: {}",
                    known.join(", ")
                )
            })
    }
}

/// Runs the chosen server over stdio until the client disconnects or the
/// process is sent SIGINT or SIGTERM, then closes the browser.
///
/// A signal ends the server cleanly, with `Ok(())`: the client's session is
/// closed, the browser is closed within the grace period (see
/// [`timeout_from_env`]), and the caller can exit.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Mcp`] if the transport fails to start or
/// breaks, and [`SeleniumBaseError::Unsupported`] if the signal handlers
/// cannot be installed.
pub async fn serve(profile: Profile) -> Result<(), SeleniumBaseError> {
    serve_with(profile, Settings::from_env()).await
}

/// [`serve`] with `settings` instead of the ones the environment gives, such
/// as the plugins the operator chose
/// ([`Settings::with_plugin_selection`]).
///
/// # Errors
///
/// As [`serve`].
pub async fn serve_with(profile: Profile, settings: Settings) -> Result<(), SeleniumBaseError> {
    let result = match profile {
        Profile::Cdp => run(cdp::host(settings)).await,
        Profile::Driver => run(driver::host(settings)).await,
        Profile::Sb => run(sb::host(settings)).await,
        Profile::Dev => run(dev::host(settings)).await,
    };
    // A `Drop` somewhere may have started its own cleanup; let it finish.
    drain_cleanups(timeout_from_env()).await;
    result
}

async fn run<S>(host: Host<S>) -> Result<(), SeleniumBaseError>
where
    S: Closeable,
{
    // Installed before anything starts: from here a SIGTERM no longer ends the
    // process by itself, so every wait below also listens for it.
    let shutdown = Shutdown::install()?;
    // The server's state machine is large; keep it off the caller's stack.
    Box::pin(run_on(host, stdio(), shutdown, timeout_from_env())).await
}

/// Serves `host` over `transport` until the client disconnects or `shutdown`
/// fires, then closes the browser, giving it `grace` to do so.
async fn run_on<S, T, E, A>(
    host: Host<S>,
    transport: T,
    mut shutdown: Shutdown,
    grace: Duration,
) -> Result<(), SeleniumBaseError>
where
    S: Closeable,
    T: IntoTransport<RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let failed =
        |error: &dyn std::fmt::Display| SeleniumBaseError::mcp(host.name(), error.to_string());

    let running = match shutdown.race(serve_server(host.clone(), transport)).await {
        Outcome::Completed(started) => started.map_err(|e| failed(&e))?,
        Outcome::Interrupted(signal) => {
            info!(%signal, server = host.name(), "stopping before a client connected");
            close_browser(&host, grace).await;
            return Ok(());
        }
    };

    let cancel = running.cancellation_token();
    let waiting = running.waiting();
    tokio::pin!(waiting);
    let outcome = match shutdown.race(&mut waiting).await {
        Outcome::Completed(outcome) => outcome,
        Outcome::Interrupted(signal) => {
            info!(%signal, server = host.name(), "stopping the server");
            cancel.cancel();
            // Cancelling ends the service loop; the bound is for a loop that
            // is stuck behind a tool call.
            if let Ok(outcome) = tokio::time::timeout(grace, &mut waiting).await {
                outcome
            } else {
                warn!(server = host.name(), "the server did not stop in time");
                Ok(QuitReason::Cancelled)
            }
        }
    };
    close_browser(&host, grace).await;
    outcome.map(drop).map_err(|e| failed(&e))
}

/// Closes the shared browser, if there is one, but never takes longer than
/// `grace`: a tool call that holds the session must not keep the process up.
async fn close_browser<S: Closeable>(host: &Host<S>, grace: Duration) {
    cleanup_within(grace, host.shutdown()).await;
}

#[cfg(test)]
mod tests {
    use std::future::pending;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

    use super::*;
    use crate::common::shutdown::Signal;

    /// A browser stand-in that notes when it was closed.
    #[derive(Debug)]
    struct Browser {
        closed: Arc<AtomicBool>,
    }

    impl Closeable for Browser {
        async fn close(self) {
            self.closed.store(true, Ordering::SeqCst);
        }
    }

    /// A browser that never finishes closing.
    #[derive(Debug)]
    struct StuckBrowser;

    impl Closeable for StuckBrowser {
        async fn close(self) {
            pending::<()>().await;
        }
    }

    async fn host_with<S: Closeable>(session: S) -> Host<S> {
        let host = Host::new(
            "test",
            "a server for tests",
            Vec::new(),
            Settings::new("out"),
        );
        host.ctx()
            .start(async { Ok(session) })
            .await
            .expect("seed the session");
        host
    }

    async fn send(client: &mut DuplexStream, line: &str) {
        client.write_all(line.as_bytes()).await.unwrap();
        client.write_all(b"\n").await.unwrap();
    }

    /// Reads from the server until it has sent a full line.
    async fn read_line(client: &mut DuplexStream) -> String {
        let mut line = Vec::new();
        let mut byte = [0_u8; 1];
        loop {
            client.read_exact(&mut byte).await.unwrap();
            if byte[0] == b'\n' {
                return String::from_utf8(line).unwrap();
            }
            line.push(byte[0]);
        }
    }

    /// Performs the MCP `initialize` handshake as a client.
    async fn handshake(client: &mut DuplexStream) {
        send(
            client,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        )
        .await;
        let reply = read_line(client).await;
        assert!(reply.contains("\"result\""), "unexpected reply: {reply}");
        send(
            client,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )
        .await;
    }

    #[tokio::test]
    async fn a_signal_while_a_client_is_connected_closes_the_browser() {
        let closed = Arc::new(AtomicBool::new(false));
        let host = host_with(Browser {
            closed: Arc::clone(&closed),
        })
        .await;
        let (server_end, mut client) = tokio::io::duplex(8192);
        let (shutdown, trigger) = Shutdown::manual();

        let (result, ()) = tokio::join!(
            run_on(host, server_end, shutdown, Duration::from_secs(10)),
            async {
                handshake(&mut client).await;
                trigger.fire(Signal::Terminate);
            }
        );

        assert!(result.is_ok(), "{result:?}");
        assert!(closed.load(Ordering::SeqCst), "the browser was left open");
    }

    #[tokio::test]
    async fn a_signal_before_any_client_connects_still_closes_the_browser() {
        let closed = Arc::new(AtomicBool::new(false));
        let host = host_with(Browser {
            closed: Arc::clone(&closed),
        })
        .await;
        let (server_end, _client) = tokio::io::duplex(8192);
        let (shutdown, trigger) = Shutdown::manual();
        trigger.fire(Signal::Interrupt);

        let result = run_on(host, server_end, shutdown, Duration::from_secs(10)).await;

        assert!(result.is_ok(), "{result:?}");
        assert!(closed.load(Ordering::SeqCst), "the browser was left open");
    }

    #[tokio::test]
    async fn a_client_that_disconnects_still_closes_the_browser() {
        let closed = Arc::new(AtomicBool::new(false));
        let host = host_with(Browser {
            closed: Arc::clone(&closed),
        })
        .await;
        let (server_end, mut client) = tokio::io::duplex(8192);
        // Keep the trigger alive so only the disconnect can end the server.
        let (shutdown, _trigger) = Shutdown::manual();

        let (result, ()) = tokio::join!(
            run_on(host, server_end, shutdown, Duration::from_secs(10)),
            async {
                handshake(&mut client).await;
                drop(client);
            }
        );

        assert!(result.is_ok(), "{result:?}");
        assert!(closed.load(Ordering::SeqCst), "the browser was left open");
    }

    #[tokio::test(start_paused = true)]
    async fn a_browser_that_will_not_close_does_not_hold_up_shutdown() {
        let host = host_with(StuckBrowser).await;
        let (server_end, _client) = tokio::io::duplex(8192);
        let (shutdown, trigger) = Shutdown::manual();
        trigger.fire(Signal::Terminate);
        let started = tokio::time::Instant::now();

        let result = run_on(host, server_end, shutdown, Duration::from_secs(7)).await;

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(started.elapsed(), Duration::from_secs(7));
    }

    /// The `dev` server over the protocol, as `serve` runs it, with nothing to
    /// close: a client lists its tools, calls one and goes away.
    #[tokio::test]
    async fn the_dev_server_lists_its_tools_and_answers_a_call_over_the_protocol() {
        let host = dev::host(Settings::new("out"));
        let expected: Vec<&str> = host.tool_names();
        let (server_end, mut client) = tokio::io::duplex(1 << 20);
        let (shutdown, _trigger) = Shutdown::manual();

        let (result, ()) = tokio::join!(
            run_on(host, server_end, shutdown, Duration::from_secs(10)),
            async {
                handshake(&mut client).await;
                send(
                    &mut client,
                    r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
                )
                .await;
                let listed: serde_json::Value =
                    serde_json::from_str(&read_line(&mut client).await).unwrap();
                let names: Vec<&str> = listed["result"]["tools"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|tool| tool["name"].as_str().unwrap())
                    .collect();
                assert_eq!(names, expected);
                assert_eq!(names.len(), 12);

                send(
                    &mut client,
                    r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"find_api","arguments":{"query":"save_screenshot","limit":2}}}"#,
                )
                .await;
                let called: serde_json::Value =
                    serde_json::from_str(&read_line(&mut client).await).unwrap();
                assert_ne!(
                    called["result"]["isError"],
                    serde_json::json!(true),
                    "{called}"
                );
                let text = called["result"]["content"][0]["text"].as_str().unwrap();
                assert!(text.contains("\"name\": \"save_screenshot\""), "{text}");

                send(
                    &mut client,
                    r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"click_element","arguments":{}}}"#,
                )
                .await;
                let missing: serde_json::Value =
                    serde_json::from_str(&read_line(&mut client).await).unwrap();
                assert!(missing["error"]["message"]
                    .as_str()
                    .is_some_and(|message| message.starts_with("unknown tool: click_element")));
                drop(client);
            }
        );

        assert!(result.is_ok(), "{result:?}");
    }
}
