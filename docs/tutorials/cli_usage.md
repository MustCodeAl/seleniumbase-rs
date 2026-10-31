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
- How to read the page that a wrong command prints.
- How to install drivers, run a local proxy and download the Selenium Server.
- How to move the selectors of a test into page objects.

</Info>

<Visibility for="agents">

Summary for AI exports: `sbrs` is the command line of the `seleniumbase-rs-cli` package (`cargo install --path crates/cli`). The options of `sbrs` itself (`--uc`, `--cdp`, `--headless`, `--browser`, `--proxy`, `--proxy-list`, `--url`, ...) are global: write them before or after the subcommand, but each one once. A command that acts on a page takes it from `--url URL` (`SB_START_PAGE` or the `start_page` key of the settings file also work) and stops with status 2, before a browser starts, when it has none; `open`, `smoke`, `download-file`, `print-pdf`, `pdf-text` and `assert-pdf-text` take a URL of their own and refuse `--url`. The generators take the file as an argument: `mkfile FILE`, `mkpres FILE --es`, `mkchart FILE`, `mkrec FILE --url URL`, `print FILE -n`, `translate FILE --es` (the language is a flag), `convert FILE.py`. The page-object commands are `extract-objects`, `inject-objects`, `revert-objects` and `objectify`. `install DRIVER [VERSION]` (alias `get`) needs the driver name, and `-p` copies the driver to `SB_DRIVER_PATH_DIR`, else `/usr/local/bin`. `proxy` runs a local forward proxy and `download server` fetches the Selenium Server jar. A wrong command line prints the page of the command it reached and then `error: ...`, with status 2. Also on this page: `cdp`, `devtools`, `plugin`, `screenshot`, `save-source`, `assert-element`, `wait-for-text`, `patch-chromedriver`, `patch-chrome`, `doctor`, `prompt`, `test`, `encrypt`, `decrypt`, `report`, `run-scenario`, `mkdir`, `methods`, `options`, `behave-options`, `import-python`, `import-playwright`, `import-puppeteer` and `completions`. Every command and flag is in the [CLI reference](/help/cli_reference).

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
version. `sbrs <COMMAND> --help` (or `sbrs help <COMMAND>`) shows the arguments
and flags of one command, then the global flags, and ends with an `Examples:`
section whose lines you can copy.

## When a command is wrong

A command line that `sbrs` cannot use does not get a lone `Usage:` line. It gets
the page of the deepest command the words reached (what it does, `Usage:`, its
arguments and flags, `Examples:`), a blank line, and last the error, so the
error is the line above your prompt. The exit status is 2 and no browser has
started. A name that is not a command gets the list of commands by group
instead (about 30 lines at a width of 80), so there is no 100-line page to scroll
back through.

<CodeGroup>

```text Unknown command
$ sbrs nosuch
Usage: sbrs [FLAGS] <COMMAND>
Page commands: accept-alert  add-text  assert-attribute  assert-element
  assert-link-text  assert-pdf-text  assert-text-not-visible
  assert-text-visible  assert-title  cache-clear  cdp  cdp-click-element
  cdp-type-text  check-if-unchecked  choose-file  clear-cookies
  clear-local-storage  click-partial-link-text  click-visible-elements  deferred
  dismiss-alert  download-file  drag-and-drop  get-alert-text  get-attribute
  get-cookies  get-current-url  get-local-storage-item  get-property  get-text
  get-title  get-value  get-window-position  highlight-click  hover
  hover-and-click  human-click  human-type  is-checked  is-element-visible
  is-link-text-visible  is-partial-link-text-visible  is-text-visible
  load-cookies  open  pdf-text  print-pdf  remove-local-storage-item
  save-cookies  save-source  screenshot  select-option  send-keys
  set-local-storage-item  smoke  smooth-scroll-to  type-alert-text  uc-click
  uc-type  uncheck-if-checked  wait-for-and-accept-alert
  wait-for-and-dismiss-alert  wait-for-element-not-visible
  wait-for-element-present  wait-for-ready-state-complete  wait-for-text
Files and generators: convert  extract-objects  import-playwright
  import-puppeteer  import-python  inject-objects  mkchart  mkdir  mkfile
  mkpres  objectify  print  revert-objects  translate
Record and replay: mkrec  recorder  replay
Tests: behave-gui  behave-options  caseplans  commander  run-scenario  test
Drivers, grid and servers: devtools  download  grid-hub  grid-node  install
  patch-chrome  patch-chromedriver  proxy
Security: decrypt  encrypt  fingerprint  solve-captcha
Info and setup: completions  doctor  methods  options  plugin  prompt  report
  version
Run 'sbrs help <COMMAND>' for one command, 'sbrs --help' for everything.

error: unrecognized subcommand 'nosuch'
```

```text Missing argument
$ sbrs mkrec --uc --url=google.com
Create a new test file with the Recorder: record your actions in a browser, then
save them as a test.

Usage: sbrs mkrec [--overwrite] [--behave] <FILE>

Arguments:
  <FILE>  The test file to create (FILE.rs)

Flags:
      --overwrite                 Overwrite the file when it exists
      --behave                    Also write the recording as a Gherkin feature
                                  (same as --rec-behave)
  -h, --help                      Print help

Global flags: written before or after the command; see 'sbrs --help'

Examples:
    $ sbrs mkrec new_test.rs --url seleniumbase.io/demo_page

error: the following required arguments were not provided:
  <FILE>
```

```text Unknown flag
$ sbrs mkrec new_test.rs --nosuch
Create a new test file with the Recorder: record your actions in a browser, then
save them as a test.

Usage: sbrs mkrec [--overwrite] [--behave] <FILE>

Arguments:
  <FILE>  The test file to create (FILE.rs)

Flags:
      --overwrite                 Overwrite the file when it exists
      --behave                    Also write the recording as a Gherkin feature
                                  (same as --rec-behave)
  -h, --help                      Print help

Global flags: written before or after the command; see 'sbrs --help'

Examples:
    $ sbrs mkrec new_test.rs --url seleniumbase.io/demo_page

error: unexpected argument '--nosuch' found

  tip: to pass '--nosuch' as a value, use '-- --nosuch'
```

```text No page
$ sbrs get-text --css h1
Print the visible text of an element.

Usage: sbrs get-text <--css <CSS>>

Flags:
      --css <CSS>                 CSS selector for the target element
  -h, --help                      Print help

Global flags: written before or after the command; see 'sbrs --help'

Examples:
    $ sbrs --url https://example.com get-text --css p

error: get-text acts on a page, and every sbrs command starts a fresh browser; give the page: sbrs --url https://example.com get-text …
```

</CodeGroup>

In the second run `--uc` and `--url` are fine: they are global options, which
are accepted before or after the command. What is wrong is the missing `FILE`.
The page lists only the flags of the command itself and sums the global ones up
in one line, so the part you need is short. A request for help or for the
version is no failure: it prints on standard output and exits with status 0.

## Open a page

Every `sbrs` command starts a fresh browser and quits it at the end, so a
command that acts on a page has to be told which page.

<Steps>

<Step title="Open a URL of your own" icon="globe">

```bash
./target/debug/sbrs open https://seleniumbase.io
```

`open` prints `Title: ...` for the page. It takes its URL as an argument, as do
`smoke`, `download-file`, `print-pdf`, `pdf-text` and `assert-pdf-text`. Those
six refuse `--url`, because a start page would be a second page.

</Step>

<Step title="Give any other command its page with --url" icon="link">

```bash
./target/debug/sbrs --url https://seleniumbase.io get-title
./target/debug/sbrs get-text --css h1 --url https://seleniumbase.io
```

`--url` (also `--start-page` and `--start_page`) is a global option, so it goes
before or after the command. A bare host such as `seleniumbase.io` gets
`https://`, and a local file becomes a `file://` URL.

</Step>

<Step title="Or set the page once" icon="gear">

```bash
SB_START_PAGE=https://seleniumbase.io ./target/debug/sbrs get-title
```

The command line wins over `SB_START_PAGE`, which wins over the `start_page` key
of the settings file. A command that acts on a page and finds none of the three
stops with status 2 before a browser starts (the "No page" tab above).

</Step>

<Step title="See where the command ended up" icon="location-dot">

A command that acts on the page (a click, typing) prints
`Now at: <url> (<title>)` when it is done, so a click that navigates shows where
it went. The global `--save-screenshot` saves a screenshot to `latest_logs/`
just before the browser closes and prints its path.

</Step>

</Steps>

<Note>

`clear-cookies`, `load-cookies`, `set-local-storage-item`,
`remove-local-storage-item`, `clear-local-storage` and `cache-clear` change only
the browser profile, which a fresh browser throws away. They stop with status 2
unless you add `--user-data-dir DIR`.

</Note>

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
./target/debug/sbrs --url https://seleniumbase.io screenshot
./target/debug/sbrs --url https://seleniumbase.io save-source
./target/debug/sbrs --url https://seleniumbase.io --save-screenshot get-title
```

## Assertions and waits

```bash
./target/debug/sbrs --url https://seleniumbase.io assert-element --css "body"
./target/debug/sbrs --url https://seleniumbase.io wait-for-text --css "body" --text "SeleniumBase" --timeout 15
```

Several steps in one browser are `run-scenario`, `deferred` and the MCP server.
There is no `go-back`, `refresh` or `switch-to-window` command: one that only
changes the state of a browser that is thrown away has no effect you could see.

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

The hint is shortened: `sbrs install` needs the driver's name, so type
`sbrs install chromedriver` (see [Install a driver](#install-a-driver)).

## Install a driver

```bash
./target/debug/sbrs install chromedriver            # for the installed Chrome
./target/debug/sbrs install chromedriver 155        # the newest chromedriver of Chrome 155
./target/debug/sbrs get chromedriver previous       # `get` is another name for `install`
./target/debug/sbrs get uc_driver                   # chromedriver patched for UC Mode
./target/debug/sbrs get geckodriver 0.36.0          # one geckodriver release
./target/debug/sbrs get edgedriver latest           # the stable msedgedriver
./target/debug/sbrs get chs                         # chrome-headless-shell
./target/debug/sbrs get chromium                    # the newest Chromium snapshot
```

`sbrs get` is another name for `sbrs install`, as in Python SeleniumBase, and
the driver is not optional: `sbrs install` alone prints the page of the command
and exits with status 2. Each download goes to a folder of `./downloaded_drivers/`
named after the program (and its version, such as `chromedriver-141.0.7390.76/`),
and `sbrs` prints one line, `NAME VERSION installed at PATH`. For chromium a
second line names the chromedriver of the same revision, and with `-p` a last
line says where the driver was copied.

The first argument is one of these words:

| DRIVER | What it downloads |
|---|---|
| `chromedriver` | The chromedriver for the installed Chrome |
| `uc_driver` | The chromedriver patched for UC Mode, kept in `uc_driver-VERSION/` |
| `geckodriver` (or `firefoxdriver`) | geckodriver, for Firefox |
| `edgedriver` (or `msedgedriver`) | msedgedriver, for the installed Edge |
| `cft` | Chrome for Testing, the browser |
| `chs` | chrome-headless-shell, the browser |
| `chromium` | A Chromium snapshot, the browser, with the chromedriver of the same revision |

The second argument, the version, is one of these:

| VERSION | What it asks for |
|---|---|
| `browser` | The driver for the installed browser |
| `stable`, `latest` | The newest stable release |
| `beta`, `dev`, `canary` | The newest release of that channel |
| `previous` (also `latest-1`) | The newest release of the major version before the stable one |
| `mlatest` | The newest build of the installed browser's major version; it needs an installed browser |
| `keep` | The driver already downloaded, else the one for the installed browser |
| a major version, such as `155` | The newest build of that major version |
| a full build, such as `141.0.7390.76` | That build |

geckodriver reads `latest`, `keep` or a release such as `0.36.0`. chromium has
revisions, not versions: give `--revision REVISION` (the newest snapshot when
it is left out), or `latest` or `keep`.

<Warning>

`-p` (`--path`) also copies the driver into the folder that `SB_DRIVER_PATH_DIR`
names, else into `/usr/local/bin`, which an ordinary user often may not write.
`sbrs` never starts `sudo`: a folder that is missing is refused before anything
is downloaded, and one that cannot be written gives an error that says so (the
driver stays in `downloaded_drivers/`). It never replaces a symbolic link or any
other file that is not a regular one. On Windows there is no default folder, so
`-p` needs `SB_DRIVER_PATH_DIR`. It copies drivers only: `cft`, `chs` and
`chromium` are browsers and refuse it, and `--revision` is for `chromium` alone.

</Warning>

Without a version, chromedriver is the one that matches the installed Chrome,
from Chrome for Testing (the latest stable one when Chrome is not found or no
driver is listed for its version), msedgedriver the one for the installed
Edge's major version from `msedgedriver.microsoft.com`, and geckodriver the
newest GitHub release; `cft` and `chs` follow the installed Chrome's major
version, or the stable release when there is none. It needs network access and
fetches only the driver or browser you name: the
[Pure CDP engine](/tutorials/pure_cdp) needs none. Starting a WebDriver session finds or
fetches the driver by itself, so run `install` ahead of time only to prepare an
offline machine or a container.

## Run a local proxy

`sbrs proxy` runs a small forward proxy on this machine: HTTP and `CONNECT`, with
no authentication. It is for trying a proxy setting such as `--proxy` without an
account at a proxy service.

<Steps>

<Step title="Start it" icon="play">

```bash
sbrs proxy
```

```text Output
Proxy listening on http://127.0.0.1:8899
Use it with: sbrs --proxy 127.0.0.1:8899 open https://example.com
```

It listens on `127.0.0.1:8899` unless `--hostname HOST` or `--port PORT` say
otherwise; `--port 0` picks a free port and prints it.

</Step>

<Step title="Point sbrs at it" icon="route">

In another terminal, give the browser the proxy with the global `--proxy`
option:

```bash
sbrs --proxy 127.0.0.1:8899 open https://example.com
```

`CONNECT host:port` becomes a tunnel, so HTTPS passes through untouched. An
absolute `http://` request is sent to its host once and the connection is closed.

</Step>

<Step title="Or try it with curl" icon="terminal">

Start any local web server, then ask for its page through the proxy:

```bash
curl -x 127.0.0.1:8899 http://127.0.0.1:8000/
```

The response is the local server's page. With `SB_LOG_LEVEL=info` the proxy
logs each request, such as `GET http://127.0.0.1:8000/ peer=127.0.0.1:64982`.

</Step>

<Step title="Stop it" icon="stop">

Ctrl+C or SIGTERM stops it with status 0.

</Step>

</Steps>

A request head over 64 KiB and more than 256 connections at once are refused,
and a connection that sends nothing for 30 seconds is cut.

<Warning>

Anything but a loopback address for `--hostname` makes this an open proxy for
other machines, and the command says so when it starts:

```text
Proxy listening on http://0.0.0.0:18899
Use it with: sbrs --proxy 127.0.0.1:18899 open https://example.com
warning: 0.0.0.0:18899 accepts connections from other machines; this is an open proxy
```

</Warning>

## Download the Selenium Server

`sbrs download server` saves the Selenium Server 4 jar, the program that
`grid-hub` and `grid-node` run, and prints its path and SHA-256.

```bash
sbrs download server                      # the version this build pins
sbrs download server --version 4.51.0     # another release
sbrs grid-hub start                       # uses the newest jar in downloaded_drivers/
sbrs grid-node start --hub 127.0.0.1      # registers with that hub
sbrs grid-node status
```

The jar is `selenium-server-VERSION.jar` in `./downloaded_drivers/`. The release
this build pins is checked against its SHA-256; another `--version` is saved as
it arrives, with its SHA-256 printed and `not verified`. Python's Selenium 3
server is not offered, because Grid 3 is gone.

`grid-hub` and `grid-node` take an action (`start`, `stop`, `restart` or
`status`) and download nothing. They need Java, and a jar from `--jar`, else
`SB_SELENIUM_SERVER_JAR`, else the newest one in `downloaded_drivers/`; the
process record and log are kept in the folder `SB_GRID_DIR` names. Their flags
are below; `-v` and `--timeout` are the names Python's `sbase grid-hub` uses.

| Flag | What it does |
|---|---|
| `--jar PATH`, `--java PATH` | The Selenium Server jar, and the Java executable (default: `java` on the `PATH`) |
| `--grid-host HOST`, `--grid-port PORT` | The address and port to listen on (default port: 4444 for a hub, 5555 for a node) |
| `--hub HOST_OR_URL` | For a node, the hub to register with: a host or a URL |
| `-v`, `--verbose` | Log at INFO instead of WARNING |
| `--timeout SECONDS` | Close sessions that have been idle this many seconds |
| `--wait SECONDS` | How long `start` waits for the Grid to become ready (default: 60) |
| `--skip-port-check` | Start even if something already accepts connections on the port |

More arguments for the Selenium Server go after `--`. `sbrs grid_hub` and
`sbrs grid_node` are accepted as well.


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

## Look up methods and options

```bash
sbrs methods                 # the common test methods, as they are called on sb
sbrs options                 # each browser option: flag, SB_* variable, settings key
sbrs options --markdown      # the same table, for documentation
sbrs options --all           # and the options of sbrs test
sbrs options --env           # the environment variables sbrs and sbrs-mcp read
sbrs behave-options          # how Python's behave -D options map to sbrs test
```

`methods` is Python's `sbase methods` in Rust spelling (`sb.open(url)`,
`sb.click(selector)`, ...), each with what it does; the whole API is in
`cargo doc --open` and [the parity page](/parity). The same option can be set
with the flag, in the environment and in the settings file, and the command
line wins over the environment, which wins over the file.


## Generate files

```bash
sbrs mkfile tests/login.rs                        # one browser test (alias: new)
sbrs mkfile tests/login.rs --url https://my.site  # ... that opens your page
sbrs mkfile tests/login.rs --zh --pf              # Chinese strings, the rstest sb fixture
sbrs mkdir tests/ui                               # a suite: main.rs, example tests, README
sbrs mkdir tests/ui --basic                       # the scaffolding only, no example tests
sbrs mkpres tests/deck.rs --es                    # a test that builds a three-slide deck, in Spanish
sbrs mkchart tests/chart.rs --ja                  # a test that shows a chart on a slide
sbrs print tests/login.rs -n                      # a file, with line numbers
sbrs translate scenario.json --es --copy          # writes scenario_es.json
sbrs translate scenario.json --zh -p -n           # prints the Chinese scenario with line numbers
sbrs convert login.py                             # a Selenium IDE export into login_SB.rs
```

The file is an argument, never a `--file` flag. The generated Rust is an
`#[rstest]` test on the `sb` fixture (see [Writing Browser Tests](/rust-test-tooling)),
and a test in `tests/` runs with `cargo test --test login`. A folder sitting
directly inside `tests/` is one test target; `tests/ui/README.md` says how to
register a folder anywhere else.

`mkfile` takes a syntax flag for the shape of the test: `--pf` (the default, the
`sb` fixture), `--bc` (a `BaseCase` made and quit by hand), `--cm` (the
`run_browser_test` closure), `--dc` (the pure CDP `sb_cdp` fixture) or `--dm` (a
pure CDP browser launched by hand); `--cf` is refused, because class fixtures
have no Rust form. `--uc` and `--cdp` write a basic test for UC Mode and for pure
CDP, `-b` leaves the body empty and `-r` adds a recorder breakpoint.

`mkfile`, `mkpres`, `mkchart` and `translate` take the language as a flag:
`--en`, `--zh`, `--nl`, `--fr`, `--it`, `--ja`, `--ko`, `--pt`, `--ru` or `--es`.
On the first three it chooses the language of the strings the test types
and expects (the method names stay English). On `translate` it chooses the
language of the actions of a JSON scenario, the file `run-scenario` runs, and it
may be written as a name too (`--spanish`); `-p` prints the translation (the
default), `-o` replaces the file, `-c` writes a copy next to it
(`scenario_es.json`) and `-n` numbers the printed lines. `print` shows a file as
it is, with no syntax highlighting.

`convert` is Python's `sbase convert` for a Selenium IDE or Katalon Recorder
export: it reads `login.py` as Selenium WebDriver code and writes a Rust test,
`login_SB.rs`, next to it. The original is kept, an existing `login_SB.rs` is
never replaced unless you pass `--force`, and what could not be converted stays
a `TODO` in the file and is listed on standard error; the exit status is 1 when
a statement could not be read at all. Python's writes a SeleniumBase class in a
`.py` file; this writes Rust.

The generators are careful with what they write:

- A name is relative to the current directory and may contain letters, digits,
  `_`, `-` and `.`, with `/` between folders. `..`, absolute paths, backslashes
  and other characters are rejected before anything is created, and a link that
  leads out of the directory is not followed.
- An existing file is never overwritten. `mkdir` checks every file first and
  creates none of them if one exists. Pass `--force` to replace regular files.
- They need no browser and ignore `sbase_config.toml`, so they work in an empty
  folder.

## Page objects

Page objects keep the selectors of a test in one file, so a page that changes
needs one edit and not one per test. Python's `sbase` has the commands; here
they work on a Rust test. Take this test:

```rust tests/signin.rs
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn signin(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://seleniumbase.io/simple/login").await?;
        sb.type_text("#username", "demo_user").await?;
        sb.type_text("#password", "secret_pass").await?;
        sb.click("#log-in").await?;
        sb.assert_text("Welcome!", "h1").await?;
        Ok(())
    })
    .await
}
```

<Steps>

<Step title="Extract the selectors" icon="file-export">

```bash
sbrs extract-objects tests/signin.rs
```

```text Output
Wrote 4 page objects to tests/page_objects.rs.
```

`signin.rs` is not changed. The new file is next to it:

```rust tests/page_objects.rs
// PAGE OBJECTS FILE >>> (autogenerated by `sbrs extract-objects`)
#![allow(dead_code)]

/// https://seleniumbase.io/simple/login
pub mod seleniumbase_io_simple_login {
    pub const USERNAME: &str = "#username";
    pub const PASSWORD: &str = "#password";
    pub const LOG_IN: &str = "#log-in";
    pub const H1: &str = "h1";
}
```

</Step>

<Step title="Inject them into the test" icon="file-import">

```bash
sbrs inject-objects tests/signin.rs -c
```

```text Output
Replaced 4 selectors in tests/signin.rs with page objects.
```

The test now reads the constants, and `-c` adds the selector as a comment to
each line that changed:

```rust tests/signin.rs
mod page_objects;
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn signin(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://seleniumbase.io/simple/login").await?;
        sb.type_text(page_objects::seleniumbase_io_simple_login::USERNAME, "demo_user").await?; // "#username"
        sb.type_text(page_objects::seleniumbase_io_simple_login::PASSWORD, "secret_pass").await?; // "#password"
        sb.click(page_objects::seleniumbase_io_simple_login::LOG_IN).await?; // "#log-in"
        sb.assert_text("Welcome!", page_objects::seleniumbase_io_simple_login::H1).await?; // "h1"
        Ok(())
    })
    .await
}
```

</Step>

<Step title="Put the selectors back" icon="rotate-left">

```bash
sbrs revert-objects tests/signin.rs -c
```

```text Output
Restored 4 selectors in tests/signin.rs.
```

The test is the first one again, byte for byte. `-c` also takes off the
comments that `inject-objects -c` appended; without it they stay.
`page_objects.rs` is kept.

</Step>

</Steps>

`sbrs objectify tests/signin.rs -c` does the first two steps in one command.
The three commands also answer to `extract_objects`, `inject_objects` and
`revert_objects`.

How the names and the files come about:

- A selector is a string literal that is the selector argument of a call, such
  as `sb.click("#go")`, `sb.type_text("#user", "ada")`,
  `sb.assert_text("Hi", "h1")` or `sb.drag_and_drop(".a", ".b")`;
  `sbrs methods` lists the common calls.
- Each page that the test opens gets a module of `page_objects.rs`, named with
  the words of its URL (`seleniumbase_io_simple_login`; `page` for selectors
  before any `open`). Each selector gets a constant named with the words of the
  selector (`USERNAME`; `_2` is added to a name that is taken).
- A `page_objects.rs` that `sbrs` wrote is added to; one that `sbrs` did not
  write is refused. A selector that has no page object stays as it is when
  you inject.


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

Each Python test becomes an `#[rstest]` test on the `sb` fixture, the form
`sbrs mkfile` writes (a script that drives CDP Mode becomes a pure CDP test).
Use `--source selenium-base` or `--source selenium` to override automatic
detection. Use `--test-name user_can_log_in` to choose the generated function
name when the file holds one test. Unsupported statements are retained as
`TODO` comments and diagnostics; [Migrate Python
Tests](/python-migration) lists what is converted.

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

The supported values are `bash`, `elvish`, `fish`, `powershell`, `zsh`, `fig` (a Fig spec) and `usage` (a usage spec), see below. PowerShell is `powershell`: Homebrew's formula calls the shell `pwsh` and passes `powershell` to `sbrs`, and `sbrs completions pwsh` itself is refused.

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

```powershell PowerShell
# In your profile: notepad $PROFILE
sbrs completions powershell | Out-String | Invoke-Expression
```

</CodeGroup>

<Note>

Ensure the destination directory exists. For Zsh, add `$HOME/.zfunc` to
`fpath` before running `compinit`. The PowerShell line goes in your profile, so
it runs each time a session starts and needs no file.

</Note>

<Note>

Re-run the line after upgrading `sbrs`. The script is written by the `sbrs` you
run it with (the Homebrew formula writes it when it installs), so write it again
when you upgrade by other means, and the script comes from the version you use.

</Note>

With the script loaded, `sbrs install <Tab>` offers the driver names, and
`sbrs install chromedriver sta<Tab>` completes `stable`. A global flag completes
whether it comes before or after the command, so `sbrs get-title --hea<Tab>`
offers the global flags that start with `--hea`.

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

The options of `sbrs` itself are global: write them before or after the
command, but each one once (`sbrs --headless get-title --headless` is refused
with status 2). The options below are the ones you reach for most; the page of
any command lists the rest.

<ParamField body="--url" type="URL">

The page the browser opens first (also `--start-page` and `--start_page`; the
environment variable is `SB_START_PAGE`). A bare host gets `https://`. Commands
that take a URL of their own refuse it. See [Open a page](#open-a-page).

</ParamField>

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

**Likely cause.** The flag is written twice (`sbrs --headless get-title --headless`),
or it is the flag of another command.

**Fix.** Write a global flag once, before or after the command: `sbrs --headless open URL`
and `sbrs open URL --headless` are the same.

</Accordion>

<Accordion title="error: get-text acts on a page" icon="link">

**Likely cause.** The command acts on a page and no start page was given.

**Fix.** Add `--url`: `sbrs --url https://example.com get-text --css p`, or set
`SB_START_PAGE`.

</Accordion>

<Accordion title="sbrs install prints a page and exits with status 2" icon="download">

**Likely cause.** `install` (and `get`) needs the driver's name.

**Fix.** Name it: `sbrs install chromedriver`.

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
