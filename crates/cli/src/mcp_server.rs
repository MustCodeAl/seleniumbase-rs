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
//! Streamable HTTP, next to stdio: `--listen ADDR` (or `--transport http`) serves the same
//! toolset at `http://ADDR/mcp`, with a browser for each MCP session. The default address is
//! loopback, `127.0.0.1:8931`; the server prints one line to standard error,
//! `sbrs-mcp: listening on http://127.0.0.1:PORT/mcp (server NAME)`, once it listens. The
//! `Host` and `Origin` of every request are checked (DNS rebinding), and an address that is
//! not loopback needs a bearer token, read from `SB_MCP_TOKEN` or `--token-file FILE`, never
//! from the command line.
//!
//! ```bash
//! sbrs-mcp --server cdp --listen 127.0.0.1:0
//! SB_MCP_TOKEN=secret sbrs-mcp --server sb --listen 0.0.0.0:8931 --allow-host mcp.example.com
//! ```
//!

//! With no command the program serves. `sbrs-mcp install CLIENT...` registers it with MCP
//! clients (`uninstall` removes the registration, `clients` lists them); see
//! [`mcp_install`]. The client commands `tools`, `call`, `batch`, `resources`, `read`, `prompts`
//! and `prompt` call the tools of a server, this one or another, from the command line (see
//! [`mcp_client`]); `sbrs-mcp call --server dev find_api '{"query":"click"}'` needs no setup.

use std::net::{IpAddr, SocketAddr};
use std::path::Path;
use std::str::FromStr;

use seleniumbase_rs::cli::plugin_cmd::PluginChoice;
use seleniumbase_rs::cli::scripts::rich_helper;
use seleniumbase_rs::mcp::{self, HttpOptions, Profile, DEFAULT_LISTEN, TOKEN_VAR};
use seleniumbase_rs::{init_tracing_from_runtime, RuntimeConfig, SeleniumBaseError};

#[cfg(feature = "mcp-client")]
mod mcp_client;
mod mcp_cmd;
mod mcp_install;

// The derive of `Cli` names the type of the subcommand field by its last path segment.
use mcp_cmd::Command;

#[derive(Debug, usage::Cli)]
// usage prints `bin` in `--version`, in help and in the `Usage:` line of an error, so the
// program is named for what it is (`tests/mcp_version.rs`).
#[usage(
    bin = "sbrs-mcp",
    name = "sbrs-mcp",
    version,
    about = "SeleniumBaseRust MCP server (stdio or streamable HTTP)",
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
    /// How to serve: stdio (the default) or http (streamable HTTP)
    ///
    /// With http, each MCP session gets a browser of its own and the server prints one line
    /// to standard error, `sbrs-mcp: listening on http://ADDR/PATH (server NAME)`. `--listen`
    /// alone implies http.
    #[usage(long, value_name = "stdio|http")]
    transport: Option<Transport>,
    /// Serve over HTTP on this IP address and port (default 127.0.0.1:8931; implies --transport http)
    ///
    /// Port 0 picks a free port. Any address that is not loopback needs a token (SB_MCP_TOKEN or
    /// --token-file), and the server then logs a warning: it speaks plain HTTP, so put TLS in
    /// front of it.
    #[usage(long, value_name = "ADDR")]
    listen: Option<String>,
    /// The URL path of the HTTP endpoint (default /mcp)
    #[usage(long, value_name = "PATH")]
    path: Option<String>,
    /// Read the bearer token for HTTP clients from this file (default: SB_MCP_TOKEN)
    ///
    /// There is no flag that takes the token itself: a command line can be read by every user
    /// of the machine. The file wins over SB_MCP_TOKEN. Keep it readable by you only.
    #[usage(long, value_name = "FILE")]
    token_file: Option<String>,
    /// Let a browser page from this origin use the HTTP endpoint (repeatable)
    ///
    /// ORIGIN is scheme://host[:port]. Pages from localhost and requests that carry no Origin
    /// header (every client that is not a browser) are always let in.
    #[usage(long, value_name = "ORIGIN")]
    allow_origin: Vec<String>,
    /// Accept this name in the Host header of HTTP requests, besides localhost (repeatable)
    ///
    /// HOST is a name or an IP address, optionally followed by :PORT. Needed when the server is
    /// reached by a name or from another machine.
    #[usage(long, value_name = "HOST")]
    allow_host: Vec<String>,
    /// The most MCP sessions, and so browsers, the HTTP server holds at once (default 4)
    #[usage(long, value_name = "N")]
    max_sessions: Option<usize>,

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

/// Prints the report of `error` and ends the process with the status of its category.
fn exit_with(error: &SeleniumBaseError) -> ! {
    rich_helper::print_error_report(error);
    std::process::exit(i32::from(error.exit_code()))
}

/// How the server talks to its client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transport {
    /// Newline-delimited JSON-RPC on standard input and output.
    Stdio,
    /// MCP's streamable HTTP transport.
    Http,
}

impl FromStr for Transport {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "stdio" => Ok(Self::Stdio),
            "http" => Ok(Self::Http),
            _ => Err(format!(
                "unknown transport {name:?}; expected one of: stdio, http"
            )),
        }
    }
}

/// The HTTP options the command line asks for, or `None` to serve over stdio.
///
/// Checked now, so that a mistake stops the server before it listens and exits with
/// status 2 like every other mistake in what the operator typed.
fn http_options(cli: &Cli) -> Result<Option<HttpOptions>, SeleniumBaseError> {
    let http = match cli.transport {
        Some(Transport::Http) => true,
        Some(Transport::Stdio) => false,
        None => cli.listen.is_some(),
    };
    if !http {
        let flags_of_http = cli.listen.is_some()
            || cli.path.is_some()
            || cli.token_file.is_some()
            || !cli.allow_origin.is_empty()
            || !cli.allow_host.is_empty()
            || cli.max_sessions.is_some();
        if flags_of_http {
            return Err(SeleniumBaseError::invalid_config(
                "--listen, --path, --token-file, --allow-origin, --allow-host and --max-sessions \
                 are for the HTTP transport; add --transport http",
            ));
        }
        return Ok(None);
    }
    let mut options = HttpOptions::default();
    if let Some(listen) = &cli.listen {
        options.listen = parse_listen(listen)?;
    }
    if let Some(path) = &cli.path {
        options.path.clone_from(path);
    }
    options.allow_origins.clone_from(&cli.allow_origin);
    options.allow_hosts.clone_from(&cli.allow_host);
    if let Some(max) = cli.max_sessions {
        options.max_sessions = max;
    }
    options.token = http_token(cli.token_file.as_deref())?;
    options.validate()?;
    Ok(Some(options))
}

/// `IP:PORT`, or an IP address alone for the default port.
fn parse_listen(text: &str) -> Result<SocketAddr, SeleniumBaseError> {
    if let Ok(addr) = text.parse::<SocketAddr>() {
        return Ok(addr);
    }
    if let Ok(ip) = text.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, DEFAULT_LISTEN.port()));
    }
    Err(SeleniumBaseError::invalid_config(format!(
        "--listen {text:?} is not an IP address with a port, such as 127.0.0.1:8931 or [::1]:8931"
    )))
}

/// The bearer token: the first line of `--token-file`, else `SB_MCP_TOKEN`, else none.
fn http_token(file: Option<&str>) -> Result<Option<String>, SeleniumBaseError> {
    if let Some(file) = file {
        let text = std::fs::read_to_string(file).map_err(|error| {
            SeleniumBaseError::invalid_config(format!("cannot read --token-file {file}: {error}"))
        })?;
        warn_if_shared(file);
        let token = text.trim();
        if token.is_empty() {
            return Err(SeleniumBaseError::invalid_config(format!(
                "--token-file {file} is empty"
            )));
        }
        return Ok(Some(token.to_owned()));
    }
    match std::env::var(TOKEN_VAR) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value.trim().to_owned())),
        Ok(_) | Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(SeleniumBaseError::invalid_config(format!(
            "{TOKEN_VAR} is not valid UTF-8"
        ))),
    }
}

/// Tells the operator when other users of the machine can read the token file.
#[cfg(unix)]
fn warn_if_shared(file: &str) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(file) {
        if metadata.permissions().mode() & 0o077 != 0 {
            eprintln!("sbrs-mcp: warning: {file} can be read by other users; run chmod 600 on it");
        }
    }
}

#[cfg(not(unix))]
fn warn_if_shared(_file: &str) {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut cli = Cli::parse();
    if let Some(command) = cli.command.take() {
        std::process::exit(mcp_cmd::run(command));
    }
    let choice = plugin_choice(&cli).unwrap_or_else(|error| {
        // The same report and exit status as `sbrs`: 2 for what the operator typed.
        rich_helper::print_error_report(&error);
        std::process::exit(i32::from(error.exit_code()))
    });
    let http = http_options(&cli).unwrap_or_else(|error| exit_with(&error));
    let serving_http = http.is_some();
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
        match http {
            Some(options) => mcp::serve_http(cli.server, settings, options).await,
            None => mcp::serve_with(cli.server, settings).await,
        }
    });
    // Standard input is read on a blocking thread that only a closed pipe can
    // end. Dropping the runtime would wait for it, so a server stopped by a
    // signal would hang until its client went away; abandon the thread.
    runtime.shutdown_background();
    if serving_http {
        // A port in use or a listener that failed is reported like any other error of the
        // command line, with the status of its category.
        if let Err(error) = &outcome {
            exit_with(error);
        }
    }
    Ok(outcome?)
}
