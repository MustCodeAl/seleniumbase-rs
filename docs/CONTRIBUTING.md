# Contributing to SeleniumBase-rs

Thank you for helping make `seleniumbase-rs` a powerful, idiomatic Rust
browser-automation framework. Whether you are fixing a bug, adding a new stealth
evasion, writing a tutorial, or improving the MCP server, this guide will get
you oriented quickly.

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Quick Start](#quick-start)
3. [Development Environment](#development-environment)
4. [Project Layout](#project-layout)
5. [Architecture at a Glance](#architecture-at-a-glance)
6. [Code Style](#code-style)
7. [Error Handling](#error-handling)
8. [Tracing and Observability](#tracing-and-observability)
9. [Testing](#testing)
10. [Parity with Python SeleniumBase](#parity-with-python-seleniumbase)
11. [Adding a Stealth Evasion Provider](#adding-a-stealth-evasion-provider)
12. [Adding a Masking Mode or Fingerprint Field](#adding-a-masking-mode-or-fingerprint-field)
13. [Adding a Macro](#adding-a-macro)
14. [Adding an MCP Tool](#adding-an-mcp-tool)
15. [Adding an `sbase` Subcommand](#adding-an-sbase-subcommand)
16. [Adding a Profile Payload Field](#adding-a-profile-payload-field)
17. [Binary Patching](#binary-patching)
18. [Writing Documentation](#writing-documentation)
19. [Working with AI Assistants](#working-with-ai-assistants)
20. [Opening a Pull Request](#opening-a-pull-request)
21. [Publishing](#publishing)
22. [Licensing](#licensing)

## Code of Conduct

- Be respectful and constructive.
- Assume good intent.
- Focus feedback on code and design, not individuals.
- Do not include third-party anti-detect product names, trademarks, or proprietary
  payload examples in code, docs, or tests. Use generic terminology such as
  "external profile", "anti-detect payload", or "browser fingerprint".

## Quick Start

```bash
# Clone the crate
git clone https://github.com/MustCodeAl/seleniumbase-rs.git
cd seleniumbase-rs

# The checks every change must pass (the same ones CI runs)
cargo fmt --all -- --check
cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings
cargo test --features "mcp-server test-util"
cargo clippy --features scrape --lib -- -D warnings

# Build the mdBook
mdbook build
```

Features, binaries, the Dockerfile and build-time tips are in
[docs/building.md](docs/building.md).

Read `CLAUDE.md` once even if you are not using an AI assistant: it is a short map
of the modules, the commands and the things that have bitten before.

Three environment flags keep builds cheap on a small machine, and they let
several checkouts share compiled dependencies:

```bash
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
```

Do not run two heavy cargo commands at once in the same target directory (cargo
serialises them with a lock), and do not share one `CARGO_TARGET_DIR` between
several checkouts of this repository: they overwrite each other's compiled
library and doctests then fail with "extern location does not exist".

## Development Environment

With [mise](https://mise.jdx.dev), `mise install` sets up the tools below from
`mise.toml` (Rust 1.99 with clippy and rustfmt, Python 3.14 for the parity
scripts, Node for the Tauri example, cmake, mdBook, just and cargo-deny), and
`mise run gate` runs the checks a change has to pass; `mise tasks` lists the
rest (`chrome`, `scrape-gate`, `imp-gate`, `parity-docs`, `docs`, `smoke`).

### Required

- [Rust](https://rustup.rs/) 1.99 or newer (stable channel); `rust-version` in
  `Cargo.toml` is the newest stable release and the `msrv` CI job enforces it.
- `cargo`, `rustfmt` and `clippy`.
- Python 3.10+ only for the scripts in `tools/` (the parity tables). The crate
  itself, every feature included, needs no Python.

### Optional but recommended

- [mdBook](https://rust-lang.github.io/mdBook/) for documentation:
  `cargo install mdbook`.
- [cargo-nextest](https://nexte.st/) for faster test runs
  (`cargo install cargo-nextest`; it does not run doctests, so also run
  `cargo test --doc`). `.config/nextest.toml` has a one-thread `chrome` group
  for the tests that need a real Chrome.
- `cargo-deny` and `cargo-audit` for the dependency checks, and `just` for the
  recipes in the `justfile`.
- `uv` to regenerate the parity docs (`just parity-docs`).
- A Chrome (and, for the WebDriver engine, a matching chromedriver: `sbase
  install`) for the tests marked `#[ignore]`.
- `libwebkit2gtk-4.1-dev` (Linux) or Xcode (macOS) if you build the Tauri
  example.

## Project Layout

```text
seleniumbase-rs/
├── src/
│   ├── lib.rs                 # Crate root and public re-exports
│   ├── api/                   # BaseCase (base_case.rs, base_case_impls/), capability
│   │                          # traits, recorder, scenario, run_settings, charts, tours
│   ├── browser/               # BrowserConfig, launcher, session, options/, driver matching
│   ├── sb_cdp/                # The pure CDP engine: Browser, Page, Locator, pool, mock
│   ├── stealth/               # Fingerprint, evasion providers, humanize, patcher, reactor
│   ├── wire/                  # (feature scrape/impersonate) pure TLS/HTTP2 wire formats
│   ├── scrape/                # (feature scrape) challenge, egress, jar, ledger, passport, runtime
│   ├── impersonate/           # (feature impersonate) client, http_session/, mitm/,
│   │                          # stealth_session/, tls_server/ (ephemeral CA), capture_h2
│   ├── mcp/                   # MCP servers: cdp, driver, sb; developer tools in devtools.rs
│   ├── cli/                   # sbase: bin/sbase.rs (clap), commands.rs, scripts/
│   ├── behave/                # Gherkin/BDD runner
│   ├── plugins/               # Reporting and logging plugins
│   ├── config/, core/, common/, storage/, utilities/, utils/, profile_payloads/
│   ├── error.rs               # SeleniumBaseError: hints, exit codes, selector advice
│   └── bin/mcp_server.rs      # The seleniumbase-mcp binary
├── tests/                     # Integration tests (tests/golden is compiled in)
├── benches/                   # cpu, cdp_latency
├── examples/                  # Runnable examples
├── parity/                    # api.toml: the status of every Python API
├── docs/                      # mdBook sources, help pages, design/ notes
└── CLAUDE.md                  # The module map for coding assistants
```

## Architecture at a Glance

`seleniumbase-rs` is organized in layers:

1. **Two engines** — the WebDriver engine (`BrowserConfig`, `BrowserSession`
   and the `BaseCase` state machine over `thirtyfour`) and the pure CDP engine
   (`sb_cdp`), which talks to Chrome directly with no driver. They share the
   capability traits where it makes sense, not a common base class.
2. **API layer** — Capability traits (`BrowserApi`, `ElementApi`,
   `AssertionApi`, `ScreenshotApi`) and the `base_case_impls` modules. New
   helpers should be added to the smallest applicable module and then exposed
   through the trait or `BaseCase` impl.
3. **Stealth layer** — `Fingerprint` + `StealthFlags` describe what to spoof;
   `EvasionProvider`s generate the JavaScript bootstrap; `ChromedriverPatcher`
   mutates the local driver binary; `StealthReactor` intercepts network requests
   via CDP.
4. **Tooling layer** — `sbase` CLI, the MCP server, the behave runner, and the
   Python importer.

When adding a feature, try to keep each layer independent. For example, a new
fingerprint provider should not depend on `BaseCase`, and a new CLI command
should not contain browser logic directly.

## Code Style

- Follow the Rust style enforced by `cargo fmt`.
- Keep Clippy clean with `-D warnings` (command in [Quick Start](#quick-start)).
  `src/mcp` and `src/sb_cdp` are held to the stricter pedantic lint set. An
  `#[allow(...)]` needs a `reason = "..."`, and a function over 100 lines fails
  the lint, so split it.
- Prefer explicit types on public APIs; this helps Copilot and other agents infer intent.
- Use `tracing` for logs instead of `println!` in library code.
- Avoid `unsafe` unless absolutely necessary and clearly documented.
- Keep functions small and focused. If a `BaseCase` helper grows beyond ~50
  lines, add it to the appropriate `base_case_impls` module.
- Prefer `?` and `crate::Result<T>` over manual `match` propagation.
- Use `thiserror` for new error variants and keep variants structured (avoid
  generic string payloads).

## Error Handling

`seleniumbase-rs` uses a single `SeleniumBaseError` enum in `src/error.rs`.

- Add a new variant when a failure mode needs special handling, a unique hint,
  or stable down-stream matching.
- Use helper constructors such as `SeleniumBaseError::element_not_found` instead
  of formatting strings by hand.
- Mark transient failures (network, driver not ready) by returning `true` from
  `is_transient()`.
- Provide actionable hints via `hint()`.
- Instrument error paths with `tracing::error!` and include context with
  `ResultExt::sb_context()`:

```rust
use seleniumbase_rs::{ResultExt, SeleniumBaseError};

some_fallible_op()
    .sb_context("downloading chromedriver")?;

SeleniumBaseError::driver_not_ready("chromedriver").log();
```

See `docs/ABI_API.md` and `docs/help/common_problems.md` for the error design
philosophy.

## Tracing and Observability

The crate uses `tracing` throughout. Common patterns:

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(self), fields(url = %url))]
pub async fn open(&mut self, url: &str) -> crate::Result<()> {
    info!("opening page");
    // ...
}
```

Feature flags control subscriber output:

| Feature | Effect |
|---|---|
| `full-tracing` | Enables detailed span lifecycle logging. |
| `error-backtrace` | Captures backtraces on `SeleniumBaseError`. |

Run the CLI with tracing enabled:

```bash
RUST_LOG=info cargo run --bin sbase --features full-tracing -- open https://example.com
```

See `docs/tutorials/tracing.md` for subscriber configuration examples.

## Testing

- Write the test first when you can: the signature with a `todo!()` body, the
  tests, then the code. A bug fix gets a test that fails without the fix.
- Add unit tests for pure logic: selectors, fingerprint generation, macros,
  providers, JSON parsing.
- Use `#[tokio::test]` for async helpers that do not require a browser. The pure
  CDP engine has a mock browser (`Browser::new_mocked`, `MockCtrl`), so waits,
  retries and error paths can be tested without Chrome (see `tests/sb_cdp_*.rs`).
- Tests that need a real Chrome are named `*_chrome.rs` (plus `browser_smoke`)
  and are `#[ignore]`d. Run them one test binary at a time:
  `cargo test --test mcp_cdp_chrome -- --ignored`.
- A test that fakes `java` or another executable must run the script once
  (`--warm`) before timing it: macOS can take seconds to run a freshly written
  script the first time.
- When writing doctests, mark browser-dependent snippets with `no_run` or
  `ignore`. Tutorials in `docs/tutorials/` are not compiled, so keep their code
  true by hand.

### Verification matrix (run before every PR)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings
cargo test --features "mcp-server test-util"
cargo clippy --features scrape --lib -- -D warnings   # the scrape feature
cargo clippy --features impersonate --lib -- -D warnings  # the impersonate feature (cmake; builds BoringSSL)
cargo check --features turso --all-targets            # code that only builds with turso
cargo deny check && cargo audit                        # when Cargo.lock changed
mdbook build                                           # when docs changed
```

Count failures from the exit code and the `test result:` lines, not from a
filtered search of the output. A change that touches `Cargo.toml` must keep the
`Dockerfile` skeleton step in step: a new `[[bin]]`, `[[bench]]`, `[[example]]`
or `[[test]]` must be listed there, and `tests/dockerfile_skeleton.rs` fails
if it is not.

`cargo deny check` gates advisories, licenses, banned crates, and dependency
sources against `deny.toml`. Every advisory that is ignored there carries a
written rationale, so if a new one appears, either upgrade the dependency or
add an entry explaining why the risk is acceptable — do not widen the ignore
list silently.

The crate declares `rust-version = "1.99"`, the newest stable release when it was
last raised, which the `msrv` CI job enforces with that exact toolchain. The
floor is a choice to track current stable, not something the crate's own source
needs, so when it moves, bump `rust-version`, the `msrv` job, and the
`Dockerfile` base image together.

On macOS, if a test binary cannot load `libpython`, prefix the test commands with:

```bash
DYLD_LIBRARY_PATH="$HOME/.local/share/mise/installs/python/3.14.6/lib" cargo test --features "mcp-server test-util"
```

## Parity with Python SeleniumBase

The project's goal is that every API of Python SeleniumBase 4.55.2 (and of its
MCP server) exists here or has a stated reason why not.

- `parity/api.toml` records one entry per upstream API with a `status`:
  `implemented` (Rust code and a test), `composed` (existing Rust calls already
  do it; name them in `rust`), `not-applicable` (with a `reason` specific to that
  API) or `planned`. Never mark something `implemented` without code and a test;
  `tests/parity.rs` checks that every Rust path in the manifest exists.
- After editing the manifest, regenerate `docs/parity.md` with `just parity-docs`
  (or `uv run tools/parity/render_docs.py`) and commit both. `just parity-check`
  verifies them.
- `parity/upstream.toml` and `docs/UPSTREAM_SYNC.md` describe how to move to a
  newer upstream release.

## Adding a Stealth Evasion Provider

The stealth layer uses a provider registry. `evasions::bootstrap_script(fp)` is
a thin wrapper over the registry.

1. Open `src/stealth/providers/builtin.rs` and implement `EvasionProvider`:

```rust
use seleniumbase_rs::stealth::providers::{EvasionContext, EvasionProvider};
use seleniumbase_rs::stealth::fingerprint::masked;

pub struct MyEvasionProvider;

impl EvasionProvider for MyEvasionProvider {
    fn name(&self) -> &str {
        "my_evasion" // stable, unique, snake_case
    }

    fn priority(&self) -> i32 {
        // Lower runs earlier. native_toString is 5; navigator props ~30;
        // late/self-defense providers are 110+.
        100
    }

    fn applies(&self, fp: &crate::stealth::fingerprint::Fingerprint) -> bool {
        masked(fp.flags.navigator_masking)
    }

    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let value = EvasionContext::escape(
            ctx.fingerprint.vendor.as_deref().unwrap_or("Example"),
        );
        Some(format!("(function() {{ /* use '{value}' */ }})();"))
    }
}
```

2. Register it in `all()` at the bottom of `builtin.rs` (keep sorted by priority).
3. Wrap replaced natives so `Function.prototype.toString` still reports
   `[native code]`:

```js
obj.method = (window.__sbNative || function(f){return f;})(patched, 'method');
```

The `native_to_string` provider (priority 5) installs `window.__sbNative`
before any other provider runs.

4. Use `ctx.seed` for deterministic per-session noise (canvas, audio, WebGL).
5. Add a unit test asserting the snippet contains expected markers and that
   `applies` is gated correctly.
6. Update `docs/tutorials/fingerprint_stealth.md`.

## Adding a Masking Mode or Fingerprint Field

1. Add the field to `StealthFlags` or `Fingerprint` in
   `src/stealth/fingerprint.rs`.
2. Annotate new serde-backed fields with `#[serde(default)]`.
3. Update `balanced()` and `all_custom()` constructors.
4. Add coherence validation in `Fingerprint::validate()` if the field has
   required companions.
5. Wire the field into the launcher (`browser/session.rs`), provider scripts,
   or the reactor if applicable.
6. Document the new flag in `docs/tutorials/fingerprint_stealth.md`.

## Adding a Macro

Public macros live in `src/macros.rs` and are re-exported at the crate root.

```rust
#[macro_export]
macro_rules! sb_focus {
    ($sb:expr, $css:expr) => {{
        $sb.focus($css)
    }};
}
```

1. Add the macro with `#[macro_export]`.
2. Write a compile-time unit test under the definition.
3. Update `docs/tutorials/macros.md` and the macro table in `README.md`.

## Adding an MCP Tool

The MCP server in `src/bin/mcp_server.rs` uses the `rmcp` crate.

A tool is a `ToolDef` in `src/mcp/`; read `src/mcp/devtools.rs` for working
examples (they are generic over the session type, so one definition serves both
engines) and `src/mcp/stealth.rs` for how an extra toolset is attached.

1. Declare it with `ToolDef::new(name, title, description, effect, schema,
   handler)`. The handler is an `async fn(Ctx<S>, Args) -> Result<Output,
   ToolError>`. Pick the `Effect` honestly: `Observe` reads the open web,
   `Inspect` looks at this machine, `Act` changes the page, `Set` changes the
   session idempotently, `Overwrite` writes local files.
2. Return `ToolError::Failed(SeleniumBaseError)` for failures: the server turns it
   into a failure report with the page's state, the hint and the next tools to
   try. Files a tool writes go through the output directory
   (`SB_MCP_OUTPUT_DIR`), never to an arbitrary path.
3. Register it in the server that offers it (`cdp.rs`, `driver.rs`, `sb.rs`).
4. Test it without Chrome first (`tests/mcp_cdp.rs`, `tests/mcp_devtools.rs`),
   then with a real Chrome in a `*_chrome.rs` test if it reads a page.
5. Document it in `docs/tutorials/mcp_server.md`, add it to `CHANGELOG.md`, and
   give it an entry in `parity/api.toml` if upstream has the same tool.

## Adding an `sbase` Subcommand

1. Add the new variant to `SbaseCommand` in `src/cli/bin/sbase.rs` with an
   idiomatic `///` description.
2. Add argument parsing in the `clap` derive macro.
3. Add a handler branch in `async fn main()`.
4. If the command launches a browser, prefer creating a `BaseCase` through
   `run_browser_test()` so cleanup is deterministic.
5. Update `docs/help/commander.md` and `README.md` if the command is user-facing.

## Adding a Profile Payload Field

External profile payloads are parsed in `src/profile_payloads/profile.rs`.

1. Add the field to `ProfileParams` (or the appropriate nested struct).
2. Map it to `BrowserConfig` or `Fingerprint` in `to_browser_config()` and
   `browser()`.
3. Add a unit test for parsing and round-tripping.
4. Update `docs/tutorials/browser_profiles.md`.

## Binary Patching

`ChromedriverPatcher` modifies a local chromedriver binary to help evade
binary-signature detection.

- Patches are defined as `EnginePatch` structs in `src/stealth/patcher.rs`.
- Each patch has a human-readable `id`, byte sequence search/replace, and
  optional marker detection.
- Add new patches to `ChromedriverPatcher::default_patches()`.
- Always test patches against the driver versions listed in
  `docs/help/binary_patching.md`.
- Backups are stored next to the binary with `.sb-backup` extension.

## Writing Documentation

- All public items should have doc comments (`///`).
- Use examples in doc comments; they run as doctests.
- User-facing tutorials go in `docs/tutorials/`.
- Reference/help pages go in `docs/help/`.
- AI-agent guidance goes in `COPILOT.md` and `CLAUDE.md`.
- After adding a page, add it to `docs/SUMMARY.md`.
- A user-visible change also gets an entry in `CHANGELOG.md`.
- Say what was verified and how. A claim like "works in Chrome" needs a real
  Chrome behind it; otherwise say it was checked against the mock or by reading.
- Build the book locally before pushing: `mdbook build`.

## Working with AI Assistants

Assistants are welcome, and the repository is set up for them: `AGENTS.md` is the
short rule sheet any agent can read, `llms.txt` indexes the docs for models,
`CLAUDE.md` is the module map and rule list, `rust-analyzer.toml` makes analysis cover the
optional features, and `docs/tutorials/vibe_coding.md` shows how to use the MCP
servers to explore a page and export a test. Whoever opens the pull request owns
the result:

- Run the checks yourself and report their exit codes; do not paste "it should
  pass".
- Read the diff. Reject tests that cannot fail, assertions that were loosened to
  make a test pass, and `sleep` calls added instead of waits.
- Do not let an assistant mark a parity entry `implemented` without code and a
  test, or invent an API: check it against the code or the library's docs.
- Keep secrets out of prompts, logs and committed files.

## Opening a Pull Request

1. Branch from `main` unless told otherwise.
2. Keep commits focused and atomic, with conventional-commit subjects
   (`feat(scope): ...`, `fix(scope): ...`, `test: ...`, `docs: ...`, `refactor: ...`).
   Commits carry no co-author or tool trailers, and never contain secrets.
3. Update the tests, `CHANGELOG.md` and the docs in the same change.
4. Fill out the PR description with:
   - What changed and why.
   - Verification commands that passed.
   - Any breaking API changes and migration notes.
5. Ensure CI is green.

### PR review checklist

- [ ] `cargo fmt --all -- --check` is clean.
- [ ] `cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings` is clean.
- [ ] `cargo test --features "mcp-server test-util"` passes, and so does
      `cargo clippy --features scrape --lib -- -D warnings`.
- [ ] `cargo deny check` and `cargo audit` are clean if `Cargo.lock` changed.
- [ ] A new `[[bin]]`, `[[bench]]`, `[[example]]` or `[[test]]` is in the
      `Dockerfile` skeleton.
- [ ] `parity/api.toml` and `docs/parity.md` are updated for a new API.
- [ ] `mdbook build` succeeds if docs changed.
- [ ] New public items have doc comments.
- [ ] New features are covered by tests or examples.
- [ ] Documentation reflects the change.
- [ ] No third-party anti-detect product names or trademarks were added.

## Publishing

The crate is not published to crates.io; it is distributed from the repository.
To publish a change to `main`:

1. Run the full verification matrix on the exact commit you will push.
2. `git fetch`, and confirm `origin/main` has not moved since you started.
3. Push as a fast-forward only (for example
   `git push origin <commit>:refs/heads/main`), then check that your local
   branch and `origin/main` are the same commit.
4. Never force-push.

A docs-only commit on top of a commit that passed the checks may be pushed
after it, and its message should say so.

## Licensing

By contributing, you agree that your contributions will be licensed under the
same license as the project (MIT).
