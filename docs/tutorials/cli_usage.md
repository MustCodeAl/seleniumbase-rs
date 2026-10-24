---
title: "CLI Usage (sbrs)"
sidebarTitle: "CLI usage"
---

The `sbrs` binary provides quick commands for common tasks: opening pages,
running smoke tests, executing CDP commands, patching binaries, generating
files, and more. This page covers the most common workflows.

<Info>

**What you will learn**

- How to build and invoke the CLI.
- How to run smoke tests and assertions from the command line.
- How to execute CDP commands and JSON scenarios.
- How to patch binaries and diagnose the environment.
- How to show the installed Chrome and its driver in your shell prompt.
- How to turn on the built-in browser plugins.

</Info>

<Visibility for="agents">

Summary for AI exports: `sbrs` is the command line of the `seleniumbase-rs-cli` package (`cargo install --path crates/cli`). Global options such as `--uc`, `--cdp`, `--headless`, `--browser`, `--proxy` and `--proxy-list` go before the subcommand. Commands on this page include `open`, `smoke`, `cdp`, `devtools`, `plugin`, `screenshot`, `save-source`, `assert-element`, `wait-for-text`, `patch-chromedriver`, `patch-chrome`, `doctor`, `install`, `prompt`, `test`, `encrypt`, `decrypt`, `report`, `run-scenario`, `mkfile`, `mkdir`, `import-python`, `import-playwright`, `import-puppeteer` and `completions`. Every command and flag is in the [CLI reference](/help/cli_reference).

</Visibility>

## Build the CLI

```bash
cargo build --bin sbrs
```

The binary is built by the workspace package `seleniumbase-rs-cli` (`crates/cli`);
the library `seleniumbase-rs` at the repository root carries no command-line code.
To install it: `cargo install --path crates/cli`.

A `justfile` and `Makefile` provide common shortcuts such as `just lint`,
`just test-all`, `just docs`, and `just completions`.

## Help and version

```bash
./target/debug/sbrs --help
./target/debug/sbrs --version
./target/debug/sbrs <COMMAND> --help
```

Every top-level option and subcommand argument includes a description, so
`--help` shows what each flag does. `--version` prints the crate name and
version.

## Open a page

```bash
./target/debug/sbrs open https://seleniumbase.io
```

## Run a smoke test with UC mode

```bash
./target/debug/sbrs --uc smoke https://seleniumbase.io --title-contains SeleniumBase
```

## Execute a raw CDP command

```bash
./target/debug/sbrs --cdp cdp --cmd Browser.getVersion
./target/debug/sbrs --cdp cdp --cmd Network.setCacheDisabled --params '{"cacheDisabled":true}'
```

## Share a browser with DevTools MCP servers

```bash
./target/debug/sbrs devtools                         # Chrome on 127.0.0.1:9222
./target/debug/sbrs devtools --headless --port 0 --json
```

Launches Chrome with its DevTools endpoint open, prints the endpoint and configs
for Google's `chrome-devtools-mcp` and Microsoft's Playwright MCP, and runs until
Ctrl+C, which closes the browser. See [DevTools MCP servers](/tutorials/devtools_mcp).

## Run built-in plugins

```bash
./target/debug/sbrs plugin list                       # the plugins of this build
./target/debug/sbrs plugin info shield                # what shield may do
./target/debug/sbrs --plugin shield open https://seleniumbase.io
./target/debug/sbrs --plugin shield --plugin-opt 'shield.level="strict"' open https://seleniumbase.io
```

`--plugin NAME` and `--plugin-opt NAME.KEY=VALUE` (a TOML value, so quote a
string) go before the command or after it, repeat, and add to the `[plugins]` table of the
settings file and `SB_PLUGINS`; the command line wins over the variable, which
wins over the file. They apply to every command that starts a browser and to
`sbrs devtools`. A mistake stops the command with exit status 2 before a browser
starts. See [Choosing plugins](/plugins/selecting).

## Save artifacts

```bash
./target/debug/sbrs screenshot
./target/debug/sbrs save-source
```

## Assertions and waits

```bash
./target/debug/sbrs open https://seleniumbase.io
./target/debug/sbrs assert-element --css "body"
./target/debug/sbrs wait-for-text --css "body" --text "SeleniumBase" --timeout 15
```

## Patch chromedriver

```bash
./target/debug/sbrs patch-chromedriver --path /path/to/chromedriver
```

## Patch Chrome binary

```bash
./target/debug/sbrs patch-chrome --path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
./target/debug/sbrs patch-chrome --path /usr/bin/google-chrome --cache-dir /tmp/sb-chrome-patches
```

## Diagnostic check

```bash
./target/debug/sbrs doctor
```

`doctor` prints the active `SB_*` environment variables, the detected Chrome
binary, and the patched-binary cache path. For chromedriver it looks, in order,
at the file named by `CHROMEDRIVER_PATH`, at `downloaded_drivers/` (where
`sbrs install` puts it), and on `PATH`; a `CHROMEDRIVER_PATH` that names no
file is reported rather than skipped. It also prints the driver's major version
next to Chrome's, because a chromedriver only drives the Chrome of its own major
version:

```text doctor output
Chromedriver
  Found: downloaded_drivers/chromedriver
  Version: 151, but Chrome is 155
  Hint: a chromedriver only drives its own Chrome version; run `sbrs install` to fetch the matching one
```

## Install a driver

```bash
./target/debug/sbrs install                    # chromedriver for the installed Chrome
./target/debug/sbrs install chromedriver 155   # the newest chromedriver of Chrome 155
./target/debug/sbrs get geckodriver            # the newest geckodriver release
./target/debug/sbrs get geckodriver 0.36.0     # one geckodriver release
./target/debug/sbrs get edgedriver             # msedgedriver for the installed Edge
./target/debug/sbrs get edgedriver latest      # the stable msedgedriver
```

`sbrs get` is another name for `sbrs install`, as in Python SeleniumBase. The
driver is `chromedriver` (the default), `geckodriver` (or `firefoxdriver`) or
`edgedriver` (or `msedgedriver`); the version is `latest`, a major version or a
full build, and for geckodriver a release such as `0.36.0`. The driver goes to
`./downloaded_drivers/` and its path is printed.

Without a version, chromedriver is the one that matches the installed Chrome,
from Chrome for Testing (the latest stable one when Chrome is not found or no
driver is listed for its version), msedgedriver the one for the installed
Edge's major version from `msedgedriver.microsoft.com`, and geckodriver the
newest GitHub release. It needs network access and fetches only the driver: the
[Pure CDP engine](/tutorials/pure_cdp) needs none. Starting a WebDriver session finds or
fetches the driver by itself, so run `install` ahead of time only to prepare an
offline machine or a container. Python's `--path` (copy the driver to a folder
on the `PATH`) is not supported.

## Show the browser in your shell prompt

```bash
sbrs prompt                                   # chrome 155 ✓driver
sbrs prompt --ascii                           # chrome 155 +driver
sbrs prompt --format '{browser}{major} {mark}'  # chrome155 ✓
sbrs prompt --verbose                         # and, on standard error, how each part was found
```

`sbrs prompt` prints one short line for a shell prompt: the major version of
the installed Chrome (or Chromium) and whether a chromedriver of that version
is already at hand. The chromedriver is looked for as `doctor` does: the file
`CHROMEDRIVER_PATH` names, `downloaded_drivers/` in the current folder, then the
`PATH`. `✓driver` means it matches, `✗driver` that it is missing or of another
major version (`sbrs install` fetches the right one), and `?driver` that its
version could not be read. `--ascii` writes `+`, `-` and `?` instead, for
terminals and fonts without check marks.

When no browser is found it prints nothing and exits with 0, so the prompt
simply leaves the segment out; `--verbose` says on standard error what it
looked at. The browser is the one `--browser-path` names, else `SB_CHROME_BIN`,
else the installed Chrome or Chromium (`CHROME_BIN`, `CHROMIUM_BIN`, the usual
install folders, the `PATH`). A path that is given but holds no file counts as
no browser; it is never replaced by another one.

A prompt runs it before every command line, so it starts no browser and uses
no network:

- The browser's version is read from files: the app's `Info.plist` on macOS,
  the version folder beside `chrome.exe` on Windows.
- A version that only running the program tells (`chromedriver --version`, and
  Chrome on Linux) is cached for ten minutes in
  `seleniumbase-rs/prompt-versions.json` in the user cache folder
  (`~/Library/Caches` on macOS, `$XDG_CACHE_HOME` or `~/.cache` on Linux,
  `%LOCALAPPDATA%` on Windows), together with the program's size and
  modification time, so a new download or a browser update shows at once.
  `--no-cache` asks the programs every time. A program that has not answered
  within two seconds is given up on: a chromedriver shows as `?driver`, a
  browser leaves the line out.

Measured with `hyperfine` on an Apple M1 (a debug build), a warm `sbrs prompt`
takes about 9 ms, as long as `sbrs version` takes to start; with `--no-cache`,
which runs `chromedriver --version`, about 23 ms. Asking Chrome itself with
`--version` would take about 45 ms.

| Placeholder | Becomes |
|---|---|
| `{browser}` | `chrome` or `chromium` |
| `{major}` | The browser's major version, such as `155` |
| `{driver}` | The mark and the word `driver`: `✓driver`, `✗driver` or `?driver` |
| `{mark}` | The mark alone |
| `{driver_major}` | The chromedriver's major version; empty when it is missing or unknown |

The default is `--format '{browser} {major} {driver}'`. `{{` and `}}` print a
brace. An unknown placeholder is refused with exit status 2, so a typo shows
when you set the prompt up, not as a segment that never appears.

<Tabs>

<Tab title="Bash" icon="terminal">

```bash ~/.bashrc
# ~/.bashrc
PS1='$(sbrs prompt --format "[{browser} {major} {driver}] " 2>/dev/null)'"$PS1"
```

The single quotes keep `$(...)` for later: Bash runs it each time it draws the
prompt (the `promptvars` option, on by default).

</Tab>

<Tab title="Zsh" icon="terminal">

```zsh ~/.zshrc
# ~/.zshrc
setopt PROMPT_SUBST
PROMPT='$(sbrs prompt --format "[{browser} {major} {driver}] " 2>/dev/null)'"$PROMPT"
```

For the right-hand side instead, set `RPROMPT='$(sbrs prompt 2>/dev/null)'`.
With `PROMPT_SUBST` the line goes through prompt expansion, so a `%` in your own
`--format` has to be written `%%`.

</Tab>

<Tab title="Fish" icon="fish">

```fish fish_right_prompt.fish
# ~/.config/fish/functions/fish_right_prompt.fish
function fish_right_prompt
    type -q sbrs; and sbrs prompt
end
```

Fish draws the right prompt beside your own `fish_prompt` and runs the function
every time; this file replaces a right prompt you already have.

</Tab>

<Tab title="PowerShell" icon="windows">

```powershell PowerShell profile
# In your profile: notepad $PROFILE
$global:SbrsPromptBefore = $function:prompt
function prompt {
    $segment = if (Get-Command sbrs -ErrorAction SilentlyContinue) { sbrs prompt --ascii }
    $(if ($segment) { "[$segment] " }) + (& $global:SbrsPromptBefore)
}
```

PowerShell decodes a program's output with the console's code page, which in
Windows PowerShell 5.1 turns `✓` into other characters; hence `--ascii`. In
PowerShell 7 with `[Console]::OutputEncoding = [Text.Encoding]::UTF8` in the
profile, the marks show and `--ascii` can go.

</Tab>

<Tab title="Starship" icon="rocket">

```toml starship.toml
# ~/.config/starship.toml
[custom.sbrs]
description = "The installed Chrome and whether a matching chromedriver is at hand"
command = "sbrs prompt"
when = true
format = "([$output]($style) )"
style = "bold blue"
```

A custom module shows only when `when` holds (or one of its `detect_*` lists
matches), so `when = true` shows it everywhere; `detect_folders =
["downloaded_drivers"]` in its place shows it only in projects that have
downloaded a driver. The parentheses make a conditional group, which leaves
out the space too when `sbrs prompt` prints nothing. The module comes where
`$all` is in your `format` (the default), or where you put `${custom.sbrs}`.
Starship gives a command 500 ms (`command_timeout`), far more than it needs.

</Tab>

<Tab title="Warp" icon="bolt">

Warp draws its own prompt by default (the Warp prompt, made of its context
chips), which does not show your shell's `PS1`. To see the segment, open
**Settings > Appearance** and, under **Input**, choose the **Shell prompt (PS1)**
as the prompt type (the menu labels it **Shell (PS1)**). Warp then draws the
prompt your shell configuration builds: the Bash or Zsh `PS1` above, or the
Starship module, which Warp's documentation lists as working in Bash, Zsh, Fish
and PowerShell. Warp shows a right-hand prompt (`RPROMPT`, `fish_right_prompt`)
in Zsh and Fish but not in Bash.

The repository's Warp Workflow `.warp/workflows/sbrs-prompt.yaml` runs
`sbrs prompt --verbose`, to see what the segment says and why.

</Tab>

</Tabs>

## Run your tests

`sbrs test` runs `cargo test` (or one example) and carries the global options
into the tests as `SB_*` environment variables, the way Python SeleniumBase's
pytest options reach its tests:

```bash
sbrs test                                  # every test
sbrs test login                            # tests whose name contains "login"
sbrs test --test browser_smoke             # one file from tests/
sbrs test --example basic_test             # run one example instead
sbrs --headless --browser firefox -n 4 test -- --nocapture
```

Only options you actually pass are exported (`SB_HEADLESS`, `SB_BROWSER`,
`SB_MODE`, `SB_PROXY`, ...), so the config file and your own `SB_*` variables
still apply to everything else. A test sees them when it loads its settings
with `Settings::load()`. `-n` becomes `--test-threads`, and the exit status
is Cargo's. `--dashboard`, `--html`, `--junit-xml` and `--alluredir` write the
reports of the run; see [Reports of a run](/tutorials/writing_tests#reports-of-a-run).

## Proxies from a list

`--proxy-list FILE` reads a list of proxies, one per line, optionally named
(the Rust counterpart of Python's `PROXY_LIST`):

<CodeGroup>

```text proxies.txt
office = 10.0.0.5:3128
eu = alice:s3cret@proxy.example.com:8080
socks5://10.0.0.9:1080
```

```bash Usage
sbrs --proxy-list proxies.txt --proxy eu open https://example.com   # by name
sbrs --proxy-list proxies.txt open https://example.com              # a random one
```

</CodeGroup>

Each line is checked when the file is read, and an error names the line number
(never the password). The same type, `ProxyList`, hands proxies out in turn or
at random to Rust code; see `seleniumbase_rs::config::proxy_list`.

## Encrypt and decrypt secrets

Keep a password out of a test file by storing it encrypted.

<Tip>

The passphrase comes from the environment, never from an argument, so it stays
out of shell history and process listings.

</Tip>

```bash
export SB_ENCRYPTION_KEY='a long passphrase'
sbrs encrypt 'my password'          # prints a token such as sbenc1:600000:...
echo 'my password' | sbrs encrypt   # or read the text from standard input
sbrs decrypt 'sbenc1:600000:...'    # prints: my password
```

<Info>

Tokens use AES-256-GCM with a key derived from the passphrase (PBKDF2-HMAC-SHA256,
600,000 iterations, a fresh random salt each time). A wrong passphrase or a
modified token fails to decrypt. They are not interchangeable with Python
SeleniumBase's `sbrs encrypt`, which is a reversible obfuscation with a fixed
key.

</Info>

## Read a results database

With the `turso` feature (`cargo build --features turso`), `sbrs report history` reads
the database a `ResultStore` writes. See
[Results Database and Encrypted Profiles](/tutorials/results_and_profiles).

```bash
sbrs report history --db reports/results.db            # the latest runs
sbrs report history --db reports/results.db --run 12   # one run's results
sbrs report history --db reports/results.db --run 12 --failed
sbrs report history --db reports/results.db --flaky    # pass-and-fail across recent runs
sbrs report history --json                             # machine-readable; any of the above
```

The path can also come from `SB_REPORT_DB`. A path that does not exist is an
error; `report` never creates a database.

## Run a JSON scenario

```bash
./target/debug/sbrs run-scenario --file ./scenario.json
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
sbrs mkfile tests/login.rs                       # one browser test (alias: new)
sbrs mkfile tests/login.rs --url https://my.site # ... that opens your page
sbrs mkdir tests/ui                              # a suite: main.rs, helpers.rs, examples, README
sbrs mkdir tests/ui --basic                      # the scaffolding only, no example tests
```

The generated Rust uses `run_browser_test` (see [Writing Browser Tests](/rust-test-tooling)),
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

## Terminal apps

<Badge color="gray">Feature: tui</Badge>

Three commands are full-screen terminal apps. They are part of the opt-in `tui`
feature:

<CodeGroup>

```bash From a clone
cargo install --path crates/cli --features tui
```

```bash From git
cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --features tui
```

</CodeGroup>

<Note>

An `sbrs` built without the `tui` feature prints a hint and exits with status 2.

</Note>

```bash
sbrs commander                  # browse and run the tests and examples of the project
sbrs caseplans                  # write, edit and summarise a case plan for each test
sbrs behave-gui features/       # pick Gherkin scenarios, run them, see the results
```

`caseplans` (alias `case-plans`) and `behave-gui` (alias `gui-behave`) are the
Python Case Plans Generator and Behave Commander, which open Tkinter windows
there. Each takes an optional folder or file; see [Case Plans
Generator](/help/caseplans) and [Behave Commander](/help/behave-gui).
`sbrs caseplans --starter` and `sbrs behave-gui --new` are the earlier
commands that only write a file (a checklist, a new feature); they need no
feature.

## Import Python tests

Convert a SeleniumBase or Selenium WebDriver Python file:

```bash
./target/debug/sbrs import-python ./tests/login_test.py \
  --output ./tests/login_test.rs
```

Use `--source selenium-base` or `--source selenium` to override automatic
detection. Use `--test-name user_can_log_in` to choose the generated function
name. Unsupported statements are retained as `TODO` comments and diagnostics.

<Warning>

Review generated code before compiling or running it.

</Warning>

## Import Playwright and Puppeteer tests

Convert Playwright tests or scripts (JavaScript, TypeScript or Python) and
Puppeteer scripts or tests (JavaScript or TypeScript, or a Chrome DevTools
Recorder `.json` export) into Rust tests for the pure CDP engine:

```bash
./target/debug/sbrs import-playwright ./e2e/login.spec.ts --output ./tests/login.rs
./target/debug/sbrs import-playwright ./tests/test_login.py   # pytest-playwright
./target/debug/sbrs import-puppeteer ./scripts/checkout.js --output ./tests/checkout.rs
./target/debug/sbrs import-puppeteer ./recordings/checkout.json  # Recorder export
```

`import-pw` and `import-pptr` are short names for the two commands. The
language comes from the file extension (`.js`, `.mjs`, `.cjs`, `.ts`, `.mts`,
`.py`, `.json`), else from the content; `--language js|ts|python` (Puppeteer:
`js|ts|json`) overrides it. Each `test()`/`it()` becomes one `#[tokio::test]`
named after its `describe` and test titles; a script without titles (codegen
output, a plain script) becomes one test named after the file, or after
`--test-name`. Without `--output` the Rust goes to standard output; one line
per statement that became a `TODO(playwright)` or `TODO(puppeteer)` comment
goes to standard error. As with `import-python`, the exit status tells what
happened:

| Case | Exit status |
|---|---|
| A gap, a statement that became a `TODO` comment: a warning | 0 |
| A statement the importer could not read: an error | 1 |
| A file it cannot read | 6 |
| A source it refuses (over 2 MB, or a Recorder export given to `import-playwright`) | 2 |

Run the generated tests headless with `SB_HEADLESS=true cargo test`. See
[Migrate Playwright Tests](/playwright-migration) and
[Migrate Puppeteer Tests](/puppeteer-migration) for every mapping.

## Install shell completions

The supported values are `bash`, `elvish`, `fish`, `powershell`, `zsh`, `fig` (a Fig spec) and `usage` (a usage spec), see below.

The shell scripts are short: each time you press the completion key they ask
`sbrs` itself for the candidates, so `sbrs` has to be on `PATH`, and the
candidates are exactly the commands, aliases, flags and values the command line
accepts. Every command and flag is listed in the
[CLI reference](/help/cli_reference).

<CodeGroup>

```bash Bash
./target/debug/sbrs completions bash \
  > "${BASH_COMPLETION_USER_DIR:-$HOME/.local/share/bash-completion/completions}/sbrs"
```

```bash Zsh
mkdir -p "$HOME/.zfunc"
./target/debug/sbrs completions zsh > "$HOME/.zfunc/_sbrs"
```

```bash Fish
./target/debug/sbrs completions fish \
  > "$HOME/.config/fish/completions/sbrs.fish"
```

</CodeGroup>

<Note>

Ensure the destination directory exists. For Zsh, add `$HOME/.zfunc` to
`fpath` before running `compinit`.

</Note>

### usage and mise

`sbrs completions usage` prints a [usage](https://usage.jdx.dev) spec (KDL),
the same file as `completions/sbrs.usage.kdl` in the repository. The `usage` CLI
turns it into completions for more shells, Markdown and man pages:

```bash
./target/debug/sbrs completions usage > sbrs.usage.kdl
usage generate completion zsh sbrs --file sbrs.usage.kdl
```

### Warp, inshellisense and Amazon Q

**Warp** uses your shell's own completions for commands it has no built-in spec
for (the "native shell completions" setting, on by default), so once the Bash,
Zsh or Fish completion above is installed, `sbrs` completes in Warp too.

Warp's built-in specs, [inshellisense](https://github.com/microsoft/inshellisense)
and Amazon Q read Fig completion specs
([withfig/autocomplete](https://github.com/withfig/autocomplete)).
`sbrs completions fig` writes one:

```bash
./target/debug/sbrs completions fig > sbrs.ts
```

The spec is not in withfig/autocomplete yet, so these tools do not know `sbrs`
until it is. How the repository's copy is made and how to submit it is in
[Submit the Fig spec](/completions/README).

**Warp Workflows:** the repository has ready-made `sbrs` commands in
`.warp/workflows/` (smoke test, install a driver, doctor, explain the prompt
segment, record, import Playwright or Puppeteer tests, create a test folder). Warp shows them in the
command palette when you are inside the repository; copy them to
`~/.warp/workflows/` to have them everywhere.

## Common global flags

<ParamField body="--uc">

Enable UC (undetected) mode.

</ParamField>

<ParamField body="--cdp">

Enable CDP mode.

</ParamField>

<ParamField body="--headless">

Run browser headlessly.

</ParamField>

<ParamField body="--browser" type="NAME">

Select browser (`chrome`, `chromium`, `edge`, `firefox`). Without it the config file's browser, else Chrome.

</ParamField>

<ParamField body="--proxy" type="PROXY">

Route traffic through a proxy (`host:port`, or a name from `--proxy-list`).

</ParamField>

<ParamField body="--proxy-list" type="FILE">

Choose the proxy from a list of proxies.

</ParamField>

<ParamField body="-n" type="N">

Number of parallel tests for `sbrs test`.

</ParamField>

## Troubleshooting

<AccordionGroup>

<Accordion title="sbrs: command not found" icon="terminal">

**Likely cause.** Binary not on `PATH`.

**Fix.** Use `./target/debug/sbrs` or install with `cargo install --path crates/cli`.

</Accordion>

<Accordion title="Global flag rejected" icon="flag">

**Likely cause.** Flag placed after the subcommand.

**Fix.** Put global flags before the subcommand: `sbrs --headless open URL`.

</Accordion>

<Accordion title="CDP command fails" icon="bolt">

**Likely cause.** Not in CDP mode.

**Fix.** Add `--cdp` or use a CDP-enabled config.

</Accordion>

<Accordion title="doctor shows missing Chrome" icon="stethoscope">

**Likely cause.** Chrome not installed or not on PATH.

**Fix.** Set `SB_CHROME_BIN` to the full path.

</Accordion>

<Accordion title="The sbrs prompt segment does not appear" icon="eye-slash">

**Likely cause.** No browser found, or `SB_CHROME_BIN` names no file.

**Fix.** Run `sbrs prompt --verbose`.

</Accordion>

</AccordionGroup>
