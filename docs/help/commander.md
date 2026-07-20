# SeleniumBase Commander

Commander is a terminal UI for browsing and running tests or examples.

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
