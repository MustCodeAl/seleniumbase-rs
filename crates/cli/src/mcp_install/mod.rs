//! `sbrs-mcp install`, `uninstall` and `clients`: register this server with the MCP clients on
//! the machine, and remove the registration again.
//!
//! One install registers two servers, which is what an assistant wants: the browser server as
//! `NAME` (default `seleniumbase`, `--server cdp|driver|sb`) and the development server as
//! `NAME-dev` (`--server dev`, no browser). `--only` registers just one of them.
//!
//! The registered command is the absolute path of the running binary, because a desktop app
//! does not have the user's shell `PATH`. Each client is changed the way that client allows:
//!
//! - Claude Code through `claude mcp add-json` and `claude mcp remove`, never by writing
//!   `~/.claude.json`, which the CLI owns.
//! - Codex through `codex mcp add` and `codex mcp remove` when the installed `codex` has them,
//!   else through the tables `[mcp_servers.NAME]` of `config.toml`, edited so that comments
//!   and other tables stay.
//! - The other clients through their JSON file: only the one key that holds the servers
//!   changes. A file with comments is not rewritten; the snippet to add by hand is printed.
//!
//! Before a file is changed it is copied to `FILE.bak-sbrs-mcp`, and the new content replaces
//! it atomically with the old permissions. Every location derives from an [`Env`], which a
//! test sets to a temporary folder.

mod apply;
mod args;
mod client;
mod entry;
mod env;
mod external;
mod fsx;
mod jsonedit;
mod jsonfile;
mod report;
mod tomlfile;

#[cfg(test)]
mod tests;

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use seleniumbase_rs::cli::plugin_cmd::check_selection;
use seleniumbase_rs::config::plugins::PluginSelection;
use seleniumbase_rs::SeleniumBaseError;

pub use args::Command;

use apply::{perform, registered, Job, Scope, Task};
use client::Client;
use entry::{names, plan, Only, PlanInput};
use env::{absolute, running_binary, Env};
use report::{exit_code, ClientReport};

/// What `install` was asked for.
#[derive(Clone, Debug)]
pub struct InstallRequest {
    /// The client names, as typed.
    pub clients: Vec<String>,
    /// The program to register: the running binary.
    pub exe: PathBuf,
    /// The name of the browser entry.
    pub name: String,
    /// The browser toolset: `cdp`, `driver` or `sb`.
    pub browser: String,
    /// The folder for `SB_MCP_OUTPUT_DIR`; a folder of the user's data folder when `None`.
    pub output_dir: Option<PathBuf>,
    /// `--plugin` values.
    pub plugins: Vec<String>,
    /// `--plugin-opt` values.
    pub plugin_opts: Vec<String>,
    /// User or project config.
    pub scope: Scope,
    /// Which entries.
    pub only: Only,
    /// Show and change nothing.
    pub dry_run: bool,
    /// Replace a different entry of the same name.
    pub force: bool,
}

/// What `uninstall` was asked for.
#[derive(Clone, Debug)]
pub struct UninstallRequest {
    /// The client names, as typed.
    pub clients: Vec<String>,
    /// The name of the browser entry.
    pub name: String,
    /// User or project config.
    pub scope: Scope,
    /// Which entries.
    pub only: Only,
    /// Show and change nothing.
    pub dry_run: bool,
}

/// What `clients` was asked for.
#[derive(Clone, Debug)]
pub struct ClientsRequest {
    /// The name of the browser entry, to look for it and for `NAME-dev`.
    pub name: String,
}

/// Runs the subcommand and returns the exit status of the program.
pub fn run(command: Command) -> i32 {
    let env = match Env::detect() {
        Ok(env) => env,
        Err(message) => {
            eprintln!("sbrs-mcp: {message}");
            return 1;
        }
    };
    let stdout = io::stdout();
    let stderr = io::stderr();
    let (mut out, mut err) = (stdout.lock(), stderr.lock());
    match command {
        Command::Install(args) => {
            let exe = match running_binary() {
                Ok(exe) => exe,
                Err(message) => {
                    let _ = writeln!(err, "sbrs-mcp: {message}");
                    return 1;
                }
            };
            install(&env, &args.into_request(exe), &mut out, &mut err)
        }
        Command::Uninstall(args) => uninstall(&env, &args.into_request(), &mut out, &mut err),
        Command::Clients(args) => clients(&env, &args.into_request(), &mut out, &mut err),
    }
}

/// The clients named on the command line, each once, or the reason to stop.
fn parse_clients(requested: &[String], err: &mut dyn Write) -> Result<Vec<Client>, i32> {
    if requested.is_empty() {
        let _ = writeln!(
            err,
            "sbrs-mcp: name at least one client; the known clients are: {}",
            Client::known_ids()
        );
        return Err(2);
    }
    let mut clients = Vec::new();
    let mut unknown = Vec::new();
    for name in requested {
        match Client::from_id(name) {
            Some(client) if clients.contains(&client) => {}
            Some(client) => clients.push(client),
            None => unknown.push(format!("`{name}`")),
        }
    }
    if !unknown.is_empty() {
        let _ = writeln!(
            err,
            "sbrs-mcp: unknown client {}; the known clients are: {}",
            unknown.join(", "),
            Client::known_ids()
        );
        return Err(2);
    }
    Ok(clients)
}

/// Whether `name` can name a server in every client: letters, digits, `-` and `_`.
fn check_name(name: &str, err: &mut dyn Write) -> Result<(), i32> {
    let fits = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if fits {
        return Ok(());
    }
    let _ = writeln!(
        err,
        "sbrs-mcp: --name `{name}` is not a name every client accepts; use letters, digits, \
         `-` and `_`"
    );
    Err(2)
}

/// Checks the plugin flags the way the server will at start, so that a mistake is reported
/// now and not by a client that cannot start the server.
fn check_plugins(plugins: &[String], options: &[String]) -> Result<(), SeleniumBaseError> {
    if plugins.is_empty() && options.is_empty() {
        return Ok(());
    }
    let mut selection = PluginSelection::from_list("");
    selection.apply_flags(plugins, options)?;
    check_selection(&selection)
}

fn print_reports(
    reports: &[ClientReport],
    dry_run: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) {
    for report in reports {
        // A closed pipe is the reader's choice; the status still says how the work went.
        let _ = report.print(dry_run, out, err);
    }
}

/// Registers the servers with the requested clients; returns the exit status: 0 when every
/// entry is added, replaced or already the same, 2 when a client was refused or a different
/// entry was left alone, 1 when something failed.
pub fn install(
    env: &Env,
    request: &InstallRequest,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let clients = match parse_clients(&request.clients, err) {
        Ok(clients) => clients,
        Err(status) => return status,
    };
    if let Err(status) = check_name(&request.name, err) {
        return status;
    }
    if request.only == Only::Dev && (!request.plugins.is_empty() || !request.plugin_opts.is_empty())
    {
        let _ = writeln!(
            err,
            "sbrs-mcp: --plugin and --plugin-opt are for the browser server, which --only dev \
             leaves out"
        );
        return 2;
    }
    if let Err(error) = check_plugins(&request.plugins, &request.plugin_opts) {
        let _ = writeln!(err, "sbrs-mcp: {error}");
        return 2;
    }
    let output_dir = absolute(
        &request
            .output_dir
            .clone()
            .unwrap_or_else(|| env.data_dir.join("sbrs-mcp")),
        &env.cwd,
    );
    let (Some(exe), Some(output)) = (request.exe.to_str(), output_dir.to_str()) else {
        let _ = writeln!(
            err,
            "sbrs-mcp: the program path and the output folder must be valid UTF-8"
        );
        return 2;
    };
    if request.dry_run {
        let _ = writeln!(err, "dry run: nothing is written or run");
    } else if let Err(e) = fs::create_dir_all(&output_dir) {
        let _ = writeln!(
            err,
            "sbrs-mcp: cannot create the output folder {}: {e}",
            output_dir.display()
        );
        return 1;
    }
    let entries = plan(&PlanInput {
        exe,
        name: &request.name,
        browser: &request.browser,
        output_dir: output,
        plugins: &request.plugins,
        plugin_opts: &request.plugin_opts,
        only: request.only,
    });
    let job = Job {
        env,
        scope: request.scope,
        dry_run: request.dry_run,
    };
    let task = Task::Install {
        entries: &entries,
        force: request.force,
    };
    let reports: Vec<ClientReport> = clients
        .iter()
        .map(|client| perform(&job, *client, &task))
        .collect();
    print_reports(&reports, request.dry_run, out, err);
    exit_code(&reports)
}

/// Removes the entries `install` made; returns the exit status as [`install`] does.
pub fn uninstall(
    env: &Env,
    request: &UninstallRequest,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    let clients = match parse_clients(&request.clients, err) {
        Ok(clients) => clients,
        Err(status) => return status,
    };
    if let Err(status) = check_name(&request.name, err) {
        return status;
    }
    if request.dry_run {
        let _ = writeln!(err, "dry run: nothing is written or run");
    }
    let names = names(&request.name, request.only);
    let job = Job {
        env,
        scope: request.scope,
        dry_run: request.dry_run,
    };
    let task = Task::Uninstall { names: &names };
    let reports: Vec<ClientReport> = clients
        .iter()
        .map(|client| perform(&job, *client, &task))
        .collect();
    print_reports(&reports, request.dry_run, out, err);
    exit_code(&reports)
}

/// Lists the clients: where each keeps its config, whether the config and the client's program
/// are on this machine, and which of the two entries are registered.
pub fn clients(
    env: &Env,
    request: &ClientsRequest,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    if let Err(status) = check_name(&request.name, err) {
        return status;
    }
    let wanted = names(&request.name, Only::Both);
    for client in Client::ALL {
        // A closed pipe is the reader's choice.
        let _ = write_client(env, client, &wanted, out);
    }
    0
}

fn found(exists: bool) -> &'static str {
    if exists {
        "found"
    } else {
        "not found"
    }
}

fn write_client(
    env: &Env,
    client: Client,
    wanted: &[String],
    out: &mut dyn Write,
) -> io::Result<()> {
    writeln!(out, "{}  {}", client.id(), client.title())?;
    match client.user_path(env) {
        Some(path) => writeln!(
            out,
            "  config      {} ({})",
            path.display(),
            found(path.exists())
        )?,
        None => writeln!(out, "  config      not supported on this system")?,
    }
    match client.project_path(env) {
        Some(path) => writeln!(
            out,
            "  project     {} ({})",
            path.display(),
            found(path.exists())
        )?,
        None => writeln!(out, "  project     none")?,
    }
    if let Some(program) = client.program() {
        match env.find_program(program) {
            Some(path) => writeln!(out, "  program     {program} (found: {})", path.display())?,
            None => writeln!(out, "  program     {program} (not found on PATH)")?,
        }
    }
    writeln!(out, "  writes      {}", client.how())?;
    let mut places = Vec::new();
    for (scope, word) in [(Scope::User, "user"), (Scope::Project, "project")] {
        let present = match registered(env, client, scope) {
            Ok(existing) => {
                let mut present: Vec<&str> = wanted
                    .iter()
                    .filter(|name| existing.contains_key(*name))
                    .map(String::as_str)
                    .collect();
                if present.is_empty() {
                    present.push("none");
                }
                present.join(", ")
            }
            Err(message) => format!("unreadable ({message})"),
        };
        let has_location = match scope {
            Scope::User => client.user_path(env).is_some(),
            Scope::Project => client.project_path(env).is_some(),
        };
        if has_location {
            places.push(format!("{word}: {present}"));
        }
    }
    writeln!(out, "  registered  {}", places.join("; "))?;
    writeln!(out, "  docs        {}", client.docs())
}
