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
enum DevCommand {
    /// Authoring and scaffolding tools for seleniumbase-rs plugins.
    Plugin(DevPluginArgs),
}

#[derive(Debug, usage::Args)]
struct DevPluginArgs {
    #[usage(subcommand)]
    command: DevPluginCommand,
}

#[derive(Debug, usage::Subcommands)]
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
struct DevPluginNewArgs {
    /// Name of the new plugin crate; omit to launch the interactive TUI wizard.
    #[usage(value_hint = usage::ValueHint::Other)]
    name: Option<String>,
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
    /// Destination directory for the scaffolded plugin.
    #[usage(long = "out-dir", value_name = "DIR", value_hint = usage::ValueHint::DirPath)]
    out_dir: Option<PathBuf>,
    /// Force the interactive TUI wizard even when NAME is provided.
    #[usage(long, bool_value, conflicts("--no-tui"))]
    tui: bool,
    /// Disable the interactive TUI wizard and require NAME.
    #[usage(long = "no-tui", bool_value)]
    no_tui: bool,
}

#[derive(Debug, usage::Args)]
struct DevPluginPreviewArgs {
    /// Name of the plugin crate to preview.
    #[usage(value_hint = usage::ValueHint::Other)]
    name: String,
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
    /// Development environment configuration.
    #[usage(long = "dev-env", default = "cargo", value_hint = usage::ValueHint::Other)]
    dev_env: String,
    /// AI agent rule file to generate.
    #[usage(long = "ai-agent", default = "none", value_hint = usage::ValueHint::Other)]
    ai_agent: String,
    /// Primary IDE for instructions.
    #[usage(long, default = "cursor", value_hint = usage::ValueHint::Other)]
    ide: String,
    /// Target execution environment.
    #[usage(long = "target-env", default = "local", value_hint = usage::ValueHint::Other)]
    target_env: String,
}

#[derive(Debug, usage::Args)]
struct DevPluginCheckArgs {
    /// Path to the plugin project directory (defaults to current folder).
    #[usage(default = ".", value_hint = usage::ValueHint::DirPath)]
    path: PathBuf,
}

fn options_from_new(args: &DevPluginNewArgs) -> plugin_scaffold::PluginScaffoldOptions {
    plugin_scaffold::PluginScaffoldOptions {
        name: args.name.clone().unwrap_or_default(),
        template: args.template.clone(),
        backend: args.backend.clone(),
        focus: args.focus.clone(),
        integrations: args.integrations.clone(),
        headless: args.headless,
        dev_env: args.dev_env.clone(),
        ai_agent: args.ai_agent.clone(),
        ide: args.ide.clone(),
        target_env: args.target_env.clone(),
        out_dir: args.out_dir.clone(),
    }
}

fn run_new(args: &DevPluginNewArgs) -> Result<u8, Box<dyn std::error::Error>> {
    let want_tui = !args.no_tui && (args.tui || args.name.is_none());
    let opts = options_from_new(args);

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

    if args.name.is_none() {
        eprintln!("error: <NAME> is required when --no-tui is set");
        return Ok(2);
    }

    let result = plugin_scaffold::scaffold_plugin(&opts)?;
    std::io::stdout()
        .lock()
        .write_all(result.instructions.as_bytes())?;
    Ok(0)
}

fn run_preview(args: &DevPluginPreviewArgs) -> Result<(), Box<dyn std::error::Error>> {
    let opts = plugin_scaffold::PluginScaffoldOptions {
        name: args.name.clone(),
        template: args.template.clone(),
        backend: args.backend.clone(),
        focus: args.focus.clone(),
        integrations: args.integrations.clone(),
        headless: args.headless,
        dev_env: args.dev_env.clone(),
        ai_agent: args.ai_agent.clone(),
        ide: args.ide.clone(),
        target_env: args.target_env.clone(),
        out_dir: None,
    };
    let (files, instructions) = plugin_scaffold::render_plugin_files(&opts)?;
    let mut out = std::io::stdout().lock();
    for (path, content) in files {
        writeln!(out, "=== {} ===\n{}", path.display(), content)?;
    }
    writeln!(out, "{instructions}")?;
    Ok(())
}

fn run() -> Result<u8, Box<dyn std::error::Error>> {
    let cli = DevCli::parse();
    match cli.command {
        DevCommand::Plugin(plugin) => match plugin.command {
            DevPluginCommand::New(args) => run_new(&args),
            DevPluginCommand::Preview(args) => {
                run_preview(&args)?;
                Ok(0)
            }
            DevPluginCommand::Check(args) => {
                let report = plugin_manager::check_plugin_project(&args.path)?;
                println!("{report}");
                Ok(0)
            }
        },
    }
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
        assert_eq!(args.template, "cli");
        assert!(args.headless);
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
