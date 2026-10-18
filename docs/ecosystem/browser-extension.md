---
title: "Browser Extension"
sidebarTitle: "Browser extension"
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
    <span style={{ whiteSpace: "nowrap" }}>{name}</span>
    {note ? <span style={{ color: "var(--text-muted)", marginLeft: "6px" }}>{note}</span> : null}
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

Browser extensions for [seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs), the Rust port of
SeleniumBase. One source tree builds Manifest V3 extensions for Chrome (also Edge and other Chromium
browsers) and Firefox.

<GitHub.Repo repo="MustCodeAl/seleniumbase-rs-browser-extension" />

The extension is written in strict TypeScript and built with [Bun](https://bun.sh); the built `dist/` holds
plain JavaScript for each browser.

<Columns cols={1}>

<Card color="#1a85d4" title="Recorder" icon="circle-dot">

Start and stop recording from the toolbar popup (or the Chrome side panel, or the Firefox sidebar). Clicks, double clicks, typing (one step per field), selects, checkboxes, key presses (Enter, Escape, arrows), file inputs (the file name only) and, if you turn it on, scrolls become steps you can edit, reorder and delete. Export as a seleniumbase-rs **pure CDP** Rust test, a **BaseCase** Rust test, or a **Chrome DevTools Recorder** JSON file (the `@puppeteer/replay` format that `sbrs import-puppeteer` reads).

</Card>

<Card color="#199f43" title="Inspector" icon="magnifying-glass">

Pick an element (hover, click; Escape cancels) and see its selectors, each with the number of elements it matches, and calls to copy for both engines (`page.locator(...).click()`, `sb.click(...)`, ...). The **Page** tab shows what the page sees: `navigator.webdriver`, user agent and client hints, languages, platform, hardware, screen and window sizes, time zone, WebGL vendor and renderer, a canvas hash, plugin counts, permission states and headless markers, and copies, exports and imports it as the JSON of seleniumbase-rs's `Fingerprint` (see [The Page tab](#the-page-tab-capture-copy-export-import)).

</Card>

<Card color="#d47628" title="DevTools Recorder export" icon="file-export">

<Badge color="blue">Chrome, Edge</Badge>

Adds "seleniumbase-rs (pure CDP)" and "seleniumbase-rs (BaseCase)" to the Export menu of the DevTools Recorder panel.

</Card>

</Columns>

<Info>

Nothing is sent anywhere. Recordings are kept in the extension's own storage and leave it only when you press
Download, Copy or Export. The value of a password or payment field is never read, so it cannot be recorded; the
generated code reads it from an environment variable instead (`RECORDED_PASSWORD`, ...).

</Info>

<Visibility for="agents">

Summary for AI exports: build the extension with `bun install` and `bun run build`, then load `dist/chrome` (Chrome, Edge) or `dist/firefox`. The recorder exports a seleniumbase-rs pure CDP Rust test, a BaseCase Rust test or a Chrome DevTools Recorder JSON file; the inspector shows selectors and the page's fingerprint. Nothing is sent anywhere.

</Visibility>

## The Page tab: capture, copy, export, import

The **Page** tab reads what the page's own scripts see and writes it as the JSON of a seleniumbase-rs
`Fingerprint` (fingerprint API v2, seleniumbase-rs 0.5): one entry per setting, each its value
(`"timezone": "Europe/Paris"`, `"screen": { "width": 1512, "height": 982 }`). A setting the page cannot tell
(the position, the fonts, a proxy, the WebRTC addresses) is left out, which seleniumbase-rs reads as "natural".
The only word it writes is `"off"`, for `client_hints` on a secure page of a browser that has no
`navigator.userAgentData` (Firefox, Safari).

| Button | What it does |
| --- | --- |
| **Capture again** | Reads the page again. |
| **Copy** | Puts the JSON on the clipboard. |
| **Export** | Saves it as `fingerprint-<host>-<date>.json` (a Blob link, so the extension needs no `downloads` permission). |
| **Import** | Opens a `.json` file you pick, checks it as API v2, and lists every setting the file claims next to the page's value as `match`, `differs` or `not observable` (a page cannot read the position, the fonts or a proxy, and a `"random"` setting has no value until seleniumbase-rs draws one). |

Import refuses the old flat shape of seleniumbase-rs 0.4 and earlier (`screen_width`, `webgl_vendor`, `flags`,
...) with each field named, as well as bad JSON and an `"off"` where seleniumbase-rs refuses one. The file is read
and compared inside the panel; nothing is uploaded.

<Note>

In Firefox the toolbar popup can close when the file dialog opens; use the sidebar for Import.

</Note>

## Install from source

You need [Bun](https://bun.sh) (`mise install` sets everything up).

<Steps>

<Step title="Build" icon="hammer">

```sh Terminal
bun install
bun run build          # dist/chrome, dist/firefox, and a zip of each
```

</Step>

<Step title="Load the extension" icon="puzzle-piece">

<Tabs>

<Tab title="Chrome / Edge" icon="chrome">

Open `chrome://extensions` (or `edge://extensions`), turn on Developer mode, **Load
unpacked**, choose `dist/chrome`.

</Tab>

<Tab title="Firefox" icon="firefox-browser">

Open `about:debugging#/runtime/this-firefox`, **Load Temporary Add-on**, choose
`dist/firefox/manifest.json` (or the zip).

</Tab>

</Tabs>

</Step>

<Step title="Start recording" icon="circle-dot">

Press the toolbar button, then **Start recording**.

</Step>

</Steps>

<Note>

The first time, the browser asks to let the extension "read and change your data on all websites". That is only so
the recording can follow links from page to page; say no and it still records the page you started on.

</Note>

### Safari

Safari is not built or tested: no converter is installed here. The route is Apple's converter, then Xcode:

```sh Terminal
xcrun safari-web-extension-converter dist/chrome --app-name "seleniumbase-rs Companion" --project-location ../safari
```

Build the generated project and turn the extension on in Safari's settings. Expect to review the manifest it
reports: Safari has no `sidePanel` and no DevTools Recorder, and may treat optional host access differently.

## Develop

```sh Terminal
mise run build       # or: bun run scripts/build.ts
mise run test        # bun test: selectors, code generators, steps, manifests, build output
bun run typecheck    # tsc --noEmit, strict, on the tooling and on the extension sources
mise run lint        # manifest rules, then web-ext lint on both dists
mise run e2e         # a real Chrome driven from Rust (see "End-to-end tests" below)
```

## Layout and extension points

<LayoutTree>
  <LayoutDir name="src" open>
    <LayoutDir name="manifest">
      <LayoutFile name="base.json" note={"manifest keys for every browser; base.json's \"chrome\" and \"firefox\" keys are per browser"} />
    </LayoutDir>
    <LayoutDir name="shared" open note="no browser APIs except api.ts: selector generation, code generation, the step model">
      <LayoutFile name="selectors.ts" highlight note="generateSelectors(): candidates in order of preference, each with its match count" />
      <LayoutFile name="steps.ts" highlight note="the step model, merging typing, masking secrets, sanitising incoming steps" />
      <LayoutDir name="codegen">
        <LayoutFile name="cdp.ts" />
        <LayoutFile name="basecase.ts" />
        <LayoutFile name="replay.ts" note="DevTools Recorder JSON, both ways" />
        <LayoutFile name="common.ts" />
        <LayoutFile name="index.ts" />
      </LayoutDir>
      <LayoutFile name="rust.ts" note="rustStr() and friends: everything recorded goes into Rust through these" />
      <LayoutFile name="fingerprint.ts" note="the page report as API v2 Fingerprint JSON; the export file name" />
      <LayoutFile name="fingerprint-file.ts" note="reads a Fingerprint file (v2 yes, flat v1 refused) and compares it with a page" />
      <LayoutFile name="calls.ts" />
      <LayoutFile name="protocol.ts" highlight />
      <LayoutFile name="api.ts" />
      <LayoutFile name="dom.ts" />
      <LayoutFile name="download.ts" />
      <LayoutFile name="clipboard.ts" />
      <LayoutFile name="pickview.ts" />
      <LayoutFile name="guards.ts" />
    </LayoutDir>
    <LayoutDir name="background" note="the router: registerHandler(type, handler, from); store.ts helpers" />
    <LayoutDir name="recorder" note="one folder per feature (see below)" />
    <LayoutDir name="inspector" note="one folder per feature (see below)" />
    <LayoutDir name="devtools-recorder" note="one folder per feature (see below)" />
    <LayoutDir name="ui" note="the panel (popup, side panel, sidebar) and its views" />
    <LayoutDir name="bridge" note="placeholder: a disabled feature waiting for its code" />
    <LayoutDir name="converter" note="placeholder: a disabled feature waiting for its code" />
  </LayoutDir>
  <LayoutDir name="scripts" open>
    <LayoutFile name="build.ts" highlight note="src/ to dist/<browser>/ and zips" />
  </LayoutDir>
  <LayoutDir name="tests" note="bun test" />
  <LayoutDir name="e2e" note="a Rust crate that loads dist/chrome-e2e into Chrome" />
</LayoutTree>

**A feature is a folder with a `feature.json`.** The build finds `src/*/feature.json` (a folder with
`"enabled": false` is skipped) and, for each browser it lists (`"browsers"`, both by default):

* bundles `background` (a module) into the one background bundle, after `src/background/index.ts`;
* bundles each file of `scripts` on its own, as a classic script, to `dist/<browser>/<feature>/<file>`
  (content scripts, page scripts, a DevTools page script);
* bundles `panelViews` (a module that calls `registerView` from `src/ui/registry.ts`) into the panel;
* copies every other file (html, css, images) as it is, except `feature.json`, `*.md`, files starting with `_`;
* merges `manifest` (and `chrome` / `firefox`) into that browser's manifest. Arrays are joined without
  repeats, objects are merged, and two different values for one key stop the build, so features cannot
  silently override one another.

So **to add a page**, put the `.html` and its script in the feature folder, list the script under `scripts`, and
open it with `ext.runtime.getURL("<feature>/page.html")`; to make it reachable from a web page, declare
`web_accessible_resources` in the feature's `manifest`. **To add a permission**, put it in the feature's
`manifest` (`permissions`, `optional_permissions`, `optional_host_permissions`, ...) and add it to the allow
list in `scripts/lint.ts` on purpose. Static `host_permissions` and `content_scripts` are not used: the extension
injects its scripts into the tab you point it at (`activeTab`), or into any tab once you allow all sites.

**Messages** (`src/shared/protocol.ts`): `{ v: 1, type: "<feature>/<name>", ...payload }`, answered with
`{ ok: true, ...data }` or `{ ok: false, error }`. The panel talks to the background with
`ext.runtime.sendMessage`; a content script does the same; the background talks to a content script with
`ext.tabs.sendMessage`. A feature adds a type from its background module:

```js
import { registerHandler, broadcast } from "../background/router.ts";
registerHandler("bridge/connect", async (message, sender) => ({ connected: true }), "ui"); // "ui" | "content" | "any"
```

The router refuses a message from the wrong kind of sender. After a change, `broadcast(MSG.STATE_CHANGED, { area })`
makes the panel's `ctx.onChange(area, fn)` run. Anything a feature keeps goes through `src/background/store.ts`
(`local` survives a restart, `session` does not), and the background must not hold state in memory: the browser
stops and starts it.

### Placeholder features

Two features are disabled placeholders that wait for their code.

<AccordionGroup>

<Accordion title="bridge: the live bridge to a running seleniumbase-rs process" icon="plug">

The live bridge between the extension and a running seleniumbase-rs process. Nothing here yet.

To build it:

1. Change `feature.json` to describe the feature (remove `"enabled": false`), for example

   ```json
   {
     "background": "background.ts",
     "panelViews": "view.ts",
     "manifest": { "optional_permissions": ["nativeMessaging"] }
   }
   ```

2. `background.ts` registers its message types with `registerHandler` (`../background/router.ts`); message type
   names start with `bridge/` and are added to `src/shared/protocol.ts`.
3. `view.ts` calls `registerView` (`../ui/registry.ts`) to add a tab to the panel.
4. Any permission is added to the allow list in `scripts/lint.ts` on purpose, and explained in the README.

See "Layout and extension points" above for what the build does with each field.

</Accordion>

<Accordion title="converter: the in-browser converter" icon="right-left">

The in-browser converter (Selenium IDE, Playwright, Puppeteer and Python SeleniumBase recordings to
seleniumbase-rs Rust). Nothing here yet.

To build it: turn on `feature.json` (remove `"enabled": false`), put the conversion in a module under
`src/shared/` if the recorder or the DevTools Recorder export should use it too (they already share
`src/shared/codegen/`: a converter that produces steps — see `src/shared/steps.ts` — gets all three export
formats for free), add a panel tab with `"panelViews"`, and register any message types with `registerHandler`.

See "Layout and extension points" above.

</Accordion>

</AccordionGroup>

## Security

<Check>

No remote code, no `eval` or `new Function` (the build and the tests check the output), a strict extension
CSP (`script-src 'self'`), no static host permissions, no `content_scripts` in the manifest. Page-derived text
is only ever put in the DOM as text, and into Rust only through `rustStr`. A content script's messages are
validated and bounded (`sanitizeStep`) before they are stored.

</Check>

## Limits

<Warning>

Steps inside iframes and shadow roots are not recorded (a shadow host is recorded as the target). The recorder
follows the tab you started in, not other tabs. Only Chrome 155, Edge 155 and Firefox ESR 153.4 were run; see
"End-to-end tests" below for what was and was not exercised in a real browser (the toolbar popup, the permission prompt,
the side panel and the DevTools Recorder's Export menu were not).

</Warning>

## End-to-end tests

A Rust crate (`sbrs-companion-e2e`) that loads the built extension into a real Chrome (and Edge) through
seleniumbase-rs's pure CDP engine, and a shell check that loads it into a real Firefox in Docker.

```sh Terminal
bun run scripts/build.ts --e2e      # dist/chrome, dist/firefox, dist/chrome-e2e, dist/firefox-e2e
cd e2e
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo test --test e2e                                       # parity with ActionRecorder; no browser
cargo test --test e2e -- --ignored --test-threads=1         # the browser tests, one at a time
```

The crate also reads the Page tab's JSON and the shared accepted and refused fingerprint files with
seleniumbase-rs 0.5.0, so the extension and the library agree on which files are valid, and drives Export and
Import in Chrome.

<Tip>

Run the browser tests one at a time: each starts its own browser and takes a minute or two.

</Tip>

### Why `dist/chrome-e2e`

The extension asks for host access to all sites as an *optional* permission, at the moment you press
**Start recording**, and otherwise relies on `activeTab`, which is granted by clicking the toolbar button.
A headless browser has no toolbar and cannot answer the permission prompt. `dist/chrome-e2e` and
`dist/firefox-e2e` are the same build plus `host_permissions` for `http://127.0.0.1/*` and
`http://localhost/*`; the build test checks that this is the only difference. The tests also load
`dist/chrome` (without it) and check that it says plainly that it has no access to the page.

### How the extension is driven

* The extension is loaded with `Extensions.loadUnpacked` (branded Chrome ignores `--load-extension` since 137;
  the browser is started with `--enable-unsafe-extension-debugging`).
* A headless browser has no popup. The popup's page, `ui/panel.html`, is opened as an ordinary tab, and the
  messages the popup's buttons send are sent from it with `chrome.runtime.sendMessage`: the same background
  code answers them. What is *not* exercised this way is the popup chrome itself (the toolbar button, the
  permission prompt, the side panel's opening).
* The page being recorded gets real input events from `Input.dispatchMouseEvent` / `dispatchKeyEvent`
  (`Locator::click`, `type_text`, `press`), which the page sees as trusted events, as a person's input is.

### What each test checks

<AccordionGroup>

<Accordion title="parity_with_the_action_recorder" icon="flask" description="Browser: none">

**Checks:** `tests/golden/*.rs.txt` equal what `ActionRecorder` writes; the bun tests hold the extension's generators to the same files

</Accordion>

<Accordion title="records_exports_and_replays_in_chrome" icon="flask" description="Browser: Chrome">

**Checks:** records a flow with real input (typing, a password, a select, a checkbox, a button, a form submit across two pages, a link); the steps; the password never recorded; the three exports; the exported pure CDP test and the BaseCase test **compile** in a scratch crate; the pure CDP test **runs** against the same pages (headless) and the pages log the same events as during the recording

</Accordion>

<Accordion title="inspector_selectors_match_one_element_each" icon="flask" description="Browser: Chrome">

**Checks:** picks 21 awkward elements with the real picker (real clicks); every selector it calls unique matches exactly that element in the page; no generated-looking class or id is used; the page report, parsed into seleniumbase-rs's `Fingerprint`; the build without host access; the DevTools Recorder plugin's `stringify` on a Recorder JSON; hostile text through the bundled generators, and the results **compile**

</Accordion>

<Accordion title="edge_loads_the_extension_and_records" icon="flask" description="Browser: Edge">

**Checks:** Edge accepts `Extensions.loadUnpacked`, the background answers, a click is recorded

</Accordion>

<Accordion title="firefox/check.sh" icon="flask" description="Browser: Firefox ESR (Docker)">

**Checks:** geckodriver installs `dist/firefox-e2e` as a temporary add-on; the background runs; the recorder starts and records typing and a click made through WebDriver; the export. WebDriver may not open a `moz-extension:` address, so the panel page is shown in an iframe of a loopback page (the e2e build lists it as web-accessible for `127.0.0.1`), and the fixture tab is found by trying small tab ids

</Accordion>

</AccordionGroup>

### Firefox

```sh Terminal
docker build -t sbrs-firefox-check /path/to/seleniumbase-rs/docker/firefox-check
docker run --rm --shm-size 512m \
  -v "$PWD/dist/firefox-e2e:/ext:ro" -v "$PWD/e2e/firefox/check.sh:/check.sh:ro" \
  sbrs-firefox-check /bin/sh /check.sh
docker rmi sbrs-firefox-check
```

### What a headless run cannot show

<Warning>

The side panel, the Firefox sidebar, the toolbar popup as a popup, the permission prompt, and the DevTools
Recorder panel's **Export** menu (the plugin is registered only inside a real DevTools window; the tests call
the registered plugin object). Safari: no converter is installed here; the route is
`xcrun safari-web-extension-converter dist/chrome` followed by a build in Xcode.

</Warning>

## Licence

MIT.
