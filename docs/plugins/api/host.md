---
title: "The seleniumbase-rs Host"
sidebarTitle: "Host"
description: "Reference for the host side of the plugin API in seleniumbase-rs: the plugins features, the plugin_api and builtin_plugins re-exports, the launch builders of both engines, Browser::connect_with_plugins, plugin_host::builtin_registry, build and build_with, the request guard, and SeleniumBaseError::Plugin."
---

<Badge color="blue">Feature: plugins</Badge>

seleniumbase-rs is the host that runs plugins: its two engines build the
`LaunchContext` and `LaunchSettings`, call the set's hooks and give plugins
their handles. A program only builds a set, or names plugins, and hands them to
a launch builder. Every item on this page is in the crate `seleniumbase-rs`
and needs its feature `plugins` (the `plugin-*` features turn it on). Turning
plugins on by name from a file, a variable or flags is on
[Selection](/plugins/api/selection); the user guide is
[Choosing plugins](/plugins/selecting).

<RequestExample>

```rust Pure CDP: a set
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

let plugins = PluginSet::new().with(Hello)?;
let options = LaunchOptions::builder().headless(true).plugins(plugins).build()?;
let browser = Browser::launch(options).await?; // check, before_launch, on_browser
browser.close().await?; // on_close
```

```rust Pure CDP: by name
// seleniumbase-rs with features = ["plugin-shield", "plugin-block-resources"]
use seleniumbase_rs::config::plugins::PluginSelection;
use seleniumbase_rs::sb_cdp::LaunchOptions;
use serde_json::json;

let options = LaunchOptions::builder()
    .plugins_named(["block-resources"])
    .plugin_selection(
        &PluginSelection::new()
            .enable("shield")
            .with_options("shield", json!({ "level": "strict" })),
    )
    .build()?; // resolves the names here
```

```rust Attach
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs::sb_cdp::Browser;

// No before_launch: nothing is launched. A LAUNCH_ONLY plugin refuses.
let plugins = PluginSet::new().with(Hello)?;
let browser = Browser::connect_with_plugins("127.0.0.1", 9222, plugins).await?;
```

```rust WebDriver
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs::{BaseCase, BrowserConfig};

// seleniumbase-rs with features = ["plugin-shield"]
let config = BrowserConfig::default()
    .with_plugins(PluginSet::new().with(Hello)?) // first: it replaces the set
    .with_plugins_named(["shield"])?; // resolved at once
let mut sb = BaseCase::new(config).await?;
sb.quit().await?;
```

```rust Your own registry
use std::path::Path;

use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::plugin_api::Descriptor;
use seleniumbase_rs::plugin_host;

// The [plugins] table of the settings file, with SB_PLUGINS on top.
let selection = Settings::load_plugin_selection(None::<&Path>)?;
let registry = plugin_host::builtin_registry().with(Descriptor::of::<Hello>());
let plugins = plugin_host::build_with(&registry, &selection)?;
```

</RequestExample>

<ResponseExample>

```text Unknown name
plugin error: no plugin named `sheild` is available (known: shield, block-resources, crx). The built-in plugins are cargo features of seleniumbase-rs: plugin-shield, plugin-block-resources, plugin-crx, plugin-fetcher, plugin-extractor, plugin-batcher, plugin-crawler (plugins-builtin turns them all on). A plugin from another crate is added with PluginSet::with, or by name through Registry::with(Descriptor::of::<P>()) and plugin_host::build_with
```

```text Named twice
plugin error: a plugin named `shield` is already in the set
```

```text Stray options
plugin error: options were given for `shield`, which is not in `enable` ([plugins.shield]); add it to `enable`, or remove the options (a misspelled `enable` looks like this too)
```

</ResponseExample>

## Cargo features

<Badge color="gray">features</Badge> `seleniumbase-rs/Cargo.toml`

| Feature | Turns on |
|---|---|
| `plugins` | The optional dependency `seleniumbase-rs-plugin`: `plugin_api`, `plugin_host`, the launch-builder methods and the hooks of both engines. No plugin of its own |
| `plugin-shield` | `plugins`, and `seleniumbase-rs-plugins-builtin/shield` |
| `plugin-block-resources` | `plugins`, and `seleniumbase-rs-plugins-builtin/block-resources` |
| `plugin-crx` | `plugins`, and `seleniumbase-rs-plugins-builtin/crx` |
| `plugin-fetcher` | `plugins`, and `seleniumbase-rs-plugins-builtin/fetcher` |
| `plugin-extractor` | `plugins`, and `seleniumbase-rs-plugins-builtin/extractor` |
| `plugin-batcher` | `plugins`, and `seleniumbase-rs-plugins-builtin/batcher` |
| `plugin-crawler` | `plugins`, and `seleniumbase-rs-plugins-builtin/crawler` |
| `plugins-builtin` | every `plugin-*` feature above |

The library's default features are empty. The command-line package
`seleniumbase-rs-cli` has `plugins-builtin` among its defaults and passes
`plugins` and each `plugin-*` through. Both plugin dependencies come from
`https://github.com/MustCodeAl/seleniumbase-rs-plugins` at
`rev = "628991af0e6c5711298e3bd3692dc2974887401a"` (version `0.1`).

## plugin_api

<Badge color="gray">mod</Badge> <Badge color="gray" stroke>re-export</Badge> `seleniumbase_rs::plugin_api`

```rust
#[cfg(feature = "plugins")]
pub use seleniumbase_rs_plugin as plugin_api;
```

The whole API crate, so a program needs no dependency of its own on it:
`seleniumbase_rs::plugin_api::PluginSet` is
`seleniumbase_rs_plugin::PluginSet`. A plugin crate depends on
`seleniumbase-rs-plugin` itself, never on seleniumbase-rs. Not to be confused
with `seleniumbase_rs::plugins`, the test-reporting plugins.

## builtin_plugins

<Badge color="gray">mod</Badge> <Badge color="gray" stroke>re-export</Badge> `seleniumbase_rs::builtin_plugins`

```rust
#[cfg(any(feature = "plugin-shield", feature = "plugin-block-resources", feature = "plugin-crx"))]
pub use seleniumbase_rs_plugins_builtin as builtin_plugins;
```

The crate `seleniumbase-rs-plugins-builtin`: its `registry()` (the built-in
plugins whose feature is on, in the order `shield`, `block-resources`, `crx`),
a module per plugin with that plugin's own types (`shield`, `block_resources`,
`crx`), and re-exports of `Descriptor`, `PluginSpec` and `Registry`. Most
programs enable the built-in plugins by name instead. The `wasm` plugin host is
not in it: it needs the path of a module, so it is added with
`PluginSet::with`.

## LaunchOptionsBuilder::plugins

<Badge color="green">fn</Badge> `seleniumbase_rs::sb_cdp::LaunchOptionsBuilder::plugins`

```rust
#[cfg(feature = "plugins")]
#[must_use]
pub fn plugins(mut self, plugins: seleniumbase_rs_plugin::PluginSet) -> Self
```

<ParamField body="plugins" type="PluginSet" required>

The plugins whose hooks run when the browser starts, for every tab it opens,
and when it closes. It replaces a set given before.

</ParamField>

<ResponseField name="return" type="LaunchOptionsBuilder">

The builder.

</ResponseField>

`Browser::launch` checks the set, lets each plugin change the start-up
settings, starts the browser and calls `on_browser`; every tab the library
attaches to gets the plugins' packages and `on_page_created`;
`Browser::close` calls `on_close`. An error from any hook but `on_close` stops
the launch or the tab. `Browser::launch_mocked` (feature `test-util`) runs the
same steps against a scripted browser.

## LaunchOptionsBuilder::plugins_named

<Badge color="green">fn</Badge> `seleniumbase_rs::sb_cdp::LaunchOptionsBuilder::plugins_named`

```rust
#[cfg(feature = "plugins")]
#[must_use]
pub fn plugins_named<I, S>(mut self, names: I) -> Self
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
```

<ParamField body="names" type="impl IntoIterator<Item = impl Into<String>>" required>

Names of built-in plugins: `shield`, `block-resources`, `crx`, each present
only when its feature is on.

</ParamField>

<ResponseField name="return" type="LaunchOptionsBuilder">

The builder, with the names recorded.

</ResponseField>

The names are resolved by `plugin_host::builtin_registry()` when `build` runs,
each with its default options, and added after the set given to `plugins`. Use
`plugin_selection` to give options.

## LaunchOptionsBuilder::plugin_selection

<Badge color="green">fn</Badge> `seleniumbase_rs::sb_cdp::LaunchOptionsBuilder::plugin_selection`

```rust
#[cfg(feature = "plugins")]
#[must_use]
pub fn plugin_selection(mut self, selection: &PluginSelection) -> Self
```

<ParamField body="selection" type="&PluginSelection" required>

Plugin names and options, such as `Settings::plugins` (the `[plugins]` table
and `SB_PLUGINS`). See [PluginSelection](/plugins/api/selection#pluginselection).

</ParamField>

<ResponseField name="return" type="LaunchOptionsBuilder">

The builder. The selection's names are added after those recorded before, and
its options replace options of the same plugin.

</ResponseField>

## LaunchOptionsBuilder::build

<Badge color="green">fn</Badge> `seleniumbase_rs::sb_cdp::LaunchOptionsBuilder::build`

```rust
pub fn build(self) -> Result<LaunchOptions, SeleniumBaseError>
```

<ResponseField name="return" type="Result<LaunchOptions, SeleniumBaseError>">

The options, with the named plugins built and added to the set.

</ResponseField>

With the feature `plugins`, `build` resolves the names of `plugins_named` and
`plugin_selection` through `builtin_registry()` and reports an unknown name,
options a plugin refuses, options for a plugin that is not enabled, and a
plugin named twice as `SeleniumBaseError::Plugin`. When no name was given, the
set of `plugins` is kept as it is. Otherwise an empty set becomes strict, and a
set that already holds plugins keeps its own strictness.

## Browser::connect_with_plugins

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> `seleniumbase_rs::sb_cdp::Browser::connect_with_plugins`

```rust
#[cfg(feature = "plugins")]
pub async fn connect_with_plugins(
    host: &str,
    port: u16,
    plugins: seleniumbase_rs_plugin::PluginSet,
) -> Result<Self, SeleniumBaseError>
```

<ParamField body="host" type="&str" required>

The host of a browser that listens for the DevTools Protocol, such as
`127.0.0.1`.

</ParamField>

<ParamField body="port" type="u16" required>

Its DevTools port, such as `9222`.

</ParamField>

<ParamField body="plugins" type="PluginSet" required>

The plugins.

</ParamField>

<ResponseField name="return" type="Result<Browser, SeleniumBaseError>">

The attached browser. What `Browser::connect` returns, and
`SeleniumBaseError::Plugin` when a plugin refuses or `on_browser` fails.

</ResponseField>

Attaches to a running browser, as `Browser::connect` does, and runs the
plugins' hooks. The set (with fresh per-launch instances) is checked as
attaching (`connecting: true`, so a `LAUNCH_ONLY` plugin refuses), against the
browser's headless state read from its user agent; then `on_browser` runs, and
every tab the library attaches to gets the packages and `on_page_created`.
Nothing is launched, so `before_launch` does not run.

For URL-based attachment with a fingerprint, use the existing launch builder
with `.cdp_url(endpoint).fingerprint(&fingerprint).plugins(plugins)` and call
`Browser::launch`. The host performs the same attach checks, prepares existing
windows, and runs the usual page and close hooks. `BrowserConfig::with_plugins`
also carries its set into a direct CDP session through `browser_config`.
See [Tauri CDP attachment](/ecosystem/tauri-api/cdp).

## BrowserConfig::with_plugins

<Badge color="green">fn</Badge> `seleniumbase_rs::BrowserConfig::with_plugins`

```rust
#[cfg(feature = "plugins")]
pub fn with_plugins(mut self, plugins: seleniumbase_rs_plugin::PluginSet) -> Self
```

<ParamField body="plugins" type="PluginSet" required>

The plugins whose hooks run for every session started with this
configuration. It replaces the set the configuration holds, named plugins
included, so call it before `with_plugins_named` and `with_plugin_selection`.

</ParamField>

<ResponseField name="return" type="BrowserConfig">

The configuration.

</ResponseField>

`BrowserSession::connect`, and so `BaseCase::new`, checks the set against the
browser, lets `before_launch` change the start-up settings before the browser
starts, then calls `on_browser` and gives the first window the packages and
`on_page_created`; quitting calls `on_close`. A refusal or a failing hook stops
the session with `SeleniumBaseError::Plugin`. Plugins cannot be combined with
`reuse_session` (`SeleniumBaseError::InvalidConfig`), and do not run with
Internet Explorer. What each browser accepts is in
[The WebDriver engine](/tutorials/plugins#the-webdriver-engine).

## BrowserConfig::with_plugins_named

<Badge color="green">fn</Badge> `seleniumbase_rs::BrowserConfig::with_plugins_named`

```rust
#[cfg(feature = "plugins")]
pub fn with_plugins_named<I, S>(self, names: I) -> Result<Self, SeleniumBaseError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
```

<ParamField body="names" type="impl IntoIterator<Item = impl Into<String>>" required>

Names of built-in plugins, each with its default options.

</ParamField>

<ResponseField name="return" type="Result<BrowserConfig, SeleniumBaseError>">

The configuration with the plugins added after those of `with_plugins`, or
`SeleniumBaseError::Plugin` for a name this build does not have (the message
lists the ones it has) or a plugin already in the set.

</ResponseField>

Unlike the CDP builder, a configuration has no `build`, so the names are
resolved at once.

## BrowserConfig::with_plugin_selection

<Badge color="green">fn</Badge> `seleniumbase_rs::BrowserConfig::with_plugin_selection`

```rust
#[cfg(feature = "plugins")]
pub fn with_plugin_selection(
    mut self,
    selection: &PluginSelection,
) -> Result<Self, SeleniumBaseError>
```

<ParamField body="selection" type="&PluginSelection" required>

Plugin names and options, such as `Settings::plugins`.

</ParamField>

<ResponseField name="return" type="Result<BrowserConfig, SeleniumBaseError>">

The configuration with the plugins added after those it holds, or
`SeleniumBaseError::Plugin` for an unknown name, options a plugin refuses,
options for a plugin that is not enabled, or a plugin already in the set.

</ResponseField>

## BrowserConfig::plugins

<Badge color="green">fn</Badge> `seleniumbase_rs::BrowserConfig::plugins`

```rust
#[cfg(feature = "plugins")]
#[must_use]
pub fn plugins(&self) -> &seleniumbase_rs_plugin::PluginSet
```

<ResponseField name="return" type="&PluginSet">

The plugins of this configuration, named ones included.

</ResponseField>

## plugin_host::builtin_registry

<Badge color="green">fn</Badge> `seleniumbase_rs::plugin_host::builtin_registry`

```rust
#[must_use]
pub fn builtin_registry() -> Registry
```

<ResponseField name="return" type="Registry">

The built-in plugins this build contains, by name: those whose cargo feature
is on, in the order `shield`, `block-resources`, `crx`. Empty when none is on.

</ResponseField>

The names that `plugins_named`, `plugin_selection`, `with_plugins_named`,
`with_plugin_selection`, the settings file, `SB_PLUGINS` and `sbrs --plugin`
accept come from this registry only. Add your own with
`builtin_registry().with(Descriptor::of::<P>())` and build with `build_with`.

## plugin_host::build

<Badge color="green">fn</Badge> `seleniumbase_rs::plugin_host::build`

```rust
pub fn build(selection: &PluginSelection) -> Result<PluginSet, SeleniumBaseError>
```

<ParamField body="selection" type="&PluginSelection" required>

The plugins to turn on, with their options.

</ParamField>

<ResponseField name="return" type="Result<PluginSet, SeleniumBaseError>">

`build_with(&builtin_registry(), selection)`.

</ResponseField>

## plugin_host::build_with

<Badge color="green">fn</Badge> `seleniumbase_rs::plugin_host::build_with`

```rust
pub fn build_with(
    registry: &Registry,
    selection: &PluginSelection,
) -> Result<PluginSet, SeleniumBaseError>
```

<ParamField body="registry" type="&Registry" required>

The plugins that can be named, such as `builtin_registry()` plus your own.

</ParamField>

<ParamField body="selection" type="&PluginSelection" required>

The plugins to turn on, in order, with their options.

</ParamField>

<ResponseField name="return" type="Result<PluginSet, SeleniumBaseError>">

A strict set of the named plugins, in the selection's order; an empty strict
set when the selection names nothing.

</ResponseField>

Each plugin gets the default grants of `PluginSet::with_shared`: what it
declares, except Critical capabilities. Nothing is silently ignored; each of
these is `SeleniumBaseError::Plugin` naming the plugin, before any browser
starts:

| Mistake | Reason |
|---|---|
| Options for a name that is not in `enable` | ``options were given for `NAME`, which is not in `enable` ...`` |
| A name the registry does not have (every name is looked up before any plugin is built) | ``no plugin named `NAME` is available (known: ...)``, with the cargo features that add the official ones, or `none: this build has no built-in plugin` |
| Options the plugin refuses | `its options were refused: MESSAGE` |
| A name enabled twice | ``a plugin named `NAME` is already in the set`` |

## Request guard

<Badge color="gray">host behaviour</Badge> `src/plugin_host/guard.rs`

What both engines do with the raw protocol commands plugins send through
`BrowserHandle::send` and `PageHandle::send`. The functions are private to
seleniumbase-rs; their effect is part of the API a plugin sees.

<AccordionGroup>

<Accordion title="Request interception is refused" icon="ban" defaultOpen>

Every `Fetch.*` command and the deprecated `Network.setRequestInterception`
are refused, on the browser and on a tab, with `PluginError::Other`: the host
owns request interception (one `Fetch.enable` per tab, shared with the
library's proxy login). A plugin that wants to stop requests sends
`Network.setBlockedURLs`. Other domains named like it (`FetchLike.enable`,
`fetch.enable`) are not matched.

</Accordion>

<Accordion title="Blocklists are merged" icon="filter">

`Network.setBlockedURLs` replaces a tab's blocklist, so two plugins that each
sent theirs would undo each other. On a tab, the host keeps the union of
everything sent to it, seeded with what the library blocks itself (its ad-block
list and the tracker hosts of a fingerprint), and sends the union: each
pattern once, in the order it first arrived. With chromedriver, a window keeps
its union when the session switches away and back. A command whose `urls` is
missing or holds a non-string, or that uses `urlPatterns`, is refused and adds
nothing.

</Accordion>

<Accordion title="Everything else is passed on" icon="arrow-right">

Other commands go to the browser as sent. A plugin that reaches `Fetch` by
wrapping it in `Target.sendMessageToTarget` is not stopped yet; see
[Limits](/plugins/architecture#limits).

</Accordion>

</AccordionGroup>

## SeleniumBaseError::Plugin

<Badge color="orange">enum variant</Badge> `seleniumbase_rs::SeleniumBaseError::Plugin`

```rust
#[error("plugin error: {reason}")]
Plugin {
    plugin: String,
    reason: String,
},
```

A plugin refused to go on, or one of its hooks failed, so the launch or the tab
it was preparing was stopped. The launch builders, both engines and
`plugin_host` report every plugin refusal, hook failure and naming mistake
as this variant; a value a plugin set that the engine cannot use (a proxy that
does not parse, a WebRTC policy it does not know) is
`SeleniumBaseError::InvalidConfig`.

<ParamField body="plugin" type="String">

The plugin's name, or empty when the error does not name one.

</ParamField>

<ParamField body="reason" type="String">

What went wrong, as the plugin set reported it.

</ParamField>

`SeleniumBaseError::category()` is `plugin`, `exit_code()` is `4`, and
`hint()` says plugins fail closed and what to do for each kind of refusal.
`impl From<PluginError> for SeleniumBaseError` (feature `plugins`) makes
`reason` the `PluginError`'s text and fills `plugin` from it:

| `PluginError` | `plugin` |
|---|---|
| `DuplicateName`, `Undeclared` | the name |
| `MissingDependency`, `Requirement`, `Hook`, `Unsupported`, `Capability` | `plugin` |
| `Conflict` | `a`, the plugin that declares the conflict |
| `Sealed` | `b`, the plugin that tried to change the field |
| `Cycle`, `Config`, `Other`, and variants added later | empty |

The command-line helpers report a mistake the person typed as
`SeleniumBaseError::InvalidConfig` instead, with exit status 2
([Selection](/plugins/api/selection)).

## SeleniumBaseError::plugin

<Badge color="green">fn</Badge> `seleniumbase_rs::SeleniumBaseError::plugin`

```rust
pub fn plugin(plugin: impl Into<String>, reason: impl Into<String>) -> Self
```

<ParamField body="plugin" type="impl Into<String>" required>

The plugin's name; empty when unknown.

</ParamField>

<ParamField body="reason" type="impl Into<String>" required>

What went wrong.

</ParamField>

<ResponseField name="return" type="SeleniumBaseError">

`SeleniumBaseError::Plugin { plugin, reason }`.

</ResponseField>
