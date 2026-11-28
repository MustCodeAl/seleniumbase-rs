//! Scaffolding engine for creating new `seleniumbase-rs` plugin projects.
//!
//! Generates a complete, modular Rust crate configured with templates, backends,
//! CI environments, and tailored AI agent instructions.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Why scaffolding a plugin failed.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PluginScaffoldError {
    /// The plugin name is invalid.
    #[error("invalid plugin name {name:?}: {reason}")]
    InvalidName {
        /// The name as given.
        name: String,
        /// What is wrong with it.
        reason: String,
    },
    /// Target directory already exists and is not empty.
    #[error("target directory '{}' already exists and is not empty", .0.display())]
    DirectoryNotEmpty(PathBuf),
    /// An I/O error occurred while creating directories or writing files.
    #[error("I/O error at '{}': {source}", .path.display())]
    Io {
        /// The path where the I/O error occurred.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// Options for scaffolding a new plugin project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginScaffoldOptions {
    /// The plugin name.
    pub name: String,
    /// Template style (e.g. "library", "cli").
    pub template: String,
    /// Backend foundation (e.g. "cdp", "webdriver").
    pub backend: Option<String>,
    /// Architectural focus (e.g. "general", "cli", "scraping").
    pub focus: String,
    /// Target integrations (e.g. "mcp", "ci").
    pub integrations: Option<String>,
    /// Whether headless mode is enabled by default.
    pub headless: bool,
    /// Development environment configuration (e.g. "cargo", "mise", "docker", "gitlab-ci", "github-actions").
    pub dev_env: String,
    /// AI agent rule file to generate (e.g. "none", "cursor", "claude-code", "gemini", "antigravity", "codex").
    pub ai_agent: String,
    /// Primary IDE for quick start instructions (e.g. "cursor", "vscode", "zed", "clion", "neovim", "helix").
    pub ide: String,
    /// Target execution environment (e.g. "local", "docker", "cloud").
    pub target_env: String,
    /// Destination directory path (defaults to plugin name).
    pub out_dir: Option<PathBuf>,
}

impl Default for PluginScaffoldOptions {
    fn default() -> Self {
        Self {
            name: String::new(),
            template: "library".to_string(),
            backend: None,
            focus: "general".to_string(),
            integrations: None,
            headless: false,
            dev_env: "cargo".to_string(),
            ai_agent: "none".to_string(),
            ide: "cursor".to_string(),
            target_env: "local".to_string(),
            out_dir: None,
        }
    }
}

impl PluginScaffoldOptions {
    /// Creates options with the given plugin name and default settings.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

/// Result of scaffolding a plugin project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldResult {
    /// Cleaned plugin name.
    pub name: String,
    /// Root directory of the created plugin.
    pub root_dir: PathBuf,
    /// Paths of all files created.
    pub files_created: Vec<PathBuf>,
    /// Quick start instructions for the user.
    pub instructions: String,
}

/// Renders all plugin scaffold files and instructions in memory without writing to disk.
///
/// # Errors
/// Returns [`PluginScaffoldError::InvalidName`] if the name is empty or contains path traversal characters.
pub fn render_plugin_files(
    opts: &PluginScaffoldOptions,
) -> Result<(Vec<(PathBuf, String)>, String), PluginScaffoldError> {
    let clean_name = opts.name.trim().to_lowercase().replace(' ', "-");
    if clean_name.is_empty() {
        return Err(PluginScaffoldError::InvalidName {
            name: opts.name.clone(),
            reason: "plugin name cannot be empty".to_string(),
        });
    }
    if clean_name.contains("..")
        || clean_name.contains('/')
        || clean_name.contains('\\')
        || clean_name.starts_with('.')
        || clean_name.starts_with('-')
    {
        return Err(PluginScaffoldError::InvalidName {
            name: opts.name.clone(),
            reason: "must not contain path traversal, slashes, or start with '.' or '-'"
                .to_string(),
        });
    }

    let root = match &opts.out_dir {
        Some(dir) => dir.clone(),
        None => PathBuf::from(&clean_name),
    };

    let mut files = Vec::new();

    // 1. Cargo.toml
    let cargo_toml = generate_cargo_toml(&clean_name, opts);
    files.push((root.join("Cargo.toml"), cargo_toml));

    // 2. src/lib.rs
    let lib_rs = generate_lib_rs(&clean_name, opts);
    files.push((root.join("src/lib.rs"), lib_rs));

    // 3. src/main.rs (if CLI focus)
    if opts.focus == "cli" || opts.template == "cli" {
        let main_rs = generate_main_rs(&clean_name, opts);
        files.push((root.join("src/main.rs"), main_rs));
    }

    // 4. tests/plugin_test.rs
    let test_rs = generate_test_rs(&clean_name, opts);
    files.push((root.join("tests/plugin_test.rs"), test_rs));

    // 5. README.md
    let readme = generate_readme(&clean_name, opts);
    files.push((root.join("README.md"), readme));

    // 6. Dev Environment files
    match opts.dev_env.as_str() {
        "mise" => {
            let mise_toml = generate_mise_toml(opts);
            files.push((root.join(".mise.toml"), mise_toml));
        }
        "docker" => {
            let dockerfile = generate_dockerfile(opts);
            files.push((root.join("Dockerfile"), dockerfile));

            let compose = generate_docker_compose(&clean_name);
            files.push((root.join("docker-compose.yml"), compose));
        }
        "gitlab-ci" => {
            let gitlab = generate_gitlab_ci();
            files.push((root.join(".gitlab-ci.yml"), gitlab));
        }
        "github-actions" => {
            let gha = generate_github_actions();
            files.push((root.join(".github/workflows/ci.yml"), gha));
        }
        _ => {}
    }

    // 7. AI Agent Rule Files
    match opts.ai_agent.as_str() {
        "cursor" => {
            let cursorrules = generate_cursorrules(&clean_name, opts);
            files.push((root.join(".cursorrules"), cursorrules));
        }
        "claude-code" => {
            let claude_md = generate_claude_md(&clean_name, opts);
            files.push((root.join("CLAUDE.md"), claude_md));
        }
        "gemini" | "antigravity" => {
            let agents_md = generate_agents_md(&clean_name, opts);
            files.push((root.join("AGENTS.md"), agents_md));
        }
        "codex" => {
            let copilot_md = generate_copilot_md(&clean_name, opts);
            files.push((root.join("COPILOT.md"), copilot_md));
        }
        _ => {}
    }

    let instructions = generate_instructions(&clean_name, &root, opts);
    Ok((files, instructions))
}

/// Scaffolds a new plugin project by writing all generated files to disk.
///
/// # Errors
/// Returns [`PluginScaffoldError`] if directory creation or file writing fails,
/// or if the target directory already exists and is not empty.
pub fn scaffold_plugin(
    opts: &PluginScaffoldOptions,
) -> Result<ScaffoldResult, PluginScaffoldError> {
    let clean_name = opts.name.trim().to_lowercase().replace(' ', "-");
    let root = match &opts.out_dir {
        Some(dir) => dir.clone(),
        None => PathBuf::from(&clean_name),
    };

    if root.exists() {
        let mut entries = fs::read_dir(&root).map_err(|source| PluginScaffoldError::Io {
            path: root.clone(),
            source,
        })?;
        if entries.next().is_some() {
            return Err(PluginScaffoldError::DirectoryNotEmpty(root));
        }
    }

    let (files, instructions) = render_plugin_files(opts)?;

    let mut created_files = Vec::new();
    for (path, content) in files {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| PluginScaffoldError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&path, content).map_err(|source| PluginScaffoldError::Io {
            path: path.clone(),
            source,
        })?;
        created_files.push(path);
    }

    Ok(ScaffoldResult {
        name: clean_name,
        root_dir: root,
        files_created: created_files,
        instructions,
    })
}

fn to_pascal_case(s: &str) -> String {
    s.split(&['-', '_'][..])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect()
}

fn generate_cargo_toml(name: &str, opts: &PluginScaffoldOptions) -> String {
    let mut backend_features = String::new();
    if let Some(ref b) = opts.backend {
        backend_features = format!(", \"{b}\"");
    }

    let bin_section = if opts.focus == "cli" || opts.template == "cli" {
        format!("\n[[bin]]\nname = \"{name}\"\npath = \"src/main.rs\"\n")
    } else {
        String::new()
    };

    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
description = "SeleniumBase-RS automation plugin: {name}"
license = "MIT"
{bin_section}
[dependencies]
seleniumbase-rs = {{ version = "0.9", features = ["plugins-builtin"{backend_features}] }}
tokio = {{ version = "1.40", features = ["full"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}
thiserror = "2.0"

[dev-dependencies]
seleniumbase-rs = {{ version = "0.9", features = ["testing", "test-util"] }}
"#
    )
}

fn generate_lib_rs(name: &str, opts: &PluginScaffoldOptions) -> String {
    let struct_name = to_pascal_case(name);
    let focus = &opts.focus;
    let template = &opts.template;
    let headless = opts.headless;

    format!(
        r#"//! {name} - SeleniumBase-RS Plugin
//!
//! Architectural Focus: {focus}
//! Template: {template}
//! Headless Default: {headless}

use seleniumbase_rs::Result;
use seleniumbase_rs::sb_cdp::{{ContextOptions, LaunchOptions, Page}};

/// Core plugin controller for `{name}`.
#[derive(Debug, Clone)]
pub struct {struct_name} {{
    enabled: bool,
    headless: bool,
}}

impl Default for {struct_name} {{
    fn default() -> Self {{
        Self::new()
    }}
}}

impl {struct_name} {{
    /// Creates a new instance of the plugin.
    #[must_use]
    pub fn new() -> Self {{
        Self {{
            enabled: true,
            headless: {headless},
        }}
    }}

    /// Configures browser launch options for this plugin.
    pub fn configure_launch(&self, builder: &mut LaunchOptions) -> Result<()> {{
        if !self.enabled {{
            return Ok(());
        }}
        tracing::debug!(plugin = "{name}", "Applying plugin launch configurations");
        Ok(())
    }}

    /// Intercepts and decorates a newly created browser context.
    pub fn configure_context(&self, options: &mut ContextOptions) -> Result<()> {{
        if !self.enabled {{
            return Ok(());
        }}
        tracing::debug!(plugin = "{name}", "Applying context options");
        Ok(())
    }}

    /// Main automation task hook executed on a live browser page.
    pub async fn execute(&self, page: &Page) -> Result<()> {{
        tracing::info!(plugin = "{name}", "Executing plugin automation task");
        Ok(())
    }}
}}
"#
    )
}

fn generate_main_rs(name: &str, _opts: &PluginScaffoldOptions) -> String {
    let struct_name = to_pascal_case(name);
    format!(
        r#"//! CLI runner for {name}.

use {name}::{struct_name};
use seleniumbase_rs::sb_cdp::Browser;
use seleniumbase_rs::Result;

#[tokio::main]
async fn main() -> Result<()> {{
    tracing_subscriber::fmt::init();
    tracing::info!("Starting {name} CLI...");

    let plugin = {struct_name}::new();
    let browser = Browser::launch().await?;
    let page = browser.new_page().await?;

    plugin.execute(&page).await?;

    browser.close().await?;
    tracing::info!("{name} task finished successfully.");
    Ok(())
}}
"#
    )
}

fn generate_test_rs(name: &str, _opts: &PluginScaffoldOptions) -> String {
    let struct_name = to_pascal_case(name);
    format!(
        r#"use {name}::{struct_name};
use seleniumbase_rs::Result;

#[tokio::test]
async fn test_plugin_initialization() -> Result<()> {{
    let plugin = {struct_name}::new();
    assert!(format!("{{plugin:?}}").contains("{struct_name}"));
    Ok(())
}}
"#
    )
}

fn generate_readme(name: &str, opts: &PluginScaffoldOptions) -> String {
    format!(
        r#"# {name}

SeleniumBase-RS automation plugin.

## Overview
- **Focus**: `{}`
- **Template**: `{}`
- **Backend**: `{}`
- **Integrations**: `{}`
- **Target Env**: `{}`

## Development

```bash
# Build
cargo build

# Run unit and integration tests
cargo test

# Linting
cargo clippy --all-targets -- -D warnings
```
"#,
        opts.focus,
        opts.template,
        opts.backend.as_deref().unwrap_or("none"),
        opts.integrations.as_deref().unwrap_or("standalone"),
        opts.target_env
    )
}

fn generate_mise_toml(opts: &PluginScaffoldOptions) -> String {
    let extra_task = if opts.focus == "cli" {
        r#"
[tasks.run]
description = "Run the scaffolded CLI tool"
depends = ["build"]
run = "cargo run --"
"#
    } else if opts.headless {
        r#"
[tasks.verify]
description = "Check headless browser readiness via sbrs"
run = "sbrs doctor"
env = { SB_HEADLESS = "true" }
"#
    } else {
        r#"
[tasks.verify]
description = "Check browser readiness via sbrs"
run = "sbrs doctor"
"#
    };

    format!(
        r#"[tools]
rust = "1.100.1"

[tool_alias]
sbrs = "github:MustCodeAl/seleniumbase-rs"

[tasks.fmt]
description = "Check Rust formatting"
run = "cargo fmt --all -- --check"

[tasks.lint]
description = "Run clippy with warnings treated as errors"
sources = ["Cargo.toml", "src/**/*.rs", "tests/**/*.rs"]
run = "cargo clippy --all-targets -- -D warnings"

[tasks.build]
description = "Compile the plugin crate"
sources = ["Cargo.toml", "src/**/*.rs"]
run = "cargo build"

[tasks.test]
description = "Run plugin unit and integration tests"
depends = ["build"]
sources = ["Cargo.toml", "src/**/*.rs", "tests/**/*.rs"]
run = "cargo test"

[tasks.check]
description = "Run formatting, clippy lints, and tests"
depends = ["fmt", "lint", "test"]
{extra_task}"#
    )
}

fn generate_dockerfile(_opts: &PluginScaffoldOptions) -> String {
    r#"FROM rust:1.100-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo test --no-run
CMD ["cargo", "test"]
"#
    .to_string()
}

fn generate_docker_compose(name: &str) -> String {
    format!(
        r#"version: '3.8'
services:
  {name}:
    build: .
    volumes:
      - .:/app
    environment:
      - SB_HEADLESS=true
"#
    )
}

fn generate_gitlab_ci() -> String {
    r#"image: rust:1.100-bookworm

stages:
  - test

test:
  stage: test
  script:
    - cargo fmt --all -- --check
    - cargo clippy --all-targets -- -D warnings
    - cargo test
"#
    .to_string()
}

fn generate_github_actions() -> String {
    r#"name: CI
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: 1.100.0
          components: clippy, rustfmt
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo test
"#
    .to_string()
}

fn generate_cursorrules(name: &str, opts: &PluginScaffoldOptions) -> String {
    format!(
        r#"# Cursor Rules for {name}
You are an expert systems engineer developing the `{name}` plugin for seleniumbase-rs.

- Focus: {focus}
- Template: {template}
- Headless: {headless}

Follow strict Rust API Guidelines:
1. No `get_` getters (C-GETTER). Use `.name()` or `.name_mut()`.
2. Error handling: use `thiserror` and `Result<T, SeleniumBaseError>`. Never `unwrap()` in production.
3. No `unsafe` in library code.
4. Use `tracing` instead of `println!`.
5. Run tests with `cargo test`.
"#,
        focus = opts.focus,
        template = opts.template,
        headless = opts.headless
    )
}

fn generate_claude_md(name: &str, opts: &PluginScaffoldOptions) -> String {
    format!(
        r#"# CLAUDE.md - {name}

Commands:
- `cargo test`: Run tests
- `cargo clippy --all-targets -- -D warnings`: Lint check
- `cargo fmt --all -- --check`: Format check

Architecture:
- Plugin Name: `{name}`
- Focus: `{}`
- Template: `{}`
- Built on: `seleniumbase-rs`
"#,
        opts.focus, opts.template
    )
}

fn generate_agents_md(name: &str, _opts: &PluginScaffoldOptions) -> String {
    format!(
        r#"# AGENTS.md for {name}

Instructions for AI coding agents working on this plugin.

## Technical Rules
- No `get_` getters.
- Deterministic resource teardown.
- Tests before implementation.
- Zero clippy warnings.
- Run `cargo test` to verify.
"#
    )
}

fn generate_copilot_md(name: &str, _opts: &PluginScaffoldOptions) -> String {
    format!(
        r#"# COPILOT.md for {name}
Guidelines for GitHub Copilot / Codex working on `{name}` plugin.
"#
    )
}

fn generate_instructions(name: &str, root: &Path, opts: &PluginScaffoldOptions) -> String {
    let ide_cmd = match opts.ide.as_str() {
        "vscode" => "code",
        "cursor" => "cursor",
        "zed" => "zed",
        "clion" => "clion",
        "neovim" => "nvim",
        "helix" => "hx",
        _ => "cursor",
    };

    let env_cmd = match opts.dev_env.as_str() {
        "mise" => "mise install && cargo test",
        "docker" => "docker compose up --build",
        _ => "cargo test",
    };

    format!(
        r#"
================================================================================
  SELENIUMBASE-RS PLUGIN SCAFFOLDED: {name}
  Location: {path}
================================================================================

[Quick Start Instructions]
1. Navigate into the new plugin directory:
   cd {path}

2. Open in your IDE ({ide}):
   {ide_cmd} .

3. Development Setup ({dev_env}):
   {env_cmd}

4. AI Agent System Prompt ({ai_agent}):
   Copy and paste this into your AI coding assistant:
   -----------------------------------------------------------------------------
   You are an expert systems engineer developing the '{name}' plugin for
   seleniumbase-rs.
   - Architectural Focus: {focus}
   - Template Style: {template}
   - Backend Foundation: {backend}
   - Target Integrations: {integrations}
   - Headless Mode: {headless}
   - Deployment Target: {target_env}

   Strict Guidelines:
   * Follow Rust API Guidelines: zero unsafe, strong types, no get_ getters.
   * Deterministic cleanup: use RAII and sb.run().
   * Verify all code with: cargo test
   -----------------------------------------------------------------------------
================================================================================
"#,
        name = name,
        path = root.display(),
        ide = opts.ide,
        ide_cmd = ide_cmd,
        dev_env = opts.dev_env,
        env_cmd = env_cmd,
        ai_agent = opts.ai_agent,
        focus = opts.focus,
        template = opts.template,
        backend = opts.backend.as_deref().unwrap_or("none"),
        integrations = opts.integrations.as_deref().unwrap_or("standalone"),
        headless = opts.headless,
        target_env = opts.target_env
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_default_plugin_files() {
        let opts = PluginScaffoldOptions::new("my-custom-plugin");
        let (files, instructions) = render_plugin_files(&opts).unwrap();
        assert!(files.iter().any(|(p, _)| p.ends_with("Cargo.toml")));
        assert!(files.iter().any(|(p, _)| p.ends_with("src/lib.rs")));
        assert!(files
            .iter()
            .any(|(p, _)| p.ends_with("tests/plugin_test.rs")));
        assert!(files.iter().any(|(p, _)| p.ends_with("README.md")));
        assert!(instructions.contains("SELENIUMBASE-RS PLUGIN SCAFFOLDED: my-custom-plugin"));
    }

    #[test]
    fn render_cli_focus_includes_main_rs() {
        let mut opts = PluginScaffoldOptions::new("cli-tool");
        opts.focus = "cli".to_string();
        let (files, _) = render_plugin_files(&opts).unwrap();
        assert!(files.iter().any(|(p, _)| p.ends_with("src/main.rs")));
    }

    #[test]
    fn render_mise_dev_env_includes_tasks_and_alias() {
        let mut opts = PluginScaffoldOptions::new("mise-plugin");
        opts.dev_env = "mise".to_string();
        opts.headless = true;
        let (files, instructions) = render_plugin_files(&opts).unwrap();
        let (_, content) = files
            .iter()
            .find(|(p, _)| p.ends_with(".mise.toml"))
            .expect(".mise.toml generated");
        assert!(content.contains("rust = \"1.100.1\""));
        assert!(content.contains("[tool_alias]"));
        assert!(content.contains("[tasks.fmt]"));
        assert!(content.contains("[tasks.lint]"));
        assert!(content.contains("[tasks.test]"));
        assert!(content.contains("[tasks.check]"));
        assert!(content.contains("[tasks.verify]"));
        assert!(content.contains("SB_HEADLESS = \"true\""));
        assert!(instructions.contains("mise install"));
    }

    #[test]
    fn invalid_names_are_rejected() {
        for bad_name in [
            "",
            "  ",
            "../escaped",
            "sub/dir",
            "a\\b",
            ".hidden",
            "-flag",
        ] {
            let opts = PluginScaffoldOptions::new(bad_name);
            assert!(render_plugin_files(&opts).is_err());
        }
    }
}
