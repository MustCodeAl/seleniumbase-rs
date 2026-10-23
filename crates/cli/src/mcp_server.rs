//! SeleniumBase MCP server.
//!
//! Serves one of four toolsets to an MCP client over stdio:
//!
//! - `cdp`: Chrome over the DevTools Protocol, with no WebDriver.
//! - `driver`: the `Driver()` toolset over WebDriver.
//! - `sb`: the broader `SB()` toolset over WebDriver, plus this crate's
//!   stealth tools (the default).
//! - `dev`: development, library, command-line and diagnostics tools; it starts
//!   no browser, so it runs next to one of the others.
//!
//! ```bash
//! cargo run --bin sbrs-mcp --features mcp-server -- --server cdp
//! ```
//!
//! Files the tools write go to `./mcp_output`, or to the directory named by
//! the `SB_MCP_OUTPUT_DIR` environment variable.
//!
//! The operator, not the model, chooses the built-in plugins every browser the
//! server starts gets: `--plugin NAME` (repeatable) and
//! `--plugin-opt NAME.KEY=VALUE`, added to the `[plugins]` table of the settings
//! file and `SB_PLUGINS`. A mistake in them stops the server before it listens.
//!
//! The server stops when the client disconnects, or on SIGINT or SIGTERM. Either
//! way it closes the browser first, waiting at most `SB_SHUTDOWN_TIMEOUT_SECS`
//! seconds (30 by default) for it to go.
//!
//! With no command the program serves. `sbrs-mcp install CLIENT...` registers it with MCP
//! clients (`uninstall` removes the registration, `clients` lists them); see
//! [`mcp_install`].

use std::path::Path;

use seleniumbase_rs::cli::plugin_cmd::PluginChoice;
use seleniumbase_rs::cli::scripts::rich_helper;
use seleniumbase_rs::mcp::{self, Profile};
use seleniumbase_rs::{init_tracing_from_runtime, RuntimeConfig, SeleniumBaseError};

mod mcp_install;

// The derive of `Cli` names the type of the subcommand field by its last path segment.
use mcp_install::Command;

#[derive(Debug, usage::Cli)]
// usage prints `bin` in `--version`, in help and in the `Usage:` line of an error, so the
// program is named for what it is (`tests/mcp_version.rs`).
#[usage(
    bin = "sbrs-mcp",
    name = "sbrs-mcp",
    version,
    about = "SeleniumBaseRust MCP server (stdio)",
    unknown_flags = "error",
    args_override_self = false,
    // `sbrs-mcp --server cdp install cursor` would drop the flag without a word.
    args_conflicts_with_subcommands
)]
struct Cli {
    /// Which toolset to serve: cdp, driver, sb or dev (development tools, no browser).
    #[usage(long, default = "sb")]
    server: Profile,
    /// Give every browser the server starts a built-in plugin (repeatable)
    ///
    /// The operator chooses; no tool argument adds a plugin. Adds to the plugins the
    /// [plugins] table of the settings file (sbase_config.toml in the folder the server runs
    /// in) and SB_PLUGINS turn on. Precedence, highest first: the command line, then
    /// SB_PLUGINS (which replaces the names the file enables), then the settings file.
    /// `sbrs plugin list` shows the plugins this build has.
    #[usage(long, value_name = "NAME")]
    plugin: Vec<String>,
    /// Set one option of a plugin, NAME.KEY=VALUE with a TOML VALUE (repeatable)
    ///
    /// Quote the whole option so the shell keeps the quotes of a string:
    /// --plugin-opt 'shield.level="strict"'. An option given here replaces the same option
    /// from SB_PLUGINS or the settings file and keeps the plugin's other options. The plugin
    /// must be turned on by --plugin, SB_PLUGINS or the file.
    #[usage(long, value_name = "NAME.KEY=VALUE")]
    plugin_opt: Vec<String>,
    #[usage(subcommand)]
    command: Option<Command>,
}

/// The plugins the operator chose: the `[plugins]` table of the settings file (only that
/// table is read) and `SB_PLUGINS`, with `--plugin` and `--plugin-opt` on top, checked now so
/// that a mistake stops the server before it listens, not at the first `start_browser`.
fn plugin_choice(cli: &Cli) -> Result<PluginChoice, SeleniumBaseError> {
    let choice = PluginChoice::load(None::<&Path>, &cli.plugin, &cli.plugin_opt)?;
    choice.check()?;
    Ok(choice)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cli = Cli::parse();
    if let Some(command) = cli.command.take() {
        std::process::exit(mcp_install::run(command));
    }
    let choice = plugin_choice(&cli).unwrap_or_else(|error| {
        // The same report and exit status as `sbrs`: 2 for what the operator typed.
        rich_helper::print_error_report(&error);
        std::process::exit(i32::from(error.exit_code()))
    });
    let settings = mcp::Settings::from_env().with_plugin_selection(choice.selection().clone());
    let runtime = tokio::runtime::Runtime::new()?;
    let outcome = runtime.block_on(async {
        // One bad SB_* variable must not drop the good ones, SB_LOG_LEVEL
        // among them; the bad ones are reported once logging works.
        let (config, problems) = RuntimeConfig::from_env_lenient();
        init_tracing_from_runtime(&config);
        for problem in &problems {
            tracing::warn!(%problem, "ignored an invalid SB_* variable, its default is used");
        }
        choice.log();
        mcp::serve_with(cli.server, settings).await
    });
    // Standard input is read on a blocking thread that only a closed pipe can
    // end. Dropping the runtime would wait for it, so a server stopped by a
    // signal would hang until its client went away; abandon the thread.
    runtime.shutdown_background();
    Ok(outcome?)
}
