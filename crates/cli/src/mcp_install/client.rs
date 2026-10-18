//! The MCP clients `sbrs-mcp install` knows, and where each keeps its config.
//!
//! Every location and format here was checked against the client's own documentation; the
//! page is named beside it. A client whose format could not be confirmed is left out:
//!
//! - Windsurf: its documentation moved to Devin Desktop, which lists a different file
//!   (`~/.config/devin/mcp_config.json`) and does not mention `~/.codeium/windsurf`.
//! - LM Studio: its documentation says to open the file from the app (Program, Install, Edit
//!   mcp.json) and never gives its path.

use std::path::PathBuf;

use super::env::{Env, Os};
use super::jsonfile::{Layout, LayoutKind};

/// A client `sbrs-mcp install` can register with.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Client {
    /// Claude Code, through its `claude mcp` commands.
    ClaudeCode,
    /// The Claude desktop app.
    ClaudeDesktop,
    /// The OpenAI Codex CLI.
    Codex,
    /// Cursor.
    Cursor,
    /// The Gemini CLI.
    GeminiCli,
    /// opencode.
    Opencode,
    /// Visual Studio Code.
    Vscode,
    /// Zed.
    Zed,
}

/// How a client's config is edited.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    /// A JSON file in the given layout.
    Json,
    /// Claude Code: the `claude mcp` commands write, the JSON files are only read.
    ClaudeCli,
    /// Codex: `codex mcp` when the program has it, else `config.toml`.
    Codex,
}

impl Client {
    /// Every client, in the order `clients` lists them.
    pub const ALL: [Self; 8] = [
        Self::ClaudeCode,
        Self::ClaudeDesktop,
        Self::Codex,
        Self::Cursor,
        Self::GeminiCli,
        Self::Opencode,
        Self::Vscode,
        Self::Zed,
    ];

    /// The name used on the command line.
    pub const fn id(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::ClaudeDesktop => "claude-desktop",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
            Self::GeminiCli => "gemini-cli",
            Self::Opencode => "opencode",
            Self::Vscode => "vscode",
            Self::Zed => "zed",
        }
    }

    /// The name people know it by.
    pub const fn title(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::ClaudeDesktop => "Claude Desktop",
            Self::Codex => "OpenAI Codex CLI",
            Self::Cursor => "Cursor",
            Self::GeminiCli => "Gemini CLI",
            Self::Opencode => "opencode",
            Self::Vscode => "Visual Studio Code",
            Self::Zed => "Zed",
        }
    }

    /// The client called `id`, if there is one.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|client| client.id() == id)
    }

    /// The names of all the clients, for a message.
    pub fn known_ids() -> String {
        Self::ALL
            .iter()
            .map(|client| client.id())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The pages this client's paths and format were checked against.
    pub const fn docs(self) -> &'static str {
        match self {
            Self::ClaudeCode => "https://code.claude.com/docs/en/mcp",
            Self::ClaudeDesktop => "https://modelcontextprotocol.io/quickstart/user",
            Self::Codex => "https://developers.openai.com/codex/mcp",
            Self::Cursor => "https://cursor.com/docs/context/mcp",
            Self::GeminiCli => {
                "https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md"
            }
            Self::Opencode => {
                "https://opencode.ai/docs/mcp-servers/ and https://opencode.ai/docs/config/"
            }
            Self::Vscode => {
                "https://code.visualstudio.com/docs/copilot/customization/mcp-servers and \
                 https://code.visualstudio.com/docs/configure/profiles"
            }
            Self::Zed => {
                "https://zed.dev/docs/ai/mcp and \
                 https://github.com/zed-industries/zed/blob/main/docs/src/configuring-zed.md"
            }
        }
    }

    /// How this client's config is written.
    pub const fn kind(self) -> Kind {
        match self {
            Self::ClaudeCode => Kind::ClaudeCli,
            Self::Codex => Kind::Codex,
            _ => Kind::Json,
        }
    }

    /// One line on how `install` changes this client.
    pub const fn how(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude mcp add-json / claude mcp remove",
            Self::Codex => "codex mcp add / remove, else config.toml (table mcp_servers)",
            Self::ClaudeDesktop | Self::Cursor | Self::GeminiCli => "JSON, key mcpServers",
            Self::Opencode => "JSON, key mcp (type local)",
            Self::Vscode => "JSON, key servers (type stdio)",
            Self::Zed => "JSON, key context_servers",
        }
    }

    /// The program that can register a server (`claude`, `codex`) or help with it (`code`).
    pub const fn program(self) -> Option<&'static str> {
        match self {
            Self::ClaudeCode => Some("claude"),
            Self::Codex => Some("codex"),
            Self::Vscode => Some("code"),
            _ => None,
        }
    }

    /// Why `--scope project` is refused, for a client without a project config.
    ///
    /// Zed is one of them: its documentation does not say that `.zed/settings.json` may hold
    /// `context_servers`, so nothing is written there.
    pub const fn no_project_reason(self) -> &'static str {
        match self {
            Self::Zed => {
                "Zed's documentation does not say that .zed/settings.json can hold context_servers"
            }
            Self::Codex => "sbrs-mcp registers with Codex in its user config only",
            _ => "it keeps no project config",
        }
    }

    /// The layout of the JSON this client reads, for the clients with a JSON file.
    pub const fn layout(self) -> Option<Layout> {
        match self {
            Self::ClaudeDesktop | Self::GeminiCli => Some(Layout {
                key: "mcpServers",
                kind: LayoutKind::Plain { typed: false },
            }),
            Self::ClaudeCode | Self::Cursor => Some(Layout {
                key: "mcpServers",
                kind: LayoutKind::Plain { typed: true },
            }),
            Self::Vscode => Some(Layout {
                key: "servers",
                kind: LayoutKind::Plain { typed: true },
            }),
            Self::Zed => Some(Layout {
                key: "context_servers",
                kind: LayoutKind::Plain { typed: false },
            }),
            Self::Opencode => Some(Layout {
                key: "mcp",
                kind: LayoutKind::Opencode,
            }),
            Self::Codex => None,
        }
    }

    /// The user-level config file, or `None` where the client has none.
    pub fn user_path(self, env: &Env) -> Option<PathBuf> {
        Some(match self {
            Self::ClaudeCode => env.home.join(".claude.json"),
            // Documented for macOS and Windows only; Linux builds are not covered.
            Self::ClaudeDesktop => match env.os {
                Os::Mac | Os::Windows => env
                    .config_dir
                    .join("Claude")
                    .join("claude_desktop_config.json"),
                Os::Linux => return None,
            },
            Self::Codex => env.codex_home.join("config.toml"),
            Self::Cursor => env.home.join(".cursor").join("mcp.json"),
            Self::GeminiCli => env.home.join(".gemini").join("settings.json"),
            Self::Opencode => opencode_file(&env.home.join(".config").join("opencode")),
            // `User` is the default profile's folder; a profile has its own under `profiles`.
            Self::Vscode => env.config_dir.join("Code").join("User").join("mcp.json"),
            Self::Zed => match env.os {
                Os::Windows => env.config_dir.join("Zed").join("settings.json"),
                Os::Linux => env
                    .xdg_config_home
                    .clone()
                    .unwrap_or_else(|| env.home.join(".config"))
                    .join("zed")
                    .join("settings.json"),
                Os::Mac => env.home.join(".config").join("zed").join("settings.json"),
            },
        })
    }

    /// The project-level config file in the working folder, or `None` where there is none.
    pub fn project_path(self, env: &Env) -> Option<PathBuf> {
        Some(match self {
            Self::ClaudeCode => env.cwd.join(".mcp.json"),
            Self::Cursor => env.cwd.join(".cursor").join("mcp.json"),
            Self::GeminiCli => env.cwd.join(".gemini").join("settings.json"),
            Self::Opencode => opencode_file(&env.cwd),
            Self::Vscode => env.cwd.join(".vscode").join("mcp.json"),
            Self::ClaudeDesktop | Self::Codex | Self::Zed => return None,
        })
    }
}

/// `opencode.json` in `dir`, or the `opencode.jsonc` that is there instead.
fn opencode_file(dir: &std::path::Path) -> PathBuf {
    let json = dir.join("opencode.json");
    let jsonc = dir.join("opencode.jsonc");
    if !json.exists() && jsonc.exists() {
        jsonc
    } else {
        json
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_client_is_found_by_its_id() {
        for client in Client::ALL {
            assert_eq!(Client::from_id(client.id()), Some(client));
        }
        assert_eq!(Client::from_id("windsurf"), None);
        assert_eq!(Client::from_id("Cursor"), None);
    }

    #[test]
    fn only_the_documented_clients_have_a_project_path() {
        let env = Env::rooted(std::path::Path::new("/r"), Os::Linux);
        let with: Vec<&str> = Client::ALL
            .into_iter()
            .filter(|client| client.project_path(&env).is_some())
            .map(Client::id)
            .collect();
        assert_eq!(
            with,
            ["claude-code", "cursor", "gemini-cli", "opencode", "vscode"]
        );
    }

    // The expected paths are written with `/`, as a Unix path is.
    #[cfg(unix)]
    #[test]
    fn user_paths_follow_each_systems_layout() {
        let root = std::path::Path::new("/r");
        let mac = Env::rooted(root, Os::Mac);
        assert_eq!(
            Client::ClaudeDesktop.user_path(&mac),
            Some(PathBuf::from(
                "/r/home/Library/Application Support/Claude/claude_desktop_config.json"
            ))
        );
        assert_eq!(
            Client::Vscode.user_path(&mac),
            Some(PathBuf::from(
                "/r/home/Library/Application Support/Code/User/mcp.json"
            ))
        );
        assert_eq!(
            Client::Zed.user_path(&mac),
            Some(PathBuf::from("/r/home/.config/zed/settings.json"))
        );
        let windows = Env::rooted(root, Os::Windows);
        assert_eq!(
            Client::ClaudeDesktop.user_path(&windows),
            Some(PathBuf::from(
                "/r/AppData/Roaming/Claude/claude_desktop_config.json"
            ))
        );
        assert_eq!(
            Client::Zed.user_path(&windows),
            Some(PathBuf::from("/r/AppData/Roaming/Zed/settings.json"))
        );
        let mut linux = Env::rooted(root, Os::Linux);
        assert_eq!(Client::ClaudeDesktop.user_path(&linux), None);
        assert_eq!(
            Client::Vscode.user_path(&linux),
            Some(PathBuf::from("/r/home/.config/Code/User/mcp.json"))
        );
        assert_eq!(
            Client::Zed.user_path(&linux),
            Some(PathBuf::from("/r/home/.config/zed/settings.json"))
        );
        linux.xdg_config_home = Some(PathBuf::from("/xdg"));
        assert_eq!(
            Client::Zed.user_path(&linux),
            Some(PathBuf::from("/xdg/zed/settings.json"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_dot_folder_clients_keep_their_files_under_home() {
        let env = Env::rooted(std::path::Path::new("/r"), Os::Linux);
        assert_eq!(
            Client::Cursor.user_path(&env),
            Some(PathBuf::from("/r/home/.cursor/mcp.json"))
        );
        assert_eq!(
            Client::GeminiCli.user_path(&env),
            Some(PathBuf::from("/r/home/.gemini/settings.json"))
        );
        assert_eq!(
            Client::Codex.user_path(&env),
            Some(PathBuf::from("/r/home/.codex/config.toml"))
        );
        assert_eq!(
            Client::Opencode.user_path(&env),
            Some(PathBuf::from("/r/home/.config/opencode/opencode.json"))
        );
        assert_eq!(
            Client::ClaudeCode.user_path(&env),
            Some(PathBuf::from("/r/home/.claude.json"))
        );
    }
}
