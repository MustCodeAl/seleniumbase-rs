//! The command line of `sbrs-mcp install`, `uninstall` and `clients`.

use std::path::PathBuf;

use super::apply::Scope;
use super::entry::Only;
use super::{ClientsRequest, InstallRequest, UninstallRequest};

/// The browser toolsets `install` can register.
#[derive(Clone, Copy, Debug, usage::ValueEnum)]
pub enum BrowserServer {
    /// Chrome over the DevTools Protocol, no WebDriver (the lightest).
    Cdp,
    /// The `Driver()` toolset over WebDriver.
    Driver,
    /// The broader `SB()` toolset over WebDriver, plus the stealth tools.
    Sb,
}

impl BrowserServer {
    fn word(self) -> &'static str {
        match self {
            Self::Cdp => "cdp",
            Self::Driver => "driver",
            Self::Sb => "sb",
        }
    }
}

/// Where `install` and `uninstall` work.
#[derive(Clone, Copy, Debug, usage::ValueEnum)]
pub enum ScopeArg {
    /// The user's own config, for every project.
    User,
    /// The config file in the current folder.
    Project,
}

impl From<ScopeArg> for Scope {
    fn from(value: ScopeArg) -> Self {
        match value {
            ScopeArg::User => Self::User,
            ScopeArg::Project => Self::Project,
        }
    }
}

/// The entries `--only` can pick.
#[derive(Clone, Copy, Debug, usage::ValueEnum)]
pub enum OnlyArg {
    /// Just the browser server (NAME).
    Browser,
    /// Just the development server (NAME-dev).
    Dev,
}

fn only(arg: Option<OnlyArg>) -> Only {
    match arg {
        None => Only::Both,
        Some(OnlyArg::Browser) => Only::Browser,
        Some(OnlyArg::Dev) => Only::Dev,
    }
}

// The commands of `sbrs-mcp` besides serving: `install`, `uninstall` and `clients`. (Plain
// comments, not docs, as on the other declarations of this crate.)
#[derive(Debug, usage::Subcommands)]
pub enum Command {
    /// Register sbrs-mcp with MCP clients, so an assistant can use it.
    ///
    /// Registers two servers: the browser server as NAME (--server picks cdp, driver or sb)
    /// and the development server as NAME-dev, which starts no browser. --only registers
    /// just one. Both start this program by its absolute path, because desktop apps do not
    /// have your shell PATH, and write their files to --output-dir (SB_MCP_OUTPUT_DIR).
    ///
    /// Clients: claude-code, claude-desktop, codex, cursor, gemini-cli, opencode, vscode,
    /// zed. `sbrs-mcp clients` shows where each keeps its config. Claude Code and Codex are
    /// changed through their own `mcp` commands, the others by editing their config file:
    /// only the key that holds the servers changes, a backup FILE.bak-sbrs-mcp is written
    /// first, and a file with comments (JSONC) is never rewritten; the snippet to add by
    /// hand is printed instead.
    ///
    /// An entry of the same name that starts the same process is left as it is (exit 0). A
    /// different one is left alone and reported (exit 2) unless --force replaces it.
    /// Exit status 2 also means a refusal: an unknown client, a scope the client does not
    /// have, a file that cannot be rewritten, or a program that is not on PATH.
    Install(InstallArgs),
    /// Remove the entries `install` made (NAME and NAME-dev) from MCP clients.
    ///
    /// An entry that is not there is reported and is not an error. --only removes just one.
    Uninstall(UninstallArgs),
    /// List the MCP clients, where each keeps its config and whether it is on this machine.
    ///
    /// For each client: the user config file and the project one, whether they exist, whether
    /// the client's program (claude, codex, code) is on PATH, and which of NAME and NAME-dev
    /// are registered.
    Clients(ClientsArgs),
}

// The options of `install`. (Plain comments, not docs: a doc comment on this struct would be
// help text.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct InstallArgs {
    /// The clients to register with.
    #[usage(value_name = "CLIENT")]
    clients: Vec<String>,
    /// Which browser toolset to register: cdp, driver or sb.
    #[usage(long, value_enum, default = "cdp")]
    server: BrowserServer,
    /// The name of the browser entry; the development entry is NAME-dev.
    #[usage(long, value_name = "NAME", default = "seleniumbase")]
    name: String,
    /// The folder the tools write their files to (default: sbrs-mcp in your data folder).
    ///
    /// Registered as SB_MCP_OUTPUT_DIR and created now.
    #[usage(long, value_name = "DIR", value_hint = usage::ValueHint::AnyPath)]
    output_dir: Option<PathBuf>,
    /// Give every browser the server starts a built-in plugin (repeatable)
    ///
    /// Added to the browser server's arguments as --plugin NAME; the development server
    /// starts no browser and gets none. Checked now against the plugins this build has.
    #[usage(long, value_name = "NAME")]
    plugin: Vec<String>,
    /// Set one option of a plugin, NAME.KEY=VALUE with a TOML VALUE (repeatable)
    ///
    /// Added as --plugin-opt. Quote the whole option so the shell keeps the quotes of a
    /// string: --plugin-opt 'shield.level="strict"'.
    #[usage(long, value_name = "NAME.KEY=VALUE")]
    plugin_opt: Vec<String>,
    /// Register in your own config (user) or in the one in this folder (project).
    ///
    /// Project works for claude-code (.mcp.json), cursor (.cursor/mcp.json), vscode
    /// (.vscode/mcp.json), gemini-cli (.gemini/settings.json) and opencode (opencode.json).
    /// The other clients refuse it with exit status 2.
    #[usage(long, value_enum, default = "user")]
    scope: ScopeArg,
    /// Register just the browser server or just the development server.
    #[usage(long, value_enum)]
    only: Option<OnlyArg>,
    /// Show what would be written or run, and write and run nothing.
    #[usage(long)]
    print: bool,
    /// Replace an entry of the same name that starts something different.
    #[usage(long)]
    force: bool,
}

impl InstallArgs {
    /// The request these arguments make, for the program at `exe`.
    pub fn into_request(self, exe: PathBuf) -> InstallRequest {
        InstallRequest {
            clients: self.clients,
            exe,
            name: self.name,
            browser: self.server.word().to_owned(),
            output_dir: self.output_dir,
            plugins: self.plugin,
            plugin_opts: self.plugin_opt,
            scope: self.scope.into(),
            only: only(self.only),
            dry_run: self.print,
            force: self.force,
        }
    }
}

// The options of `uninstall`. (Plain comments, not docs.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct UninstallArgs {
    /// The clients to remove the registration from.
    #[usage(value_name = "CLIENT")]
    clients: Vec<String>,
    /// The name of the browser entry; the development entry is NAME-dev.
    #[usage(long, value_name = "NAME", default = "seleniumbase")]
    name: String,
    /// Remove from your own config (user) or from the one in this folder (project).
    #[usage(long, value_enum, default = "user")]
    scope: ScopeArg,
    /// Remove just the browser server or just the development server.
    #[usage(long, value_enum)]
    only: Option<OnlyArg>,
    /// Show what would be removed, and remove nothing.
    #[usage(long)]
    print: bool,
}

impl UninstallArgs {
    /// The request these arguments make.
    pub fn into_request(self) -> UninstallRequest {
        UninstallRequest {
            clients: self.clients,
            name: self.name,
            scope: self.scope.into(),
            only: only(self.only),
            dry_run: self.print,
        }
    }
}

// The options of `clients`. (Plain comments, not docs.)
#[derive(Debug, usage::Args)]
#[usage(args_override_self = false)]
pub struct ClientsArgs {
    /// The name of the browser entry to look for; the development entry is NAME-dev.
    #[usage(long, value_name = "NAME", default = "seleniumbase")]
    name: String,
}

impl ClientsArgs {
    /// The request these arguments make.
    pub fn into_request(self) -> ClientsRequest {
        ClientsRequest { name: self.name }
    }
}
