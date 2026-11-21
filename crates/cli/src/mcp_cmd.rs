//! The commands of `sbrs-mcp` besides serving: `install`, `uninstall`, `clients`, `repair` and `upgrade` (see
//! [`mcp_install`]) and, with the `mcp-client` feature, the client commands `tools`, `call`,
//! `resources`, `read`, `prompts`, `prompt` and `batch` (see [`mcp_client`]).

#[cfg(feature = "mcp-client")]
use crate::mcp_client::{self, BatchArgs, CallArgs, ListArgs, PromptArgs, ReadArgs, ToolsArgs};
use crate::mcp_install::{
    self, Action, ClientsArgs, InstallArgs, RepairArgs, UninstallArgs, UpgradeArgs,
};

// The commands of `sbrs-mcp` besides serving. (Plain comments, not docs, as on the other
// declarations of this crate.)
#[derive(Debug, usage::Subcommands)]
pub enum Command {
    /// Register sbrs-mcp with MCP clients, so an assistant can use it.
    ///
    /// Registers two servers: the browser server as NAME (--server picks cdp, driver or sb)
    /// and the development server as NAME-dev, which starts no browser. --only registers
    /// just one. Both start this program by its absolute path, because desktop apps do not
    /// have your shell PATH, and write their files to --output-dir (SB_MCP_OUTPUT_DIR).
    ///
    /// Clients: claude-code, claude-desktop, codex, cursor, gemini-cli, grok, opencode, vscode,
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
    /// the client's program (claude, codex, grok, code) is on PATH, and which of NAME and NAME-dev
    /// are registered.
    Clients(ClientsArgs),
    /// Find what is broken in the entries MCP clients hold for sbrs-mcp, and fix it.
    ///
    /// Looks at every client that has a config here (its user config, and the project config in
    /// the current folder) for the entries that start sbrs-mcp: NAME and NAME-dev, and any other
    /// name that starts a program called sbrs-mcp. It finds a program that is not there or is not
    /// executable, a path that is another sbrs-mcp than this one, a relative name a desktop app
    /// may not find, the same server registered twice, and a name other than NAME and NAME-dev
    /// (reported, left as it is). The fix points an entry at this program, keeping its arguments
    /// and environment, and removes the duplicate. Every finding and every fix is printed for
    /// each client.
    ///
    /// A config that cannot be read (JSON or TOML that does not parse, or JSON with comments) is
    /// reported and never rewritten. The rest of a config keeps its bytes, and the old file is
    /// kept as FILE.bak-sbrs-mcp, as `install` does. --dry-run writes and runs nothing.
    ///
    /// Exit status: 0 when nothing is left broken, 1 when a fix failed or --dry-run found
    /// something to fix, 2 when something cannot be fixed by this command (an unreadable config,
    /// a client program that is not on PATH) or a client is unknown.
    Repair(RepairArgs),
    /// Rewrite the entries to this program, then say how to upgrade the program itself.
    ///
    /// Every entry NAME and NAME-dev that a client holds is rewritten to what `install` writes
    /// today for this program: its absolute path (for Homebrew the link that stays after `brew
    /// upgrade`), and its server, plugin options and output folder as they were. An entry that
    /// does not start a local program is left alone. Then it finds out how this program was
    /// installed and prints the command that upgrades it: `brew upgrade sbrs` for Homebrew, `cargo
    /// install --git ... --tag <latest tag> seleniumbase-rs-cli --locked` for `cargo install`
    /// (the tag is looked up with `git ls-remote`), and the page of the releases for a release
    /// archive or anything else. The command runs only with --run. --dry-run writes and runs
    /// nothing.
    ///
    /// Exit status as `repair`: 0 when it worked, 1 when a write or the command failed, 2 when
    /// something was refused or there is nothing to run.
    #[usage(alias = "update")]
    Upgrade(UpgradeArgs),
    /// List the tools of an MCP server, or show one with the schema of its arguments.
    ///
    /// The server is --url (streamable HTTP, token in SB_MCP_TOKEN or --token-file),
    /// --connect PROGRAM (any server that speaks stdio) or --server cdp|driver|sb|dev (this
    /// program, started as that server); with none of them it is SB_MCP_URL, else the dev
    /// server, so `sbrs-mcp tools` works with no setup. Exit status: 0 worked, 1 the tool
    /// reported a failure, 2 a mistake in what you typed (an unknown tool, a bad argument),
    /// 3 the server could not be reached, refused the token or broke the protocol.
    #[cfg(feature = "mcp-client")]
    Tools(ToolsArgs),
    /// Call a tool of an MCP server and print what it returns.
    ///
    /// The arguments are a JSON object (sbrs-mcp call open_url '{"url":"https://x.test"}',
    /// or `-` for standard input), --json, or --arg KEY=VALUE (repeatable). The text goes to
    /// standard output; --raw prints the structured result as compact JSON; --out DIR saves
    /// images; --progress prints progress to standard error. The server is chosen as for
    /// `tools`. A command is its own MCP session: a browser a server starts closes when the
    /// command ends, so use `batch` to run several calls in one session.
    #[cfg(feature = "mcp-client")]
    Call(CallArgs),
    /// Run several tool calls in one MCP session, so a browser one call starts is there for
    /// the next.
    ///
    /// Reads one JSON object per line from FILE or standard input, such as
    /// {"tool":"open_url","arguments":{"url":"https://x.test"}}; blank lines and lines that
    /// start with # are skipped, and a bad line stops the command before anything runs. The
    /// calls run in order and stop at the first failure unless --keep-going. The exit
    /// status is that of the first failure.
    #[cfg(feature = "mcp-client")]
    Batch(BatchArgs),
    /// List the resources of an MCP server (read-only data such as the call journal).
    #[cfg(feature = "mcp-client")]
    Resources(ListArgs),
    /// Read a resource of an MCP server by its URI, such as sbrs://journal.
    #[cfg(feature = "mcp-client")]
    Read(ReadArgs),
    /// List the prompts of an MCP server.
    ///
    /// A `*` marks an argument that is required.
    #[cfg(feature = "mcp-client")]
    Prompts(ListArgs),
    /// Render a prompt of an MCP server with its arguments (--arg KEY=VALUE, always text).
    #[cfg(feature = "mcp-client")]
    Prompt(PromptArgs),
}

/// Runs the command and returns the exit status of the program.
pub fn run(command: Command) -> i32 {
    match command {
        Command::Install(args) => mcp_install::run(Action::Install(args)),
        Command::Uninstall(args) => mcp_install::run(Action::Uninstall(args)),
        Command::Clients(args) => mcp_install::run(Action::Clients(args)),
        Command::Repair(args) => mcp_install::run(Action::Repair(args)),
        Command::Upgrade(args) => mcp_install::run(Action::Upgrade(args)),
        #[cfg(feature = "mcp-client")]
        Command::Tools(args) => mcp_client::tools(args),
        #[cfg(feature = "mcp-client")]
        Command::Call(args) => mcp_client::call(args),
        #[cfg(feature = "mcp-client")]
        Command::Batch(args) => mcp_client::batch(args),
        #[cfg(feature = "mcp-client")]
        Command::Resources(args) => mcp_client::resources(args),
        #[cfg(feature = "mcp-client")]
        Command::Read(args) => mcp_client::read(args),
        #[cfg(feature = "mcp-client")]
        Command::Prompts(args) => mcp_client::prompts(args),
        #[cfg(feature = "mcp-client")]
        Command::Prompt(args) => mcp_client::prompt(args),
    }
}
