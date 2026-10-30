# AGENTS.md

Instructions for AI coding agents working in this repository (any tool that reads
`AGENTS.md`). Humans want `CONTRIBUTING.md`. `CLAUDE.md` is the longer module map
and the rules for Claude Code; `COPILOT.md` is the short Copilot guide. If they
disagree, this file and `CLAUDE.md` win. If you write browser tests *with* this
crate rather than change it, go to "Writing browser tests with seleniumbase-rs"
below.

## What this is

`seleniumbase-rs` is a Rust port of Python SeleniumBase 4.55.2 and of its MCP
server (crate `seleniumbase-rs`, MIT, Rust 1.99, not on crates.io). Two engines:
WebDriver (`src/browser`, `src/api`, built on `thirtyfour`) and a pure CDP engine
(`src/sb_cdp`). Stealth lives in `src/evasion`; MCP servers in `src/mcp`. The
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
  someone else is using. With [mr-boxington](https://mr-boxington.jdx.dev)
  (`mbx`) each checkout gets its own managed target and compiled work is shared
  through its cache; keep running plain `cargo` and set neither
  `CARGO_TARGET_DIR` nor `RUSTC_WRAPPER`.

## How to work

- **Test first.** Write the signature (`todo!()` body) and the tests, make the
  type checker happy, implement, run the narrow test, refactor. A bug fix gets a
  test that fails without the fix.
- **Prefer structural tools over text search** when you have them: a code index
  or a language server for symbols and callers, `ast-grep` for code shapes. Read a
  slice of a file, not all of it. Recommended: [TokenSave](https://github.com/aovestdipaperino/tokensave)
  (`tokensave init` here, then its MCP tools for symbols, callers, impact and
  file reads) and [RTK](https://www.rtk-ai.app/) (run `cargo`, `git` and test
  commands as `rtk cargo test` and so on, to get condensed output with the same
  exit codes). When `.tokensave/` exists, search and read code through TokenSave
  rather than `grep` or whole-file reads. For builds, mr-boxington (`mbx`)
  caches compiled work across checkouts and worktrees.
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
- **After changing a public method of `BaseCase` or the pure CDP engine**, write
  `src/mcp/dev/api_index.json` (what the `dev` MCP server's `find_api` searches)
  again with `SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features
  mcp-server api_index`; a test fails when it is stale.
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
session as a Rust test. Load the browserless `dev` server next to it
(`sbrs-mcp --server dev`): it finds API methods, converts Python tests, explains
`sbrs` commands, errors and settings, and installs drivers. `sbrs-mcp install
claude-code` (or `codex`, `cursor`, ...) registers both servers with a client. The
workflow is in `docs/tutorials/vibe_coding.md`; `llms.txt` lists the docs worth
reading.

## Writing browser tests with seleniumbase-rs

For an assistant that writes tests *with* this crate, in a user's project or in
`examples/` and `docs/`; the rules above are for changing the crate. They follow
the checklist that thirtyfour publishes as
[Reliable AI-Generated Tests](https://github.com/stevepryde/thirtyfour/blob/main/docs/src/getting-started/reliable-tests.md),
written for this crate's API. `docs/tutorials/reliable_tests.md` explains each rule
with a do and an avoid. If you are not sure a method exists, ask the `dev` MCP
server (`find_api`) before you write the call.

1. **The fixture owns the browser.** With the feature `testing`, a test asks for
   `sb: Sb` (WebDriver) or `sb_cdp: SbCdp` (pure CDP) and puts its steps in
   `sb.run(async |sb| { .. })` or `sb_cdp.run(async |page| { .. })`, which
   launches the browser, reruns when configured, keeps evidence and closes the
   browser on every path. Without the feature use `run_browser_test` or `run_cdp_test`. Never call
   `BaseCase::new` or `Browser::launch` in a test body without a close that
   cannot be skipped.

   ```rust
   use seleniumbase_rs::testing::{rstest, sb, Sb};
   use seleniumbase_rs::{tokio, Result};

   #[rstest]
   #[tokio::test]
   async fn cart_shows_the_total(sb: Sb) -> Result<()> {
       sb.run(async |sb| {
           sb.open("https://shop.example.com/cart").await?;
           sb.assert_text("$42.00", "#total").await
       })
       .await
   }
   ```

2. **Import from `seleniumbase_rs` only.** Start a file with
   `use seleniumbase_rs::tokio;` (or add `tokio` to the braces of the import
   you have) before `#[tokio::main]` or `#[tokio::test]`. The manifest of a test
   crate lists `seleniumbase-rs` (feature `testing`) and nothing else; see
   `docs/imports.md`.
3. **Use selectors the application owns:** `#id`, `[data-testid="total"]`,
   `link=Sign in`. Not `:nth-child`, generated class names or long paths.
4. **Never sleep to wait.** No `sb.sleep(..)`, no `tokio::time::sleep`, no poll
   loop. Actions and assertions already wait (10 seconds by default). For the
   rest: `sb.wait_for_element_visible("#toast", 10)`,
   `sb.wait_for_text("Saved", "#status", 10)`,
   `sb.wait_for_element_absent(".spinner", 10)`; pure CDP
   `page.locator(".spinner").wait_for(State::Absent)`.
5. **Assert with the retrying assertions:** `sb.assert_text(text, selector)` (text
   first), `assert_exact_text`, `assert_element`, `assert_title`; pure CDP
   `locator.expect().to_have_text(..)`. Not `get_text` plus `assert_eq!`: it reads
   once. End every call with `?`.
6. **Scope the query and know how many match.** A selector or a `Locator` acts on
   the first match and there is no strict mode. Put the container in the selector
   (`#cart [data-sku="B-7"] .remove`, not `.remove`); `switch_to_frame(..)` before
   an iframe and `switch_to_default_content()` after; pure CDP chains
   `page.locator("#cart").locator(".remove")`. When exactly one must match,
   `page.locator("#checkout").expect().to_have_count(1)`.
7. **Find again, do not hold elements.** Pass selector strings, which find the
   element again on each call, or keep a `Locator`. A `WebElement` from
   `find_element` goes stale when the page re-renders.
8. **Cleanup runs on every path.** Do not call `quit` inside `sb.run` or
   `run_browser_test`. A `Browser` you launch is closed before you return the
   outcome, so a failed step cannot skip it:

   ```rust
   let outcome = steps(&page).await;
   browser.close().await?; // whether the steps passed or failed
   outcome
   ```

   A pool lease is `release().await`d and the pool `close().await`d the same way.
9. **Keep the failure evidence bounded.** The fixture writes
   `latest_logs/<test id>/screenshot.png`, `page_source.html` and
   `basic_test_info.txt`. Add to it with `sb.save_screenshot_to_logs()` and
   `sb.save_page_source_to_logs()`. Never print page source or put it in a
   message. A HAR has no bodies unless you ask; cap them with
   `HarOptions::new().content(true).max_content_bytes(256 * 1024)`. Set
   `SB_TIME_LIMIT` so that a hung test ends.
10. **One browser, context or lease per concurrent flow.** Do not run two flows
    on one tab or one `Page`. Use `BrowserPool::acquire()` per flow (a `Lease` is an
    isolated context and a tab) or `browser.new_context()`.
11. **Headless in CI, set outside the code:** `SB_HEADLESS=true cargo test`. The
    fixtures read it. Do not write `.with_headless(false)` or `.headless(false)`
    in a committed test.
12. **Write a body that is safe to run twice.** `SB_RERUNS` calls the closure
    again, so it must not move out of what it captures (`for p in &paths`, not
    `for p in paths`) and must not rely on what an earlier attempt left behind.
    Do not swallow an error (`let _ =`) or weaken an assertion to get a green run.
13. **Keep raw protocol calls in one helper.** `Page::execute`,
    `Browser::execute` and `BaseCase::execute_cdp_cmd` are Chromium only: put them
    in one module and gate the tests that need them behind a Cargo feature of the
    test crate or `#[ignore = "needs Chrome DevTools"]`. The crate has no WebDriver
    BiDi API.
14. **Test page-driving code without Chrome.** With the feature `test-util`,
    `Browser::new_mocked()` and `sb_cdp.run_with_browser(..)` run a test on a
    scripted browser. Launch a real browser only for what needs one.
15. **Credentials and data come from the run,** not from the file:
    `sb.account()`, `sb.data()`, `sb.variables()` (`SB_ACCOUNT`, `SB_DATA`,
    `SB_VARIABLES`). No literal password, token or account name in a test.

Where the crate cannot enforce a rule (no strict single match, no scoped query on
`BaseCase`, no hard time limit outside the fixtures, nothing that refuses a
sleep), the page lists it under "What the crate cannot enforce yet".


## Done means

The change compiles, its tests pass, the gate commands above are clean, the docs
and changelog are updated, and your report says what ran, what did not, and what
you could not verify.
