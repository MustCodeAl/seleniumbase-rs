---
title: "ADR 004: A Plugin Framework for Community Crates"
sidebarTitle: "004: Plugin framework"
---

## Status

Accepted, in progress. Proposed (2026-10-12), revised the same day: the
framework lives in its own repository, `seleniumbase-rs-plugins`, and this crate
keeps only the hook points that call it. Phase 1 (the plugins repository),
phase 2 (the hook points of the pure CDP engine) and phase 3 (the hook points
of the WebDriver engine), behind the `plugins` feature, are done, except the
`test_plugin` hook; phases 4 and 5 remain. The phases at the end give the
order, what each one holds and where the result differs from the plan.

## Context

Puppeteer and Playwright users extend their browsers with plugins:
[puppeteer-extra](https://github.com/berstend/puppeteer-extra) and its
maintained fork [Zorilla](https://github.com/zorillajs/zorilla) wrap `launch()`
and call every registered plugin at fixed points (`beforeLaunch`, `afterLaunch`,
`onPageCreated`, `onClose`, ...). Stealth, ad blocking, captcha solving,
resource blocking and user preferences are all separate packages that anyone can
publish, and `puppeteer.use(StealthPlugin())` is all a user writes. They are an
example of the ecosystem we want, not a design to copy: the design below is
shaped by Rust (no runtime loading, traits, Cargo) and by this crate's two
engines.

seleniumbase-rs has three extension points today, and none of them lets a
third-party crate change how a browser starts or what every page gets:

| Today | What it covers | What it cannot do |
|---|---|---|
| `plugins::observer::TestPlugin` | Test started, failed (with the page), finished | Change launch options; see pages as they open |
| `stealth::providers::EvasionProvider` | JavaScript added to every document by the stealth layer | Anything outside the stealth bootstrap script |
| `LaunchOptions` flags (`ad_block`, `shield_webrtc`, `identity`) | Fixed built-in behaviours | New behaviours without changing this crate |

The Chrome extension in
[seleniumbase-rs-browser-extension](https://github.com/MustCodeAl/seleniumbase-rs-browser-extension)
is a different thing: it runs inside the browser. This ADR is about Rust code
that runs in the program driving the browser.

## Decision

A small plugin API lives in its own repository and crate; seleniumbase-rs
depends on it and calls it; community plugins depend only on it.

### Where the code lives

| Repository / crate | Holds | Depends on |
|---|---|---|
| `seleniumbase-rs-plugins` / `seleniumbase-rs-plugin` | the `Plugin` trait, `PluginSet` (order, dependencies, requirements), `LaunchSettings`, the `BrowserHandle` and `PageHandle` traits, `Package` | `async-trait`, `serde_json`, `thiserror` only |
| `seleniumbase-rs-plugins` / `seleniumbase-rs-plugin-testing` | recording fakes of `BrowserHandle` and `PageHandle`, so a plugin is tested without a browser or this crate | the API crate |
| `seleniumbase-rs-plugins` / `plugins/*` | official plugins (block resources, JS packages, user preferences, ...) and `template/` for new ones | the API crate |
| `seleniumbase-rs` | the hook points: it turns `LaunchSettings` into `LaunchOptions` or `BrowserConfig`, implements the handle traits for both engines, and calls the hooks | the API crate |

Plugins never name `seleniumbase-rs` types, so they do not break when this crate
changes, and the API crate changes rarely. Both engines take a `PluginSet`.
Community plugins are ordinary crates.

### The trait

Every hook is optional and does nothing by default, so a plugin implements only
what it needs.

```rust
#[async_trait]
pub trait Plugin: Send + Sync + 'static {
    /// Unique, lower-case name ("adblock", "block-resources").
    fn name(&self) -> &str;

    /// Other plugins this one needs, by name.
    fn dependencies(&self) -> &[&str] { &[] }

    /// Flags such as RUN_LAST, HEADFUL, LAUNCH_ONLY, CHROMIUM_ONLY.
    fn requirements(&self) -> Requirements { Requirements::empty() }

    /// JavaScript and CSS to load into pages (see "Packages").
    fn packages(&self) -> Vec<Package> { Vec::new() }

    /// Called once when added to a set.
    async fn on_registered(&self) -> Result<()> { Ok(()) }

    /// Either engine: change the browser's start-up settings (arguments,
    /// preferences, extensions, proxy, headless) before it starts.
    async fn before_launch(&self, launch: &mut LaunchSettings) -> Result<()> { Ok(()) }

    /// The browser is running (launched or connected).
    async fn on_browser(&self, browser: &BrowserHandle) -> Result<()> { Ok(()) }

    /// A tab was attached, before any command of the caller runs in it.
    async fn on_page_created(&self, page: &PageHandle) -> Result<()> { Ok(()) }

    /// The browser is about to close.
    async fn on_close(&self) -> Result<()> { Ok(()) }
}
```

`BrowserHandle` and `PageHandle` are small types that send a CDP command
(`send(method, params)`), add a script to every new document, evaluate
JavaScript, and say which engine and browser they belong to. Plugins receive
them instead of `sb_cdp::Browser` or `BaseCase`, so the same plugin works with
both engines and the hook signatures stay stable when those types grow.

The existing `TestPlugin` stays as it is. A `Plugin` that also wants the test
hooks returns one from `fn test_plugin(&self) -> Option<Box<dyn TestPlugin>>`.

### Using it

```rust
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_plugin::PluginSet;
use seleniumbase_rs_plugin_block_resources::BlockResources;

let plugins = PluginSet::new()
    .with(BlockResources::images_and_fonts())
    .with(my_plugin);
let options = LaunchOptions::builder().headless(true).plugins(plugins).build()?;
let browser = Browser::launch(options).await?;
```

On WebDriver the same set goes to `BrowserConfig::with_plugins(plugins)`.

### Where the hooks run

| Hook | Pure CDP engine | WebDriver engine |
|---|---|---|
| `before_launch` | `Browser::launch`, before `launch()`; the settings become `LaunchOptions` | `BaseCase::new`, before the driver starts; the settings become `BrowserConfig` |
| `on_browser` | after launch and after `Browser::connect` | after the session starts |
| `on_page_created` | in `prepare_session`, for every tab the library attaches to | Chromium only, through the DevTools bridge; other browsers skip it |
| `packages` | added in `prepare_session` | added at session start on Chromium; on other browsers after each `open` |
| `on_close` | `Browser::close` and drop | `BaseCase::quit` |

### Rules the set enforces

- **Order.** Plugins run in the order they were added, except those with
  `RUN_LAST`, which run after the rest (Zorilla does the same).
- **Dependencies.** A missing dependency stops the launch with
  `SeleniumBaseError::Plugin`. Rust cannot install a crate at run time, so the
  error names the plugin and the crate to add. A plugin may also bring its own
  dependencies with `fn bundled(&self) -> Vec<Arc<dyn Plugin>>`; they are added
  when missing.
- **Requirements.** `HEADFUL` refuses a headless launch, `LAUNCH_ONLY` refuses
  `connect`, `CHROMIUM_ONLY` refuses Firefox and Safari. Each refusal is an
  error, not a warning.
- **Errors fail closed.** An error from `before_launch`,
  `on_browser` or `on_page_created` stops that launch or that page and is
  returned to the caller: a stealth or proxy plugin that did not run must not
  leave an unprotected browser. Errors from `on_close` and from the test hooks
  are logged, as `TestPlugin` errors are today.
- **Names are unique.** Adding a second plugin with the same name is an error.

### Packages (JavaScript and CSS)

This is the JS package manager of Python SeleniumBase (`add_js_link`,
`add_css_link`), made declarative so a plugin can say what every page needs:

```rust
Package::script_url("https://cdn.jsdelivr.net/npm/intro.js@7.2.0/intro.min.js")
    .integrity("sha384-...")      // Subresource Integrity, checked by the browser
    .only_on(["example.com"])     // hosts to load it on; default: none
Package::style_url("https://cdn.jsdelivr.net/npm/intro.js@7.2.0/introjs.min.css")
Package::script("window.myHelper = () => 42;")
Package::style("body { zoom: 0.9 }")
```

A package needs a pinned version in its URL and either `only_on` or
`.everywhere()`, so a plugin never loads a CDN script into every site by
accident. Code from a URL can be fetched by the program and injected inline
(`.inline()`), as `add_js_code_from_link` does, for pages whose content
security policy blocks the CDN.

The same work fills a gap: `sb_cdp::Page` gets `add_script_tag`,
`add_style_tag` and `add_init_script`, which `BaseCase` already has as
`add_js_link`, `add_css_link` and `add_js_code`.

### Distribution

- Crates are named `seleniumbase-rs-plugin-<name>` and tagged
  `seleniumbase-rs-plugin` on GitHub, so a search finds them. The docs keep a
  list of community plugins.
- `template/` in the plugins repository has the crate layout, a test against the
  recording fakes, and CI. Copy it, or use it with `cargo generate`.
- Plugin authors test with `seleniumbase-rs-plugin-testing`: the fakes record
  each CDP command and script a hook sends, with no browser and no dependency on
  this crate. An end-to-end check can still use `Browser::new_mocked`.
- Until the crates are on crates.io, a plugin and the program must resolve to the
  same `seleniumbase-rs-plugin`; two git tags are two different crates to Cargo.
  The API crate changes rarely, and when it does the program fixes a mismatch
  with one `[patch]` entry, which the docs show. Publishing to crates.io later
  ends the problem through semver.

### What is not part of this

- **Loading plugins at run time** (`dlopen`, a plugin folder). Rust has no
  stable ABI, so a plugin built with another compiler or crate version would
  crash. Plugins are compiled in.
- **WebAssembly plugins.** Possible later, if people want plugins in other
  languages; not needed for Rust crates.
- **A plugin hub or registry service.** A list in the docs and a GitHub topic
  are enough until there are many plugins.

## Security

A plugin is Rust code that runs with the program's rights and sees every page,
cookie and password the browser handles. Adding one is the same trust decision
as adding any dependency: review it, pin it, and run `cargo deny`. The docs say
this on the page that explains how to install a plugin.

Packages run inside the page, with the site's origin. Pinned versions,
Subresource Integrity and `only_on` keep a compromised CDN or a typo from
reaching every site the browser visits.

## Consequences

- Built-in behaviours can move into plugins over time (`ad_block`,
  `shield_webrtc`, the stealth bootstrap as a `StealthPlugin` over the
  `EvasionRegistry`). The existing `LaunchOptions` flags keep working; they
  become shortcuts that add the built-in plugin.
- Every launch and every new tab now walks the plugin set. With no plugins that
  is an empty loop.
- The hook list becomes public API. Adding a hook with a default body is not a
  breaking change; changing a signature is.

## Phases

1. **The plugins repository.** Create `seleniumbase-rs-plugins` (public) with
   the API crate (`Plugin`, `PluginSet`, `Requirements`, `Package`,
   `LaunchSettings`, the handle traits), the testing crate, tests for order,
   dependencies, requirements and fail-closed errors, and `template/`.
2. **Hook points, pure CDP engine.** seleniumbase-rs depends on the API crate:
   `LaunchOptions::plugins`, the hooks in `Browser::launch`, `connect`,
   `prepare_session` and `close`, `PageHandle` for `sb_cdp::Page`,
   `Page::add_script_tag`, `add_style_tag`, `add_init_script`,
   `SeleniumBaseError::Plugin`; one official plugin (block resources).

   *Done* (2026-10-15), behind the optional `plugins` feature, with the API
   crate pinned to a git revision and re-exported as
   `seleniumbase_rs::plugin_api`. `LaunchOptionsBuilder::plugins`;
   `Browser::launch` checks the set, runs `before_launch` (the settings become
   `LaunchOptions`: arguments, preferences written into the profile, extensions
   loaded with `Extensions.loadUnpacked`, proxy, headless), checks again, and
   calls `on_browser`; `Browser::connect_with_plugins` checks as attaching and
   calls `on_browser`; `prepare_session` adds the packages and calls
   `on_page_created` through a `PageHandle` on the tab's session; `close` and
   the last drop call `on_close` once. Errors stop the launch or the tab as
   `SeleniumBaseError::Plugin`, and a tab opened by `new_page` is closed again.
   Block resources lives in the plugins repository. Left for phase 3, and done
   there: the `Page::add_script_tag`, `add_style_tag` and `add_init_script`
   methods, and `PageHandle` for `sb_cdp::Page` itself (the hooks use a handle
   on the tab's session). The tests run against the mock transport, not a real
   Chrome. Guide: [Browser Plugins](/tutorials/plugins).
3. **Hook points, WebDriver engine.** `BrowserConfig::with_plugins`, the
   Chromium page hooks through the DevTools bridge, `test_plugin`.

   *Done* (2026-10-15), except `test_plugin`, which the API crate does not
   have yet. `BrowserConfig::with_plugins`; `BrowserSession::connect`, and so
   `BaseCase::new`, checks the set against a WebDriver `LaunchContext` (a
   server other than the local default counts as attaching), runs
   `before_launch` on settings taken from the configuration and puts them back
   (Chromium: arguments, capability preferences, extension folders, proxy,
   headless; Firefox: its arguments and preferences, headless; Safari:
   nothing), checks again, then calls `on_browser` and gives the first window
   the packages and `on_page_created`. Windows opened with
   `switch_to_new_window` and `BaseCase::open_new_window` get them too, and
   `apply_plugins_to_window` gives them to a window reached another way;
   `reconnect` runs the hooks again for the new browser; `quit` and drop call
   `on_close`. A refusal or a failing hook stops the session with
   `SeleniumBaseError::Plugin` and quits the browser. Unlike the table above,
   `on_page_created` runs on every browser, Firefox and Safari refusing `send`;
   their init scripts and packages run as WebDriver scripts at once and again
   after each `open`, after the page's own scripts. Plugins together with
   `reuse_session` are refused. `sb_cdp::Page` got
   `add_script_tag`, `add_style_tag`, `add_init_script` and
   `remove_init_script`, and implements `PageHandle`. Tested against a mock
   WebDriver server and the mocked browser, and in Chrome 155 headless on
   macOS (`tests/webdriver_plugins_chrome.rs`,
   `tests/sb_cdp_page_inject_chrome.rs`); Firefox and Safari only against the
   mock server.
4. **Built-ins and community.** `ad_block` and `shield_webrtc` as plugins, a
   `StealthPlugin`, the real Bootstrap Tour and Hopscotch libraries for the two
   tour themes that fall back to the built-in tour today, the template
   and a "Writing a plugin" tutorial.
5. **Later, if asked for.** Plugins that add MCP tools (feature `mcp-server`),
   and a `data()` hook that lets plugins share values, as puppeteer-extra's
   `getDataFromPlugins` does.
