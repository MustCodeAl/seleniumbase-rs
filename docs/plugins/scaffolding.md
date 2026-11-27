---
title: "Plugin Scaffolding"
sidebarTitle: "Scaffold a plugin"
description: "Generate a ready-to-build plugin crate with sbrs plugin new, the Rust plugin_scaffold API, or the dev MCP server."
---

{/* layout-tree-component:start */}

export const LayoutTree = ({ children }) => (
  <div className="not-prose layout-tree" style={{ margin: "1.25rem 0", fontFamily: "var(--font-sans)", fontSize: "14px", color: "var(--text-secondary)" }}>
    <style>{`
      .layout-tree summary { list-style: none; }
      .layout-tree summary::-webkit-details-marker { display: none; }
      .layout-tree .lt-row:hover { background-color: var(--surface-subtle); }
      .layout-tree .lt-fold { color: #d5a910; }
      .dark .layout-tree .lt-fold { color: #ffd452; }
      .layout-tree .lt-fold-closed { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-open { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-closed { display: flex; }
      .layout-tree .lt-hl { color: var(--primary); background-color: color-mix(in srgb, var(--primary) 8%, transparent); }
      .dark .layout-tree .lt-hl { color: var(--primary-light); }
      .layout-tree .lt-ic-gray { color: #84848a; }
      .dark .layout-tree .lt-ic-gray { color: #adadb1; }
      .layout-tree .lt-ic-blue { color: #1a85d4; }
      .dark .layout-tree .lt-ic-blue { color: #69b1ff; }
      .layout-tree .lt-ic-cyan { color: #1ca1c7; }
      .dark .layout-tree .lt-ic-cyan { color: #68cdf2; }
      .layout-tree .lt-ic-green { color: #199f43; }
      .dark .layout-tree .lt-ic-green { color: #5ecc71; }
      .layout-tree .lt-ic-indigo { color: #693acf; }
      .dark .layout-tree .lt-ic-indigo { color: #9d6afb; }
      .layout-tree .lt-ic-orange { color: #d47628; }
      .dark .layout-tree .lt-ic-orange { color: #ffa359; }
      .layout-tree .lt-ic-pink { color: #d32a61; }
      .dark .layout-tree .lt-ic-pink { color: #ff678d; }
      .layout-tree .lt-ic-purple { color: #a631be; }
      .dark .layout-tree .lt-ic-purple { color: #d568ea; }
      .layout-tree .lt-ic-red { color: #d52c36; }
      .dark .layout-tree .lt-ic-red { color: #ff6762; }
      .layout-tree .lt-ic-yellow { color: #d5a910; }
      .dark .layout-tree .lt-ic-yellow { color: #ffd452; }
    `}</style>
    {children}
  </div>
)

export const LayoutRow = ({ icon, name, note, highlight = false, button = false }) => (
  <div className={highlight ? "lt-row lt-hl" : "lt-row"} style={{ display: "flex", alignItems: "flex-start", gap: "6px", minHeight: "30px", padding: "5px 8px", margin: "0 2px", borderRadius: "6px", lineHeight: "20px", cursor: button ? "pointer" : "default" }}>
    <span style={{ width: "16px", height: "20px", flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", color: highlight ? "inherit" : "var(--text-muted)" }}>{icon}</span>
    <span style={{ whiteSpace: "nowrap", fontFamily: "var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)", fontSize: "13px", fontWeight: 600, color: highlight ? "inherit" : "var(--text-primary)" }}>{name}</span>
    {note ? <span style={{ color: "var(--text-muted)", fontSize: "13px", fontStyle: "italic", marginLeft: "10px", opacity: 0.9 }}>{note}</span> : null}
  </div>
)

export const LayoutFolderIcon = ({ kind }) => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    {kind === "empty" ? (
      <path fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" d="M2 4.2A1.2 1.2 0 0 1 3.2 3h3.1c.32 0 .62.13.85.35L8.4 4.6h4.4A1.2 1.2 0 0 1 14 5.8v6a1.2 1.2 0 0 1-1.2 1.2H3.2A1.2 1.2 0 0 1 2 11.8z" />
    ) : kind === "open" ? (
      <g fill="currentColor">
        <path opacity=".5" d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H13a1.5 1.5 0 0 1 1.5 1.5V7H4.2a1.5 1.5 0 0 0-1.4.95L1.5 11.3z" />
        <path d="M4.2 8h10.6a.7.7 0 0 1 .65.95l-1.6 4.1a1.5 1.5 0 0 1-1.4.95H2.2a.7.7 0 0 1-.65-.95l1.25-3.2A1.5 1.5 0 0 1 4.2 8z" />
      </g>
    ) : (
      <g fill="currentColor">
        <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H1.5z" />
        <path opacity=".6" d="M1.5 4.5V12A1.5 1.5 0 0 0 3 13.5h10a1.5 1.5 0 0 0 1.5-1.5V6A1.5 1.5 0 0 0 13 4.5z" />
      </g>
    )}
  </svg>
)

export const LayoutFileIcon = () => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    <path fill="currentColor" opacity=".5" d="M8 1v3a3 3 0 0 0 3 3h3v5.5a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 2 12.5v-9A2.5 2.5 0 0 1 4.5 1z" />
    <path fill="currentColor" d="M9.5 1a.5.5 0 0 1 .354.146l4 4A.5.5 0 0 1 14 5.5V6h-3a2 2 0 0 1-2-2V1z" />
  </svg>
)

export const LayoutDir = ({ name, note, open = false, highlight = false, children }) =>
  children ? (
    <details open={open}>
      <summary>
        <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><span className="lt-fold-open" style={{ display: "flex" }}><LayoutFolderIcon kind="open" /></span><span className="lt-fold-closed"><LayoutFolderIcon kind="closed" /></span></span>} name={name} note={note} highlight={highlight} button />
      </summary>
      <div style={{ marginLeft: "17.5px", borderLeft: "1px solid var(--border-subtle)", paddingLeft: "6px" }}>
        {children}
      </div>
    </details>
  ) : (
    <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><LayoutFolderIcon kind="empty" /></span>} name={name} note={note} highlight={highlight} />
  )

export const layoutIconColor = (name) => {
  const n = String(name).toLowerCase()
  const ext = n.includes(".") ? n.split(".").pop() : ""
  if (n === "dockerfile" || n.startsWith("dockerfile.") || n.startsWith("docker-compose")) return "blue"
  if (["rs", "json", "jsonc", "html", "htm", "zip", "gz", "tgz", "crx", "xpi", "swift", "svg"].includes(ext)) return "orange"
  if (["ts", "tsx", "mts", "cts", "py"].includes(ext)) return "blue"
  if (["js", "mjs", "cjs", "jsx"].includes(ext)) return "yellow"
  if (["md", "mdx", "sh", "bash", "zsh", "fish", "ps1"].includes(ext)) return "green"
  if (["css", "scss"].includes(ext)) return "indigo"
  if (["png", "jpg", "jpeg", "gif", "webp", "ico"].includes(ext)) return "pink"
  if (ext === "go") return "cyan"
  return "gray"
}

export const LayoutFile = ({ name, note, highlight = false }) => (
  <LayoutRow icon={<span className={highlight ? "" : "lt-ic-" + layoutIconColor(name)} style={{ display: "flex" }}><LayoutFileIcon /></span>} name={name} note={note} highlight={highlight} />
)

{/* layout-tree-component:end */}

`sbrs plugin new` generates a ready-to-build Rust plugin crate with tests, optional CLI entry points, CI or container configurations, and AI coding assistant rules. The same generator is exposed programmatically in `seleniumbase_rs::cli::plugin_scaffold` and as a read-only inspection tool on the `dev` MCP server (`sbrs-mcp --server dev`).

For low-level `Plugin` trait hooks (`before_launch`, `on_browser`, `on_page_created`, `on_close`) in a standalone `seleniumbase-rs-plugin` crate, see [Writing your own plugin](/plugins/writing-your-own).

<Card color="#1a85d4" title="Quick reference" icon="wand-magic-sparkles">

- CLI subcommand: `sbrs plugin new <NAME> [OPTIONS]`
- Rust module: `seleniumbase_rs::cli::plugin_scaffold`
- MCP `dev` tool: `scaffold_plugin` (read-only preview)
- Templates: `library` (default) or `cli` (`src/main.rs` + `[[bin]]`)
- Dev environments: `cargo` (default), `mise`, `docker`, `gitlab-ci`, `github-actions`
- AI agent rule files: `none` (default), `cursor`, `claude-code`, `gemini`, `antigravity`, `codex`

</Card>

## CLI command (`sbrs plugin new`)

```bash Terminal
sbrs plugin new <NAME> [OPTIONS]
```

`<NAME>` is trimmed, lowercased, and converted so spaces become hyphens (`"My Plugin"` becomes `my-plugin`). Names that are empty, start with `.` or `-`, or contain `/`, `\`, or `..` are rejected before touching the filesystem. If the destination directory already exists and is non-empty, `sbrs plugin new` exits with an error rather than overwriting existing files.

### Flags and options

| Flag | Default | Accepted / typical values | Effect |
|---|---|---|---|
| `<NAME>` | (required) | e.g. `rate-limiter`, `site-auditor` | Crate and directory name (unless `--out-dir` is given); converted to `PascalCase` for the controller struct. |
| `--template <TEMPLATE>` | `library` | `library`, `cli` | `library` creates a library crate; `cli` adds `src/main.rs` and a `[[bin]]` section in `Cargo.toml`. |
| `--backend <BACKEND>` | unset | e.g. `cdp`, `webdriver` | Appends `"{backend}"` to the `seleniumbase-rs` dependency feature list in `Cargo.toml` and records it in `README.md`. |
| `--focus <FOCUS>` | `general` | `general`, `cli`, `scraping` | Records architectural focus in docs and prompts; `cli` also generates `src/main.rs` and `[[bin]]`. |
| `--integrations <INTEGRATIONS>` | unset | e.g. `mcp`, `ci` | Documents target integrations in `README.md` (`standalone` when unset) and in the quick-start prompt. |
| `--headless` | `false` | boolean flag (`--headless`, `--no-headless`) | Sets the default `headless` boolean in the generated `{StructName}::new()` constructor. |
| `--dev-env <DEV_ENV>` | `cargo` | `cargo`, `mise`, `docker`, `gitlab-ci`, `github-actions` | Writes matching toolchain, container, or CI pipeline files alongside the crate. |
| `--ai-agent <AI_AGENT>` | `none` | `none`, `cursor`, `claude-code`, `gemini`, `antigravity`, `codex` | Generates a repository rule file tailored to the selected coding agent. |
| `--ide <IDE>` | `cursor` | `cursor`, `vscode`, `zed`, `clion`, `neovim`, `helix` | Chooses the editor launch command (`cursor`, `code`, `zed`, `clion`, `nvim`, `hx`) in the printed quick-start guide. |
| `--target-env <TARGET_ENV>` | `local` | `local`, `docker`, `cloud` | Records the deployment target in `README.md` and quick-start instructions. |
| `--out-dir <DIR>` | `<NAME>` | any directory path | Writes the scaffolded crate into `<DIR>` instead of `./<NAME>`. |

### Examples

<Tabs>

<Tab title="Library plugin" icon="cube">

Generate a library plugin focused on scraping with headless execution enabled by default:

```bash Terminal
sbrs plugin new price-monitor \
  --template library \
  --focus scraping \
  --headless \
  --ide vscode
```

</Tab>

<Tab title="CLI plugin" icon="terminal">

Generate an executable plugin crate (`src/lib.rs` + `src/main.rs`) in a custom workspace folder:

```bash Terminal
sbrs plugin new link-auditor \
  --template cli \
  --focus cli \
  --headless \
  --integrations ci \
  --out-dir crates/link-auditor
```

</Tab>

<Tab title="CI and containers" icon="server">

Include GitHub Actions, GitLab CI, Docker Compose, or `mise` task runners at creation time:

```bash Terminal
# GitHub Actions (.github/workflows/ci.yml with Rust 1.100.0, rustfmt, clippy, cargo test)
sbrs plugin new session-guard --dev-env github-actions

# GitLab CI (.gitlab-ci.yml using rust:1.100-bookworm)
sbrs plugin new header-check --dev-env gitlab-ci

# Docker + Compose (Dockerfile + docker-compose.yml with SB_HEADLESS=true)
sbrs plugin new batch-runner --template cli --dev-env docker --target-env docker

# mise (.mise.toml with rust = "1.100.0" and build/test/check tasks)
sbrs plugin new dom-snapshot --dev-env mise
```

</Tab>

<Tab title="AI agent rules" icon="robot">

Generate agent instruction files enforcing `seleniumbase-rs` Rust API conventions (`C-GETTER`, `thiserror`, `tracing`, no `unwrap()`, no `unsafe`):

```bash Terminal
# Writes AGENTS.md (for Gemini CLI or Antigravity)
sbrs plugin new auth-helper --ai-agent antigravity --ide zed

# Writes CLAUDE.md
sbrs plugin new form-filler --ai-agent claude-code

# Writes .cursorrules
sbrs plugin new cookie-sync --ai-agent cursor --ide cursor

# Writes COPILOT.md
sbrs plugin new table-parser --ai-agent codex --ide vscode
```

</Tab>

</Tabs>

## Generated directory structure

<LayoutTree>
  <LayoutDir name="my-plugin" open note="root_dir (--out-dir or cleaned plugin name)">
    <LayoutFile name="Cargo.toml" highlight note="Edition 2024 package, seleniumbase-rs dependencies, optional [[bin]]" />
    <LayoutFile name="README.md" note="Overview metadata and cargo build/test/clippy commands" />
    <LayoutDir name="src" open note="plugin source">
      <LayoutFile name="lib.rs" highlight note="PascalCase plugin struct (new, configure_launch, configure_context, execute)" />
      <LayoutFile name="main.rs" note="optional: generated when --template cli or --focus cli" />
    </LayoutDir>
    <LayoutDir name="tests" open note="integration tests">
      <LayoutFile name="plugin_test.rs" note="tokio::test initialization smoke test" />
    </LayoutDir>
    <LayoutFile name=".mise.toml" note="optional: generated when --dev-env mise" />
    <LayoutFile name="Dockerfile" note="optional: generated when --dev-env docker" />
    <LayoutFile name="docker-compose.yml" note="optional: generated when --dev-env docker" />
    <LayoutFile name=".gitlab-ci.yml" note="optional: generated when --dev-env gitlab-ci" />
    <LayoutDir name=".github" note="optional: generated when --dev-env github-actions">
      <LayoutDir name="workflows" open>
        <LayoutFile name="ci.yml" note="fmt, clippy (-D warnings), and cargo test on Rust 1.100.0" />
      </LayoutDir>
    </LayoutDir>
    <LayoutFile name="AGENTS.md" note="optional: --ai-agent gemini or antigravity (or .cursorrules / CLAUDE.md / COPILOT.md)" />
  </LayoutDir>
</LayoutTree>

### Generated files in detail

- **`Cargo.toml`**: Edition `2024`, MIT license, `seleniumbase-rs` (`features = ["plugins-builtin"]` plus any `--backend` feature), `tokio`, `serde`, `serde_json`, `tracing`, `tracing-subscriber`, `thiserror`, and `[dev-dependencies]` with `seleniumbase-rs` (`features = ["testing", "test-util"]`). Adds `[[bin]]` pointing to `src/main.rs` when `--template cli` or `--focus cli` is active.
- **`src/lib.rs`**: Defines a `#[derive(Debug, Clone)]` controller struct named in `PascalCase` (for example, `my-plugin` produces `pub struct MyPlugin`), implementing `Default`, `new()`, `configure_launch(&self, builder: &mut LaunchOptions) -> Result<()>`, `configure_context(&self, options: &mut ContextOptions) -> Result<()>`, and `pub async fn execute(&self, page: &Page) -> Result<()>`.
- **`src/main.rs`** (CLI mode only): Initializes `tracing_subscriber`, constructs the plugin controller, launches a `sb_cdp::Browser`, opens a `Page`, runs `plugin.execute(&page).await?`, and closes the browser cleanly.
- **`tests/plugin_test.rs`**: Async `#[tokio::test]` verifying constructor and `Debug` invariants.
- **Dev environment files**:
  - `mise`: `.mise.toml` pinning `rust = "1.100.0"` with `build`, `test`, and `check` tasks.
  - `docker`: `Dockerfile` (`rust:1.100-bookworm`) and `docker-compose.yml` (`SB_HEADLESS=true`).
  - `gitlab-ci`: `.gitlab-ci.yml` running `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
  - `github-actions`: `.github/workflows/ci.yml` running the same three checks on `ubuntu-latest` with `1.100.0`.
- **AI agent rule files**: `.cursorrules` (`cursor`), `CLAUDE.md` (`claude-code`), `AGENTS.md` (`gemini` or `antigravity`), or `COPILOT.md` (`codex`).

When `sbrs plugin new` completes, it prints quick-start terminal commands (`cd`, IDE launch command, environment test command, and a ready-to-paste system prompt) to standard output.

## Programmatic Rust API

Tools and custom generators can render or write plugin scaffolds directly through `seleniumbase_rs::cli::plugin_scaffold`.

### Types and functions

```rust
pub struct PluginScaffoldOptions {
    pub name: String,
    pub template: String,
    pub backend: Option<String>,
    pub focus: String,
    pub integrations: Option<String>,
    pub headless: bool,
    pub dev_env: String,
    pub ai_agent: String,
    pub ide: String,
    pub target_env: String,
    pub out_dir: Option<PathBuf>,
}

impl PluginScaffoldOptions {
    pub fn new(name: impl Into<String>) -> Self;
}

pub struct ScaffoldResult {
    pub name: String,
    pub root_dir: PathBuf,
    pub files_created: Vec<PathBuf>,
    pub instructions: String,
}

#[non_exhaustive]
pub enum PluginScaffoldError {
    InvalidName { name: String, reason: String },
    DirectoryNotEmpty(PathBuf),
    Io { path: PathBuf, source: std::io::Error },
}

pub fn render_plugin_files(
    opts: &PluginScaffoldOptions,
) -> Result<(Vec<(PathBuf, String)>, String), PluginScaffoldError>;

pub fn scaffold_plugin(
    opts: &PluginScaffoldOptions,
) -> Result<ScaffoldResult, PluginScaffoldError>;
```

- **`render_plugin_files(&opts)`**: Pure in-memory generator. Validates `opts.name` and returns `(files, instructions)` where `files` is a `Vec<(PathBuf, String)>` of relative/target paths and file contents. Performs no filesystem I/O.
- **`scaffold_plugin(&opts)`**: Checks that the target directory does not already exist with entries (returning `PluginScaffoldError::DirectoryNotEmpty`), calls `render_plugin_files`, creates parent directories, writes every file to disk, and returns a `ScaffoldResult`.

### Example

```rust,no_run
use std::path::PathBuf;
use seleniumbase_rs::cli::plugin_scaffold::{
    PluginScaffoldOptions, render_plugin_files, scaffold_plugin,
};

fn main() -> Result<(), seleniumbase_rs::cli::plugin_scaffold::PluginScaffoldError> {
    let mut opts = PluginScaffoldOptions::new("audit-headers");
    opts.focus = "scraping".to_owned();
    opts.headless = true;
    opts.dev_env = "github-actions".to_owned();
    opts.ai_agent = "claude-code".to_owned();
    opts.out_dir = Some(PathBuf::from("crates/audit-headers"));

    // Preview generated files in memory without touching disk:
    let (preview_files, _instructions) = render_plugin_files(&opts)?;
    assert!(preview_files.iter().any(|(path, _)| path.ends_with("Cargo.toml")));

    // Write the scaffolded crate to disk:
    let result = scaffold_plugin(&opts)?;
    println!("Created {} files in {}", result.files_created.len(), result.root_dir.display());
    Ok(())
}
```

## MCP `dev` server tool (`scaffold_plugin`)

When an AI coding assistant is connected to the browserless [DevTools MCP server](/ecosystem/mcp/devtools) (`sbrs-mcp --server dev`), it can call the `scaffold_plugin` tool to preview every file `sbrs plugin new` would produce.

Because `scaffold_plugin` has `Effect::Inspect`, it calls `render_plugin_files` in memory and **writes nothing to disk**. The response contains each file framed under `=== <path> ===` followed by the quick-start instructions, allowing the agent to inspect, tailor, and write the files using its own workspace tools.

### Tool parameters

| Parameter | Type | Required | Default | Allowed values |
|---|---|---|---|---|
| `name` | `string` | yes | | Valid crate name (e.g. `"my-plugin"`) |
| `template` | `string` | no | `"library"` | `"library"`, `"cli"` |
| `backend` | `string` | no | | e.g. `"cdp"`, `"webdriver"` |
| `focus` | `string` | no | `"general"` | e.g. `"general"`, `"cli"`, `"scraping"` |
| `integrations` | `string` | no | | e.g. `"mcp"`, `"ci"` |
| `headless` | `boolean` | no | `false` | `true`, `false` |
| `dev_env` | `string` | no | `"cargo"` | `"cargo"`, `"mise"`, `"docker"`, `"gitlab-ci"`, `"github-actions"` |
| `ai_agent` | `string` | no | `"none"` | `"none"`, `"cursor"`, `"claude-code"`, `"gemini"`, `"antigravity"`, `"codex"` |
| `ide` | `string` | no | `"cursor"` | `"cursor"`, `"vscode"`, `"zed"`, `"clion"`, `"neovim"`, `"helix"` |
| `target_env` | `string` | no | `"local"` | e.g. `"local"`, `"docker"`, `"cloud"` |
| `out_dir` | `string` | no | `<name>` | Destination directory path |

### Example MCP call arguments

```json
{
  "name": "token-inspector",
  "template": "library",
  "focus": "scraping",
  "headless": true,
  "dev_env": "github-actions",
  "ai_agent": "antigravity",
  "ide": "zed"
}
```
