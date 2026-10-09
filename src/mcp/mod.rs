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
mod tool;

pub mod cdp;

pub use host::{Closeable, Ctx, Host, Session, Settings, Started, OUTPUT_DIR_VAR};
pub use schema::{Prop, Schema};
pub use tool::{Args, Effect, Output, ToolDef, ToolError, ToolFuture};

use rmcp::serve_server;
use rmcp::transport::io::stdio;

use crate::error::SeleniumBaseError;

/// Which server to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Profile {
    /// The Pure CDP engine: no WebDriver.
    Cdp,
}

impl Profile {
    /// Every profile, in the order they are documented.
    pub const ALL: [Self; 1] = [Self::Cdp];

    /// The name used on the command line.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Cdp => "cdp",
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

/// Runs the chosen server over stdio until the client disconnects, then
/// closes the browser.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Mcp`] if the transport fails to start or
/// breaks.
pub async fn serve(profile: Profile) -> Result<(), SeleniumBaseError> {
    match profile {
        Profile::Cdp => run(cdp::host(Settings::from_env())).await,
    }
}

async fn run<S>(host: Host<S>) -> Result<(), SeleniumBaseError>
where
    S: Closeable,
{
    let failed = |error: &dyn std::fmt::Display| SeleniumBaseError::mcp(host.name(), error.to_string());
    let running = serve_server(host.clone(), stdio()).await.map_err(|e| failed(&e))?;
    let outcome = running.waiting().await;
    host.shutdown().await;
    outcome.map(drop).map_err(|e| failed(&e))
}
