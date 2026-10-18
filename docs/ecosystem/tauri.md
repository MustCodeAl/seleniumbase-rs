---
title: "Tauri Plugin"
sidebarTitle: "Tauri plugin"
---

export const LayoutTree = ({ children }) => (
  <div style={{ fontSize: "0.875rem", lineHeight: 1.7, border: "1px solid rgba(127,127,127,0.25)", borderRadius: "0.75rem", padding: "0.75rem 1rem", margin: "1rem 0" }}>
    {children}
  </div>
)

export const LayoutNote = ({ note }) =>
  note ? <span style={{ opacity: 0.7, marginLeft: "0.75rem" }}>{note}</span> : null

export const LayoutDir = ({ name, note, open = false, children }) =>
  children ? (
    <details open={open} style={{ margin: "0.1rem 0" }}>
      <summary style={{ cursor: "pointer" }}>
        <Icon icon="folder" size={14} /> <code>{name}/</code>
        <LayoutNote note={note} />
      </summary>
      <div style={{ marginLeft: "0.4rem", paddingLeft: "1rem", borderLeft: "1px solid rgba(127,127,127,0.3)" }}>
        {children}
      </div>
    </details>
  ) : (
    <div style={{ margin: "0.1rem 0", paddingLeft: "1.1rem" }}>
      <Icon icon="folder" size={14} /> <code>{name}/</code>
      <LayoutNote note={note} />
    </div>
  )

export const LayoutFile = ({ name, note, highlight = false }) => (
  <div style={{ margin: "0.1rem 0", paddingLeft: "1.1rem" }}>
    <Icon icon="file" size={14} /> <code style={highlight ? { fontWeight: 700 } : undefined}>{name}</code>
    <LayoutNote note={note} />
  </div>
)

<GitHub.Repo repo="MustCodeAl/tauri-plugin-sbrs-automation" />

<Badge color="orange" shape="pill">Early: 0.1</Badge> <Badge color="purple" shape="pill">Tauri 3.0.0-alpha.4</Badge> <Badge color="blue" shape="pill">W3C WebDriver</Badge> <Badge color="green" icon="apple" shape="pill">macOS: verified</Badge> <Badge color="gray" icon="windows" shape="pill" stroke>Windows: not run yet</Badge> <Badge color="gray" icon="linux" shape="pill" stroke>Linux: not run yet</Badge>

Test a [Tauri](https://tauri.app) app's real window with
[seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs) (`BaseCase`
and the `sbrs` CLI) on **macOS, Windows and Linux**.

Tauri apps use the system webview. Windows (WebView2) and Linux (WebKitGTK)
have WebDriver drivers, but **macOS
(<Tooltip tip="The system web view of macOS, which Tauri apps use there.">WKWebView</Tooltip>)
has none**. This plugin runs a small W3C WebDriver endpoint inside your app, so
the same tests drive the app everywhere.

<Panel>

<Card title="Key facts" icon="circle-info">

- Endpoint: `http://127.0.0.1:4445/{token}/`
- On in debug builds, or with `SBRS_AUTOMATION=1`
- Token: `SBRS_AUTOMATION_TOKEN`, or a random one printed at start
- One window, `main`, and one session at a time
- Input is simulated in the page
- Screenshots on macOS only

</Card>

<Card title="API reference" icon="code" href="/ecosystem/tauri-api/overview">

Every route with its fields, replies and errors.

</Card>

</Panel>

<Note>

Early (0.1, Tauri 3 alpha). It serves the core of WebDriver: sessions,
timeouts, navigation, finding elements (CSS, XPath, link text, tag name),
clicking, typing, reading text, attributes, properties and state, running
scripts (sync and async), the window's rectangle (read, move, resize,
maximize, minimize, full screen), user prompts (alert, confirm, prompt),
same-origin frames, actions (mouse and keys) and, on macOS, screenshots.
See [Supported commands](#supported-commands). Clicks, typing, actions and
dialogs are simulated in the page with DOM events and page script, not
OS-level input, so the mouse and keyboard are never used.

</Note>

<Check>

**Verified on a real app** (macOS, Tauri `3.0.0-alpha.4` with the wry
runtime): the `tauri-profile-manager` example of seleniumbase-rs is driven by
`BaseCase` (thirtyfour 0.37) in an end-to-end test.

- **2026-10-15**: the test starts a session, reads the title, finds elements
  by CSS selector, waits for them to be displayed and enabled, clicks, clears
  and types into inputs, reads and asserts text, reads and resizes the window,
  and ends the session; sync and async scripts were run on the same app with
  raw WebDriver requests.
- **2026-10-15**, then: the same test answered a confirm and a prompt (answer
  set before, and the default dismiss), entered an `<iframe srcdoc>` (finding,
  a script, a click and its alert inside) and was refused a sandboxed frame,
  double-clicked, hovered and pressed Space and Enter through W3C actions, and
  took a page screenshot (1800 x 1400 for a 900 x 700 window on a Retina
  display) and an element screenshot.
- **2026-10-16**: it was refused accepting a confirm that had been dismissed
  when it opened, saw an alert leave the answer to the confirm after it and
  cleared the two in order, and hovered and took the element screenshot of a
  paragraph far below the view on a page with `scroll-behavior: smooth`.

</Check>

<Warning>

**Not run on a real app yet:** Windows and Linux; the XPath, link-text and
tag-name strategies, which no test covers; `DELETE /actions`, entering a
frame by index and `POST /frame/parent`, whose requests the Rust unit tests
check against a fake page script; right clicks, drags and Backspace, whose
page side is checked only by the tests of `src/bridge.js` in a fake page under
Node.js (`tests/bridge.test.mjs`, which `cargo test` runs and which need
`node` on `PATH`), not in a webview; and element screenshots of a zoomed page,
checked only by a unit test of the conversion to the web view's points.

</Warning>

## Set up your app

<Steps>

<Step title="Add the plugin" icon="cube">

Add the plugin, best behind a feature so release builds never have it:

```toml src-tauri/Cargo.toml
[features]
e2e = ["dep:tauri-plugin-sbrs-automation"]

[dependencies]
tauri-plugin-sbrs-automation = { git = "https://github.com/MustCodeAl/tauri-plugin-sbrs-automation", optional = true }
```

</Step>

<Step title="Register it" icon="plug">

Register it:

```rust The app's builder
let mut builder = tauri::Builder::default();
#[cfg(feature = "e2e")]
{
    builder = builder.plugin(tauri_plugin_sbrs_automation::init());
}
```

</Step>

<Step title="Allow its command" icon="shield-halved">

Allow its command. With the plugin behind a feature, put the permission in
a capability of its own, `src-tauri/capabilities/e2e/automation.json`:

<LayoutTree>
<LayoutDir name="src-tauri" open note="the Rust side of the app">
<LayoutFile name="Cargo.toml" note="the e2e feature and the optional plugin" />
<LayoutFile name="build.rs" highlight note="reads capabilities/e2e only with the feature" />
<LayoutDir name="capabilities" open>
<LayoutFile name="default.json" note="cannot name sbrs-automation:default next to an optional plugin" />
<LayoutDir name="e2e" open>
<LayoutFile name="automation.json" highlight note="grants sbrs-automation:default to the main window" />
</LayoutDir>
</LayoutDir>
</LayoutDir>
</LayoutTree>

```json src-tauri/capabilities/e2e/automation.json
{
  "identifier": "e2e-automation",
  "windows": ["main"],
  "permissions": ["sbrs-automation:default"]
}
```

and let `build.rs` read that folder only with the feature. A capability
that names `sbrs-automation:default` while the plugin is not compiled in
fails the build (`Permission sbrs-automation:default not found`), so it
cannot go in `default.json` next to an optional plugin:

```rust src-tauri/build.rs
fn main() {
    let capabilities = if std::env::var_os("CARGO_FEATURE_E2E").is_some() {
        "./capabilities/**/*"
    } else {
        "./capabilities/*.json"
    };
    println!("cargo:rerun-if-changed=capabilities");
    let attributes = tauri_build::Attributes::new().capabilities_path_pattern(capabilities);
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("{error:#}");
    }
}
```

If the plugin is always compiled in, `"sbrs-automation:default"` can simply
join the permissions of `capabilities/default.json`.

</Step>

<Step title="Name the runtime (Tauri 3)" icon="gear">

On Tauri 3, the app must name its runtime, or it stops at start with
`RuntimeNotConfigured`: depend on `tauri-runtime-wry` (the same alpha) and
call `.runtime(tauri_runtime_wry::Wry::default())` on the builder.

</Step>

</Steps>

<Prompt description="Ask a coding agent to set it up" actions={["copy", "cursor"]}>

Add tauri-plugin-sbrs-automation (git https://github.com/MustCodeAl/tauri-plugin-sbrs-automation) to this Tauri app behind a Cargo feature named e2e, as https://mustcodeal-seleniumbase-rs.docs7.io/ecosystem/tauri describes: make the dependency optional, register the plugin on the builder only with the feature, grant sbrs-automation:default in src-tauri/capabilities/e2e/automation.json, let build.rs read the capabilities folder recursively only when CARGO_FEATURE_E2E is set, and on Tauri 3 depend on tauri-runtime-wry and call .runtime(tauri_runtime_wry::Wry::default()) on the builder.

</Prompt>

## Run a test

Start the app with a token of your choice (or read the random one it prints):

```bash Terminal
SBRS_AUTOMATION_TOKEN=secret cargo tauri dev --features e2e
```

Then point `BaseCase` at it. The URL ends with `/`: WebDriver clients join
each command path (`session`, ...) to the server URL as a relative URL, so
without the slash the token is replaced and every request is refused with
`the URL does not start with the token`.

```rust A test
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::test]
async fn saving_a_profile() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let config = BrowserConfig::default().with_webdriver_url("http://127.0.0.1:4445/secret/");
    let mut sb = BaseCase::new(config).await?;
    sb.type_text("#name", "Work").await?;
    sb.click("#save").await?;
    sb.assert_text("Saved", "#status").await?;
    sb.quit().await
}
```

Any WebDriver client works the same way with `http://127.0.0.1:4445/secret/`
as its server URL. A new session waits until the app's window has loaded its
page, so a test can start one as soon as the endpoint answers
`GET /secret/status`. For a test that starts the app itself, see
`examples/tauri-profile-manager/src-tauri/tests/e2e_automation.rs` in
seleniumbase-rs.

## Settings

The plugin reads these environment variables once, when the app starts.

<ParamField body="SBRS_AUTOMATION" type="string" default="on in debug builds, off in release">

`0`, `false` or `off` turn the plugin off; any other value, such as `1`, turns
it on. Unset or empty, the build decides. Off, the plugin injects no page
script and serves no endpoint.

</ParamField>

<ParamField body="SBRS_AUTOMATION_PORT" type="integer" default="4445">

The port on `127.0.0.1`. If the port is taken (a second instance, a Selenium
container), the app runs without the endpoint and says so on stderr. A value
that is not a port number counts as unset.

</ParamField>

<ParamField body="SBRS_AUTOMATION_TOKEN" type="string" default="random">

The first segment of every path. Unset or empty: a random token of 32
hexadecimal digits, printed when the app starts, in the endpoint's URL.

</ParamField>

<ParamField body="SBRS_AUTOMATION_TRACE_WINDOW" type="string" default="off">

Any non-empty value makes each window command print the window's state on
stderr before it asks for a change, after, and when it answers: what Tauri
reports and, on macOS, what the `NSWindow` says (style mask, `isZoomed`,
frame, content view, screen). For finding out why a resize did not stick.

</ParamField>

## Supported commands

Every path starts with `/<token>`; session commands with `/session/{id}`. The
[API reference](/ecosystem/tauri-api/overview) gives each command's fields,
replies and errors, with request and response examples.

| Command | Endpoint | Notes |
|---|---|---|
| [Status](/ecosystem/tauri-api/status) | `GET /status` | No session needed. |
| [New and delete session](/ecosystem/tauri-api/sessions) | `POST /session`, `DELETE /session/{id}` | One session at a time: a new one replaces the last. A new session starts in the top page, with no dialog answer or input held from the last one. |
| [Timeouts](/ecosystem/tauri-api/timeouts) | `GET`, `POST /timeouts` | `script`, `pageLoad`, `implicit` (finding looks again until it is over). |
| [Navigation](/ecosystem/tauri-api/navigation) | `POST /url`, `GET /url`, `GET /title`, `POST /back`, `/forward`, `/refresh` | Waits for the new page to load. |
| [Page source](/ecosystem/tauri-api/navigation#get-page-source) | `GET /source` | Of the current frame. |
| [Window](/ecosystem/tauri-api/window) | `GET /window`, `/window/handles`, `POST /window`, `GET`/`POST /window/rect`, `POST /window/maximize`, `/minimize`, `/fullscreen` | One window, `main`. A change waits until the window has it (normally well under a second; each state change is waited for up to three seconds, the final rectangle up to two more), and the answer is the window's rectangle then, which can differ from the one asked for: the system keeps a window below the menu bar and may refuse a size. |
| [Frames](/ecosystem/tauri-api/frames) | `POST /frame`, `POST /frame/parent` | Same-origin frames only. |
| [Find elements](/ecosystem/tauri-api/elements) | `POST /element`, `/elements`, `/element/{id}/element`, `/element/{id}/elements`, `GET /element/active` | CSS, XPath, link text, partial link text, tag name. |
| [Element state](/ecosystem/tauri-api/element-state) | `GET /element/{id}/text`, `/name`, `/rect`, `/enabled`, `/selected`, `/displayed`, `/attribute/{name}`, `/property/{name}`, `/css/{name}` | |
| [Element interaction](/ecosystem/tauri-api/element-interaction) | `POST /element/{id}/click`, `/clear`, `/value` | DOM events. |
| [Scripts](/ecosystem/tauri-api/scripts) | `POST /execute/sync`, `/execute/async` | Run in the current frame. |
| [Actions](/ecosystem/tauri-api/actions) | `POST /actions`, `DELETE /actions` | Mouse, keys and pauses. |
| [User prompts](/ecosystem/tauri-api/alerts) | `GET /alert/text`, `POST /alert/accept`, `/alert/dismiss`, `/alert/text` | Simulated: answered when they open. |
| [Screenshots](/ecosystem/tauri-api/screenshots) | `GET /screenshot`, `GET /element/{id}/screenshot` | macOS only; PNG. |

<Info>

Not served: cookies, shadow roots, and opening or closing windows (`unknown
command`); screenshots on Windows and Linux, wheel input, pen and touch
pointers (`unsupported operation`); cross-origin frames (`no such frame`). The
endpoint drives one window, `main` (or, without one, another window of the
app). Cookies are left out on purpose: on macOS the app's page is
`tauri://localhost`, where WKWebView keeps no cookies at all
(`document.cookie` reads back empty after a write).

</Info>

### Details

<Warning>

Dialogs are answered when they open. A test that should accept a confirm
accepts first, then clicks; the usual WebDriver order, click and then accept,
gets `unsupported operation`. See [User prompts](/ecosystem/tauri-api/alerts).

</Warning>

<Columns cols={2}>

<Card title="User prompts" icon="message" href="/ecosystem/tauri-api/alerts">

How `alert`, `confirm` and `prompt` are replaced, recorded and answered.

</Card>

<Card title="Frames" icon="table-columns" href="/ecosystem/tauri-api/frames">

Same-origin frames only, and what acts in the frame entered.

</Card>

<Card title="Actions" icon="keyboard" href="/ecosystem/tauri-api/actions">

Tick by tick, as DOM events: the events the page sees, and why CSS `:hover`
and HTML drag and drop do not happen.

</Card>

<Card title="Screenshots" icon="camera" href="/ecosystem/tauri-api/screenshots">

WKWebView's own snapshot at the screen's scale; an element's part of it.

</Card>

<Card title="Window changes" icon="window-maximize" href="/ecosystem/tauri-api/window#set-window-rect">

Set Window Rect first brings the window back to its normal state, as
WebDriver asks: out of full screen, not maximized, not minimized.

</Card>

<Card title="Errors" icon="triangle-exclamation" href="/ecosystem/tauri-api/overview#errors">

Every error code the plugin answers, its HTTP status, and when.

</Card>

</Columns>

## Changing the page script

The page script is written in TypeScript in `src/bridge.ts`. `src/bridge.js` is
generated from it and is what the plugin embeds, so apps that use the plugin
build without Node or Bun.

<Steps>

<Step title="Edit the source" icon="pen">

Change `src/bridge.ts`, then run `bun install` once.

</Step>

<Step title="Generate the script" icon="gears">

Run `bun run bridge`. It builds `bridge.js` and records the SHA-256 of
`bridge.ts` on its first line. Commit both files. `bunx tsc --noEmit`
type-checks the source.

</Step>

</Steps>

<Note>

`cargo test` fails, naming this command, when `bridge.js` is older than
`bridge.ts`.

</Note>

## How it works

```mermaid
flowchart LR
    A["BaseCase / sbrs"] -- "HTTP (WebDriver)" --> B["plugin server<br/>127.0.0.1, inside the app"]
    B -- "eval" --> C["bridge.js in the webview"]
    C -- "IPC reply" --> B
    B -- "window commands,<br/>screenshots" --> D["the native window<br/>(Tauri, WKWebView)"]
```

The server turns each WebDriver command into an operation of an injected script
(`src/bridge.js`), which runs it in the page and sends the result back through
Tauri's IPC. The window commands and screenshots ask the native window instead:
the page cannot tell the window's outer size (WKWebView reports it as 0), and
WebKit draws the page out of process.

<LayoutTree>
<LayoutDir name="tauri-plugin-sbrs-automation" open note="the plugin's repository">
<LayoutDir name="src" open>
<LayoutFile name="lib.rs" note="init, the settings, the bridge to the window" />
<LayoutFile name="server.rs" note="the HTTP server on 127.0.0.1, one request at a time" />
<LayoutFile name="webdriver.rs" highlight note="the routes, the replies and the error statuses" />
<LayoutDir name="webdriver" open>
<LayoutFile name="actions.rs" note="checks W3C actions and cuts them into ticks" />
</LayoutDir>
<LayoutFile name="bridge.ts" highlight note="the page script, in TypeScript: edit this one" />
<LayoutFile name="bridge.js" note="generated from bridge.ts; what the plugin embeds" />
<LayoutFile name="window.rs" note="window commands: change, then wait until the window has it" />
<LayoutFile name="native.rs" note="the app's window, through Tauri" />
<LayoutFile name="screenshot.rs" note="macOS: the WKWebView's snapshot, as PNG" />
<LayoutFile name="trace.rs" note="the SBRS_AUTOMATION_TRACE_WINDOW output" />
</LayoutDir>
<LayoutDir name="permissions" note="sbrs-automation:default lets the page script send its results back" />
<LayoutDir name="tests" note="bridge.test.mjs runs bridge.js in a fake page under Node.js" />
</LayoutDir>
</LayoutTree>

## Security

<Warning>

The endpoint can run any JavaScript in your app's window; that is what
WebDriver's "execute script" is for. So it is guarded:

- on in debug builds only, unless you set `SBRS_AUTOMATION=1`;
- listens on `127.0.0.1` only;
- every URL must start with the secret token;
- requests from web pages (any request with an `Origin` header) are refused.

Keep it behind a Cargo feature that release builds do not enable.

</Warning>

## License
MIT.
