# Behave Commander

`sbase behave-gui` (also `sbase gui-behave`) is a terminal app that lists the
Gherkin feature files of a project, runs the scenarios you mark through this
crate's behave runner and shows what came of each. It is the Python Behave
Commander without the Tkinter window.

## What you will learn

- How to build and launch the app.
- What it lists, and how to pick scenarios and tags.
- What the run can and cannot do.

## Build and launch

The app is part of the opt-in `tui` feature, with `sbase commander`:

```bash
cargo run --features tui --bin sbase -- behave-gui
# or install the binary with it
cargo install seleniumbase-rs --features tui
```

An `sbase` built without the feature still has the subcommand, but it only
prints the following message and exits with status 2, as `commander` does:

```text
this sbase was built without the terminal UI; reinstall it with `cargo install seleniumbase-rs --features tui`
```

The folder to look in is the optional argument:

```bash
sbase behave-gui                         # the features/ folder
sbase behave-gui features/shop           # one folder, and its subfolders
sbase behave-gui features/login.feature  # one file
```

`sbase behave-gui --new` skips the app and asks for a feature name, a title and
scenarios, then writes `features/<name>.feature`. It never replaces a file that
is there, and it works in every build.

## What is listed

Every `*.feature` file under the folder (subfolders included, sorted by path;
folders whose name starts with a dot, and `target` and `node_modules`, are not
entered), with its scenarios below it:

```text
  [-] features/login.feature => Log in (3 scenarios)
      [x] line 10: valid login
      [ ] line 14: wrong value
      [ ] line 26: undefined step
```

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
with that tag. A feature's own tags count for its scenarios.

On the results, `↓`/`↑`, `PageDown`/`PageUp` and `Home`/`End` move, the block
below shows the messages of the scenario under the cursor, and `Esc` (or `q` or
`Enter`) goes back to the list.

## What the run can do

Each scenario is run with the crate's behave runner, `run_feature_text`, with
the feature's background first. The runner needs step definitions, and those
are Rust code in your own program, which `sbase` cannot load. The app has only
the built-in steps:

- `I do nothing` and `the system is ready`, which pass,
- `I wait N seconds`, which does not wait,
- `I set the value to "x"`, `the value should be "x"` and `the value should not
  be "x"`, which keep and compare one value (each scenario starts with an empty
  value).

A scenario that uses any other step ends as **UNDEF** (undefined), as in
`behave`, with the step it could not find. A step that failed is **FAIL**, and a
scenario that the tags left out, or an outline with no example rows, is
**SKIP**.

To run features with your own steps, register them in a
`seleniumbase_rs::behave::StepRegistry` in a test of your own, as the
[behave tutorial](../tutorials/behave.md) shows.

## Differences from Python

- It runs in the terminal; Python opens a Tkinter window.
- Python builds a `behave` command line and starts `python -m behave` with it;
  this runs the scenarios in the process, with the built-in steps.
- Python has options for the browser, session reuse, demo mode, the dashboard,
  headless mode, screenshots and any other `behave` option. Here the only run
  option is tags: the Rust runner has no such settings to turn.
