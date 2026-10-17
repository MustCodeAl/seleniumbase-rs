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
//! cargo run --bin sbrs-mcp --features mcp-server -- --server cdp
//! ```
//!
//! Files the tools write go to `./mcp_output`, or to the directory named by
//! the `SB_MCP_OUTPUT_DIR` environment variable.
//!
//! The server stops when the client disconnects, or on SIGINT or SIGTERM. Either
//! way it closes the browser first, waiting at most `SB_SHUTDOWN_TIMEOUT_SECS`
//! seconds (30 by default) for it to go.

use seleniumbase_rs::mcp::{self, Profile};
use seleniumbase_rs::{init_tracing_from_runtime, RuntimeConfig};

#[derive(Debug, usage::Cli)]
// The name stays the library's, as `--version` printed it before the command line had
// a package of its own. usage prints `bin` in both `--version` and the `Usage:` line,
// so the usage line of an error names `seleniumbase-rs` rather than `sbrs-mcp`; the
// version line is the one scripts read, so it wins (`tests/mcp_version.rs`).
#[usage(
    bin = "seleniumbase-rs",
    name = "seleniumbase-rs",
    version,
    about = "SeleniumBaseRust MCP server (stdio)",
    unknown_flags = "error",
    args_override_self = false
)]
struct Cli {
    /// Which toolset to serve: cdp, driver or sb.
    #[usage(long, default = "sb")]
    server: Profile,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let runtime = tokio::runtime::Runtime::new()?;
    let outcome = runtime.block_on(async {
        // One bad SB_* variable must not drop the good ones, SB_LOG_LEVEL
        // among them; the bad ones are reported once logging works.
        let (config, problems) = RuntimeConfig::from_env_lenient();
        init_tracing_from_runtime(&config);
        for problem in &problems {
            tracing::warn!(%problem, "ignored an invalid SB_* variable, its default is used");
        }
        mcp::serve(cli.server).await
    });
    // Standard input is read on a blocking thread that only a closed pipe can
    // end. Dropping the runtime would wait for it, so a server stopped by a
    // signal would hang until its client went away; abandon the thread.
    runtime.shutdown_background();
    Ok(outcome?)
}
