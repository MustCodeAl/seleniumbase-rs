---
title: "MCP servers"
---

export const LayoutTree = ({ children }) => (
  <div className="not-prose layout-tree" style={{ margin: "1.25rem 0", fontFamily: "var(--font-sans)", fontSize: "14px", color: "var(--text-secondary)" }}>
    <style>{`
      .layout-tree summary { list-style: none; }
      .layout-tree summary::-webkit-details-marker { display: none; }
      .layout-tree .lt-row:hover { background-color: var(--surface-subtle); }
      .layout-tree .lt-fold { color: #d5a910; }
      .dark .layout-tree .lt-fold { color: #ffd452; }
      .layout-tree .lt-fold-closed { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-open { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-closed { display: flex; }
      .layout-tree .lt-hl { color: var(--primary); background-color: color-mix(in srgb, var(--primary) 8%, transparent); }
      .dark .layout-tree .lt-hl { color: var(--primary-light); }
      .layout-tree .lt-ic-gray { color: #84848a; }
      .dark .layout-tree .lt-ic-gray { color: #adadb1; }
      .layout-tree .lt-ic-blue { color: #1a85d4; }
      .dark .layout-tree .lt-ic-blue { color: #69b1ff; }
      .layout-tree .lt-ic-cyan { color: #1ca1c7; }
      .dark .layout-tree .lt-ic-cyan { color: #68cdf2; }
      .layout-tree .lt-ic-green { color: #199f43; }
      .dark .layout-tree .lt-ic-green { color: #5ecc71; }
      .layout-tree .lt-ic-indigo { color: #693acf; }
      .dark .layout-tree .lt-ic-indigo { color: #9d6afb; }
      .layout-tree .lt-ic-orange { color: #d47628; }
      .dark .layout-tree .lt-ic-orange { color: #ffa359; }
      .layout-tree .lt-ic-pink { color: #d32a61; }
      .dark .layout-tree .lt-ic-pink { color: #ff678d; }
      .layout-tree .lt-ic-purple { color: #a631be; }
      .dark .layout-tree .lt-ic-purple { color: #d568ea; }
      .layout-tree .lt-ic-red { color: #d52c36; }
      .dark .layout-tree .lt-ic-red { color: #ff6762; }
      .layout-tree .lt-ic-yellow { color: #d5a910; }
      .dark .layout-tree .lt-ic-yellow { color: #ffd452; }
    `}</style>
    {children}
  </div>
)

export const LayoutRow = ({ icon, name, note, highlight = false, button = false }) => (
  <div className={highlight ? "lt-row lt-hl" : "lt-row"} style={{ display: "flex", alignItems: "flex-start", gap: "6px", minHeight: "30px", padding: "5px 8px", margin: "0 2px", borderRadius: "6px", lineHeight: "20px", cursor: button ? "pointer" : "default" }}>
    <span style={{ width: "16px", height: "20px", flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", color: highlight ? "inherit" : "var(--text-muted)" }}>{icon}</span>
    <span style={{ whiteSpace: "nowrap", fontFamily: "var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)", fontSize: "13px", fontWeight: 600, color: highlight ? "inherit" : "var(--text-primary)" }}>{name}</span>
    {note ? <span style={{ color: "var(--text-muted)", fontSize: "13px", fontStyle: "italic", marginLeft: "10px", opacity: 0.9 }}>{note}</span> : null}
  </div>
)

export const LayoutFolderIcon = ({ kind }) => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    {kind === "empty" ? (
      <path fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" d="M2 4.2A1.2 1.2 0 0 1 3.2 3h3.1c.32 0 .62.13.85.35L8.4 4.6h4.4A1.2 1.2 0 0 1 14 5.8v6a1.2 1.2 0 0 1-1.2 1.2H3.2A1.2 1.2 0 0 1 2 11.8z" />
    ) : kind === "open" ? (
      <g fill="currentColor">
        <path opacity=".5" d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H13a1.5 1.5 0 0 1 1.5 1.5V7H4.2a1.5 1.5 0 0 0-1.4.95L1.5 11.3z" />
        <path d="M4.2 8h10.6a.7.7 0 0 1 .65.95l-1.6 4.1a1.5 1.5 0 0 1-1.4.95H2.2a.7.7 0 0 1-.65-.95l1.25-3.2A1.5 1.5 0 0 1 4.2 8z" />
      </g>
    ) : (
      <g fill="currentColor">
        <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H1.5z" />
        <path opacity=".6" d="M1.5 4.5V12A1.5 1.5 0 0 0 3 13.5h10a1.5 1.5 0 0 0 1.5-1.5V6A1.5 1.5 0 0 0 13 4.5z" />
      </g>
    )}
  </svg>
)

export const LayoutFileIcon = () => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    <path fill="currentColor" opacity=".5" d="M8 1v3a3 3 0 0 0 3 3h3v5.5a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 2 12.5v-9A2.5 2.5 0 0 1 4.5 1z" />
    <path fill="currentColor" d="M9.5 1a.5.5 0 0 1 .354.146l4 4A.5.5 0 0 1 14 5.5V6h-3a2 2 0 0 1-2-2V1z" />
  </svg>
)

export const LayoutDir = ({ name, note, open = false, highlight = false, children }) =>
  children ? (
    <details open={open}>
      <summary>
        <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><span className="lt-fold-open" style={{ display: "flex" }}><LayoutFolderIcon kind="open" /></span><span className="lt-fold-closed"><LayoutFolderIcon kind="closed" /></span></span>} name={name} note={note} highlight={highlight} button />
      </summary>
      <div style={{ marginLeft: "17.5px", borderLeft: "1px solid var(--border-subtle)", paddingLeft: "6px" }}>
        {children}
      </div>
    </details>
  ) : (
    <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><LayoutFolderIcon kind="empty" /></span>} name={name} note={note} highlight={highlight} />
  )

export const layoutIconColor = (name) => {
  const n = String(name).toLowerCase()
  const ext = n.includes(".") ? n.split(".").pop() : ""
  if (n === "dockerfile" || n.startsWith("dockerfile.") || n.startsWith("docker-compose")) return "blue"
  if (["rs", "json", "jsonc", "html", "htm", "zip", "gz", "tgz", "crx", "xpi", "swift", "svg"].includes(ext)) return "orange"
  if (["ts", "tsx", "mts", "cts", "py"].includes(ext)) return "blue"
  if (["js", "mjs", "cjs", "jsx"].includes(ext)) return "yellow"
  if (["md", "mdx", "sh", "bash", "zsh", "fish", "ps1"].includes(ext)) return "green"
  if (["css", "scss"].includes(ext)) return "indigo"
  if (["png", "jpg", "jpeg", "gif", "webp", "ico"].includes(ext)) return "pink"
  if (ext === "go") return "cyan"
  return "gray"
}

export const LayoutFile = ({ name, note, highlight = false }) => (
  <LayoutRow icon={<span className={highlight ? "" : "lt-ic-" + layoutIconColor(name)} style={{ display: "flex" }}><LayoutFileIcon /></span>} name={name} note={note} highlight={highlight} />
)

`sbrs-mcp` exposes a browser to any [Model Context Protocol] client
(Claude Desktop, Claude Code, ...) over stdio. It serves one of three toolsets,
the same tools as the Python `seleniumbase-mcp` project, and adds tools of its
own for developing, debugging and testing:

| `--server` | Engine | Tools |
| --- | --- | --- |
| `cdp` | Pure CDP (no WebDriver) | 24 + 9 developer tools + `get_devtools_endpoint` |
| `driver` | WebDriver, `Driver()` toolset | 26 |
| `sb` (default) | WebDriver, `SB()` toolset, plus this crate's stealth tools | 88 + 7 stealth + 9 developer tools |

<Info>

All three keep one browser session for the whole conversation: call
`start_browser` once, use the other tools, then `close_browser`.

</Info>

## Install and configure

<Steps>

<Step title="Install the binary" icon="download">

```bash Terminal
cargo install --path crates/cli --bin sbrs-mcp
```

Or, without a checkout:
`cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli --locked`
(both binaries; the `mcp-server` feature is on by default in that package).

</Step>

<Step title="Register it with your client" icon="plug">

For example in `.mcp.json`:

```json .mcp.json
{
  "mcpServers": {
    "seleniumbase-cdp": {
      "command": "sbrs-mcp",
      "args": ["--server", "cdp"],
      "env": { "SB_MCP_OUTPUT_DIR": "/tmp/seleniumbase-output" }
    }
  }
}
```

Use `["--server", "driver"]` or `["--server", "sb"]` for the others.

To run a built-in plugin in every browser the server starts, add
`"--plugin", "shield"` to `args` (and `"--plugin-opt", "shield.level=\"strict\""`
for an option, a TOML value). The operator chooses the plugins; no tool argument
adds one, and a mistake in them stops the server before it listens. See
[Choosing plugins](/plugins/selecting#sbrs-mcp).

</Step>

</Steps>

<Note>

Logs go to stderr; stdout carries only the protocol.

</Note>

## Which server?

<Tabs>

<Tab title="cdp" icon="chrome">

`cdp` is the lightest and the hardest to detect: no chromedriver, and it
can pass common CAPTCHA widgets. Chrome or Chromium only
<Badge color="gray">Needs Chrome</Badge>. Its tools group
related actions behind an `action`/`mode`/`state` argument, so there are few of
them (`click_element`, `type_text`, `wait_for_condition`, `manage_tabs`, ...).

</Tab>

<Tab title="driver and sb" icon="globe">

`driver` and `sb` use WebDriver, so they also work with Edge and
Firefox. `sb` has a tool for nearly every `BaseCase` method (`click_link`,
`drag_and_drop`, `get_mfa_code`, `download_file`, ...). They need a
chromedriver that matches your Chrome; a mismatch is reported as a readable
error when `start_browser` runs.

</Tab>

</Tabs>

## How tools behave

- **Errors are results.** A failed action (missing element, timeout, bad
  argument) is returned to the model as an error result it can read and act on,
  not as a protocol failure. See [When a call fails](#when-a-call-fails).
- **Waits are bounded.** Every `timeout` or pause is capped at one hour.
- **Files stay in one folder.** Screenshots, saved pages, PDFs, downloads and
  cookie files are written under `SB_MCP_OUTPUT_DIR` (default `./mcp_output`).
  Absolute paths and `..` are refused. Cookie files go to `saved_cookies/` there.
- **Annotations are honest.** Each tool declares whether it reads, navigates or
  overwrites files, so a client can decide what to confirm. A tool whose actions
  differ in kind (for example `manage_cookies`) declares only its title.
- **Defaults.** `start_browser` runs headless on Linux and headed elsewhere unless
  you set `headless`.

Where the files go, under `SB_MCP_OUTPUT_DIR`:

<LayoutTree>
  <LayoutDir name="mcp_output" open note="SB_MCP_OUTPUT_DIR, default ./mcp_output; screenshots, saved pages, PDFs and downloads are written here">
    <LayoutDir name="saved_cookies" note="cookie files" />
    <LayoutDir name="debug_bundles" note="capture_debug_bundle writes one folder here">
      <LayoutDir name="<name>" open note="a folder labelled by the name argument">
        <LayoutFile name="screenshot.png" />
        <LayoutFile name="page.html" note="values of password and hidden inputs replaced" />
        <LayoutFile name="bundle.json" note="URL, title, viewport, user agent, console messages, failed requests, cookie and storage names, the last tool calls" />
      </LayoutDir>
    </LayoutDir>
    <LayoutDir name="exported_tests">
      <LayoutFile name="<test_name>.rs" highlight note="export_session_as_test writes the session as a Rust tokio test that uses BaseCase" />
    </LayoutDir>
  </LayoutDir>
</LayoutTree>

## When a call fails

Every server, `driver` included, answers a failed call with a report: a stable
`kind` (the error's category, or `invalid_argument`, `no_browser`, `refused`), a
one-line summary, the call that was tried, the hint (likely cause and fix), and
up to three next steps that name tools this server has. For a failure the page
can explain (an element, a wait, an assertion, a navigation or a script) the
server first reads the page: URL, title, ready state, recent JavaScript errors,
and how many elements the selector matched and how many were visible. Reading it
takes at most two seconds; a page that does not answer is reported as not read,
and an argument error never touches the page.

<ResponseExample>

```text Failure report
click_element failed: element not found: #buy (strategy: css selector)
kind: element_not_found
tried: click_element {"selector":"#buy","timeout":0.5}
page: https://shop.test/cart (title "Cart", ready state complete)
selector "#buy": 0 matching, 0 visible
recent JavaScript errors: Uncaught Error: cart is undefined
hint: Nothing on the page matched the css selector `#buy` in time. The page may
still be loading, the element may be inside an iframe or a shadow root, ...
next steps:
  1. diagnose_selector {"selector": "#buy"}: says whether the selector is invalid, ...
  2. wait_for_condition {"selector": "#buy", "state": "visible", "timeout": 15}: ...
  3. suggest_selectors {"text": "<the text on the element>"}: ranked selectors, ...
```

</ResponseExample>

The same fields come as structured content for clients that read JSON. A
mistyped choice or tool name is answered with the nearest valid one (`"bakc"`:
`did you mean "back"?`).

## Developer tools

`cdp` and `sb` add nine tools that help a model write tests, find out why
something fails and check a page. They read the page with one self-contained
script per call, which leaves nothing behind (except the console recorder, on
request) and never sends `Runtime.enable`.

### `inspect_page`

An outline of the current page: URL, title, ready state, viewport, headings,
forms, iframes, shadow-DOM hosts, counts, and every visible control with its
role, accessible name, a selector that finds it, and whether it is enabled or off
screen.

<ParamField body="scope" type="string">

A selector to look inside.

</ParamField>

<ParamField body="limit" type="integer" default="50">

Controls to list, 1 to 500.

</ParamField>

<ParamField body="include_hidden" type="boolean" default="false">

Also list controls that are not visible.

</ParamField>

The call and its result:

<RequestExample>

```json Request
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "inspect_page",
    "arguments": { "limit": 20 }
  }
}
```

</RequestExample>

<ResponseExample>

```json Result
{
  "title": "Shop", "ready_state": "complete",
  "interactive": [ { "role": "button", "name": "Place order", "tag": "button",
                     "selector": "[data-testid=\"place-order\"]", "visible": true, "enabled": true } ],
  "counts": { "interactive": 12, "iframes": 1, "shadow_hosts": 1 },
  "truncated": false
}
```

</ResponseExample>

When to use it: first, on a page you have not seen, instead of reading the HTML.

### `suggest_selectors`

Finds the element meant by `text`, `role` or a rough `selector` (at least one),
and returns ranked selectors for it: test attributes (`data-testid` and
similar), id, name, aria-label, placeholder, link text, a text XPath, classes and
a short CSS path. Each is checked against the page: `matches`, `unique`,
`finds_element`, and `stable` (false for ids and classes that look generated).

<ParamField body="text" type="string">

Text the element shows or is labelled with.

</ParamField>

<ParamField body="role" type="string">

ARIA role such as `button`, `link`, `textbox` or `checkbox`.

</ParamField>

<ParamField body="selector" type="string">

A rough selector that finds it.

</ParamField>

<ParamField body="limit" type="integer" default="5">

The number of selectors per element, 1 to 20.

</ParamField>

```json
{ "elements": [ { "element": "button#submit \"Place order\"", "role": "button",
    "selectors": [ { "selector": "[data-testid=\"place-order\"]", "strategy": "test attribute",
                     "matches": 1, "unique": true, "finds_element": true, "stable": true } ] } ] }
```

When to use it: before writing a test step, or when a selector breaks.

### `diagnose_selector`

Explains why `selector` finds nothing or cannot be used: invalid syntax (with
the browser's message), no match, matches that are hidden (`display`,
`visibility`, `opacity`, no size), off screen, disabled or covered by another
element (which one), matches inside a same-origin iframe or an open shadow root,
and near misses (a similar id or class, the same text in different case or
spacing, a `data-test*` variant). It ends with a one-line `verdict`, `advice`
about the selector's text (`lnk=` for `link=`, XPath without `//`,
`:contains()`, which is not CSS) and up to three `next_steps`.

<ParamField body="selector" type="string" required>

CSS selector, XPath, or `link=text`.

</ParamField>

<RequestExample>

```json Request
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "diagnose_selector",
    "arguments": { "selector": "#covered" }
  }
}
```

</RequestExample>

<ResponseExample>

```json Result
{ "selector": "#covered", "matches": 1, "visible_matches": 1,
  "verdict": "The element is covered by div#overlay.cookie-banner \"Cookies\", which would get the click.",
  "next_steps": [ "Close or remove what covers it first (\"#overlay\")." ] }
```

</ResponseExample>

When to use it: right after a click, type or wait failed on a selector.

### `get_console_messages`

The page's console messages and uncaught errors. Recording starts when you pass
`start: true`: it replaces the page's console functions, which a page script can
notice, and lasts until the page navigates. The Pure CDP engine also reports, as
`page_errors`, the uncaught errors since the page loaded, with no recording.

<ParamField body="level" type="string" default="all">

The least serious level to include: `all`, `error`, `warning`, `info` or `log`.

</ParamField>

<ParamField body="start" type="boolean" default="false">

Start recording console messages in this page.

</ParamField>

<ParamField body="clear" type="boolean" default="false">

Forget the messages after reading them.

</ParamField>

<ParamField body="limit" type="integer" default="100">

Most messages to return, 1 to 500.

</ParamField>

When to use it: when a page behaves oddly, or a failure report lists script
errors.

### `get_failed_requests`

Requests that failed. Always: responses of status 400 and up from the browser's
performance timeline (it knows the status of same-origin and CORS-enabled
resources only). On the Pure CDP engine, `watch: true` also records every
failed, blocked or refused request of the active tab from then on (through the
`Network` domain), with its error, such as `net::ERR_NAME_NOT_RESOLVED`.

<ParamField body="watch" type="boolean" default="false">

Start recording failed requests (Pure CDP engine).

</ParamField>

<ParamField body="clear" type="boolean" default="false">

Forget the recorded failures after reading them.

</ParamField>

<ParamField body="limit" type="integer" default="100">

Most requests to return, 1 to 500.

</ParamField>

When to use it: when a page is blank or half-loaded, or a navigation failed.

### `audit_page`

Accessibility and HTML checks on the page as it is now, after its scripts ran,
with the rules of `seleniumbase_rs::api::html_inspector::HtmlInspector`: images
without alt text, controls without a label or name, duplicate ids, empty links
and buttons, heading order, missing `lang` or title, and more. Each finding has
its rule, severity, message and a selector. Static checks only: no colour
contrast or focus order.

<ParamField body="severity" type="string" default="all">

`all` (warnings too) or `error` (errors only).

</ParamField>

<ParamField body="limit" type="integer" default="50">

Most findings to return, 1 to 500.

</ParamField>

### `capture_debug_bundle`

Writes a folder under `debug_bundles/` in the output directory with
`screenshot.png`, `page.html` (values of password and hidden inputs replaced)
and `bundle.json`: URL, title, viewport, user agent, console messages, failed
requests, cookie and storage *names* (never values), and the last tool calls.
In those calls the arguments that can hold a secret (`text`, `value`,
`password`, `secret`, `token`, `totp_key`, `expression`, `script` and `proxy`)
are replaced by their length, and so is the result of every call that
succeeded: it reads `[redacted, N characters]`, so what `get_cookies`,
`manage_cookies` or a storage tool returned is not saved with the report. A call
that failed keeps its error text, since that says what went wrong.

<ParamField body="name" type="string">

A label for the folder; only letters, digits, `-` and `_` are kept.

</ParamField>

<ParamField body="calls" type="integer" default="20">

How many recent tool calls to include, 1 to 200.

</ParamField>

When to use it: to attach to a bug report, or to hand a failure to a person.

### `export_session_as_test`

Writes the successful steps of the browser session since the last
`start_browser` as a Rust `#[tokio::test]` that uses `BaseCase`, to
`exported_tests/<test_name>.rs` in the output directory. Reads are left out;
waits and assertions are kept; a change with no `BaseCase` equivalent (a script,
a fixed pause, a click by position) becomes a `// skipped:` comment in its
place and is listed in the result.

<ParamField body="test_name" type="string" default="exported_session">

Name of the test function and file, such as `checkout_works`. It must be a Rust
identifier.

</ParamField>

```rust,ignore
#[tokio::test]
async fn checkout_works() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://shop.test").await?;
    sb.type_text("#q", "shoes").await?;
    sb.click("#go").await?;
    sb.assert_text("Shoes", "h1").await?;
    sb.quit().await?;
    Ok(())
}
```

When to use it: once a session does what a test should check.

### `diagnose_environment`

Needs no browser. Reports the platform, whether Chrome and chromedriver are
found, their major versions and whether they fit (a mismatch names the fix:
`sbrs install`, then `sbrs doctor`), whether the output directory exists and is
writable and how much disk is free there (from `df`; `null` where there is
none), `CHROMEDRIVER_PATH`, every `SB_*` setting in the environment (a value
whose name says it is secret, such as `SB_API_TOKEN`, and the user and password
in a proxy URL are redacted), whether a session is running, and a list of
`problems` with the fix for each.

When to use it: before the first `start_browser`, or when one fails.

### `get_devtools_endpoint`

<Badge color="gray">cdp server only</Badge>

Only in the `cdp` server. Reports the DevTools endpoint of the running browser
(`http_url` such as `http://127.0.0.1:53017`, `websocket_url`, `version_url`)
and `mcp_configs` that attach Google's `chrome-devtools-mcp` (`--browserUrl`) or
Microsoft's Playwright MCP (`--cdp-endpoint`) to the same browser, plus the
`claude mcp add` commands for Claude Code. Without a browser it answers
`running: false`.

When to use it: to hand the session to a second DevTools client, for example
the performance traces of `chrome-devtools-mcp`, while this server keeps
driving the page. [DevTools MCP servers](/tutorials/devtools_mcp) covers those servers
and `sbrs devtools`, which starts such a browser from the command line.

## Prompts to try

Once the server is registered, these prompts exercise the developer tools.

<Prompt description="Find a selector that survives a redesign" icon="crosshairs" actions={["copy"]}>

Open https://example.com with the seleniumbase-cdp server, call inspect_page, then use suggest_selectors to give me a stable selector for the main link. Prefer test attributes and tell me which selectors are marked unstable.

</Prompt>

<Prompt description="Debug a click that does nothing" icon="bug" actions={["copy"]}>

The click on the checkout button failed. Run diagnose_selector on it, then get_console_messages with start set to true, repeat the click, and report any covering element, script error or failed request from get_failed_requests.

</Prompt>

<Prompt description="Turn a session into a test" icon="file-export" actions={["copy"]}>

Walk through the sign-in flow on the site I name, check it with audit_page, and when it works call export_session_as_test with the test name sign_in_works. Show me the file it wrote and list the steps it skipped.

</Prompt>

<Visibility for="agents">

Summary for AI exports: `sbrs-mcp --server cdp|driver|sb` serves MCP over stdio. Call `start_browser` once, use the tools, then `close_browser`. Files are written under `SB_MCP_OUTPUT_DIR`. A failed call returns an error result with a `kind`, the call tried and next steps, not a protocol error.

</Visibility>

## Security

<Danger>

These tools let a model drive a real browser as you. Only connect clients you
trust, and prefer a throwaway browser profile.

</Danger>

- `run_javascript`, `execute_script` and `evaluate` run arbitrary script in the
  current page.
- Cookies and web storage can hold logins. `manage_cookies`, `get_cookies` and
  the storage tools return them to the model.
- `choose_file` uploads a file from this machine; `download_file` fetches a URL
  from this machine's network.
- `patch_chromedriver` (in `sb`) modifies a binary on disk.
- `get_devtools_endpoint` (in `cdp`) gives out the address of the browser's
  DevTools endpoint. It listens on `127.0.0.1` only, but any program on this
  machine that knows it can drive the browser.
- `capture_debug_bundle` saves a screenshot and the page's markup, which can show
  personal data; values of password and hidden inputs, cookies and storage, and
  the results of earlier tool calls are left out. `export_session_as_test` writes the text the model typed into the
  test, since the test must type it again.

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
tool" in the [developer guide](/DEVELOPER_GUIDE).

## Next steps

<Columns cols={2}>

<Card color="#1a85d4" title="DevTools MCP servers" icon="screwdriver-wrench" href="/tutorials/devtools_mcp">

Attach `chrome-devtools-mcp` or Playwright MCP to the same browser.

</Card>

<Card color="#199f43" title="Choosing plugins" icon="plug" href="/plugins/selecting#sbrs-mcp">

Run built-in plugins in every browser the server starts.

</Card>

<Card color="#d47628" title="Vibe coding" icon="wand-magic-sparkles" href="/tutorials/vibe_coding">

Describe a test, let an agent drive the browser, review the Rust it hands you.

</Card>

<Card color="#a631be" title="Pure CDP" icon="bolt" href="/tutorials/pure_cdp">

The engine behind the `cdp` server.

</Card>

</Columns>

[Model Context Protocol]: https://modelcontextprotocol.io
