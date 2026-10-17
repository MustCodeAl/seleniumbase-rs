# AGENTS.md

Instructions for AI coding agents working in this repository (any tool that reads
`AGENTS.md`). Humans want `CONTRIBUTING.md`. `CLAUDE.md` is the longer module map
and the rules for Claude Code; `COPILOT.md` is the short Copilot guide. If they
disagree, this file and `CLAUDE.md` win.

## What this is

`seleniumbase-rs` is a Rust port of Python SeleniumBase 4.55.2 and of its MCP
server (crate `seleniumbase-rs`, MIT, Rust 1.99, not on crates.io). Two engines:
WebDriver (`src/browser`, `src/api`, built on `thirtyfour`) and a pure CDP engine
(`src/sb_cdp`). Stealth lives in `src/stealth`; MCP servers in `src/mcp`. The
library has no binary: the CLI (`sbrs`, `sbrs-mcp`) is the workspace package
`seleniumbase-rs-cli` in `crates/cli` (helpers in the library's `src/cli`).
Optional features of the library are off by default. The experimental
stealth scraping layer (wire fingerprints, TLS and HTTP/2 impersonation) is not
here: since 0.3.0 it lives in
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
and depends on this crate's public API.

## Read first

1. `CLAUDE.md`: the module map, the commands, what has bitten before.
2. The file you are about to change, and the tests next to it.
3. `parity/api.toml` if you add or change a public API.

## Commands

```bash
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all -- --check
cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings
cargo test --features "mcp-server test-util"
cargo check --features turso --all-targets
```

- Run the narrowest test while you work (`cargo test --lib <filter>`,
  `cargo test --test <name>`); run the whole set once, at the end.
- Tests that need a real Chrome are named `*_chrome.rs` and are `#[ignore]`d. Run
  one binary at a time: `cargo test --test mcp_cdp_chrome -- --ignored`. Tests
  that run the `sbrs` binaries are in `crates/cli/tests` (`-p seleniumbase-rs-cli`).
- Count failures from the exit code and the `test result:` lines, not from a
  filtered search of the output.
- Do not run two heavy cargo commands at once, and do not share one
  `CARGO_TARGET_DIR` between several checkouts or worktrees: they overwrite each
  other's compiled library and doctests fail. Never `cargo clean` a directory
  someone else is using.

## How to work

- **Test first.** Write the signature (`todo!()` body) and the tests, make the
  type checker happy, implement, run the narrow test, refactor. A bug fix gets a
  test that fails without the fix.
- **Prefer structural tools over text search** when you have them: a code index
  or a language server for symbols and callers, `ast-grep` for code shapes. Read a
  slice of a file, not all of it.
- **Check external APIs against their documentation**, not memory (crates,
  protocols, browser flags).
- **Match the neighbours:** `thiserror` errors in the library, no `unwrap()`
  outside tests, a doc comment on every public item, `tracing` instead of
  `println!`. `src/mcp` and `src/sb_cdp` use the pedantic lint set: an `#[allow]`
  needs `reason = "..."`, and a function over 100 lines fails.
- **Do not edit generated files by hand:** `docs/parity.md` is generated from
  `parity/api.toml` (`just parity-docs`).
- **A new `[[bin]]`, `[[bench]]`, `[[example]]` or `[[test]]`**, in either
  manifest, must also be listed in the `Dockerfile` skeleton step
  (`tests/dockerfile_skeleton.rs` checks). Keep `usage` and other command-line
  dependencies in `crates/cli` (`tests/library_manifest.rs` checks).
- **After changing the command line** (`crates/cli/src/sbrs.rs`), write the
  generated `completions/sbrs.usage.kdl` and `docs/help/cli_reference.md` again with
  `SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`.
- **Update the docs and `CHANGELOG.md`** with the change.

## Honesty rules

- Never claim a test, build or lint passed unless you ran it and saw it pass.
  Report the commands and their exit codes.
- Say what was verified on a real browser and what only against the mock or by
  reading the code.
- Never mark a `parity/api.toml` entry `implemented` without code and a test; use
  `composed` only when you named the Rust path and it exists, and give every
  `not-applicable` a reason specific to that API.
- Do not weaken or delete an assertion, or add a `sleep`, to make a test pass.
  Fix the cause.

## Boundaries

- No secrets in code, tests, logs, prompts or commits.
- No force-push, no history rewrites on shared branches, no publishing to
  crates.io, and nothing that changes the repository's CI or release settings
  without being asked.
- Commit with conventional-commit subjects (`feat(scope): ...`, `fix: ...`,
  `test: ...`, `docs: ...`), small and one concern each. Commits carry no
  co-author or tool trailers.
- Push only when asked: gate the exact commit, `git fetch`, confirm the remote
  did not move, push as a fast-forward, and check that local and remote match.
- Keep third-party anti-detect product names and trademarks out of code, docs and
  tests (see `CONTRIBUTING.md`).
- The stealth work is written as our own code; do not add a dependency on the
  projects it takes ideas from, and do not copy their code or folder layout.

## Machine limits

Builds are heavy. Keep `CARGO_BUILD_JOBS` low, work in your own worktree, commit
often, and save work as a commit before you stop. A slow first run of a freshly
written script on macOS is normal: warm it once in tests that fake executables.

## Using this crate from an agent

If you are driving a browser rather than editing the crate, use the MCP servers
(`sbrs-mcp --server cdp`): page inspection, selector diagnosis, console
and network views, an accessibility audit, a debug bundle, and an export of the
session as a Rust test. The workflow is in
`docs/tutorials/vibe_coding.md`; `llms.txt` lists the docs worth reading.

## Done means

The change compiles, its tests pass, the gate commands above are clean, the docs
and changelog are updated, and your report says what ran, what did not, and what
you could not verify.
