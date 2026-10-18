---
title: "Contributing to SeleniumBase-rs"
---

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
12. [Adding a Fingerprint Setting or Policy Field](#adding-a-fingerprint-setting-or-policy-field)
13. [Adding a Macro](#adding-a-macro)
14. [Adding an MCP Tool](#adding-an-mcp-tool)
15. [Adding an `sbrs` Subcommand](#adding-an-sbrs-subcommand)
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

# Build the mdBook
mdbook build
```

Features, binaries, the Dockerfile and build-time tips are in
[docs/building.md](/building).

<Tip>

Read `CLAUDE.md` once even if you are not using an AI assistant: it is a short map
of the modules, the commands and the things that have bitten before.

</Tip>

Three environment flags keep builds cheap on a small machine, and they let
several checkouts share compiled dependencies:

```bash
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
```

<Warning>

Do not run two heavy cargo commands at once in the same target directory (cargo
serialises them with a lock), and do not share one `CARGO_TARGET_DIR` between
several checkouts of this repository: they overwrite each other's compiled
library and doctests then fail with "extern location does not exist".

</Warning>

## Development Environment

<Info>

With [mise](https://mise.jdx.dev), `mise install` sets up the tools below from
`mise.toml` (Rust 1.99 with clippy and rustfmt, Python 3.14 for the parity
scripts, Node for the Tauri example, mdBook, just and cargo-deny), and
`mise run gate` runs the checks a change has to pass; `mise tasks` lists the
rest (`chrome`, `parity-docs`, `docs`, `smoke`).

</Info>

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
- A Chrome (and, for the WebDriver engine, a matching chromedriver: `sbrs
  install`) for the tests marked `#[ignore]`.
- `libwebkit2gtk-4.1-dev` (Linux) or Xcode (macOS) if you build the Tauri
  example.

### Recommended for working with an AI assistant

- [TokenSave](https://github.com/aovestdipaperino/tokensave)
  (`brew install aovestdipaperino/tap/tokensave`, then `tokensave init` in the
  checkout; `.tokensave/` is ignored by git): a code graph of the repository
  that an assistant queries over MCP (`tokensave serve`): symbols by name,
  callers and callees, the impact of a change, and files read by symbol. The
  assistant reads far less of the code to answer a question or make a change.
- [RTK](https://www.rtk-ai.app/) (`brew install rtk`): a command proxy that
  condenses the output of `cargo`, `git`, test runners and similar tools before
  it reaches an assistant (`rtk cargo test`, `rtk git log`), so long build and
  test logs do not fill its context. Exit codes are unchanged.

Neither is needed to build, test or contribute.

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
│   ├── mcp/                   # MCP servers: cdp, driver, sb; developer tools in devtools.rs
│   ├── cli/                   # the library half of sbrs: commands.rs, scaffold.rs, scripts/
│   ├── behave/                # Gherkin/BDD runner
│   ├── plugins/               # Reporting and logging plugins
│   ├── config/, core/, common/, storage/, utilities/, utils/, profile_payloads/
│   └── error.rs               # SeleniumBaseError: hints, exit codes, selector advice
├── crates/cli/                # Package seleniumbase-rs-cli: the sbrs and sbrs-mcp binaries
│   ├── src/                   # sbrs.rs (usage derives), fig.rs, report.rs, mcp_server.rs
│   └── tests/                 # Every test that runs the real sbrs or sbrs-mcp binary
├── tests/                     # Integration tests of the library (tests/golden is compiled in)
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
3. **Stealth layer** — `Fingerprint` (one `Spoof` per setting) + `StealthPolicy` describe what to spoof;
   `EvasionProvider`s generate the JavaScript bootstrap; `ChromedriverPatcher`
   mutates the local driver binary; `StealthReactor` intercepts network requests
   via CDP.
4. **Tooling layer** — `sbrs` CLI, the MCP server, the behave runner, and the
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
RUST_LOG=info cargo run --bin sbrs --features full-tracing -- open https://example.com
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
  `cargo test --test mcp_cdp_chrome -- --ignored`. The tests that run the
  `sbrs` binaries are in `crates/cli/tests`; name the package for those:
  `cargo test -p seleniumbase-rs-cli --test cli_e2e_chrome -- --ignored`.
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
cargo check --features turso --all-targets            # code that only builds with turso
cargo deny check && cargo audit                        # when Cargo.lock changed
mdbook build                                           # when docs changed
```

Count failures from the exit code and the `test result:` lines, not from a
filtered search of the output. The repository is a workspace of two packages, the
library `seleniumbase-rs` and the command line `seleniumbase-rs-cli`
(`crates/cli`), and the commands above cover both (`default-members`). Use
`-p seleniumbase-rs` for the library alone, with its own default features: the
command line turns `pdf` and `mcp-server` on, and a build of both unifies them.
A change that touches a `Cargo.toml` must keep the `Dockerfile` skeleton step in
step: a new `[[bin]]`, `[[bench]]`, `[[example]]` or `[[test]]`, in either
manifest, must be listed there, and `tests/dockerfile_skeleton.rs` fails if it
is not. `tests/library_manifest.rs` fails if a binary or a command-line crate
(`usage`, or the clap crates it replaced) comes back into the library package.

`cargo deny check` gates advisories, licenses, banned crates, and dependency
sources against `deny.toml`. Every advisory that is ignored there carries a
written rationale, so if a new one appears, either upgrade the dependency or
add an entry explaining why the risk is acceptable.

<Warning>

Do not widen the ignore list silently.

</Warning>

The crate declares `rust-version = "1.99"`, the newest stable release when it was
last raised, which the `msrv` CI job enforces with that exact toolchain. The
floor is a choice to track current stable, not something the crate's own source
needs, so when it moves, bump `rust-version`, the `msrv` job, and the
`Dockerfile` base image together.

<Tip>

On macOS, if a test binary cannot load `libpython`, prefix the test commands with:

```bash
DYLD_LIBRARY_PATH="$HOME/.local/share/mise/installs/python/3.14.6/lib" cargo test --features "mcp-server test-util"
```

</Tip>

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

<Steps>

<Step title="Implement EvasionProvider">

Open `src/stealth/providers/builtin.rs` and implement `EvasionProvider`:

```rust
use seleniumbase_rs::stealth::providers::{EvasionContext, EvasionProvider};
use seleniumbase_rs::stealth::fingerprint::Dimension;

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
        // The one predicate: on when the fingerprint holds a value for the
        // dimension, or its flag says Mask; off when the flag says Natural.
        fp.is_masked(Dimension::Navigator)
    }

    fn script(&self, ctx: &EvasionContext) -> Option<String> {
        let value = EvasionContext::escape(
            ctx.fingerprint.vendor.as_deref().unwrap_or("Example"),
        );
        Some(format!("(function() {{ /* use '{value}' */ }})();"))
    }
}
```

</Step>

<Step title="Register it">

Register it in `all()` at the bottom of `builtin.rs` (keep sorted by priority).

</Step>

<Step title="Wrap replaced natives">

Wrap replaced natives so `Function.prototype.toString` still reports
`[native code]`:

```js
obj.method = (window.__sbNative || function(f){return f;})(patched, 'method');
```

The `native_to_string` provider (priority 5) installs `window.__sbNative`
before any other provider runs.

</Step>

<Step title="Seed the noise">

Use `ctx.seed` for deterministic per-session noise (canvas, audio, WebGL).

</Step>

<Step title="Test it">

Add a unit test asserting the snippet contains expected markers and that
`applies` is gated correctly.

</Step>

<Step title="Document it">

Update `docs/tutorials/fingerprint_stealth.md`.

</Step>

</Steps>

## Adding a Fingerprint Setting or Policy Field

A setting is what the browser claims (one `Spoof<T>` field of `Fingerprint`); a
policy field is how the claims are delivered (a field of `StealthPolicy` in
`src/stealth/policy.rs`).

For a setting:

1. Add the field to `Fields` in `src/stealth/fingerprint.rs`, with its default
   in `impl Default for Fields` and `#[serde(skip_serializing_if = "Spoof::is_natural")]`,
   and a read accessor in the `accessors!` list of `Fingerprint`. If its value is
   not a plain type, add the value type to `src/stealth/settings.rs` with a
   conversion from the obvious tuple.
2. Add its setter to the `setters!` list of `FingerprintBuilder`, and its name to
   `has_random` and `without_random`. Decide whether it has an off state: if not,
   add it to `refuse_useless_off`; and what `Random` draws: add the arm to
   `draw_random` (taking the value from the `Draw` in `randomize.rs`), or make it
   an error there.
3. Give the dimension it belongs to (or a new `Dimension` variant, which is
   `#[non_exhaustive]`) an arm in `Fingerprint::is_masked`. Providers, launch
   arguments and overrides ask `is_masked`, never a field by hand.
4. Add coherence validation in `Fingerprint::validate()` if the setting has
   required companions.
5. Wire the setting into the launcher (`browser/session.rs`), provider scripts,
   or the reactor if applicable. A provider defines only a setting that is a
   `Value`, and handles `Off` where it has one.
6. Add the payload key to `ProfileParams::fingerprint_builder` if a payload can
   carry it, reading the common words of `Word`.
7. Document the setting in `docs/tutorials/fingerprint_stealth.md`.

For a policy field, add the field, its default and its preset values to
`StealthPolicy`, a method of the same name, and a builder shortcut
(`with_policy`); a preset never changes a setting.

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

<Steps>

<Step title="Add the macro">

Add the macro with `#[macro_export]`.

</Step>

<Step title="Test it">

Write a compile-time unit test under the definition.

</Step>

<Step title="Document it">

Update `docs/tutorials/macros.md` and the macro table in `README.md`.

</Step>

</Steps>

## Adding an MCP Tool

The MCP server (`src/mcp/`, started by `crates/cli/src/mcp_server.rs`) uses the
`rmcp` crate.

A tool is a `ToolDef` in `src/mcp/`; read `src/mcp/devtools.rs` for working
examples (they are generic over the session type, so one definition serves both
engines) and `src/mcp/stealth.rs` for how an extra toolset is attached.

<Steps>

<Step title="Declare it">

Declare it with `ToolDef::new(name, title, description, effect, schema,
handler)`. The handler is an `async fn(Ctx<S>, Args) -> Result<Output,
ToolError>`. Pick the `Effect` honestly: `Observe` reads the open web,
`Inspect` looks at this machine, `Act` changes the page, `Set` changes the
session idempotently, `Overwrite` writes local files.

</Step>

<Step title="Report failures">

Return `ToolError::Failed(SeleniumBaseError)` for failures: the server turns it
into a failure report with the page's state, the hint and the next tools to
try. Files a tool writes go through the output directory
(`SB_MCP_OUTPUT_DIR`), never to an arbitrary path.

</Step>

<Step title="Register it">

Register it in the server that offers it (`cdp.rs`, `driver.rs`, `sb.rs`).

</Step>

<Step title="Test it">

Test it without Chrome first (`tests/mcp_cdp.rs`, `tests/mcp_devtools.rs`),
then with a real Chrome in a `*_chrome.rs` test if it reads a page.

</Step>

<Step title="Document it">

Document it in `docs/tutorials/mcp_server.md`, add it to `CHANGELOG.md`, and
give it an entry in `parity/api.toml` if upstream has the same tool.

</Step>

</Steps>

## Adding an `sbrs` Subcommand

<Steps>

<Step title="Add the variant">

Add the new variant to `Commands` in `crates/cli/src/sbrs.rs` with an
idiomatic `///` description (its first paragraph is the short help).

</Step>

<Step title="Parse the arguments">

Declare its arguments and flags with `#[usage(...)]` attributes on the fields, as
the neighbouring variants do (`long`, `short`, `default`, `conflicts(...)`,
`value_enum`; a switch takes `bool_value`).

</Step>

<Step title="Handle it">

Add a handler branch in `async fn run`.

</Step>

<Step title="Clean up">

If the command launches a browser, prefer creating a `BaseCase` through
`run_browser_test()` so cleanup is deterministic.

</Step>

<Step title="Write the generated files">

Write `completions/sbrs.usage.kdl` and `docs/help/cli_reference.md` again with
`SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`;
the same test fails in CI when they are stale.

</Step>

<Step title="Document it">

Update `docs/help/commander.md` and `README.md` if the command is user-facing.

</Step>

</Steps>

## Adding a Profile Payload Field

External profile payloads are parsed in `src/profile_payloads/profile.rs`.

<Steps>

<Step title="Add the field">

Add the field to `ProfileParams` (or the appropriate nested struct).

</Step>

<Step title="Map it">

Map it to `BrowserConfig` or `Fingerprint` in `to_browser_config()` and
`browser()`.

</Step>

<Step title="Test it">

Add a unit test for parsing and round-tripping.

</Step>

<Step title="Document it">

Update `docs/tutorials/browser_profiles.md`.

</Step>

</Steps>

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

<Steps>

<Step title="Branch">

Branch from `main` unless told otherwise.

</Step>

<Step title="Commit">

Keep commits focused and atomic, with conventional-commit subjects
(`feat(scope): ...`, `fix(scope): ...`, `test: ...`, `docs: ...`, `refactor: ...`).
Commits carry no co-author or tool trailers, and never contain secrets.

</Step>

<Step title="Update tests and docs">

Update the tests, `CHANGELOG.md` and the docs in the same change.

</Step>

<Step title="Describe the change">

Fill out the PR description with:

- What changed and why.
- Verification commands that passed.
- Any breaking API changes and migration notes.

</Step>

<Step title="Check CI">

Ensure CI is green.

</Step>

</Steps>

### PR review checklist

- [ ] `cargo fmt --all -- --check` is clean.
- [ ] `cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings` is clean.
- [ ] `cargo test --features "mcp-server test-util"` passes.
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

<Steps>

<Step title="Verify the exact commit">

Run the full verification matrix on the exact commit you will push.

</Step>

<Step title="Fetch">

`git fetch`, and confirm `origin/main` has not moved since you started.

</Step>

<Step title="Fast-forward push">

Push as a fast-forward only (for example
`git push origin <commit>:refs/heads/main`), then check that your local
branch and `origin/main` are the same commit.

</Step>

</Steps>

<Danger>

Never force-push.

</Danger>

A docs-only commit on top of a commit that passed the checks may be pushed
after it, and its message should say so.

## Licensing

By contributing, you agree that your contributions will be licensed under the
same license as the project (MIT).
