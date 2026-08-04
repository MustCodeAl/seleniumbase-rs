# CLI Usage (`sbase`)

The `sbase` binary provides quick commands for common tasks: opening pages,
running smoke tests, executing CDP commands, patching binaries, generating
files, and more. This page covers the most common workflows.

## What you will learn

- How to build and invoke the CLI.
- How to run smoke tests and assertions from the command line.
- How to execute CDP commands and JSON scenarios.
- How to patch binaries and diagnose the environment.

## Build the CLI

```bash
cd rust-port
cargo build --bin sbase
```

## Help

```bash
./target/debug/sbase --help
./target/debug/sbase <COMMAND> --help
```

Every top-level option and subcommand argument includes a description, so
`--help` shows what each flag does.

## Open a page

```bash
./target/debug/sbase open https://seleniumbase.io
```

## Run a smoke test with UC mode

```bash
./target/debug/sbase --uc smoke https://seleniumbase.io --title-contains SeleniumBase
```

## Execute a raw CDP command

```bash
./target/debug/sbase --cdp cdp --cmd Browser.getVersion
./target/debug/sbase --cdp cdp --cmd Network.setCacheDisabled --params '{"cacheDisabled":true}'
```

## Save artifacts

```bash
./target/debug/sbase screenshot
./target/debug/sbase save-source
```

## Assertions and waits

```bash
./target/debug/sbase open https://seleniumbase.io
./target/debug/sbase assert-element --css "body"
./target/debug/sbase wait-for-text --css "body" --text "SeleniumBase" --timeout 15
```

## Patch chromedriver

```bash
./target/debug/sbase patch-chromedriver --path /path/to/chromedriver
```

## Patch Chrome binary

```bash
./target/debug/sbase patch-chrome --path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
./target/debug/sbase patch-chrome --path /usr/bin/google-chrome --cache-dir /tmp/sb-chrome-patches
```

## Diagnostic check

```bash
./target/debug/sbase doctor
```

`doctor` prints the active `SB_*` environment variables, the detected Chrome
binary, and the patched-binary cache path.

## Run a JSON scenario

```bash
./target/debug/sbase run-scenario --file ./scenario.json
```

Example `scenario.json`:

```json
{
  "name": "basic_flow",
  "steps": [
    {"action": "open", "url": "https://seleniumbase.io"},
    {"action": "assert_element", "css": "body"},
    {"action": "wait_for_text", "css": "body", "text": "SeleniumBase", "timeout": 15}
  ]
}
```

## Generate files

```bash
./target/debug/sbase mkfile MyTest.rs
./target/debug/sbase mkdir my_tests
./target/debug/sbase mkpres MyPresentation
./target/debug/sbase mkchart MyChart
```

## Import Python tests

Convert a SeleniumBase or Selenium WebDriver Python file:

```bash
./target/debug/sbase import-python ./tests/login_test.py \
  --output ./tests/login_test.rs
```

Use `--source selenium-base` or `--source selenium` to override automatic
detection. Use `--test-name user_can_log_in` to choose the generated function
name. Unsupported statements are retained as `TODO` comments and diagnostics.
Review generated code before compiling or running it.

## Install shell completions

The supported values are `bash`, `elvish`, `fish`, `powershell`, and `zsh`.

```bash
# Bash
./target/debug/sbase completions bash \
  > "${BASH_COMPLETION_USER_DIR:-$HOME/.local/share/bash-completion/completions}/sbase"

# Zsh
mkdir -p "$HOME/.zfunc"
./target/debug/sbase completions zsh > "$HOME/.zfunc/_sbase"

# Fish
./target/debug/sbase completions fish \
  > "$HOME/.config/fish/completions/sbase.fish"
```

Ensure the destination directory exists. For Zsh, add `$HOME/.zfunc` to
`fpath` before running `compinit`.

## Common global flags

| Flag | Description |
|---|---|
| `--uc` | Enable UC (undetected) mode. |
| `--cdp` | Enable CDP mode. |
| `--headless` | Run browser headlessly. |
| `--browser NAME` | Select browser (`chrome`, `chromium`, `edge`, `firefox`). |
| `--proxy URL` | Route traffic through a proxy. |
| `--timeout SECS` | Set default timeout. |
| `--verbose` | Increase log output. |

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `sbase: command not found` | Binary not on `PATH` | Use `./target/debug/sbase` or install with `cargo install --path .`. |
| Subcommand flag ignored | Flag placed before subcommand | Put flags after the subcommand: `sbase open --headless`. |
| CDP command fails | Not in CDP mode | Add `--cdp` or use a CDP-enabled config. |
| `doctor` shows missing Chrome | Chrome not installed or not on PATH | Set `SB_CHROME_BIN` to the full path. |
