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
cargo build --bin sbase
```

A `justfile` and `Makefile` provide common shortcuts such as `just lint`,
`just test-all`, `just docs`, and `just completions`.

## Help and version

```bash
./target/debug/sbase --help
./target/debug/sbase --version
./target/debug/sbase <COMMAND> --help
```

Every top-level option and subcommand argument includes a description, so
`--help` shows what each flag does. `--version` prints the crate name and
version.

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
binary, and the patched-binary cache path. For chromedriver it looks, in order,
at the file named by `CHROMEDRIVER_PATH`, at `downloaded_drivers/` (where
`sbase install` puts it), and on `PATH`; a `CHROMEDRIVER_PATH` that names no
file is reported rather than skipped.

## Run your tests

`sbase test` runs `cargo test` (or one example) and carries the global options
into the tests as `SB_*` environment variables, the way Python SeleniumBase's
pytest options reach its tests:

```bash
sbase test                                  # every test
sbase test login                            # tests whose name contains "login"
sbase test --test browser_smoke             # one file from tests/
sbase test --example basic_test             # run one example instead
sbase --headless --browser firefox -n 4 test -- --nocapture
```

Only options you actually pass are exported (`SB_HEADLESS`, `SB_BROWSER`,
`SB_MODE`, `SB_PROXY`, ...), so the config file and your own `SB_*` variables
still apply to everything else. A test sees them when it loads its settings
with `Settings::load(None)`. `-n` becomes `--test-threads`, and the exit status
is Cargo's.

## Proxies from a list

`--proxy-list FILE` reads a list of proxies, one per line, optionally named
(the Rust counterpart of Python's `PROXY_LIST`):

```text
# proxies.txt
office = 10.0.0.5:3128
eu = alice:s3cret@proxy.example.com:8080
socks5://10.0.0.9:1080
```

```bash
sbase --proxy-list proxies.txt --proxy eu open https://example.com   # by name
sbase --proxy-list proxies.txt open https://example.com              # a random one
```

Each line is checked when the file is read, and an error names the line number
(never the password). The same type, `ProxyList`, hands proxies out in turn or
at random to Rust code; see `seleniumbase_rs::config::proxy_list`.

## Encrypt and decrypt secrets

Keep a password out of a test file by storing it encrypted. The passphrase comes
from the environment, never from an argument, so it stays out of shell history
and process listings:

```bash
export SB_ENCRYPTION_KEY='a long passphrase'
sbase encrypt 'my password'          # prints a token such as sbenc1:600000:...
echo 'my password' | sbase encrypt   # or read the text from standard input
sbase decrypt 'sbenc1:600000:...'    # prints: my password
```

Tokens use AES-256-GCM with a key derived from the passphrase (PBKDF2-HMAC-SHA256,
600,000 iterations, a fresh random salt each time). A wrong passphrase or a
modified token fails to decrypt. They are not interchangeable with Python
SeleniumBase's `sbase encrypt`, which is a reversible obfuscation with a fixed
key.

## Read a results database

With the `turso` feature (`cargo build --features turso`), `sbase report` reads
the database a `ResultStore` writes. See
[Results Database and Encrypted Profiles](results_and_profiles.md).

```bash
sbase report --db reports/results.db            # the latest runs
sbase report --db reports/results.db --run 12   # one run's results
sbase report --db reports/results.db --run 12 --failed
sbase report --db reports/results.db --flaky    # pass-and-fail across recent runs
sbase report --json                             # machine-readable; any of the above
```

The path can also come from `SB_REPORT_DB`. A path that does not exist is an
error; `report` never creates a database.

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
sbase mkfile tests/login.rs                       # one browser test (alias: new)
sbase mkfile tests/login.rs --url https://my.site # ... that opens your page
sbase mkdir tests/ui                              # a suite: main.rs, helpers.rs, examples, README
sbase mkdir tests/ui --basic                      # the scaffolding only, no example tests
```

The generated Rust uses `run_browser_test` (see [Writing Browser Tests](../rust-test-tooling.md)),
and a test in `tests/` runs with `cargo test --test login`. A folder sitting
directly inside `tests/` is one test target; `tests/ui/README.md` says how to
register a folder anywhere else.

The generators are careful with what they write:

- A name is relative to the current directory and may contain letters, digits,
  `_`, `-` and `.`, with `/` between folders. `..`, absolute paths, backslashes
  and other characters are rejected before anything is created, and a link that
  leads out of the directory is not followed.
- An existing file is never overwritten. `mkdir` checks every file first and
  creates none of them if one exists. Pass `--force` to replace regular files.
- They need no browser and ignore `sbase_config.toml`, so they work in an empty
  folder.

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
| `--browser NAME` | Select browser (`chrome`, `chromium`, `edge`, `firefox`). Without it the config file's browser, else Chrome. |
| `--proxy PROXY` | Route traffic through a proxy (`host:port`, or a name from `--proxy-list`). |
| `--proxy-list FILE` | Choose the proxy from a list of proxies. |
| `-n N` | Number of parallel tests for `sbase test`. |

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `sbase: command not found` | Binary not on `PATH` | Use `./target/debug/sbase` or install with `cargo install --path .`. |
| Global flag rejected | Flag placed after the subcommand | Put global flags before the subcommand: `sbase --headless open URL`. |
| CDP command fails | Not in CDP mode | Add `--cdp` or use a CDP-enabled config. |
| `doctor` shows missing Chrome | Chrome not installed or not on PATH | Set `SB_CHROME_BIN` to the full path. |
