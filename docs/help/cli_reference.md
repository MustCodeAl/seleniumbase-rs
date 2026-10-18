---
title: "CLI reference"
sidebarTitle: "CLI reference"
description: "Every sbrs command with its arguments and flags, generated from the parser."
---

<Info>

Generated from the usage spec of `sbrs` (`completions/sbrs.usage.kdl`) by `SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`. Do not edit it by hand. For the common commands by task, see the [CLI TL;DR](/help/cli_tldr).

</Info>

<Badge>sbrs 0.5.1 (seleniumbase-rs)</Badge>

A Rust port of the Python SeleniumBase testing framework. Provides browser automation, stealth/undetected modes, and a command-line helper.

```bash Usage
sbrs [FLAGS] <SUBCOMMAND>
```

## Commands

| Command | What it does |
|---|---|
| [`sbrs accept-alert`](#sbrs-accept-alert) | Accept the active browser alert. |
| [`sbrs add-text`](#sbrs-add-text) | Append text to an input or textarea. |
| [`sbrs assert-attribute`](#sbrs-assert-attribute) | Assert an element attribute equals an expected value. |
| [`sbrs assert-element`](#sbrs-assert-element) | Assert that an element exists. |
| [`sbrs assert-link-text`](#sbrs-assert-link-text) | Assert a link with the given text is visible. |
| [`sbrs assert-pdf-text`](#sbrs-assert-pdf-text) | Assert that a page or PDF URL contains the expected text. |
| [`sbrs assert-text-not-visible`](#sbrs-assert-text-not-visible) | Assert that an element does not contain the given text. |
| [`sbrs assert-text-visible`](#sbrs-assert-text-visible) | Assert that an element contains the expected text. |
| [`sbrs assert-title`](#sbrs-assert-title) | Assert the page title contains the expected text. |
| [`sbrs behave-gui`](#sbrs-behave-gui) | Launch the Behave Commander, a terminal app (needs a build with the `tui` feature). |
| [`sbrs cache-clear`](#sbrs-cache-clear) | Clear the browser cache via CDP. |
| [`sbrs caseplans`](#sbrs-caseplans) | Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature). |
| [`sbrs cdp`](#sbrs-cdp) | Execute a Chrome DevTools Protocol command. |
| [`sbrs cdp-click-element`](#sbrs-cdp-click-element) | Click an element using CDP. |
| [`sbrs cdp-type-text`](#sbrs-cdp-type-text) | Type text using CDP input injection. |
| [`sbrs check-if-unchecked`](#sbrs-check-if-unchecked) | Check a checkbox only if it is unchecked. |
| [`sbrs choose-file`](#sbrs-choose-file) | Upload a local file into a file input. |
| [`sbrs clear-cookies`](#sbrs-clear-cookies) | Delete all browser cookies. |
| [`sbrs clear-local-storage`](#sbrs-clear-local-storage) | Clear the page local storage. |
| [`sbrs click-partial-link-text`](#sbrs-click-partial-link-text) | Click a link containing the given text. |
| [`sbrs click-visible-elements`](#sbrs-click-visible-elements) | Click all visible elements matching a selector. |
| [`sbrs close-window`](#sbrs-close-window) | Close the current browser window. |
| [`sbrs commander`](#sbrs-commander) | Launch the interactive commander (needs a build with the `tui` feature). |
| [`sbrs completions`](#sbrs-completions) | Generate a completion script for a supported shell, or a Fig spec. |
| [`sbrs decrypt`](#sbrs-decrypt) | Decrypt a token made by `sbrs encrypt`; read from stdin if TOKEN is omitted. Python's `sbrs unobfuscate` is the same command. |
| [`sbrs deferred`](#sbrs-deferred) | Run several assertions, then report every failure at once. |
| [`sbrs devtools`](#sbrs-devtools) | Launch Chrome with an open DevTools endpoint and print MCP client configs. |
| [`sbrs dismiss-alert`](#sbrs-dismiss-alert) | Dismiss the active browser alert. |
| [`sbrs doctor`](#sbrs-doctor) | Run a diagnostic check on the environment and configuration. |
| [`sbrs download-file`](#sbrs-download-file) | Download a file from a URL (browser download with an HTTP fallback). |
| [`sbrs drag-and-drop`](#sbrs-drag-and-drop) | Drag one element onto another. |
| [`sbrs encrypt`](#sbrs-encrypt) | Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted. |
| [`sbrs export-recording`](#sbrs-export-recording) | Export the recorded test actions. |
| [`sbrs get-active-element-css`](#sbrs-get-active-element-css) | Print a CSS selector for the currently focused element. |
| [`sbrs get-alert-text`](#sbrs-get-alert-text) | Print the text of the active alert. |
| [`sbrs get-attribute`](#sbrs-get-attribute) | Print an element attribute value. |
| [`sbrs get-cookies`](#sbrs-get-cookies) | Print all browser cookies as JSON. |
| [`sbrs get-current-url`](#sbrs-get-current-url) | Print the current URL. |
| [`sbrs get-local-storage-item`](#sbrs-get-local-storage-item) | Print a local storage value by key. |
| [`sbrs get-property`](#sbrs-get-property) | Print an element property value. |
| [`sbrs get-text`](#sbrs-get-text) | Print the visible text of an element. |
| [`sbrs get-title`](#sbrs-get-title) | Print the page title. |
| [`sbrs get-value`](#sbrs-get-value) | Print the value of a form element. |
| [`sbrs get-window-position`](#sbrs-get-window-position) | Print the browser window position. |
| [`sbrs go-back`](#sbrs-go-back) | Navigate back in browser history. |
| [`sbrs go-forward`](#sbrs-go-forward) | Navigate forward in browser history. |
| [`sbrs grid-hub`](#sbrs-grid-hub) | Start, stop, restart or check a Selenium Grid hub. |
| [`sbrs grid-node`](#sbrs-grid-node) | Start, stop, restart or check a Selenium Grid node. |
| [`sbrs highlight-click`](#sbrs-highlight-click) | Highlight an element, then click it. |
| [`sbrs hover`](#sbrs-hover) | Hover over an element. |
| [`sbrs hover-and-click`](#sbrs-hover-and-click) | Hover over one element, then click another. |
| [`sbrs human-click`](#sbrs-human-click) | Click an element with human-like timing and noise. |
| [`sbrs human-type`](#sbrs-human-type) | Type text with human-like timing and noise. |
| [`sbrs import-playwright`](#sbrs-import-playwright) | Convert Playwright tests or a script (JavaScript, TypeScript or Python) into Rust tests for the pure CDP engine. |
| [`sbrs import-puppeteer`](#sbrs-import-puppeteer) | Convert Puppeteer scripts or tests (JavaScript, TypeScript) or a Chrome Recorder JSON export into Rust tests for the pure CDP engine. |
| [`sbrs import-python`](#sbrs-import-python) | Convert common SeleniumBase or Selenium Python code into a Rust test. |
| [`sbrs install`](#sbrs-install) | Download a driver into ./downloaded_drivers/ (Python's `sbrs get`). |
| [`sbrs is-checked`](#sbrs-is-checked) | Print whether a checkbox is checked. |
| [`sbrs is-element-visible`](#sbrs-is-element-visible) | Print whether an element is visible. |
| [`sbrs is-link-text-visible`](#sbrs-is-link-text-visible) | Print whether a link with the given text is visible. |
| [`sbrs is-partial-link-text-visible`](#sbrs-is-partial-link-text-visible) | Print whether a link containing the given text is visible. |
| [`sbrs is-text-visible`](#sbrs-is-text-visible) | Print whether the expected text is visible in an element. |
| [`sbrs load-cookies`](#sbrs-load-cookies) | Load browser cookies from a JSON file. |
| [`sbrs mkchart`](#sbrs-mkchart) | Create an HTML chart. |
| [`sbrs mkdir`](#sbrs-mkdir) | Create a folder with a runnable browser-test suite. |
| [`sbrs mkfile`](#sbrs-mkfile) | Create a browser-test file. |
| [`sbrs mkpres`](#sbrs-mkpres) | Create an HTML presentation. |
| [`sbrs mkrec`](#sbrs-mkrec) | Create a test recording scaffold. |
| [`sbrs objectify`](#sbrs-objectify) | Objectify a test recording. |
| [`sbrs open`](#sbrs-open) | Open a URL in the browser. |
| [`sbrs open-new-tab`](#sbrs-open-new-tab) | Open a new browser tab. |
| [`sbrs open-new-window`](#sbrs-open-new-window) | Open a new browser window. |
| [`sbrs patch-chrome`](#sbrs-patch-chrome) | Patch a Chrome/Chromium binary for native-level spoofing. |
| [`sbrs patch-chromedriver`](#sbrs-patch-chromedriver) | Patch a chromedriver binary to reduce detection surface. |
| [`sbrs pdf-text`](#sbrs-pdf-text) | Print the text extracted from a page or PDF URL. |
| [`sbrs plugin`](#sbrs-plugin) | List the built-in plugins, or show what one declares. |
| [`sbrs plugin info`](#sbrs-plugin-info) | Show what one plugin declares and what it may do. |
| [`sbrs plugin list`](#sbrs-plugin-list) | List the built-in plugins of this build, and whether they are on. |
| [`sbrs print`](#sbrs-print) | Print a file. |
| [`sbrs print-pdf`](#sbrs-print-pdf) | Print a page to PDF and save it to disk. |
| [`sbrs prompt`](#sbrs-prompt) | Print one short line for a shell prompt: the installed Chrome's major version and whether a matching chromedriver is at hand. |
| [`sbrs record`](#sbrs-record) | Record browser interactions into a replayable action file. |
| [`sbrs refresh`](#sbrs-refresh) | Reload the current page. |
| [`sbrs remove-local-storage-item`](#sbrs-remove-local-storage-item) | Remove a local storage entry by key. |
| [`sbrs replay`](#sbrs-replay) | Replay a recorded action file against a fresh browser session. |
| [`sbrs run-scenario`](#sbrs-run-scenario) | Run a scenario file and write an optional dashboard. |
| [`sbrs save-cookies`](#sbrs-save-cookies) | Save browser cookies to a JSON file. |
| [`sbrs save-source`](#sbrs-save-source) | Save the current page source to a file. |
| [`sbrs screenshot`](#sbrs-screenshot) | Capture a screenshot of the current page. |
| [`sbrs select-option`](#sbrs-select-option) | Select an option from a dropdown. |
| [`sbrs send-keys`](#sbrs-send-keys) | Send keystrokes to an element. |
| [`sbrs set-local-storage-item`](#sbrs-set-local-storage-item) | Set a local storage key/value pair. |
| [`sbrs set-window-position`](#sbrs-set-window-position) | Set the browser window position. |
| [`sbrs smoke`](#sbrs-smoke) | Run a smoke test against a URL. |
| [`sbrs smooth-scroll-to`](#sbrs-smooth-scroll-to) | Smoothly scroll an element into view. |
| [`sbrs switch-to-default-content`](#sbrs-switch-to-default-content) | Switch focus back to the main document. |
| [`sbrs switch-to-default-window`](#sbrs-switch-to-default-window) | Switch focus to the default window. |
| [`sbrs switch-to-frame`](#sbrs-switch-to-frame) | Switch focus to an iframe. |
| [`sbrs switch-to-newest-window`](#sbrs-switch-to-newest-window) | Switch focus to the newest browser window. |
| [`sbrs switch-to-parent-frame`](#sbrs-switch-to-parent-frame) | Switch focus to the parent frame. |
| [`sbrs switch-to-window`](#sbrs-switch-to-window) | Switch focus to a window by handle. |
| [`sbrs test`](#sbrs-test) | Run the project's tests, or one example, through Cargo. |
| [`sbrs throttle3g`](#sbrs-throttle3g) | Throttle the browser connection to 3G speeds. |
| [`sbrs translate`](#sbrs-translate) | Write the actions of a JSON scenario in another language. |
| [`sbrs type-alert-text`](#sbrs-type-alert-text) | Type text into the active alert prompt. |
| [`sbrs uc-click`](#sbrs-uc-click) | Perform a UC-mode click on an element. |
| [`sbrs uc-type`](#sbrs-uc-type) | Perform UC-mode typing into an element. |
| [`sbrs uncheck-if-checked`](#sbrs-uncheck-if-checked) | Uncheck a checkbox only if it is checked. |
| [`sbrs version`](#sbrs-version) | Print the sbrs version, as `sbrs --version` does. |
| [`sbrs wait-for-and-accept-alert`](#sbrs-wait-for-and-accept-alert) | Wait for an alert, then accept it. |
| [`sbrs wait-for-and-dismiss-alert`](#sbrs-wait-for-and-dismiss-alert) | Wait for an alert, then dismiss it. |
| [`sbrs wait-for-element-not-visible`](#sbrs-wait-for-element-not-visible) | Wait until an element is no longer visible. |
| [`sbrs wait-for-element-present`](#sbrs-wait-for-element-present) | Wait until an element is present in the DOM. |
| [`sbrs wait-for-ready-state-complete`](#sbrs-wait-for-ready-state-complete) | Wait until the document readyState is complete. |
| [`sbrs wait-for-text`](#sbrs-wait-for-text) | Wait until an element contains the expected text. |

## Global flags

They go before the command: `sbrs --headless open URL`.

<ParamField query="--webdriver <URL>" type="string">

WebDriver server URL (default: SB_WEBDRIVER_URL, else http://localhost:4444)

</ParamField>

<ParamField query="--browser <BROWSER>" type="string">

Browser engine to launch (default: chrome, or the config file's)

One of: `chrome`, `chromium`, `edge`, `firefox`, `brave`, `comet`, `opera`, `safari`, `ie`, `cft`, `chs`

</ParamField>

<ParamField query="--headed" type="boolean">

Run the browser in headed mode

</ParamField>

<ParamField query="--headless" type="boolean">

Run the browser in headless mode

</ParamField>

<ParamField query="--cdp" type="boolean">

Enable Chrome DevTools Protocol mode

</ParamField>

<ParamField query="--uc" type="boolean">

Enable undetected-chromedriver style evasions

</ParamField>

<ParamField query="--user-agent <USER_AGENT>" type="string">

Override the default user agent string

</ParamField>

<ParamField query="--agent <AGENT>" type="string">

Alias for --user-agent

Also: `-a`

</ParamField>

<ParamField query="--locale <LOCALE>" type="string">

Locale string for the browser context

</ParamField>

<ParamField query="--ad-block" type="boolean">

Refuse the requests of ad and tracker hosts (Chromium WebDriver sessions)

</ParamField>

<ParamField query="--proxy <PROXY>" type="string">

Proxy (host:port or scheme://host:port); with --proxy-list, a name from the list

</ParamField>

<ParamField query="--proxy-list <FILE>" type="string">

File of proxies to choose from; without --proxy a random one is used

</ParamField>

<ParamField query="--proxy-pac-url <PROXY_PAC_URL>" type="string">

URL to a proxy auto-config (PAC) file

</ParamField>

<ParamField query="--user-data-dir <USER_DATA_DIR>" type="string">

Path to a persistent browser user-data directory

</ParamField>

<ParamField query="--extension-dir <EXTENSION_DIR>" type="string">

Path to an unpacked extension directory

</ParamField>

<ParamField query="--reuse-session" type="boolean">

Share browsers among the tests of the run (SB_REUSE_SESSION)

Also: `--reuse_session`, `--rs`

</ParamField>

<ParamField query="--rcs" type="boolean">

Share a browser among the tests of one Rust module (SB_REUSE_CLASS_SESSION)

Also: `--reuse-class-session`, `--reuse_class_session`

</ParamField>

<ParamField query="--mobile" type="boolean">

Use a mobile device emulation profile

</ParamField>

<ParamField query="--esc-end" type="boolean">

End the current test when ESC is pressed in the browser

Also: `--esc_end`, `--ee`

</ParamField>

<ParamField query="--rec-sb-cdp" type="boolean">

Write recordings as pure CDP code (sb_cdp Page and Locator calls)

Also: `--rec_sb_cdp`

</ParamField>

<ParamField query="--threads <THREADS>" type="string">

Number of parallel threads for test runs

Also: `-n`

</ParamField>

<ParamField query="--config <CONFIG>" type="string">

Path to a TOML/JSON settings file

Also: `-c`

</ParamField>

<ParamField query="--plugin <NAME>" type="string[]">

Turn on a built-in plugin in the browsers the command starts (repeatable)

Adds to the plugins the settings file (the [plugins] table) and SB_PLUGINS turn on. Precedence, highest first: the command line, then SB_PLUGINS (which replaces the names the file enables), then the settings file. `sbrs plugin list` shows the plugins this build has. `sbrs test` passes the names on to the tests as SB_PLUGINS.

</ParamField>

<ParamField query="--plugin-opt <NAME.KEY=VALUE>" type="string[]">

Set one option of a plugin, NAME.KEY=VALUE with a TOML VALUE (repeatable)

Quote the whole option so the shell keeps the quotes of a string: --plugin-opt 'shield.level="strict"'. An option given here replaces the same option from SB_PLUGINS or the settings file and keeps the plugin's other options, so the precedence is the command line, then SB_PLUGINS, then the file. The plugin must be turned on by --plugin, SB_PLUGINS or the file. Not available for `sbrs test`: put the options in the [plugins.NAME] table of the settings file.

</ParamField>

<ParamField query="--help" type="boolean">

Print help

Also: `-h`

</ParamField>

<ParamField query="--version" type="boolean">

Print version

Also: `-V`

</ParamField>

## sbrs accept-alert

Accept the active browser alert.

```bash Usage
sbrs accept-alert
```

## sbrs add-text

Append text to an input or textarea.

```bash Usage
sbrs add-text <--css <CSS>> <--text <TEXT>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--text <TEXT>" type="string" required>

Text to append to the element

</ParamField>

## sbrs assert-attribute

Assert an element attribute equals an expected value.

```bash Usage
sbrs assert-attribute <FLAGS>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--attribute <ATTRIBUTE>" type="string" required>

Attribute name to read or assert

</ParamField>

<ParamField query="--value <VALUE>" type="string" required>

Expected attribute value

</ParamField>

## sbrs assert-element

Assert that an element exists.

```bash Usage
sbrs assert-element <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs assert-link-text

Assert a link with the given text is visible.

```bash Usage
sbrs assert-link-text <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Exact link text to assert

</ParamField>

## sbrs assert-pdf-text

Assert that a page or PDF URL contains the expected text.

```bash Usage
sbrs assert-pdf-text <--text <TEXT>> [--save-pdf <SAVE_PDF>] <URL>
```

**Arguments**

<ParamField path="URL" type="string" required>

URL of the page or PDF to check

</ParamField>

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Text that must appear in the PDF

</ParamField>

<ParamField query="--save-pdf <SAVE_PDF>" type="string">

Also keep the intermediate PDF at this path

</ParamField>

## sbrs assert-text-not-visible

Assert that an element does not contain the given text.

```bash Usage
sbrs assert-text-not-visible <--css <CSS>> <--text <TEXT>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--text <TEXT>" type="string" required>

Text that should not be visible

</ParamField>

## sbrs assert-text-visible

Assert that an element contains the expected text.

```bash Usage
sbrs assert-text-visible <--css <CSS>> <--text <TEXT>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--text <TEXT>" type="string" required>

Expected visible text

</ParamField>

## sbrs assert-title

Assert the page title contains the expected text.

```bash Usage
sbrs assert-title <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Expected page title substring

</ParamField>

## sbrs behave-gui

Also: `gui-behave`

Launch the Behave Commander, a terminal app (needs a build with the `tui` feature).

Lists the `.feature` files under PATH with their scenarios, runs the ones you mark through the crate's behave runner and shows the results. PATH is a folder of feature files or one file; by default `features`.

The runner has only the built-in steps: steps written in Rust are not loaded, and a scenario that uses one is reported as undefined. The Python command opens a Tkinter window; this one runs in the terminal.

```bash Usage
sbrs behave-gui [--new] [PATH]
```

**Arguments**

<ParamField path="PATH" type="string">

A folder of feature files, or one `.feature` file.

</ParamField>

**Flags**

<ParamField query="--new" type="boolean">

Prompt for a name, a title and scenarios and write a new feature file instead; this needs no `tui` feature.

</ParamField>

## sbrs cache-clear

Clear the browser cache via CDP.

```bash Usage
sbrs cache-clear
```

## sbrs caseplans

Also: `case-plans`

Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature).

Lists the tests under PATH and creates, edits and saves a case plan for each, `case_plans/<test>.md` next to its file, and writes a summary of the plans to `case_summary.md`. PATH is a folder of tests or one test file; by default the `tests/` and `examples/` folders of the current folder.

The Python command opens a Tkinter window; this one runs in the terminal.

```bash Usage
sbrs caseplans [--starter] [PATH]
```

**Arguments**

<ParamField path="PATH" type="string">

A folder of tests, or one test file.

</ParamField>

**Flags**

<ParamField query="--starter" type="boolean">

Write the starter checklist `CASE_PLANS.md` instead; this needs no `tui` feature.

</ParamField>

## sbrs cdp

Execute a Chrome DevTools Protocol command.

```bash Usage
sbrs cdp <--cmd <CMD>> [--params <PARAMS>]
```

**Flags**

<ParamField query="--cmd <CMD>" type="string" required>

CDP method name (e.g. Runtime.evaluate)

</ParamField>

<ParamField query="--params <PARAMS>" type="string">

JSON object of CDP parameters

</ParamField>

## sbrs cdp-click-element

Click an element using CDP.

```bash Usage
sbrs cdp-click-element <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs cdp-type-text

Type text using CDP input injection.

```bash Usage
sbrs cdp-type-text <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Text to inject via CDP

</ParamField>

## sbrs check-if-unchecked

Check a checkbox only if it is unchecked.

```bash Usage
sbrs check-if-unchecked <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs choose-file

Upload a local file into a file input.

```bash Usage
sbrs choose-file [--url <URL>] <SELECTOR> <FILE>
```

**Arguments**

<ParamField path="SELECTOR" type="string" required>

CSS selector of the &lt;input type=file> element

</ParamField>

<ParamField path="FILE" type="string" required>

Local file to upload

</ParamField>

**Flags**

<ParamField query="--url <URL>" type="string">

Page to open first (defaults to a generated blank upload page)

</ParamField>

## sbrs clear-cookies

Delete all browser cookies.

```bash Usage
sbrs clear-cookies
```

## sbrs clear-local-storage

Clear the page local storage.

```bash Usage
sbrs clear-local-storage
```

## sbrs click-partial-link-text

Click a link containing the given text.

```bash Usage
sbrs click-partial-link-text <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Link text substring to click

</ParamField>

## sbrs click-visible-elements

Click all visible elements matching a selector.

```bash Usage
sbrs click-visible-elements <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs close-window

Close the current browser window.

```bash Usage
sbrs close-window
```

## sbrs commander

Launch the interactive commander (needs a build with the `tui` feature).

```bash Usage
sbrs commander
```

## sbrs completions

Generate a completion script for a supported shell, or a Fig spec.

```bash Usage
sbrs completions <SHELL>
```

**Arguments**

<ParamField path="SHELL" type="string" required>

Shell (or `fig` for a Fig spec) to generate completions for

One of: `bash`, `zsh`, `fish`, `elvish`, `powershell`, `fig`, `usage`

</ParamField>

## sbrs decrypt

Also: `unobfuscate`

Decrypt a token made by `sbrs encrypt`; read from stdin if TOKEN is omitted. Python's `sbrs unobfuscate` is the same command.

```bash Usage
sbrs decrypt [TOKEN]
```

**Arguments**

<ParamField path="TOKEN" type="string">

The token to decrypt.

</ParamField>

## sbrs deferred

Run several assertions, then report every failure at once.

```bash Usage
sbrs deferred [FLAGS]
```

**Flags**

<ParamField query="--url <URL>" type="string">

URL to open before running the assertions

</ParamField>

<ParamField query="--assert <SPEC>" type="string[]">

Assertion spec: element=&lt;css>, text=&lt;css>:&lt;text>, title=&lt;text>, or url=&lt;text>

</ParamField>

<ParamField query="--spec <SPEC>" type="string">

JSON file containing an array of assertion specs

</ParamField>

## sbrs devtools

Launch Chrome with an open DevTools endpoint and print MCP client configs.

Starts Chrome with the Pure CDP engine (no chromedriver), prints the DevTools HTTP endpoint, the browser WebSocket URL and configs for Google's chrome-devtools-mcp and Microsoft's Playwright MCP, then keeps the browser running until Ctrl+C, which closes it and exits with 0. The global --browser (chrome, chromium, edge or another Chromium browser), --headed, --user-agent, --locale, --ad-block, --extension-dir and --proxy-list apply as well.

```bash Usage
sbrs devtools [FLAGS]
```

**Flags**

<ParamField query="--headless" type="boolean">

Run the browser without a window

</ParamField>

<ParamField query="--port <PORT>" type="string" default="9222">

DevTools port on 127.0.0.1 (0 lets Chrome pick a free one)

</ParamField>

<ParamField query="--user-data-dir <DIR>" type="string">

Profile directory to keep (default: a throwaway one; never Chrome's own default profile)

</ParamField>

<ParamField query="--proxy <PROXY>" type="string">

Proxy: SERVER:PORT, USER:PASS@SERVER:PORT or scheme://SERVER:PORT

</ParamField>

<ParamField query="--browser-path <PATH>" type="string">

Browser binary to launch, such as Microsoft Edge or another Chromium browser

</ParamField>

<ParamField query="--url <URL>" type="string">

Page to open once the browser runs

</ParamField>

<ParamField query="--json" type="boolean">

Print the endpoint and configs as one line of JSON instead of text

</ParamField>

## sbrs dismiss-alert

Dismiss the active browser alert.

```bash Usage
sbrs dismiss-alert
```

## sbrs doctor

Run a diagnostic check on the environment and configuration.

```bash Usage
sbrs doctor
```

## sbrs download-file

Download a file from a URL (browser download with an HTTP fallback).

```bash Usage
sbrs download-file [-o --output <OUTPUT>] [--timeout <TIMEOUT>] <URL>
```

**Arguments**

<ParamField path="URL" type="string" required>

URL of the file to download

</ParamField>

**Flags**

<ParamField query="--output <OUTPUT>" type="string">

Output file path (defaults to the URL file name in the current directory)

Also: `-o`

</ParamField>

<ParamField query="--timeout <TIMEOUT>" type="string" default="30">

Seconds to wait for the browser download before falling back to HTTP

</ParamField>

## sbrs drag-and-drop

Drag one element onto another.

```bash Usage
sbrs drag-and-drop <--source-css <SOURCE_CSS>> <--target-css <TARGET_CSS>>
```

**Flags**

<ParamField query="--source-css <SOURCE_CSS>" type="string" required>

CSS selector of the element to drag

</ParamField>

<ParamField query="--target-css <TARGET_CSS>" type="string" required>

CSS selector of the drop target

</ParamField>

## sbrs encrypt

Also: `obfuscate`

Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted.

The passphrase comes from the SB_ENCRYPTION_KEY environment variable, never from an argument, so it stays out of shell history and process listings. Python's `sbrs obfuscate` is the same command.

```bash Usage
sbrs encrypt [TEXT]
```

**Arguments**

<ParamField path="TEXT" type="string">

The text to encrypt.

</ParamField>

## sbrs export-recording

Export the recorded test actions.

```bash Usage
sbrs export-recording
```

## sbrs get-active-element-css

Print a CSS selector for the currently focused element.

```bash Usage
sbrs get-active-element-css
```

## sbrs get-alert-text

Print the text of the active alert.

```bash Usage
sbrs get-alert-text
```

## sbrs get-attribute

Print an element attribute value.

```bash Usage
sbrs get-attribute <--css <CSS>> <--attribute <ATTRIBUTE>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--attribute <ATTRIBUTE>" type="string" required>

Attribute name to read or assert

</ParamField>

## sbrs get-cookies

Print all browser cookies as JSON.

```bash Usage
sbrs get-cookies
```

## sbrs get-current-url

Print the current URL.

```bash Usage
sbrs get-current-url
```

## sbrs get-local-storage-item

Print a local storage value by key.

```bash Usage
sbrs get-local-storage-item <--key <KEY>>
```

**Flags**

<ParamField query="--key <KEY>" type="string" required>

Storage key

</ParamField>

## sbrs get-property

Print an element property value.

```bash Usage
sbrs get-property <--css <CSS>> <--property <PROPERTY>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--property <PROPERTY>" type="string" required>

Property name to read

</ParamField>

## sbrs get-text

Print the visible text of an element.

```bash Usage
sbrs get-text <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs get-title

Print the page title.

```bash Usage
sbrs get-title
```

## sbrs get-value

Print the value of a form element.

```bash Usage
sbrs get-value <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs get-window-position

Print the browser window position.

```bash Usage
sbrs get-window-position
```

## sbrs go-back

Navigate back in browser history.

```bash Usage
sbrs go-back
```

## sbrs go-forward

Navigate forward in browser history.

```bash Usage
sbrs go-forward
```

## sbrs grid-hub

Start, stop, restart or check a Selenium Grid hub.

Needs a Selenium Server jar you supply; nothing is downloaded. The process record and log are kept in a folder of your own (SB_GRID_DIR).

```bash Usage
sbrs grid-hub [FLAGS] <ACTION> [-- EXTRA_ARGS]…
```

**Arguments**

<ParamField path="ACTION" type="string" required>

What to do with the Grid process.

One of: `start`, `stop`, `restart`, `status`

</ParamField>

<ParamField path="EXTRA_ARGS" type="string[]">

More arguments for the Selenium Server, after `--`.

</ParamField>

**Flags**

<ParamField query="--jar <PATH>" type="string">

The Selenium Server jar (default: the SB_SELENIUM_SERVER_JAR variable). It is never downloaded.

</ParamField>

<ParamField query="--java <PATH>" type="string">

The Java executable (default: `java` on PATH).

</ParamField>

<ParamField query="--grid-host <HOST>" type="string">

The address to listen on.

</ParamField>

<ParamField query="--grid-port <PORT>" type="string">

The port to listen on (default: 4444 for a hub, 5555 for a node).

</ParamField>

<ParamField query="--hub <HOST_OR_URL>" type="string">

For a node, the hub to register with: a host or a URL.

</ParamField>

<ParamField query="--session-timeout <SECONDS>" type="string">

Close sessions that have been idle this many seconds.

</ParamField>

<ParamField query="--wait <SECONDS>" type="string" default="60">

How many seconds `start` waits for the Grid to become ready.

</ParamField>

<ParamField query="--grid-verbose" type="boolean">

Log at INFO instead of WARNING.

</ParamField>

<ParamField query="--skip-port-check" type="boolean">

Start even if something already accepts connections on the port.

</ParamField>

## sbrs grid-node

Start, stop, restart or check a Selenium Grid node.

A node registers with the hub named by `--hub`, or with this machine's.

```bash Usage
sbrs grid-node [FLAGS] <ACTION> [-- EXTRA_ARGS]…
```

**Arguments**

<ParamField path="ACTION" type="string" required>

What to do with the Grid process.

One of: `start`, `stop`, `restart`, `status`

</ParamField>

<ParamField path="EXTRA_ARGS" type="string[]">

More arguments for the Selenium Server, after `--`.

</ParamField>

**Flags**

<ParamField query="--jar <PATH>" type="string">

The Selenium Server jar (default: the SB_SELENIUM_SERVER_JAR variable). It is never downloaded.

</ParamField>

<ParamField query="--java <PATH>" type="string">

The Java executable (default: `java` on PATH).

</ParamField>

<ParamField query="--grid-host <HOST>" type="string">

The address to listen on.

</ParamField>

<ParamField query="--grid-port <PORT>" type="string">

The port to listen on (default: 4444 for a hub, 5555 for a node).

</ParamField>

<ParamField query="--hub <HOST_OR_URL>" type="string">

For a node, the hub to register with: a host or a URL.

</ParamField>

<ParamField query="--session-timeout <SECONDS>" type="string">

Close sessions that have been idle this many seconds.

</ParamField>

<ParamField query="--wait <SECONDS>" type="string" default="60">

How many seconds `start` waits for the Grid to become ready.

</ParamField>

<ParamField query="--grid-verbose" type="boolean">

Log at INFO instead of WARNING.

</ParamField>

<ParamField query="--skip-port-check" type="boolean">

Start even if something already accepts connections on the port.

</ParamField>

## sbrs highlight-click

Highlight an element, then click it.

```bash Usage
sbrs highlight-click <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs hover

Hover over an element.

```bash Usage
sbrs hover <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs hover-and-click

Hover over one element, then click another.

```bash Usage
sbrs hover-and-click <--hover-css <HOVER_CSS>> <--click-css <CLICK_CSS>>
```

**Flags**

<ParamField query="--hover-css <HOVER_CSS>" type="string" required>

CSS selector to hover over

</ParamField>

<ParamField query="--click-css <CLICK_CSS>" type="string" required>

CSS selector to click

</ParamField>

## sbrs human-click

Click an element with human-like timing and noise.

```bash Usage
sbrs human-click <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs human-type

Type text with human-like timing and noise.

```bash Usage
sbrs human-type <--css <CSS>> <--text <TEXT>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--text <TEXT>" type="string" required>

Text to type with humanized timing

</ParamField>

## sbrs import-playwright

Also: `import-pw`

Convert Playwright tests or a script (JavaScript, TypeScript or Python) into Rust tests for the pure CDP engine.

```bash Usage
sbrs import-playwright [FLAGS] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

Playwright source file to convert.

</ParamField>

**Flags**

<ParamField query="--output <OUTPUT>" type="string">

Output file for the generated Rust tests

Also: `-o`

</ParamField>

<ParamField query="--language <LANGUAGE>" type="string">

Source language (default: from the file extension, else from the content)

One of: `js`, `ts`, `python`

</ParamField>

<ParamField query="--test-name <TEST_NAME>" type="string">

Name of the test generated for a script without test titles (default: the file name)

</ParamField>

## sbrs import-puppeteer

Also: `import-pptr`

Convert Puppeteer scripts or tests (JavaScript, TypeScript) or a Chrome Recorder JSON export into Rust tests for the pure CDP engine.

```bash Usage
sbrs import-puppeteer [FLAGS] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

Puppeteer source file (or Recorder `.json` export) to convert.

</ParamField>

**Flags**

<ParamField query="--output <OUTPUT>" type="string">

Output file for the generated Rust tests

Also: `-o`

</ParamField>

<ParamField query="--language <LANGUAGE>" type="string">

Source language (default: from the file extension, else from the content)

One of: `js`, `ts`, `json`

</ParamField>

<ParamField query="--test-name <TEST_NAME>" type="string">

Name of the test generated for a script without test titles (default: the file name)

</ParamField>

## sbrs import-python

Convert common SeleniumBase or Selenium Python code into a Rust test.

```bash Usage
sbrs import-python [FLAGS] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

Python source file to convert.

</ParamField>

**Flags**

<ParamField query="--output <OUTPUT>" type="string">

Output file for the generated Rust test

Also: `-o`

</ParamField>

<ParamField query="--source <SOURCE>" type="string" default="auto">

Source API to assume when converting Python code

One of: `auto`, `selenium-base`, `selenium`

</ParamField>

<ParamField query="--test-name <TEST_NAME>" type="string">

Name of the generated test function

</ParamField>

## sbrs install

Also: `get`

Download a driver into ./downloaded_drivers/ (Python's `sbrs get`).

Without a version, chromedriver and edgedriver are the drivers for the installed Chrome and Edge, and geckodriver is the newest release.

```bash Usage
sbrs install [DRIVER] [VERSION]
```

**Arguments**

<ParamField path="DRIVER" type="string" default="chromedriver">

The driver: chromedriver, geckodriver (or firefoxdriver), edgedriver (or msedgedriver).

</ParamField>

<ParamField path="VERSION" type="string">

`latest`, a major version (chromedriver, edgedriver), a full build, or a geckodriver version such as 0.36.0.

</ParamField>

## sbrs is-checked

Print whether a checkbox is checked.

```bash Usage
sbrs is-checked <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs is-element-visible

Print whether an element is visible.

```bash Usage
sbrs is-element-visible <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs is-link-text-visible

Print whether a link with the given text is visible.

```bash Usage
sbrs is-link-text-visible <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Exact link text to check

</ParamField>

## sbrs is-partial-link-text-visible

Print whether a link containing the given text is visible.

```bash Usage
sbrs is-partial-link-text-visible <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Link text substring to check

</ParamField>

## sbrs is-text-visible

Print whether the expected text is visible in an element.

```bash Usage
sbrs is-text-visible <--css <CSS>> <--text <TEXT>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--text <TEXT>" type="string" required>

Expected text to check

</ParamField>

## sbrs load-cookies

Load browser cookies from a JSON file.

```bash Usage
sbrs load-cookies <--file <FILE>>
```

**Flags**

<ParamField query="--file <FILE>" type="string" required>

JSON file to load cookies from

</ParamField>

## sbrs mkchart

Create an HTML chart.

```bash Usage
sbrs mkchart <--file <FILE>>
```

**Flags**

<ParamField query="--file <FILE>" type="string" required>

Output HTML chart path

</ParamField>

## sbrs mkdir

Create a folder with a runnable browser-test suite.

Names are relative to the current directory and use letters, digits, `_`, `-` and `.`, with `/` between folders. Nothing is overwritten unless `--force` is given.

```bash Usage
sbrs mkdir [-b --basic] [--force] <DIR>
```

**Arguments**

<ParamField path="DIR" type="string" required>

Folder to create, for example `tests/ui`.

</ParamField>

**Flags**

<ParamField query="--basic" type="boolean">

Create only the scaffolding, with no example tests.

Also: `-b`

</ParamField>

<ParamField query="--force" type="boolean">

Replace generated files that already exist.

</ParamField>

## sbrs mkfile

Also: `new`

Create a browser-test file.

The name follows the same rules as `mkdir`; `.rs` is added when missing. An existing file is never overwritten unless `--force` is given.

```bash Usage
sbrs mkfile [FLAGS] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

File to create, for example `tests/login.rs`.

</ParamField>

**Flags**

<ParamField query="--url <URL>" type="string">

Page the test opens (default: https://example.com).

</ParamField>

<ParamField query="--basic" type="boolean">

Leave the test body empty: it only opens the page.

Also: `-b`

</ParamField>

<ParamField query="--force" type="boolean">

Replace the file if it already exists.

</ParamField>

## sbrs mkpres

Create an HTML presentation.

```bash Usage
sbrs mkpres <--file <FILE>>
```

**Flags**

<ParamField query="--file <FILE>" type="string" required>

Output HTML presentation path

</ParamField>

## sbrs mkrec

Create a test recording scaffold.

```bash Usage
sbrs mkrec <--file <FILE>>
```

**Flags**

<ParamField query="--file <FILE>" type="string" required>

Output recording scaffold path

</ParamField>

## sbrs objectify

Objectify a test recording.

```bash Usage
sbrs objectify
```

## sbrs open

Open a URL in the browser.

```bash Usage
sbrs open <URL>
```

**Arguments**

<ParamField path="URL" type="string" required>

URL to open in the browser.

</ParamField>

## sbrs open-new-tab

Open a new browser tab.

```bash Usage
sbrs open-new-tab
```

## sbrs open-new-window

Open a new browser window.

```bash Usage
sbrs open-new-window
```

## sbrs patch-chrome

Patch a Chrome/Chromium binary for native-level spoofing.

```bash Usage
sbrs patch-chrome <--path <PATH>> [--cache-dir <CACHE_DIR>]
```

**Flags**

<ParamField query="--path <PATH>" type="string" required>

Path to the Chrome/Chromium binary

</ParamField>

<ParamField query="--cache-dir <CACHE_DIR>" type="string">

Directory where patched binaries are cached

</ParamField>

## sbrs patch-chromedriver

Patch a chromedriver binary to reduce detection surface.

```bash Usage
sbrs patch-chromedriver <--path <PATH>>
```

**Flags**

<ParamField query="--path <PATH>" type="string" required>

Path to the chromedriver binary

</ParamField>

## sbrs pdf-text

Print the text extracted from a page or PDF URL.

```bash Usage
sbrs pdf-text [--save-pdf <SAVE_PDF>] <URL>
```

**Arguments**

<ParamField path="URL" type="string" required>

URL of the page or PDF to extract text from

</ParamField>

**Flags**

<ParamField query="--save-pdf <SAVE_PDF>" type="string">

Also keep the intermediate PDF at this path

</ParamField>

## sbrs plugin

List the built-in plugins, or show what one declares.

Plugins run inside the browsers a command starts. Turn them on by name with the global options --plugin and --plugin-opt, the SB_PLUGINS variable, or the [plugins] table of the settings file; the command line wins over SB_PLUGINS, which wins over the file. `sbrs plugin list` shows the plugins this build has.

```bash Usage
sbrs plugin <SUBCOMMAND>
```

### sbrs plugin info

Show what one plugin declares and what it may do.

Prints the plugin's category, phase, capabilities (with their risk, and whether the host enforces them), what it needs, its place among the other plugins and the options it was given. Exits with status 2 and lists the known names when there is no such plugin.

```bash Usage
sbrs plugin info <NAME>
```

**Arguments**

<ParamField path="NAME" type="string" required>

The plugin's name, as `sbrs plugin list` shows it.

</ParamField>

### sbrs plugin list

List the built-in plugins of this build, and whether they are on.

Prints each plugin's name, category and summary, and `yes` under ENABLED when the settings file, SB_PLUGINS or the global --plugin option turns it on. A build without built-in plugins says so; rebuild it with --features plugins-builtin.

```bash Usage
sbrs plugin list
```

## sbrs print

Print a file.

```bash Usage
sbrs print <--file <FILE>>
```

**Flags**

<ParamField query="--file <FILE>" type="string" required>

File path to print

</ParamField>

## sbrs print-pdf

Print a page to PDF and save it to disk.

```bash Usage
sbrs print-pdf [-o --output <OUTPUT>] <URL>
```

**Arguments**

<ParamField path="URL" type="string" required>

URL to render as a PDF

</ParamField>

**Flags**

<ParamField query="--output <OUTPUT>" type="string" default="page.pdf">

Output PDF file path

Also: `-o`

</ParamField>

## sbrs prompt

Print one short line for a shell prompt: the installed Chrome's major version and whether a matching chromedriver is at hand.

Prints `chrome 155 ✓driver`, with `✗driver` when the chromedriver is missing or of another version and `?driver` when its version cannot be read. It starts no browser and uses no network: versions come from files (a macOS app's Info.plist, the version folder beside chrome.exe), and one that only `--version` tells is cached for ten minutes in the user cache folder. Prints nothing and exits with 0 when no browser is found; --verbose says why, on standard error. The chromedriver is looked for as `sbrs doctor` does: CHROMEDRIVER_PATH, downloaded_drivers/, PATH.

```bash Usage
sbrs prompt [FLAGS]
```

**Flags**

<ParamField query="--format <TEMPLATE>" type="string" default="{browser} {major} {driver}">

The line to print; placeholders \{browser\} \{major\} \{driver\} \{mark\} \{driver_major\}, \{\{ and \}\} for braces

</ParamField>

<ParamField query="--ascii" type="boolean">

Mark the driver with + - ? instead of ✓ ✗ ?

</ParamField>

<ParamField query="--browser-path <PATH>" type="string">

Browser to report on (default: SB_CHROME_BIN, else the installed Chrome or Chromium)

</ParamField>

<ParamField query="--no-cache" type="boolean">

Run --version again instead of using the versions cached in the user cache folder

</ParamField>

<ParamField query="--verbose" type="boolean">

Explain on standard error what was found and how

</ParamField>

## sbrs record

Record browser interactions into a replayable action file.

```bash Usage
sbrs record [-o --output <OUTPUT>] [--duration <DURATION>] <URL>
```

**Arguments**

<ParamField path="URL" type="string" required>

URL to open and record

</ParamField>

**Flags**

<ParamField query="--output <OUTPUT>" type="string" default="actions.json">

Output action file (.json or .yaml)

Also: `-o`

</ParamField>

<ParamField query="--duration <DURATION>" type="string">

Stop recording after this many seconds (default: run until Ctrl-C)

</ParamField>

## sbrs refresh

Reload the current page.

```bash Usage
sbrs refresh
```

## sbrs remove-local-storage-item

Remove a local storage entry by key.

```bash Usage
sbrs remove-local-storage-item <--key <KEY>>
```

**Flags**

<ParamField query="--key <KEY>" type="string" required>

Storage key

</ParamField>

## sbrs replay

Replay a recorded action file against a fresh browser session.

```bash Usage
sbrs replay <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

JSON action file produced by `sbrs record`

</ParamField>

## sbrs run-scenario

Run a scenario file and write an optional dashboard.

```bash Usage
sbrs run-scenario <--file <FILE>> [--dashboard <DASHBOARD>]
```

**Flags**

<ParamField query="--file <FILE>" type="string" required>

Scenario file to run

</ParamField>

<ParamField query="--dashboard <DASHBOARD>" type="string">

Optional path to write a scenario dashboard

</ParamField>

## sbrs save-cookies

Save browser cookies to a JSON file.

```bash Usage
sbrs save-cookies <--file <FILE>>
```

**Flags**

<ParamField query="--file <FILE>" type="string" required>

JSON file to save cookies to

</ParamField>

## sbrs save-source

Save the current page source to a file.

```bash Usage
sbrs save-source [--path <PATH>]
```

**Flags**

<ParamField query="--path <PATH>" type="string">

Output HTML file path (default: a new file in latest_logs/)

</ParamField>

## sbrs screenshot

Capture a screenshot of the current page.

```bash Usage
sbrs screenshot [--path <PATH>]
```

**Flags**

<ParamField query="--path <PATH>" type="string">

Output image file path (default: a new file in latest_logs/)

</ParamField>

## sbrs select-option

Select an option from a dropdown.

```bash Usage
sbrs select-option <FLAGS>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--text <TEXT>" type="string">

Visible text of the option to select

</ParamField>

<ParamField query="--value <VALUE>" type="string">

Option value attribute

</ParamField>

## sbrs send-keys

Send keystrokes to an element.

```bash Usage
sbrs send-keys <--css <CSS>> <--text <TEXT>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--text <TEXT>" type="string" required>

Keys to send to the element

</ParamField>

## sbrs set-local-storage-item

Set a local storage key/value pair.

```bash Usage
sbrs set-local-storage-item <--key <KEY>> <--value <VALUE>>
```

**Flags**

<ParamField query="--key <KEY>" type="string" required>

Storage key

</ParamField>

<ParamField query="--value <VALUE>" type="string" required>

Value to store in local storage

</ParamField>

## sbrs set-window-position

Set the browser window position.

```bash Usage
sbrs set-window-position <--x <X>> <--y <Y>>
```

**Flags**

<ParamField query="--x <X>" type="string" required>

Window X coordinate in pixels

</ParamField>

<ParamField query="--y <Y>" type="string" required>

Window Y coordinate in pixels

</ParamField>

## sbrs smoke

Run a smoke test against a URL.

```bash Usage
sbrs smoke [--title-contains <TITLE_CONTAINS>] <URL>
```

**Arguments**

<ParamField path="URL" type="string" required>

URL to open or smoke-test

</ParamField>

**Flags**

<ParamField query="--title-contains <TITLE_CONTAINS>" type="string">

Expected substring in the page title

</ParamField>

## sbrs smooth-scroll-to

Smoothly scroll an element into view.

```bash Usage
sbrs smooth-scroll-to <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs switch-to-default-content

Switch focus back to the main document.

```bash Usage
sbrs switch-to-default-content
```

## sbrs switch-to-default-window

Switch focus to the default window.

```bash Usage
sbrs switch-to-default-window
```

## sbrs switch-to-frame

Switch focus to an iframe.

```bash Usage
sbrs switch-to-frame <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs switch-to-newest-window

Switch focus to the newest browser window.

```bash Usage
sbrs switch-to-newest-window
```

## sbrs switch-to-parent-frame

Switch focus to the parent frame.

```bash Usage
sbrs switch-to-parent-frame
```

## sbrs switch-to-window

Switch focus to a window by handle.

```bash Usage
sbrs switch-to-window <--handle <HANDLE>>
```

**Flags**

<ParamField query="--handle <HANDLE>" type="string" required>

Window handle to switch to

</ParamField>

## sbrs test

Run the project's tests, or one example, through Cargo.

The global options reach the tests as SB_* environment variables (for example `sbrs --headless --browser firefox test` sets SB_HEADLESS and SB_BROWSER), so a test that loads `Settings` sees them, and `-n` sets how many tests run at once. `--plugin` reaches them as SB_PLUGINS, with the plugins the settings file and the variable already turn on; `--plugin-opt` cannot be passed on, so `sbrs test` refuses it.

```bash Usage
sbrs test [FLAGS] [FILTER] [-- HARNESS_ARGS]…
```

**Arguments**

<ParamField path="FILTER" type="string">

Run only the tests whose name contains this text.

</ParamField>

<ParamField path="HARNESS_ARGS" type="string[]">

Arguments for the test harness, after `--` (for example --nocapture).

</ParamField>

**Flags**

<ParamField query="--test <NAME>" type="string">

Run one integration test file from tests/.

</ParamField>

<ParamField query="--example <NAME>" type="string">

Run one example from examples/ instead of the tests.

</ParamField>

<ParamField query="--release" type="boolean">

Build with the release profile.

</ParamField>

<ParamField query="--features <FEATURES>" type="string[]">

Cargo features to enable, separated by commas.

</ParamField>

## sbrs throttle3g

Throttle the browser connection to 3G speeds.

```bash Usage
sbrs throttle3g
```

## sbrs translate

Write the actions of a JSON scenario in another language.

Rewrites each step's `action` (`click`, `open`, ...) with the translation dictionary and changes nothing else in the file; the steps keep running, because a scenario can be read in any of the languages. Python SeleniumBase translates Python test files; a Rust test is compiled code, so this translates the portable JSON scenario instead.

```bash Usage
sbrs translate [FLAGS] <FILE> <LANGUAGE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

The scenario file.

</ParamField>

<ParamField path="LANGUAGE" type="string" required>

The language: a code (en, zh, nl, fr, it, ja, ko, pt, ru, es) or its English name.

</ParamField>

**Flags**

<ParamField query="--print" type="boolean">

Print the translation and write nothing (the default).

Also: `-p`

</ParamField>

<ParamField query="--overwrite" type="boolean">

Replace the file with the translation.

Also: `-o`

</ParamField>

<ParamField query="--copy" type="boolean">

Write the translation to a new file next to it, such as `login_es.json`.

Also: `-c`

</ParamField>

## sbrs type-alert-text

Type text into the active alert prompt.

```bash Usage
sbrs type-alert-text <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Text to type into the alert prompt

</ParamField>

## sbrs uc-click

Perform a UC-mode click on an element.

```bash Usage
sbrs uc-click <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs uc-type

Perform UC-mode typing into an element.

```bash Usage
sbrs uc-type <--css <CSS>> <--text <TEXT>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--text <TEXT>" type="string" required>

Text to type in UC mode

</ParamField>

## sbrs uncheck-if-checked

Uncheck a checkbox only if it is checked.

```bash Usage
sbrs uncheck-if-checked <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

## sbrs version

Print the sbrs version, as `sbrs --version` does.

```bash Usage
sbrs version
```

## sbrs wait-for-and-accept-alert

Wait for an alert, then accept it.

```bash Usage
sbrs wait-for-and-accept-alert [--timeout <TIMEOUT>]
```

**Flags**

<ParamField query="--timeout <TIMEOUT>" type="string" default="10">

Maximum wait time in seconds

</ParamField>

## sbrs wait-for-and-dismiss-alert

Wait for an alert, then dismiss it.

```bash Usage
sbrs wait-for-and-dismiss-alert [--timeout <TIMEOUT>]
```

**Flags**

<ParamField query="--timeout <TIMEOUT>" type="string" default="10">

Maximum wait time in seconds

</ParamField>

## sbrs wait-for-element-not-visible

Wait until an element is no longer visible.

```bash Usage
sbrs wait-for-element-not-visible <--css <CSS>> [--timeout <TIMEOUT>]
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--timeout <TIMEOUT>" type="string" default="10">

Maximum wait time in seconds

</ParamField>

## sbrs wait-for-element-present

Wait until an element is present in the DOM.

```bash Usage
sbrs wait-for-element-present <--css <CSS>> [--timeout <TIMEOUT>]
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--timeout <TIMEOUT>" type="string" default="10">

Maximum wait time in seconds

</ParamField>

## sbrs wait-for-ready-state-complete

Wait until the document readyState is complete.

```bash Usage
sbrs wait-for-ready-state-complete
```

## sbrs wait-for-text

Wait until an element contains the expected text.

```bash Usage
sbrs wait-for-text <FLAGS>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

<ParamField query="--text <TEXT>" type="string" required>

Expected text substring to wait for

</ParamField>

<ParamField query="--timeout <TIMEOUT>" type="string" default="10">

Maximum wait time in seconds

</ParamField>

