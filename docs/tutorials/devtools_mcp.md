---
title: "DevTools MCP servers"
---

# DevTools MCP servers

Browser vendors ship their own MCP servers that give a model the browser's
developer tools: performance traces, network and console inspection, the
accessibility tree. They attach to a browser that is already running, through
its DevTools endpoint. `sbrs devtools` starts such a browser for them, and the
`sbrs-mcp` Pure CDP server can hand out the endpoint of the browser it drives,
so a vendor server and this crate's tools can work on the same tabs.

| Server | Browser | Attaches with |
| --- | --- | --- |
| Google `chrome-devtools-mcp` | Chrome (Edge unofficially) | `--browserUrl http://127.0.0.1:PORT` |
| Microsoft Playwright MCP (`@playwright/mcp`) | Chrome, Edge, any Chromium | `--cdp-endpoint http://127.0.0.1:PORT` |
| Mozilla `@mozilla/firefox-devtools-mcp` | Firefox | Marionette and WebDriver BiDi, not this endpoint |
| Apple `safaridriver --mcp` | Safari 27 | Safari's own automation, not this endpoint |

The versions below are the ones that were current on 2026-10-15
(`chrome-devtools-mcp` 1.11.0, `@playwright/mcp` 0.0.83,
`@mozilla/firefox-devtools-mcp` 0.10.5). The generated configs pin the first two,
so a config keeps running the code it was written for; raise the version when
you want a newer one.

## `sbrs devtools`

```bash
sbrs devtools
```

It launches Chrome with the Pure CDP engine (no chromedriver), with its DevTools
endpoint on `127.0.0.1:9222` and a throwaway profile, and prints:

- the DevTools HTTP endpoint (`http://127.0.0.1:9222`) and the browser WebSocket
  URL (`ws://127.0.0.1:9222/devtools/browser/<id>`),
- a ready-to-paste client config for `chrome-devtools-mcp` and one for
  Playwright MCP,
- the `claude mcp add` commands for Claude Code.

The browser keeps running until you press Ctrl+C (or send SIGTERM), which closes
it and exits with status 0. Closing the browser window ends the command too.

| Option | Meaning |
| --- | --- |
| `--port N` | DevTools port on `127.0.0.1` (default 9222). `--port 0` lets Chrome pick a free one; a taken port is refused before Chrome starts. |
| `--headless` | No window. Without it the browser is headed, except on Linux. |
| `--user-data-dir DIR` | Keep the profile in `DIR`. Chrome refuses the DevTools endpoint on its own default profile, so use a directory of its own. |
| `--proxy SPEC` | `SERVER:PORT`, `USER:PASS@SERVER:PORT` or `scheme://SERVER:PORT`. Credentials are answered over the protocol, never put on Chrome's command line. |
| `--browser-path PATH` | Launch this binary, such as Microsoft Edge or another Chromium browser. |
| `--url URL` | Open this page once the browser runs. |
| `--json` | Print the endpoint and the configs as one line of JSON instead of text. |

The global options that make sense for a Pure CDP browser apply as well, written
before `devtools`: `--browser chrome|chromium|edge|brave|...`, `--headed`,
`--user-agent`, `--locale`, `--ad-block`, `--extension-dir` and `--proxy-list`.
`--uc` and `--cdp` change nothing here: this engine never uses chromedriver.

```bash
sbrs devtools --headless --port 0 --json
```

```json
{"http_url":"http://127.0.0.1:53017","websocket_url":"ws://127.0.0.1:53017/devtools/browser/…","version_url":"http://127.0.0.1:53017/json/version","mcp_configs":{"chrome_devtools_mcp":{…},"playwright_mcp":{…}},"claude_code":["claude mcp add chrome-devtools -- npx …","claude mcp add playwright -- npx …"],"browser":"Google Chrome"}
```

The `--json` line is the first line on standard output, so a script can read it
and start its client.

## The endpoint of an `sbrs-mcp` session

The Pure CDP server (`sbrs-mcp --server cdp`) has a tool for this:
`get_devtools_endpoint` returns the same addresses and configs for the browser
that `start_browser` launched, or `running: false` before one runs. See
[MCP servers](mcp_server.md#get_devtools_endpoint).

From Rust, `Browser::devtools_endpoint()` returns a `DevToolsEndpoint` with
`http_url()`, `websocket_url()`, `chrome_devtools_mcp_config()`,
`playwright_mcp_config()` and `claude_code_commands()`, and
`LaunchOptionsBuilder::debugging_port(9222)` fixes the port:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let options = LaunchOptions::builder().debugging_port(9222).build()?;
let browser = Browser::launch(options).await?;
let endpoint = browser.devtools_endpoint();
println!("{}", endpoint.chrome_devtools_mcp_config());
# Ok(())
# }
```

## Google chrome-devtools-mcp

The npm package `chrome-devtools-mcp` (by the Chrome DevTools team, source at
<https://github.com/ChromeDevTools/chrome-devtools-mcp>) runs on Node.js
(`^20.19.0`, `^22.12.0` or 23 and later) and drives Chrome through Puppeteer.
With `sbrs devtools` running, the client config is:

```json
{
  "mcpServers": {
    "chrome-devtools": {
      "command": "npx",
      "args": [
        "-y",
        "chrome-devtools-mcp@1.11.0",
        "--browserUrl=http://127.0.0.1:9222",
        "--no-usage-statistics",
        "--no-performance-crux"
      ]
    }
  }
}
```

For Claude Code:

```bash
claude mcp add chrome-devtools -- npx -y chrome-devtools-mcp@1.11.0 --browserUrl=http://127.0.0.1:9222 --no-usage-statistics --no-performance-crux
```

- `--browserUrl` (aliases `--browser-url`, `-u`) attaches to a running Chrome.
  `--wsEndpoint ws://127.0.0.1:9222/devtools/browser/<id>` (alias `-w`) is the
  other way in, with the WebSocket URL `sbrs devtools` prints; `--wsHeaders`
  adds headers to it.
- `--no-usage-statistics` turns off the usage statistics the server sends to
  Google by default (setting `CI` or `CHROME_DEVTOOLS_MCP_NO_USAGE_STATISTICS`
  does too). `--no-performance-crux` stops its performance tools from looking
  up page URLs in Google's CrUX data.
- Without an attach option the server launches a Chrome of its own (`--headless`,
  `--isolated`, `--channel`, `--executablePath`, `--userDataDir`).
- `--autoConnect` (Chrome 144 and later) attaches to the Chrome you use every
  day, after you enable `chrome://inspect/#remote-debugging` in it. On macOS 27
  it fails with `Could not find DevToolsActivePort`, because the system denies
  the terminal app access to Chrome's profile folder (open issue
  [#3089](https://github.com/ChromeDevTools/chrome-devtools-mcp/issues/3089)).
  The workaround is System Settings > Privacy & Security > Files & Folders, the
  terminal app, Google Chrome. `--browserUrl` avoids the problem.

## Microsoft Playwright MCP

The npm package `@playwright/mcp` (source at
<https://github.com/microsoft/playwright-mcp>) needs Node.js 18 or later. It
attaches over the DevTools Protocol with `--cdp-endpoint`, which takes the HTTP
address or the WebSocket URL:

```json
{
  "mcpServers": {
    "playwright": {
      "command": "npx",
      "args": ["-y", "@playwright/mcp@0.0.83", "--cdp-endpoint=http://127.0.0.1:9222"]
    }
  }
}
```

```bash
claude mcp add playwright -- npx -y @playwright/mcp@0.0.83 --cdp-endpoint=http://127.0.0.1:9222
```

- Playwright's own documentation says a DevTools Protocol connection
  (`connectOverCDP`) works with Chromium browsers only and is "significantly
  lower fidelity" than its own protocol. `--cdp-header` adds headers and
  `--cdp-timeout` (default 30000 ms) bounds the connection.
- `--extension` attaches to the Chrome or Edge you use every day through the
  Playwright extension instead of a debugging port.
- Without an attach option it launches its own browser: `--browser
  chrome|firefox|webkit|msedge`, `--headless`, `--isolated`, `--user-data-dir`.
  Its Firefox and WebKit are Playwright's patched builds, not the browsers you
  install.

## Microsoft Edge

Edge is Chromium, so the same endpoint and configs work:

```bash
sbrs --browser edge devtools
# or, for an Edge somewhere else:
sbrs devtools --browser-path "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"
```

Playwright MCP supports Edge (`--cdp-endpoint`, `--extension`, or `--browser
msedge`). `chrome-devtools-mcp` supports only Google Chrome and Chrome for Testing
officially and says other Chromium browsers may work; one reported difference is
that Edge tabs opened with Ctrl+T were missing from `list_pages` (issue #2880).

## Firefox: Mozilla's firefox-devtools-mcp

Firefox does not speak the Chrome DevTools Protocol, so neither server above
can attach to it, and `sbrs devtools` refuses `--browser firefox`. Mozilla's
`@mozilla/firefox-devtools-mcp` (source at
<https://github.com/mozilla/firefox-devtools-mcp>, Node.js 20.19 or later,
Firefox 100 or later) drives Firefox through WebDriver Classic and WebDriver
BiDi:

```json
{
  "mcpServers": {
    "firefox-devtools": {
      "command": "npx",
      "args": ["-y", "@mozilla/firefox-devtools-mcp@latest", "--headless", "--viewport", "1280x720"]
    }
  }
}
```

To attach to a Firefox that is already running, start it with both
`--marionette` and `--remote-debugging-port`, and run the server with
`--connect-existing --marionette-port 2828`.

## Safari: safaridriver --mcp

Safari 27 ships an MCP server inside `safaridriver`, which is part of macOS:

```json
{
  "mcpServers": {
    "safari-mcp": {
      "command": "/usr/bin/safaridriver",
      "args": ["--mcp"]
    }
  }
}
```

```bash
claude mcp add safari-mcp -- /usr/bin/safaridriver --mcp
```

It needs two Safari settings: Advanced, "Show features for web developers", and
then Developer, "Allow remote automation and external agents" (see
<https://webkit.org/blog/18136/introducing-the-safari-mcp-server-for-web-developers/>).

## Security

- The DevTools endpoint listens on `127.0.0.1` only, but any program on this
  machine can connect to it and drive the browser: read its pages and cookies,
  type, and run script. Run `sbrs devtools` with a throwaway profile (the
  default) and stop it when you are done.
- These servers give the model what the browser shows, including logged-in
  pages. Each project's README says so; connect only clients you trust.
- `chrome-devtools-mcp` sends usage statistics to Google unless
  `--no-usage-statistics` is given; the generated config gives it.

## What was checked

`sbrs devtools` and `get_devtools_endpoint` have unit tests for the configs and
the arguments, a mocked-browser test of the tool, and real-Chrome tests
(`tests/devtools_cli_chrome.rs`, run with
`cargo test --test devtools_cli_chrome -- --ignored --test-threads=1`) that start
the command against Chrome 155 on macOS, read `/json/version` from the printed
endpoint, and stop it with SIGINT and SIGTERM. The vendor MCP servers themselves
were not installed or run for this page: their flags come from their READMEs
and documentation as of 2026-10-15.
