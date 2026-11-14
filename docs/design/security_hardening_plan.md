---
title: "Design: security hardening plan"
sidebarTitle: "Security hardening plan"
---

Status: implementation plan, written before any code, on branch `lane/sec-2` (from `2327ffb8`, the
merge of `batch/friend-2026-11-07`, 2026-11-07). Nothing was compiled or run while it was written:
the facts come from reading the worktree with TokenSave, from the GitLab Advanced SAST report of
pipeline 3039131874 (commit `266bd5f5`, an ancestor of this branch), and from the docs of
`subtle` 2.6, `secrecy` 0.10.3 and `argon2` 0.6.0 (Context7, docs.rs and the crates.io API). What
could not be checked says "verify". The plan has eight steps; each is one commit, in this order,
and every step leaves the tree building and its tests green.

## What the worktree already has

The goal's six items, measured against the code as it stands:

| Item | What is there | What is left |
|---|---|---|
| 1. Constant-time bearer check | `src/mcp/http.rs`: `Gate::check` (line 365) hashes the sent token with SHA-256 (`digest`, line 1090) and compares it to the stored digest with `same_digest` (line 1098), a hand-written XOR fold behind `std::hint::black_box`. Both sides are 32 bytes, so a token of another length already takes the same path. The test `the_token_is_a_bearer_credential_and_must_match_exactly` (line 1477) covers the right token, wrong ones, wrong lengths, a missing header, a wrong scheme and the case-insensitive scheme. | Swap the fold for `subtle::ConstantTimeEq`; add a unit test of `same_digest`. |
| 2. Password KDF for `sbrs encrypt` | `src/common/encryption.rs`: PBKDF2-HMAC-SHA256, 600,000 iterations, a random 16-byte salt, AES-256-GCM (ring). Token `sbenc1:<iterations>:<salt>:<nonce+ciphertext>`; the iteration count is bounded on decrypt (1..=10,000,000). | It is a password KDF, but not a memory-hard one: move tokens to argon2id with a new format, refuse `sbenc1`. |
| 3. `secrecy` and `zeroize` | `HttpOptions::token: Option<String>` with a hand-written `Debug` that prints `<redacted>` (test near http.rs:1338). `mcp::client::Target::Http { bearer: Option<String> }` derives `Debug`, so `{:?}` prints the bearer token. `generate_token() -> Result<String, _>`. The CLI keeps the passphrase and the typed texts in plain `String`s (`crates/cli/src/crypto.rs`, `sbrs.rs::encryption_passphrase`). The derived key is a plain `[u8; 32]`. | `SecretString` for the token, the bearer, the passphrase and hidden input; `Zeroizing` for the key and the buffers. |
| 3b. IMAP password | Core never holds it: `src/mcp/email.rs` and `src/plugin_host` pass the plugin the *name* of the variable (`password_env`); the plugin crate `seleniumbase-rs-plugins-builtin` (git dependency, rev `06669044`) reads the value. | Nothing in core; a list of what the plugins repository needs (below). |
| 4. Selector recursion | `src/utils/selectors.rs`: `xpath_root_branch` (line 1049) and `branch_starts_at_the_root` (line 1058) call each other once per leading `(`, so 10,000 parentheses recurse 10,000 frames deep. It is the only recursion cycle in the file (`tokensave_recursion`). Callers: `sb_cdp::locator::Step::new` (sets `confine`) and `browser::element::Element::lookup` (refuses a root-anchored XPath), both reached from MCP tool arguments. | Bound it; add the 256 KiB-stack test. |
| 5. `cargo deny` / `cargo audit` | `deny.toml` allows MIT, Apache-2.0 and BSD-3-Clause; `subtle` 2.6.1 and `zeroize` 1.9.1 are already in `Cargo.lock` (transitively); `argon2` and `secrecy` are not. | Run both after the new dependencies. |
| 6. SAST triage | 81 findings: 1 Critical, 74 High, 6 Medium (two jobs: `gitlab-advanced-sast` with 2, `gitlab-advanced-sast-ext` with 79). Read by source and sink (table in step 1). None is a vulnerability: every path comes from the owner's command line, settings, environment or test code, or is a fixed name under a folder the owner chose, or sits in a `#[cfg(test)]` module; the Critical, the regex and the cleartext findings are not the weakness they name. The inputs that *are* untrusted (MCP tool paths, download names, archive entries) are already confined. | Write `docs/design/security_triage.md`. |

## Rules for the implementer

* Work only in `/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/.claude/worktrees/sec-2`, branch
  `lane/sec-2`; `cd` there in every shell command. Never push, no `git stash`, no attribution lines.
* TokenSave for every read, search and edit of code (`graph_root` = the worktree). In this worktree,
  edit code with `tokensave_ast_grep_rewrite` (absolute path), `tokensave_insert_at` (a unique
  anchor), one-line `tokensave_str_replace` and one-line `tokensave_replace_lines`; never
  `replace_symbol`, `insert_at_symbol` or `delete_symbol`. A whole new function body goes in with
  `insert_at` after removing the old one with `ast_grep_rewrite`, or as one `ast_grep_rewrite` whose
  pattern is the old item. Docs and TOML may use `tokensave_multi_str_replace`.
* Before code that calls `argon2`, `secrecy`, `subtle`, `zeroize`, `base64` or `ring` in a way this
  plan does not spell out, read their docs (Context7 `resolve-library-id` then `query-docs`; docs.rs
  for one step if Context7 lacks the version) and hover the item with rust-analyzer
  (`set_workspace` to the worktree first).
* Test-first: write the test, see it fail (a compile error counts where the plan says so), change
  the code, see it pass. Where a step is a refactor with no red run possible, say so in the commit.
* Cargo environment: `export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`; one
  cargo command at a time; never `CARGO_TARGET_DIR`, `--target-dir` or `RUSTC_WRAPPER`. Write long
  output to the scratchpad and count failures from the `test result:` lines and the exit code.
* Edit `Cargo.toml` with `cargo add` (cargo-edit); a feature list is edited by hand with a one-line
  `tokensave_str_replace`.

## Step 1: triage of the SAST findings (docs only)

First, because steps 5 and 6 move lines of `crates/cli/src/sbrs.rs`, and the triage reads the code
at the lines the scan names.

**Files.** New: `docs/design/security_triage.md`. Changed: `docs/SUMMARY.md`, `docs/docs.json`,
`docs/stealth/fingerprints.md`, `CHANGELOG.md`.

**What to do.**

1. Front matter `title: "Design: triage of the GitLab Advanced SAST findings"`,
   `sidebarTitle: "SAST triage"`, then a Status paragraph: scanner, pipeline 3039131874, scanned
   commit `266bd5f5`, date, and how to use the note (read the next scan against the table; a
   finding whose sink and source match a row is already decided).
2. A table with one row per finding: `#`, severity, CWE, sink `file:line` (the scanned commit's
   line), source `file:line`, verdict, reason, "line at `2327ffb8`" when it differs. Verdicts:
   * **test code**: inside a `#[cfg(test)]` module, joining constant names under a temporary folder;
   * **owner input**: a path from the owner's command line, settings file, environment, or the
     argument of a library call written by the test author (paths from the owner's own command line
     or settings file are trusted input, and the row says which one);
   * **fixed name**: a constant file name joined under a folder the owner chose;
   * **not this weakness**: the scanner's model does not fit the code (say why).
3. Rows marked "read" below were read for this plan. Rows marked "check" were not: read each sink
   with `tokensave_read` (`mode: "lines"`, a few lines around it) before writing the row. Three
   flagged files changed after the scan: `crates/cli/src/sbrs.rs`, `src/config/settings_file.rs`,
   `src/core/test_run.rs`. Map their lines through the hunks of
   `rtk git diff 266bd5f5 HEAD -- <file>` before reading.
4. **If a "check" row turns out to take a path from a web page, a network answer, an archive entry
   or an MCP tool argument, stop the triage, add a fix step** (test-first, one commit: confine the
   path with `artifacts::confined_path` or `artifacts::bare_file_name`, as `sb_cdp/download.rs`
   does) and record the row as "fixed in `<commit>`".
5. A section "Untrusted paths that are already confined", so the next reader does not search
   again:
   * MCP tools write only through `mcp::Settings::output_path` (`src/mcp/host.rs:139`), whose
     `confined` (line 153) refuses absolute paths and `..`; names come from `artifacts::safe_stem`
     or `is_test_name` (`src/mcp/devtools/exports.rs`).
   * Downloads name files through `artifacts::bare_file_name` and `artifacts::confined_path`
     (`src/artifacts.rs:254`, `:275`; tests `a_name_that_could_leave_the_folder_is_refused`,
     `a_confined_path_stays_in_its_folder`).
   * Archives: `zip` 9.2.2 `ZipArchive::extract` refuses entries that climb out
     (`src/browser/chrome_for_testing.rs:412`); `enclosed_name()` in `src/browser/downloader.rs:217`
     and `src/utils/extensions/mod.rs:95`; `tar` 0.4.46 `unpack` skips `..`, capped by
     `MAX_UNPACKED_TAR_BYTES`.
   * The recorder writes `<folder>/<stem>_rec.rs` with the stem checked by `check_file_name` and the
     scenario passed through `rust_identifier` (`src/api/recorder/output.rs:230`, `:254`).
6. A section "Not found by the scan": the selector recursion (fixed in step 2), and a note that a
   fingerprint file can name a proxy and its login (`src/evasion/fingerprint/file.rs:99`), so it is
   configuration, not data.
7. A section "Dependencies", filled in step 7.
8. An "Owner actions" section: dismiss the 81 findings in GitLab's vulnerability report as false
   positives with a link to this note (the lane does not change GitLab).

**The rows.**

| # | Sev | CWE | Sink (scan) | Source | Verdict | Reason | Seen |
|---|---|---|---|---|---|---|---|
| 1 | H | 22 | `crates/cli/src/mcp_client/mod.rs:200` | same line | owner input | expected: a path from `sbrs-mcp` flags | check |
| 2 | H | 22 | `crates/cli/src/mcp_client/mod.rs:203` | same line | owner input | as 1 | check |
| 3 | H | 22 | `crates/cli/src/mcp_install/tomlfile.rs:24` | `:23` | fixed name | expected: a client's config file at the place `mcp_install` computes for it | check |
| 4 | H | 22 | `crates/cli/src/mcp_server.rs:391` | `:390` | owner input | `http_token` reads `--token-file` or `SB_MCP_TOKEN_FILE` | read |
| 5–9 | H | 22 | `crates/cli/src/sbrs.rs:2501`, `:3259`, `:3493`, `:4446`, `:4595` | the line, or the line before it | owner input | expected: command-line arguments; the file changed after the scan, map the lines | check |
| 10 | H | 22 | `src/api/base_case_impls/base_case_impl_parity.rs:51` | `:50` | owner input | `read_saved_cookies(path)`: the test author's path | read |
| 11 | H | 22 | `base_case_impl_parity.rs:609` | `:606` | test code | `saved_cookies_are_loaded_the_way_python_loads_them` | read |
| 12, 13 | H | 22 | `base_case_impl_recorder.rs:750`, `:794` | same lines | test code | recorder tests | read |
| 14 | H | 22 | `src/api/pdf.rs:67` | `src/cli/scripts/sb_convert.rs:72` | owner input | expected: the file named to `sbrs` | check |
| 15 | H | 78 | `src/api/recorder/js/recorder.js:650` | `:619` | not this weakness | `/\r|\n/.exec(text)` is `RegExp.prototype.exec` on a constant regex, not a process | read |
| 16 | M | 1333 | `src/api/recorder/js/recorder.js:650` | `:619` | not this weakness | `/\r|\n/` has no nested quantifier and runs in linear time | read |
| 17 | H | 22 | `src/api/recorder/output.rs:308` | `:307` | fixed name | `print_numbered(path)` prints the file just written | read |
| 18–20 | H | 22 | `output.rs:333`, `:341`, `:343` | `:326` | fixed name | `write_test`: `<folder>/<stem>_rec.rs`, stem checked by `check_file_name` | read |
| 21–23 | H | 22 | `output.rs:391`, `:397`, `:400` | `:377` | fixed name | `write_feature`: as 18 | read |
| 24–28 | H | 22 | `output.rs:442`, `:472`, `:482`, `:507`, `:537` | same lines or `:505` | test code | the tests of `write_recording` | read |
| 29 | H | 22 | `src/api/scenario.rs:725` | `:723` | owner input | expected: a scenario file the test names | check |
| 30 | H | 22 | `src/api/traits.rs:110` | `sb_convert.rs:72` | owner input | expected: a path argument of a trait method | check |
| 31 | H | 22 | `src/browser/chromium_snapshots.rs:210` | same line | fixed name | `installed_revision`: `<root>/chromium/<REVISION_FILE>` | read |
| 32 | H | 22 | `src/cli/plugin_cmd.rs:864` | `:852` | owner input | the task input file named on the command line | read |
| 33 | H | 22 | `src/cli/plugin_cmd.rs:917` | `sb_convert.rs:72` | owner input | expected: as 32 | check |
| 34 | H | 22 | `src/cli/prompt.rs:322` | `sb_convert.rs:72` | owner input | expected | check |
| 35 | H | 22 | `src/cli/scripts/case_plans.rs:211` | same line | owner input | expected | check |
| 36 | H | 22 | `src/cli/scripts/recorder_tui.rs:1057` | same line | owner input | expected | check |
| 37, 38 | H | 22 | `src/cli/scripts/sb_behave_gui.rs:159`, `:204` | `:157`, `:201` | owner input | expected | check |
| 39–41 | H | 22 | `src/cli/scripts/sb_convert.rs:86`, `:167`, `:223` | `:72`, same lines | owner input | expected: `sbrs convert FILE` | check |
| 42, 43 | H | 22 | `src/cli/scripts/sb_grid.rs:1023`, `:1088` | `:1019`, `:1089` | owner input | expected | check |
| 44 | H | 22 | `src/cli/scripts/sb_objectify/api.rs:460` | `:459` | owner input | `read(path)` of `sbrs extract-objects FILE` | read |
| 45–47 | H | 22 | `sb_objectify/api.rs:466`, `:493`, `:500` | `:465`, `:492` | fixed name | `page_objects.rs` next to the owner's file | read |
| 48 | H | 22 | `src/cli/scripts/sb_recorder.rs:377` | `:376` | owner input | expected | check |
| 49 | H | 22 | `src/cli/scripts/sb_translate.rs:78` | same line | owner input | expected | check |
| 50 | M | 330 | `src/config/proxy_list.rs:458` | none | not this weakness | `StdRng::seed_from_u64` runs only when the owner sets a seed, to repeat an order; otherwise `rand::rng()`; choosing a proxy is no secret | read |
| 51, 52 | H | 22 | `src/config/settings.rs:247`, `:264` | `:246`, `:263` | owner input | expected: the settings file | check |
| 53 | H | 22 | `src/config/settings_file.rs:62` | `sb_convert.rs:72` | owner input | expected; the file changed after the scan, map the line | check |
| 54 | H | 22 | `src/core/allure.rs:998` | same line | fixed name or test code | expected | check |
| 55 | H | 22 | `src/core/dashboard.rs:1421` | same line | fixed name or test code | expected | check |
| 56, 57 | H | 22 | `src/core/junit.rs:676`, `:680` | `:675` | fixed name or test code | expected | check |
| 58 | H | 22 | `src/core/log_helper.rs:829` | `:823` | fixed name | expected | check |
| 59, 60 | H | 22 | `src/core/report_helper.rs:556`, `:562` | `:555` | fixed name | expected | check |
| 61, 62 | H | 22 | `src/core/test_run.rs:392`, `:422` | `:391`, `:421` | fixed name | `RUN_FILE`, `RESULTS_FILE` under the run folder | read |
| 63 | M | 117 | `src/core/test_run.rs:441` | `:421` | not this weakness | `tracing::warn!` of a `serde_json` error about a line of the results file this program writes in the owner's folder | read |
| 64 | H | 22 | `src/evasion/fingerprint/file.rs:84` | `:66` | owner input | `Fingerprint::load(path)` | read |
| 65 | H | 22 | `src/evasion/fingerprint/file.rs:240` | `:237` | test code | `a_drawn_identity_survives_a_save_and_a_load...` | read |
| 66 | M | 319 | `src/evasion/reactor.rs:145` | none | not this weakness | `http://{host}:{port}/json/version`: Chrome's DevTools endpoint has no TLS, and the owner names the host | read |
| 67, 69 | H | 22 | `src/sb_cdp/launch.rs:671`, `:723` | `:669` | fixed name | `DevToolsActivePort` in the profile folder | read |
| 68 | C | 78 | `src/sb_cdp/launch.rs:703` | `src/config/runtime.rs:109`, `src/evasion/fingerprint/file.rs:66` | not this weakness | `Command::new(&binary).args(...)`: no shell; the binary comes from the owner's settings; a fingerprint value only becomes the value of one `--user-agent=`, `--lang=` or `--window-size=` argument, never an argument of its own; switches (`extra_args`) come from the owner's settings | read |
| 70, 71 | H | 22 | `src/sb_cdp/launch.rs:770`, `:787` | `:769` | fixed name | expected: the profile's preference files | check |
| 72 | M | 319 | `src/sb_cdp/launch.rs:855` | none | not this weakness | `http://127.0.0.1:{port}/json/version`, loopback | read |
| 73 | H | 22 | `src/sb_cdp/launch.rs:1197` | same line | fixed name or test code | expected | check |
| 74–77 | H | 22 | `src/sb_cdp/trace.rs:1174`, `:1437`, `:1603`, `:1625` | same lines | test code | trace tests reading files of a temporary folder | read |
| 78 | H | 22 | `src/testing/evidence.rs:245` | `:244` | test code | a test helper `read(path)`; confirm it is under `#[cfg(test)]` | check |
| 79 | H | 22 | `src/testing/lifecycle.rs:933` | same line | test code | a lifecycle test | read |
| 80 | H | 22 | `src/utilities/grid_server.rs:318` | `:317` | owner input or fixed name | expected | check |
| 81 | H | 22 | `src/utilities/grid_server.rs:922` | `:919` | test code | `log_with`, a test helper | read |

Rows 5–9, 12–13, 18–28, 37–47, 56–57, 59–62, 67/69, 70–71 and 74–77 are each one finding per sink;
write one row per finding in the note (81 rows), so the count matches the report.

**Fingerprint docs.** In `docs/stealth/fingerprints.md`, next to where it shows loading a file
(`Fingerprint::load`), add: "A fingerprint file is configuration, like a settings file: it can name
a proxy and the proxy's login, so load only files you made or have read."

**Registry.** `docs/SUMMARY.md`: after `  - [Security hardening plan](design/security_hardening_plan.md)`
add `  - [Triage of the GitLab Advanced SAST findings](design/security_triage.md)`. `docs/docs.json`:
after `"design/security_hardening_plan"` add `"design/security_triage"` (one-line edits: add the comma,
then `insert_at` the new line).

**CHANGELOG** (`### Security` of `[Unreleased]`, line 302):

```text
- The 81 findings of GitLab Advanced SAST (pipeline 3039131874) were triaged: none is a
  vulnerability. `docs/design/security_triage.md` gives the reason for each, to read the next scan
  against.
```

Then `rtk python3 -I tools/docs7_changelog.py`.

**Acceptance.** 81 rows, every "check" row read; the counts match the report (1 C, 74 H, 6 M).

**Commit.** `docs(design): triage of the GitLab Advanced SAST findings`

## Step 2: bound the root-branch check of an XPath

**Files.** `src/utils/selectors.rs`, `CHANGELOG.md`.

**Decision.** A depth bound, not an explicit worklist. A worklist removes the stack frames but
keeps the work quadratic (each level splits the rest of the string again: 10,000 parentheses are
about a hundred million character steps). A bound of 64 groups makes both the stack and the time linear.
Past the bound the branch counts as *starting at the root*, which is the safe answer for both
callers: `Step::confine` then filters matches to the element's scope (a relative XPath's matches are
inside it anyway), and `Element::lookup` refuses the selector. No XPath a person writes nests 64
groups.

**Code** (replaces lines 1040–1066; keep the doc comment of `xpath_root_branch` and add the last
paragraph):

```rust
/// How many groups deep [`xpath_root_branch`] looks. A branch inside more groups than this
/// counts as starting at the root: the answer that keeps a search inside its element.
const MAX_GROUP_DEPTH: usize = 64;

/// The first branch of the union `xpath` that starts at the root of
/// the document, or `None` if every branch starts at the context node.
///
/// (existing paragraph unchanged)
///
/// A branch nested in more than [`MAX_GROUP_DEPTH`] groups counts as starting at the root,
/// so a hostile selector costs linear time and a bounded stack.
pub(crate) fn xpath_root_branch(xpath: &str) -> Option<&str> {
    xpath_union_branches(xpath)
        .into_iter()
        .map(str::trim)
        .find(|branch| branch_starts_at_the_root(branch, 0))
}

/// Whether `branch`, trimmed, starts at the root of the document (see
/// [`xpath_root_branch`]); `depth` is the number of groups around it.
fn branch_starts_at_the_root(branch: &str, depth: usize) -> bool {
    if let Some(group) = branch.strip_prefix('(') {
        if depth >= MAX_GROUP_DEPTH {
            return true;
        }
        return xpath_union_branches(group)
            .into_iter()
            .map(str::trim)
            .any(|inner| branch_starts_at_the_root(inner, depth + 1));
    }
    branch.starts_with('/')
        || branch
            .strip_prefix("id")
            .is_some_and(|call| call.trim_start().starts_with('('))
}
```

Read the first doc line of `xpath_root_branch` at line 1039 before replacing, and keep it word for
word.

**Tests first** (in `mod tests_xpath_branches`, after
`the_root_branch_is_the_first_that_starts_with_a_slash_or_a_call_of_id`):

```rust
#[test]
fn ten_thousand_nested_groups_are_read_on_a_small_stack() {
    let deep = |inner: &str| format!("{}{inner}{}", "(".repeat(10_000), ")".repeat(10_000));
    let (root, relative) = (deep("//a"), deep(".//a"));
    let answers = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            (
                xpath_root_branch(&root).map(str::len),
                xpath_root_branch(&relative).map(str::len),
            )
        })
        .expect("a thread")
        .join()
        .expect("the check returns");
    // Past the bound a branch counts as starting at the root, the answer that confines.
    assert_eq!(answers, (Some(20_003), Some(20_004)));
}

#[test]
fn a_branch_in_more_groups_than_the_bound_counts_as_starting_at_the_root() {
    let nested = |depth: usize| format!("{}.//a{}", "(".repeat(depth), ")".repeat(depth));
    assert_eq!(xpath_root_branch(&nested(MAX_GROUP_DEPTH)), None);
    assert!(xpath_root_branch(&nested(MAX_GROUP_DEPTH + 1)).is_some());
}
```

Red: add `const MAX_GROUP_DEPTH: usize = 64;` alone first (unused), then the two tests, then run.
The first test aborts the test binary with "has overflowed its stack" (that is the red; the filter
keeps the abort to these tests); the second fails its `None` assertion only if a depth counter
exists, so with the old code it passes for 64 and fails nothing: it is a guard for the bound, not a
red test.

**Commands.**

```bash
rtk proxy cargo test -p seleniumbase-rs --lib utils::selectors::tests_xpath_branches   # red, then green
rtk proxy cargo test -p seleniumbase-rs --lib utils::selectors                          # every selector test
rtk proxy cargo test -p seleniumbase-rs --features test-util --test offline sb_cdp_mock::
```

**Acceptance.** Both new tests and every test in `utils::selectors` pass; `tokensave_recursion` on
`src/utils/selectors.rs` still lists the one cycle, now bounded (it cannot see the bound; the test
is the proof).

**CHANGELOG** (`### Security`):

```text
- A selector with thousands of nested XPath groups, such as one an MCP client sends, no longer
  overflows the stack and ends the process: the check of whether an XPath starts at the root of the
  page looks 64 groups deep, and a branch nested deeper counts as starting at the root, so the search
  stays inside its element.
```

**Commit.** `fix(selectors): bound how deep the root-branch check of an XPath looks`

## Step 3: compare bearer-token digests with `subtle`

**Files.** `Cargo.toml`, `src/mcp/http.rs`, `docs/tutorials/mcp_server.md`, `CHANGELOG.md`.

**Dependency.** `rtk cargo add subtle@2.6 --optional` in the worktree root, then add `"dep:subtle"`
to the `mcp-server` feature (`Cargo.toml:187`):
`mcp-server = ["dep:rmcp", "dep:axum", "dep:subtle"]`.

**Code.**

```rust
use subtle::ConstantTimeEq as _;

/// The SHA-256 of `text`. Tokens are compared by their digests, which are 32 bytes whatever was
/// sent, so a token of another length takes the same path as a wrong one.
fn digest(text: &str) -> [u8; 32] { /* unchanged */ }

/// Whether two digests are equal, in time that does not depend on where they differ.
fn same_digest(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left[..].ct_eq(&right[..]).into()
}
```

`subtle`'s slice `ct_eq` returns early only when the lengths differ (its docs), which two
`[u8; 32]` never do; `bool::from(Choice)` is the conversion the docs name for the end of a check.

**Tests.** Keep `the_token_is_a_bearer_credential_and_must_match_exactly` (it already covers the
right token, a wrong one, wrong lengths, a missing header and a wrong scheme). Add:

```rust
#[test]
fn equal_digests_are_the_same_and_one_changed_byte_is_not() {
    let token = digest("s3cret");
    assert!(same_digest(&token, &digest("s3cret")));
    for at in [0, 15, 31] {
        let mut other = token;
        other[at] ^= 1;
        assert!(!same_digest(&token, &other), "byte {at}");
    }
}
```

This is a refactor with unchanged behaviour: the new test is green before and after the change. Say
so in the commit body.

**Commands.** `rtk proxy cargo test -p seleniumbase-rs --features mcp-server --lib mcp::http`, then
`rtk proxy cargo test -p seleniumbase-rs --features "mcp-server test-util" --test offline mcp_http::`.

**Docs.** `docs/tutorials/mcp_server.md`, in the paragraph on the bearer token (near line 1393):
"The server compares the token it is sent with its own in constant time."

**CHANGELOG** (`### Security`):

```text
- The MCP HTTP server compares the SHA-256 digests of the sent and the expected bearer token with
  `subtle`'s constant-time equality, in place of a loop of its own; both digests are 32 bytes, so a
  token of another length takes the same path.
```

**Commit.** `refactor(mcp): compare bearer-token digests with subtle`

## Step 4: hold the MCP bearer token as a `SecretString`

**Files.** `Cargo.toml`, `crates/cli/Cargo.toml`, `src/mcp/mod.rs`, `src/mcp/http.rs`,
`src/mcp/client.rs`, `crates/cli/src/mcp_server.rs`, `docs/tutorials/mcp_server.md`, `CHANGELOG.md`.

**Dependencies.** Library: `rtk cargo add secrecy@0.10.3 --optional`, and `"dep:secrecy"` in
`mcp-server` (which `mcp-client` includes). CLI: `rtk cargo add -p seleniumbase-rs-cli secrecy@0.10.3`
(a direct dependency: `crypto.rs` needs it in step 6 even in a build without `mcp-server`).

**Public API** (breaking; no shims):

```rust
// src/mcp/mod.rs, after `pub use http::{...}`:
/// The type of the bearer token (`HttpOptions::token`, `generate_token`,
/// `client::Target::Http`) and the trait whose `expose_secret` reads it.
pub use secrecy::{ExposeSecret, SecretString};

// src/mcp/http.rs
pub struct HttpOptions {
    /// The bearer token every request must carry, if any. Required when `listen` is not a
    /// loopback address. Held as a secret: `Debug` never shows it, and its memory is wiped when
    /// it is dropped.
    pub token: Option<SecretString>,
    // other fields unchanged
}
pub fn with_token(mut self, token: impl Into<String>) -> Self {
    self.token = Some(SecretString::from(token.into()));
    self
}
pub fn generate_token() -> Result<SecretString, SeleniumBaseError>;   // the same 64 hex digits
// validate: `check_token(token.expose_secret())?`
// Gate::new: `token_digest: options.token.as_ref().map(|token| digest(token.expose_secret())),`

// src/mcp/client.rs
#[derive(Debug, Clone)]   // PartialEq and Eq removed: no code compares targets
#[non_exhaustive]
pub enum Target {
    Http {
        url: String,
        /// The bearer token the server asks for, without the `Bearer ` prefix. Held as a
        /// secret: `Debug` never shows it.
        bearer: Option<SecretString>,
    },
    Spawn { /* unchanged */ },
}
// connect: `config = config.auth_header(token.expose_secret().to_owned());`
```

Hover `StreamableHttpClientTransportConfig::auth_header` with rust-analyzer before the edit; it took
the `String` before, and `.to_owned()` fits either a `String` or an `impl Into<String>` parameter.
Keep the hand-written `Debug` of `HttpOptions` (it prints `<redacted>`, which its test checks).

**CLI** (`crates/cli/src/mcp_server.rs`):

```rust
use secrecy::{ExposeSecret, SecretString};

struct Http {
    options: HttpOptions,
    generated: Option<SecretString>,
}

/// The bearer token from `file`, else from `SB_MCP_TOKEN`, trimmed; `None` when neither is set.
fn http_token(file: Option<(&str, &str)>) -> Result<Option<SecretString>, SeleniumBaseError> {
    if let Some((file, how)) = file {
        let text = SecretString::from(std::fs::read_to_string(file).map_err(/* unchanged */)?);
        warn_if_shared(file);
        let token = text.expose_secret().trim();
        if token.is_empty() { /* unchanged error */ }
        return Ok(Some(SecretString::from(token)));
    }
    match std::env::var(TOKEN_VAR).map(SecretString::from) {
        Ok(value) if !value.expose_secret().trim().is_empty() => {
            Ok(Some(SecretString::from(value.expose_secret().trim())))
        }
        Ok(_) | Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(/* unchanged */),
    }
}
```

`announce_token(token: &str, ...)` keeps its signature; its caller passes `token.expose_secret()`
(find it with `tokensave_callers` on `announce_token`). Printing the made token once on standard
error is the point of that function. `mcp_client/args.rs:104` needs no change: the types flow.

**Tests first.**

* `src/mcp/client.rs`: `a_target_never_shows_its_bearer_token`: build
  `Target::Http { url: "http://127.0.0.1:1/mcp".to_owned(), bearer: Some(...) }` with the token
  `t0p-s3cr3t-b34r3r`, and assert that `format!("{target:?}")` does not contain it. Red with the
  `String` field (derived `Debug` prints it); then change the field and build it with
  `SecretString::from("t0p-s3cr3t-b34r3r")`. Update `a_target_is_named_by_its_url_or_its_command` to
  `Some(SecretString::from("secret"))`.
* `crates/cli/src/mcp_server.rs` tests (run `tokensave_test_map` on the file first and put it next to
  the existing `http_token` tests): `a_token_file_is_trimmed_and_kept_secret`: write `"  abc-token\n"`
  to a temporary file, call `http_token(Some((path, "--token-file")))`, assert
  `expose_secret() == "abc-token"` and that `format!("{result:?}")` does not contain `abc-token`. Red
  while the result is `Option<String>`.
* `src/mcp/http.rs`: `a_made_token_is_long_random_hex` reads `first.expose_secret()` and compares
  `first.expose_secret() != second.expose_secret()`. The existing `Debug` test keeps passing.

**Commands.**

```bash
rtk proxy cargo test -p seleniumbase-rs --features mcp-client --lib mcp::client
rtk proxy cargo test -p seleniumbase-rs --features mcp-server --lib mcp::http
rtk proxy cargo test -p seleniumbase-rs-cli --bin sbrs-mcp
rtk proxy cargo test -p seleniumbase-rs-cli --test offline -- mcp_http:: mcp_env:: mcp_client::
rtk proxy cargo test -p seleniumbase-rs --features "mcp-server test-util" --test offline mcp_http::
```

**Docs.** `docs/tutorials/mcp_server.md`, the same paragraph: "It keeps the token as a secret: it is
never written to a log or by `Debug`, and its memory is wiped when the server ends." No
`tests/contract_*.rs` names `HttpOptions` or `Target` (checked); run `mise run contracts` anyway.

**CHANGELOG** (`### Breaking`):

```text
- **Breaking: the MCP bearer token is a `SecretString`.** `mcp::HttpOptions::token` is
  `Option<SecretString>`, `mcp::generate_token` returns a `SecretString`, and
  `mcp::client::Target::Http::bearer` is `Option<SecretString>`; `Target` no longer implements
  `PartialEq` and `Eq`. `seleniumbase_rs::mcp` re-exports `SecretString` and `ExposeSecret` (read a
  token with `expose_secret()`). `HttpOptions::with_token` still takes a string. `Debug` never shows
  a token, and its memory is wiped when it is dropped.
```

**Commit.** `feat(mcp)!: hold the HTTP bearer token as a SecretString`

## Step 5: argon2id tokens in format 2

**Files.** `Cargo.toml`, `src/common/encryption.rs`, `crates/cli/src/sbrs.rs` (`wrong_passphrase`),
`crates/cli/src/crypto.rs` (test helper `tokens`), `crates/cli/tests/offline/cli_crypto.rs:48`,
`cli_aliases.rs:61`, `cli_e2e.rs:539`, docs listed below, `parity/api.toml`,
`tools/parity/seed_manifest.py`, `CHANGELOG.md`.

**Dependencies** (both unconditional: `common::encryption` is always compiled):
`rtk cargo add argon2@0.6 --no-default-features --features alloc,zeroize` and
`rtk cargo add zeroize@1`. `cargo add` refuses a feature that does not exist; if `zeroize` is not a
feature of `argon2` 0.6, drop it (our key buffer is wiped by `Zeroizing` either way) and say so.
`argon2` 0.6.0 needs Rust 1.85; the crate is on 1.99.

**Decisions.**

* **KDF.** argon2id (`Algorithm::Argon2id`, `Version::V0x13`), 19 MiB (19,456 KiB), 2 passes, one
  lane, a 32-byte key: OWASP's minimum for argon2id. `Params::DEFAULT_T_COST` (2) and
  `DEFAULT_P_COST` (1) of `argon2` 0.6.0 agree; docs.rs does not render `DEFAULT_M_COST`, so the plan
  writes the memory cost itself.
* **The costs travel in the token, bounded.** Tests and later tuning need other costs, and a fixed
  cost per format version would make every test derive a 19 MiB key in an unoptimized build. A token
  is untrusted, so decryption refuses more than 64 MiB or 8 passes, and less than argon2's minimum of
  8 KiB, before it derives anything: the same rule the iteration count of format 1 had.
* **Format.** `sbenc2:` followed by the URL-safe base64, without padding, of:

  | Bytes | Field |
  |---|---|
  | 0–3 | memory cost in KiB, big-endian `u32` |
  | 4–7 | passes, big-endian `u32` |
  | 8–23 | salt, 16 random bytes |
  | 24–35 | nonce, 12 random bytes |
  | 36– | AES-256-GCM ciphertext, then its 16-byte tag |

  `sbenc` is the magic and `2` the version. They are text, not bytes, so people, secret scanners and
  the decrypt error can tell the formats apart. The associated data is `sbenc2:` followed by bytes
  0–23, so a changed header fails like a changed ciphertext. URL-safe base64 needs no escaping in a
  shell, a URL or a TOML string. Before writing the code, confirm
  `base64::engine::general_purpose::URL_SAFE_NO_PAD` in `base64` 0.23 (Context7 or docs.rs).
* **Format 1 is refused, not read.** A `sbenc1:` token gets an `InvalidConfig` that says to decrypt
  it with the sbrs that made it and encrypt the text again. A higher version gets "newer format".
* **Errors.** Malformed, retired, newer and over-bound tokens: `InvalidConfig`, before any key is
  derived. A wrong passphrase or a modified token: `Authentication` (it was `Unsupported`, which
  means something else; the vault already uses `Authentication` for a wrong passphrase).
* **Signatures stay `&str` in, `String` out.** `docs/concepts/encryption.md` and the snippet that
  `encrypt_loop` prints call them with `&str`, and this module is compiled without `secrecy`. The
  secrets inside are wiped: the derived key is `Zeroizing<[u8; 32]>`, and the buffer that held the
  plaintext is sealed in place (sized so that appending the tag does not reallocate).
* **The profile vault keeps PBKDF2.** `src/storage/vault.rs` (`turso`) stores its salt and iteration
  count in its own header and calls `derive_key`; moving it needs a vault schema version. `derive_key`,
  `PBKDF2_ITERATIONS` and `SALT_LEN` stay, documented as the vault's.
* `aes_encrypt` and `aes_decrypt` (raw-key, public, `Aad::empty`) are not passphrase tokens and stay.

**Code** (`src/common/encryption.rs`; `encrypt_with_iterations` and `TOKEN_PREFIX = "sbenc1"` go):

```rust
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use zeroize::Zeroizing;

/// PBKDF2-HMAC-SHA256 work factor of the profile vault (OWASP's 2023 recommendation).
pub(crate) const PBKDF2_ITERATIONS: u32 = 600_000;
/// Salt length of the vault and of a token.
pub(crate) const SALT_LEN: usize = 16;
/// The vault's key: PBKDF2-HMAC-SHA256 (tokens use argon2id, see [`encrypt_with_passphrase`]).
pub(crate) fn derive_key(passphrase: &str, salt: &[u8], iterations: u32)
    -> Result<[u8; 32], SeleniumBaseError>;   // body unchanged

/// The start of a token of the format this module writes: the magic `sbenc`, the version, a colon.
const TOKEN_PREFIX: &str = "sbenc2:";
/// The start of a token of the first format (PBKDF2), refused with the way to convert it.
const RETIRED_PREFIX: &str = "sbenc1:";
/// The magic every format starts with.
const TOKEN_MAGIC: &str = "sbenc";
/// The format version this module writes.
const TOKEN_VERSION: u16 = 2;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;
/// The two costs, the salt and the nonce.
const HEADER_LEN: usize = 8 + SALT_LEN + NONCE_LEN;

/// The argon2id costs a token is made with, written in its header. The parallelism is 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TokenCost {
    /// Memory, in KiB.
    memory_kib: u32,
    /// Passes over the memory.
    passes: u32,
}

impl TokenCost {
    /// OWASP's minimum for argon2id: 19 MiB, 2 passes, one lane.
    const DEFAULT: Self = Self { memory_kib: 19 * 1024, passes: 2 };
    /// The least argon2 takes with one lane.
    const MIN_MEMORY_KIB: u32 = 8;
    /// The most a token may ask for: it is untrusted input.
    const MAX_MEMORY_KIB: u32 = 64 * 1024;
    const MAX_PASSES: u32 = 8;

    fn is_allowed(self) -> bool {
        (Self::MIN_MEMORY_KIB..=Self::MAX_MEMORY_KIB).contains(&self.memory_kib)
            && (1..=Self::MAX_PASSES).contains(&self.passes)
    }
}

/// The token key: argon2id over `passphrase` and `salt` at `cost`, wiped when dropped.
fn token_key(passphrase: &str, salt: &[u8], cost: TokenCost)
    -> Result<Zeroizing<[u8; 32]>, SeleniumBaseError>
{
    let params = argon2::Params::new(cost.memory_kib, cost.passes, 1, Some(32))
        .map_err(|error| SeleniumBaseError::InvalidConfig(format!("argon2id costs: {error}")))?;
    let kdf = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = Zeroizing::new([0_u8; 32]);
    kdf.hash_password_into(passphrase.as_bytes(), salt, &mut key[..])
        .map_err(|error| SeleniumBaseError::Unsupported(format!("argon2id failed: {error}")))?;
    Ok(key)
}

/// The AES-256-GCM key for `key`.
fn sealing_key(key: &[u8; 32]) -> Result<LessSafeKey, SeleniumBaseError>;   // UnboundKey::new + LessSafeKey::new, as aes_encrypt does

/// The associated data of a token: its prefix, costs and salt.
fn associated_data(costs_and_salt: &[u8]) -> Vec<u8> {
    [TOKEN_PREFIX.as_bytes(), costs_and_salt].concat()
}

/// [`encrypt_with_passphrase`] at `cost`; the tests use a cheap one.
fn encrypt_with_cost(plaintext: &str, passphrase: &str, cost: TokenCost)
    -> Result<String, SeleniumBaseError>;
// 1. refuse an empty passphrase (InvalidConfig, the message of today);
// 2. body = memory_kib.to_be_bytes() ++ passes.to_be_bytes() ++ salt (SystemRandom) ++ nonce (SystemRandom);
// 3. key = token_key(passphrase, &salt, cost)?;
// 4. let mut sealed = Vec::with_capacity(plaintext.len() + TAG_LEN); sealed.extend_from_slice(plaintext.as_bytes());
//    sealing_key(&key)?.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce),
//        Aad::from(associated_data(&body[..8 + SALT_LEN])), &mut sealed)  -> Unsupported on error;
// 5. body.extend_from_slice(&sealed); Ok(format!("{TOKEN_PREFIX}{}", URL_SAFE_NO_PAD.encode(&body)))

/// Encrypts `plaintext` under a key derived from `passphrase` with argon2id (19 MiB, 2 passes)
/// and a fresh random salt, with AES-256-GCM.
///
/// The same text encrypts differently each time, and a wrong passphrase or a modified token fails
/// to decrypt rather than yielding garbage. The token is `sbenc2:` followed by the URL-safe
/// base64, without padding, of the costs, the salt, the nonce and the ciphertext with its tag; the
/// prefix, the costs and the salt are authenticated with the ciphertext.
///
/// This is not compatible with Python SeleniumBase's `sbase encrypt`, which is a reversible
/// obfuscation with a fixed key, nor with the `sbenc1:` tokens of earlier seleniumbase-rs.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for an empty passphrase.
pub fn encrypt_with_passphrase(plaintext: &str, passphrase: &str) -> Result<String, SeleniumBaseError> {
    encrypt_with_cost(plaintext, passphrase, TokenCost::DEFAULT)
}

/// Decrypts a token made by [`encrypt_with_passphrase`].
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] for a token that is malformed, of the first
/// format (`sbenc1:`) or of a newer one, or that asks for more than 64 MiB or 8 passes, all before
/// any key is derived; and [`SeleniumBaseError::Authentication`] if the passphrase is wrong or the
/// token was modified.
pub fn decrypt_with_passphrase(token: &str, passphrase: &str) -> Result<String, SeleniumBaseError>;
// let token = token.trim();
// let Some(encoded) = token.strip_prefix(TOKEN_PREFIX) else { return Err(refusal_of(token)) };
// let body = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| malformed())?;
// let (memory, rest) = body.split_first_chunk::<4>().ok_or_else(malformed)?;
// let (passes, rest) = rest.split_first_chunk::<4>().ok_or_else(malformed)?;
// let (salt, rest) = rest.split_first_chunk::<SALT_LEN>().ok_or_else(malformed)?;
// let (nonce, sealed) = rest.split_first_chunk::<NONCE_LEN>().ok_or_else(malformed)?;
// if sealed.len() < TAG_LEN { return Err(malformed()) }
// let cost = TokenCost { memory_kib: u32::from_be_bytes(*memory), passes: u32::from_be_bytes(*passes) };
// if !cost.is_allowed() { return Err(InvalidConfig("this token asks for more key-derivation work
//     than a token may: at most 64 MiB and 8 passes")) }
// let key = token_key(passphrase, salt, cost)?;
// let mut opened = Zeroizing::new(sealed.to_vec());
// let plain = sealing_key(&key)?.open_in_place(Nonce::assume_unique_for_key(*nonce),
//     Aad::from(associated_data(&body[..8 + SALT_LEN])), &mut opened[..])
//     .map_err(|_| SeleniumBaseError::Authentication(
//         "the passphrase is wrong, or the token was modified".to_owned()))?;
// String::from_utf8(plain.to_vec()).map_err(|_| InvalidConfig("the token does not hold text"))
// where malformed() = InvalidConfig("not a token made by `sbrs encrypt`")

/// Why `token`, which does not start with [`TOKEN_PREFIX`], is refused.
fn refusal_of(token: &str) -> SeleniumBaseError;
// sbenc1: -> "this token is in the first format (`sbenc1:`, PBKDF2), which this seleniumbase-rs
//            no longer reads: decrypt it with the sbrs that made it, then encrypt the text again
//            with `sbrs encrypt`"
// sbencN: with N a number above TOKEN_VERSION -> "this token is in format N, newer than the format
//            2 this seleniumbase-rs reads: upgrade it"
// anything else -> malformed()
```

`split_first_chunk` is stable since Rust 1.77. Library code takes no `unwrap()`. Hover `Aad::from`,
`seal_in_place_append_tag` and `open_in_place` of ring 0.17 with rust-analyzer: the existing
`aes_encrypt` already passes `&mut Vec<u8>` to the first.

**CLI.** `crates/cli/src/sbrs.rs::wrong_passphrase` matches `SeleniumBaseError::Authentication(_)` in
place of `Unsupported(_)` (message unchanged). `crates/cli/src/crypto.rs::tests::tokens` filters on
`"sbenc2:"`. The three offline tests assert `token.starts_with("sbenc2:")`.

**Tests first** (replace the five passphrase tests at `encryption.rs:220-288`; keep the `xor_*`,
`aes_*` and `an_empty_passphrase_is_refused` tests). Test helpers:
`const CHEAP: TokenCost = TokenCost { memory_kib: 8, passes: 1 };`, plus
`body_of(token) -> Vec<u8>` and `token_of(&[u8]) -> String`, which strip or add the prefix and
the base64.

1. `a_token_round_trips_and_two_encryptions_of_one_text_differ`: two `encrypt_with_cost(..., CHEAP)`
   tokens differ and both decrypt to the text.
2. `a_token_is_the_magic_the_version_and_url_safe_base64`: starts with `sbenc2:`; the rest has no
   `+`, `/` or `=`; `body_of` is `HEADER_LEN + text.len() + TAG_LEN` bytes.
3. `a_token_carries_owasps_argon2id_cost_by_default`: `encrypt_with_passphrase("x", "pass")`; the
   first eight bytes of the body are 19,456 and 2 (one real 19 MiB derivation: the only one in the
   library's tests).
4. `the_wrong_passphrase_is_an_authentication_error`: `Err(Authentication(_))`.
5. `a_change_anywhere_in_the_token_is_refused`: for byte 8 (salt), 24 (nonce), `HEADER_LEN`
   (ciphertext) and the last (tag), flipping one bit gives `Err(Authentication(_))`; so does setting
   the passes from 1 to 2 (allowed, but another key).
6. `a_token_of_the_first_format_is_refused_with_the_way_to_convert_it`:
   `"sbenc1:600000:AAAAAAAAAAAAAAAAAAAAAA==:AAAA"` gives `InvalidConfig` with `sbenc1` and
   `encrypt the text again` in its message.
7. `a_token_of_a_newer_format_is_refused`: `"sbenc3:AAAA"` gives `InvalidConfig` with `newer`.
8. `malformed_tokens_are_refused_before_any_key_is_derived`: `""`, `"plain text"`, `"sbenc2"`,
   `"sbenc2:"`, `"sbenc2:!!!"`, a body of 20 bytes, a body of `HEADER_LEN + TAG_LEN - 1` bytes,
   `"other:AAAA"`: all `InvalidConfig`.
9. `a_token_cannot_ask_for_unbounded_work`: bodies with memory `u32::MAX`, memory 4, memory
   `MAX_MEMORY_KIB + 1`, passes 0 and passes 9: all `InvalidConfig` (memory `u32::MAX` would try to
   allocate 4 TiB if the bound came after the derivation).

Red: add `TokenCost` and `fn encrypt_with_cost(...) -> Result<String, SeleniumBaseError> { todo!() }`
first so the tests compile, then run them: they fail.

**Commands.**

```bash
rtk proxy cargo test -p seleniumbase-rs --lib common::encryption        # red, then green
rtk proxy cargo check -p seleniumbase-rs --features turso               # the vault still builds
rtk proxy cargo test -p seleniumbase-rs-cli --bin sbrs crypto::
rtk proxy cargo test -p seleniumbase-rs-cli --test offline -- cli_crypto:: cli_aliases:: cli_e2e::
```

The CLI tests run the real `sbrs` at the default cost in an unoptimized build: expect each
encryption to take about a second (not measured). If a test takes more than about five seconds,
stop and report the time; the remedy is a separate commit with `[profile.dev.package.argon2]` and
`[profile.dev.package.blake2]` at `opt-level = 3`, after reading the Cargo docs on profile overrides
(test builds inherit `dev`: verify).

**Docs.**

* `docs/concepts/encryption.md` (the token section around lines 165–205): argon2id and its costs,
  the format table above, `sbenc1` refused, the example `assert!(token.starts_with("sbenc2:"))`,
  the CLI example comments (`sbenc2:`).
* `docs/tutorials/cli_usage.md:745-752`: the example tokens `sbenc2:...` and the KDF sentence.
* `docs/glossary.md:1334-1338`: the PBKDF2 entry says that the profile vault uses it and `sbrs
  encrypt` uses Argon2id; add an `Argon2id` entry in the file's order and style: "A memory-hard
  key-derivation function, the winner of the Password Hashing Competition: it needs a block of
  memory as well as time, which makes guessing a passphrase on graphics cards and custom chips
  expensive. `sbrs encrypt` derives its key with Argon2id (19 MiB, 2 passes, one lane: OWASP's
  minimum) and a random 16-byte salt."
* Leave `docs/benchmarks.md:130` and `docs/tutorials/results_and_profiles.md:118` alone: they are
  about the vault, which keeps PBKDF2.
* `parity/api.toml:3060` divergence: "AES-256-GCM under an Argon2id key from SB_ENCRYPTION_KEY
  (asked once, without echo, when it is not set and standard input is a terminal), not Python's
  fixed-key obfuscation; tokens are not interchangeable." Then
  `rtk python3 -I tools/parity/render_docs.py`. `tools/parity/seed_manifest.py:253`: the same text
  without the parenthesis.
* The help of `sbrs encrypt` and `sbrs decrypt` does not name the KDF, so it stays, and so do
  `completions/sbrs.usage.kdl` and `docs/help/cli_reference.md` (the final gate's `usage_spec` test
  proves it).

**CHANGELOG** (`### Breaking`):

```text
- **Breaking: `sbrs encrypt` tokens are argon2id tokens of format 2 (`sbenc2:`).** The key comes
  from argon2id (19 MiB, 2 passes, one lane) over the passphrase and a random 16-byte salt, in
  place of PBKDF2-HMAC-SHA256. The token is `sbenc2:` and the URL-safe base64 of the costs, the
  salt, the nonce and the AES-256-GCM ciphertext; the prefix, the costs and the salt are
  authenticated with it. A token of the first format (`sbenc1:`) is refused with a message that
  says to decrypt it with the sbrs that made it and encrypt the text again, and a token that asks
  for more than 64 MiB or 8 passes is refused before any key is derived. A wrong passphrase or a
  modified token is now `SeleniumBaseError::Authentication` (it was `Unsupported`). The derived key
  and the buffer that held the text are wiped after use. The profile vault keeps PBKDF2.
```

Then `rtk python3 -I tools/docs7_changelog.py`.

**Commit.** `feat(encryption)!: argon2id tokens in a versioned format`

## Step 6: keep the passphrase and hidden input of `sbrs encrypt` as secrets

**Files.** `crates/cli/src/crypto.rs`, `crates/cli/src/sbrs.rs`, `docs/concepts/encryption.md`,
`CHANGELOG.md`.

**Code.**

```rust
use secrecy::{ExposeSecret, SecretString};

pub(crate) trait Input {
    /// A line read without echo, held as a secret, or `None` at the end of the input.
    fn hidden(&mut self) -> io::Result<Option<SecretString>>;
    /// A line read with echo, or `None` at the end of the input.
    fn visible(&mut self) -> io::Result<Option<String>>;
}
// Terminal::hidden: Ok(line) => Ok(Some(SecretString::from(line)))
fn ask_passphrase(input: &mut impl Input, err: &mut impl Write, verify: bool)
    -> Result<Option<SecretString>, Failure>;
// first.expose_secret().is_empty(); first.expose_secret() == second.expose_secret()
pub(crate) fn encrypt_loop(input: &mut impl Input, out: &mut impl Write, err: &mut impl Write,
    passphrase: Option<SecretString>) -> Result<(), Failure>;
// text and again are SecretString; encrypt_with_passphrase(text.expose_secret(), key.expose_secret())
pub(crate) fn decrypt_loop(input: &mut impl Input, out: &mut impl Write, err: &mut impl Write,
    passphrase: Option<SecretString>) -> Result<(), Failure>;
// interactive: std::env::var("SB_ENCRYPTION_KEY").ok().filter(|key| !key.is_empty()).map(SecretString::from)

// crates/cli/src/sbrs.rs
fn encryption_passphrase() -> Result<SecretString, Box<dyn std::error::Error>>;
// Commands::Encrypt: let text = SecretString::from(text_or_stdin(text)?);
//     println!("{}", encrypt_with_passphrase(text.expose_secret(), passphrase.expose_secret())?);
// Commands::Decrypt: decrypt_with_passphrase(&token, passphrase.expose_secret())
```

Tokens (read with echo) and the decrypted text that `decrypt` prints are not secrets of this
program, so they stay `String`. The test `Script` keeps its lines as `String` and wraps them in
`hidden`; `run(script, passphrase: Option<&str>, decrypt)` maps `SecretString::from`.

**Tests first.** In `crypto.rs` tests: `what_is_read_without_echo_is_a_secret`, a type-level
check:

```rust
#[test]
fn what_is_read_without_echo_is_a_secret() {
    fn secret(_: &io::Result<Option<SecretString>>) {}
    let mut script = Script::new(&["p4ss"], &[]);   // use the constructor the tests already have
    secret(&script.hidden());
}
```

Red is a compile error while `hidden` returns `String`; say so in the commit. Every existing
`crypto.rs` test stays green, among them `a_missing_passphrase_is_asked_for_once_and_never_shown`.

**Commands.** `rtk proxy cargo test -p seleniumbase-rs-cli --bin sbrs crypto::`, then
`rtk proxy cargo test -p seleniumbase-rs-cli --test offline -- cli_crypto:: cli_aliases::`.

**Docs.** `docs/concepts/encryption.md`, the `sbrs encrypt` paragraph: "sbrs holds the passphrase
and the texts it reads without echo as secrets, wiped from memory when they are dropped."

**CHANGELOG** (`### Security`):

```text
- `sbrs encrypt` and `sbrs decrypt` hold the passphrase and the texts they read without echo as
  secrets, wiped from memory when they are dropped.
```

**Commit.** `feat(cli): keep the encryption passphrase and hidden input as secrets`

## Step 7: `cargo deny` and `cargo audit`

**Commands.** `rtk cargo deny check` and `rtk cargo audit`, each with its full output in the
scratchpad.

**Expected.** Both pass. The new crates are `argon2` 0.6 (MIT OR Apache-2.0) with what it pulls in
(`blake2`, `base64ct`, `cpufeatures`; verify in `Cargo.lock`), `secrecy` 0.10.3 (MIT OR
Apache-2.0); `subtle` (BSD-3-Clause) and `zeroize` were already in the lockfile and are now direct.
All of these licenses are in `deny.toml`'s `allow` list. A new `multiple-versions` warning is a
warning: list it.

**If either fails** on a crate this lane added, stop and report it; do not swap crates or upgrade
others in this lane. A failure on a crate the lane did not touch goes into the triage note as
"found, not this lane's".

**Docs.** The "Dependencies" section of `docs/design/security_triage.md`: the two commands, their
exit codes, the new crates with versions and licenses, and the warnings.

**Commit.** `docs(design): record cargo deny and cargo audit after the hardening` (only if the note
changed; a lockfile change made by `cargo add` belongs to the step that added the crate).

## Step 8: the gate

Run once, on the last commit, one command at a time:

```bash
export CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
rtk cargo fmt --all -- --check
rtk proxy cargo clippy --all-targets --features "mcp-server test-util testing bdd" -- -D warnings
rtk proxy cargo test --features "mcp-server test-util testing bdd"
RUSTDOCFLAGS="-D warnings" rtk proxy cargo doc --no-deps --features "mcp-server test-util testing bdd"
rtk proxy cargo test -p seleniumbase-rs --features mcp-server api_index
rtk proxy cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec
rtk proxy cargo check -p seleniumbase-rs --features turso
mise run contracts
```

If `api_index` fails, write it again with
`SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index` and commit
(`chore(mcp): regenerate the api index`). Then the TokenSave checks the quality gate asks for:
`tokensave_unsafe_patterns`, `tokensave_unused_imports` and `tokensave_doc_coverage` on
`src/common/encryption.rs`, `src/mcp/http.rs`, `src/mcp/client.rs`, `src/utils/selectors.rs`.
No Chrome test is needed: no step changes what a browser sees. Run
`cargo test -p seleniumbase-rs --test chrome element_chrome:: -- --ignored --test-threads=1` with
`SB_HEADLESS=true` only if the owner asks for one. It exercises `Element::lookup`.

## What the plugins repository needs (IMAP)

Core holds no IMAP password; this is for `seleniumbase-rs-plugins` (the `imap` plugin of
`seleniumbase-rs-plugins-builtin`). Its source is not in this repository and was not read, so the
list says what to look for, not what is there:

* Read the password with `std::env::var(password_env).map(SecretString::from)` and keep it in the
  built plugin (`Imap`) as `secrecy::SecretString`; expose it only to build the login command.
* Wipe the buffer the login command is written into (`zeroize::Zeroizing<Vec<u8>>` or `String`).
* `Debug` of `Imap` and `ImapConfig` must not show the password (`password_env` is a name and may be
  shown); a test that formats both with `{:?}` and looks for a known password.
* An error of a failed login must not quote the command it sent.
* When it is done, bump the git rev in core's `Cargo.toml`; core's `src/mcp/email.rs` needs no
  change.

## What is left out, and why

* **The vault on argon2id**: its header and schema version are a lane of their own, and its
  passphrase and key handling (`VaultKey`) were not read for this plan.
* **The proxy login as a `SecretString`** (`ProxyConfig`, `--proxy USER:PASS@...`): not in the goal;
  a follow-up with the fingerprint file's proxy.
* **Copies we do not own**: the process environment (`SB_MCP_TOKEN`, `SB_ENCRYPTION_KEY`), the
  header buffers of hyper, axum and rmcp, the `String` that `rpassword` returns before it is
  wrapped, and stack copies. `secrecy` wipes the heap copy we hold.
* **Wiping the random bytes of `generate_token`**: 32 bytes on the stack whose hex form is the
  secret we do keep.
* **Dismissing the findings in GitLab** and a `sast-ruleset.toml`: dismissal changes the owner's
  project, and a ruleset would hide later findings of the same rules; the triage note is the
  reference.
* **A per-package `opt-level` for argon2**: only if step 5 measures slow tests.
* **`aes_encrypt` and `aes_decrypt`**: public, raw-key, no passphrase; out of the goal.

## Decisions, with their reasons

1. **Item 1 keeps the digests.** They give equal lengths for free; `subtle` replaces only the loop
   (its slice comparison returns early on a length mismatch, which 32-byte digests never have).
2. **argon2id at OWASP's minimum** (19 MiB, 2 passes, one lane), from the RustCrypto `argon2` 0.6.0.
3. **Costs in the authenticated header, bounded** (8 KiB to 64 MiB, 1 to 8 passes), so that tests
   can be cheap and costs can change without a format version, while a hostile token cannot ask
   for unbounded work.
4. **A printable `sbenc2:` prefix** as magic and version, then URL-safe base64 of the binary body;
   the prefix, costs and salt are associated data.
5. **Format 1 is refused with the way out**, not read: reading it would keep PBKDF2 alive for
   tokens.
6. **`Authentication` for a wrong passphrase**, in place of `Unsupported`.
7. **The library keeps `&str` signatures**; the CLI holds the secrets, the library wipes what it
   derives.
8. **The vault keeps PBKDF2** in this lane.
9. **`SecretString` for the MCP token and bearer**, re-exported from `seleniumbase_rs::mcp`; `Target`
   loses `PartialEq` (no code compares targets; a secret should not be compared in passing).
10. **What is read without echo is a secret** in the CLI's `Input` trait.
11. **The selector check is bounded at 64 groups** with "starts at the root" past it: linear and
    safe for both callers. A worklist would stay quadratic.
12. **No SAST finding is a vulnerability**; owner input is trusted input; the untrusted inputs are
    confined already, and the note says where.
13. **The triage goes first**, so the lines it reads are still the scanned ones.

## Risks, and how to see them

| Risk | How it shows | What to do |
|---|---|---|
| argon2 is slow in unoptimized test builds | CLI tests take seconds each; cargo reports a test running for over 60 s | Report the time; a later commit with per-package `opt-level` |
| `argon2` 0.6 feature names differ | `cargo add` refuses `zeroize` | Drop that feature, say so |
| `rmcp`'s `auth_header` takes another type | `cargo check` fails in `client.rs` | Hover it; `.to_owned()` fits `String` and `impl Into<String>` |
| A downstream crate compares `Target` | Its build fails on `PartialEq` | Breaking changes are fine; compare `label()` |
| Users' `sbenc1:` tokens stop decrypting | Their tests fail with the new message | By design; the message and CHANGELOG say how to convert |
| A "check" row of the triage is a real traversal | The source is a page, a network answer, an archive or an MCP argument | Stop, add a fix step test-first, one commit |
| The bound changes the answer for a selector nested more than 64 groups deep | A selector test fails | None exists; if one does, raise the bound, do not remove it |
| `secrecy` changes its `Debug` text | Tests that match the text break | Tests check only that the secret is absent |
| A hostile token makes decrypt use 64 MiB | Memory peak of one decryption | Accepted: bounded, and only the owner's own CLI and tests decrypt |
| The next scan reports the same 81 findings | GitLab lists them again | The owner dismisses them, linking the triage note |

## Fallbacks used while writing this plan

* `tokensave_read` refused a scratchpad file outside the graph root (the table of SAST sources and
  sinks): `rtk read` read it, once.
* RTK's filter truncated a pipeline whose output went to a file: rebuilt with `rtk proxy`.
* The Context7 plugin's monthly quota ran out: the Context7 connector answered for `argon2`, and
  docs.rs pages (fetched with curl into the scratchpad) gave the constants and the signature of
  `hash_password_into` in `argon2` 0.6.0; the crates.io API gave the newest versions.
* One `rtk rg` aimed at `src/error.rs` was refused by the hook; `tokensave_search` had already
  answered it (`SeleniumBaseError::Authentication`, error.rs:171).
