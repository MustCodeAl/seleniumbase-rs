---
title: "MCP servers"
---

{/* layout-tree-component:start */}

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

{/* layout-tree-component:end */}

`sbrs-mcp` exposes a browser to any [Model Context Protocol] client
(Claude Desktop, Claude Code, ...) over stdio. It serves one of three browser
toolsets, the same tools as the Python `seleniumbase-mcp` project with tools of
our own for developing, debugging and testing, and a fourth server, `dev`, that
starts no browser and helps with the library, the `sbrs` command line and
finding out what is wrong:

| `--server` | Engine | Tools |
| --- | --- | --- |
| `cdp` | Pure CDP (no WebDriver) | 24 + 9 developer tools + `get_devtools_endpoint` |
| `driver` | WebDriver, `Driver()` toolset | 26 + 8 debugging tools |
| `sb` (default) | WebDriver, `SB()` toolset, plus this crate's stealth tools | 88 + 7 stealth + 9 developer tools |
| `dev` | None: no browser | 12 tools for the library, the command line and diagnosing problems |

<Info>

The three browser servers keep one browser session for the whole conversation:
call `start_browser` once, use the other tools, then `close_browser`. The `dev`
server has no session.

</Info>

<Tip>

Every tool costs context in each session that loads its server, and some clients
stop at about 40 tools. Install `dev` plus one browser server (`cdp` is the
lightest) rather than several browser servers. See [Which server for which
use](#which-server-for-which-use).

</Tip>

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

```bash Terminal
sbrs-mcp install claude-code
```

One command gives the assistant both the browser (`seleniumbase`) and the
development tools (`seleniumbase-dev`). Name several clients to register with all of
them. [Install into a client](#install-into-a-client) has the clients, the flags and
what is changed on disk; [Register by hand](#register-by-hand) shows the entry to
write yourself.

</Step>

</Steps>

<Note>

Logs go to stderr; stdout carries only the protocol.

</Note>

## Install into a client

```bash Terminal
sbrs-mcp install claude-code cursor   # register with these clients
sbrs-mcp install --print vscode       # show what would be written, change nothing
sbrs-mcp clients                      # where each client keeps its config, what is registered
sbrs-mcp uninstall cursor             # remove the registrations
```

`install` registers two servers with each client, so that one command gives an
assistant the browser and the development tools:

| Entry | Starts | For |
| --- | --- | --- |
| `seleniumbase` | `sbrs-mcp --server cdp` | Driving a browser. `--server driver` or `--server sb` picks another browser toolset. |
| `seleniumbase-dev` | `sbrs-mcp --server dev` | Development, library, CLI and diagnostics tools. It starts no browser. |

Both entries start the binary by its **absolute path** (the one you ran, with links
resolved), because desktop apps do not have your shell `PATH`, and both get the same
`SB_MCP_OUTPUT_DIR`. Running `sbrs-mcp` with no command still starts the server, so a
client that starts it with only flags keeps working.

### Flags

| Flag | Default | Meaning |
| --- | --- | --- |
| `--server` | `cdp` | The browser toolset: `cdp` (the lightest), `driver` or `sb`. |
| `--name NAME` | `seleniumbase` | The name of the browser entry. The development entry is `NAME-dev`. |
| `--only browser` or `--only dev` | both | Register, or on `uninstall` remove, just one of the two entries. |
| `--output-dir DIR` | `sbrs-mcp` in your data folder | Where the tools write their files. It becomes `SB_MCP_OUTPUT_DIR` and is created. |
| `--plugin NAME` and `--plugin-opt NAME.KEY=VALUE` | none | Added to the browser entry's arguments, as on the server itself, and checked now against the plugins this build has. The development entry starts no browser and gets none. |
| `--scope user` or `--scope project` | `user` | Your own config, or the config file in the current folder. A client without a project file refuses `project` with exit status 2. |
| `--print` | off | Show exactly what would be written or run, and write and run nothing. |
| `--force` | off | Replace an entry of the same name that starts something different. |

### Clients

| Client | User config | `--scope project` | How it is changed |
| --- | --- | --- | --- |
| `claude-code` | `~/.claude.json` | `.mcp.json` | `claude mcp add-json` and `claude mcp remove`, never by writing the file, which Claude Code writes itself. Without `claude` on `PATH` the commands are printed and the exit status is 2. |
| `claude-desktop` | macOS `~/Library/Application Support/Claude/claude_desktop_config.json`, Windows `%APPDATA%\Claude\claude_desktop_config.json`, not Linux | none | JSON, key `mcpServers`. |
| `codex` | `~/.codex/config.toml` (in `$CODEX_HOME` when that is set) | none | `codex mcp add` and `codex mcp remove` when the installed `codex` has them, else the tables `[mcp_servers.NAME]` and `[mcp_servers.NAME.env]` of `config.toml`, edited so that its comments and other tables stay. |
| `cursor` | `~/.cursor/mcp.json` | `.cursor/mcp.json` | JSON, key `mcpServers`, with `"type": "stdio"`. |
| `gemini-cli` | `~/.gemini/settings.json` | `.gemini/settings.json` | JSON, key `mcpServers`. |
| `opencode` | `~/.config/opencode/opencode.json`, or the `opencode.jsonc` that is there instead | `opencode.json` | JSON, key `mcp`, with `"type": "local"`, the command and its arguments as one array, and `environment`. |
| `vscode` | macOS `~/Library/Application Support/Code/User/mcp.json`, Windows `%APPDATA%\Code\User\mcp.json`, Linux `~/.config/Code/User/mcp.json` | `.vscode/mcp.json` | JSON, key `servers`, with `"type": "stdio"`. Only the default profile's file. |
| `zed` | macOS and Linux `~/.config/zed/settings.json` (`$XDG_CONFIG_HOME/zed` on Linux), Windows `%APPDATA%\Zed\settings.json` | none | JSON, key `context_servers`. |

`sbrs-mcp clients` prints the same for your machine: the paths, whether each config
and each client's program (`claude`, `codex`, `code`) was found, and which of the two
entries are registered. The paths and formats were checked against the clients' own
documentation:
[Claude Code](https://code.claude.com/docs/en/mcp),
[Claude Desktop](https://modelcontextprotocol.io/quickstart/user),
[Codex](https://developers.openai.com/codex/mcp),
[Cursor](https://cursor.com/docs/context/mcp),
[Gemini CLI](https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md),
[opencode](https://opencode.ai/docs/mcp-servers/),
[VS Code](https://code.visualstudio.com/docs/copilot/customization/mcp-servers) and
[Zed](https://zed.dev/docs/ai/mcp).
Windsurf and LM Studio are not in the list: their documentation does not give the
path (LM Studio) or no longer gives the one that was used (Windsurf).

<Warning>

The Claude app rewrites `claude_desktop_config.json` while it runs, and an entry
written while it ran has been seen to disappear. Quit Claude before you run
`sbrs-mcp install claude-desktop`, and open it again after. The command prints this
reminder every time.

</Warning>

### What it does to your files

- **Only the entries change.** For a JSON file, every other key, entry, blank line
  and inline array keeps its exact text; the new entries are written in the layout
  of the ones around them. In `config.toml` the comments and the other tables stay.
- **A backup comes first.** A file that exists is copied to `FILE.bak-sbrs-mcp`
  before it changes. The new content goes to a temporary file in the same folder, is
  flushed, takes the old permissions and is renamed over the original, and missing
  folders are created. A file that needs no change is not touched.
- **An entry that is there already** and starts the same process is left alone
  (exit status 0, `unchanged`). A different one of the same name is left alone and
  reported (exit status 2) unless you pass `--force`. The two entries are decided and
  reported separately.
- **A file with comments is not rewritten.** Zed's `settings.json` and VS Code's
  `mcp.json` often have them, and a rewrite would lose them. The same goes for a
  file that is not valid JSON. The snippet to add by hand is printed and the exit
  status is 2; for VS Code the `code --add-mcp` command is printed too.
- **Exit status.** `0` when every entry is added, replaced or unchanged; `2` for an
  unknown client, a scope the client does not have, a file or program that cannot be
  used, or a different entry left alone; `1` when a write or a client's own program
  fails.

### Register by hand

To write the entry yourself, for example in `.mcp.json`, give the absolute path of
the binary:

```json .mcp.json
{
  "mcpServers": {
    "seleniumbase": {
      "command": "/absolute/path/to/sbrs-mcp",
      "args": ["--server", "cdp"],
      "env": { "SB_MCP_OUTPUT_DIR": "/tmp/seleniumbase-output" }
    },
    "seleniumbase-dev": {
      "command": "/absolute/path/to/sbrs-mcp",
      "args": ["--server", "dev"],
      "env": { "SB_MCP_OUTPUT_DIR": "/tmp/seleniumbase-output" }
    }
  }
}
```

Use `["--server", "driver"]` or `["--server", "sb"]` for the other browser toolsets.

To run a built-in plugin in every browser the server starts, add
`"--plugin", "shield"` to the browser entry's `args` (and
`"--plugin-opt", "shield.level=\"strict\""` for an option, a TOML value). The
operator chooses the plugins; no tool argument adds one, and a mistake in them stops
the server before it listens. See
[Choosing plugins](/plugins/selecting#sbrs-mcp).

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

<Tab title="dev" icon="screwdriver-wrench">

`dev` starts no browser. It is for the work around the browser: finding the right
method, writing Rust tests, using the `sbrs` command line, and finding out what
is wrong. Load it next to one of the others.

</Tab>

</Tabs>

### Which server for which use

| You want to | Load |
| --- | --- |
| Drive Chrome without a driver, hard to detect | `cdp` |
| Drive Edge or Firefox, or port a Python `Driver()` script | `driver` |
| A tool for nearly every `BaseCase` method, and the stealth tools | `sb` |
| Find a method, write or convert a test, read `sbrs --help`, check settings and errors | `dev` |
| The usual setup for an agent that writes and debugs tests | `dev` plus one of `cdp`, `driver` or `sb` |

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

## Page tools and refs

The `cdp`, `driver` and `sb` servers have five tools that read a page the way a
model needs it (the `dev` server has no browser and none of them). They only look:
they leave nothing behind in the page, each caps what it returns and says when it
cut it, and all five declare an `outputSchema`, return `structuredContent` next to
their text and mark the result as untrusted page content.

| Tool | What it returns | Arguments (default) |
| --- | --- | --- |
| `page_snapshot` | The page as a compact accessibility tree: landmarks, headings, links, buttons and form controls with role, name, state and value. Interactive nodes carry a ref such as `[ref=e5]`. Passwords and card numbers are never printed. | `scope`; `mode` (`accessibility`, or `outline` for the tag tree; `accessibility`); `depth` (1 to 30; 12); `max_nodes` (1 to 2000; 400); `include_hidden` (false) |
| `read_page_text` | The readable text as Markdown: the main content, headings as `#`, links as `[text](url)`, lists and tables. Scripts, styles and hidden nodes are skipped. | `scope`; `format` (`markdown` or `text`; `markdown`); `main_only` (true); `max_chars` (1000 to 200000; 20000) |
| `extract_links` | The visible links with text, absolute URL, `rel`, `target` and the landmark they sit in. Duplicates are collapsed with a count, `javascript:` links are flagged, and the first 100 links carry a ref. | `scope`; `same_origin` (false); `contains`; `limit` (1 to 2000; 200) |
| `extract_tables` | The visible tables (HTML tables and `role=table` or `grid`) as caption, headers and rows, with `colspan` and `rowspan` expanded. Each table carries a selector and a ref. | `scope`; `limit` (1 to 100; 20); `max_rows` (1 to 5000; 200) |
| `extract_forms` | The forms as data: action, method, each field with name, type, label, required flag, value, checked state and options, and the submit button. Fields and forms carry refs; password and card values are left out. | `scope` |

`scope` limits a tool to the first element that a selector (or a ref) matches.

### Refs

`page_snapshot` gives each interactive node a short ref (`e1`, `e2`, ...). Any
argument that holds a selector (`selector`, `scope` and every `*_selector`) of any
tool may then be written `ref=e5` instead of a CSS selector:

```json
{ "selector": "ref=e5" }
```

Before the call runs, the server checks in the page that the element still has the
role and name it had in the snapshot, and only then swaps in the selector it stored.
When something else has taken its place, the call fails and says what is there now,
so a click never lands on the wrong element:

```text
ref e5 is stale: the element is now button "Save", it was textbox "Email"; call page_snapshot again
```

Refs belong to the last snapshot, so take a new one after the page changes. They
live on the server, not in the page, where a script could look for them. A ref for
an element inside an open shadow root works on the `driver` and `sb` servers only;
on `cdp` the call says so.

The text of the last `page_snapshot` is also the resource `sbrs://page/snapshot`,
and the last text a page tool read is `sbrs://page/text`, while a browser runs. The
`write_test` and `extract_data` prompts use these tools.

## Fingerprint tools

Three tools check what the running browser shows a page. The `cdp`, `driver` and
`sb` servers have them; they mark their output untrusted and declare output
schemas. (The `check_fingerprint` tool of the `dev` server is another thing: it
validates a fingerprint you hand it and needs no browser.)

| Tool | What it does | Arguments |
| --- | --- | --- |
| `capture_fingerprint` | Reads what the browser shows a page, as a fingerprint: the navigator, Client Hints, screen, time zone, WebGL, media devices and more, plus readings no setting holds (the webdriver flag, plugins, canvas and audio hashes, headless markers). It is the reading of [`sbrs fingerprint capture`](/help/cli_reference#sbrs-fingerprint-capture), with progress. An https page or `about:blank` gives the whole fingerprint; an http page has no Client Hints or media devices. | `url` (opened first); `save_to` (a file name in the output folder, saved as JSON and private to its owner) |
| `compare_fingerprint` | Asks whether the browser presents a fingerprint. It lists the settings the page contradicts, the ones it matches, and the ones no page can read (`accept_languages`, `geolocation`, `fonts`, `proxy`, `webrtc`), which never count as matches. The text starts with `presents` and `verified`, each yes or no. | exactly one of `fingerprint` (a JSON object, or a string holding one), `path` (a file in the output folder or the working directory) or `preset` |
| `explain_fingerprint` | Says what a detection script would find: the signs of automation (the webdriver flag, headless markers, a software renderer, ...) with how loudly a detector reacts and what to change, whether the readings agree with each other, and the values a script reads first. | `url` (opened first) |

```json
{ "preset": "macos" }
```

The presets are `windows`, `edge`, `macos`, `linux`, `android` and `ios`. A proxy
password in a fingerprint that a tool returns is shown as `***`; a file written with
`save_to` keeps the real one.

Related tools and prompts:

- `start_browser` takes `fingerprint`, `fingerprint_path` or `fingerprint_preset`
  and applies it as the browser starts. On `driver` and `sb` this is for Chrome,
  Chromium and Edge, not Firefox.
- The `sb` server also has `diff_fingerprints` (`a` is the expected fingerprint and
  `b` the one found, each a preset name, a file path or JSON text) and
  `patch_fingerprint` (an RFC 7386 JSON merge patch for a fingerprint, path or
  preset, where `null` removes a setting; `save_to` saves a valid result). Neither
  needs a browser.
- The `check_fingerprint` prompt takes `file_or_preset` and walks a model through
  `start_browser`, `capture_fingerprint`, `compare_fingerprint` and
  `explain_fingerprint`.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth) for what the
settings mean.


## Developer tools

`cdp` and `sb` add nine tools that help a model write tests, find out why
something fails and check a page. `driver` has eight of them
(`get_failed_requests` and `audit_page` stay with `cdp` and `sb`, and it adds
`get_page_info`), as the same definitions, so a model that drives with `Driver()`
can inspect the page and the session too. They read the page with one self-contained
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
use seleniumbase_rs::{tokio, BaseCase, BrowserConfig};

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

## Beyond the browser: development, library and CLI tools

The `dev` server (`sbrs-mcp --server dev`) starts no browser, so it has no
`start_browser`. Its twelve tools answer from the library itself: the API index,
the Python parity manifest, the `sbrs` CLI reference and the common-problems page
are embedded in the binary, so the answers match the version you run. Register it
next to a browser server:

```json
{
  "mcpServers": {
    "seleniumbase-cdp": { "command": "sbrs-mcp", "args": ["--server", "cdp"] },
    "seleniumbase-dev": { "command": "sbrs-mcp", "args": ["--server", "dev"] }
  }
}
```

| Job | Tools |
| --- | --- |
| Find the right method | `find_api`, `parity_lookup` |
| Write Rust tests | `python_to_rust`, `scaffold` |
| Use the command line | `cli_help`, `list_plugins`, `plugin_info`, `install_driver` |
| Find out what is wrong | `explain_error`, `check_config`, `check_fingerprint`, `diagnose_environment` |

Only `install_driver` changes anything. Every other tool is annotated read-only,
and none of them touches a web page.

### `find_api`

Searches the public API of this version: every public method of `BaseCase` (the
WebDriver engine) and of the pure CDP engine's `Browser`, `Page`, `Locator` and
`BrowserContext`. An exact name ranks first, then names that contain the query,
then methods whose name, type or documentation mention every word. Each match has
its type, its full signature as written in the source, the first paragraph of its
documentation and whether it is `async`.

| Argument | Meaning |
| --- | --- |
| `query` | A name or words: `screenshot`, `wait for text`, or `Page::click` to search one type |
| `engine` | `basecase`, `cdp` or `any` (default) |
| `limit` | 1 to 50, default 10 |

```json
{ "query": "wait for text", "limit": 2 }
```

```json
{
  "query": "wait for text",
  "found": 21,
  "returned": 2,
  "matches": [
    {
      "name": "wait_for_text",
      "owner": "BaseCase",
      "engine": "basecase",
      "signature": "pub async fn wait_for_text(&self, text: &str, css: &str, timeout_secs: u64) -> Result<(), SeleniumBaseError>",
      "doc": "Waits up to `timeout_secs` for the element `css` to be displayed and contain `text`. ...",
      "is_async": true,
      "file": "src/api/base_case.rs"
    }
  ],
  "note": "19 more matches were cut by limit; narrow the query or raise the limit."
}
```

The index is generated from the source by a test and committed as
`src/mcp/dev/api_index.json`; a second test fails when it is out of date, and
`SB_UPDATE_API_INDEX=1 cargo test --features mcp-server api_index` rewrites it.

When to use it: before writing a call from memory, or to see what the pure CDP
`Page` can do that `BaseCase` calls differently.

### `parity_lookup`

Looks up a name of Python SeleniumBase 4.55.2 (a `BaseCase` or `sb.cdp` method, a
`Driver()` method, a command, an option or an upstream MCP tool) in
`parity/api.toml`. `self.`, `sb.`, `sb.cdp.` and a call's arguments are ignored
(`sb.cdp.click("#a")` looks up `click` in the CDP methods). The answer gives, for
each place the name is listed, its status (`implemented`, `composed`,
`not-applicable` or `planned`), the Rust path, and any note or difference in
behavior. With no exact match it lists close names and the nearest spelling.

```json
{ "python_name": "self.assert_text" }
```

```json
{
  "found": true,
  "matches": [
    {
      "python_name": "assert_text",
      "section": "basecase",
      "status": "implemented",
      "rust": "BaseCase::assert_text",
      "divergence": "Waits up to the default timeout (set_timeout, 10 seconds unless changed), where Python uses SMALL_TIMEOUT (6 seconds); ..."
    }
  ]
}
```

When to use it: porting a Python script, or when a Python name has no obvious
Rust twin. [API parity](/parity) lists the same data as a table.

### `python_to_rust`

Converts a Python SeleniumBase or Selenium test to a Rust test with the importer
of `sbrs import-python`. The Python is read, never run, and at most 256 KiB are
taken. The answer is text: which API family was detected, the diagnostics with
their line numbers, then the Rust. A statement the importer cannot translate stays
in the Rust as a `// TODO` comment.

| Argument | Meaning |
| --- | --- |
| `code` | The Python source |
| `source` | `auto` (default), `seleniumbase` or `selenium` |
| `test_name` | Name of the Rust function, default `imported_python_test` |

```text
Detected API: seleniumbase. The conversion is complete: no statement was refused.
Diagnostics (1):
- line 6, warning: SeleniumBase method 'do_magic' needs manual migration

=== Rust test example ===
use seleniumbase_rs::{run_browser_test, tokio, BrowserConfig};

#[tokio::test]
async fn example() -> seleniumbase_rs::Result<()> {
    run_browser_test(BrowserConfig::default(), async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_text("Example Domain", "h1").await?;
        // TODO(source line 6): SeleniumBase method 'do_magic' needs manual migration
        Ok(())
    }).await
}
```

When to use it: for a Python file to migrate. Follow it with `parity_lookup` for
each `TODO`.

### `scaffold`

Shows the files `sbrs mkfile` (`kind` `test`) and `sbrs mkdir` (`kind` `suite`)
would create, as text with each file's path and content, from the same templates
and the same name checks. It writes nothing: create the files with your own tools.
`name` is a relative name of letters, digits, `_`, `-` and `.`; `url` (tests only)
is the page the test opens; `basic` leaves out the example assertions or example
tests.

```json
{ "kind": "test", "name": "tests/login", "url": "https://shop.test/login" }
```

```text
Scaffold for kind test, name tests/login. Nothing was written; create these 1 file(s) yourself.

=== tests/login.rs ===
//! Browser test generated by `sbrs mkfile`.
...
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn login(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://shop.test/login").await?;
        ...
        Ok(())
    })
    .await
}

Run it with `cargo test --test login` when the file is in your crate's tests/ folder. ...
```

`sbrs mkchart` and `sbrs mkpres` write test files of their own (a chart slide and
a slide deck), which `scaffold` does not offer.

### `cli_help`

The `sbrs` command line, read from the usage spec of this version, the file the
parser, the shell completions and the [CLI reference](/help/cli_reference) are
generated from, so the answer cannot disagree with `sbrs --help`.
With no `command`: the global flags, one line each, and every command with a
one-line description. With a command or an alias (`wait-for-text`, `obfuscate`,
`plugin list`): its usage, arguments, flags with their defaults and choices, and
aliases; a group such as `plugin` shows its subcommands too. `global` gives the
global flags in full. An unknown command is answered with the nearest name.

```json
{ "command": "wait-for-text" }
```

```text
sbrs wait-for-text
Wait until an element contains the expected text.

Usage: sbrs wait-for-text --css <CSS> --text <TEXT> [--timeout <TIMEOUT>]

Flags:
- --css <CSS>: CSS selector for the target element [required]
- --text <TEXT>: Expected text substring to wait for [required]
- --timeout <TIMEOUT>: Maximum wait time in seconds [default: 10]
- -h, --help: Print help
```

### `list_plugins` and `plugin_info`

`list_plugins` is `sbrs plugin list`: the built-in browser plugins compiled into
this build, their category, whether the operator turned them on (with `--plugin`,
`SB_PLUGINS` or the settings file), and a summary. `plugin_info` (argument `name`)
is `sbrs plugin info NAME`: the version, the capabilities the plugin asks for and
how risky each is, what it needs from the browser, what it conflicts with and its
options. Neither turns anything on: the operator chooses the plugins.

A build without the feature `plugins-builtin` says so and names it, instead of
listing nothing:

```text
No plugins: this build has no built-in plugin; rebuild with --features plugins-builtin (or one of plugin-shield, plugin-block-resources, plugin-crx, plugin-fetcher, plugin-extractor, plugin-batcher, plugin-crawler, plugin-browser).
```

### `install_driver`

Downloads a driver as `sbrs install` does, through the same code, into the
`downloaded_drivers` folder of the folder the server runs in (where the library
looks first). It is the only tool of this server that changes anything: it
reaches the network, and it replaces a driver of another version already there.
Its annotations say so: not read-only, destructive, idempotent, open-world.

| Argument | Meaning |
| --- | --- |
| `browser` | `chrome` (chromedriver, default), `edge` (msedgedriver) or `firefox` (geckodriver) |
| `version` | `latest`, a major version such as `155`, or a full version; geckodriver takes `latest` or a release such as `0.36.0`. Default: the driver for the installed browser |

```json
{ "browser": "chrome" }
```

```json
{
  "driver": "chromedriver",
  "browser": "chrome",
  "requested_version": null,
  "path": "downloaded_drivers/chromedriver",
  "installed_version": "ChromeDriver 155.0.7100.4 (...)",
  "cache": "the downloaded_drivers folder of the folder the server runs in",
  "note": "Installed. diagnose_environment checks that the driver and the browser agree."
}
```

The version is what the installed driver prints for `--version`; if it cannot be
run (macOS may need to allow it first) the result says so. A failed download is an
error with the reason. Only one install runs at a time.

When to use it: after `diagnose_environment` reports that chromedriver is missing
or does not match Chrome.

### `explain_error`

Explains an error of this library or of an MCP server. Give the error `message` as
printed, even wrapped in a failure report (`click_element failed: element not
found: ...`), or just a `category`. The library's own `SeleniumBaseError` is
rebuilt from the text and asked for its `category()`, `hint()` and `exit_code()`;
the answer adds whether the error is transient, what the `sbrs` exit status means,
the next steps a browser server would suggest, and the sections of the
[common problems](/help/common_problems) page that apply. The errors a server
reports itself (`no_browser`, `invalid_argument`, `refused`, `unknown_tool`) are
explained as the server's. Text that matches nothing is reported as not
recognized, with the known categories, rather than guessed at.

```json
{ "message": "click_element failed: element not found: lnk=Home (strategy: css selector)" }
```

```json
{
  "recognized": true,
  "category": "element_not_found",
  "from": "message",
  "specific": true,
  "hint": "Nothing on the page matched the css selector `lnk=Home` in time. ... Did you mean `link=Home`?",
  "transient": false,
  "exit_code": 3,
  "exit_meaning": "an element could not be found or used, or a wait timed out",
  "next_steps_with_a_browser_server": ["diagnose_selector {\"selector\": \"lnk=Home\"}: ...", "..."],
  "docs": [{ "heading": "An element is not found or a wait times out", "text": "..." }]
}
```

With only a `category` the advice is the general one for it, and the answer says so
(`"specific": false`).

### `check_config`

Checks the configuration without starting anything. It reads the settings as
`sbrs` does (the settings file, then the `SB_*` variables) and reports every
problem instead of the first: an unreadable or malformed file, each variable that
does not parse (with its name), an unknown browser or mode, a window size with only
one side, plugins this build cannot run. It also reads the runtime variables
(`SB_LOG_LEVEL`, `SB_SHUTDOWN_TIMEOUT_SECS` and the rest), whose mistakes `sbrs`
ignores with a warning, and summarises what would be launched. The optional `path`
names a settings file inside the folder the server runs in; without it the file
`sbase_config.toml` there is used, if there is one.

Secrets are redacted (the user and password of a proxy URL), and an error never
quotes the lines of the file or the value it refused.

```json
{
  "ok": false,
  "starts_nothing": true,
  "problems": [
    {
      "source": "SB_HEADLESS",
      "category": "invalid_config",
      "message": "invalid configuration: cannot parse bool: maybe",
      "hint": "Review the configuration values ..."
    }
  ],
  "warnings": [
    {
      "message": "invalid configuration: SB_SHUTDOWN_TIMEOUT_SECS: invalid digit found in string",
      "effect": "the variable is ignored and its default is used"
    }
  ],
  "settings": { "browser": "chrome", "proxy": "[redacted]@proxy.test:8080", "...": "..." },
  "runtime": { "webdriver_url": "http://localhost:4444", "...": "..." },
  "plugins": { "enabled": [], "check": "ok", "chosen_by_the_server_operator": [] }
}
```

### `check_fingerprint`

Parses JSON as a [fingerprint](/tutorials/fingerprint_stealth) of this library, or, when
it has `parameters` or `browser_type` with `os_type`, as the profile payload of an
anti-detect browser tool, and returns what `Fingerprint::validate` finds: whether it
is coherent, the errors (a platform that contradicts the user agent) and the
warnings (a mobile agent without touch points). It is the same report as
`validate_fingerprint` of the `sb` server gives for a preset. A document that is
neither is described, with the reason for each parse, instead of guessed at.

```json
{ "json": "{\"user_agent\": \"Mozilla/5.0\"}" }
```

```json
{
  "coherent": false,
  "errors": ["user-agent does not contain a token for OS persona Windows"],
  "warnings": [],
  "parsed": true,
  "format": "fingerprint",
  "fingerprint": { "os": "Windows", "browser": "Chromium", "user_agent": "Mozilla/5.0", "platform": "Win32", "screen": null }
}
```

### `diagnose_environment` on the dev server

The same tool the `cdp`, `driver` and `sb` servers have, here too. Besides what
[its entry above](#diagnose_environment) lists, the report has `driver_cache` (the
`downloaded_drivers` folder, whether it is there and what is in it) and `build`
(the library version and the cargo features this build has and lacks, such as
`pdf` and `plugins-builtin`). An output folder that does not exist yet is judged
by the nearest folder above it, since the first tool that writes there makes it.

### Debugging: which tool for which symptom

| Symptom | Call, in this order |
| --- | --- |
| An element is not found | `explain_error` on the text; then `diagnose_selector` (hidden, covered, in an iframe or a shadow root, near misses), `suggest_selectors`, `inspect_page` |
| A wait or a navigation times out | `explain_error` (it tells `wait_timeout` from `navigation`); `get_failed_requests` on `cdp` and `sb`; `get_console_messages` with `start` true, then repeat the step |
| The page is blank, half loaded or looks wrong | `inspect_page`, `get_console_messages`, `get_failed_requests`, then `capture_debug_bundle` to keep the evidence |
| The driver and Chrome disagree ("session not created") | `diagnose_environment` (the `versions` field), then `install_driver`, then `diagnose_environment` again |
| A site blocks or detects the browser | `check_fingerprint` on the fingerprint you use, `audit_page`; for a `stealth` or `patcher` error, `explain_error` |
| A setting is ignored, or the server stops at start | `check_config` (every problem at once), `list_plugins` for a plugin name |
| An error text you do not understand | `explain_error` |
| A flag or a method you do not remember | `cli_help`, `find_api`, `parity_lookup` |

The `driver` server has the browser-side debugging tools too (`inspect_page`,
`diagnose_selector`, `suggest_selectors`, `get_console_messages`,
`capture_debug_bundle`, `get_page_info`, `export_session_as_test`,
`diagnose_environment`). `get_failed_requests` and `audit_page` are on `cdp` and
`sb`. On `driver` and `sb`, `get_console_messages` records from the moment it is
asked to (`start` true): WebDriver has no console events of its own to read.

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

<Prompt description="Port a Python test and see what is left" icon="code" actions={["copy"]}>

Convert this Python SeleniumBase test with python_to_rust. For every TODO it leaves, call parity_lookup with the Python method and find_api for the Rust one, then tell me what I still have to write by hand.

</Prompt>

<Prompt description="Find out why the setup fails" icon="stethoscope" actions={["copy"]}>

Run diagnose_environment and check_config on the dev server. Explain the first problem with explain_error, and if chromedriver does not match Chrome, fix it with install_driver and check again.

</Prompt>

<Visibility for="agents">

Summary for AI exports: `sbrs-mcp --server cdp|driver|sb|dev` serves MCP over stdio. On `cdp`, `driver` and `sb`, call `start_browser` once, use the tools, then `close_browser`; `dev` has no browser and helps with the library, the `sbrs` command line and diagnosing problems (`find_api`, `explain_error`, `check_config`, ...). Files are written under `SB_MCP_OUTPUT_DIR`. A failed call returns an error result with a `kind`, the call tried and next steps, not a protocol error. The `cdp`, `driver` and `sb` servers also have the page tools `page_snapshot` (its refs can be written `ref=e5` in any selector argument), `read_page_text`, `extract_links`, `extract_tables` and `extract_forms`, and the fingerprint tools `capture_fingerprint`, `compare_fingerprint` and `explain_fingerprint`. `sbrs-mcp tools|call|batch|resources|read|prompts|prompt` is an MCP client for the shell: `--server cdp|driver|sb|dev` starts this program as that server, `--connect PROGRAM` any stdio server, `--url` a running HTTP server (token in `SB_MCP_TOKEN` or `--token-file`); exit status 0 worked, 1 the tool failed, 2 a mistake in what was typed, 3 the server could not be reached.

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
- The `dev` server reads and computes: `install_driver` is the one tool that
  changes anything (it downloads a driver into `downloaded_drivers/` and may
  replace one of another version), and it says so in its annotations. `scaffold`
  writes nothing. `check_config` reads one settings file inside the folder the
  server runs in, redacts secrets, and does not quote the file's lines in its
  errors.

## Calling tools from a shell

`sbrs-mcp` is also an MCP client. A person, a script or an AI agent that has only a
shell can use any tool of these servers, or of any other MCP server, without an MCP
host. The commands need the cargo feature `mcp-client`, which the
`seleniumbase-rs-cli` package turns on by default.

| Command | What it does |
| --- | --- |
| `sbrs-mcp tools [TOOL]` | Lists the tools; with a name, shows that tool with the JSON Schema of its arguments. `--raw` prints JSON instead of a table. |
| `sbrs-mcp call TOOL [JSON]` | Calls a tool and prints what it returns. |
| `sbrs-mcp batch [FILE]` | Runs several calls in one session (see below). |
| `sbrs-mcp resources` | Lists the resources, such as the call journal. |
| `sbrs-mcp read URI` | Reads a resource, such as `sbrs://journal`. `--out DIR` saves a binary one. |
| `sbrs-mcp prompts` | Lists the prompts; a `*` marks a required argument. |
| `sbrs-mcp prompt NAME` | Renders a prompt; arguments are `--arg KEY=VALUE`, always text. |

```bash
sbrs-mcp tools                                    # the dev server, no setup
sbrs-mcp tools click_element --server cdp         # one tool and its argument schema
sbrs-mcp call find_api --arg query=click          # a dev-server tool
```

### Choosing the server

Give at most one of these; the commands take the same flags.

- **No flag:** the URL in `SB_MCP_URL` if it is set, else this program started as
  the `dev` server, so `sbrs-mcp tools` works with no setup.
- **`--server cdp`, `driver`, `sb` or `dev`:** this program, started as that
  server. It stops when the command ends and closes its browser.
- **`--connect PROGRAM`:** any MCP server that speaks stdio. Its arguments are
  `--connect-arg ARG`, one each (write `--connect-arg=--flag` when one starts with
  a dash).
- **`--url http://127.0.0.1:8931/mcp`:** a running streamable HTTP server, such as
  one started with `sbrs-mcp --listen 127.0.0.1:8931`. The bearer token it asks for
  comes from `SB_MCP_TOKEN` or from `--token-file FILE`, never from the command
  line; `--token-file` goes with `--url` only.

`--timeout SECONDS` (default 60) is how long to wait for the connection and for each
request.

### Arguments and output of `call`

Give the arguments one way: a JSON object after the tool name (or `-` for standard
input), `--json OBJECT` (or `-`), or `--arg KEY=VALUE`, repeated. In `--arg`, VALUE
is JSON when it parses as JSON (`true`, `3`, `[1,2]`), text in single quotes is that
text, and anything else is a string: `--arg selector=ref=e1` sends the string
`ref=e1`.

```bash
sbrs-mcp call --server cdp open_url '{"url": "https://example.com"}'
```

The text goes to standard output. `--raw` prints the structured result as one line of
JSON (the text, for a tool that sends none), `--out DIR` saves the images of the
result as `image-1.png`, `image-2.png`, ..., and `--progress` prints a tool's
progress to standard error (on by default on a terminal).

### One session for several calls

A command is its own MCP session, and a browser that a server starts closes when the
command ends: a `call` of `start_browser` does not leave a browser for the next
command. `batch` runs the calls in one session. It reads one JSON object per line from
FILE or standard input; blank lines and lines that start with `#` are skipped, and a
bad line stops the command before anything runs.

```bash
sbrs-mcp batch --server cdp --raw <<'EOF'
{"tool": "start_browser", "arguments": {"headless": true}}
{"tool": "open_url", "arguments": {"url": "https://example.com"}}
{"tool": "page_snapshot"}
{"tool": "close_browser"}
EOF
```

The calls run in order and stop at the first failure unless `--keep-going` is given.
`--raw` prints one JSON object per call, and `--out` and `--progress` work as for
`call`.

### Exit status

| Status | Meaning |
| --- | --- |
| 0 | It worked. |
| 1 | The tool reported a failure (or the output could not be written). |
| 2 | A mistake in what was typed: a bad flag or argument, an unknown tool, resource or prompt. |
| 3 | The server could not be reached, refused the token or broke the protocol. |

For `batch` the status is that of the first failure.


## Using the servers from Rust

```rust,no_run
use seleniumbase_rs::mcp::{self, Profile};

# async fn run() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
mcp::serve(Profile::Cdp).await?;
# Ok(())
# }
```

`mcp::cdp::host`, `mcp::driver::host`, `mcp::sb::host` and `mcp::dev::host` build
a `Host` you can call directly with `Host::call(name, arguments)`, which is how
the test suite exercises every tool against a mocked browser. The `dev` server
takes its driver downloader as a parameter (`mcp::dev::host_with_installer`), so
a test can install without the network. To add a tool, see "Adding an MCP
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
