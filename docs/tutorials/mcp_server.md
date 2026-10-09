# MCP servers

`seleniumbase-mcp` exposes a browser to any [Model Context Protocol] client
(Claude Desktop, Claude Code, ...) over stdio. It serves one of three toolsets,
the same tools as the Python `seleniumbase-mcp` project:

| `--server` | Engine | Tools |
| --- | --- | --- |
| `cdp` | Pure CDP (no WebDriver) | 24 |
| `driver` | WebDriver, `Driver()` toolset | 26 |
| `sb` (default) | WebDriver, `SB()` toolset, plus this crate's stealth tools | 88 + 8 |

All three keep one browser session for the whole conversation: call
`start_browser` once, use the other tools, then `close_browser`.

## Install and configure

```bash
cargo install --path . --bin seleniumbase-mcp --features mcp-server
```

Then register it with your client, for example in `.mcp.json`:

```json
{
  "mcpServers": {
    "seleniumbase-cdp": {
      "command": "seleniumbase-mcp",
      "args": ["--server", "cdp"],
      "env": { "SB_MCP_OUTPUT_DIR": "/tmp/seleniumbase-output" }
    }
  }
}
```

Use `["--server", "driver"]` or `["--server", "sb"]` for the others. Logs go to
stderr; stdout carries only the protocol.

## Which server?

- **`cdp`** is the lightest and the hardest to detect: no chromedriver, and it
  can pass common CAPTCHA widgets. Chrome or Chromium only. Its tools group
  related actions behind an `action`/`mode`/`state` argument, so there are few of
  them (`click_element`, `type_text`, `wait_for_condition`, `manage_tabs`, ...).
- **`driver`** and **`sb`** use WebDriver, so they also work with Edge and
  Firefox. `sb` has a tool for nearly every `BaseCase` method (`click_link`,
  `drag_and_drop`, `get_mfa_code`, `download_file`, ...). They need a
  chromedriver that matches your Chrome; a mismatch is reported as a readable
  error when `start_browser` runs.

## How tools behave

- **Errors are results.** A failed action (missing element, timeout, bad
  argument) is returned to the model as an error result it can read and retry,
  not as a protocol failure. Messages include a hint where one exists.
- **Waits are bounded.** Every `timeout` or pause is capped at one hour.
- **Files stay in one folder.** Screenshots, saved pages, PDFs, downloads and
  cookie files are written under `SB_MCP_OUTPUT_DIR` (default `./mcp_output`).
  Absolute paths and `..` are refused. Cookie files go to `saved_cookies/` there.
- **Annotations are honest.** Each tool declares whether it reads, navigates or
  overwrites files, so a client can decide what to confirm. A tool whose actions
  differ in kind (for example `manage_cookies`) declares only its title.
- **Defaults.** `start_browser` runs headless on Linux and headed elsewhere unless
  you set `headless`.

## Security

These tools let a model drive a real browser as you.

- `run_javascript`, `execute_script` and `evaluate` run arbitrary script in the
  current page.
- Cookies and web storage can hold logins. `manage_cookies`, `get_cookies` and
  the storage tools return them to the model.
- `choose_file` uploads a file from this machine; `download_file` fetches a URL
  from this machine's network.
- `patch_chromedriver` (in `sb`) modifies a binary on disk.

Only connect clients you trust, and prefer a throwaway browser profile.

## Using the servers from Rust

```rust,no_run
use seleniumbase_rs::mcp::{self, Profile};

# async fn run() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
mcp::serve(Profile::Cdp).await?;
# Ok(())
# }
```

`mcp::cdp::host`, `mcp::driver::host` and `mcp::sb::host` build a `Host` you can
call directly with `Host::call(name, arguments)`, which is how the test suite
exercises every tool against a mocked browser. To add a tool, see "Adding an MCP
tool" in the [developer guide](../DEVELOPER_GUIDE.md).

[Model Context Protocol]: https://modelcontextprotocol.io
