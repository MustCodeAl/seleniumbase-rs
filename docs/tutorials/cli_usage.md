# CLI Usage (`sbase`)

The `sbase` binary provides quick commands for common tasks.

## Build the CLI

```bash
cd rust-port
cargo build --bin sbase
```

## Help

```bash
./target/debug/sbase --help
```

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
