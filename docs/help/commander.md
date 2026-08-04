# SeleniumBase Commander

Commander is a terminal UI for browsing and running tests or examples. It is a
convenient way to explore the project without memorizing every example path.

## What you will learn

- How to launch Commander.
- How to navigate and filter the list.
- What kinds of files Commander discovers.

## Launch

```bash
cargo run --bin sbase -- commander
```

## Navigation

| Key | Action |
|-----|--------|
| `↑` / `↓` or `j` / `k` | Move selection |
| `Enter` | Run selected item |
| `/` or `f` | Filter list |
| `r` | Refresh list |
| `q` | Quit |

## What it runs

Commander discovers:

- Examples in `examples/`.
- Tests in `tests/`.
- Scenarios in `scenarios/`.

Select an item and press `Enter` to run it with the default `BrowserConfig`.

## Tips

- Use the filter key (`/`) to quickly find a test by name.
- Commander runs items with the default config; for custom flags, use the CLI
  directly.
- If a file is missing from the list, check that it is under one of the
  discovered directories and has the expected extension.
