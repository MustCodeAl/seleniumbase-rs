---
title: "Browser Plugins"
---

# Browser Plugins

A browser plugin is a Rust crate that changes how a browser starts and what
every tab gets: block images, add a script to every page, set a preference,
load an extension, route through a proxy. You add one to a `PluginSet`, give the
set to the launch options, and its hooks run at fixed points. It is the idea of
[puppeteer-extra](https://github.com/berstend/puppeteer-extra) in Rust: plugins
are ordinary crates, compiled into your program, and anyone can publish one.

The design is [ADR 004](../adr/004-plugin-framework.md). Today the hooks run in
the Pure CDP engine (`sb_cdp`); the WebDriver engine (`BaseCase`) follows.

Browser plugins are not [Test Plugins](test_plugins.md), which run around a
test (screenshots on failure, reports), and not the
[browser extension](https://github.com/MustCodeAl/seleniumbase-rs-browser-extension),
which runs inside the browser. A browser plugin is Rust code in the program that
drives the browser.

## What you will learn

- Turn plugins on and use one
- Install a plugin from the plugins repository
- Write a plugin, and test it without a browser
- When each hook runs, and what happens when one fails
- What you trust when you add a plugin

## Turning plugins on

Plugins are behind the `plugins` feature, off by default. The feature is newer
than release 0.2.0, so take the crate from the main branch:

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", branch = "main", features = ["plugins"] }
```

The plugin API is its own crate, `seleniumbase-rs-plugin`, in the
[seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)
repository. This crate re-exports it as `seleniumbase_rs::plugin_api`, so a
program needs no second dependency to use or write plugins.

## Using a plugin

Build a set, give it to `LaunchOptionsBuilder::plugins`, and launch as usual:

```rust,no_run
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

`PluginSet::with` refuses a second plugin with the same name. Plugins run in
the order they were added, except those that ask to run last.

To attach to a browser that is already running, use
`Browser::connect_with_plugins(host, port, plugins)` instead of `connect`. There
is nothing to launch, so `before_launch` does not run; the rest does.

## Installing a plugin

Plugins are crates named `seleniumbase-rs-plugin-<name>`. The official ones live
in the plugins repository:

| Crate | What it does |
|---|---|
| `seleniumbase-rs-plugin-block-resources` | Stops pages from loading images, fonts or media (`Network.setBlockedURLs`) |
| `seleniumbase-rs-plugin-crx` | Loads packed Chrome extensions (`.crx` files): unpacks them and adds the folders to the start-up settings, which the launch loads |

Add one with `cargo add`, from the **same revision** of the repository that
this crate uses (it is in this crate's `Cargo.toml`, next to
`seleniumbase-rs-plugin`):

```sh
cargo add --git https://github.com/MustCodeAl/seleniumbase-rs-plugins \
    --rev a70c00fb4bc1c82c09c1175db6efe01b9daf3768 \
    seleniumbase-rs-plugin-block-resources
```

The revision matters until the crates are on crates.io: two revisions of the
same git repository are two different crates to Cargo, so a plugin built
against another revision implements a different `Plugin` trait. The compiler
then says the trait `Plugin` is not implemented for the plugin, and mentions two
versions of `seleniumbase_rs_plugin`. Use one revision for every crate from that
repository, and the one this crate pins.

## Writing a plugin

Implement `Plugin`. Every hook is optional; `name` is the only method you must
write.

```rust,no_run
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
Plugins never see `sb_cdp::Browser` or `Page`, so a plugin keeps working when
those types grow, and the same plugin will work with the WebDriver engine.

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
| `Requirements::LAUNCH_ONLY` | a browser attached with `connect_with_plugins` |
| `Requirements::CHROMIUM_ONLY` | browsers that are not Chromium-based |
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

## Security

A plugin is Rust code that runs with your program's rights. It sees every page,
cookie and password the browser handles, and it can send any protocol command.
Adding one is the same trust decision as adding any dependency:

- Read it, or trust whoever wrote it, before you add it.
- Pin it to a revision (`--rev`), as above, so it cannot change under you.
- Run `cargo deny check` and `cargo audit` on the result; this repository's
  `deny.toml` allows only the git sources it names.

Packages run inside the page, with the site's origin. Pinned versions,
Subresource Integrity and `only_on` keep a compromised CDN or a typo from
reaching every site the browser visits.

## What has and has not been verified

The hooks, their order, the commands they send, the fail-closed errors, closing
a tab a plugin refused, and `on_close` on close and on drop are covered by tests
against the mocked browser (`tests/sb_cdp_plugins.rs`); the conversion between
the launch options and `LaunchSettings`, and the merging of preferences into a
profile, by unit tests. None of it has been run against a real Chrome yet:
loading an extension with `Extensions.loadUnpacked` follows the repository's
existing real-Chrome test of the same command, and the `Default/Preferences`
file is the one Chrome reads at start-up, but neither was checked in a live
browser for this feature. `connect_with_plugins` is tested only for its checks,
not against a running browser.
