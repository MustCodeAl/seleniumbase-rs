---
title: "Settings and Global Configuration"
sidebarTitle: "Settings"
---

`seleniumbase-rs` supports multiple configuration layers: built-in defaults, a
global config file, `SB_*` environment variables, and programmatic
`BrowserConfig` overrides. This makes it easy to keep environment-specific
settings out of source code while still allowing per-test customization.

This page explains the configuration hierarchy, the available settings, and how
to load and combine them.

## Configuration hierarchy

Settings are resolved from lowest to highest priority. A higher-priority source
overrides a lower-priority one:

1. Built-in defaults.
2. Global config file (`sbase_config.toml`, `.sbase_config.toml`, or `.sbase_config`).
3. Environment variables (`SB_*`).
4. Programmatic `BrowserConfig`.
5. CLI flags (when using `sbrs`).

<Info>

This means a `BrowserConfig` value in code wins over an environment variable,
and a CLI flag wins over everything else.

</Info>

<Visibility for="agents">

Summary for AI exports: settings resolve from lowest to highest priority: built-in defaults, the global config file (`sbase_config.toml`, `.sbase_config.toml` or `.sbase_config` in the current directory), `SB_*` environment variables, a programmatic `BrowserConfig`, and CLI flags when using `sbrs`. `Settings::load()` reads the global file and applies the variables; `Settings::from_file(path)` reads the file alone; `settings.to_browser_config()` makes the `BrowserConfig`. `BrowserConfig::default()` reads no variables, while `BrowserConfig::from_env()` reads the runtime ones such as `SB_WEBDRIVER_URL` and `SB_CHROME_BIN`.

</Visibility>

## Global config file

Create `sbase_config.toml` in your project root (or a `.sbase_config` file in
JSON; a flat `KEY=VALUE` file is not supported):

<CodeGroup>

```toml sbase_config.toml
browser = "chrome"
headless = false
timeout_seconds = 30
window_width = 1920
window_height = 1080
proxy = "http://proxy.example.com:8080"
proxy_pac_url = ""
mode = "webdriver"
reuse_session = false
reuse_class_session = false
mobile = false
threads = 2
locale = "en-US"
user_agent = ""
user_data_dir = ""
extension_dir = ""
ad_block = false
esc_end = false
rec_sb_cdp = false
```

```json .sbase_config
{ "headless": false, "locale": "en-US", "timeout_seconds": 10 }
```

</CodeGroup>

The file is optional. If it is missing, built-in defaults are used. The
`Settings::load_global()` function searches for `sbase_config.toml`, then
`.sbase_config.toml`, then `.sbase_config` in the current directory.

## Loading settings programmatically

```rust
use seleniumbase_rs::config::settings::Settings;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load the global config file (if present) and apply SB_* env overrides.
    let settings = Settings::load()?;

    // Or load a specific file, with the SB_* overrides on top.
    let settings = Settings::load_from("staging.toml")?;

    // Or only the file, leaving the environment out.
    let settings = Settings::from_file("staging.toml")?;

    // Convert to BrowserConfig.
    let config = settings.to_browser_config();

    Ok(())
}
```

`Settings::load()` reads the global config file and `Settings::load_from(path)`
reads the file you name; both apply the environment variables on top.
`Settings::from_file` reads the file alone. All three refuse a `browser` or
`mode` that is not a known name (`SB_BROWSER=chrom` is an error, not Chrome),
and a `window_width` without a `window_height`.

## Environment variables

Every setting in `Settings` can be overridden with an `SB_` prefixed variable:

| Variable | Maps to |
|----------|---------|
| `SB_BROWSER` | `browser` |
| `SB_HEADLESS` | `headless` (see [Headless and headed browsers](/tutorials/headless) for which entry points read it) |
| `SB_TIMEOUT` | `timeout_seconds`: the default wait `run_browser_test` gives its `BaseCase` |
| `SB_SCREENSHOT_DIR` | `screenshot_dir` (stored for your code; the library does not read it) |
| `SB_PROXY` | `proxy` |
| `SB_PROXY_PAC_URL` | `proxy_pac_url` |
| `SB_WINDOW_WIDTH` | `window_width`: with `window_height`, the browser window's size |
| `SB_WINDOW_HEIGHT` | `window_height` |
| `SB_WEBDRIVER_URL` | `webdriver_url` (default `http://localhost:4444`) |
| `SB_CHROME_BIN` | `chrome_bin`: the browser binary |
| `SB_USER_DATA_DIR` | `user_data_dir` |
| `SB_EXTENSION_DIR` | `extension_dir` |
| `SB_LOCALE` | `locale` |
| `SB_USER_AGENT` | `user_agent` |
| `SB_MODE` | `mode` (`webdriver`, `cdp`, or `uc`) |
| `SB_AD_BLOCK` | `ad_block` |
| `SB_REUSE_SESSION` | `reuse_session` |
| `SB_REUSE_CLASS_SESSION` | `reuse_class_session` |
| `SB_MOBILE` | `mobile` |
| `SB_THREADS` | `threads` |
| `SB_ESC_END` | `esc_end`: pressing ESC in the browser ends the test (Python's `--esc-end`) |
| `SB_REC_SB_CDP` | `rec_sb_cdp`: recordings are written as pure CDP code (Python's `--rec-sb-cdp`) |
| `SB_REMOTE_WEBDRIVER_TIMEOUT` | `remote_webdriver_timeout_seconds` |

Boolean values accept `true`/`false`, `1`/`0`, `yes`/`no`, or `on`/`off`.

Example:

```bash
export SB_BROWSER=firefox
export SB_HEADLESS=true
export SB_TIMEOUT=60
export SB_PROXY=http://proxy:8080
export SB_THREADS=4
```

### Variables of `sbrs` and `sbrs-mcp`

The table above is the keys of the settings file. The two command lines read more:
every setting of `sbrs` has an `SB_*` variable, and `sbrs-mcp` has `SB_MCP_*`
variables for its own flags. The `--help` of a flag ends with its variable
(`(SB_DARK_MODE)`, or `(SB_MODE=uc)` for a shorthand flag). To see them all, with
who reads each one, the flag that sets the same thing, its default and what it sets:

```bash
sbrs options --env
sbrs options --env --markdown
```

`--markdown` prints the table for documentation. The list is made from the option
table and the list of the other variables, so it names every variable the command
lines read.

**Order.** A flag wins over its variable, which wins over the settings file, which
wins over the default. A variable that is empty or blank counts as unset.

**A bad value stops the program.** A variable that cannot be used ends `sbrs` or
`sbrs-mcp` with status 2, before a browser starts or the server listens, and the
message names the variable and the flag it stands for:

```text
SB_HEADLESS (--headless): cannot parse bool: maybe
```

A few variables only warn, such as a `SB_LOG_FORMAT` that is neither `pretty` nor
`json`: it is reported and the default is used.

Variables that are not keys of the settings file include these (the rest are in
`sbrs options --env`):

| Variable | Flag | Default | What it sets |
| --- | --- | --- | --- |
| `SB_PROXY_LIST` | `sbrs --proxy-list` | none | A file of proxies to choose from; without a proxy a random one is used |
| `SB_SAVE_SCREENSHOT` | `sbrs --save-screenshot` | off | Save a screenshot to `latest_logs/` just before the browser closes, and print its path |
| `SB_PATCH_CACHE_DIR` | `sbrs patch-chrome --cache-dir` | the user cache folder | The folder that holds patched browser binaries |
| `SB_MCP_SERVER` | `sbrs-mcp --server` | `sb` | Which toolset the MCP server serves: `cdp`, `driver`, `sb` or `dev` |
| `SB_MCP_TRANSPORT` | `sbrs-mcp --transport` | `stdio`, or `http` when a listen address is given | How the server talks to its client: `stdio` or `http` |
| `SB_MCP_LISTEN` | `sbrs-mcp --listen` | `127.0.0.1:8931` for http | The IP address and port of the HTTP endpoint; setting it serves over HTTP |
| `SB_MCP_PATH` | `sbrs-mcp --path` | `/mcp` | The URL path of the HTTP endpoint |
| `SB_MCP_TOKEN` | none | none | The bearer token the HTTP endpoint asks for; an address that is not loopback needs one |
| `SB_MCP_TOKEN_FILE` | `sbrs-mcp --token-file` | none | A file that holds the bearer token; it wins over `SB_MCP_TOKEN` |
| `SB_MCP_NO_TOKEN` | `sbrs-mcp --no-token` | `false` | Serve HTTP on a loopback address without a bearer token; without it a loopback listener given no token gets a random one, printed once and written to `mcp-token` in the output folder |
| `SB_MCP_ALLOW_ORIGIN` | `sbrs-mcp --allow-origin` | none | Origins (`scheme://host[:port]`) of browser pages that may use the HTTP endpoint, separated by commas |
| `SB_MCP_ALLOW_HOST` | `sbrs-mcp --allow-host` | none | Host names accepted in the `Host` header besides localhost, separated by commas |
| `SB_MCP_MAX_SESSIONS` | `sbrs-mcp --max-sessions` | `4` | The most MCP sessions, and so browsers, the HTTP server holds at once |
| `SB_MCP_OUTPUT_DIR` | `sbrs-mcp install --output-dir` | `./mcp_output` | The folder the tools of the MCP server write their files to |
| `SB_MCP_URL` | `--url` of the `sbrs-mcp` client commands | none | The server that `tools`, `call` and the other client commands connect to when no flag names one (not in the `options --env` list) |

See [MCP servers](/tutorials/mcp_server#calling-tools-from-a-shell) for the client
commands and the `--url` and `--token-file` flags.


## Runtime environment variables

In addition to `Settings`, the crate reads `SB_*` runtime configuration for
library behavior:

<ParamField body="SB_WEBDRIVER_URL" type="string" default="http://localhost:4444">

WebDriver endpoint.

</ParamField>

<ParamField body="SB_CHROME_BIN" type="string" default="auto-detect">

Explicit Chrome/Chromium binary path.

</ParamField>

<ParamField body="SB_PATCH_CACHE_DIR" type="string" default="platform cache">

Directory for patched Chrome/Chromedriver copies.

</ParamField>

<ParamField body="SB_LOG_LEVEL" type="string">

The tracing filter. Unset, blank or invalid, `sbrs` and `sbrs-mcp` use `RUST_LOG`,
then `info`.

</ParamField>

<ParamField body="SB_LOG_FORMAT" type="string" default="pretty">

`pretty` (a readable line, coloured on a terminal) or `json` (one object per line, never
coloured). Anything else is reported and the default is used.

</ParamField>

<ParamField body="NO_COLOR" type="any value">

Turns colour off for the log lines, for the banners of `sbrs mkrec` and for the
[debug prompts](/tutorials/writing_tests#debug-prompts) (`--pdb`, `--trace`, `breakpoint`)
([no-color.org](https://no-color.org)). An empty value does not count. Colour is used only when the
stream (standard error for logs and prompts, standard output for banners) is a terminal and `TERM`
is not `dumb`.

</ParamField>

<ParamField body="SB_SHUTDOWN_TIMEOUT_SECS" type="integer" default="30">

Graceful shutdown timeout.

</ParamField>

<ParamField body="SB_CHROMEDRIVER_PORT" type="integer" default="0">

<Badge color="gray">Not read yet</Badge>

Parsed into `RuntimeConfig::chromedriver_port`; nothing in the crate reads it yet.

</ParamField>

<ParamField body="SB_IMPLICIT_WAIT_SECS" type="integer" default="30">

<Badge color="gray">Not read yet</Badge>

Parsed into `RuntimeConfig::implicit_wait`; nothing in the crate reads it yet. For
the default wait of a test, use `SB_TIMEOUT` or `BaseCase::set_timeout`.

</ParamField>

<ParamField body="SB_REMOTE_WEBDRIVER_TIMEOUT" type="integer">

Unset by default. Client-side timeout, in seconds, for each HTTP request sent to
a Remote WebDriver server.

</ParamField>

## Remote WebDriver timeout

When tests run against a Selenium Grid, a node that dies mid-session can leave a
command waiting forever, because the WebDriver client has no reason to give up
on a connection that never closes. `remote_webdriver_timeout_seconds` (or the
`SB_REMOTE_WEBDRIVER_TIMEOUT` variable) puts a client-side limit on each HTTP
request sent to the server.

It is unset by default, matching SeleniumBase's `REMOTE_WEBDRIVER_TIMEOUT`, so
the client keeps its own default until you opt in.

<Warning>

If you set it, make it larger than your page-load timeout and larger than the
longest a new-session request may wait in the grid's queue. A value below either
one cuts off sessions that were healthy.

</Warning>

<CodeGroup>

```toml sbase_config.toml
remote_webdriver_timeout_seconds = 600
```

```bash Environment
export SB_REMOTE_WEBDRIVER_TIMEOUT=600
```

</CodeGroup>

## BrowserConfig in code

For per-test overrides, build a `BrowserConfig` directly:

```rust
use seleniumbase_rs::{BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_headless(true)
    .with_proxy("http://proxy:8080")
    .with_locale("en-US")
    .with_extra_args(vec!["--disable-blink-features=AutomationControlled".into()]);
```

`BrowserConfig` also supports `from_env()` to load runtime variables directly:

```rust
use seleniumbase_rs::BrowserConfig;

let config = BrowserConfig::from_env()?;
```

## Proxy lists and ad-block lists

Two small types in `seleniumbase_rs::config` hold lists that are too long for
a config file field.

`ProxyList` is Python's `PROXY_LIST`: proxies, optionally named, read from a
text file (see [CLI usage](/tutorials/cli_usage#proxies-from-a-list) for the format).
Each entry is validated when it is read. It resolves a name or a literal proxy
(`resolve`), and hands proxies out in turn (`next_round_robin`) or at random
(`random`); the turn counter is atomic, so one list can serve parallel tests:

```rust
use seleniumbase_rs::config::proxy_list::ProxyList;
use seleniumbase_rs::BrowserConfig;

let proxies = ProxyList::from_file("proxies.txt")?;
let entry = proxies.next_round_robin().expect("a non-empty list");
let config = BrowserConfig::default().with_proxy(entry.spec());
```

`ProxyEntry`'s `Debug` and `Display` never show a password; only `spec()` does.

`AdBlockList` is a set of hosts to refuse, with their subdomains.
`AdBlockList::builtin()` is the crate's starter list of advertising and
tracking hosts, `AdBlockList::parse` reads the host rules of a hosts file or
an Adblock Plus list (and counts the rules it cannot express), and
`url_patterns()` gives the patterns for the DevTools command
`Network.setBlockedURLs`. `is_blocked(url)` matches on the parsed host, so a
blocked name inside a query string does not block the page.

A Chromium WebDriver session sends such a list to the browser when
`BrowserConfig::ad_block` is on: the built-in list, or yours through
`with_ad_block_list(list)`, which replaces it. `bounded_url_patterns()` is what
is sent. It leaves out a host whose parent domain is also listed and refuses a
list that needs more than 2,000 patterns, so the browser is never given half of
what you asked for. See [Blocking ads](/help/customizing_test_runs#blocking-ads).

## Best practices

- Commit a sample config file (e.g., `sbase_config.example.toml`) to the repo so
  new contributors know what settings exist.
- Keep secrets (passwords, API keys) out of config files; use environment
  variables or a secrets manager.
- Use `Settings::load_global()` in test harnesses and `BrowserConfig` overrides
  for individual tests.
- Use `BrowserConfig::from_env()` in deployable applications so operators can
  change behavior without rebuilding.

## Configuration checklist

- [ ] A sample config file is committed to the repository.
- [ ] Secrets are loaded from environment variables.
- [ ] Per-test overrides use `BrowserConfig` builders.
- [ ] CI uses environment variables or a dedicated config file.
- [ ] `SB_*` variables are documented for operators.

## Related reading

<Columns cols={2}>

<Card color="#1a85d4" title="CLI Usage" icon="terminal" href="/tutorials/cli_usage">

Command-line overrides and flags.

</Card>

<Card color="#199f43" title="Tracing and Logging" icon="list" href="/tutorials/tracing">

`SB_LOG_LEVEL` and `SB_LOG_FORMAT`.

</Card>

<Card color="#d47628" title="Docker Guide" icon="box" href="/help/docker">

Containerized configuration.

</Card>

</Columns>
