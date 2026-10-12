# SeleniumBase Commander

Commander is a terminal UI for browsing and running tests or examples. It is a
convenient way to explore the project without memorizing every example path.

## What you will learn

- How to build and launch Commander.
- How to navigate and filter the list.
- What kinds of files Commander discovers, and what it runs for each.

## Build and launch

Commander is an opt-in feature. The default build leaves out the terminal UI
(`ratatui` and `crossterm`), so enable the `tui` feature:

```bash
cargo run --features tui --bin sbase -- commander
# or install the binary with it
cargo install seleniumbase-rs --features tui
```

An `sbase` built without the feature still has the subcommand, but it only
prints the following message and exits with status 2:

```text
this sbase was built without the terminal UI; reinstall it with `cargo install seleniumbase-rs --features tui`
```

Run it from the project root: Commander looks for `tests/` and `examples/` in
the current directory.

## Navigation

| Key | Action |
|-----|--------|
| `↓` / `↑` or `j` / `k` | Move selection (wraps around at either end) |
| `Enter` | Run selected item |
| `/` or `f` | Filter list |
| `r` | Refresh list (rescan `tests/` and `examples/`) |
| `q` | Quit |

In filter mode, type to narrow the list (a case-insensitive match on the file
name), `Backspace` deletes a character, `Enter` keeps the filter and returns to
the list, and `Esc` clears it. The result popup closes with `Esc`, `Enter` or
`q`.

## What it runs

Commander discovers:

- `*.rs` files directly inside `examples/`, shown as `[example]`.
- `*.rs` files directly inside `tests/`, shown as `[test]`.

It does not descend into subfolders, and it does not look for scenarios or any
other file type.

Select an item and press `Enter`. Commander runs `cargo test --test <name>` for
a test or `cargo run --example <name>` for an example, waits for it to finish
(the UI does not redraw meanwhile), and shows a popup: `PASS: <name>` with a
green border, or `FAIL: <name> (exit code: ...)` with a red one, followed by the
last 20 lines of the combined standard output and standard error.

## Tips

- Use the filter key (`/`) to quickly find a test by name.
- Commander passes no flags of its own to Cargo. The child process inherits
  Commander's environment, so set `SB_*` variables (for example `SB_HEADLESS`)
  before launching it, or use `sbase test` for per-run flags.
- A test or example that needs an opt-in Cargo feature (for instance the
  examples that require `gui`) cannot be built from Commander, because it
  cannot pass `--features`. Use `sbase test --example <name> --features gui`.
- If a file is missing from the list, check that it sits directly in `tests/` or
  `examples/` and ends in `.rs`, then press `r`.

## See also

The same feature builds two more terminal apps: the
[Case Plans Generator](./caseplans.md) (`sbase caseplans`) and the
[Behave Commander](./behave-gui.md) (`sbase behave-gui`).
