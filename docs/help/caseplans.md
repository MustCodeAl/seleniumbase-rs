---
title: "Case Plans Generator"
---

`sbrs caseplans` (also `sbrs case-plans`) is a terminal app for writing a
case plan for each of your tests: a short markdown table of the steps the test
performs and the result each one expects. It is the Python Case Plans Generator
without the Tkinter window.

## What you will learn

- How to build and launch the app.
- Which tests it lists and where it keeps their plans.
- How to create, edit and save a plan, and how to write the summary of all
  plans.

## Build and launch

The app is part of the opt-in `tui` feature, with `sbrs commander`:

<CodeGroup>

```bash Run from source
cargo run --features tui --bin sbrs -- caseplans
```

```bash Install the binary with it
cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --features tui
```

</CodeGroup>

<Note>

An `sbrs` built without the feature still has the subcommand, but it only
prints the following message and exits with status 2, as `commander` does.

</Note>

```text
this sbrs was built without the terminal UI; reinstall it with `cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --features tui`
```

Run it from the project root. The folder to look in is the optional argument:

```bash
sbrs caseplans                 # the tests/ and examples/ folders
sbrs caseplans tests/ui        # the *.rs files of one folder
sbrs caseplans tests/login.rs  # one test file
```

A folder that has a `tests` or an `examples` folder is searched there, as
`commander` does; any other folder is searched itself. Only the `*.rs` files
directly inside a folder count, and they are listed by name.

`sbrs caseplans --starter` skips the app and writes a checklist, `CASE_PLANS.md`,
in the current folder. It never replaces a file that is there, and it works in
every build.

## What is listed

Every test function, one row each: `tests/login.rs::valid_login` is the test
`valid_login` of `tests/login.rs`. A function counts when a `test` attribute
(`#[test]`, `#[tokio::test]`, `#[rstest]`) comes before it. This is a read of
the text, not a parse, so a `#[test]` inside a string looks like a test. A file
with no test function is listed as the file alone.

Each row shows `[x]` when you have marked it and `[plan]` when its plan exists.

## Where plans go

The plan of a test is `case_plans/<name>.md` in the folder of its file, where
the name is the last part of the row with `::` turned into `.`:

| Row | Plan |
|-----|------|
| `tests/login.rs::valid_login` | `tests/case_plans/login.valid_login.md` |
| `tests/ui/cart.rs::empty` | `tests/ui/case_plans/cart.empty.md` |
| `tests/about.rs` | `tests/case_plans/about.rs.md` |

<Info>

Cargo ignores a `case_plans` folder inside `tests/`, so the plans do not become
test targets.

</Info>

A new plan is the Python template:

````markdown
``login.rs::valid_login``
---
| # | Step Description | Expected Result |
| - | ---------------- | --------------- |
| 1 | Perform Action 1 | Verify Action 1 |
| 2 | Perform Action 2 | Verify Action 2 |
````

## Keys

On the list:

| Key | Action |
|-----|--------|
| `↓` / `↑` or `j` / `k` | Move (wraps around at either end) |
| `PageDown` / `PageUp`, `Home` / `End` | Move a page, or to the first or last row |
| `Space` | Mark or unmark the row |
| `a` / `n` | Mark every row shown / clear all marks |
| `c` | Create a boilerplate plan for each marked test that has none (for the row under the cursor when nothing is marked). A plan that exists is never replaced. |
| `Enter` or `e` | Open the plan of the row in the editor |
| `s` | Write the summary to `case_summary.md` |
| `/` or `f` | Filter the rows (case-insensitive); `Enter` keeps the filter, `Esc` clears it |
| `r` | List the tests again; marks are kept |
| `q` or `Ctrl-C` | Quit |

In the editor:

| Key | Action |
|-----|--------|
| Characters, `Enter`, `Backspace`, `Delete` | Edit |
| Arrow keys, `Home`, `End`, `PageUp`, `PageDown` | Move the cursor |
| `Ctrl-S` | Save the plan |
| `Esc` | Back to the list |
| `Ctrl-C` | Quit |

When the plan has changes that are not saved, `Esc` and `Ctrl-C` ask first: `s`
saves and then closes (or quits), `d` throws the changes away, and `Esc` goes on
editing. A plan is saved with LF line ends and a closing newline.

## The summary

`s` writes `case_summary.md` in the current folder, for the listed tests that
have a plan, and says how many of each kind there are:

- a plan whose table was filled in,
- a plan that is still the template,
- a plan with no table yet.

The file counts them in a table and folds each plan into a `<details>` block, as
the Python summary does, so it reads well on a code host. It is replaced each
time.

## Differences from Python

- It runs in the terminal; Python opens a Tkinter window.
- Python lists the tests pytest collects (`test_file.py::Class::test_method`);
  this lists Rust test functions found in the files.
- Python only creates boilerplate plans. Here you can also open a plan, edit it
  and save it without leaving the app.
- Python writes plans and the summary with CRLF line ends; this writes LF.
