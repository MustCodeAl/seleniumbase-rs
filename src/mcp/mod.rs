//! Model Context Protocol servers for browser automation.
//!
//! Three servers expose SeleniumBase to an MCP client, each over one shared
//! browser session:
//!
//! - [`cdp`]: the Pure CDP engine ([`sb_cdp`](crate::sb_cdp)), which drives
//!   Chrome directly without a WebDriver.
//!
//! A server is a [`Host`]: a list of [`ToolDef`]s plus the session they share.
//! Tool failures are returned to the model as error results it can act on,
//! not as protocol errors.
//!
//! ```no_run
//! # async fn run() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! use seleniumbase_rs::mcp;
//!
//! mcp::serve(mcp::Profile::Cdp).await?;
//! # Ok(())
//! # }
//! ```

mod host;
mod schema;
mod support;
mod tool;
mod webdriver;

pub mod cdp;
pub mod driver;
pub mod sb;
mod stealth;

pub use host::{Closeable, Ctx, Host, Session, Settings, Started, OUTPUT_DIR_VAR};
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
}

impl Profile {
    /// Every profile, in the order they are documented.
    pub const ALL: [Self; 3] = [Self::Cdp, Self::Driver, Self::Sb];

    /// The name used on the command line.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Cdp => "cdp",
            Self::Driver => "driver",
            Self::Sb => "sb",
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
    let result = match profile {
        Profile::Cdp => run(cdp::host(Settings::from_env())).await,
        Profile::Driver => run(driver::host(Settings::from_env())).await,
        Profile::Sb => run(sb::host(Settings::from_env())).await,
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
}
