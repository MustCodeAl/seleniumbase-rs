---
title: "CLI reference"
---

*Generated from the usage spec of `sbrs` (`completions/sbrs.usage.kdl`) by `SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec`. Do not edit it by hand.*

# `sbrs`
- **Version:** 0.4.0 (seleniumbase-rs)

A Rust port of the Python SeleniumBase testing framework. Provides browser automation, stealth/undetected modes, and a command-line helper.


- **Usage:** `sbrs [FLAGS] <SUBCOMMAND>`

## Flags
- **`--webdriver <URL>`** — WebDriver server URL (default: SB_WEBDRIVER_URL, else http://localhost:4444)
- **`--browser <BROWSER>`** — Browser engine to launch (default: chrome, or the config file's)

  **Choices:** `chrome`, `chromium`, `edge`, `firefox`, `brave`, `comet`, `opera`, `safari`, `ie`, `cft`, `chs`
- **`--headed`** — Run the browser in headed mode
- **`--headless`** — Run the browser in headless mode
- **`--cdp`** — Enable Chrome DevTools Protocol mode
- **`--uc`** — Enable undetected-chromedriver style evasions
- **`--user-agent <USER_AGENT>`** — Override the default user agent string
- **`-a --agent <AGENT>`** — Alias for --user-agent
- **`--locale <LOCALE>`** — Locale string for the browser context
- **`--ad-block`** — Refuse the requests of ad and tracker hosts (Chromium WebDriver sessions)
- **`--proxy <PROXY>`** — Proxy (host:port or scheme://host:port); with --proxy-list, a name from the list
- **`--proxy-list <FILE>`** — File of proxies to choose from; without --proxy a random one is used
- **`--proxy-pac-url <PROXY_PAC_URL>`** — URL to a proxy auto-config (PAC) file
- **`--user-data-dir <USER_DATA_DIR>`** — Path to a persistent browser user-data directory
- **`--extension-dir <EXTENSION_DIR>`** — Path to an unpacked extension directory
- **`--reuse-session`** — Share browsers among the tests of the run (SB_REUSE_SESSION)

  **Aliases:** `--reuse_session`, `--rs`
- **`--rcs`** — Share a browser among the tests of one Rust module (SB_REUSE_CLASS_SESSION)

  **Aliases:** `--reuse-class-session`, `--reuse_class_session`
- **`--mobile`** — Use a mobile device emulation profile
- **`--esc-end`** — End the current test when ESC is pressed in the browser

  **Aliases:** `--esc_end`, `--ee`
- **`--rec-sb-cdp`** — Write recordings as pure CDP code (sb_cdp Page and Locator calls)

  **Aliases:** `--rec_sb_cdp`
- **`-n --threads <THREADS>`** — Number of parallel threads for test runs
- **`-c --config <CONFIG>`** — Path to a TOML/JSON settings file
- **`--plugin <NAME>`** — Turn on a built-in plugin in the browsers the command starts (repeatable)

  Adds to the plugins the settings file (the [plugins] table) and SB_PLUGINS turn on. Precedence, highest first: the command line, then SB_PLUGINS (which replaces the names the file enables), then the settings file. `sbrs plugin list` shows the plugins this build has. `sbrs test` passes the names on to the tests as SB_PLUGINS.
- **`--plugin-opt <NAME.KEY=VALUE>`** — Set one option of a plugin, NAME.KEY=VALUE with a TOML VALUE (repeatable)

  Quote the whole option so the shell keeps the quotes of a string: --plugin-opt 'shield.level="strict"'. An option given here replaces the same option from SB_PLUGINS or the settings file and keeps the plugin's other options, so the precedence is the command line, then SB_PLUGINS, then the file. The plugin must be turned on by --plugin, SB_PLUGINS or the file. Not available for `sbrs test`: put the options in the [plugins.NAME] table of the settings file.
- **`-h --help`** — Print help
- **`-V --version`** — Print version

## `sbrs version`

- **Usage:** `sbrs version`

Print the sbrs version, as `sbrs --version` does.

### Flags
- **`-h --help`** — Print help

## `sbrs open`

- **Usage:** `sbrs open <URL>`

Open a URL in the browser.

### Arguments
- **`<URL>`** — URL to open in the browser.

### Flags
- **`-h --help`** — Print help

## `sbrs smoke`

- **Usage:** `sbrs smoke [--title-contains <TITLE_CONTAINS>] <URL>`

Run a smoke test against a URL.

### Arguments
- **`<URL>`** — URL to open or smoke-test

### Flags
- **`--title-contains <TITLE_CONTAINS>`** — Expected substring in the page title
- **`-h --help`** — Print help

## `sbrs cdp`

- **Usage:** `sbrs cdp <--cmd <CMD>> [--params <PARAMS>]`

Execute a Chrome DevTools Protocol command.

### Flags
- **`--cmd <CMD>`** — CDP method name (e.g. Runtime.evaluate)
- **`--params <PARAMS>`** — JSON object of CDP parameters
- **`-h --help`** — Print help

## `sbrs cache-clear`

- **Usage:** `sbrs cache-clear`

Clear the browser cache via CDP.

### Flags
- **`-h --help`** — Print help

## `sbrs throttle3g`

- **Usage:** `sbrs throttle3g`

Throttle the browser connection to 3G speeds.

### Flags
- **`-h --help`** — Print help

## `sbrs screenshot`

- **Usage:** `sbrs screenshot [--path <PATH>]`

Capture a screenshot of the current page.

### Flags
- **`--path <PATH>`** — Output image file path (default: a new file in latest_logs/)
- **`-h --help`** — Print help

## `sbrs save-source`

- **Usage:** `sbrs save-source [--path <PATH>]`

Save the current page source to a file.

### Flags
- **`--path <PATH>`** — Output HTML file path (default: a new file in latest_logs/)
- **`-h --help`** — Print help

## `sbrs assert-element`

- **Usage:** `sbrs assert-element <--css <CSS>>`

Assert that an element exists.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs wait-for-text`

- **Usage:** `sbrs wait-for-text <FLAGS>`

Wait until an element contains the expected text.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--text <TEXT>`** — Expected text substring to wait for
- **`--timeout <TIMEOUT>`** — Maximum wait time in seconds

  **Default:** `10`
- **`-h --help`** — Print help

## `sbrs hover`

- **Usage:** `sbrs hover <--css <CSS>>`

Hover over an element.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs hover-and-click`

- **Usage:** `sbrs hover-and-click <--hover-css <HOVER_CSS>> <--click-css <CLICK_CSS>>`

Hover over one element, then click another.

### Flags
- **`--hover-css <HOVER_CSS>`** — CSS selector to hover over
- **`--click-css <CLICK_CSS>`** — CSS selector to click
- **`-h --help`** — Print help

## `sbrs select-option`

- **Usage:** `sbrs select-option <FLAGS>`

Select an option from a dropdown.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--text <TEXT>`** — Visible text of the option to select
- **`--value <VALUE>`** — Option value attribute
- **`-h --help`** — Print help

## `sbrs drag-and-drop`

- **Usage:** `sbrs drag-and-drop <--source-css <SOURCE_CSS>> <--target-css <TARGET_CSS>>`

Drag one element onto another.

### Flags
- **`--source-css <SOURCE_CSS>`** — CSS selector of the element to drag
- **`--target-css <TARGET_CSS>`** — CSS selector of the drop target
- **`-h --help`** — Print help

## `sbrs cdp-click-element`

- **Usage:** `sbrs cdp-click-element <--css <CSS>>`

Click an element using CDP.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs cdp-type-text`

- **Usage:** `sbrs cdp-type-text <--text <TEXT>>`

Type text using CDP input injection.

### Flags
- **`--text <TEXT>`** — Text to inject via CDP
- **`-h --help`** — Print help

## `sbrs go-back`

- **Usage:** `sbrs go-back`

Navigate back in browser history.

### Flags
- **`-h --help`** — Print help

## `sbrs go-forward`

- **Usage:** `sbrs go-forward`

Navigate forward in browser history.

### Flags
- **`-h --help`** — Print help

## `sbrs refresh`

- **Usage:** `sbrs refresh`

Reload the current page.

### Flags
- **`-h --help`** — Print help

## `sbrs get-text`

- **Usage:** `sbrs get-text <--css <CSS>>`

Print the visible text of an element.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs get-attribute`

- **Usage:** `sbrs get-attribute <--css <CSS>> <--attribute <ATTRIBUTE>>`

Print an element attribute value.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--attribute <ATTRIBUTE>`** — Attribute name to read or assert
- **`-h --help`** — Print help

## `sbrs get-property`

- **Usage:** `sbrs get-property <--css <CSS>> <--property <PROPERTY>>`

Print an element property value.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--property <PROPERTY>`** — Property name to read
- **`-h --help`** — Print help

## `sbrs get-title`

- **Usage:** `sbrs get-title`

Print the page title.

### Flags
- **`-h --help`** — Print help

## `sbrs get-current-url`

- **Usage:** `sbrs get-current-url`

Print the current URL.

### Flags
- **`-h --help`** — Print help

## `sbrs clear-cookies`

- **Usage:** `sbrs clear-cookies`

Delete all browser cookies.

### Flags
- **`-h --help`** — Print help

## `sbrs accept-alert`

- **Usage:** `sbrs accept-alert`

Accept the active browser alert.

### Flags
- **`-h --help`** — Print help

## `sbrs dismiss-alert`

- **Usage:** `sbrs dismiss-alert`

Dismiss the active browser alert.

### Flags
- **`-h --help`** — Print help

## `sbrs get-alert-text`

- **Usage:** `sbrs get-alert-text`

Print the text of the active alert.

### Flags
- **`-h --help`** — Print help

## `sbrs type-alert-text`

- **Usage:** `sbrs type-alert-text <--text <TEXT>>`

Type text into the active alert prompt.

### Flags
- **`--text <TEXT>`** — Text to type into the alert prompt
- **`-h --help`** — Print help

## `sbrs clear-local-storage`

- **Usage:** `sbrs clear-local-storage`

Clear the page local storage.

### Flags
- **`-h --help`** — Print help

## `sbrs get-local-storage-item`

- **Usage:** `sbrs get-local-storage-item <--key <KEY>>`

Print a local storage value by key.

### Flags
- **`--key <KEY>`** — Storage key
- **`-h --help`** — Print help

## `sbrs set-local-storage-item`

- **Usage:** `sbrs set-local-storage-item <--key <KEY>> <--value <VALUE>>`

Set a local storage key/value pair.

### Flags
- **`--key <KEY>`** — Storage key
- **`--value <VALUE>`** — Value to store in local storage
- **`-h --help`** — Print help

## `sbrs remove-local-storage-item`

- **Usage:** `sbrs remove-local-storage-item <--key <KEY>>`

Remove a local storage entry by key.

### Flags
- **`--key <KEY>`** — Storage key
- **`-h --help`** — Print help

## `sbrs switch-to-window`

- **Usage:** `sbrs switch-to-window <--handle <HANDLE>>`

Switch focus to a window by handle.

### Flags
- **`--handle <HANDLE>`** — Window handle to switch to
- **`-h --help`** — Print help

## `sbrs switch-to-frame`

- **Usage:** `sbrs switch-to-frame <--css <CSS>>`

Switch focus to an iframe.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs switch-to-default-content`

- **Usage:** `sbrs switch-to-default-content`

Switch focus back to the main document.

### Flags
- **`-h --help`** — Print help

## `sbrs get-cookies`

- **Usage:** `sbrs get-cookies`

Print all browser cookies as JSON.

### Flags
- **`-h --help`** — Print help

## `sbrs export-recording`

- **Usage:** `sbrs export-recording`

Export the recorded test actions.

### Flags
- **`-h --help`** — Print help

## `sbrs patch-chromedriver`

- **Usage:** `sbrs patch-chromedriver <--path <PATH>>`

Patch a chromedriver binary to reduce detection surface.

### Flags
- **`--path <PATH>`** — Path to the chromedriver binary
- **`-h --help`** — Print help

## `sbrs patch-chrome`

- **Usage:** `sbrs patch-chrome <--path <PATH>> [--cache-dir <CACHE_DIR>]`

Patch a Chrome/Chromium binary for native-level spoofing.

### Flags
- **`--path <PATH>`** — Path to the Chrome/Chromium binary
- **`--cache-dir <CACHE_DIR>`** — Directory where patched binaries are cached
- **`-h --help`** — Print help

## `sbrs encrypt`

- **Usage:** `sbrs encrypt [TEXT]`
- **Aliases:** `obfuscate`

Encrypt text with a passphrase (AES-256-GCM); read from stdin if TEXT is omitted.

The passphrase comes from the SB_ENCRYPTION_KEY environment variable, never from an argument, so it stays out of shell history and process listings. Python's `sbrs obfuscate` is the same command.

### Arguments
- **`[TEXT]`** — The text to encrypt.

### Flags
- **`-h --help`** — Print help

## `sbrs decrypt`

- **Usage:** `sbrs decrypt [TOKEN]`
- **Aliases:** `unobfuscate`

Decrypt a token made by `sbrs encrypt`; read from stdin if TOKEN is omitted. Python's `sbrs unobfuscate` is the same command.

### Arguments
- **`[TOKEN]`** — The token to decrypt.

### Flags
- **`-h --help`** — Print help

## `sbrs doctor`

- **Usage:** `sbrs doctor`

Run a diagnostic check on the environment and configuration.

### Flags
- **`-h --help`** — Print help

## `sbrs prompt`

- **Usage:** `sbrs prompt [FLAGS]`

Print one short line for a shell prompt: the installed Chrome's major version and whether a matching chromedriver is at hand.

Prints `chrome 155 ✓driver`, with `✗driver` when the chromedriver is missing or of another version and `?driver` when its version cannot be read. It starts no browser and uses no network: versions come from files (a macOS app's Info.plist, the version folder beside chrome.exe), and one that only `--version` tells is cached for ten minutes in the user cache folder. Prints nothing and exits with 0 when no browser is found; --verbose says why, on standard error. The chromedriver is looked for as `sbrs doctor` does: CHROMEDRIVER_PATH, downloaded_drivers/, PATH.

### Flags
- **`--format <TEMPLATE>`** — The line to print; placeholders {browser} {major} {driver} {mark} {driver_major}, {{ and }} for braces

  **Default:** `{browser} {major} {driver}`
- **`--ascii`** — Mark the driver with + - ? instead of ✓ ✗ ?
- **`--browser-path <PATH>`** — Browser to report on (default: SB_CHROME_BIN, else the installed Chrome or Chromium)
- **`--no-cache`** — Run --version again instead of using the versions cached in the user cache folder
- **`--verbose`** — Explain on standard error what was found and how
- **`-h --help`** — Print help

## `sbrs assert-text-visible`

- **Usage:** `sbrs assert-text-visible <--css <CSS>> <--text <TEXT>>`

Assert that an element contains the expected text.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--text <TEXT>`** — Expected visible text
- **`-h --help`** — Print help

## `sbrs assert-text-not-visible`

- **Usage:** `sbrs assert-text-not-visible <--css <CSS>> <--text <TEXT>>`

Assert that an element does not contain the given text.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--text <TEXT>`** — Text that should not be visible
- **`-h --help`** — Print help

## `sbrs assert-attribute`

- **Usage:** `sbrs assert-attribute <FLAGS>`

Assert an element attribute equals an expected value.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--attribute <ATTRIBUTE>`** — Attribute name to read or assert
- **`--value <VALUE>`** — Expected attribute value
- **`-h --help`** — Print help

## `sbrs assert-title`

- **Usage:** `sbrs assert-title <--text <TEXT>>`

Assert the page title contains the expected text.

### Flags
- **`--text <TEXT>`** — Expected page title substring
- **`-h --help`** — Print help

## `sbrs wait-for-ready-state-complete`

- **Usage:** `sbrs wait-for-ready-state-complete`

Wait until the document readyState is complete.

### Flags
- **`-h --help`** — Print help

## `sbrs get-window-position`

- **Usage:** `sbrs get-window-position`

Print the browser window position.

### Flags
- **`-h --help`** — Print help

## `sbrs set-window-position`

- **Usage:** `sbrs set-window-position <--x <X>> <--y <Y>>`

Set the browser window position.

### Flags
- **`--x <X>`** — Window X coordinate in pixels
- **`--y <Y>`** — Window Y coordinate in pixels
- **`-h --help`** — Print help

## `sbrs close-window`

- **Usage:** `sbrs close-window`

Close the current browser window.

### Flags
- **`-h --help`** — Print help

## `sbrs switch-to-parent-frame`

- **Usage:** `sbrs switch-to-parent-frame`

Switch focus to the parent frame.

### Flags
- **`-h --help`** — Print help

## `sbrs is-element-visible`

- **Usage:** `sbrs is-element-visible <--css <CSS>>`

Print whether an element is visible.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs is-text-visible`

- **Usage:** `sbrs is-text-visible <--css <CSS>> <--text <TEXT>>`

Print whether the expected text is visible in an element.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--text <TEXT>`** — Expected text to check
- **`-h --help`** — Print help

## `sbrs wait-for-element-not-visible`

- **Usage:** `sbrs wait-for-element-not-visible <--css <CSS>> [--timeout <TIMEOUT>]`

Wait until an element is no longer visible.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--timeout <TIMEOUT>`** — Maximum wait time in seconds

  **Default:** `10`
- **`-h --help`** — Print help

## `sbrs save-cookies`

- **Usage:** `sbrs save-cookies <--file <FILE>>`

Save browser cookies to a JSON file.

### Flags
- **`--file <FILE>`** — JSON file to save cookies to
- **`-h --help`** — Print help

## `sbrs load-cookies`

- **Usage:** `sbrs load-cookies <--file <FILE>>`

Load browser cookies from a JSON file.

### Flags
- **`--file <FILE>`** — JSON file to load cookies from
- **`-h --help`** — Print help

## `sbrs highlight-click`

- **Usage:** `sbrs highlight-click <--css <CSS>>`

Highlight an element, then click it.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs is-checked`

- **Usage:** `sbrs is-checked <--css <CSS>>`

Print whether a checkbox is checked.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs check-if-unchecked`

- **Usage:** `sbrs check-if-unchecked <--css <CSS>>`

Check a checkbox only if it is unchecked.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs uncheck-if-checked`

- **Usage:** `sbrs uncheck-if-checked <--css <CSS>>`

Uncheck a checkbox only if it is checked.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs open-new-window`

- **Usage:** `sbrs open-new-window`

Open a new browser window.

### Flags
- **`-h --help`** — Print help

## `sbrs open-new-tab`

- **Usage:** `sbrs open-new-tab`

Open a new browser tab.

### Flags
- **`-h --help`** — Print help

## `sbrs switch-to-newest-window`

- **Usage:** `sbrs switch-to-newest-window`

Switch focus to the newest browser window.

### Flags
- **`-h --help`** — Print help

## `sbrs switch-to-default-window`

- **Usage:** `sbrs switch-to-default-window`

Switch focus to the default window.

### Flags
- **`-h --help`** — Print help

## `sbrs get-active-element-css`

- **Usage:** `sbrs get-active-element-css`

Print a CSS selector for the currently focused element.

### Flags
- **`-h --help`** — Print help

## `sbrs wait-for-element-present`

- **Usage:** `sbrs wait-for-element-present <--css <CSS>> [--timeout <TIMEOUT>]`

Wait until an element is present in the DOM.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--timeout <TIMEOUT>`** — Maximum wait time in seconds

  **Default:** `10`
- **`-h --help`** — Print help

## `sbrs add-text`

- **Usage:** `sbrs add-text <--css <CSS>> <--text <TEXT>>`

Append text to an input or textarea.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--text <TEXT>`** — Text to append to the element
- **`-h --help`** — Print help

## `sbrs send-keys`

- **Usage:** `sbrs send-keys <--css <CSS>> <--text <TEXT>>`

Send keystrokes to an element.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--text <TEXT>`** — Keys to send to the element
- **`-h --help`** — Print help

## `sbrs get-value`

- **Usage:** `sbrs get-value <--css <CSS>>`

Print the value of a form element.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs click-visible-elements`

- **Usage:** `sbrs click-visible-elements <--css <CSS>>`

Click all visible elements matching a selector.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs wait-for-and-accept-alert`

- **Usage:** `sbrs wait-for-and-accept-alert [--timeout <TIMEOUT>]`

Wait for an alert, then accept it.

### Flags
- **`--timeout <TIMEOUT>`** — Maximum wait time in seconds

  **Default:** `10`
- **`-h --help`** — Print help

## `sbrs wait-for-and-dismiss-alert`

- **Usage:** `sbrs wait-for-and-dismiss-alert [--timeout <TIMEOUT>]`

Wait for an alert, then dismiss it.

### Flags
- **`--timeout <TIMEOUT>`** — Maximum wait time in seconds

  **Default:** `10`
- **`-h --help`** — Print help

## `sbrs is-link-text-visible`

- **Usage:** `sbrs is-link-text-visible <--text <TEXT>>`

Print whether a link with the given text is visible.

### Flags
- **`--text <TEXT>`** — Exact link text to check
- **`-h --help`** — Print help

## `sbrs is-partial-link-text-visible`

- **Usage:** `sbrs is-partial-link-text-visible <--text <TEXT>>`

Print whether a link containing the given text is visible.

### Flags
- **`--text <TEXT>`** — Link text substring to check
- **`-h --help`** — Print help

## `sbrs assert-link-text`

- **Usage:** `sbrs assert-link-text <--text <TEXT>>`

Assert a link with the given text is visible.

### Flags
- **`--text <TEXT>`** — Exact link text to assert
- **`-h --help`** — Print help

## `sbrs click-partial-link-text`

- **Usage:** `sbrs click-partial-link-text <--text <TEXT>>`

Click a link containing the given text.

### Flags
- **`--text <TEXT>`** — Link text substring to click
- **`-h --help`** — Print help

## `sbrs human-type`

- **Usage:** `sbrs human-type <--css <CSS>> <--text <TEXT>>`

Type text with human-like timing and noise.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--text <TEXT>`** — Text to type with humanized timing
- **`-h --help`** — Print help

## `sbrs human-click`

- **Usage:** `sbrs human-click <--css <CSS>>`

Click an element with human-like timing and noise.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs smooth-scroll-to`

- **Usage:** `sbrs smooth-scroll-to <--css <CSS>>`

Smoothly scroll an element into view.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs uc-click`

- **Usage:** `sbrs uc-click <--css <CSS>>`

Perform a UC-mode click on an element.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`-h --help`** — Print help

## `sbrs uc-type`

- **Usage:** `sbrs uc-type <--css <CSS>> <--text <TEXT>>`

Perform UC-mode typing into an element.

### Flags
- **`--css <CSS>`** — CSS selector for the target element
- **`--text <TEXT>`** — Text to type in UC mode
- **`-h --help`** — Print help

## `sbrs install`

- **Usage:** `sbrs install [DRIVER] [VERSION]`
- **Aliases:** `get`

Download a driver into ./downloaded_drivers/ (Python's `sbrs get`).

Without a version, chromedriver and edgedriver are the drivers for the installed Chrome and Edge, and geckodriver is the newest release.

### Arguments
- **`[DRIVER]`** — The driver: chromedriver, geckodriver (or firefoxdriver), edgedriver (or msedgedriver).

  **Default:** `chromedriver`
- **`[VERSION]`** — `latest`, a major version (chromedriver, edgedriver), a full build, or a geckodriver version such as 0.36.0.

### Flags
- **`-h --help`** — Print help

## `sbrs test`

- **Usage:** `sbrs test [FLAGS] [FILTER] [-- HARNESS_ARGS]…`

Run the project's tests, or one example, through Cargo.

The global options reach the tests as SB_* environment variables (for example `sbrs --headless --browser firefox test` sets SB_HEADLESS and SB_BROWSER), so a test that loads `Settings` sees them, and `-n` sets how many tests run at once. `--plugin` reaches them as SB_PLUGINS, with the plugins the settings file and the variable already turn on; `--plugin-opt` cannot be passed on, so `sbrs test` refuses it.

### Arguments
- **`[FILTER]`** — Run only the tests whose name contains this text.
- **`[-- HARNESS_ARGS]…`** — Arguments for the test harness, after `--` (for example --nocapture).

### Flags
- **`--test <NAME>`** — Run one integration test file from tests/.
- **`--example <NAME>`** — Run one example from examples/ instead of the tests.
- **`--release`** — Build with the release profile.
- **`--features <FEATURES>`** — Cargo features to enable, separated by commas.
- **`-h --help`** — Print help

## `sbrs grid-hub`

- **Usage:** `sbrs grid-hub [FLAGS] <ACTION> [-- EXTRA_ARGS]…`

Start, stop, restart or check a Selenium Grid hub.

Needs a Selenium Server jar you supply; nothing is downloaded. The process record and log are kept in a folder of your own (SB_GRID_DIR).

### Arguments
- **`<ACTION>`** — What to do with the Grid process.

  **Choices:** `start`, `stop`, `restart`, `status`
- **`[-- EXTRA_ARGS]…`** — More arguments for the Selenium Server, after `--`.

### Flags
- **`--jar <PATH>`** — The Selenium Server jar (default: the SB_SELENIUM_SERVER_JAR variable). It is never downloaded.
- **`--java <PATH>`** — The Java executable (default: `java` on PATH).
- **`--grid-host <HOST>`** — The address to listen on.
- **`--grid-port <PORT>`** — The port to listen on (default: 4444 for a hub, 5555 for a node).
- **`--hub <HOST_OR_URL>`** — For a node, the hub to register with: a host or a URL.
- **`--session-timeout <SECONDS>`** — Close sessions that have been idle this many seconds.
- **`--wait <SECONDS>`** — How many seconds `start` waits for the Grid to become ready.

  **Default:** `60`
- **`--grid-verbose`** — Log at INFO instead of WARNING.
- **`--skip-port-check`** — Start even if something already accepts connections on the port.
- **`-h --help`** — Print help

## `sbrs grid-node`

- **Usage:** `sbrs grid-node [FLAGS] <ACTION> [-- EXTRA_ARGS]…`

Start, stop, restart or check a Selenium Grid node.

A node registers with the hub named by `--hub`, or with this machine's.

### Arguments
- **`<ACTION>`** — What to do with the Grid process.

  **Choices:** `start`, `stop`, `restart`, `status`
- **`[-- EXTRA_ARGS]…`** — More arguments for the Selenium Server, after `--`.

### Flags
- **`--jar <PATH>`** — The Selenium Server jar (default: the SB_SELENIUM_SERVER_JAR variable). It is never downloaded.
- **`--java <PATH>`** — The Java executable (default: `java` on PATH).
- **`--grid-host <HOST>`** — The address to listen on.
- **`--grid-port <PORT>`** — The port to listen on (default: 4444 for a hub, 5555 for a node).
- **`--hub <HOST_OR_URL>`** — For a node, the hub to register with: a host or a URL.
- **`--session-timeout <SECONDS>`** — Close sessions that have been idle this many seconds.
- **`--wait <SECONDS>`** — How many seconds `start` waits for the Grid to become ready.

  **Default:** `60`
- **`--grid-verbose`** — Log at INFO instead of WARNING.
- **`--skip-port-check`** — Start even if something already accepts connections on the port.
- **`-h --help`** — Print help

## `sbrs translate`

- **Usage:** `sbrs translate [FLAGS] <FILE> <LANGUAGE>`

Write the actions of a JSON scenario in another language.

Rewrites each step's `action` (`click`, `open`, ...) with the translation dictionary and changes nothing else in the file; the steps keep running, because a scenario can be read in any of the languages. Python SeleniumBase translates Python test files; a Rust test is compiled code, so this translates the portable JSON scenario instead.

### Arguments
- **`<FILE>`** — The scenario file.
- **`<LANGUAGE>`** — The language: a code (en, zh, nl, fr, it, ja, ko, pt, ru, es) or its English name.

### Flags
- **`-p --print`** — Print the translation and write nothing (the default).
- **`-o --overwrite`** — Replace the file with the translation.
- **`-c --copy`** — Write the translation to a new file next to it, such as `login_es.json`.
- **`-h --help`** — Print help

## `sbrs mkdir`

- **Usage:** `sbrs mkdir [-b --basic] [--force] <DIR>`

Create a folder with a runnable browser-test suite.

Names are relative to the current directory and use letters, digits, `_`, `-` and `.`, with `/` between folders. Nothing is overwritten unless `--force` is given.

### Arguments
- **`<DIR>`** — Folder to create, for example `tests/ui`.

### Flags
- **`-b --basic`** — Create only the scaffolding, with no example tests.
- **`--force`** — Replace generated files that already exist.
- **`-h --help`** — Print help

## `sbrs mkfile`

- **Usage:** `sbrs mkfile [FLAGS] <FILE>`
- **Aliases:** `new`

Create a browser-test file.

The name follows the same rules as `mkdir`; `.rs` is added when missing. An existing file is never overwritten unless `--force` is given.

### Arguments
- **`<FILE>`** — File to create, for example `tests/login.rs`.

### Flags
- **`--url <URL>`** — Page the test opens (default: https://example.com).
- **`-b --basic`** — Leave the test body empty: it only opens the page.
- **`--force`** — Replace the file if it already exists.
- **`-h --help`** — Print help

## `sbrs commander`

- **Usage:** `sbrs commander`

Launch the interactive commander (needs a build with the `tui` feature).

### Flags
- **`-h --help`** — Print help

## `sbrs caseplans`

- **Usage:** `sbrs caseplans [--starter] [PATH]`
- **Aliases:** `case-plans`

Launch the Case Plans Generator, a terminal app (needs a build with the `tui` feature).

Lists the tests under PATH and creates, edits and saves a case plan for each, `case_plans/<test>.md` next to its file, and writes a summary of the plans to `case_summary.md`. PATH is a folder of tests or one test file; by default the `tests/` and `examples/` folders of the current folder.

The Python command opens a Tkinter window; this one runs in the terminal.

### Arguments
- **`[PATH]`** — A folder of tests, or one test file.

### Flags
- **`--starter`** — Write the starter checklist `CASE_PLANS.md` instead; this needs no `tui` feature.
- **`-h --help`** — Print help

## `sbrs behave-gui`

- **Usage:** `sbrs behave-gui [--new] [PATH]`
- **Aliases:** `gui-behave`

Launch the Behave Commander, a terminal app (needs a build with the `tui` feature).

Lists the `.feature` files under PATH with their scenarios, runs the ones you mark through the crate's behave runner and shows the results. PATH is a folder of feature files or one file; by default `features`.

The runner has only the built-in steps: steps written in Rust are not loaded, and a scenario that uses one is reported as undefined. The Python command opens a Tkinter window; this one runs in the terminal.

### Arguments
- **`[PATH]`** — A folder of feature files, or one `.feature` file.

### Flags
- **`--new`** — Prompt for a name, a title and scenarios and write a new feature file instead; this needs no `tui` feature.
- **`-h --help`** — Print help

## `sbrs print`

- **Usage:** `sbrs print <--file <FILE>>`

Print a file.

### Flags
- **`--file <FILE>`** — File path to print
- **`-h --help`** — Print help

## `sbrs objectify`

- **Usage:** `sbrs objectify`

Objectify a test recording.

### Flags
- **`-h --help`** — Print help

## `sbrs mkpres`

- **Usage:** `sbrs mkpres <--file <FILE>>`

Create an HTML presentation.

### Flags
- **`--file <FILE>`** — Output HTML presentation path
- **`-h --help`** — Print help

## `sbrs mkchart`

- **Usage:** `sbrs mkchart <--file <FILE>>`

Create an HTML chart.

### Flags
- **`--file <FILE>`** — Output HTML chart path
- **`-h --help`** — Print help

## `sbrs mkrec`

- **Usage:** `sbrs mkrec <--file <FILE>>`

Create a test recording scaffold.

### Flags
- **`--file <FILE>`** — Output recording scaffold path
- **`-h --help`** — Print help

## `sbrs run-scenario`

- **Usage:** `sbrs run-scenario <--file <FILE>> [--dashboard <DASHBOARD>]`

Run a scenario file and write an optional dashboard.

### Flags
- **`--file <FILE>`** — Scenario file to run
- **`--dashboard <DASHBOARD>`** — Optional path to write a scenario dashboard
- **`-h --help`** — Print help

## `sbrs completions`

- **Usage:** `sbrs completions <SHELL>`

Generate a completion script for a supported shell, or a Fig spec.

### Arguments
- **`<SHELL>`** — Shell (or `fig` for a Fig spec) to generate completions for

  **Choices:** `bash`, `zsh`, `fish`, `elvish`, `powershell`, `fig`, `usage`

### Flags
- **`-h --help`** — Print help

## `sbrs import-python`

- **Usage:** `sbrs import-python [FLAGS] <FILE>`

Convert common SeleniumBase or Selenium Python code into a Rust test.

### Arguments
- **`<FILE>`** — Python source file to convert.

### Flags
- **`-o --output <OUTPUT>`** — Output file for the generated Rust test
- **`--source <SOURCE>`** — Source API to assume when converting Python code

  **Choices:** `auto`, `selenium-base`, `selenium`

  **Default:** `auto`
- **`--test-name <TEST_NAME>`** — Name of the generated test function
- **`-h --help`** — Print help

## `sbrs import-playwright`

- **Usage:** `sbrs import-playwright [FLAGS] <FILE>`
- **Aliases:** `import-pw`

Convert Playwright tests or a script (JavaScript, TypeScript or Python) into Rust tests for the pure CDP engine.

### Arguments
- **`<FILE>`** — Playwright source file to convert.

### Flags
- **`-o --output <OUTPUT>`** — Output file for the generated Rust tests
- **`--language <LANGUAGE>`** — Source language (default: from the file extension, else from the content)

  **Choices:** `js`, `ts`, `python`
- **`--test-name <TEST_NAME>`** — Name of the test generated for a script without test titles (default: the file name)
- **`-h --help`** — Print help

## `sbrs import-puppeteer`

- **Usage:** `sbrs import-puppeteer [FLAGS] <FILE>`
- **Aliases:** `import-pptr`

Convert Puppeteer scripts or tests (JavaScript, TypeScript) or a Chrome Recorder JSON export into Rust tests for the pure CDP engine.

### Arguments
- **`<FILE>`** — Puppeteer source file (or Recorder `.json` export) to convert.

### Flags
- **`-o --output <OUTPUT>`** — Output file for the generated Rust tests
- **`--language <LANGUAGE>`** — Source language (default: from the file extension, else from the content)

  **Choices:** `js`, `ts`, `json`
- **`--test-name <TEST_NAME>`** — Name of the test generated for a script without test titles (default: the file name)
- **`-h --help`** — Print help

## `sbrs download-file`

- **Usage:** `sbrs download-file [-o --output <OUTPUT>] [--timeout <TIMEOUT>] <URL>`

Download a file from a URL (browser download with an HTTP fallback).

### Arguments
- **`<URL>`** — URL of the file to download

### Flags
- **`-o --output <OUTPUT>`** — Output file path (defaults to the URL file name in the current directory)
- **`--timeout <TIMEOUT>`** — Seconds to wait for the browser download before falling back to HTTP

  **Default:** `30`
- **`-h --help`** — Print help

## `sbrs print-pdf`

- **Usage:** `sbrs print-pdf [-o --output <OUTPUT>] <URL>`

Print a page to PDF and save it to disk.

### Arguments
- **`<URL>`** — URL to render as a PDF

### Flags
- **`-o --output <OUTPUT>`** — Output PDF file path

  **Default:** `page.pdf`
- **`-h --help`** — Print help

## `sbrs pdf-text`

- **Usage:** `sbrs pdf-text [--save-pdf <SAVE_PDF>] <URL>`

Print the text extracted from a page or PDF URL.

### Arguments
- **`<URL>`** — URL of the page or PDF to extract text from

### Flags
- **`--save-pdf <SAVE_PDF>`** — Also keep the intermediate PDF at this path
- **`-h --help`** — Print help

## `sbrs assert-pdf-text`

- **Usage:** `sbrs assert-pdf-text <--text <TEXT>> [--save-pdf <SAVE_PDF>] <URL>`

Assert that a page or PDF URL contains the expected text.

### Arguments
- **`<URL>`** — URL of the page or PDF to check

### Flags
- **`--text <TEXT>`** — Text that must appear in the PDF
- **`--save-pdf <SAVE_PDF>`** — Also keep the intermediate PDF at this path
- **`-h --help`** — Print help

## `sbrs choose-file`

- **Usage:** `sbrs choose-file [--url <URL>] <SELECTOR> <FILE>`

Upload a local file into a file input.

### Arguments
- **`<SELECTOR>`** — CSS selector of the &lt;input type=file> element
- **`<FILE>`** — Local file to upload

### Flags
- **`--url <URL>`** — Page to open first (defaults to a generated blank upload page)
- **`-h --help`** — Print help

## `sbrs deferred`

- **Usage:** `sbrs deferred [FLAGS]`

Run several assertions, then report every failure at once.

### Flags
- **`--url <URL>`** — URL to open before running the assertions
- **`--assert <SPEC>`** — Assertion spec: element=&lt;css>, text=&lt;css>:&lt;text>, title=&lt;text>, or url=&lt;text>
- **`--spec <SPEC>`** — JSON file containing an array of assertion specs
- **`-h --help`** — Print help

## `sbrs record`

- **Usage:** `sbrs record [-o --output <OUTPUT>] [--duration <DURATION>] <URL>`

Record browser interactions into a replayable action file.

### Arguments
- **`<URL>`** — URL to open and record

### Flags
- **`-o --output <OUTPUT>`** — Output action file (.json or .yaml)

  **Default:** `actions.json`
- **`--duration <DURATION>`** — Stop recording after this many seconds (default: run until Ctrl-C)
- **`-h --help`** — Print help

## `sbrs replay`

- **Usage:** `sbrs replay <FILE>`

Replay a recorded action file against a fresh browser session.

### Arguments
- **`<FILE>`** — JSON action file produced by `sbrs record`

### Flags
- **`-h --help`** — Print help

## `sbrs devtools`

- **Usage:** `sbrs devtools [FLAGS]`

Launch Chrome with an open DevTools endpoint and print MCP client configs.

Starts Chrome with the Pure CDP engine (no chromedriver), prints the DevTools HTTP endpoint, the browser WebSocket URL and configs for Google's chrome-devtools-mcp and Microsoft's Playwright MCP, then keeps the browser running until Ctrl+C, which closes it and exits with 0. The global --browser (chrome, chromium, edge or another Chromium browser), --headed, --user-agent, --locale, --ad-block, --extension-dir and --proxy-list apply as well.

### Flags
- **`--headless`** — Run the browser without a window
- **`--port <PORT>`** — DevTools port on 127.0.0.1 (0 lets Chrome pick a free one)

  **Default:** `9222`
- **`--user-data-dir <DIR>`** — Profile directory to keep (default: a throwaway one; never Chrome's own default profile)
- **`--proxy <PROXY>`** — Proxy: SERVER:PORT, USER:PASS@SERVER:PORT or scheme://SERVER:PORT
- **`--browser-path <PATH>`** — Browser binary to launch, such as Microsoft Edge or another Chromium browser
- **`--url <URL>`** — Page to open once the browser runs
- **`--json`** — Print the endpoint and configs as one line of JSON instead of text
- **`-h --help`** — Print help

## `sbrs plugin`

- **Usage:** `sbrs plugin <SUBCOMMAND>`

List the built-in plugins, or show what one declares.

Plugins run inside the browsers a command starts. Turn them on by name with the global options --plugin and --plugin-opt, the SB_PLUGINS variable, or the [plugins] table of the settings file; the command line wins over SB_PLUGINS, which wins over the file. `sbrs plugin list` shows the plugins this build has.

### Flags
- **`-h --help`** — Print help

## `sbrs plugin list`

- **Usage:** `sbrs plugin list`

List the built-in plugins of this build, and whether they are on.

Prints each plugin's name, category and summary, and `yes` under ENABLED when the settings file, SB_PLUGINS or the global --plugin option turns it on. A build without built-in plugins says so; rebuild it with --features plugins-builtin.

### Flags
- **`-h --help`** — Print help

## `sbrs plugin info`

- **Usage:** `sbrs plugin info <NAME>`

Show what one plugin declares and what it may do.

Prints the plugin's category, phase, capabilities (with their risk, and whether the host enforces them), what it needs, its place among the other plugins and the options it was given. Exits with status 2 and lists the known names when there is no such plugin.

### Arguments
- **`<NAME>`** — The plugin's name, as `sbrs plugin list` shows it.

### Flags
- **`-h --help`** — Print help