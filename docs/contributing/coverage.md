---
title: "Code coverage"
sidebarTitle: "Coverage"
description: "Measure which lines the tests of seleniumbase-rs reach with cargo llvm-cov or tarpaulin, with a mocked browser and with real Chrome run headless."
keywords: ["coverage", "llvm-cov", "tarpaulin", "nextest", "tests", "headless", "chrome"]
---

This page says how to measure which lines of the crate its tests reach. Part of
the suite needs no browser, because it drives the pure CDP engine through a
mock; the rest starts a real Chrome. The page covers both, and how to run the
second part headless.

<Info>

**What you will learn**

- Which tests need no browser, and which start a real Chrome.
- How to measure coverage with `cargo llvm-cov` or `cargo tarpaulin`.
- How to add the real-Chrome tests to a report, headless, one binary at a time.
- What no CI job does today.

</Info>

<Visibility for="agents">

Summary for AI exports: the tests that need no browser are the library unit tests and the `tests/sb_cdp_*.rs` files without a `_chrome` suffix, which use `Browser::new_mocked()` (feature `test-util`). The tests that start Chrome are `tests/*_chrome.rs`, `tests/browser_smoke.rs` and `crates/cli/tests/*_chrome.rs`; they are `#[ignore]`d, so add `-- --ignored`, and run one binary at a time. Measure with `cargo llvm-cov --features "mcp-server test-util testing bdd" --html` (report in `target/llvm-cov/html`) or `cargo tarpaulin --features "mcp-server test-util testing bdd" --out Html`. No CI job measures coverage. The commands on this page were not run when it was written.

</Visibility>

<Warning>

A coverage run builds the tests again with instrumentation, so treat it like a
cold build: run it alone, with the settings from
[Keep builds small](/contributing/from-source#keep-builds-small). The commands
on this page were **not run** when it was written, which is why each block is
marked. Read the tool's own help before you rely on a flag.

</Warning>

## What needs a browser

| Tests | Where | Needs Chrome | How to run them |
|---|---|---|---|
| Library unit tests | `src/` | <Badge color="green">no</Badge> | `cargo test --lib` |
| Integration tests with a mocked browser | `tests/sb_cdp_*.rs` without `_chrome`, such as `sb_cdp_mock.rs` | <Badge color="green">no</Badge> | `cargo test --features "mcp-server test-util" --test sb_cdp_mock` |
| Real-Chrome tests of the library | `tests/*_chrome.rs`, `tests/browser_smoke.rs` | <Badge color="orange">yes</Badge> | `-- --ignored`, one binary at a time |
| Tests of the `sbrs` binaries | `crates/cli/tests/` | only the `*_chrome.rs` files | `cargo test -p seleniumbase-rs-cli` |

The mocked browser is `Browser::new_mocked()` with a `MockCtrl` that answers
CDP commands, behind the `test-util` feature; see
[Reproduce without Chrome](/contributing/debugging#reproduce-without-chrome).
A report from the first two rows alone tells you what is reached without a
browser; code it shows as missed is reached, if at all, only by the tests that
start a real Chrome.

## Measure the tests that need no browser

<Tabs>

<Tab title="cargo llvm-cov" icon="chart-pie">

<Badge color="orange" icon="flask" stroke>Not run here</Badge>

```bash Terminal
cargo +stable install cargo-llvm-cov --locked
cargo llvm-cov --features "mcp-server test-util testing bdd" --html
```

`--html` writes the report to `target/llvm-cov/html`; add `--open` to open it in
a browser. With cargo-nextest installed, the same runs through it:

```bash Terminal
cargo llvm-cov nextest --features "mcp-server test-util testing bdd"
```

nextest does not run doctests. `--ignore-filename-regex` leaves files out of the
report, and `--fail-under-lines 80` makes the command exit with status 1 below
80 per cent of lines.

</Tab>

<Tab title="cargo tarpaulin" icon="chart-line">

<Badge color="orange" icon="flask" stroke>Not run here</Badge>

```bash Terminal
cargo install cargo-tarpaulin --locked
cargo tarpaulin --features "mcp-server test-util testing bdd" --out Html --output-dir target/coverage
```

The library's own unit tests alone, as the repository's tool notes run it:

```bash Terminal
cargo tarpaulin --features "mcp-server test-util" --lib
```

`--engine llvm` picks the LLVM instrumentation instead of ptrace, `--timeout`
sets the test timeout, and `--skip-clean` skips the clean step to speed up later
runs.

</Tab>

</Tabs>

<Note>

`cargo llvm-cov` sets `RUSTC_WRAPPER` itself unless you pass
`--no-rustc-wrapper`. [mr-boxington](/contributing/from-source#keep-builds-small)
caches nothing while `RUSTC_WRAPPER` is set, so expect a coverage build to be
compiled in full.

</Note>

## Add the tests that start Chrome

The real-Chrome tests are `#[ignore]`d, so the default run skips them. Run them
one test binary at a time: a Chrome that is starved of CPU and memory misses its
20 second DevTools deadline (see
[Tests that need a real Chrome](/contributing/from-source#tests-that-need-a-real-chrome)).
With `cargo llvm-cov`, collect each run without a report and write one report at
the end:

<Badge color="orange" icon="flask" stroke>Not run here</Badge>

```bash Terminal
cargo llvm-cov clean --workspace
cargo llvm-cov --no-report --features "mcp-server test-util testing bdd"
cargo llvm-cov --no-report --features "mcp-server test-util testing bdd" \
    --test sb_cdp_chrome -- --ignored --test-threads=1
cargo llvm-cov report --html
```

Add one `--no-report` line for each `*_chrome.rs` file you want in the report.
Arguments after `--` go to the test binary, as with `cargo test`. To include
doctests in the report, pass `--doctests` to `cargo llvm-cov report`.

`cargo tarpaulin --ignored` runs the ignored tests too, in the same invocation as
the rest; use it only on a machine that can start that many browsers at once.

### Run them headless

A real-Chrome test needs no screen:

- `BrowserConfig` is headless by default, and `SB_HEADLESS=true` makes sure of
  it. The test image of the repository sets it.
- As root or in a container, Chrome needs `--no-sandbox`: the tests pass it when
  `CI` is set.
- A test that needs a headed Chrome gets a virtual display on Linux: set
  `SBRS_XVFB=1` in the test container, or use `--xvfb` /
  `BrowserConfig::with_virtual_display` outside it. The container commands,
  with Xvfb installed, are in
  [Run the tests in a container](/building#run-the-tests-in-a-container).

Real-Chrome behaviour differs between headless and headed runs, so a coverage
figure from one mode says little about the other. The modes themselves are
explained in [Headless mode](/tutorials/headless).

## In CI

<Note>

No CI job and no recipe measures coverage: `.github/workflows/` builds, tests
and lints, and `justfile`, `Makefile` and `mise.toml` have no coverage task. A
job would need the settings above, one cargo command at a time, and a Chrome for
the ignored tests.

</Note>

## Next steps

<Columns cols={2}>

<Card color="#d47628" title="Build from source" icon="hammer" href="/contributing/from-source">

The toolchain, the checks and the real-Chrome tests.

</Card>

<Card color="#1a85d4" title="Debugging" icon="bug" href="/contributing/debugging">

Logs, a mocked browser and stepping through a test with lldb.

</Card>

<Card color="#199f43" title="Headless mode" icon="ghost" href="/tutorials/headless">

How the browser runs without a window.

</Card>

<Card color="#a631be" title="Benchmarking" icon="gauge-high" href="/contributing/benchmarking">

Measure what a change costs.

</Card>

</Columns>
