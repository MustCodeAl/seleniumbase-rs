---
title: "Browser Plugins"
sidebarTitle: "Use plugins"
---

<Badge color="blue">Feature: plugins</Badge>

A browser plugin is a Rust crate that changes how a browser starts and what
every tab gets: block images, add a script to every page, set a preference,
load an extension, route through a proxy. You add one to a `PluginSet`, give the
set to the launch options, and its hooks run at fixed points. It is the idea of
[puppeteer-extra](https://github.com/berstend/puppeteer-extra) in Rust: plugins
are ordinary crates, compiled into your program, and anyone can publish one.

The design is [ADR 004](/adr/004-plugin-framework). The hooks run in both
engines: the Pure CDP engine (`sb_cdp`) and the WebDriver engine (`BaseCase`).

<Note>

Browser plugins are not [Test Plugins](/tutorials/test_plugins), which run around a
test (screenshots on failure, reports), and not the
[browser extension](https://github.com/MustCodeAl/seleniumbase-rs-browser-extension),
which runs inside the browser. A browser plugin is Rust code in the program that
drives the browser.

</Note>

<Info>

**What you will learn**

- Turn plugins on and use one, with either engine
- Install a plugin from the plugins repository
- Write a plugin, and test it without a browser
- Steer navigation, the network and browser contexts, hear tests end, and run a plugin's task by name
- When each hook runs, and what happens when one fails
- What each browser lets a plugin do on the WebDriver engine
- What you trust when you add a plugin

</Info>

<Visibility for="agents">

Summary for AI exports: a browser plugin is a Rust crate that implements `Plugin` and runs hooks (`before_launch`, `on_browser`, `on_page_created`, `on_close`) around a browser launch. Turn plugins on with the `plugins` feature, build a `PluginSet`, and give it to `LaunchOptionsBuilder::plugins` (pure CDP engine) or `BrowserConfig::with_plugins` (`BaseCase`). Plugins fail closed: a refusal or a hook error stops the launch or the tab with `SeleniumBaseError::Plugin`. A plugin is Rust code that runs with your program's rights. Since plugin API 0.1.3 a plugin can also steer navigation (`before_navigate`, `after_navigate`), block or watch requests (`network`, `on_request`, `on_response`), change browser contexts (`before_context`), hear tests start and end (`on_test_start`, `on_test_end`) and offer a task run by name (`task`, `sbrs plugin run NAME`, the MCP tool `run_plugin_task`); those hooks need a capability declared in `metadata()`. A panic in a hook becomes `PluginError::Panicked`, and a set with a timer cuts a hook off at its time limit with `PluginError::Timeout`.

</Visibility>

## Turning plugins on

Plugins are behind the `plugins` feature, off by default. The feature is in
release 0.3.0 and later (the WebDriver engine from 0.4.0):

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", tag = "v0.6.0", features = ["plugins"] }
```

The official plugins are features of their own (`plugin-shield`,
`plugin-block-resources`, `plugin-crx`, `plugin-fetcher`, `plugin-extractor`,
`plugin-batcher`, `plugin-crawler`, and `plugins-builtin` for all of them),
turned on by name in the settings file, in `SB_PLUGINS`, in a builder call, or
on the command line (`sbrs --plugin shield open https://example.com`, with the
plugins compiled into the prebuilt `sbrs` and `sbrs-mcp`): see
[Choosing plugins](/plugins/selecting).

The plugin API is its own crate, `seleniumbase-rs-plugin`, in the
[seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)
repository. This crate re-exports it as `seleniumbase_rs::plugin_api`, so a
program needs no second dependency to use or write plugins.

## Using a plugin

Build a set, then give it to the engine you use:

<Tabs>

<Tab title="Pure CDP engine" icon="bolt">

Build a set, give it to `LaunchOptionsBuilder::plugins`, and launch as usual:

```rust,no_run src/main.rs
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_plugin_block_resources::BlockResources;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let plugins = PluginSet::new().with(BlockResources::images_and_fonts())?;
let options = LaunchOptions::builder()
    .headless(true)
    .plugins(plugins)
    .build()?;
let browser = Browser::launch(options).await?;
let page = browser.default_page().await?;
page.goto("https://example.com").await?; // no images, no web fonts
browser.close().await?;
# Ok(())
# }
```

To attach to a browser that is already running, use
`Browser::connect_with_plugins(host, port, plugins)` instead of `connect`. There
is nothing to launch, so `before_launch` does not run; the rest does.

</Tab>

<Tab title="BaseCase (WebDriver)" icon="flask">

With `BaseCase`, the same set goes to `BrowserConfig::with_plugins`:

```rust,no_run src/main.rs
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs::{BaseCase, BrowserConfig};
use seleniumbase_rs_plugin_block_resources::BlockResources;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let plugins = PluginSet::new().with(BlockResources::images_and_fonts())?;
let mut sb = BaseCase::new(BrowserConfig::default().with_plugins(plugins)).await?;
sb.open("https://example.com").await?; // no images, no web fonts
sb.quit().await?;
# Ok(())
# }
```

`BrowserSession::connect` takes the same configuration. Chrome and the other
Chromium browsers give plugins everything the Pure CDP engine does; Firefox and
Safari give less, as [The WebDriver engine](#the-webdriver-engine) explains.

</Tab>

</Tabs>

`PluginSet::with` refuses a second plugin with the same name. Plugins run in
the order they were added, except those that ask to run last.

## Installing a plugin

Plugins are crates named `seleniumbase-rs-plugin-<name>`. The official ones live
in the plugins repository:

| Crate | What it does |
|---|---|
| `seleniumbase-rs-plugin-block-resources` | Stops pages from loading images, fonts or media (`Network.setBlockedURLs`) |
| `seleniumbase-rs-plugin-crx` | Loads packed Chrome extensions (`.crx` files): unpacks them and adds the folders to the start-up settings, which the launch loads |
| `seleniumbase-rs-plugin-wasm` | Runs plugins written in any language that compiles to WebAssembly, in a sandbox (no host imports, fuel and memory limits) |

Add one with `cargo add`, from the same revision of the plugins repository
that this crate pins (the `rev` of `seleniumbase-rs-plugin` in its
`Cargo.toml`, currently `628991af0e6c5711298e3bd3692dc2974887401a`):

```sh Terminal
cargo add --git https://github.com/MustCodeAl/seleniumbase-rs-plugins \
    --rev 628991af0e6c5711298e3bd3692dc2974887401a \
    seleniumbase-rs-plugin-block-resources
```

The official plugins also come with this crate, one cargo feature each, and are
turned on by name without code; see [Choosing plugins](/plugins/selecting).

<Warning>

This crate takes `seleniumbase-rs-plugin` from that revision, so the plugin and
this crate must resolve to the same source. Until the crates are on crates.io,
Cargo counts one git repository under two references (a branch, a tag, a
revision) as two different crates: a plugin added with another `--rev`, or with
no reference (the default branch), implements a different `Plugin` trait. The
compiler then says the trait `Plugin` is not implemented for the plugin, and
mentions two versions of `seleniumbase_rs_plugin`. When this crate moves to a new
revision, move the plugin crates with it.

</Warning>

## Writing a plugin

Implement `Plugin`. Every hook is optional; `name` is the only method you must
write.

```rust,no_run src/lib.rs
use seleniumbase_rs::plugin_api::{
    async_trait, BrowserHandle, LaunchSettings, Package, PageHandle, Plugin, PluginError, Result,
};
use serde_json::json;

/// Germany, everywhere: the language, the time zone, and a page script.
struct German;

#[async_trait]
impl Plugin for German {
    fn name(&self) -> &str {
        "german"
    }

    /// A script that every tab gets before `on_page_created` runs.
    fn packages(&self) -> Vec<Package> {
        vec![Package::script("window.planet = 'Erde';").everywhere()]
    }

    /// Before the browser starts: change its arguments, preferences,
    /// extensions, proxy or headless mode.
    async fn before_launch(&self, launch: &mut LaunchSettings) -> Result<()> {
        launch.args.push("--lang=de-DE".to_owned());
        launch
            .prefs
            .insert("intl.accept_languages".to_owned(), json!("de-DE,de"));
        Ok(())
    }

    /// The browser runs: send browser-level protocol commands. An error
    /// stops the launch.
    async fn on_browser(&self, browser: &dyn BrowserHandle) -> Result<()> {
        let version = browser.send("Browser.getVersion", json!({})).await?;
        let product = version["product"].as_str().unwrap_or_default();
        if product.starts_with("HeadlessChrome/") || product.starts_with("Chrome/") {
            Ok(())
        } else {
            Err(PluginError::other(format!("german needs Chrome, not {product}")))
        }
    }

    /// Every tab, before your program uses it.
    async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
        page.send(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "Europe/Berlin" }),
        )
        .await?;
        page.add_init_script("document.documentElement.lang = 'de';").await
    }
}
```

A `PageHandle` has four methods: `send` (a protocol command to the tab),
`add_init_script` (a script for every document the tab loads from now on, run
in the current one too), `evaluate` (a JavaScript value from the current
document) and `engine`. A `BrowserHandle` has `send`, `engine` and `family`.
Plugins never see `sb_cdp::Browser`, `Page` or `BaseCase`, so a plugin keeps
working when those types grow, and the same plugin works with both engines.

A plugin crate depends on `seleniumbase-rs-plugin` (and `serde_json`), never on
`seleniumbase-rs`. The plugins repository has a `template/` to copy.

### Packages

`packages` returns JavaScript and CSS to load into every tab:
`Package::script(code)`, `Package::style(css)`, `Package::script_url(url)` and
`Package::style_url(url)`. Each must say where it loads, `.only_on(["example.com"])`
or `.everywhere()`; a package that says neither is refused when the browser
starts, so a plugin never loads a script into every site by accident. For a URL,
pin the version and add `.integrity("sha384-...")` so the browser checks the
file.

### Requirements and dependencies

`requirements` returns flags the set checks before anything starts:

| Flag | Refuses |
|---|---|
| `Requirements::HEADFUL` | a headless browser |
| `Requirements::LAUNCH_ONLY` | a browser attached with `connect_with_plugins`, and a WebDriver session on a server other than the local default (`with_webdriver_url`, such as a Selenium Grid) |
| `Requirements::CHROMIUM_ONLY` | browsers that are not Chromium-based: Firefox and Safari on the WebDriver engine |
| `Requirements::RUN_LAST` | nothing; the plugin runs after the others |

`dependencies` names other plugins that must be in the same set. The set is
checked again after every `before_launch` has run, so one plugin cannot switch
on headless mode under another that needs a window.

### Testing a plugin

Without a browser, the `seleniumbase-rs-plugin-testing` crate in the plugins
repository has recording fakes (`RecordingPage`, `RecordingBrowser`) that
remember every command and script a hook sends.

Through this crate, the `test-util` feature runs the real launch steps against a
scripted mock: `Browser::launch_mocked(options, &mock)` checks the set, runs
`before_launch`, loads the extensions, runs `on_browser` and prepares the first
tab, with no Chrome. Script the mock's answers first, then read back what the
browser was sent. With the `German` plugin above:

```rust,no_run
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, MockCtrl};
use serde_json::json;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let mock = MockCtrl::new();
mock.reply("Browser.getVersion", json!({ "product": "Chrome/155.0" }));
let options = LaunchOptions::builder()
    .plugins(PluginSet::new().with(German)?)
    .build()?;
let browser = Browser::launch_mocked(options, &mock).await?;

let timezone = mock.calls_to("Emulation.setTimezoneOverride");
assert_eq!(timezone.len(), 1, "the first tab got it");
assert_eq!(timezone[0].params["timezoneId"], "Europe/Berlin");
browser.close().await?;
# Ok(())
# }
```

## More hooks

Since plugin API 0.1.3 a plugin can watch and steer what the browser does, not
only how it starts. Every new method has a default body, so a plugin written
for an earlier version compiles and behaves as before.

| Hook | Runs when | Needs |
|---|---|---|
| `before_navigate` | the program is about to navigate a tab; return `NavDecision::Continue`, a redirect, or `NavDecision::Cancel(reason)`. An error cancels it too. | `Capability::PageNavigate` to redirect |
| `after_navigate` | a navigation the program started has ended | |
| `network`, then `on_request` | `network()` declares rules the host runs itself (block a URL, add a header) and filters; a request that matches an `intercept` filter is paused and `on_request` decides. An error blocks the request. | `Capability::NetIntercept` |
| `on_response` | a response that matches an `observe` filter arrived; it only watches | `Capability::NetObserve` |
| `protocol_events`, then `on_protocol_event` | a DevTools event the plugin named, such as `Log.entryAdded`, arrived | |
| `before_context`, `on_context_created`, `on_context_closed` | a browser context is about to be made (change its settings, such as its proxy), exists, is gone | `Capability::Contexts` |
| `before_close` | the browser is about to close and still answers commands, so the plugin can save state; `on_close` follows | |
| `on_test_start`, `on_test_end` | a test starts; a test ended, with its tab still open. `on_test_end` may fail the test. | `Capability::TestHooks` |
| `task`, `job_runner` | work the user starts by name; one kind of job a batching plugin hands out | `Capability::Task` for `task` |

A plugin says what it needs in `metadata()`. A plugin that does not hold a
capability is not called for the hooks it guards. The host merges the network
declarations of all plugins, so the browser pauses only the requests some plugin
asked for. The navigation, network, context and protocol-event hooks run on the
Pure CDP engine; the WebDriver engine has no network observation or
interception, so a plugin that needs `NetIntercept` or `NetObserve` is refused
there before anything starts.

```rust,no_run src/lib.rs
use seleniumbase_rs::plugin_api::{
    async_trait, Capability, Category, Metadata, NavDecision, Navigation, NetRule,
    NetworkInterest, PageHandle, Plugin, RequestFilter, Result,
};

/// Keeps tests away from production and blocks a tracker.
struct Fence;

#[async_trait]
impl Plugin for Fence {
    fn name(&self) -> &str {
        "fence"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::NetIntercept)
    }

    // The host blocks these itself, without calling the plugin.
    fn network(&self) -> NetworkInterest {
        NetworkInterest::none().rule(NetRule::block(
            RequestFilter::any().url("https://tracker.example/*"),
        ))
    }

    async fn before_navigate(
        &self,
        _page: &dyn PageHandle,
        navigation: &Navigation,
    ) -> Result<NavDecision> {
        Ok(if navigation.url.starts_with("https://prod.example/") {
            NavDecision::Cancel("tests must not touch production".into())
        } else {
            NavDecision::Continue
        })
    }
}
```

A navigation `Fence` cancels comes back to the program as an error naming the
plugin: `navigation to https://prod.example/pay cancelled by plugin fence: tests
must not touch production`. To test the network part without a browser,
`seleniumbase-rs-plugin-testing` has `drive_request` and `drive_response`, which
run a request or a response through a set the way the host does.

### Tasks

A task is work a plugin does when the user starts it by name, such as crawling
a site. Implement `Task`, return it from `Plugin::task`, and hold
`Capability::Task`. `run` gets a `TaskContext`: the browser, the input as JSON,
a `Sink` that takes results one at a time, the plugin set (to reach another
plugin's `JobRunner` by name), a `Clock`, and a `Prompt` to ask a person for
help, such as signing in. It returns a summary.

```rust,no_run src/lib.rs
use seleniumbase_rs::plugin_api::{
    async_trait, Capability, Category, Metadata, Plugin, Result, Task, TaskContext,
};
use serde_json::{json, Value};

struct Titles;

#[async_trait]
impl Task for Titles {
    async fn run(&self, cx: &TaskContext<'_>) -> Result<Value> {
        let url = cx.input["url"].as_str().unwrap_or("https://example.com");
        let start = Some(url); // the plugin API's new_page takes the first address
        let page = cx.browser.new_page(start).await?;
        let title = page.evaluate("document.title").await?;
        cx.sink.record(&json!({ "url": url, "title": title }))?;
        Ok(json!({ "pages": 1 }))
    }
}

struct TitlesPlugin(Titles);

#[async_trait]
impl Plugin for TitlesPlugin {
    fn name(&self) -> &str {
        "titles"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Core).capability(Capability::Task)
    }

    fn task(&self) -> Option<&dyn Task> {
        Some(&self.0)
    }
}
```

Run a task from the command line, which starts a browser, gives the task the
JSON in `--input` and writes each result as one JSON line to `--out`:

```bash
sbrs plugin run crawler --input crawl.json --out found.jsonl
```

An MCP client runs one with the `run_plugin_task` tool, and gets the results as
progress. The official `batcher` and `crawler` plugins are tasks; their READMEs
in the plugins repository list their inputs.


## When the hooks run

| Hook | Pure CDP engine |
|---|---|
| `before_launch` | In `Browser::launch`, before the process starts. The changed settings become the launch options: arguments, preferences (written into the profile's `Default/Preferences`), extensions (loaded over the protocol), the proxy and headless mode. |
| `on_browser` | Once the browser answers, before the first tab is prepared; after `connect_with_plugins`. |
| packages, then `on_page_created` | For every tab the library attaches to (the first tab, `new_page`, `new_window`, `pages`, tabs in a `BrowserContext`), after the library's own set-up, plugin by plugin. |
| `on_close` | Once per launch: in `Browser::close`; or when the last handle is dropped without `close`, in the background; or, when the launch fails after `before_launch` ran, before `launch` returns the error. A first attempt that is retried does not count, so a plugin's clean-up does not run under the retry. |

The proxy reaches `before_launch` as its server only: a password in the launch
options is not shown to plugins, and it stays if they leave the proxy as it is.
A plugin that sets a new proxy may include a password in it.

## The WebDriver engine

`BrowserConfig::with_plugins` gives a set to `BaseCase::new`, and to
`BrowserSession::connect`, which `BaseCase::new` calls. The hooks run at these
points:

| Hook | WebDriver engine |
|---|---|
| `before_launch` | Before the driver is asked for a browser. The settings go back into the `BrowserConfig` the session starts with, and the set is checked again. |
| `on_browser` | Once the session runs. |
| packages, then `on_page_created` | For the first window; for each window opened with `switch_to_new_window`, `open_new_window`, `open_new_tab` or `open_new_tab_url`; and for a window you switched to yourself, such as a pop-up the page opened, when you call `apply_plugins_to_window` (on `BaseCase` or `BrowserSession`). Switching to a window runs no hook. |
| `on_close` | At `quit`, while the browser still runs; in the background when the session is dropped without `quit`; and when the launch fails after `before_launch` ran. |

`reconnect` checks the plugins for the new browser first, so a refusal leaves
the session as it was; then the old browser's plugins get `on_close`, and the
new browser goes through every hook, `before_launch` starting again from the
configuration. `get_new_driver` starts a browser with its own run of the hooks.

What reaches a plugin depends on the browser:

| | Chromium browsers (Chrome, Edge, Chromium, Brave, Opera, Comet, Chrome for Testing) | Firefox | Safari |
|---|---|---|---|
| `before_launch` shows | the extra arguments, the `prefs` of `goog:chromeOptions` (`ms:edgeOptions` for Edge), the extension folders, the proxy (its server only), headless | Firefox's own arguments and preferences, headless | headless, always off |
| `before_launch` may not | (no limit: every change reaches the browser) | add extension folders (install add-ons with `BaseCase::install_addon` once the session runs) or set a proxy | change anything: a change is refused |
| `send`, on the browser and on a page | chromedriver's DevTools channel (`goog/cdp/execute`), which reaches the window the session is on | refused with an error | refused with an error |
| `add_init_script`, and packages | `Page.addScriptToEvaluateOnNewDocument` for the window's next documents, and run in the current one | run now as a WebDriver script, and again after each `open` | as Firefox |
| `evaluate` | a WebDriver script in the current frame | the same | the same |

On Firefox and Safari a script for every document runs after the page's own
scripts, not before them, and as the body of a function: its top-level
declarations stay local, so assign to `window` to share a value. It runs again
only after `open` (`BrowserSession::goto`), not after a click, a redirect,
`refresh`, `go_back` or `go_forward` takes the window to another document. A
plugin that must run before the page's scripts, or that needs `send`, should
say `Requirements::CHROMIUM_ONLY`, so it is refused on those browsers instead
of half working.

The set is also checked against what the engine can give. A plugin that declares
what it needs (`metadata()`) is refused before anything starts when the engine
cannot provide it: WebDriver has no network observation or interception, and
Firefox and Safari have no DevTools channel. A WebDriver server that answers
`browserName: tauri` (tauri-plugin-sbrs-automation) is checked again once the
session exists, as an embedded webview, which gives page scripts and
evaluation only; a refusal quits the session. A plugin that declares nothing is
not refused for lack of a DevTools channel on those browsers, as before: it
fails when it calls `send`.

The launch settings that are Chromium switches (`host_resolver_rules`,
`disable_features`, `enable_features`, `webrtc_policy`, `disable_quic`) become
one `--host-resolver-rules`, `--disable-features`, `--enable-features`,
`--force-webrtc-ip-handling-policy` and `--disable-quic` argument, joined with
the ones the configuration already has. A plugin that sets one for Firefox or
Safari is refused. On every engine a plugin's `Fetch.*` command is refused
(the host owns request interception), and `Network.setBlockedURLs` adds to the
window's blocklist, with the `ad_block` list, instead of replacing it.

A Firefox preference must be true or false, a whole number or text. Internet
Explorer takes no plugins. Plugins cannot be combined with `reuse_session`: a
browser kept for reuse would get the hooks a second time, or not at all, so the
combination is refused. A session on a server other than the local default
(`with_webdriver_url`) counts as attaching for `LAUNCH_ONLY`; `before_launch`
still runs there, and its settings travel in the capabilities, so extension
folders must exist on the machine that runs the browser.

## When a plugin fails

Plugins fail closed. A refusal from the checks, or an error from
`before_launch`, `on_browser`, a package or `on_page_created`, stops the launch
or the tab and comes back as `SeleniumBaseError::Plugin`, which names the plugin
and the hook. A launch stopped by a plugin is not retried. A tab the library
opened for `new_page` or `new_window` is closed again rather than handed out
without what the plugin adds: a stealth or proxy plugin that did not run must
not leave an unprotected browser.

Errors from `on_close` are logged (event `cdp.plugin.close.failed`) and do not
stop the browser from closing.

A plugin cannot take the program down. A panic in any hook is caught and comes
back as `PluginError::Panicked`, naming the plugin and the hook. A set that has
a timer (`PluginSet::with_timer`) cuts every hook off at its limit with
`PluginError::Timeout`: `before_navigate` 5 s, `on_request` 2 s (the request is
blocked), `before_launch` and `on_test_end` 30 s, every other hook 10 s. A
plugin may ask for other limits by setting the fields of a `Budgets::default()`
and passing it to `Metadata::budget`; a limit is raised no higher than four
times the default. A set without a timer applies no limit.

Every browser, tab and context handle a hook gets is guarded for that plugin.
`Fetch.*`, `Target.setAutoAttach`, `Browser.close` and the crash commands are
refused for every plugin, a protocol command needs the capability it calls for,
and a hook that would run its own hooks again (navigating inside
`before_navigate`, opening a tab inside a page hook) fails with
`PluginError::Reentrant`. Errors from `after_navigate`, `on_response`,
`on_protocol_event`, `before_close`, `on_context_closed`, `on_test_start` and
`on_test_end` are logged and change nothing.

On the WebDriver engine, a refusal, a launch setting the browser cannot take,
or an error from `before_launch`, `on_browser`, a package or `on_page_created`
for the first window stops `BaseCase::new` (or `BrowserSession::connect`) with
`SeleniumBaseError::Plugin`, and a browser that had started is quit first. For
a window opened later, the error is what the call that opened it returns; the
window stays open, so close it or stop the test. A value the browser cannot be
given (an extension folder whose path holds a comma, a Firefox preference that
is a fraction) is `SeleniumBaseError::InvalidConfig`, as is plugins with
`reuse_session`. Errors from `on_close` are logged as
`webdriver.plugin.close.failed`.

## Security

<Danger>

A plugin is Rust code that runs with your program's rights. It sees every page,
cookie and password the browser handles, and it can send any protocol command.
Adding one is the same trust decision as adding any dependency.

</Danger>

- Read it, or trust whoever wrote it, before you add it.
- Keep `Cargo.lock` in version control. It records the commit each git
  dependency resolved to, so a plugin cannot change under you until you run
  `cargo update`, and you can read the change before you commit it.
- Run `cargo deny check` and `cargo audit` on the result; this repository's
  `deny.toml` allows only the git sources it names.

Packages run inside the page, with the site's origin. Pinned versions,
Subresource Integrity and `only_on` keep a compromised CDN or a typo from
reaching every site the browser visits.

## What has and has not been verified

<Warning>

On the Pure CDP engine, the hooks, their order, the commands they send, the
fail-closed errors, closing a tab a plugin refused, and `on_close` on close and
on drop are covered by tests against the mocked browser
(`tests/sb_cdp_plugins.rs`); the conversion between the launch options and
`LaunchSettings`, and the merging of preferences into a profile, by unit tests.
None of it has been run against a real Chrome yet: loading an extension with
`Extensions.loadUnpacked` follows the repository's existing real-Chrome test of
the same command, and the `Default/Preferences` file is the one Chrome reads at
start-up, but neither was checked in a live browser for this feature.
`connect_with_plugins` is tested only for its checks, not against a running
browser. The same holds for the host's part of the plugin platform: a fresh
instance of a per-launch plugin for each browser, the host check, the merged
launch switches, the one blocklist per tab and the refusal of `Fetch.*` are
covered by tests against the mocked browser and the mock WebDriver server, and
by unit tests that read the command line; no real Chrome has been shown to
accept the merged switches or to apply the blocklist in iframes, workers and
pop-ups.

On the WebDriver engine, the conversion between a `BrowserConfig` and
`LaunchSettings` for each browser, and what is refused, are covered by unit
tests; the hooks, their order, what reaches the driver, the DevTools channel on
Chromium and its refusal on Firefox, the scripts run again after `open`, the
windows a session and `BaseCase` open, `reconnect`, the fail-closed errors and
`on_close` at quit, on drop and after a failed launch, by tests against a mock
WebDriver server. `tests/webdriver_plugins_chrome.rs` runs one plugin through
`BaseCase` and chromedriver against a real Chrome: a preference set in
`before_launch`, `Browser.getVersion` through the DevTools channel, a time zone,
a package and an init script in the first window, the time zone and the init
script in a window `open_new_window` opens, `evaluate`, and `on_close` at quit.
It passed in Chrome 155, headless, on macOS. Firefox and Safari have run only
against the mock server, and the extension folders, the proxy and a remote
server were not tried with a live browser.

</Warning>
