# Customizing Test Runs

SeleniumBase for Rust can be configured through the `BrowserConfig` builder, a
global config file, environment variables, or CLI flags. This guide explains the
full hierarchy and shows how to make each source of configuration work for your
workflow.

## What you will learn

- How to configure a run with the `BrowserConfig` builder.
- How to create a global TOML config file.
- How to override settings with `SB_*` environment variables.
- How CLI flags interact with programmatic and file configuration, and how
  `sbase test` hands them to your tests.

## BrowserConfig builder

The most direct way to configure a run is the `BrowserConfig` builder. Every
method returns `Self`, so calls can be chained fluently. `BrowserConfig::default()`
is headless, reads no environment variables, and starts a chromedriver for you
when none is running at the default WebDriver URL.

```rust,no_run
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(true)
        .with_mobile(true)
        .with_locale("en-US")
        .with_proxy("http://proxy.example.com:8080");

    let mut sb = BaseCase::new(config).await?;
    sb.set_timeout(15).await?;
    sb.open("https://seleniumbase.io").await?;
    sb.quit().await?;
    Ok(())
}
```

Key builder methods:

| Method | Effect |
|---|---|
| `with_mode(mode)` | Set `DriverMode::WebDriver`, `Cdp`, or `Uc`. |
| `with_browser(browser)` | Set `Browser::Chrome`, `Chromium`, `Edge`, or `Firefox`. |
| `with_headless(bool)` | Run without a visible window. |
| `with_webdriver_url(url)` | Connect to this WebDriver server instead of auto-starting a local chromedriver. |
| `with_user_agent(s)` | Override the user agent string. |
| `with_mobile(bool)` | Emulate a mobile device: a 390x844 window and an Android Chrome user agent, unless you set your own user agent. |
| `with_locale(s)` | Set `navigator.languages` / `Accept-Language`. |
| `with_proxy(url)` | Route traffic through an HTTP/HTTPS/SOCKS proxy. |
| `with_proxy_pac_url(url)` | Load proxy configuration from a PAC file. |
| `with_user_data_dir(path)` | Launch with an existing Chromium profile. |
| `with_extension_dir(path)` | Load an unpacked extension. |
| `with_start_page(url)` | The page `BaseCase::open_start_page` / `goto_start_page` returns to (default `about:blank`). |
| `with_reuse_session(bool)` | Sets `reuse_session`. Nothing in the crate reads it yet, so it does not attach to an existing session. |
| `with_threads(n)` | Sets `threads`. `BaseCase` ignores it; only `sbase test` acts on a thread count (`-n`, as `--test-threads`). |
| `with_ad_block(bool)` | Sets `ad_block`: a Chromium WebDriver session refuses the requests of advertising and tracking hosts (see below). |
| `with_ad_block_list(list)` | Block the hosts of this `AdBlockList` instead of the built-in list, and turn `ad_block` on. |
| `with_fingerprint(fp)` | Apply a [`Fingerprint`](../tutorials/fingerprint_stealth.md). |
| `with_browser_binary_path(path)` | Use this Chrome/Chromium/Edge executable. |
| `with_extra_args(vec)` / `push_extra_arg(s)` | Pass extra Chromium launch flags. |

`remote_webdriver_timeout_seconds` has no builder method. Set it on the struct,
which has public fields.

### Blocking ads

`ad_block` makes a Chromium WebDriver session (Chrome, Chromium, Edge, Brave,
Comet, Opera, Chrome for Testing, `chrome-headless-shell`) refuse every request
to a listed advertising or tracking host and its subdomains. Once the session is
connected it sends the hosts to the browser with `Network.setBlockedURLs`
through chromedriver's DevTools channel. The list is the built-in
`AdBlockList::builtin()`, or your own:

```rust
use seleniumbase_rs::config::ad_block_list::AdBlockList;
use seleniumbase_rs::BrowserConfig;

let list = AdBlockList::from_hosts(["ads.example.com", "tracker.example.net"])?;
let config = BrowserConfig::default().with_ad_block_list(list);
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

A list of your own replaces the built-in one. A list that needs more than 2,000
URL patterns is refused when the session starts. Limits:

- The block belongs to a window. The session repeats it for the window it opens
  with and for each window it switches to (`switch_to_window`,
  `switch_to_new_window`); a window reached any other way needs
  `BrowserSession::apply_ad_block()`.
- A URL that names a port (`https://ads.example.com:8443/`) is not matched.
- A frame of another site is not stopped from loading, and what it requests is
  not blocked (checked in Chrome 155). A frame of the page's own site is blocked
  like the page. Ad frames are usually of another site, so this blocks the
  scripts, pixels and images a page asks for, and not the frames that carry the
  ads.
- Firefox, Safari and Internet Explorer have no DevTools channel through
  WebDriver, so the setting blocks nothing there and a warning says so.
- Python hides the ad elements of a page once it has loaded; `BaseCase::ad_block`
  does that, when called. The pure CDP engine blocks hosts with
  `LaunchOptionsBuilder::ad_block`.

### Launch settings

The Python SeleniumBase options that shape how a browser or its driver starts
live in `BrowserConfig::launch` (a `LaunchSettings`) and have builder methods of
their own. Each value type reads the text the Python command line takes, so
`"1280,720".parse::<WindowSize>()` and `"eager".parse::<PageLoadStrategy>()`
work.

| Method | Python option | Effect |
|---|---|---|
| `with_block_images(bool)`, `with_disable_js(bool)`, `with_disable_cookies(bool)`, `with_do_not_track(bool)`, `with_external_pdf(bool)` | `--block-images`, `--disable-js`, `--disable-cookies`, `--do-not-track`, `--external-pdf` | Browser preferences (Chrome `prefs`, Firefox preferences). |
| `with_enable_sync(bool)` | `--enable-sync` | Leave Chrome Sync on; sessions switch it off otherwise. |
| `with_disable_csp(bool)` | `--disable-csp`, `--dcsp`, `--no-csp` | Remove the Content-Security-Policy headers (bundled extension, Chromium only). |
| `with_page_load_strategy(s)` | `--page-load-strategy`, `--pls` | `normal`, `eager` or `none`. |
| `with_window_size(s)`, `with_window_position(p)` | `--window-size`, `--window-position` | Size and place the window. |
| `with_device_metrics(m)` | `--metrics`, `d_width`, `d_height`, `d_p_r` | Emulate a phone screen (turns `mobile` on). |
| `with_proxy_bypass_list(l)` | `--proxy-bypass-list` | Hosts that skip the proxy. |
| `with_proxy_for_driver_download(bool)` | `--proxy-driver` | Download drivers and Chrome for Testing through the proxy. |
| `with_extension_archive(a)` | `--extension-zip`, `--crx` | Load a `.zip` or `.crx` extension (Chromium). |
| `with_automation_extension(bool)`, `with_recorder_extension(bool)` | `--use-auto-ext`, `recorder_ext` | Keep Chrome's automation extension; load the Recorder. |
| `with_firefox_options(o)` | `--firefox-arg`, `--firefox-pref` | Firefox arguments and preferences. |
| `with_capabilities(c)` | `--cap-string`, `--cap-file` | Merge extra capabilities into the session's. |
| `with_driver_version(v)` | `--driver-version` | Run this chromedriver (downloaded when missing). |
| `with_remote_debug(bool)`, `with_remote_debug_port(p)` | `--remote-debug` | Open Chrome's debugger (9222 or a port you name). |
| `with_cdp_events(c)` | `--log-cdp-events`, `--uc-cdp-events` | Keep the DevTools events; read them with `BrowserSession::cdp_events`. |
| `with_virtual_display(d)` | `--xvfb`, `--xvfb-metrics` | Run a headed browser on Xvfb (Linux). |

`with_browser` also takes `Brave`, `Comet`, `Opera`, `Safari`,
`InternetExplorer`, `ChromeForTesting` and `ChromeHeadlessShell` (the Python
`--brave`, `--comet`, `--opera`, `--safari`, `--ie`, `--cft` and `--chs`). Chrome
for Testing and `chrome-headless-shell` are downloaded into `downloaded_drivers/`
when missing; Opera, Safari and Internet Explorer need their own driver
(`operadriver`, `safaridriver`, `IEDriverServer`) on the `PATH`.

## Global config file

Create `sbase_config.toml` in the directory you run from, for shared defaults:

```toml
# sbase_config.toml
browser = "chrome"
headless = false
mobile = false
locale = "en-US"
timeout_seconds = 10
screenshot_dir = "screenshots"
proxy = "http://proxy.example.com:8080"
```

Every key is optional and any key you leave out keeps its default. The keys are
the fields of `Settings`: `browser`, `headless`, `timeout_seconds`,
`screenshot_dir`, `proxy`, `proxy_pac_url`, `window_width`, `window_height`,
`user_data_dir`, `extension_dir`, `locale`, `user_agent`, `mode` (`webdriver`,
`cdp` or `uc`), `ad_block`, `reuse_session`, `mobile`, `threads` and
`remote_webdriver_timeout_seconds`. Note that `timeout_seconds` is a whole
number of seconds.

`Settings::load_global()` looks for these names, in this order, in the
**current working directory only** (it does not search parent directories):
`sbase_config.toml`, `.sbase_config.toml`, `.sbase_config`. A file whose name
ends in `.toml` is read as TOML; any other name, including `.sbase_config`, is
read as JSON:

```json
{ "browser": "chrome", "headless": false, "timeout_seconds": 10 }
```

A flat `key=value` file is not supported. `sbase --config FILE` (or
`-c FILE`) reads one specific file by the same rule.

`timeout_seconds`, `screenshot_dir`, `window_width` and `window_height` are
stored in `Settings` for your code to read, but nothing in the crate applies
them to a browser on its own. For the timeout, pass the value on yourself:
`sb.set_timeout(settings.timeout_seconds).await?`.

To build a `BrowserConfig` from the file in your own test:

```rust,no_run
use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::BaseCase;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
// The global config file (if any), with the SB_* variables applied on top.
let settings = Settings::load(None::<&str>)?;
let config = settings.to_browser_config();

let mut sb = BaseCase::new(config).await?;
sb.set_timeout(settings.timeout_seconds).await?;
# sb.quit().await?;
# Ok(())
# }
```

`to_browser_config()` leaves the WebDriver URL at its default and does not set
a browser binary, a fingerprint or extra arguments; add those with the builder
methods afterwards.

## Environment variables

Environment variables override file values. The prefix is `SB_` (not `SBASE_`):

```bash
export SB_HEADLESS=true
export SB_LOCALE=fr-FR
export SB_BROWSER=chrome
export SB_TIMEOUT=20
```

`Settings::load` and `Settings::from_env` read exactly these, each mapped to the
field of the same name: `SB_BROWSER`, `SB_HEADLESS`, `SB_TIMEOUT`,
`SB_REMOTE_WEBDRIVER_TIMEOUT`, `SB_SCREENSHOT_DIR`, `SB_PROXY`,
`SB_PROXY_PAC_URL`, `SB_WINDOW_WIDTH`, `SB_WINDOW_HEIGHT`, `SB_USER_DATA_DIR`,
`SB_EXTENSION_DIR`, `SB_LOCALE`, `SB_USER_AGENT`, `SB_MODE`, `SB_AD_BLOCK`,
`SB_REUSE_SESSION`, `SB_MOBILE` and `SB_THREADS`. Booleans accept `true`/`false`,
`1`/`0`, `yes`/`no` and `on`/`off`. There is no automatic name mapping: a
variable that is not in this list, such as `SB_LOGGING__LOG_LEVEL`, does
nothing. See [Settings and Global Configuration](../tutorials/settings_and_config.md)
for the table, including the separate runtime variables (`SB_WEBDRIVER_URL`,
`SB_CHROME_BIN`, `SB_LOG_LEVEL`, ...), which `BrowserConfig::from_env()` and
`RuntimeConfig::from_env()` read:

```bash
export SB_WEBDRIVER_URL=http://localhost:4444/wd/hub
```

## CLI flags

When using the `sbase` binary, put its options **before** the subcommand. They
belong to `sbase` itself, not to the subcommand, so `sbase open URL --headless`
is rejected:

```bash
cargo run --bin sbase -- --headless --proxy http://proxy.example.com:8080 open https://seleniumbase.io
```

Common flags:

| Flag | Description |
|------|-------------|
| `--headless` / `--headed` | Run the browser without or with a visible window. |
| `--mobile` | Emulate a mobile device (see `with_mobile`). |
| `--proxy HOST:PORT` | Route traffic through a proxy (`scheme://host:port` also works). |
| `--proxy-list FILE` | Choose from a file of proxies: a random one, or the entry named by `--proxy`. |
| `--proxy-pac-url URL` | Use a PAC file for proxy configuration. |
| `--user-data-dir DIR` | Load a Chromium user data directory. |
| `--extension-dir DIR` | Load a Chromium extension. |
| `--user-agent STRING` / `-a STRING` | Override the user agent. |
| `--locale LOCALE` | Set the browser locale. |
| `--ad-block` | Sets `ad_block`: a Chromium WebDriver session refuses the built-in list of ad and tracker hosts (`Network.setBlockedURLs`). Firefox and Safari block nothing. Images stay on; `--block-images` turns them off. |
| `--reuse-session` / `--rs` | Sets `reuse_session`, which nothing reads yet (the `sbase --help` text for `--rs`, "Record the session actions", is wrong: it is the same flag). |
| `-n NUM` / `--threads NUM` | With `sbase test`, run that many tests at once (`--test-threads`), so that many browsers. |
| `--browser NAME` | Select `chrome`, `chromium`, `edge`, or `firefox`. |
| `--cdp` / `--uc` | Select CDP or UC mode (they cannot be combined). |
| `--webdriver URL` | WebDriver server URL (default `http://localhost:4444`). |
| `-c FILE` / `--config FILE` | Read settings from this TOML or JSON file instead of the global config file. |

Run `sbase --help` and `sbase <subcommand> --help` for the full list of flags.

### Running tests with `sbase test`

`sbase test` runs `cargo test` (or one example) and carries the global options
into the tests as `SB_*` environment variables, so a test that loads `Settings`
sees them:

```bash
# Sets SB_HEADLESS=true and SB_BROWSER=firefox, and runs 2 tests at a time.
cargo run --bin sbase -- --headless --browser firefox -n 2 test
cargo run --bin sbase -- test login --test checkout --release --features turso
```

It accepts a name filter, `--test NAME` (one file from `tests/`),
`--example NAME`, `--release`, `--features a,b`, and harness arguments after
`--`. Only the options you actually pass become variables, so everything else
still comes from your config file and environment. A non-default
`--webdriver` URL reaches the tests as `SB_WEBDRIVER_URL`.

## Combining config sources

For a test that builds its configuration with `Settings::load` and then adjusts
it, later sources win:

1. Built-in defaults (`Settings::default()`).
2. Global config file (`sbase_config.toml`, `.sbase_config.toml` or `.sbase_config`).
3. Environment variables (`SB_*`).
4. Programmatic changes, such as `BrowserConfig` builder calls.

`sbase test` adds the command-line flags as step 3 for its tests. The `sbase`
binary itself works differently when it drives a browser directly (`sbase open`
and the like): it starts from the config file (the global one, or the one named
by `--config`; only with `--config` are the `SB_*` variables applied on top),
and then CLI flags override it.

Use the config file for stable defaults, environment variables for CI-specific
overrides, and the builder or CLI flags for one-off experiments.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Environment variable ignored | Used `SBASE_` instead of `SB_`, or the variable is not one `Settings` reads | Use the `SB_` prefix and a name from the list above. |
| `sbase open` ignores `SB_HEADLESS` | `sbase` applies `SB_*` settings variables only with `--config` | Pass `--headless` or `--config FILE`. |
| Config file not loaded | File is in a parent directory, or has another name | Put it in the directory you run from, named `sbase_config.toml`. |
| Config file fails to parse | A flat `key=value` file, or a float for `timeout_seconds` | Use TOML (`.toml` name) or JSON, and whole numbers of seconds. |
| CLI flag rejected as unexpected | The flag was placed after the subcommand | Put it before: `sbase --headless open URL`. |
| Proxy not used | Format is invalid | Pass a full URL such as `http://host:8080`. |
