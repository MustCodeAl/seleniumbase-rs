//! SeleniumBase MCP server.
//!
//! Serves one of three toolsets to an MCP client over stdio:
//!
//! - `cdp`: Chrome over the DevTools Protocol, with no WebDriver.
//! - `driver`: the `Driver()` toolset over WebDriver.
//! - `sb`: the broader `SB()` toolset over WebDriver, plus this crate's
//!   stealth tools (the default).
//!
//! ```bash
//! cargo run --bin seleniumbase-mcp --features mcp-server -- --server cdp
//! ```
//!
//! Files the tools write go to `./mcp_output`, or to the directory named by
//! the `SB_MCP_OUTPUT_DIR` environment variable.
//!
//! The server stops when the client disconnects, or on SIGINT or SIGTERM. Either
//! way it closes the browser first, waiting at most `SB_SHUTDOWN_TIMEOUT_SECS`
//! seconds (30 by default) for it to go.

use clap::Parser;
use seleniumbase_rs::mcp::{self, Profile};
use seleniumbase_rs::{init_tracing_from_runtime, RuntimeConfig};

#[derive(Debug, Parser)]
#[command(version, about = "SeleniumBase MCP server (stdio)")]
struct Cli {
    /// Which toolset to serve: cdp, driver or sb.
    #[arg(long, default_value = "sb")]
    server: Profile,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let runtime = tokio::runtime::Runtime::new()?;
    let outcome = runtime.block_on(async {
        init_tracing_from_runtime(&RuntimeConfig::from_env().unwrap_or_default());
        mcp::serve(cli.server).await
    });
    // Standard input is read on a blocking thread that only a closed pipe can
    // end. Dropping the runtime would wait for it, so a server stopped by a
    // signal would hang until its client went away; abandon the thread.
    runtime.shutdown_background();
    Ok(outcome?)
}
