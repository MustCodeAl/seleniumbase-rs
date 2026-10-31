---
title: "Debug modes"
sidebarTitle: "Debug modes"
description: "Stop a test in a prompt with the browser open: --pdb, --trace, --final-debug and sb.breakpoint."
---

Python SeleniumBase drops into `pdbp` for `--pdb`, `--trace`, `--final-debug` and
`breakpoint()`. A Rust test has no `pdb`, so the library gives the same four
stops a prompt of its own: the test pauses with the browser open, you look at the
page, type a command, and the test goes on or stops. The prompts work in any
test that runs through the `sb` or `sb_cdp` fixture, and in Gherkin steps.

<Info>

**What you will learn**

- How to stop a test after it failed, before it starts, or after it ends.
- How to pause at a line of your own with `sb.breakpoint`.
- Why a prompt needs a terminal, one test at a time, and the cargo runner.

</Info>

<Visibility for="agents">

Summary for AI exports: `sbrs test --pdb` stops in a `[POST MORTEM]` prompt after the last attempt of a failed test, `--trace` in a `[TRACE]` prompt before the body, `--final-debug` (also `--ftrace`, `--fdebug`, `--final-trace`) in a `[FINAL TRACE]` prompt after each test, all with the browser open. `sb.breakpoint("message")` (on `Page`: `page.breakpoint("message")`) pauses at that line. Type `c` and Enter to go on, `q` to stop the test. Any of the three flags makes `sbrs test` run `cargo test -- --test-threads 1 --nocapture` and refuses `--runner nextest` with status 2. Where standard input is not a terminal (CI, a pipe, nextest) a prompt is skipped with a warning in the log; `SB_BREAKPOINTS=0` skips `breakpoint` calls silently.

</Visibility>

## Stop a test

Each stop is an option of `sbrs test`, an `SB_*` variable and a key of the
`[test]` table of the settings file. Pick the one that matches what you want to
see.

<Steps>

<Step title="After a failure: --pdb" icon="skull">

```bash Terminal
sbrs test checkout --pdb
```

When a test fails and its last attempt is over, the test pauses in a
`[POST MORTEM]` prompt with the browser still open and the evidence already
written to `latest_logs/<test id>/`. The prompt shows the code of the test body,
marks the lines that name the failing selector or address with `>>`, and says
what failed. `r` runs the test again with a new browser, once more than
`--reruns` allows; `c` or `q` ends it as failed.

`SB_PDB`, key `pdb`. Python: pytest's `--pdb`.

</Step>

<Step title="Before the body: --trace" icon="flag">

```bash Terminal
sbrs test checkout --trace
```

Each test pauses in a `[TRACE]` prompt before its body runs: the browser is
open and nothing has been done yet. Use it to look at the page the browser
starts on, or to set it up by hand before the test begins.

`SB_TRACE`, key `trace`. Python: pytest's `--trace`.

</Step>

<Step title="After every test: --final-debug" icon="flag-checkered">

```bash Terminal
sbrs test checkout --final-debug
```

After a test ends, pass or fail, it pauses in a `[FINAL TRACE]` prompt before
the browser quits. With reruns it happens after each attempt. `--ftrace`,
`--fdebug` and `--final-trace` are the same flag.

`SB_FINAL_DEBUG`, key `final_debug`. Python: `--final-trace` and `--fdebug`.

</Step>

<Step title="At a line of your own: sb.breakpoint" icon="pause">

```rust tests/checkout.rs
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn checkout(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com/cart").await?;
        sb.breakpoint("Paused at the cart.").await?;
        sb.click("#checkout").await
    })
    .await
}
```

The test pauses in a `[BREAKPOINT]` prompt at that call, and the listing marks
the line with `->`. On the pure CDP engine the call is
`page.breakpoint("Paused at the cart.").await?` inside `sb_cdp.run(async |page| ..)`.
It needs no flag: it pauses whenever the test runs in a terminal, and
`SB_BREAKPOINTS=0` (also `false`, `no` or `off`) turns every `breakpoint` call
off, as Python's `PYTHONBREAKPOINT=0` does.

</Step>

</Steps>

## At the prompt

```text
[BREAKPOINT]: Paused at the cart.
  test:  checkout.checkout   page: https://example.com/cart  "Cart"
Commands: c continue | s screenshot | src page source | u url | js <expr> | l list | w where | dt devtools | q quit | h help
Type 'c' and press [ENTER] to continue execution:
```

Work in the browser as long as you like, then type `c` and press Enter. `q`
stops the test, which then fails with a lifecycle error, `stopped at the debug
prompt`. `s` and
`src` save a screenshot and the page source to `latest_logs/<test id>/`, `js
<expr>` evaluates an expression in the page, and `h` lists the rest. The full
table of commands, the listing and the colours are described in [Writing Tests
with the sb Fixture](/tutorials/writing_tests#debug-prompts).

The time you spend at a prompt counts neither against `--time-limit` nor against
the test's own limit, so a test is not stopped while you look around.

<Warning>

A prompt reads from standard input, so it needs a terminal and one test at a
time.

- **`sbrs test` sets the terminal up for you.** Any of `--pdb`, `--trace` and
  `--final-debug` makes it run `cargo test -- --test-threads 1 --nocapture`, even
  with `-n 4`. In a plain `cargo test`, add `-- --test-threads 1 --nocapture`
  yourself (`SB_PDB=1 cargo test checkout -- --test-threads 1 --nocapture`):
  parallel tests would take turns at the prompt, and the capture would hide it.
- **nextest gives tests no standard input.** `sbrs test --pdb --runner nextest`
  exits with status 2 (`--pdb needs the cargo runner: nextest gives tests no
  standard input`), and a prompt under a plain `cargo nextest run` is skipped.
- **CI and pipes.** Where standard input is not a terminal, a prompt is skipped
  with a warning in the log (`debug prompt skipped: standard input is not a
  terminal`) and the test goes on, so a flag or a `breakpoint` left in a command
  never hangs a run. `SB_BREAKPOINTS=0` skips the breakpoints without a warning.

</Warning>

## In a Gherkin scenario

`--pdb`, `--trace`, `--final-debug` and `breakpoint` work in the steps of a
scenario as in any other test: a failed step panics, and the post-mortem prompt
opens before the browser closes. See [Gherkin scenarios with
rstest-bdd](/tutorials/behave).

## Next steps

<Columns cols={3}>

<Card color="#1a85d4" title="Test options" icon="sliders" horizontal href="/help/test_options">

Every flag of `sbrs test`.

</Card>

<Card color="#199f43" title="Writing Tests with the sb Fixture" icon="flask" horizontal href="/tutorials/writing_tests">

The prompt commands, reruns and evidence.

</Card>

<Card color="#d47628" title="Recorder Mode" icon="circle-dot" horizontal href="/tutorials/recorder_mode">

Record a test by clicking through the page.

</Card>

</Columns>
