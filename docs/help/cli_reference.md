---
title: "CLI reference"
sidebarTitle: "CLI reference"
description: "Every sbrs command with its arguments and flags, generated from the parser."
---

<Info>

Generated from the usage spec of `sbrs` (`completions/sbrs.usage.kdl`) by `SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`. Do not edit it by hand. For the common commands by task, see the [CLI TL;DR](/help/cli_tldr).

</Info>

<Badge>sbrs 0.6.0 (seleniumbase-rs)</Badge>

A Rust port of the Python SeleniumBase testing framework. Provides browser automation, stealth/undetected modes, and a command-line helper.

```bash Usage
sbrs [FLAGS] <SUBCOMMAND>
```

## Commands

| Command | What it does |
|---|---|
| [`sbrs accept-alert`](#sbrs-accept-alert) | Accept the alert that the page opens when it loads. |
| [`sbrs add-text`](#sbrs-add-text) | Append text to an input or textarea. |
| [`sbrs assert-attribute`](#sbrs-assert-attribute) | Assert an element attribute equals an expected value. |
| [`sbrs assert-element`](#sbrs-assert-element) | Assert that an element exists. |
| [`sbrs assert-link-text`](#sbrs-assert-link-text) | Assert a link with the given text is visible. |
| [`sbrs assert-pdf-text`](#sbrs-assert-pdf-text) | Assert that a page or PDF URL contains the expected text. |
| [`sbrs assert-text-not-visible`](#sbrs-assert-text-not-visible) | Assert that an element does not contain the given text. |
| [`sbrs assert-text-visible`](#sbrs-assert-text-visible) | Assert that an element contains the expected text. |
| [`sbrs assert-title`](#sbrs-assert-title) | Assert the page title contains the expected text. |
| [`sbrs behave-gui`](#sbrs-behave-gui) | Launch the Behave Commander, a terminal app (needs a build with the `tui` feature). |
| [`sbrs behave-options`](#sbrs-behave-options) | Print how Python SeleniumBase's behave options map to `sbrs test`. |
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
| [`sbrs commander`](#sbrs-commander) | Launch the Commander, a terminal app (needs a build with the `tui` feature). |
| [`sbrs completions`](#sbrs-completions) | Generate a completion script for a supported shell, or a Fig spec. |
| [`sbrs convert`](#sbrs-convert) | Convert an exported Selenium WebDriver test (Python) into a Rust test. |
| [`sbrs decrypt`](#sbrs-decrypt) | Decrypt a token made by `sbrs encrypt`; read from stdin if TOKEN is omitted. Python's `sbrs unobfuscate` is the same command. |
| [`sbrs deferred`](#sbrs-deferred) | Run several assertions, then report every failure at once. |
| [`sbrs devtools`](#sbrs-devtools) | Launch Chrome with an open DevTools endpoint and print MCP client configs. |
| [`sbrs dismiss-alert`](#sbrs-dismiss-alert) | Dismiss the alert that the page opens when it loads. |
| [`sbrs doctor`](#sbrs-doctor) | Run a diagnostic check on the environment and configuration. |
| [`sbrs download`](#sbrs-download) | Download Selenium Server 4, the jar that `grid-hub` and `grid-node` run. |
| [`sbrs download server`](#sbrs-download-server) | Download the Selenium Server 4 jar that `grid-hub` and `grid-node` run. |
| [`sbrs download-file`](#sbrs-download-file) | Download a file from a URL (browser download with an HTTP fallback). |
| [`sbrs drag-and-drop`](#sbrs-drag-and-drop) | Drag one element onto another. |
| [`sbrs encrypt`](#sbrs-encrypt) | Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted. |
| [`sbrs extract-objects`](#sbrs-extract-objects) | Move the selectors of a Rust test into page objects in `page_objects.rs`. |
| [`sbrs fingerprint`](#sbrs-fingerprint) | Capture, write, check and compare browser fingerprints. |
| [`sbrs fingerprint capture`](#sbrs-fingerprint-capture) | Read the fingerprint of a real browser and write it as a fingerprint file. |
| [`sbrs fingerprint check`](#sbrs-fingerprint-check) | Check that a fingerprint file loads and is coherent. |
| [`sbrs fingerprint diff`](#sbrs-fingerprint-diff) | Compare two fingerprint files. |
| [`sbrs fingerprint export`](#sbrs-fingerprint-export) | Write a preset or a random identity as a fingerprint file. |
| [`sbrs fingerprint presets`](#sbrs-fingerprint-presets) | List the presets and what each claims. |
| [`sbrs get-alert-text`](#sbrs-get-alert-text) | Print the text of the alert that the page opens when it loads. |
| [`sbrs get-attribute`](#sbrs-get-attribute) | Print an element attribute value. |
| [`sbrs get-cookies`](#sbrs-get-cookies) | Print all browser cookies as JSON. |
| [`sbrs get-current-url`](#sbrs-get-current-url) | Print the current URL. |
| [`sbrs get-local-storage-item`](#sbrs-get-local-storage-item) | Print a local storage value by key. |
| [`sbrs get-property`](#sbrs-get-property) | Print an element property value. |
| [`sbrs get-text`](#sbrs-get-text) | Print the visible text of an element. |
| [`sbrs get-title`](#sbrs-get-title) | Print the page title. |
| [`sbrs get-value`](#sbrs-get-value) | Print the value of a form element. |
| [`sbrs get-window-position`](#sbrs-get-window-position) | Print the browser window position. |
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
| [`sbrs inject-objects`](#sbrs-inject-objects) | Replace the selectors of a Rust test with the page objects of `page_objects.rs`. |
| [`sbrs install`](#sbrs-install) | Download a driver or a browser into ./downloaded_drivers/ (Python's `sbrs get`). |
| [`sbrs is-checked`](#sbrs-is-checked) | Print whether a checkbox is checked. |
| [`sbrs is-element-visible`](#sbrs-is-element-visible) | Print whether an element is visible. |
| [`sbrs is-link-text-visible`](#sbrs-is-link-text-visible) | Print whether a link with the given text is visible. |
| [`sbrs is-partial-link-text-visible`](#sbrs-is-partial-link-text-visible) | Print whether a link containing the given text is visible. |
| [`sbrs is-text-visible`](#sbrs-is-text-visible) | Print whether the expected text is visible in an element. |
| [`sbrs load-cookies`](#sbrs-load-cookies) | Load browser cookies from a JSON file. |
| [`sbrs methods`](#sbrs-methods) | List the common methods of the test API, as they are called on `sb`. |
| [`sbrs mkchart`](#sbrs-mkchart) | Create a test that shows a chart on a slide. |
| [`sbrs mkdir`](#sbrs-mkdir) | Create a folder with a runnable browser-test suite. |
| [`sbrs mkfile`](#sbrs-mkfile) | Create a browser-test file. |
| [`sbrs mkpres`](#sbrs-mkpres) | Create a test that builds a slide deck. |
| [`sbrs mkrec`](#sbrs-mkrec) | Create a new test file with the Recorder: record your actions in a browser, then save them as a test. |
| [`sbrs objectify`](#sbrs-objectify) | Turn the selectors of a Rust test into page objects: extract-objects, then inject-objects. |
| [`sbrs open`](#sbrs-open) | Open a URL in the browser. |
| [`sbrs options`](#sbrs-options) | List the browser options: each flag, its `SB_*` variable and its key in the settings file. |
| [`sbrs patch-chrome`](#sbrs-patch-chrome) | Patch a Chrome/Chromium binary for native-level spoofing. |
| [`sbrs patch-chromedriver`](#sbrs-patch-chromedriver) | Patch a chromedriver binary to reduce detection surface. |
| [`sbrs pdf-text`](#sbrs-pdf-text) | Print the text extracted from a page or PDF URL. |
| [`sbrs plugin`](#sbrs-plugin) | List the built-in plugins, or show what one declares. |
| [`sbrs plugin info`](#sbrs-plugin-info) | Show what one plugin declares and what it may do. |
| [`sbrs plugin list`](#sbrs-plugin-list) | List the built-in plugins of this build, and whether they are on. |
| [`sbrs plugin run`](#sbrs-plugin-run) | Run the task of a plugin on a browser it starts, and write its results as JSON lines. |
| [`sbrs print`](#sbrs-print) | Print a file, with a line number in front of every line if you ask for it. |
| [`sbrs print-pdf`](#sbrs-print-pdf) | Print a page to PDF and save it to disk. |
| [`sbrs prompt`](#sbrs-prompt) | Print one short line for a shell prompt: the installed Chrome's major version and whether a matching chromedriver is at hand. |
| [`sbrs proxy`](#sbrs-proxy) | Run a small forward proxy on this machine (HTTP and CONNECT, no authentication). |
| [`sbrs recorder`](#sbrs-recorder) | Launch the Recorder, a terminal app (needs a build with the `tui` feature). |
| [`sbrs remove-local-storage-item`](#sbrs-remove-local-storage-item) | Remove a local storage entry by key. |
| [`sbrs replay`](#sbrs-replay) | Replay a recorded action file (recordings/NAME_rec.json) against a fresh browser. |
| [`sbrs report`](#sbrs-report) | Finish a test run and write its reports, or show the runs of a results database. |
| [`sbrs report finish`](#sbrs-report-finish) | Finish the test run of the current folder and write its reports. |
| [`sbrs revert-objects`](#sbrs-revert-objects) | Put the selectors back where a Rust test uses page objects. |
| [`sbrs run-scenario`](#sbrs-run-scenario) | Run a scenario file, the way to do several steps in one browser, and write an optional dashboard. |
| [`sbrs save-cookies`](#sbrs-save-cookies) | Save browser cookies to a JSON file. |
| [`sbrs save-source`](#sbrs-save-source) | Save the current page source to a file. |
| [`sbrs screenshot`](#sbrs-screenshot) | Capture a screenshot of the current page. |
| [`sbrs select-option`](#sbrs-select-option) | Select an option from a dropdown. |
| [`sbrs send-keys`](#sbrs-send-keys) | Send keystrokes to an element. |
| [`sbrs set-local-storage-item`](#sbrs-set-local-storage-item) | Set a local storage key/value pair. |
| [`sbrs smoke`](#sbrs-smoke) | Run a smoke test against a URL. |
| [`sbrs smooth-scroll-to`](#sbrs-smooth-scroll-to) | Smoothly scroll an element into view. |
| [`sbrs solve-captcha`](#sbrs-solve-captcha) | Solve the CAPTCHA on the page and check that it cleared. |
| [`sbrs test`](#sbrs-test) | Run the project's tests, or one example, through Cargo or cargo-nextest. |
| [`sbrs translate`](#sbrs-translate) | Write the actions of a JSON scenario in another language. |
| [`sbrs type-alert-text`](#sbrs-type-alert-text) | Type text into the prompt that the page opens when it loads. |
| [`sbrs uc-click`](#sbrs-uc-click) | Perform a UC-mode click on an element. |
| [`sbrs uc-type`](#sbrs-uc-type) | Perform UC-mode typing into an element. |
| [`sbrs uncheck-if-checked`](#sbrs-uncheck-if-checked) | Uncheck a checkbox only if it is checked. |
| [`sbrs version`](#sbrs-version) | Print the sbrs version, as `sbrs --version` does. |
| [`sbrs wait-for-and-accept-alert`](#sbrs-wait-for-and-accept-alert) | Wait for an alert that the page opens, then accept it. |
| [`sbrs wait-for-and-dismiss-alert`](#sbrs-wait-for-and-dismiss-alert) | Wait for an alert that the page opens, then dismiss it. |
| [`sbrs wait-for-element-not-visible`](#sbrs-wait-for-element-not-visible) | Wait until an element is no longer visible. |
| [`sbrs wait-for-element-present`](#sbrs-wait-for-element-present) | Wait until an element is present in the DOM. |
| [`sbrs wait-for-ready-state-complete`](#sbrs-wait-for-ready-state-complete) | Wait until the document readyState is complete. |
| [`sbrs wait-for-text`](#sbrs-wait-for-text) | Wait until an element contains the expected text. |

## Global flags

Each applies to every command and goes before the command or after it, but not both: `sbrs --headless open URL` and `sbrs open URL --headless` are the same.

<ParamField query="--webdriver <URL>" type="string">

WebDriver server URL (default: SB_WEBDRIVER_URL, else http://localhost:4444)

</ParamField>

<ParamField query="--browser <BROWSER>" type="string">

Browser engine to launch (SB_BROWSER; default: chrome, or the config file's)

One of: `chrome`, `chromium`, `edge`, `firefox`, `brave`, `comet`, `opera`, `safari`, `ie`, `cft`, `chs`

</ParamField>

<ParamField query="--headed" type="boolean">

Run the browser in headed mode (SB_HEADLESS=false)

Also: `--gui`

</ParamField>

<ParamField query="--headless" type="boolean">

Run the browser in headless mode (SB_HEADLESS)

Also: `--headless2`

</ParamField>

<ParamField query="--cdp" type="boolean">

Enable Chrome DevTools Protocol mode (SB_MODE=cdp)

</ParamField>

<ParamField query="--uc" type="boolean">

Enable undetected-chromedriver style evasions (SB_MODE=uc)

Also: `--undetected`, `--undetectable`

</ParamField>

<ParamField query="--user-agent <USER_AGENT>" type="string">

Override the default user agent string (SB_USER_AGENT)

</ParamField>

<ParamField query="--agent <AGENT>" type="string">

Alias for --user-agent (SB_USER_AGENT)

Also: `-a`

</ParamField>

<ParamField query="--locale <LOCALE>" type="string">

Locale string for the browser context (SB_LOCALE)

</ParamField>

<ParamField query="--start-page <URL>" type="string">

Page the browser opens first, Python's --start-page (SB_START_PAGE; aliases --url and --start_page)

Every command starts its own browser and closes it at the end, so a command that acts on a page (get-text, click, screenshot, assert-element, ...) acts on this one: `sbrs --url https://example.com get-text --css p`. A bare host gets `https://` and a local file becomes a `file://` URL. The command line wins over SB_START_PAGE, which wins over the `start_page` key of the settings file. Commands that open a URL of their own (open, smoke, download-file, print-pdf, pdf-text, assert-pdf-text, record) refuse it.

Also: `--url`, `--start_page`

</ParamField>

<ParamField query="--save-screenshot" type="boolean">

Save a screenshot to latest_logs/ just before the browser closes, and print its path (SB_SAVE_SCREENSHOT)

Python's `--save-screenshot`. A command that acts on the page (click, type, ...) also prints where it ended up as `Now at: <url> (<title>)`.

Also: `--save_screenshot`

</ParamField>

<ParamField query="--ad-block" type="boolean">

Refuse the requests of ad and tracker hosts (SB_AD_BLOCK; Chromium WebDriver sessions)

Also: `--adblock`, `--block-ads`

</ParamField>

<ParamField query="--proxy <PROXY>" type="string">

Proxy (SB_PROXY; host:port or scheme://host:port); with --proxy-list, a name from the list

Also: `--proxy-server`, `--proxy-string`

</ParamField>

<ParamField query="--proxy-list <FILE>" type="string">

File of proxies to choose from; without --proxy a random one is used (SB_PROXY_LIST)

</ParamField>

<ParamField query="--proxy-pac-url <PROXY_PAC_URL>" type="string">

URL to a proxy auto-config (PAC) file (SB_PROXY_PAC_URL)

Also: `--pac-url`

</ParamField>

<ParamField query="--user-data-dir <USER_DATA_DIR>" type="string">

Path to a persistent browser user-data directory (SB_USER_DATA_DIR)

</ParamField>

<ParamField query="--extension-dir <EXTENSION_DIR>" type="string">

Path to an unpacked extension directory (SB_EXTENSION_DIR)

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

Use a mobile device emulation profile (SB_MOBILE)

Also: `--mobile-emulator`

</ParamField>

<ParamField query="--esc-end" type="boolean">

End the current test when ESC is pressed in the browser (SB_ESC_END)

Also: `--esc_end`, `--ee`

</ParamField>

<ParamField query="--rec-sb-cdp" type="boolean">

Write recordings as pure CDP code (SB_REC_SB_CDP; sb_cdp Page and Locator calls)

Also: `--rec_sb_cdp`

</ParamField>

<ParamField query="--config <CONFIG>" type="string">

Path to a TOML/JSON settings file (SB_SETTINGS_FILE)

Also: `--settings-file`, `--settings`, `-c`

</ParamField>

<ParamField query="--fingerprint <FILE>" type="string">

Make every browser the command starts present the fingerprint in FILE (SB_FINGERPRINT)

The file is the fingerprint JSON that `sbrs fingerprint capture` and `export` write. A Chromium browser then runs in the CDP mode unless --uc or another mode is given. The command line wins over SB_FINGERPRINT, which wins over the [fingerprint] table of the settings file. The user agent, language and proxy named by their own options win over the fingerprint's.

</ParamField>

<ParamField query="--plugin <NAME>" type="string[]">

Turn on a built-in plugin in the browsers the command starts (SB_PLUGINS; repeatable)

Adds to the plugins the settings file (the [plugins] table) and SB_PLUGINS turn on. Precedence, highest first: the command line, then SB_PLUGINS (which replaces the names the file enables), then the settings file. `sbrs plugin list` shows the plugins this build has. `sbrs test` passes the names on to the tests as SB_PLUGINS.

</ParamField>

<ParamField query="--plugin-opt <NAME.KEY=VALUE>" type="string[]">

Set one option of a plugin, NAME.KEY=VALUE with a TOML VALUE (repeatable)

Quote the whole option so the shell keeps the quotes of a string: --plugin-opt 'shield.level="strict"'. An option given here replaces the same option from SB_PLUGINS or the settings file and keeps the plugin's other options, so the precedence is the command line, then SB_PLUGINS, then the file. The plugin must be turned on by --plugin, SB_PLUGINS or the file. Not available for `sbrs test`: put the options in the [plugins.NAME] table of the settings file.

</ParamField>

<ParamField query="--binary-location <PATH>" type="string">

The browser binary to run (SB_CHROME_BIN)

Also: `--bl`

</ParamField>

<ParamField query="--mode <VALUE>" type="string">

The driver mode: webdriver, cdp or uc (SB_MODE)

</ParamField>

<ParamField query="--chrome [CHROME]" type="string">

Run Chrome (SB_BROWSER=chrome)

One of: `true`, `false`

</ParamField>

<ParamField query="--edge [EDGE]" type="string">

Run Edge (SB_BROWSER=edge)

One of: `true`, `false`

</ParamField>

<ParamField query="--firefox [FIREFOX]" type="string">

Run Firefox (SB_BROWSER=firefox)

One of: `true`, `false`

</ParamField>

<ParamField query="--brave [BRAVE]" type="string">

Run Brave (SB_BROWSER=brave)

One of: `true`, `false`

</ParamField>

<ParamField query="--comet [COMET]" type="string">

Run Comet (SB_BROWSER=comet)

One of: `true`, `false`

</ParamField>

<ParamField query="--opera [OPERA]" type="string">

Run Opera (SB_BROWSER=opera)

One of: `true`, `false`

</ParamField>

<ParamField query="--safari [SAFARI]" type="string">

Run Safari (SB_BROWSER=safari)

One of: `true`, `false`

</ParamField>

<ParamField query="--ie [IE]" type="string">

Run Internet Explorer (SB_BROWSER=ie)

One of: `true`, `false`

</ParamField>

<ParamField query="--cft [CFT]" type="string">

Run Chrome for Testing (SB_BROWSER=cft)

One of: `true`, `false`

</ParamField>

<ParamField query="--chs [CHS]" type="string">

Run chrome-headless-shell (SB_BROWSER=chs)

One of: `true`, `false`

</ParamField>

<ParamField query="--chromium [CHROMIUM]" type="string">

Run Chromium (SB_BROWSER=chromium)

Also: `--use-chromium`

One of: `true`, `false`

</ParamField>

<ParamField query="--headless1 [HEADLESS1]" type="string">

Run headless with chrome-headless-shell (Chrome's old headless mode) (SB_HEADLESS1)

One of: `true`, `false`

</ParamField>

<ParamField query="--locale-code <VALUE>" type="string">

The browser language code, such as fr or ar_eg (SB_LOCALE_CODE)

</ParamField>

<ParamField query="--server <VALUE>" type="string">

The Selenium Grid server to connect to (SB_SERVER)

</ParamField>

<ParamField query="--port <VALUE>" type="string">

The port of the Selenium Grid server (SB_PORT)

</ParamField>

<ParamField query="--protocol <VALUE>" type="string">

The protocol of the Selenium Grid server: http or https (SB_PROTOCOL)

</ParamField>

<ParamField query="--chromium-arg <VALUE>" type="string">

Extra Chromium switches, separated by commas (SB_CHROMIUM_ARG)

</ParamField>

<ParamField query="--dark-mode [DARK_MODE]" type="string">

Force Chrome's dark theme on web pages (SB_DARK_MODE)

Also: `--dark`

One of: `true`, `false`

</ParamField>

<ParamField query="--devtools [DEVTOOLS]" type="string">

Open the developer tools with each tab (headed only) (SB_DEVTOOLS)

Also: `--open-devtools`

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-features <VALUE>" type="string">

Chromium features to switch off, separated by commas (SB_DISABLE_FEATURES)

</ParamField>

<ParamField query="--disable-gpu [DISABLE_GPU]" type="string">

Switch the GPU off (SB_DISABLE_GPU)

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-ws [DISABLE_WS]" type="string">

Switch web security off (SB_DISABLE_WS)

Also: `--dws`, `--disable-web-security`

One of: `true`, `false`

</ParamField>

<ParamField query="--guest-mode [GUEST_MODE]" type="string">

Open the browser as a guest (SB_GUEST_MODE)

Also: `--guest`

One of: `true`, `false`

</ParamField>

<ParamField query="--incognito [INCOGNITO]" type="string">

Open the browser in an incognito window (SB_INCOGNITO)

Also: `--incognito-mode`

One of: `true`, `false`

</ParamField>

<ParamField query="--host-resolver-rules <VALUE>" type="string">

Chromium rules that map host names to addresses (SB_HOST_RESOLVER_RULES)

</ParamField>

<ParamField query="--no-sandbox [NO_SANDBOX]" type="string">

Start Chromium without its sandbox (SB_NO_SANDBOX)

One of: `true`, `false`

</ParamField>

<ParamField query="--swiftshader [SWIFTSHADER]" type="string">

Render with SwiftShader, a software WebGL renderer (SB_SWIFTSHADER)

One of: `true`, `false`

</ParamField>

<ParamField query="--enable-3d-apis [ENABLE_3D_APIS]" type="string">

Keep the GPU on in a headless session, for WebGL (SB_ENABLE_3D_APIS)

One of: `true`, `false`

</ParamField>

<ParamField query="--maximize [MAXIMIZE]" type="string">

Maximize the browser window (SB_MAXIMIZE)

Also: `--maximize-window`, `--fullscreen`

One of: `true`, `false`

</ParamField>

<ParamField query="--block-images [BLOCK_IMAGES]" type="string">

Do not load images (SB_BLOCK_IMAGES)

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-js [DISABLE_JS]" type="string">

Turn JavaScript off (SB_DISABLE_JS)

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-cookies [DISABLE_COOKIES]" type="string">

Refuse every cookie (SB_DISABLE_COOKIES)

One of: `true`, `false`

</ParamField>

<ParamField query="--do-not-track [DO_NOT_TRACK]" type="string">

Send the DNT: 1 header (SB_DO_NOT_TRACK)

One of: `true`, `false`

</ParamField>

<ParamField query="--external-pdf [EXTERNAL_PDF]" type="string">

Download PDF files instead of showing them (SB_EXTERNAL_PDF)

One of: `true`, `false`

</ParamField>

<ParamField query="--enable-sync [ENABLE_SYNC]" type="string">

Leave Chrome Sync on (SB_ENABLE_SYNC)

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-csp [DISABLE_CSP]" type="string">

Strip the Content Security Policy from every response (SB_DISABLE_CSP)

Also: `--dcsp`, `--no-csp`

One of: `true`, `false`

</ParamField>

<ParamField query="--use-auto-ext [USE_AUTO_EXT]" type="string">

Keep Chrome's automation extension (SB_USE_AUTO_EXT)

Also: `--auto-ext`

One of: `true`, `false`

</ParamField>

<ParamField query="--proxy-driver [PROXY_DRIVER]" type="string">

Download the driver through the proxy (SB_PROXY_DRIVER)

One of: `true`, `false`

</ParamField>

<ParamField query="--page-load-strategy <STRATEGY>" type="string">

How long a navigation waits: normal, eager or none (SB_PAGE_LOAD_STRATEGY)

Also: `--pls`

One of: `normal`, `eager`, `none`

</ParamField>

<ParamField query="--window-size <VALUE>" type="string">

The window size: WIDTH,HEIGHT (SB_WINDOW_SIZE)

</ParamField>

<ParamField query="--window-position <VALUE>" type="string">

The window position: X,Y (SB_WINDOW_POSITION)

</ParamField>

<ParamField query="--device-metrics <VALUE>" type="string">

Emulate a phone screen: WIDTH,HEIGHT,PIXEL_RATIO (SB_DEVICE_METRICS)

Also: `--metrics`

</ParamField>

<ParamField query="--proxy-bypass-list <VALUE>" type="string">

Hosts that skip the proxy (SB_PROXY_BYPASS_LIST)

</ParamField>

<ParamField query="--driver-version <VALUE>" type="string">

The driver version: a number, latest, stable, beta, ... (SB_DRIVER_VERSION)

</ParamField>

<ParamField query="--cap-file <PATH>" type="string">

A JSON file of capabilities to add to the session's (SB_CAP_FILE)

</ParamField>

<ParamField query="--cap-string <VALUE>" type="string">

A JSON object of capabilities to add to the session's (SB_CAP_STRING)

</ParamField>

<ParamField query="--extension-zip <VALUE>" type="string">

Extension .zip or .crx files to load, separated by commas (SB_EXTENSION_ZIP)

Also: `--crx`

</ParamField>

<ParamField query="--firefox-arg <VALUE>" type="string">

Firefox arguments, separated by commas (SB_FIREFOX_ARG)

</ParamField>

<ParamField query="--firefox-pref <VALUE>" type="string">

Firefox preferences, NAME:VALUE separated by commas (SB_FIREFOX_PREF)

</ParamField>

<ParamField query="--xvfb [XVFB]" type="string">

Open a virtual display for a headed browser on Linux (SB_XVFB)

One of: `true`, `false`

</ParamField>

<ParamField query="--xvfb-metrics <VALUE>" type="string">

The virtual display size: WIDTH,HEIGHT (SB_XVFB_METRICS)

</ParamField>

<ParamField query="--remote-debug [REMOTE_DEBUG]" type="string">

Open Chrome's remote debugger on port 9222 (SB_REMOTE_DEBUG)

Also: `--remote-debugger`

One of: `true`, `false`

</ParamField>

<ParamField query="--log-cdp-events [LOG_CDP_EVENTS]" type="string">

Log the DevTools Protocol events (SB_LOG_CDP_EVENTS)

Also: `--log-cdp`

One of: `true`, `false`

</ParamField>

<ParamField query="--uc-cdp-events [UC_CDP_EVENTS]" type="string">

Log the DevTools Protocol events in undetected mode (SB_UC_CDP_EVENTS)

Also: `--uc-cdp`

One of: `true`, `false`

</ParamField>

<ParamField query="--demo [DEMO]" type="string">

Highlight each element and pause after an action (SB_DEMO)

Also: `--demo-mode`

One of: `true`, `false`

</ParamField>

<ParamField query="--slow [SLOW]" type="string">

Pause briefly after each action (SB_SLOW)

Also: `--slow-mode`, `--slowmo`

One of: `true`, `false`

</ParamField>

<ParamField query="--demo-sleep <SECONDS>" type="string">

Seconds to pause after each action in demo mode (SB_DEMO_SLEEP)

</ParamField>

<ParamField query="--message-duration <SECONDS>" type="string">

Seconds a message stays on the page (SB_MESSAGE_DURATION)

</ParamField>

<ParamField query="--skip-js-waits [SKIP_JS_WAITS]" type="string">

Do not wait for the page's JavaScript to settle (SB_SKIP_JS_WAITS)

Also: `--sjw`

One of: `true`, `false`

</ParamField>

<ParamField query="--highlights <COUNT>" type="string">

How many times to flash a highlighted element (SB_HIGHLIGHTS)

</ParamField>

<ParamField query="--interval <SECONDS>" type="string">

Seconds between the steps of a tour or presentation (SB_INTERVAL)

</ParamField>

<ParamField query="--verify-delay <SECONDS>" type="string">

Seconds to wait before each MasterQA verification (SB_VERIFY_DELAY)

</ParamField>

<ParamField query="--check-js [CHECK_JS]" type="string">

Check each page for JavaScript errors after it opens (SB_CHECK_JS)

One of: `true`, `false`

</ParamField>

<ParamField query="--wait-for-angularjs [WAIT_FOR_ANGULARJS]" type="string">

Wait for AngularJS after opening a page and clicking (SB_WAIT_FOR_ANGULARJS)

Also: `--wfa`

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-beforeunload [DISABLE_BEFOREUNLOAD]" type="string">

Remove the beforeunload prompt after each page opens (SB_DISABLE_BEFOREUNLOAD)

One of: `true`, `false`

</ParamField>

<ParamField query="--visual-baseline [VISUAL_BASELINE]" type="string">

Set the visual baselines again instead of comparing (SB_VISUAL_BASELINE)

One of: `true`, `false`

</ParamField>

<ParamField query="--crumbs [CRUMBS]" type="string">

Delete the cookies when a reused browser is handed to the next test (SB_CRUMBS)

One of: `true`, `false`

</ParamField>

<ParamField query="--timeout-multiplier <VALUE>" type="string">

Multiply the default waits by this number (SB_TIMEOUT_MULTIPLIER)

</ParamField>

<ParamField query="--rec" type="boolean">

Record the browser actions as a test (SB_RECORDER)

Python's `--rec`. Every test that runs writes `recordings/NAME_rec.rs` when its browser quits; `sb.recorder_breakpoint("...")` pauses a test so the actions can be done by hand. `sbrs test` runs the tests one at a time while it is on.

Also: `--recorder`, `--record`, `--codegen`

</ParamField>

<ParamField query="--rec-print" type="boolean">

Print the recording when the test ends (SB_REC_PRINT)

Also: `--rec_print`

</ParamField>

<ParamField query="--rec-behave" type="boolean">

Also write the recording as a Gherkin feature (SB_REC_BEHAVE)

Also: `--rec_behave`, `--rec-gherkin`

</ParamField>

<ParamField query="--rec-sb-mgr" type="boolean">

Write the recording as a script, not a test (SB_REC_SB_MGR)

Also: `--rec_sb_mgr`

</ParamField>

<ParamField query="--rec-sleep" type="boolean">

Keep the sleeps of the recording (SB_REC_SLEEP)

Also: `--record-sleep`, `--rec_sleep`, `--record_sleep`

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

Accept the alert that the page opens when it loads.

```bash Usage
sbrs accept-alert
```

```bash Examples
sbrs --url https://example.com accept-alert
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

```bash Examples
sbrs --url https://example.com add-text --css '#name' --text hello
```

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

```bash Examples
sbrs --url https://example.com assert-attribute --css a --attribute href --value https://iana.org/domains/example
```

## sbrs assert-element

Assert that an element exists.

```bash Usage
sbrs assert-element <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com assert-element --css p
```

## sbrs assert-link-text

Assert a link with the given text is visible.

```bash Usage
sbrs assert-link-text <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Exact link text to assert

</ParamField>

```bash Examples
sbrs --url https://example.com assert-link-text --text 'More information...'
```

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

```bash Examples
sbrs assert-pdf-text https://example.com/report.pdf --text Total
```

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

```bash Examples
sbrs --url https://example.com assert-text-not-visible --css p --text Error
```

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

```bash Examples
sbrs --url https://example.com assert-text-visible --css p --text 'This domain is for use in'
```

## sbrs assert-title

Assert the page title contains the expected text.

```bash Usage
sbrs assert-title <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Expected page title substring

</ParamField>

```bash Examples
sbrs --url https://example.com assert-title --text Example
```

## sbrs behave-gui

Also: `gui-behave`

Launch the Behave Commander, a terminal app (needs a build with the `tui` feature).

Lists the `.feature` files under PATH with their scenarios, runs the ones you mark with `cargo test` and shows the results. PATH is a folder of feature files or one file; by default `features`.

A scenario is matched to the test your project binds to it with rstest-bdd: one with no test named after it is reported as undefined. The Python command opens a Tkinter window; this one runs in the terminal.

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

```bash Examples
sbrs behave-gui features
```

## sbrs behave-options

Print how Python SeleniumBase's behave options map to `sbrs test`.

```bash Usage
sbrs behave-options
```

```bash Examples
sbrs behave-options
```

## sbrs cache-clear

Clear the browser cache via CDP.

```bash Usage
sbrs cache-clear
```

```bash Examples
sbrs --url https://example.com --user-data-dir ./profile cache-clear
```

## sbrs caseplans

Also: `case-plans`, `case_plans`

Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature).

Lists the tests under PATH and creates, edits and saves a case plan for each, `case_plans/<test>.md` next to its file, and writes a summary of the plans to `case_summary.md`. PATH is a folder of tests or one test file; by default the `tests/` and `examples/` folders of the current folder. `-k FILTER` starts with the list narrowed to the tests whose id contains FILTER, as `/` does in the app.

The Python command opens a Tkinter window; this one runs in the terminal.

```bash Usage
sbrs caseplans [-k <FILTER>] [--starter] [PATH]
```

**Arguments**

<ParamField path="PATH" type="string">

A folder of tests, or one test file.

</ParamField>

**Flags**

<ParamField query="-k <FILTER>" type="string">

Start with the list narrowed to the tests whose id contains FILTER.

</ParamField>

<ParamField query="--starter" type="boolean">

Write the starter checklist `CASE_PLANS.md` instead; this needs no `tui` feature.

</ParamField>

```bash Examples
sbrs caseplans --starter
sbrs case_plans tests -k login
```

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

```bash Examples
sbrs cdp --cmd Browser.getVersion
```

## sbrs cdp-click-element

Click an element using CDP.

```bash Usage
sbrs cdp-click-element <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com cdp-click-element --css a
```

## sbrs cdp-type-text

Type text using CDP input injection.

```bash Usage
sbrs cdp-type-text <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Text to inject via CDP

</ParamField>

```bash Examples
sbrs --url https://example.com cdp-type-text --text hello
```

## sbrs check-if-unchecked

Check a checkbox only if it is unchecked.

```bash Usage
sbrs check-if-unchecked <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com check-if-unchecked --css '#agree'
```

## sbrs choose-file

Upload a local file into a file input.

```bash Usage
sbrs choose-file <SELECTOR> <FILE>
```

**Arguments**

<ParamField path="SELECTOR" type="string" required>

CSS selector of the &lt;input type=file> element

</ParamField>

<ParamField path="FILE" type="string" required>

Local file to upload

</ParamField>

```bash Examples
sbrs --url https://example.com/upload choose-file 'input[type=file]' ./photo.png
```

## sbrs clear-cookies

Delete all browser cookies.

```bash Usage
sbrs clear-cookies
```

```bash Examples
sbrs --url https://example.com --user-data-dir ./profile clear-cookies
```

## sbrs clear-local-storage

Clear the page local storage.

```bash Usage
sbrs clear-local-storage
```

```bash Examples
sbrs --url https://example.com --user-data-dir ./profile clear-local-storage
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

```bash Examples
sbrs --url https://example.com click-partial-link-text --text 'More info'
```

## sbrs click-visible-elements

Click all visible elements matching a selector.

```bash Usage
sbrs click-visible-elements <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com click-visible-elements --css button
```

## sbrs commander

Also: `gui`

Launch the Commander, a terminal app (needs a build with the `tui` feature).

Lists the tests and examples under PATH and runs the one you pick with `cargo test --test NAME` or `cargo run --example NAME`, then shows the end of the output. PATH is a project folder, a folder of test files or one test file; by default the `tests/` and `examples/` folders of the current folder. `-k FILTER` starts with the list narrowed to the names that contain FILTER, as `/` does in the app.

Python's `gui` is the same command, and its window is a terminal app here.

```bash Usage
sbrs commander [-k <FILTER>] [PATH]
```

**Arguments**

<ParamField path="PATH" type="string">

A project folder, a folder of test files, or one test file.

</ParamField>

**Flags**

<ParamField query="-k <FILTER>" type="string">

Start with the list narrowed to the names that contain FILTER.

</ParamField>

```bash Examples
sbrs commander
sbrs gui tests -k login
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

```bash Examples
sbrs completions zsh
```

## sbrs convert

Convert an exported Selenium WebDriver test (Python) into a Rust test.

Python's `sbase convert` for a Selenium IDE or Katalon Recorder export: reads FILE.py as Selenium WebDriver code and writes FILE_SB.rs next to it. The original is kept, and an existing FILE_SB.rs is never replaced unless `--force` is given. What could not be converted stays as a `TODO` in the file and is listed on standard error; if a statement could not be read at all the exit status is 1. `import-python` converts any Python source and writes where you say.

```bash Usage
sbrs convert [--force] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

The exported Selenium WebDriver test (a Selenium IDE or Katalon Recorder export).

</ParamField>

**Flags**

<ParamField query="--force" type="boolean">

Replace the converted file if it already exists.

</ParamField>

```bash Examples
sbrs convert login.py
```

## sbrs decrypt

Also: `unobfuscate`

Decrypt a token made by `sbrs encrypt`; read from stdin if TOKEN is omitted. Python's `sbrs unobfuscate` is the same command.

With no TOKEN and a terminal on standard input it asks for a token on the screen, prints the text and asks again until Ctrl-C, as Python's `unobfuscate` does. When SB_ENCRYPTION_KEY is not set it asks once for the passphrase, without echo.

```bash Usage
sbrs decrypt [TOKEN]
```

**Arguments**

<ParamField path="TOKEN" type="string">

The token to decrypt.

</ParamField>

```bash Examples
sbrs decrypt TOKEN
```

## sbrs deferred

Run several assertions, then report every failure at once.

The page they run on is the global --url, if there is one.

```bash Usage
sbrs deferred [--assert <SPEC>] [--spec <SPEC>]
```

**Flags**

<ParamField query="--assert <SPEC>" type="string[]">

Assertion spec: element=&lt;css>, text=&lt;css>:&lt;text>, title=&lt;text>, or url=&lt;text>

</ParamField>

<ParamField query="--spec <SPEC>" type="string">

JSON file containing an array of assertion specs

</ParamField>

```bash Examples
sbrs --url https://example.com deferred --assert element=p --assert 'text=p:This domain is for use in'
```

## sbrs devtools

Launch Chrome with an open DevTools endpoint and print MCP client configs.

Starts Chrome with the Pure CDP engine (no chromedriver), prints the DevTools HTTP endpoint, the browser WebSocket URL and configs for Google's chrome-devtools-mcp and Microsoft's Playwright MCP, then keeps the browser running until Ctrl+C, which closes it and exits with 0. The global --browser (chrome, chromium, edge or another Chromium browser), --headless, --headed, --user-data-dir, --proxy, --user-agent, --locale, --ad-block, --extension-dir --proxy-list and --url (the page to open once the browser runs) apply as well.

```bash Usage
sbrs devtools [FLAGS]
```

**Flags**

<ParamField query="--port <PORT>" type="string" default="9222">

DevTools port on 127.0.0.1 (0 lets Chrome pick a free one)

</ParamField>

<ParamField query="--browser-path <PATH>" type="string">

Browser binary to launch, such as Microsoft Edge or another Chromium browser

</ParamField>

<ParamField query="--json" type="boolean">

Print the endpoint and configs as one line of JSON instead of text

</ParamField>

```bash Examples
sbrs devtools --port 9222
```

## sbrs dismiss-alert

Dismiss the alert that the page opens when it loads.

```bash Usage
sbrs dismiss-alert
```

```bash Examples
sbrs --url https://example.com dismiss-alert
```

## sbrs doctor

Run a diagnostic check on the environment and configuration.

Prints the settings in effect, the browser found, the patch cache and the chromedriver found, with a hint for anything that needs a fix. Exits with status 1 when no browser is found (or the one SB_CHROME_BIN names does not exist) or when the chromedriver does not match the installed Chrome, and with 0 otherwise. A chromedriver that is not found is only a hint, since the pure CDP engine needs none. --warn-only prints the same and always exits with 0.

```bash Usage
sbrs doctor [--warn-only]
```

**Flags**

<ParamField query="--warn-only" type="boolean">

Print the problems found but exit with 0 (the default exits with 1 when the browser is missing or the chromedriver does not match it)

</ParamField>

```bash Examples
sbrs doctor
```

## sbrs download

Download Selenium Server 4, the jar that `grid-hub` and `grid-node` run.

`sbrs download server` saves selenium-server-VERSION.jar in ./downloaded_drivers/ and prints its path and SHA-256. The release this build pins is checked against its checksum; another one is not.

```bash Usage
sbrs download <SUBCOMMAND>
```

```bash Examples
sbrs download server
```

### sbrs download server

Download the Selenium Server 4 jar that `grid-hub` and `grid-node` run.

Saves selenium-server-VERSION.jar in ./downloaded_drivers/ and prints its path. The default version is pinned in this build and the download is checked against its SHA-256; another --version is saved as it arrives, with its SHA-256 printed and `not verified`. When neither --jar nor SB_SELENIUM_SERVER_JAR is given, `grid-hub` and `grid-node` use the newest jar in that folder. Python's Selenium 3 server is not offered, because Grid 3 is gone.

```bash Usage
sbrs download server [--version <VERSION>]
```

**Flags**

<ParamField query="--version <VERSION>" type="string">

The release to download, such as 4.51.0 (default: the version this build pins).

</ParamField>

```bash Examples
sbrs download server
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

```bash Examples
sbrs download-file https://example.com/report.pdf --output report.pdf
```

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

```bash Examples
sbrs --url https://example.com drag-and-drop --source-css '#a' --target-css '#b'
```

## sbrs encrypt

Also: `obfuscate`

Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted.

The passphrase comes from the SB_ENCRYPTION_KEY environment variable, never from an argument, so it stays out of shell history and process listings. Python's `sbrs obfuscate` is the same command.

With no TEXT and a terminal on standard input it prompts as Python's `obfuscate` does: the text and its verification are read without echo, the token is printed with the line that decrypts it in a test, and it asks again until Ctrl-C. When SB_ENCRYPTION_KEY is not set it asks once for the passphrase (twice, to catch a typo), without echo; the passphrase is never printed or saved.

```bash Usage
sbrs encrypt [TEXT]
```

**Arguments**

<ParamField path="TEXT" type="string">

The text to encrypt.

</ParamField>

```bash Examples
sbrs encrypt 'my secret'
```

## sbrs extract-objects

Also: `extract_objects`

Move the selectors of a Rust test into page objects in `page_objects.rs`.

Python's `sbase extract-objects`, for a Rust test. FILE is read for the string literals that are the selector of a call (`sb.click("#go")`, `sb.type_text("#user", "ada")`, `sb.assert_text("Hi", "h1")`, `sb.drag_and_drop(".a", ".b")`; `sbrs methods` lists the common calls), and each one becomes a constant of a module of `page_objects.rs` in the folder of FILE: one module for each page that the test opens, named with the words of its URL (`pub mod example_com_login`, and `page` before any `open`), one constant for each selector, named with the words of the selector (`pub const USER: &str = "#user";`, `_2` after a name that is taken). A `page_objects.rs` that is there is added to; one that `sbrs` did not write is refused. FILE is not changed: `inject-objects` does that.

```bash Usage
sbrs extract-objects <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

The Rust test file whose selectors become page objects.

</ParamField>

```bash Examples
sbrs extract-objects tests/login.rs
```

## sbrs fingerprint

Capture, write, check and compare browser fingerprints.

A fingerprint file is what a browser claims to be: the user agent, the screen, the language, the graphics card and the rest, as one JSON file. `capture` reads it from a real browser; `export` writes a preset or a random identity; `check` and `diff` read files. The global --fingerprint FILE (or SB_FINGERPRINT, or the [fingerprint] table of the settings file) makes every browser a command starts present one.

```bash Usage
sbrs fingerprint <SUBCOMMAND>
```

```bash Examples
sbrs fingerprint presets
```

### sbrs fingerprint capture

Read the fingerprint of a real browser and write it as a fingerprint file.

Starts a browser as `sbrs open` does (the global --browser, --headless, --proxy and the rest apply), opens URL (a blank page when there is none), reads what a page can see of the browser, and prints the fingerprint as JSON, or writes it to FILE. A summary goes to standard error. What no setting holds (window sizes, the automation markers, the canvas hash) stays out of the fingerprint; --readings writes it to its own file.

```bash Usage
sbrs fingerprint capture [-o --output <FILE>] [--readings <FILE>] [URL]
```

**Arguments**

<ParamField path="URL" type="string">

The page to open first (default: a blank page).

</ParamField>

**Flags**

<ParamField query="--output <FILE>" type="string">

Write the fingerprint to FILE instead of printing it.

Also: `-o`

</ParamField>

<ParamField query="--readings <FILE>" type="string">

Also write the readings that no setting holds, and where the capture was taken, to FILE.

</ParamField>

```bash Examples
sbrs fingerprint capture https://example.com --output seen.json
```

### sbrs fingerprint check

Check that a fingerprint file loads and is coherent.

Prints how many settings the file gives, each error and warning, and the verdict. Exits with status 2 when there is an error or the file does not load.

```bash Usage
sbrs fingerprint check <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

The fingerprint file to check.

</ParamField>

```bash Examples
sbrs fingerprint check windows.json
```

### sbrs fingerprint diff

Compare two fingerprint files.

EXPECTED is what is expected and FOUND what is found: a setting that EXPECTED leaves natural is never compared. Prints each setting that differs. Exits with status 1 when any differs, 0 when none does, and 2 when a file does not load.

To check that a browser presents a file: `sbrs --fingerprint f.json fingerprint capture -o seen.json`, then `sbrs fingerprint diff f.json seen.json`.

```bash Usage
sbrs fingerprint diff <EXPECTED> <FOUND>
```

**Arguments**

<ParamField path="EXPECTED" type="string" required>

The file with what is expected.

</ParamField>

<ParamField path="FOUND" type="string" required>

The file with what is found.

</ParamField>

```bash Examples
sbrs fingerprint diff windows.json seen.json
```

### sbrs fingerprint export

Write a preset or a random identity as a fingerprint file.

Give --preset NAME (windows, edge, macos, linux, android, ios) or --random. A random identity follows --seed, so the same seed gives the same file, and --os, the system it claims (default: this machine's). The file goes to FILE, else to standard output.

```bash Usage
sbrs fingerprint export [FLAGS]
```

**Flags**

<ParamField query="--preset <NAME>" type="string">

The preset to write: windows, edge, macos, linux, android or ios (safari is ios).

One of: `windows`, `edge`, `macos`, `linux`, `android`, `ios`

</ParamField>

<ParamField query="--random" type="boolean">

Write a random identity instead of a preset.

</ParamField>

<ParamField query="--seed <N>" type="string">

With --random, the seed of the draw: the same seed gives the same identity.

</ParamField>

<ParamField query="--os <OS>" type="string">

With --random, the system the identity claims (default: this machine's).

One of: `windows`, `macos`, `linux`, `android`, `ios`

</ParamField>

<ParamField query="--output <FILE>" type="string">

Write the fingerprint to FILE instead of printing it.

Also: `-o`

</ParamField>

```bash Examples
sbrs fingerprint export --preset windows --output windows.json
```

### sbrs fingerprint presets

List the presets and what each claims.

```bash Usage
sbrs fingerprint presets
```

```bash Examples
sbrs fingerprint presets
```

## sbrs get-alert-text

Print the text of the alert that the page opens when it loads.

```bash Usage
sbrs get-alert-text
```

```bash Examples
sbrs --url https://example.com get-alert-text
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

```bash Examples
sbrs --url https://example.com get-attribute --css a --attribute href
```

## sbrs get-cookies

Print all browser cookies as JSON.

```bash Usage
sbrs get-cookies
```

```bash Examples
sbrs --url https://example.com get-cookies
```

## sbrs get-current-url

Print the current URL.

```bash Usage
sbrs get-current-url
```

```bash Examples
sbrs --url https://example.com get-current-url
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

```bash Examples
sbrs --url https://example.com get-local-storage-item --key theme
```

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

```bash Examples
sbrs --url https://example.com get-property --css input --property value
```

## sbrs get-text

Print the visible text of an element.

```bash Usage
sbrs get-text <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com get-text --css p
```

## sbrs get-title

Print the page title.

```bash Usage
sbrs get-title
```

```bash Examples
sbrs --url https://example.com get-title
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

```bash Examples
sbrs --url https://example.com get-value --css '#name'
```

## sbrs get-window-position

Print the browser window position.

```bash Usage
sbrs get-window-position
```

```bash Examples
sbrs --url https://example.com get-window-position
```

## sbrs grid-hub

Also: `grid_hub`

Start, stop, restart or check a Selenium Grid hub.

Needs a Selenium Server jar: --jar, SB_SELENIUM_SERVER_JAR, or the newest one in downloaded_drivers/ (`sbrs download server` saves one there). This command downloads nothing. The process record and log are kept in a folder of your own (SB_GRID_DIR).

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

The Selenium Server jar (default: SB_SELENIUM_SERVER_JAR, else the newest in downloaded_drivers/). This command never downloads it.

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

<ParamField query="--timeout <SECONDS>" type="string">

Close sessions that have been idle this many seconds.

</ParamField>

<ParamField query="--wait <SECONDS>" type="string" default="60">

How many seconds `start` waits for the Grid to become ready.

</ParamField>

<ParamField query="--verbose" type="boolean">

Log at INFO instead of WARNING.

Also: `-v`

</ParamField>

<ParamField query="--skip-port-check" type="boolean">

Start even if something already accepts connections on the port.

</ParamField>

```bash Examples
sbrs grid-hub start
sbrs grid_hub start -v --timeout 120
```

## sbrs grid-node

Also: `grid_node`

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

The Selenium Server jar (default: SB_SELENIUM_SERVER_JAR, else the newest in downloaded_drivers/). This command never downloads it.

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

<ParamField query="--timeout <SECONDS>" type="string">

Close sessions that have been idle this many seconds.

</ParamField>

<ParamField query="--wait <SECONDS>" type="string" default="60">

How many seconds `start` waits for the Grid to become ready.

</ParamField>

<ParamField query="--verbose" type="boolean">

Log at INFO instead of WARNING.

Also: `-v`

</ParamField>

<ParamField query="--skip-port-check" type="boolean">

Start even if something already accepts connections on the port.

</ParamField>

```bash Examples
sbrs grid-node start
sbrs grid-node start --hub 127.0.0.1
```

## sbrs highlight-click

Highlight an element, then click it.

```bash Usage
sbrs highlight-click <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com highlight-click --css a
```

## sbrs hover

Hover over an element.

```bash Usage
sbrs hover <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com hover --css p
```

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

```bash Examples
sbrs --url https://example.com hover-and-click --hover-css nav --click-css 'nav a'
```

## sbrs human-click

Click an element with human-like timing and noise.

```bash Usage
sbrs human-click <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com human-click --css a
```

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

```bash Examples
sbrs --url https://example.com human-type --css '#q' --text hello
```

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

```bash Examples
sbrs import-playwright login.spec.ts --output tests/login.rs
```

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

```bash Examples
sbrs import-puppeteer login.js --output tests/login.rs
```

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

```bash Examples
sbrs import-python login.py --output tests/login.rs
```

## sbrs inject-objects

Also: `inject_objects`

Replace the selectors of a Rust test with the page objects of `page_objects.rs`.

Python's `sbase inject-objects`, for a Rust test. Every selector of FILE that has a page object (made by `extract-objects`) is written as `page_objects::<page>::NAME`, and `mod page_objects;` is declared above the first item of the file. A selector that has no page object stays as it is. With `-c`, the selector is added as a comment at the end of each line that changed, as `// "#user"`.

```bash Usage
sbrs inject-objects [-c --comments] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

The Rust test file whose selectors are replaced by page objects.

</ParamField>

**Flags**

<ParamField query="--comments" type="boolean">

Add the selector as a comment, such as `// "#user"`, at the end of each line it replaced.

Also: `-c`

</ParamField>

```bash Examples
sbrs inject-objects tests/login.rs -c
```

## sbrs install

Also: `get`

Download a driver or a browser into ./downloaded_drivers/ (Python's `sbrs get`).

Without a version, chromedriver and edgedriver are the drivers for the installed Chrome and Edge, and geckodriver is the newest release. uc_driver is the chromedriver patched for the UC mode; cft, chs and chromium are browsers, and chromium takes a snapshot --revision instead of a version.

```bash Usage
sbrs install [-p --path] [--revision <REVISION>] <DRIVER> [VERSION]
```

**Arguments**

<ParamField path="DRIVER" type="string" required>

The program: chromedriver, uc_driver, geckodriver (or firefoxdriver), edgedriver (or msedgedriver), or the browsers cft, chs and chromium.

One of: `chromedriver`, `geckodriver`, `edgedriver`, `uc_driver`, `cft`, `chs`, `chromium`

</ParamField>

<ParamField path="VERSION" type="string">

A version word (stable, beta, dev, canary, latest, previous, mlatest, browser, keep), a major version, a full build, or for geckodriver a version such as 0.36.0.

</ParamField>

**Flags**

<ParamField query="--path" type="boolean">

Also copy the driver into the folder named by SB_DRIVER_PATH_DIR (else /usr/local/bin). Never uses sudo and never replaces a symbolic link. Drivers only.

Also: `-p`

</ParamField>

<ParamField query="--revision <REVISION>" type="string">

The Chromium snapshot to take (chromium only); the newest when left out.

</ParamField>

```bash Examples
sbrs install chromedriver latest
```

## sbrs is-checked

Print whether a checkbox is checked.

```bash Usage
sbrs is-checked <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com is-checked --css '#agree'
```

## sbrs is-element-visible

Print whether an element is visible.

```bash Usage
sbrs is-element-visible <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com is-element-visible --css p
```

## sbrs is-link-text-visible

Print whether a link with the given text is visible.

```bash Usage
sbrs is-link-text-visible <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Exact link text to check

</ParamField>

```bash Examples
sbrs --url https://example.com is-link-text-visible --text 'More information...'
```

## sbrs is-partial-link-text-visible

Print whether a link containing the given text is visible.

```bash Usage
sbrs is-partial-link-text-visible <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Link text substring to check

</ParamField>

```bash Examples
sbrs --url https://example.com is-partial-link-text-visible --text 'More info'
```

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

```bash Examples
sbrs --url https://example.com is-text-visible --css p --text domain
```

## sbrs load-cookies

Load browser cookies from a JSON file.

```bash Usage
sbrs load-cookies <--file <FILE>>
```

**Flags**

<ParamField query="--file <FILE>" type="string" required>

JSON file to load cookies from

</ParamField>

```bash Examples
sbrs --url https://example.com --user-data-dir ./profile load-cookies --file cookies.json
```

## sbrs methods

List the common methods of the test API, as they are called on `sb`.

Python SeleniumBase's `sbase methods` in Rust spelling: the methods most tests use (`sb.open(url)`, `sb.click(selector)`, `sb.assert_text(text, selector)`, ...), each with what it does. The whole API is in `cargo doc --open`, and docs/parity.md maps Python's names to it.

```bash Usage
sbrs methods
```

```bash Examples
sbrs methods
```

## sbrs mkchart

Create a test that shows a chart on a slide.

As `mkpres`, with a pie chart made by `create_pie_chart` and `add_data_point` and put on a slide with `extract_chart` (theme `sky`, transition `slide`).

```bash Usage
sbrs mkchart [FLAGS] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

File to create, for example `tests/chart.rs`.

</ParamField>

**Flags**

<ParamField query="--force" type="boolean">

Replace the file if it already exists.

</ParamField>

<ParamField query="--en" type="boolean">

English strings

</ParamField>

<ParamField query="--zh" type="boolean">

Chinese strings

</ParamField>

<ParamField query="--nl" type="boolean">

Dutch strings

</ParamField>

<ParamField query="--fr" type="boolean">

French strings

</ParamField>

<ParamField query="--it" type="boolean">

Italian strings

</ParamField>

<ParamField query="--ja" type="boolean">

Japanese strings

</ParamField>

<ParamField query="--ko" type="boolean">

Korean strings

</ParamField>

<ParamField query="--pt" type="boolean">

Portuguese strings

</ParamField>

<ParamField query="--ru" type="boolean">

Russian strings

</ParamField>

<ParamField query="--es" type="boolean">

Spanish strings

</ParamField>

```bash Examples
sbrs mkchart tests/chart.rs --ja
```

## sbrs mkdir

Create a folder with a runnable browser-test suite.

Names are relative to the current directory and use letters, digits, `_`, `-` and `.`, with `/` between folders. Nothing is overwritten unless `--force` is given.

The folder is one Cargo test target (`tests/<folder>/main.rs`) with example tests that use the rstest `sb` fixture, a parameterized test and a pure CDP test. `--bdd` adds a Gherkin scenario that runs on the built-in steps, and `--gha` a GitHub Actions workflow in `.github/workflows/` of the current folder.

```bash Usage
sbrs mkdir [FLAGS] <DIR>
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

<ParamField query="--gha" type="boolean">

Also write a GitHub Actions workflow, .github/workflows/sbrs-tests.yml, here.

</ParamField>

<ParamField query="--bdd" type="boolean">

Also write a Gherkin scenario that runs on the built-in steps (needs the bdd feature).

</ParamField>

<ParamField query="--force" type="boolean">

Replace generated files that already exist.

</ParamField>

```bash Examples
sbrs mkdir ui_tests
```

## sbrs mkfile

Also: `new`

Create a browser-test file.

The name follows the same rules as `mkdir`; `.rs` is added when missing. An existing file is never overwritten unless `--force` is given.

The test uses the rstest `sb` fixture unless a syntax flag says otherwise: `--pf` (the default), `--bc` (a BaseCase made and quit by hand), `--cm` (the run_browser_test closure), `--dc` (the pure CDP `sb_cdp` fixture) or `--dm` (a pure CDP browser launched by hand). A language flag (`--en`, `--zh`, `--nl`, `--fr`, `--it`, `--ja`, `--ko`, `--pt`, `--ru`, `--es`) localises the strings it types and expects. `--rec` adds a recorder breakpoint for Recorder Mode, and `--uc` and `--cdp` write a basic test for UC Mode and for pure CDP.

```bash Usage
sbrs mkfile [FLAGS] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

File to create, for example `tests/login.rs`.

</ParamField>

**Flags**

<ParamField query="--url <URL>" type="string">

Page the test opens (default: https://example.com). A bare host such as wikipedia.org gets https://; the test is then the basic one.

</ParamField>

<ParamField query="--basic" type="boolean">

Leave the test body empty: it only opens the page.

Also: `-b`

</ParamField>

<ParamField query="--rec" type="boolean">

Add a recorder breakpoint for Recorder Mode (also --record, --recorder).

Also: `--record`, `--recorder`, `-r`

</ParamField>

<ParamField query="--uc" type="boolean">

UC Mode: the run_browser_test closure with the UC driver, a basic body and English.

</ParamField>

<ParamField query="--cdp" type="boolean">

Pure CDP: the sb_cdp fixture, a basic body and English.

</ParamField>

<ParamField query="--force" type="boolean">

Replace the file if it already exists.

</ParamField>

<ParamField query="--en" type="boolean">

English strings

</ParamField>

<ParamField query="--zh" type="boolean">

Chinese strings

</ParamField>

<ParamField query="--nl" type="boolean">

Dutch strings

</ParamField>

<ParamField query="--fr" type="boolean">

French strings

</ParamField>

<ParamField query="--it" type="boolean">

Italian strings

</ParamField>

<ParamField query="--ja" type="boolean">

Japanese strings

</ParamField>

<ParamField query="--ko" type="boolean">

Korean strings

</ParamField>

<ParamField query="--pt" type="boolean">

Portuguese strings

</ParamField>

<ParamField query="--ru" type="boolean">

Russian strings

</ParamField>

<ParamField query="--es" type="boolean">

Spanish strings

</ParamField>

<ParamField query="--pf" type="boolean">

The rstest `sb` fixture (the default)

</ParamField>

<ParamField query="--bc" type="boolean">

The BaseCase made and quit by hand

</ParamField>

<ParamField query="--cm" type="boolean">

A closure run by run_browser_test

</ParamField>

<ParamField query="--dc" type="boolean">

The pure CDP `sb_cdp` fixture

</ParamField>

<ParamField query="--dm" type="boolean">

A pure CDP browser launched and closed by hand

</ParamField>

<ParamField query="--cf" type="boolean">

Not ported: class fixtures have no Rust form (use --pf)

</ParamField>

```bash Examples
sbrs mkfile tests/login.rs
```

## sbrs mkpres

Create a test that builds a slide deck.

The name follows the same rules as `mkfile`; `.rs` is added when missing. The test uses the rstest `sb` fixture and builds three slides with `create_presentation`, `add_slide` and `begin_presentation` (theme `serif`, transition `slide`). A language flag (`--en`, `--zh`, `--nl`, `--fr`, `--it`, `--ja`, `--ko`, `--pt`, `--ru`, `--es`) writes the text of the slides in that language. An existing file is never overwritten unless `--force` is given.

```bash Usage
sbrs mkpres [FLAGS] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

File to create, for example `tests/deck.rs`.

</ParamField>

**Flags**

<ParamField query="--force" type="boolean">

Replace the file if it already exists.

</ParamField>

<ParamField query="--en" type="boolean">

English strings

</ParamField>

<ParamField query="--zh" type="boolean">

Chinese strings

</ParamField>

<ParamField query="--nl" type="boolean">

Dutch strings

</ParamField>

<ParamField query="--fr" type="boolean">

French strings

</ParamField>

<ParamField query="--it" type="boolean">

Italian strings

</ParamField>

<ParamField query="--ja" type="boolean">

Japanese strings

</ParamField>

<ParamField query="--ko" type="boolean">

Korean strings

</ParamField>

<ParamField query="--pt" type="boolean">

Portuguese strings

</ParamField>

<ParamField query="--ru" type="boolean">

Russian strings

</ParamField>

<ParamField query="--es" type="boolean">

Spanish strings

</ParamField>

```bash Examples
sbrs mkpres tests/deck.rs --es
```

## sbrs mkrec

Also: `codegen`, `record`

Create a new test file with the Recorder: record your actions in a browser, then save them as a test.

Python's `sbase mkrec` (also `codegen` and `record`). Starts a browser in Recorder Mode, opening --url first when it is given, and waits: type `c` and press Enter at the prompt, or press SHIFT+ESC in the browser with --ee. The recording is written to `recordings/NAME_rec.rs` and `recordings/NAME_rec.json`, shown, and copied to FILE.

```bash Usage
sbrs mkrec [--overwrite] [--behave] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

The test file to create (FILE.rs)

</ParamField>

**Flags**

<ParamField query="--overwrite" type="boolean">

Overwrite the file when it exists

</ParamField>

<ParamField query="--behave" type="boolean">

Also write the recording as a Gherkin feature (same as --rec-behave)

Also: `--gherkin`

</ParamField>

```bash Examples
sbrs mkrec new_test.rs --url seleniumbase.io/demo_page
```

## sbrs objectify

Turn the selectors of a Rust test into page objects: extract-objects, then inject-objects.

Python's `sbase objectify`, for a Rust test: FILE is read as `extract-objects` reads it, the page objects are written to `page_objects.rs` next to it, and the selectors of FILE are replaced by them as `inject-objects` does (`-c` adds the selector as a comment to each line that changes). `revert-objects` undoes it. This replaces the starter `page_object.rs` that `sbrs objectify` used to write.

```bash Usage
sbrs objectify [-c --comments] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

The Rust test file whose selectors are replaced by page objects.

</ParamField>

**Flags**

<ParamField query="--comments" type="boolean">

Add the selector as a comment, such as `// "#user"`, at the end of each line it replaced.

Also: `-c`

</ParamField>

```bash Examples
sbrs objectify tests/login.rs -c
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

```bash Examples
sbrs open https://example.com
```

## sbrs options

List the browser options: each flag, its `SB_*` variable and its key in the settings file.

The same options can be set in the settings file, in the environment and with these flags; the command line wins over the environment, which wins over the file. `--env` lists the environment variables instead, those of `sbrs-mcp` too. `--markdown` prints the table for the documentation, and `--all` adds the options of the test run (`sbrs test`).

```bash Usage
sbrs options [FLAGS]
```

**Flags**

<ParamField query="--markdown" type="boolean">

Print a Markdown table, for the documentation.

</ParamField>

<ParamField query="--all" type="boolean">

Also list the options of the test run (`sbrs test`).

</ParamField>

<ParamField query="--env" type="boolean">

List the environment variables `sbrs` and `sbrs-mcp` read instead of the options.

One line for each variable: who reads it, the flag that sets the same thing, what it is when it is not set, and what it sets. `--markdown` prints the table for the documentation. The table is made from the option table and the list of the other variables, so it names every variable the command lines read.

</ParamField>

```bash Examples
sbrs options
sbrs options --markdown
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

Directory where patched binaries are cached (SB_PATCH_CACHE_DIR)

</ParamField>

```bash Examples
sbrs patch-chrome --path ./chrome --cache-dir ./patched
```

## sbrs patch-chromedriver

Patch a chromedriver binary to reduce detection surface.

```bash Usage
sbrs patch-chromedriver <--path <PATH>>
```

**Flags**

<ParamField query="--path <PATH>" type="string" required>

Path to the chromedriver binary

</ParamField>

```bash Examples
sbrs patch-chromedriver --path ./chromedriver
```

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

```bash Examples
sbrs pdf-text https://example.com/report.pdf
```

## sbrs plugin

List the built-in plugins, or show what one declares.

Plugins run inside the browsers a command starts. Turn them on by name with the global options --plugin and --plugin-opt, the SB_PLUGINS variable, or the [plugins] table of the settings file; the command line wins over SB_PLUGINS, which wins over the file. `sbrs plugin list` shows the plugins this build has.

```bash Usage
sbrs plugin <SUBCOMMAND>
```

```bash Examples
sbrs plugin list
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

```bash Examples
sbrs plugin info shield
```

### sbrs plugin list

List the built-in plugins of this build, and whether they are on.

Prints each plugin's name, category and summary, and `yes` under ENABLED when the settings file, SB_PLUGINS or the global --plugin option turns it on. A build without built-in plugins says so; rebuild it with --features plugins-builtin.

```bash Usage
sbrs plugin list
```

```bash Examples
sbrs plugin list
```

### sbrs plugin run

Run the task of a plugin on a browser it starts, and write its results as JSON lines.

Only plugins that offer a task can be run, and the named plugin is turned on for this run without --plugin. The task reads its input from --input (a JSON file, or `-` for standard input; `{}` when there is none) and writes one JSON value per line to --out (default: standard output). Its summary goes to standard error. The global options --plugin, --plugin-opt, --headless, --proxy and the like shape the browser and the other plugins. Ctrl-C stops the task, closes the browser and exits with status 130. Exits with status 2 and lists the plugins that have a task when the name is not one of them.

```bash Usage
sbrs plugin run [--input <FILE>] [--out <FILE>] <NAME>
```

**Arguments**

<ParamField path="NAME" type="string" required>

The plugin whose task to run, as `sbrs plugin list` shows it.

</ParamField>

**Flags**

<ParamField query="--input <FILE>" type="string">

A JSON file with the task's input, or `-` to read it from standard input.

</ParamField>

<ParamField query="--out <FILE>" type="string">

Write the results to this file, one JSON value per line (default: standard output).

</ParamField>

```bash Examples
sbrs plugin run crawler --input crawl.json --out found.jsonl
```

## sbrs print

Print a file, with a line number in front of every line if you ask for it.

The text is shown as it is: there is no syntax highlighting (Python's `sbase print` has it), and bytes that are not UTF-8 are shown as U+FFFD.

```bash Usage
sbrs print [-n --line-numbers] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

File to print.

</ParamField>

**Flags**

<ParamField query="--line-numbers" type="boolean">

Put the line number in front of every line.

Also: `-n`

</ParamField>

```bash Examples
sbrs print tests/login.rs -n
```

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

```bash Examples
sbrs print-pdf https://example.com --output page.pdf
```

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

```bash Examples
sbrs prompt
```

## sbrs proxy

Run a small forward proxy on this machine (HTTP and CONNECT, no authentication).

Point a browser at it with the global --proxy option. `CONNECT host:port` becomes a tunnel, so HTTPS passes through untouched; an absolute `http://` request is sent to its host once and the connection closed. It listens on 127.0.0.1:8899 unless told otherwise, and a request head over 64 KiB, more than 256 connections at once and 30 seconds without a byte are refused or cut. Each request is logged at INFO (SB_LOG_LEVEL=info). Ctrl-C or SIGTERM stops it with status 0.

```bash Usage
sbrs proxy [--hostname <HOST>] [--port <PORT>]
```

**Flags**

<ParamField query="--hostname <HOST>" type="string" default="127.0.0.1">

The address to listen on. Anything but a loopback address makes this an open proxy for other machines, and the command says so.

</ParamField>

<ParamField query="--port <PORT>" type="string" default="8899">

The port to listen on; 0 picks a free one.

</ParamField>

```bash Examples
sbrs proxy --port 8899
```

## sbrs recorder

Launch the Recorder, a terminal app (needs a build with the `tui` feature).

A form for the file name, the start page, the browser and the output format, and three actions. Record hands the terminal to `sbrs mkrec` and takes it back when the recording is over; Run starts the recorded test (`cargo test --test NAME`); Replay runs `recordings/NAME_rec.json` in a fresh browser (`sbrs replay`). `--url`, `--browser`, `--uc`, `--cdp`, `--ee`, `--rec-behave`, `--rec-sb-mgr` and `--rec-sb-cdp` fill the form in when the app starts.

Python's `sbase recorder` opens a Tkinter window; this one runs in the terminal.

```bash Usage
sbrs recorder
```

```bash Examples
sbrs recorder
sbrs recorder --uc --url seleniumbase.io
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

```bash Examples
sbrs --url https://example.com --user-data-dir ./profile remove-local-storage-item --key theme
```

## sbrs replay

Replay a recorded action file (recordings/NAME_rec.json) against a fresh browser.

```bash Usage
sbrs replay <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

JSON or YAML action file (recordings/NAME_rec.json)

</ParamField>

```bash Examples
sbrs replay recordings/new_test_rec.json
```

## sbrs report

Finish a test run and write its reports, or show the runs of a results database.

`sbrs report finish` ends the run of the current folder: the dashboard, the HTML report and the JUnit XML are written a last time. Use it after a plain `cargo test`, which leaves the live dashboard going; `sbrs test` finishes its run by itself. `sbrs report history` lists the runs recorded in a results database (a build with the `turso` feature).

```bash Usage
sbrs report <SUBCOMMAND>
```

```bash Examples
sbrs report finish --html report.html
```

### sbrs report finish

Finish the test run of the current folder and write its reports.

A plain `cargo test` leaves the live dashboard running, because the standard test harness cannot tell when the last test has ended. This command ends the run: the tests that started and never ended become unreported, the dashboard, the HTML report and the JUnit XML are written a last time, the logs are archived when asked, and the results go to the results database when `with_db_reporting` is on. `sbrs test` does the same by itself. The run is the one in `SB_REPORT_ROOT`, or in the current folder; the flags replace the settings the run was started with.

```bash Usage
sbrs report finish [FLAGS]
```

**Flags**

<ParamField query="--dashboard" type="boolean">

Write the live dashboard (dashboard.html) in its final form (SB_DASHBOARD).

</ParamField>

<ParamField query="--dash-title <TITLE>" type="string">

The dashboard's title; an underscore stands for a space (SB_DASH_TITLE).

</ParamField>

<ParamField query="--html <PATH>" type="string">

Write an HTML report to this file (SB_HTML).

</ParamField>

<ParamField query="--junit-xml <PATH>" type="string">

Write a JUnit XML report to this file (SB_JUNIT_XML).

</ParamField>

<ParamField query="--archive-logs" type="boolean">

Keep the logs of the run in archived_logs/ instead of deleting them later (SB_ARCHIVE_LOGS).

</ParamField>

<ParamField query="--show-report" type="boolean">

Open the HTML report or the dashboard in a browser window and wait until it is closed.

</ParamField>

<ParamField query="--json" type="boolean">

Print what was done as JSON instead of the `-----` lines.

</ParamField>

```bash Examples
sbrs report finish --html report.html --junit-xml report.xml
```

## sbrs revert-objects

Also: `revert_objects`

Put the selectors back where a Rust test uses page objects.

Python's `sbase revert-objects`, for a Rust test: the undo of `inject-objects`. Every `page_objects::<page>::NAME` of FILE is replaced by the selector it stands for, and the `mod page_objects;` line goes when the file uses no page object any more. Extract, inject and revert give the original file back byte for byte. With `-c` the comments that `inject-objects -c` appended are taken off as well (Python's `-c` keeps them); without it they stay. `page_objects.rs` is kept.

```bash Usage
sbrs revert-objects [-c --comments] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

The Rust test file whose page objects are replaced by their selectors.

</ParamField>

**Flags**

<ParamField query="--comments" type="boolean">

Also take off the comments that `inject-objects -c` appended to the lines it changed.

Also: `-c`

</ParamField>

```bash Examples
sbrs revert-objects tests/login.rs -c
```

## sbrs run-scenario

Run a scenario file, the way to do several steps in one browser, and write an optional dashboard.

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

```bash Examples
sbrs run-scenario --file scenario.json
```

## sbrs save-cookies

Save browser cookies to a JSON file.

```bash Usage
sbrs save-cookies <--file <FILE>>
```

**Flags**

<ParamField query="--file <FILE>" type="string" required>

JSON file to save cookies to

</ParamField>

```bash Examples
sbrs --url https://example.com save-cookies --file cookies.json
```

## sbrs save-source

Save the current page source to a file.

```bash Usage
sbrs save-source [--path <PATH>]
```

**Flags**

<ParamField query="--path <PATH>" type="string">

Output HTML file path (default: a new file in latest_logs/)

</ParamField>

```bash Examples
sbrs --url https://example.com save-source --path page.html
```

## sbrs screenshot

Capture a screenshot of the current page.

```bash Usage
sbrs screenshot [--path <PATH>]
```

**Flags**

<ParamField query="--path <PATH>" type="string">

Output image file path (default: a new file in latest_logs/)

</ParamField>

```bash Examples
sbrs --url https://example.com screenshot --path page.png
```

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

```bash Examples
sbrs --url https://example.com select-option --css select --text Two
```

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

```bash Examples
sbrs --url https://example.com send-keys --css '#name' --text hello
```

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

```bash Examples
sbrs --url https://example.com --user-data-dir ./profile set-local-storage-item --key theme --value dark
```

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

```bash Examples
sbrs smoke https://example.com --title-contains Example
```

## sbrs smooth-scroll-to

Smoothly scroll an element into view.

```bash Usage
sbrs smooth-scroll-to <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com smooth-scroll-to --css footer
```

## sbrs solve-captcha

Solve the CAPTCHA on the page and check that it cleared.

Finds a Turnstile, reCAPTCHA, hCaptcha, Friendly Captcha or DataDome widget (in the page, its shadow roots and readable frames), waits for it to stand still, presses it along a human pointer path and waits for the page to show a result, pressing again up to three times if the page could show one and did not. It then checks what you expect of the page: text of the site's own page and a clearance cookie. Needs `--cdp` or `--uc`.

```bash Usage
sbrs solve-captcha [FLAGS]
```

**Flags**

<ParamField query="--expect-text <EXPECT_TEXT>" type="string">

Text that must be on the page afterwards (any letter case)

</ParamField>

<ParamField query="--expect-cookie <EXPECT_COOKIE>" type="string">

Cookie the browser must hold afterwards, such as cf_clearance

</ParamField>

<ParamField query="--wait <WAIT>" type="string" default="30">

Seconds to wait for the expected text

</ParamField>

```bash Examples
sbrs --uc --url https://nowsecure.nl solve-captcha --expect-text nowsecure --expect-cookie cf_clearance
```

## sbrs test

Run the project's tests, or one example, through Cargo or cargo-nextest.

Every option of Python SeleniumBase's pytest plugin is a flag of `test`, in each of its spellings (`--no-sandbox`, `--no_sandbox`, `--dcsp`): the flags reach the tests as SB_* environment variables, which a test that loads `Settings` reads. The options before the command (`sbrs --headless --browser firefox test`) reach them too, and a flag after it wins. `--plugin` reaches them as SB_PLUGINS, with the plugins the settings file and the variable already turn on; `--plugin-opt` cannot be passed on, so `sbrs test` refuses it.

`-k` and `-m` select tests as pytest does (a marker is a module name), `-n` sets how many run at once, `--co` lists them, `--pdb`, `--trace` and `--final-debug` stop in a prompt (one test at a time, output shown), `--reruns N` runs a failing test again, and `--archive-logs` keeps the old latest_logs folder, which a run otherwise empties. `-c FILE` (or `--settings-file`) names the settings file the tests read.

`sbrs test` starts the run of the tests before Cargo and ends it after: `--dashboard` writes dashboard.html (live while the tests run), `--html PATH` and `--junit-xml PATH` write a report, `--alluredir DIR` Allure results (the tested crate needs the library's `allure` feature), `--show-report` opens the report in a browser window and waits until it is closed, and `--with-db-reporting` records the run in the results database. The exit status is Cargo's, or 1 when a report could not be written. After a plain `cargo test` use `sbrs report finish`.

```bash Usage
sbrs test [FLAGS] [FILTER] [-- HARNESS_ARGS]…
```

**Arguments**

<ParamField path="FILTER" type="string">

Run only the tests whose name contains this text.

</ParamField>

<ParamField path="HARNESS_ARGS" type="string[]">

Arguments for the test harness, after `--` (for example --exact).

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

<ParamField query="-k <EXPR>" type="string">

Select tests by name, as pytest's -k: `login or signup`, `not slow`, `(a or b) and not c`.

</ParamField>

<ParamField query="-m <EXPR>" type="string">

Select tests by marker, as pytest's -m. A marker is a module name: `-m smoke` runs the tests in `mod smoke`.

</ParamField>

<ParamField query="--threads <N|auto>" type="string">

How many tests run at once, as pytest-xdist's -n: a number or `auto` (SB_THREADS).

Also: `-n`

</ParamField>

<ParamField query="--no-capture" type="boolean">

Show what the tests print as they run (pytest's -s).

Also: `--nocapture`, `-s`

</ParamField>

<ParamField query="--co" type="boolean">

List the tests that would run and how many (`collected N items`) without running them.

Also: `--collect-only`

</ParamField>

<ParamField query="--all-scripts" type="boolean">

After the tests, run every example of the packages with `cargo run --example`.

</ParamField>

<ParamField query="--show-report" type="boolean">

Open the HTML report or the dashboard in a browser window when the tests end, and wait until the window is closed.

</ParamField>

<ParamField query="--browser <BROWSER>" type="string">

The browser to run: chrome, edge, firefox, brave, ... (SB_BROWSER)

One of: `chrome`, `chromium`, `edge`, `firefox`, `brave`, `comet`, `opera`, `safari`, `ie`, `cft`, `chs`

</ParamField>

<ParamField query="--headless [HEADLESS]" type="string">

Run without a visible window (Chrome's new headless mode) (SB_HEADLESS)

Also: `--headless2`

One of: `true`, `false`

</ParamField>

<ParamField query="--binary-location <PATH>" type="string">

The browser binary to run (SB_CHROME_BIN)

Also: `--bl`

</ParamField>

<ParamField query="--proxy <VALUE>" type="string">

The proxy server: HOST:PORT or USER:PASS@HOST:PORT (SB_PROXY)

Also: `--proxy-server`, `--proxy-string`

</ParamField>

<ParamField query="--user-data-dir <DIR>" type="string">

The browser profile folder (SB_USER_DATA_DIR)

</ParamField>

<ParamField query="--extension-dir <DIR>" type="string">

Unpacked extension folders to load, separated by commas (SB_EXTENSION_DIR)

</ParamField>

<ParamField query="--locale <VALUE>" type="string">

The browser locale (SB_LOCALE)

</ParamField>

<ParamField query="--user-agent <VALUE>" type="string">

The user agent string (SB_USER_AGENT)

Also: `--agent`

</ParamField>

<ParamField query="--mode <MODE>" type="string">

The driver mode: webdriver, cdp or uc (SB_MODE)

One of: `webdriver`, `cdp`, `uc`

</ParamField>

<ParamField query="--ad-block [AD_BLOCK]" type="string">

Block the requests of ad and tracker hosts (SB_AD_BLOCK)

Also: `--adblock`, `--block-ads`

One of: `true`, `false`

</ParamField>

<ParamField query="--reuse-session [REUSE_SESSION]" type="string">

Share browsers among the tests of the run (SB_REUSE_SESSION)

Also: `--rs`

One of: `true`, `false`

</ParamField>

<ParamField query="--reuse-class-session [REUSE_CLASS_SESSION]" type="string">

Share a browser among the tests of one module (SB_REUSE_CLASS_SESSION)

Also: `--rcs`

One of: `true`, `false`

</ParamField>

<ParamField query="--mobile [MOBILE]" type="string">

Emulate a mobile phone (SB_MOBILE)

Also: `--mobile-emulator`

One of: `true`, `false`

</ParamField>

<ParamField query="--proxy-pac-url <URL>" type="string">

A proxy auto-config URL (SB_PROXY_PAC_URL)

Also: `--pac-url`

</ParamField>

<ParamField query="--esc-end [ESC_END]" type="string">

End the current test when ESC is pressed in the browser (SB_ESC_END)

Also: `--ee`

One of: `true`, `false`

</ParamField>

<ParamField query="--rec-sb-cdp [REC_SB_CDP]" type="string">

Write recordings as pure CDP code (SB_REC_SB_CDP)

One of: `true`, `false`

</ParamField>

<ParamField query="--fingerprint <PATH>" type="string">

A fingerprint file every browser presents (SB_FINGERPRINT)

</ParamField>

<ParamField query="--start-page <URL>" type="string">

The page every browser opens first (SB_START_PAGE)

Also: `--url`

</ParamField>

<ParamField query="--settings-file <PATH>" type="string">

The settings file to read instead of sbase_config.toml (SB_SETTINGS_FILE)

Also: `--settings`

</ParamField>

<ParamField query="--chrome [CHROME]" type="string">

Run Chrome (SB_BROWSER=chrome)

One of: `true`, `false`

</ParamField>

<ParamField query="--edge [EDGE]" type="string">

Run Edge (SB_BROWSER=edge)

One of: `true`, `false`

</ParamField>

<ParamField query="--firefox [FIREFOX]" type="string">

Run Firefox (SB_BROWSER=firefox)

One of: `true`, `false`

</ParamField>

<ParamField query="--brave [BRAVE]" type="string">

Run Brave (SB_BROWSER=brave)

One of: `true`, `false`

</ParamField>

<ParamField query="--comet [COMET]" type="string">

Run Comet (SB_BROWSER=comet)

One of: `true`, `false`

</ParamField>

<ParamField query="--opera [OPERA]" type="string">

Run Opera (SB_BROWSER=opera)

One of: `true`, `false`

</ParamField>

<ParamField query="--safari [SAFARI]" type="string">

Run Safari (SB_BROWSER=safari)

One of: `true`, `false`

</ParamField>

<ParamField query="--ie [IE]" type="string">

Run Internet Explorer (SB_BROWSER=ie)

One of: `true`, `false`

</ParamField>

<ParamField query="--cft [CFT]" type="string">

Run Chrome for Testing (SB_BROWSER=cft)

One of: `true`, `false`

</ParamField>

<ParamField query="--chs [CHS]" type="string">

Run chrome-headless-shell (SB_BROWSER=chs)

One of: `true`, `false`

</ParamField>

<ParamField query="--chromium [CHROMIUM]" type="string">

Run Chromium (SB_BROWSER=chromium)

Also: `--use-chromium`

One of: `true`, `false`

</ParamField>

<ParamField query="--headed [HEADED]" type="string">

Run with a visible window (SB_HEADLESS=false)

Also: `--gui`

One of: `true`, `false`

</ParamField>

<ParamField query="--uc [UC]" type="string">

Run in undetected-chromedriver mode (SB_MODE=uc)

Also: `--undetected`, `--undetectable`

One of: `true`, `false`

</ParamField>

<ParamField query="--headless1 [HEADLESS1]" type="string">

Run headless with chrome-headless-shell (Chrome's old headless mode) (SB_HEADLESS1)

One of: `true`, `false`

</ParamField>

<ParamField query="--locale-code <VALUE>" type="string">

The browser language code, such as fr or ar_eg (SB_LOCALE_CODE)

</ParamField>

<ParamField query="--server <VALUE>" type="string">

The Selenium Grid server to connect to (SB_SERVER)

</ParamField>

<ParamField query="--port <VALUE>" type="string">

The port of the Selenium Grid server (SB_PORT)

</ParamField>

<ParamField query="--protocol <VALUE>" type="string">

The protocol of the Selenium Grid server: http or https (SB_PROTOCOL)

</ParamField>

<ParamField query="--chromium-arg <VALUE>" type="string">

Extra Chromium switches, separated by commas (SB_CHROMIUM_ARG)

</ParamField>

<ParamField query="--dark-mode [DARK_MODE]" type="string">

Force Chrome's dark theme on web pages (SB_DARK_MODE)

Also: `--dark`

One of: `true`, `false`

</ParamField>

<ParamField query="--devtools [DEVTOOLS]" type="string">

Open the developer tools with each tab (headed only) (SB_DEVTOOLS)

Also: `--open-devtools`

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-features <VALUE>" type="string">

Chromium features to switch off, separated by commas (SB_DISABLE_FEATURES)

</ParamField>

<ParamField query="--disable-gpu [DISABLE_GPU]" type="string">

Switch the GPU off (SB_DISABLE_GPU)

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-ws [DISABLE_WS]" type="string">

Switch web security off (SB_DISABLE_WS)

Also: `--dws`, `--disable-web-security`

One of: `true`, `false`

</ParamField>

<ParamField query="--guest-mode [GUEST_MODE]" type="string">

Open the browser as a guest (SB_GUEST_MODE)

Also: `--guest`

One of: `true`, `false`

</ParamField>

<ParamField query="--incognito [INCOGNITO]" type="string">

Open the browser in an incognito window (SB_INCOGNITO)

Also: `--incognito-mode`

One of: `true`, `false`

</ParamField>

<ParamField query="--host-resolver-rules <VALUE>" type="string">

Chromium rules that map host names to addresses (SB_HOST_RESOLVER_RULES)

</ParamField>

<ParamField query="--no-sandbox [NO_SANDBOX]" type="string">

Start Chromium without its sandbox (SB_NO_SANDBOX)

One of: `true`, `false`

</ParamField>

<ParamField query="--swiftshader [SWIFTSHADER]" type="string">

Render with SwiftShader, a software WebGL renderer (SB_SWIFTSHADER)

One of: `true`, `false`

</ParamField>

<ParamField query="--enable-3d-apis [ENABLE_3D_APIS]" type="string">

Keep the GPU on in a headless session, for WebGL (SB_ENABLE_3D_APIS)

One of: `true`, `false`

</ParamField>

<ParamField query="--maximize [MAXIMIZE]" type="string">

Maximize the browser window (SB_MAXIMIZE)

Also: `--maximize-window`, `--fullscreen`

One of: `true`, `false`

</ParamField>

<ParamField query="--block-images [BLOCK_IMAGES]" type="string">

Do not load images (SB_BLOCK_IMAGES)

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-js [DISABLE_JS]" type="string">

Turn JavaScript off (SB_DISABLE_JS)

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-cookies [DISABLE_COOKIES]" type="string">

Refuse every cookie (SB_DISABLE_COOKIES)

One of: `true`, `false`

</ParamField>

<ParamField query="--do-not-track [DO_NOT_TRACK]" type="string">

Send the DNT: 1 header (SB_DO_NOT_TRACK)

One of: `true`, `false`

</ParamField>

<ParamField query="--external-pdf [EXTERNAL_PDF]" type="string">

Download PDF files instead of showing them (SB_EXTERNAL_PDF)

One of: `true`, `false`

</ParamField>

<ParamField query="--enable-sync [ENABLE_SYNC]" type="string">

Leave Chrome Sync on (SB_ENABLE_SYNC)

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-csp [DISABLE_CSP]" type="string">

Strip the Content Security Policy from every response (SB_DISABLE_CSP)

Also: `--dcsp`, `--no-csp`

One of: `true`, `false`

</ParamField>

<ParamField query="--use-auto-ext [USE_AUTO_EXT]" type="string">

Keep Chrome's automation extension (SB_USE_AUTO_EXT)

Also: `--auto-ext`

One of: `true`, `false`

</ParamField>

<ParamField query="--proxy-driver [PROXY_DRIVER]" type="string">

Download the driver through the proxy (SB_PROXY_DRIVER)

One of: `true`, `false`

</ParamField>

<ParamField query="--page-load-strategy <STRATEGY>" type="string">

How long a navigation waits: normal, eager or none (SB_PAGE_LOAD_STRATEGY)

Also: `--pls`

One of: `normal`, `eager`, `none`

</ParamField>

<ParamField query="--window-size <VALUE>" type="string">

The window size: WIDTH,HEIGHT (SB_WINDOW_SIZE)

</ParamField>

<ParamField query="--window-position <VALUE>" type="string">

The window position: X,Y (SB_WINDOW_POSITION)

</ParamField>

<ParamField query="--device-metrics <VALUE>" type="string">

Emulate a phone screen: WIDTH,HEIGHT,PIXEL_RATIO (SB_DEVICE_METRICS)

Also: `--metrics`

</ParamField>

<ParamField query="--proxy-bypass-list <VALUE>" type="string">

Hosts that skip the proxy (SB_PROXY_BYPASS_LIST)

</ParamField>

<ParamField query="--driver-version <VALUE>" type="string">

The driver version: a number, latest, stable, beta, ... (SB_DRIVER_VERSION)

</ParamField>

<ParamField query="--cap-file <PATH>" type="string">

A JSON file of capabilities to add to the session's (SB_CAP_FILE)

</ParamField>

<ParamField query="--cap-string <VALUE>" type="string">

A JSON object of capabilities to add to the session's (SB_CAP_STRING)

</ParamField>

<ParamField query="--extension-zip <VALUE>" type="string">

Extension .zip or .crx files to load, separated by commas (SB_EXTENSION_ZIP)

Also: `--crx`

</ParamField>

<ParamField query="--firefox-arg <VALUE>" type="string">

Firefox arguments, separated by commas (SB_FIREFOX_ARG)

</ParamField>

<ParamField query="--firefox-pref <VALUE>" type="string">

Firefox preferences, NAME:VALUE separated by commas (SB_FIREFOX_PREF)

</ParamField>

<ParamField query="--xvfb [XVFB]" type="string">

Open a virtual display for a headed browser on Linux (SB_XVFB)

One of: `true`, `false`

</ParamField>

<ParamField query="--xvfb-metrics <VALUE>" type="string">

The virtual display size: WIDTH,HEIGHT (SB_XVFB_METRICS)

</ParamField>

<ParamField query="--remote-debug [REMOTE_DEBUG]" type="string">

Open Chrome's remote debugger on port 9222 (SB_REMOTE_DEBUG)

Also: `--remote-debugger`

One of: `true`, `false`

</ParamField>

<ParamField query="--log-cdp-events [LOG_CDP_EVENTS]" type="string">

Log the DevTools Protocol events (SB_LOG_CDP_EVENTS)

Also: `--log-cdp`

One of: `true`, `false`

</ParamField>

<ParamField query="--uc-cdp-events [UC_CDP_EVENTS]" type="string">

Log the DevTools Protocol events in undetected mode (SB_UC_CDP_EVENTS)

Also: `--uc-cdp`

One of: `true`, `false`

</ParamField>

<ParamField query="--demo [DEMO]" type="string">

Highlight each element and pause after an action (SB_DEMO)

Also: `--demo-mode`

One of: `true`, `false`

</ParamField>

<ParamField query="--slow [SLOW]" type="string">

Pause briefly after each action (SB_SLOW)

Also: `--slow-mode`, `--slowmo`

One of: `true`, `false`

</ParamField>

<ParamField query="--demo-sleep <SECONDS>" type="string">

Seconds to pause after each action in demo mode (SB_DEMO_SLEEP)

</ParamField>

<ParamField query="--message-duration <SECONDS>" type="string">

Seconds a message stays on the page (SB_MESSAGE_DURATION)

</ParamField>

<ParamField query="--skip-js-waits [SKIP_JS_WAITS]" type="string">

Do not wait for the page's JavaScript to settle (SB_SKIP_JS_WAITS)

Also: `--sjw`

One of: `true`, `false`

</ParamField>

<ParamField query="--highlights <COUNT>" type="string">

How many times to flash a highlighted element (SB_HIGHLIGHTS)

</ParamField>

<ParamField query="--interval <SECONDS>" type="string">

Seconds between the steps of a tour or presentation (SB_INTERVAL)

</ParamField>

<ParamField query="--verify-delay <SECONDS>" type="string">

Seconds to wait before each MasterQA verification (SB_VERIFY_DELAY)

</ParamField>

<ParamField query="--check-js [CHECK_JS]" type="string">

Check each page for JavaScript errors after it opens (SB_CHECK_JS)

One of: `true`, `false`

</ParamField>

<ParamField query="--wait-for-angularjs [WAIT_FOR_ANGULARJS]" type="string">

Wait for AngularJS after opening a page and clicking (SB_WAIT_FOR_ANGULARJS)

Also: `--wfa`

One of: `true`, `false`

</ParamField>

<ParamField query="--disable-beforeunload [DISABLE_BEFOREUNLOAD]" type="string">

Remove the beforeunload prompt after each page opens (SB_DISABLE_BEFOREUNLOAD)

One of: `true`, `false`

</ParamField>

<ParamField query="--visual-baseline [VISUAL_BASELINE]" type="string">

Set the visual baselines again instead of comparing (SB_VISUAL_BASELINE)

One of: `true`, `false`

</ParamField>

<ParamField query="--crumbs [CRUMBS]" type="string">

Delete the cookies when a reused browser is handed to the next test (SB_CRUMBS)

One of: `true`, `false`

</ParamField>

<ParamField query="--timeout-multiplier <VALUE>" type="string">

Multiply the default waits by this number (SB_TIMEOUT_MULTIPLIER)

</ParamField>

<ParamField query="--rec-sleep [REC_SLEEP]" type="string">

Keep the sleeps of a recording (SB_REC_SLEEP)

Also: `--record-sleep`

One of: `true`, `false`

</ParamField>

<ParamField query="--env <ENV>" type="string">

The environment the tests run against: qa, staging, production, ... (SB_ENV)

One of: `qa`, `rc`, `staging`, `develop`, `production`, `performance`, `replica`, `fedramp`, `offline`, `online`, `master`, `remote`, `legacy`, `local`, `alpha`, `beta`, `demo`, `gdpr`, `main`, `test`, `gov`, `new`, `old`, `uat`

</ParamField>

<ParamField query="--account <VALUE>" type="string">

A free account name for the tests (SB_ACCOUNT)

</ParamField>

<ParamField query="--data <VALUE>" type="string">

Free test data (SB_DATA)

</ParamField>

<ParamField query="--var1 <VALUE>" type="string">

A free variable for the tests (SB_VAR1)

</ParamField>

<ParamField query="--var2 <VALUE>" type="string">

A free variable for the tests (SB_VAR2)

</ParamField>

<ParamField query="--var3 <VALUE>" type="string">

A free variable for the tests (SB_VAR3)

</ParamField>

<ParamField query="--variables <VALUE>" type="string">

A dictionary of values: JSON or a Python dictionary literal (SB_VARIABLES)

</ParamField>

<ParamField query="--pdb [PDB]" type="string">

Stop in a post-mortem prompt when a test fails (SB_PDB)

One of: `true`, `false`

</ParamField>

<ParamField query="--trace [TRACE]" type="string">

Stop in a prompt before each test's body (SB_TRACE)

One of: `true`, `false`

</ParamField>

<ParamField query="--final-debug [FINAL_DEBUG]" type="string">

Stop in a prompt after each test, before the browser quits (SB_FINAL_DEBUG)

Also: `--fdebug`, `--final-trace`, `--ftrace`

One of: `true`, `false`

</ParamField>

<ParamField query="--reruns <COUNT>" type="string">

Run a failing test again this many times (SB_RERUNS)

</ParamField>

<ParamField query="--reruns-delay <SECONDS>" type="string">

Seconds to wait between reruns (SB_RERUNS_DELAY)

</ParamField>

<ParamField query="--time-limit <SECONDS>" type="string">

Fail a test that runs longer than this many seconds (SB_TIME_LIMIT)

Also: `--timelimit`

</ParamField>

<ParamField query="--runner <RUNNER>" type="string">

The test runner: cargo or nextest (SB_TEST_RUNNER)

One of: `cargo`, `nextest`

</ParamField>

<ParamField query="--screenshot [SCREENSHOT]" type="string">

Save a screenshot of passing tests too (SB_SAVE_SCREENSHOT)

Also: `--save-screenshot`, `--ss`

One of: `true`, `false`

</ParamField>

<ParamField query="--no-screenshot [NO_SCREENSHOT]" type="string">

Save no screenshot of failing tests (SB_NO_SCREENSHOT)

Also: `--ns`

One of: `true`, `false`

</ParamField>

<ParamField query="--list-fail-page [LIST_FAIL_PAGE]" type="string">

Print the URL of the page a failing test ended on (SB_LIST_FAIL_PAGE)

Also: `--list-fail-pages`, `--lfp`

One of: `true`, `false`

</ParamField>

<ParamField query="--archive-logs [ARCHIVE_LOGS]" type="string">

Keep the old latest_logs folder in archived_logs (SB_ARCHIVE_LOGS)

One of: `true`, `false`

</ParamField>

<ParamField query="--archive-downloads [ARCHIVE_DOWNLOADS]" type="string">

Keep the old downloaded_files folder in archived_files (SB_ARCHIVE_DOWNLOADS)

One of: `true`, `false`

</ParamField>

<ParamField query="--dashboard [DASHBOARD]" type="string">

Write the results dashboard (SB_DASHBOARD)

One of: `true`, `false`

</ParamField>

<ParamField query="--dash-title <VALUE>" type="string">

The dashboard's title (SB_DASH_TITLE)

</ParamField>

<ParamField query="--html <PATH>" type="string">

Write an HTML report to this file (SB_HTML)

</ParamField>

<ParamField query="--junit-xml <PATH>" type="string">

Write a JUnit XML report to this file (SB_JUNIT_XML)

Also: `--junitxml`

</ParamField>

<ParamField query="--alluredir <PATH>" type="string">

Write Allure results to this folder (SB_ALLURE_DIR)

Also: `--allure-dir`

</ParamField>

<ParamField query="--with-db-reporting [WITH_DB_REPORTING]" type="string">

Record each test's outcome in the results database (SB_WITH_DB_REPORTING)

One of: `true`, `false`

</ParamField>

<ParamField query="--database-env <VALUE>" type="string">

Which results database to use (SB_DATABASE_ENV)

</ParamField>

<ParamField query="--with-s3-logging [WITH_S3_LOGGING]" type="string">

Upload each test's logs to S3 (SB_WITH_S3_LOGGING)

One of: `true`, `false`

</ParamField>

<ParamField query="--rec [REC]" type="string">

Record the browser actions as a test (SB_RECORDER)

Also: `--record`, `--recorder`, `--codegen`

One of: `true`, `false`

</ParamField>

<ParamField query="--rec-behave [REC_BEHAVE]" type="string">

Also write the recording as a Gherkin feature (SB_REC_BEHAVE)

Also: `--rec-gherkin`

One of: `true`, `false`

</ParamField>

<ParamField query="--rec-print [REC_PRINT]" type="string">

Print the recording when the test ends (SB_REC_PRINT)

One of: `true`, `false`

</ParamField>

<ParamField query="--rec-sb-mgr [REC_SB_MGR]" type="string">

Write the recording as a script (SB_REC_SB_MGR)

One of: `true`, `false`

</ParamField>

```bash Examples
sbrs test login
sbrs test -k 'login and not slow' --runner nextest --headless --browser firefox
```

## sbrs translate

Write the actions of a JSON scenario in another language.

Rewrites each step's `action` (`click`, `open`, ...) with the translation dictionary and changes nothing else in the file; the steps keep running, because a scenario can be read in any of the languages. Python SeleniumBase translates Python test files; a Rust test is compiled code, so this translates the portable JSON scenario instead.

The language is a flag, `--en`, `--zh`, `--nl`, `--fr`, `--it`, `--ja`, `--ko`, `--pt`, `--ru` or `--es` (also by name: `--spanish`). The action is `-p`/`--print` (the default), `-o`/`--overwrite` or `-c`/`--copy`; `-p` may go with one of the other two. `-n` numbers the printed lines.

```bash Usage
sbrs translate [FLAGS] <FILE>
```

**Arguments**

<ParamField path="FILE" type="string" required>

The scenario file.

</ParamField>

**Flags**

<ParamField query="--en" type="boolean">

English strings

</ParamField>

<ParamField query="--zh" type="boolean">

Chinese strings

</ParamField>

<ParamField query="--nl" type="boolean">

Dutch strings

</ParamField>

<ParamField query="--fr" type="boolean">

French strings

</ParamField>

<ParamField query="--it" type="boolean">

Italian strings

</ParamField>

<ParamField query="--ja" type="boolean">

Japanese strings

</ParamField>

<ParamField query="--ko" type="boolean">

Korean strings

</ParamField>

<ParamField query="--pt" type="boolean">

Portuguese strings

</ParamField>

<ParamField query="--ru" type="boolean">

Russian strings

</ParamField>

<ParamField query="--es" type="boolean">

Spanish strings

</ParamField>

<ParamField query="--print" type="boolean">

Print the translation (the default when no other action is given). With -o or -c it is printed as well as written.

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

<ParamField query="--line-numbers" type="boolean">

Put the line number in front of every printed line.

Also: `-n`

</ParamField>

```bash Examples
sbrs translate scenario.json --zh -p -n
sbrs translate scenario.json --es --copy
```

## sbrs type-alert-text

Type text into the prompt that the page opens when it loads.

```bash Usage
sbrs type-alert-text <--text <TEXT>>
```

**Flags**

<ParamField query="--text <TEXT>" type="string" required>

Text to type into the alert prompt

</ParamField>

```bash Examples
sbrs --url https://example.com type-alert-text --text hello
```

## sbrs uc-click

Perform a UC-mode click on an element.

```bash Usage
sbrs uc-click <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --uc --url https://example.com uc-click --css a
```

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

```bash Examples
sbrs --uc --url https://example.com uc-type --css '#q' --text hello
```

## sbrs uncheck-if-checked

Uncheck a checkbox only if it is checked.

```bash Usage
sbrs uncheck-if-checked <--css <CSS>>
```

**Flags**

<ParamField query="--css <CSS>" type="string" required>

CSS selector for the target element

</ParamField>

```bash Examples
sbrs --url https://example.com uncheck-if-checked --css '#agree'
```

## sbrs version

Print the sbrs version, as `sbrs --version` does.

```bash Usage
sbrs version
```

```bash Examples
sbrs version
```

## sbrs wait-for-and-accept-alert

Wait for an alert that the page opens, then accept it.

```bash Usage
sbrs wait-for-and-accept-alert [--timeout <TIMEOUT>]
```

**Flags**

<ParamField query="--timeout <TIMEOUT>" type="string" default="10">

Maximum wait time in seconds

</ParamField>

```bash Examples
sbrs --url https://example.com wait-for-and-accept-alert --timeout 5
```

## sbrs wait-for-and-dismiss-alert

Wait for an alert that the page opens, then dismiss it.

```bash Usage
sbrs wait-for-and-dismiss-alert [--timeout <TIMEOUT>]
```

**Flags**

<ParamField query="--timeout <TIMEOUT>" type="string" default="10">

Maximum wait time in seconds

</ParamField>

```bash Examples
sbrs --url https://example.com wait-for-and-dismiss-alert --timeout 5
```

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

```bash Examples
sbrs --url https://example.com wait-for-element-not-visible --css '#spinner' --timeout 5
```

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

```bash Examples
sbrs --url https://example.com wait-for-element-present --css p --timeout 5
```

## sbrs wait-for-ready-state-complete

Wait until the document readyState is complete.

```bash Usage
sbrs wait-for-ready-state-complete
```

```bash Examples
sbrs --url https://example.com wait-for-ready-state-complete
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

```bash Examples
sbrs --url https://example.com wait-for-text --css p --text 'This domain is for use in' --timeout 5
```

