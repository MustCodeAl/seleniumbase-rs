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
    /// Antigravity.
    Antigravity,
    /// AugmentCode.
    AugmentCode,
    /// Claude Code, through its `claude mcp` commands.
    ClaudeCode,
    /// The Claude desktop app.
    ClaudeDesktop,
    /// Cline.
    Cline,
    /// The OpenAI Codex CLI.
    Codex,
    /// GitHub Copilot.
    Copilot,
    /// Cursor.
    Cursor,
    /// Factory Droid.
    Droid,
    /// The Gemini CLI.
    GeminiCli,
    /// Grok Build.
    Grok,
    /// Hermes.
    Hermes,
    /// Kilo CLI.
    Kilo,
    /// Kimi CLI.
    Kimi,
    /// Kiro.
    Kiro,
    /// Oh My Pi (OMP).
    Omp,
    /// opencode.
    Opencode,
    /// openclaw.
    Openclaw,
    /// Pi.
    Pi,
    /// Plank.
    Plank,
    /// Qwen Code.
    Qwen,
    /// Roo Code.
    RooCode,
    /// Trae.
    Trae,
    /// Mistral Vibe.
    Vibe,
    /// Visual Studio Code.
    Vscode,
    /// Warp terminal.
    Warp,
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
    /// A TOML file of `[mcp_servers.NAME]` tables, written directly.
    Toml,
}

impl Client {
    /// Every client, in the order `clients` lists them.
    pub const ALL: [Self; 27] = [
        Self::Antigravity,
        Self::AugmentCode,
        Self::ClaudeCode,
        Self::ClaudeDesktop,
        Self::Cline,
        Self::Codex,
        Self::Copilot,
        Self::Cursor,
        Self::Droid,
        Self::GeminiCli,
        Self::Grok,
        Self::Hermes,
        Self::Kilo,
        Self::Kimi,
        Self::Kiro,
        Self::Omp,
        Self::Opencode,
        Self::Openclaw,
        Self::Pi,
        Self::Plank,
        Self::Qwen,
        Self::RooCode,
        Self::Trae,
        Self::Vibe,
        Self::Vscode,
        Self::Warp,
        Self::Zed,
    ];

    /// The name used on the command line.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Antigravity => "antigravity",
            Self::AugmentCode => "auggie",
            Self::ClaudeCode => "claude",
            Self::ClaudeDesktop => "claude-desktop",
            Self::Cline => "cline",
            Self::Codex => "codex",
            Self::Copilot => "copilot",
            Self::Cursor => "cursor",
            Self::Droid => "droid",
            Self::GeminiCli => "gemini",
            Self::Grok => "grok",
            Self::Hermes => "hermes",
            Self::Kilo => "kilo",
            Self::Kimi => "kimi",
            Self::Kiro => "kiro",
            Self::Omp => "omp",
            Self::Opencode => "opencode",
            Self::Openclaw => "openclaw",
            Self::Pi => "pi",
            Self::Plank => "plank",
            Self::Qwen => "qwen",
            Self::RooCode => "roo-code",
            Self::Trae => "trae",
            Self::Vibe => "vibe",
            Self::Vscode => "vscode",
            Self::Warp => "warp",
            Self::Zed => "zed",
        }
    }

    /// The name people know it by.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Antigravity => "Antigravity",
            Self::AugmentCode => "AugmentCode",
            Self::ClaudeCode => "Claude Code",
            Self::ClaudeDesktop => "Claude Desktop",
            Self::Cline => "Cline",
            Self::Codex => "OpenAI Codex CLI",
            Self::Copilot => "GitHub Copilot",
            Self::Cursor => "Cursor",
            Self::Droid => "Factory Droid",
            Self::GeminiCli => "Gemini CLI",
            Self::Grok => "Grok Build",
            Self::Hermes => "Hermes",
            Self::Kilo => "Kilo CLI",
            Self::Kimi => "Kimi CLI",
            Self::Kiro => "Kiro",
            Self::Omp => "Oh My Pi (OMP)",
            Self::Opencode => "opencode",
            Self::Openclaw => "openclaw",
            Self::Pi => "Pi",
            Self::Plank => "Plank",
            Self::Qwen => "Qwen Code",
            Self::RooCode => "Roo Code",
            Self::Trae => "Trae",
            Self::Vibe => "Mistral Vibe",
            Self::Vscode => "Visual Studio Code",
            Self::Warp => "Warp",
            Self::Zed => "Zed",
        }
    }

    /// The client called `id`, if there is one.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "claude-code" => Some(Self::ClaudeCode),
            "gemini-cli" => Some(Self::GeminiCli),
            "github-copilot" => Some(Self::Copilot),
            "factory-droid" => Some(Self::Droid),
            "augment" => Some(Self::AugmentCode),
            "roo" => Some(Self::RooCode),
            "mistral-vibe" => Some(Self::Vibe),
            _ => Self::ALL.into_iter().find(|client| client.id() == id),
        }
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
            Self::Antigravity => "https://antigravity.google/docs/mcp",
            Self::AugmentCode => "https://docs.augmentcode.com/mcp",
            Self::ClaudeCode => "https://code.claude.com/docs/en/mcp",
            Self::ClaudeDesktop => "https://modelcontextprotocol.io/quickstart/user",
            Self::Cline => "https://github.com/cline/cline",
            Self::Codex => "https://developers.openai.com/codex/mcp",
            Self::Copilot => "https://docs.github.com/en/copilot/mcp",
            Self::Cursor => "https://cursor.com/docs/context/mcp",
            Self::Droid => "https://factory.ai/docs/droid",
            Self::GeminiCli => {
                "https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md"
            }
            Self::Grok => {
                "https://docs.x.ai/build/features/mcp-servers and https://docs.x.ai/build/settings"
            }
            Self::Hermes => "https://hermes-agent.io/docs/mcp",
            Self::Kilo => "https://kilo.dev/docs/mcp",
            Self::Kimi => "https://kimi.moonshot.cn/docs/mcp",
            Self::Kiro => "https://kiro.ai/docs/mcp",
            Self::Omp => "https://github.com/oh-my-pi/omp",
            Self::Opencode => {
                "https://opencode.ai/docs/mcp-servers/ and https://opencode.ai/docs/config/"
            }
            Self::Openclaw => "https://openclaw.ai/docs/mcp",
            Self::Pi => "https://pi.ai/docs/mcp",
            Self::Plank => "https://plank.dev/docs/mcp",
            Self::Qwen => "https://qwenlm.github.io/docs/mcp",
            Self::RooCode => "https://github.com/RooVetGit/Roo-Code",
            Self::Trae => "https://trae.ai/docs/mcp",
            Self::Vibe => "https://mistral.ai/docs/vibe/mcp",
            Self::Vscode => {
                "https://code.visualstudio.com/docs/copilot/customization/mcp-servers and \
                 https://code.visualstudio.com/docs/configure/profiles"
            }
            Self::Warp => "https://docs.warp.dev/features/mcp",
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
            Self::Grok => Kind::Toml,
            _ => Kind::Json,
        }
    }

    /// One line on how `install` changes this client.
    pub const fn how(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude mcp add-json / claude mcp remove",
            Self::Codex => "codex mcp add / remove, else config.toml (table mcp_servers)",
            Self::Grok => "config.toml (table mcp_servers)",
            Self::Opencode => "JSON, key mcp (type local)",
            Self::Vscode => "JSON, key servers (type stdio)",
            Self::Zed => "JSON, key context_servers",
            _ => "JSON, key mcpServers",
        }
    }

    /// The program that can register a server (`claude`, `codex`) or help with it (`code`).
    pub const fn program(self) -> Option<&'static str> {
        match self {
            Self::ClaudeCode => Some("claude"),
            Self::Codex => Some("codex"),
            Self::Grok => Some("grok"),
            Self::Vscode => Some("code"),
            Self::AugmentCode => Some("auggie"),
            Self::Copilot => Some("copilot"),
            Self::Droid => Some("droid"),
            Self::GeminiCli => Some("gemini"),
            Self::Hermes => Some("hermes"),
            Self::Kilo => Some("kilo"),
            Self::Kimi => Some("kimi"),
            Self::Kiro => Some("kiro"),
            Self::Omp => Some("omp"),
            Self::Openclaw => Some("openclaw"),
            Self::Pi => Some("pi"),
            Self::Plank => Some("plank"),
            Self::Qwen => Some("qwen"),
            Self::Trae => Some("trae"),
            Self::Vibe => Some("vibe"),
            Self::Warp => Some("warp"),
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
            Self::Cline | Self::RooCode => "it uses global extension settings only",
            _ => "it keeps no project config",
        }
    }

    /// The layout of the JSON this client reads, for the clients with a JSON file.
    pub const fn layout(self) -> Option<Layout> {
        match self {
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
            Self::Codex | Self::Grok => None,
            _ => Some(Layout {
                key: "mcpServers",
                kind: LayoutKind::Plain { typed: false },
            }),
        }
    }

    /// The user-level config file, or `None` where the client has none.
    pub fn user_path(self, env: &Env) -> Option<PathBuf> {
        Some(match self {
            Self::Antigravity => env.home.join(".gemini").join("antigravity").join("settings.json"),
            Self::AugmentCode => env.home.join(".augment").join("config.json"),
            Self::ClaudeCode => env.home.join(".claude.json"),
            // Documented for macOS and Windows only; Linux builds are not covered.
            Self::ClaudeDesktop => match env.os {
                Os::Mac | Os::Windows => env
                    .config_dir
                    .join("Claude")
                    .join("claude_desktop_config.json"),
                Os::Linux => return None,
            },
            Self::Cline => match env.os {
                Os::Mac => env.home.join("Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json"),
                Os::Windows => env.config_dir.join("Code").join("User").join("globalStorage").join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json"),
                Os::Linux => env.home.join(".config").join("Code").join("User").join("globalStorage").join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json"),
            },
            Self::Codex => env.codex_home.join("config.toml"),
            Self::Copilot => env.home.join(".config").join("github-copilot").join("mcp.json"),
            Self::Cursor => env.home.join(".cursor").join("mcp.json"),
            Self::Droid => env.home.join(".factory").join("mcp.json"),
            Self::GeminiCli => env.home.join(".gemini").join("settings.json"),
            Self::Grok => env.grok_home.join("config.toml"),
            Self::Hermes => env.home.join(".hermes").join("config.json"),
            Self::Kilo => env.home.join(".kilo").join("config.json"),
            Self::Kimi => env.home.join(".kimi").join("mcp.json"),
            Self::Kiro => env.home.join(".kiro").join("settings.json"),
            Self::Omp => env.home.join(".omp").join("mcp.json"),
            Self::Opencode => opencode_file(&env.home.join(".config").join("opencode")),
            Self::Openclaw => env.home.join(".openclaw").join("config.json"),
            Self::Pi => env.home.join(".pi").join("mcp.json"),
            Self::Plank => env.home.join(".plank").join("config.json"),
            Self::Qwen => env.home.join(".qwen").join("mcp.json"),
            Self::RooCode => match env.os {
                Os::Mac => env.home.join("Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json"),
                Os::Windows => env.config_dir.join("Code").join("User").join("globalStorage").join("rooveterinaryinc.roo-cline").join("settings").join("cline_mcp_settings.json"),
                Os::Linux => env.home.join(".config").join("Code").join("User").join("globalStorage").join("rooveterinaryinc.roo-cline").join("settings").join("cline_mcp_settings.json"),
            },
            Self::Trae => match env.os {
                Os::Mac => env.home.join("Library/Application Support/Trae/User/mcp.json"),
                Os::Windows => env.config_dir.join("Trae").join("User").join("mcp.json"),
                Os::Linux => env.home.join(".config").join("Trae").join("User").join("mcp.json"),
            },
            Self::Vibe => env.home.join(".vibe").join("config.json"),
            Self::Vscode => env.config_dir.join("Code").join("User").join("mcp.json"),
            Self::Warp => match env.os {
                Os::Mac => env.home.join("Library/Application Support/dev.warp.Warp-Stable/mcp.json"),
                Os::Windows => env.config_dir.join("warp").join("mcp.json"),
                Os::Linux => env.home.join(".config").join("warp-terminal").join("mcp.json"),
            },
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
            Self::Antigravity => env.cwd.join(".antigravity").join("mcp.json"),
            Self::AugmentCode => env.cwd.join(".augment").join("config.json"),
            Self::ClaudeCode => env.cwd.join(".mcp.json"),
            Self::Copilot => env.cwd.join(".github").join("copilot-mcp.json"),
            Self::Cursor => env.cwd.join(".cursor").join("mcp.json"),
            Self::Droid => env.cwd.join(".factory").join("mcp.json"),
            Self::GeminiCli => env.cwd.join(".gemini").join("settings.json"),
            Self::Grok => env.cwd.join(".grok").join("config.toml"),
            Self::Hermes => env.cwd.join(".hermes").join("config.json"),
            Self::Kilo => env.cwd.join(".kilo").join("config.json"),
            Self::Kimi => env.cwd.join(".kimi").join("mcp.json"),
            Self::Kiro => env.cwd.join(".kiro").join("settings.json"),
            Self::Omp => env.cwd.join(".omp").join("mcp.json"),
            Self::Opencode => opencode_file(&env.cwd),
            Self::Openclaw => env.cwd.join(".openclaw").join("config.json"),
            Self::Pi => env.cwd.join(".pi").join("mcp.json"),
            Self::Plank => env.cwd.join(".plank").join("config.json"),
            Self::Qwen => env.cwd.join(".qwen").join("mcp.json"),
            Self::Trae => env.cwd.join(".trae").join("mcp.json"),
            Self::Vibe => env.cwd.join(".vibe").join("config.json"),
            Self::Vscode => env.cwd.join(".vscode").join("mcp.json"),
            Self::Warp => env.cwd.join(".warp").join("mcp.json"),
            Self::ClaudeDesktop | Self::Cline | Self::Codex | Self::RooCode | Self::Zed => {
                return None;
            }
        })
    }

    /// Whether this client is installed on the machine `env` describes.
    ///
    /// Installed means its user config file is there, or a folder that belongs to
    /// the client (not the home folder and not the shared config folder), or its
    /// program (`claude`, `codex`, `grok` or `code`) is on `PATH`.
    pub fn detected(self, env: &Env) -> bool {
        if let Some(path) = self.user_path(env)
            && (path.exists() || config_folder_exists(env, &path))
        {
            return true;
        }
        self.program()
            .is_some_and(|program| env.find_program(program).is_some())
    }
}

/// Whether a folder of `file` that belongs to this client, and not to every
/// program, is already there.
fn config_folder_exists(env: &Env, file: &std::path::Path) -> bool {
    let mut dir = file.parent();
    while let Some(current) = dir {
        if is_shared_root(env, current) {
            return false;
        }
        if current.exists() {
            return true;
        }
        dir = current.parent();
    }
    false
}

/// Home and the shared config folders. A client is not "installed" merely
/// because one of these exists.
fn is_shared_root(env: &Env, path: &std::path::Path) -> bool {
    path == env.home
        || path == env.config_dir
        || path == env.data_dir
        || env.xdg_config_home.as_deref() == Some(path)
        || path == env.home.join(".config")
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
        // Verify common aliases
        assert_eq!(Client::from_id("claude-code"), Some(Client::ClaudeCode));
        assert_eq!(Client::from_id("gemini-cli"), Some(Client::GeminiCli));
        assert_eq!(Client::from_id("github-copilot"), Some(Client::Copilot));
        assert_eq!(Client::from_id("factory-droid"), Some(Client::Droid));
        assert_eq!(Client::from_id("augment"), Some(Client::AugmentCode));
        assert_eq!(Client::from_id("roo"), Some(Client::RooCode));
        assert_eq!(Client::from_id("mistral-vibe"), Some(Client::Vibe));
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
            [
                "antigravity",
                "auggie",
                "claude",
                "copilot",
                "cursor",
                "droid",
                "gemini",
                "grok",
                "hermes",
                "kilo",
                "kimi",
                "kiro",
                "omp",
                "opencode",
                "openclaw",
                "pi",
                "plank",
                "qwen",
                "trae",
                "vibe",
                "vscode",
                "warp",
            ]
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
            Client::Grok.user_path(&env),
            Some(PathBuf::from("/r/home/.grok/config.toml"))
        );
        assert_eq!(
            Client::Opencode.user_path(&env),
            Some(PathBuf::from("/r/home/.config/opencode/opencode.json"))
        );
        assert_eq!(
            Client::ClaudeCode.user_path(&env),
            Some(PathBuf::from("/r/home/.claude.json"))
        );
        assert_eq!(
            Client::Antigravity.user_path(&env),
            Some(PathBuf::from("/r/home/.gemini/antigravity/settings.json"))
        );
        assert_eq!(
            Client::AugmentCode.user_path(&env),
            Some(PathBuf::from("/r/home/.augment/config.json"))
        );
        assert_eq!(
            Client::Droid.user_path(&env),
            Some(PathBuf::from("/r/home/.factory/mcp.json"))
        );
        assert_eq!(
            Client::Hermes.user_path(&env),
            Some(PathBuf::from("/r/home/.hermes/config.json"))
        );
        assert_eq!(
            Client::Kilo.user_path(&env),
            Some(PathBuf::from("/r/home/.kilo/config.json"))
        );
        assert_eq!(
            Client::Kimi.user_path(&env),
            Some(PathBuf::from("/r/home/.kimi/mcp.json"))
        );
        assert_eq!(
            Client::Kiro.user_path(&env),
            Some(PathBuf::from("/r/home/.kiro/settings.json"))
        );
        assert_eq!(
            Client::Omp.user_path(&env),
            Some(PathBuf::from("/r/home/.omp/mcp.json"))
        );
        assert_eq!(
            Client::Openclaw.user_path(&env),
            Some(PathBuf::from("/r/home/.openclaw/config.json"))
        );
        assert_eq!(
            Client::Pi.user_path(&env),
            Some(PathBuf::from("/r/home/.pi/mcp.json"))
        );
        assert_eq!(
            Client::Plank.user_path(&env),
            Some(PathBuf::from("/r/home/.plank/config.json"))
        );
        assert_eq!(
            Client::Qwen.user_path(&env),
            Some(PathBuf::from("/r/home/.qwen/mcp.json"))
        );
        assert_eq!(
            Client::Vibe.user_path(&env),
            Some(PathBuf::from("/r/home/.vibe/config.json"))
        );
    }
}
