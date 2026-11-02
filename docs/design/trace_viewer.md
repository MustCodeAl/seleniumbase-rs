---
title: "Design: a trace viewer for the pure CDP engine"
sidebarTitle: "Trace viewer"
---

Status: design note, written before the code (branch `friend/trace`).
Code: `src/sb_cdp/trace.rs` and the folder `src/sb_cdp/trace/`.

## Goal

Run a test, then open one HTML file and see what happened: a timeline of the
actions the test took, a screenshot and a DOM snapshot after each, the page's
console, and the network. The recording is opt-in (`Page::start_trace`), costs
nothing when it is off, and the viewer is a single self-contained file: no CDN,
no request to anywhere.

Not in this change: a command that opens it (`sbrs trace show` belongs in
`crates/cli`, which another lane owns), a trace of several tabs at once, a
video.

## What Playwright does (and what we take from it)

Read for this note: the user guide page "Trace viewer"
([Playwright docs](https://playwright.dev/docs/trace-viewer)) and the trace event types in the
source, `packages/isomorphic/trace/trace.ts`, which re-exports
`versions/traceV10.ts`. (The path `packages/trace/src/trace.ts` no longer
exists.)

A Playwright trace is a zip, `trace.zip`:

- `trace.trace`: newline-delimited JSON events. `context-options` (browser,
  platform, viewport, wall and monotonic start time); `before` / `input` /
  `after`, paired by `callId` (API class and method, params, start time; the
  input point and box; end time and error); `log` lines per call; `console`
  (time, message type, text, location); `screencast-frame` (a JPEG file name
  and a timestamp, a continuous stream); `screenshot`; `frame-snapshot` (a
  DOM tree where an unchanged subtree is a reference to "node #y of the
  snapshot x ago", plus the resource URLs it needs); `event`, `stdout`,
  `stderr`, `error`.
- `trace.network`: one `resource-snapshot` per request, which is a HAR entry.
- `resources/`: the blobs, named by the SHA-1 of their content.

The viewer is a web app (`playwright show-trace` serves it; a service worker
rebuilds each frame snapshot from the nodes and resources). Its tabs: the action
list with a filmstrip timeline, Before / Action / After snapshots, Call, Log,
Errors, Console, Network, Source, Attachments, Metadata.

Taken over:

- An action is a `before` with its name, parameters and start time, and an
  `after` with the end time and the error if it failed, and every time is
  relative to the start of the trace.
- A screenshot and a DOM snapshot belong to an action.
- Console messages are time-stamped events with a level and text; the viewer
  shows the ones that fall inside the selected action.
- The network log is HAR. Here it is the recorder this crate already has
  (`Page::start_har`), written as `network.har`; there is no second recorder.
- Data is a folder of files, each referenced by name from the event list.
- The viewer's shape: a timeline on the left, the picture in the middle,
  detail tabs (Call, Console, Network) beside it.

Simpler, on purpose:

| Playwright | Here | Why |
|---|---|---|
| zip of NDJSON | a folder: `trace.json` (one document, written when the trace stops), `network.har`, `screenshots/`, `snapshots/`, `index.html` | nothing to unzip, any tool reads it; the cost is that a crash before `stop` leaves no `trace.json` (the image files are already there) |
| before + action + after snapshots | one snapshot and one screenshot after each action | the "before" of a step is the "after" of the previous one |
| a screencast, continuous | a JPEG per action | fewer files, bounded size; no frame between actions |
| DOM tree with subtree references, resources by URL | the serialised document with its stylesheets inlined as `<style>` | no snapshot server or service worker; the size is bounded by a cap per snapshot and a total budget instead of being shared between snapshots |
| `input` events with point and box | not recorded | would need a protocol call per action (a box model); the screenshot shows the page after the click |
| source tab, stack frames | not recorded | a Rust caller has no source map at the locator |
| viewer: a web app, a server | one static HTML file in the folder, opens from `file://` | the requirement is no external request |

## What this engine adds to the problem

The pure CDP engine never sends `Runtime.enable`, `Log.enable`,
`DOM.enable`, `Debugger.enable` or `Profiler.enable` and sends no `DOM.*`
command (`tests/sb_cdp_domain_hygiene.rs` reads the sources and fails on one),
because a page can see those domains being on. So a trace cannot subscribe to
the browser's console events or use the DOM domain for snapshots. It does what
the rest of the engine does and asks the page:

- The console is captured by a small script (`src/sb_cdp/trace/capture.js`).
  It wraps `console.debug/log/info/warn/error` and listens for `error` and
  `unhandledrejection`, keeps a bounded buffer in the document, and hands the
  buffer over when the next action ends. It is added with
  `Page.addScriptToEvaluateOnNewDocument` (so every later document has it) and
  evaluated once in the document that is open when the trace starts.
- The snapshot is built by the same script with `Runtime.evaluate`: a clone of
  the document element, scripts removed, each stylesheet inlined as a `<style>`
  (when the page lets us read its rules), the `value` of password inputs
  removed. The script, the title and the URL come back in one call, together
  with the console buffer: one `Runtime.evaluate` and one
  `Page.captureScreenshot` per action, and nothing when no trace runs.
- The script is visible to a page (a global `__sbtrace`, wrapped console
  methods), like the page helper `__sbcdp` the engine injects already. A trace
  is a debugging aid and is documented as not for a run that must look
  untouched.
- Limits of this: console messages of iframes are not collected, and what a
  page logs in the instant before it navigates away can be lost with the old
  document; both are documented.

## Recording actions

`Browser` keeps the running traces, by tab id (`Inner::traces`). A trace is
registered by `Page::start_trace` and removed by `stop` or by dropping the
recorder, so every handle to the tab sees it, and nothing refers back to the
`Page` (no reference cycle).

A `Locator` action is its public method wrapped once:

```text
pub async fn click(&self) -> ... { self.traced("click", Detail::None, async { ... }).await }
```

`traced` looks up the running trace of the tab. With none it awaits the body and
returns; it sends no protocol command, allocates nothing and cannot change the
result. With one it notes the start, awaits the body, notes the end and the
result, and then captures. A capture failure is swallowed (and counted in
`trace.json`); it never turns a successful action into an error.

Locator methods that call another (`fill` clears the field first, `check`
clicks) call the private, untraced step, so one call is one record. The actions
traced: `click`, `double_click`, `right_click`, `hover`, `click_and_hold`,
`drag_to`, `focus`, `clear`, `fill`, `send_keys`, `type_text`, `set_value`,
`press`, `select_option`, `check`, `uncheck`, `set_checked`, `submit`,
`scroll_into_view`, and `Page::goto`. Reads (`text`, `count`, ...) and the
assertions are not.

## Files

```text
<dir>/
  index.html          the viewer: everything below embedded (screenshots as data: URIs)
  trace.json          version, start time, duration, actions, console, limits
  network.har         the HAR (no bodies), request bodies and Authorization removed
  screenshots/0001.jpg
  snapshots/0001.html
```

`trace.json` has `version: 1`; times are milliseconds from the start of the
trace; an action is `{ id, name, selector, detail, start_ms, end_ms, ok, error,
url, title, screenshot, snapshot }`; a console record is `{ at_ms, level,
text }`; `limits` says what was left out.

## Security

Everything that comes from the page is untrusted: console text, URLs, titles,
the DOM, selectors (the caller's, but often derived from the page), error
messages. The trace is opened in a browser, so the viewer is the attack surface.

1. The page's strings never become HTML in Rust. The data is JSON, put in a
   `<script type="application/json">` element after `<`, `>`, `&`, U+2028 and
   U+2029 are written as `\uXXXX` escapes. A `</script><script>` in a console
   line therefore stays inside a JSON string. `serde_json` alone does not do
   this; there is a unit test with a hostile console line and a hostile
   selector.
2. The viewer's script builds its page with `createElement` and `textContent`;
   it has no `innerHTML`, `outerHTML`, `document.write`, `eval` or `new
   Function`. The one place an HTML string from the page is used is the
   snapshot.
3. A DOM snapshot is shown only in an `iframe` with `sandbox=""` (no
   `allow-scripts`, no `allow-same-origin`, no `allow-forms`, no
   `allow-popups`), set through `srcdoc`.
4. The viewer has a Content-Security-Policy `<meta>`:
   `default-src 'none'; script-src 'sha256-<hash of the viewer script>';
   style-src 'unsafe-inline'; img-src data:`. An injected script that got past
   point 1 would not run; the `srcdoc` iframe inherits the policy, so a
   snapshot loads nothing from anywhere (images and fonts of the page are not
   shown, which is the price of "no external request"). The hash is computed
   when the file is written (`ring`, already a dependency); a test checks it.
5. The capture script removes `<script>`, `<noscript>`, `<base>`,
   `<meta http-equiv>` from the clone and the `value` attribute of password
   inputs; the snapshot is also size-capped by the page script and again in
   Rust, because a hostile page can lie to the script.
6. Typed text is masked: the text given to `fill`, `type_text`, `send_keys` and
   `set_value` is never written, whatever the field; the action's `detail` is
   `[text not recorded]`. This is not limited to password fields because the
   caller's string cannot be classified without another protocol call, and a
   fill of a field the page calls "search" can still be a secret. Request
   bodies in the HAR (a login form posts the password) are replaced by a
   marker, as are `Authorization` and `Proxy-Authorization` header values. A
   URL's user name and password are removed in the `goto` detail.
7. What is not masked, and is said in the documentation: screenshots show what
   the page shows (a typed value in a plain field, personal data); cookies
   (`Cookie`, `Set-Cookie`) and URLs, query strings included, are in the HAR;
   a DOM snapshot has the page's text. A trace folder is sensitive: keep it out
   of version control and out of public CI artifacts.
8. The trace path must be a folder the caller names: not empty, and either
   absent (its parent exists and it is created) or an existing real directory
   (not a symlink) that is empty. Anything else is refused with
   `invalid_config`, so a trace never writes into a folder with other files,
   through a symlink, or to a path a page chose. Every file name inside it is
   made by the recorder from a counter, never from page data.
9. Bounds, so a long run cannot fill the disk: `TraceOptions` has
   `max_actions` (default 1000), `max_bytes` (default 64 MiB, for screenshots
   and snapshots together; after it, actions are still recorded but without
   pictures), `max_console_messages` (default 2000) and `max_requests`
   (default 2000); each is clamped to a hard maximum. Per item: a snapshot is
   cut at 512 KiB, a screenshot over 4 MiB is dropped, a console text is cut at
   2000 characters, a selector or error at 2000, a URL at 2048. The viewer
   repeats the pictures, so the folder takes about twice `max_bytes`.

## Tests

- Mock transport (`Browser::new_mocked`, `MockCtrl`, feature `test-util`):
  an action is recorded in order with its result; a failing action records the
  error and still fails; with no trace, a click sends no screenshot command and
  no `__sbtrace` script, and a stopped trace leaves none behind; typed
  text and request bodies are not in any file; limits (`max_actions`,
  `max_bytes`); the path refusals; the engine still sends none of the five
  `enable` commands during a trace.
- Unit tests (`trace.rs`, `trace/viewer.rs`): the JSON escaping of `</script>`,
  `<!--`, U+2028 inside the embedded data; a hostile console line and a hostile
  selector produce a file in which the only `<script` openings are the data
  element and the viewer script; the CSP hash matches; the iframe sandbox is
  empty; the viewer script has none of the forbidden calls.
- One `#[ignore]`d headless Chrome test in `tests/sb_cdp_chrome.rs`, against its
  local fixture server: a click and a fill, stop, check the folder, the
  masked text, and the HTML; then open the HTML in the same Chrome and check it
  rendered the steps.

## As built

The code wins where it differs from the plan above:

- **The page script undoes itself.** `__sbtrace` is `configurable` and has a
  `release()`; `TraceRecorder::stop` calls it (one more evaluation, which also takes the
  last console messages), so the console methods are put back when the page has not
  wrapped them since, and the global is gone. `stop` also removes the new-document
  script (`Page.removeScriptToEvaluateOnNewDocument`), and so does dropping the
  recorder. The script runs in the top document only.
- **One record per call, without a lock held across an await.** `Locator::traced` and
  `Page::traced` wrap the public methods; the four that other methods call as a step
  (`click`, `focus`, `clear`, `set_checked`) have a private `*_step` twin that notes
  nothing. `Page::goto` notes the address without its user name and password.
- **Page addresses lose their credentials too.** The `url` of an action goes through the
  same cleaning as the `goto` detail.
- **The pictures of one action are kept whole or not at all.** If the screenshot and
  the snapshot together would pass `max_bytes`, neither is written and
  `limits.pictures_skipped` counts the action.
- **Pictures are written when taken and read back when the trace stops**, so a long run
  does not hold them in memory until the end; the viewer embeds them from the files.
- **A failure of the network log is not a failure of the trace.** The HAR recorder is
  `start_har_with` (no bodies); if it cannot start or finish, `limits.network_error` says
  so and the viewer shows the note. Because the recorder attaches to cross-site frames, a
  trace and a `start_har` recording cannot run on one tab together.
- **Tests.** The mock-driven tests are in `src/sb_cdp/trace.rs` and `trace/viewer.rs`
  (no new file under `tests/`); the Chrome test is in `tests/sb_cdp_chrome.rs`.
- **Not done here.** `sbrs trace show` (it belongs in `crates/cli`) and
  `src/mcp/dev/api_index.json`, which has to be written again for the new public methods
  `Page::start_trace` and `Page::start_trace_with`.

