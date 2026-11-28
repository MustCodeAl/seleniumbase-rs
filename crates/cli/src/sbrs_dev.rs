//! `sbrs-dev`: developer command line for authoring `seleniumbase-rs` plugins.
//!
//! Keeps plugin development workflows (`sbrs-dev plugin new` interactive TUI,
//! `preview`, `check`) separate from runtime user plugin management in `sbrs plugin`.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

#[cfg(feature = "tui")]
use seleniumbase_rs::cli::scripts::plugin_scaffold_tui;
use seleniumbase_rs::cli::{plugin_manager, plugin_scaffold};

extern crate usage_rs as usage;

#[derive(Debug, usage::Cli)]
#[usage(
    name = "sbrs-dev",
    version,
    about = "Developer tools for creating and checking seleniumbase-rs plugins.",
    repository = env!("CARGO_PKG_REPOSITORY")
)]
struct DevCli {
    #[usage(subcommand)]
    command: DevCommand,
}

#[derive(Debug, usage::Subcommands)]
#[usage(run)]
enum DevCommand {
    /// Authoring and scaffolding tools for seleniumbase-rs plugins.
    Plugin(DevPluginArgs),
}

#[derive(Debug, usage::Args)]
#[usage(run)]
struct DevPluginArgs {
    #[usage(subcommand)]
    command: DevPluginCommand,
}

#[derive(Debug, usage::Subcommands)]
#[usage(run)]
enum DevPluginCommand {
    /// Create a new seleniumbase-rs plugin crate (opens the TUI wizard by default).
    #[usage(example = "sbrs-dev plugin new")]
    #[usage(example = "sbrs-dev plugin new my-plugin --template library --no-tui")]
    New(DevPluginNewArgs),
    /// Render the scaffold files in memory and print them without writing to disk.
    #[usage(example = "sbrs-dev plugin preview my-plugin --focus cli")]
    Preview(DevPluginPreviewArgs),
    /// Validate a local plugin crate directory (`Cargo.toml` and `src/lib.rs`).
    #[usage(example = "sbrs-dev plugin check ./my-plugin")]
    Check(DevPluginCheckArgs),
}

#[derive(Debug, usage::Args)]
struct DevPluginCommonArgs {
    /// Template style ("library" or "cli").
    #[usage(long, default = "library", value_hint = usage::ValueHint::Other)]
    template: String,
    /// Backend foundation ("cdp" or "webdriver").
    #[usage(long, value_hint = usage::ValueHint::Other)]
    backend: Option<String>,
    /// Architectural focus ("general", "cli", or "scraping").
    #[usage(long, default = "general", value_hint = usage::ValueHint::Other)]
    focus: String,
    /// Target integrations ("mcp", "ci", or "mcp,ci").
    #[usage(long, value_hint = usage::ValueHint::Other)]
    integrations: Option<String>,
    /// Enable headless mode by default.
    #[usage(long, bool_value)]
    headless: bool,
    /// Development environment configuration ("cargo", "mise", "docker", "gitlab-ci", "github-actions").
    #[usage(long = "dev-env", default = "cargo", value_hint = usage::ValueHint::Other)]
    dev_env: String,
    /// AI agent rule file ("none", "cursor", "claude-code", "gemini", "antigravity", "codex").
    #[usage(long = "ai-agent", default = "none", value_hint = usage::ValueHint::Other)]
    ai_agent: String,
    /// Primary IDE ("cursor", "vscode", "zed", "clion", "neovim", "helix").
    #[usage(long, default = "cursor", value_hint = usage::ValueHint::Other)]
    ide: String,
    /// Target execution environment ("local", "docker", "cloud").
    #[usage(long = "target-env", default = "local", value_hint = usage::ValueHint::Other)]
    target_env: String,
}

impl DevPluginCommonArgs {
    fn into_options(
        self,
        name: String,
        out_dir: Option<PathBuf>,
    ) -> plugin_scaffold::PluginScaffoldOptions {
        plugin_scaffold::PluginScaffoldOptions {
            name,
            template: self.template,
            backend: self.backend,
            focus: self.focus,
            integrations: self.integrations,
            headless: self.headless,
            dev_env: self.dev_env,
            ai_agent: self.ai_agent,
            ide: self.ide,
            target_env: self.target_env,
            out_dir,
        }
    }
}

#[derive(Debug, usage::Args)]
struct DevPluginNewArgs {
    /// Name of the new plugin crate; omit to launch the interactive TUI wizard.
    #[usage(value_hint = usage::ValueHint::Other)]
    name: Option<String>,
    /// Destination directory for the scaffolded plugin.
    #[usage(long = "out-dir", value_name = "DIR", value_hint = usage::ValueHint::DirPath)]
    out_dir: Option<PathBuf>,
    /// Force the interactive TUI wizard even when NAME is provided.
    #[usage(long, bool_value, conflicts("--no-tui"))]
    tui: bool,
    /// Disable the interactive TUI wizard and require NAME.
    #[usage(long = "no-tui", bool_value)]
    no_tui: bool,
    #[usage(flatten)]
    common: DevPluginCommonArgs,
}

#[derive(Debug, usage::Args)]
struct DevPluginPreviewArgs {
    /// Name of the plugin crate to preview.
    #[usage(value_hint = usage::ValueHint::Other)]
    name: String,
    #[usage(flatten)]
    common: DevPluginCommonArgs,
}

#[derive(Debug, usage::Args)]
struct DevPluginCheckArgs {
    /// Path to the plugin project directory (defaults to current folder).
    #[usage(default = ".", value_hint = usage::ValueHint::DirPath)]
    path: PathBuf,
}

impl usage::Run for DevPluginNewArgs {
    type Output = Result<u8, Box<dyn std::error::Error>>;

    fn run(self) -> Self::Output {
        let want_tui = !self.no_tui && (self.tui || self.name.is_none());
        let has_name = self.name.is_some();
        let opts = self
            .common
            .into_options(self.name.unwrap_or_default(), self.out_dir);

        if want_tui {
            #[cfg(feature = "tui")]
            {
                match plugin_scaffold_tui::run_plugin_scaffold_tui(opts)? {
                    Some(result) => {
                        std::io::stdout()
                            .lock()
                            .write_all(result.instructions.as_bytes())?;
                    }
                    None => {
                        eprintln!("Plugin scaffolding cancelled.");
                    }
                }
                return Ok(0);
            }
            #[cfg(not(feature = "tui"))]
            {
                eprintln!(
                    "this sbrs-dev was built without the terminal UI; pass `<NAME> --no-tui` or reinstall with `--features tui`"
                );
                return Ok(2);
            }
        }

        if !has_name {
            eprintln!("error: <NAME> is required when --no-tui is set");
            return Ok(2);
        }

        let result = plugin_scaffold::scaffold_plugin(&opts)?;
        std::io::stdout()
            .lock()
            .write_all(result.instructions.as_bytes())?;
        Ok(0)
    }
}

impl usage::Run for DevPluginPreviewArgs {
    type Output = Result<u8, Box<dyn std::error::Error>>;

    fn run(self) -> Self::Output {
        let opts = self.common.into_options(self.name, None);
        let (files, instructions) = plugin_scaffold::render_plugin_files(&opts)?;
        let mut out = std::io::stdout().lock();
        for (path, content) in files {
            writeln!(out, "=== {} ===\n{}", path.display(), content)?;
        }
        writeln!(out, "{instructions}")?;
        Ok(0)
    }
}

impl usage::Run for DevPluginCheckArgs {
    type Output = Result<u8, Box<dyn std::error::Error>>;

    fn run(self) -> Self::Output {
        let report = plugin_manager::check_plugin_project(&self.path)?;
        println!("{report}");
        Ok(0)
    }
}

fn run() -> Result<u8, Box<dyn std::error::Error>> {
    use usage::Run as _;
    DevCli::parse().command.run()
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    fn parse(args: &[&str]) -> Result<DevCli, String> {
        let argv: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        DevCli::parse_from_argv(&argv).map_err(|err| format!("{err:?}"))
    }

    #[test]
    fn new_allows_omitted_name_for_default_tui() {
        let cli = parse(&["sbrs-dev", "plugin", "new"]).unwrap();
        let DevCommand::Plugin(DevPluginArgs {
            command: DevPluginCommand::New(args),
        }) = cli.command
        else {
            panic!("expected plugin new");
        };
        assert!(args.name.is_none());
        assert!(!args.no_tui);
    }

    #[test]
    fn new_accepts_name_and_flags() {
        let cli = parse(&[
            "sbrs-dev",
            "plugin",
            "new",
            "sample-plugin",
            "--template",
            "cli",
            "--dev-env",
            "mise",
            "--headless",
            "--no-tui",
        ])
        .unwrap();
        let DevCommand::Plugin(DevPluginArgs {
            command: DevPluginCommand::New(args),
        }) = cli.command
        else {
            panic!("expected plugin new");
        };
        assert_eq!(args.name.as_deref(), Some("sample-plugin"));
        assert_eq!(args.common.template, "cli");
        assert!(args.common.headless);
        assert!(args.no_tui);
    }

    #[test]
    fn check_and_preview_parse() {
        let preview = parse(&["sbrs-dev", "plugin", "preview", "demo-plug"]);
        assert!(preview.is_ok());

        let check = parse(&["sbrs-dev", "plugin", "check", "."]);
        assert!(check.is_ok());
    }
}
