---
title: "ADR 004: A Plugin Framework for Community Crates"
---

# ADR 004: A Plugin Framework for Community Crates

## Status

Proposed (2026-10-12). Nothing here is built yet; the phases at the end say in
which order it will be.

## Context

Puppeteer and Playwright users extend their browsers with plugins:
[puppeteer-extra](https://github.com/berstend/puppeteer-extra) and its
maintained fork [Zorilla](https://github.com/zorillajs/zorilla) wrap `launch()`
and call every registered plugin at fixed points (`beforeLaunch`, `afterLaunch`,
`onPageCreated`, `onClose`, ...). Stealth, ad blocking, captcha solving,
resource blocking and user preferences are all separate packages that anyone can
publish, and `puppeteer.use(StealthPlugin())` is all a user writes.

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

Add a module `seleniumbase_rs::extra` (named after puppeteer-extra, so people
coming from it find it) with one trait, `Plugin`, and one ordered set,
`PluginSet`. Both engines take a `PluginSet` and call its hooks. Community
plugins are ordinary crates.

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

    /// Pure CDP engine: change the options before Chrome starts.
    async fn before_launch(&self, options: &mut LaunchOptions) -> Result<()> { Ok(()) }

    /// WebDriver engine: change the configuration before the session starts.
    async fn before_session(&self, config: &mut BrowserConfig) -> Result<()> { Ok(()) }

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
use seleniumbase_rs::extra::PluginSet;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
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
| `before_launch` | `Browser::launch`, before `launch()` | not called |
| `before_session` | not called | `BaseCase::new`, before the driver starts |
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
- **Errors fail closed.** An error from `before_launch`, `before_session`,
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
- A template repository (`seleniumbase-rs-plugin-template`) has the crate
  layout, a mock-browser test and CI.
- Plugin authors test against `Browser::new_mocked` and `MockCtrl` (feature
  `test-util`): the mock records each CDP command a hook sends.
- Until the crate is on crates.io, a plugin and the program using it must
  resolve to the same `seleniumbase-rs`; two git tags are two different crates
  to Cargo, and their `Plugin` traits do not match. The program fixes this with
  one `[patch]` entry, which the docs show.

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

1. **Core, pure CDP engine.** `extra` module, `Plugin`, `PluginSet`,
   `Requirements`, `Package`, `PageHandle`; hooks in `Browser::launch`,
   `connect`, `prepare_session` and `close`; `Page::add_script_tag`,
   `add_style_tag`, `add_init_script`; `SeleniumBaseError::Plugin`; mock tests
   for order, dependencies, requirements and fail-closed errors; one example
   plugin (`examples/plugin_block_resources.rs`).
2. **WebDriver engine.** `BrowserConfig::with_plugins`, `before_session`, the
   Chromium page hooks through the DevTools bridge, `test_plugin`.
3. **Built-ins and community.** `ad_block` and `shield_webrtc` as plugins, a
   `StealthPlugin`, the real Bootstrap Tour and Hopscotch libraries for the two
   tour themes that fall back to the built-in tour today, the template
   repository, and a "Writing a plugin" tutorial.
4. **Later, if asked for.** Plugins that add MCP tools (feature `mcp-server`),
   and a `data()` hook that lets plugins share values, as puppeteer-extra's
   `getDataFromPlugins` does.
