---
title: "Behave Commander"
sidebarTitle: "Behave commander"
---

<Badge color="blue">Feature: tui</Badge>

`sbrs behave-gui` (also `sbrs gui-behave`) is a terminal app that lists the
<Tooltip tip="Gherkin is the plain-text format of behave feature files: a Feature, its Scenarios, and Given/When/Then steps.">Gherkin</Tooltip> feature files of a project, runs the scenarios you mark with
`cargo test` and shows what came of each. It is the Python Behave Commander
without the Tkinter window.

<Info>

**What you will learn**

- How to build and launch the app.
- What it lists, and how to pick scenarios and tags.
- How a scenario is matched to a test, and what each result means.

</Info>

## Build and launch

The app is part of the opt-in `tui` feature, with `sbrs commander`:

<CodeGroup>

```bash Run from source
cargo run --features tui --bin sbrs -- behave-gui
```

```bash Install the binary with it
cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --features tui
```

</CodeGroup>

<Note>

An `sbrs` built without the feature still has the subcommand, but it only
prints the following message and exits with status 2, as `commander` does.

```text
this sbrs was built without the terminal UI; reinstall it with `cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --features tui`
```

</Note>

The folder to look in is the optional argument:

```bash
sbrs behave-gui                         # the features/ folder
sbrs behave-gui features/shop           # one folder, and its subfolders
sbrs behave-gui features/login.feature  # one file
```

`sbrs behave-gui --new` skips the app and asks for a feature name, a title and
scenarios, then writes `features/<name>.feature`. It never replaces a file that
is there, and it works in every build.

## What is listed

Every `*.feature` file under the folder (subfolders included, sorted by path;
folders whose name starts with a dot, and `target` and `node_modules`, are not
entered), with its scenarios below it:

<ResponseExample>

```text The list the app shows
  [-] features/login.feature => Log in (3 scenarios)
      [x] line 10: valid login
      [ ] line 14: wrong value
      [ ] line 26: undefined step
```

</ResponseExample>

A feature shows `[x]` when all its scenarios are marked, `[-]` when some are.
An outline is one row, whatever the number of examples.

## Keys

| Key | Action |
|-----|--------|
| `↓` / `↑` or `j` / `k` | Move (wraps around at either end) |
| `PageDown` / `PageUp`, `Home` / `End` | Move a page, or to the first or last row |
| `Space` | Mark or unmark a scenario; on a feature, mark all its scenarios (or clear them when all are marked) |
| `a` / `n` | Mark every row shown / clear all marks |
| `Enter` | Run the marked scenarios, or every scenario when none is marked |
| `t` | Edit the tags; `Enter` keeps them, `Esc` goes back to what they were |
| `/` or `f` | Filter the rows (case-insensitive, on file, feature and scenario names); `Enter` keeps the filter, `Esc` clears it |
| `l` | Read the files again; marks of scenarios that are still there are kept |
| `q` or `Ctrl-C` | Quit |

Tags are written as `behave` takes them on the command line: `@smoke,@web` runs
the scenarios that have either tag, and `-@slow` (or `~@slow`) leaves out those
with that tag. A feature's own tags count for its scenarios. The tags are read
from the feature file; a scenario they leave out is not run.

On the results, `↓`/`↑`, `PageDown`/`PageUp` and `Home`/`End` move, the block
below shows the messages of the scenario under the cursor, and `Esc` (or `q` or
`Enter`) goes back to the list.

## What the run does

The scenarios are bound to Rust tests with rstest-bdd, as the
[Gherkin tutorial](/tutorials/behave) shows. The app runs `cargo test` once for
the marked scenarios of a feature, in the folder it was started in, with a name
filter for each: the scenario's title in snake case (`valid login` becomes
`valid_login`; `scenarios!` names its tests that way). A test whose name
contains the filter belongs to the scenario, and an outline runs all of its
example rows.

Besides a pass, a scenario ends as one of these:

| Result | When |
|---|---|
| **UNDEF** (undefined) | No test has the scenario's name, or `cargo test` did not run (the message says why). Bind the scenario with `#[scenario]`, or name the test after it. |
| **FAIL** | A test failed. The message is the test's output. |
| **SKIP** | The tags left the scenario out, or its test is `#[ignore]`d. |

Steps written in Rust run, since the tests are your own, and so do the
[built-in steps](/tutorials/behave#built-in-steps) of the library: the earlier
runner could run only built-in steps.

## Differences from Python

- It runs in the terminal; Python opens a Tkinter window.
- Python builds a `behave` command line and starts `python -m behave` with it;
  this starts `cargo test`.
- Python has options for the browser, session reuse, demo mode, the dashboard,
  headless mode, screenshots and any other `behave` option. Here the only run
  option is tags; the `SB_*` variables of the environment reach the tests, so
  `SB_HEADLESS=true sbrs behave-gui` runs headless. To run scenarios with the
  flags of the Python plugin, use [`sbrs test`](/help/test_options), for example
  `sbrs test -k valid_login --headless`; `sbrs behave-options` maps behave's own
  `-D` options onto it.
