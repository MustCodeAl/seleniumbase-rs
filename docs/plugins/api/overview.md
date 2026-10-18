---
title: "Plugin API Reference"
sidebarTitle: "Overview"
description: "Every public item of seleniumbase-rs-plugin, the browser-plugin API of seleniumbase-rs, its testing crate and the host side in seleniumbase-rs, with exact signatures and examples."
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugins</Badge> <Badge color="gray" stroke>seleniumbase-rs-plugin 0.1.2</Badge>

The browser-plugin API of seleniumbase-rs is the crate `seleniumbase-rs-plugin`
in the [seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)
repository. A plugin is a type that implements its `Plugin` trait; a
`PluginSet` holds the plugins a program uses, checks them and calls their
hooks; seleniumbase-rs calls the set from both of its engines. These pages
document every public item at the revision seleniumbase-rs pins,
`628991af0e6c5711298e3bd3692dc2974887401a`, as the code defines it. For a
tutorial, read [Writing your own](/plugins/writing-your-own); for the design,
[Plugin architecture](/plugins/architecture).

<RequestExample>

```rust A plugin
use seleniumbase_rs_plugin::{
    async_trait, Capability, Category, HostScope, Metadata, PageHandle, Plugin, Result,
};

/// Logs a line in the console of every page.
pub struct Hello;

#[async_trait]
impl Plugin for Hello {
    fn name(&self) -> &str {
        "hello"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Testing)
            .version(env!("CARGO_PKG_VERSION"))
            .summary("Logs a line in the console of every page")
            .capability(Capability::PageScript(HostScope::Everywhere))
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
        page.add_init_script("console.log('hello from a plugin')").await
    }
}
```

```rust Test it
use seleniumbase_rs_plugin::PluginSet;
use seleniumbase_rs_plugin_testing::{block_on, Call, RecordingPage};

#[test]
fn every_page_says_hello() {
    let page = RecordingPage::new();
    let set = PluginSet::strict().with(Hello).unwrap();
    block_on(set.on_page_created(&page)).unwrap();
    assert_eq!(
        page.calls(),
        [Call::InitScript("console.log('hello from a plugin')".into())]
    );
}
```

```rust Pure CDP engine
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

let plugins = PluginSet::new().with(Hello)?;
let options = LaunchOptions::builder().headless(true).plugins(plugins).build()?;
let browser = Browser::launch(options).await?;
browser.close().await?;
```

```rust WebDriver engine
use seleniumbase_rs::plugin_api::PluginSet;
use seleniumbase_rs::{BaseCase, BrowserConfig};

let plugins = PluginSet::new().with(Hello)?;
let mut sb = BaseCase::new(BrowserConfig::default().with_plugins(plugins)).await?;
sb.open("https://example.com").await?;
sb.quit().await?;
```

</RequestExample>

<Info>

**Quick reference**

- API crate: `seleniumbase-rs-plugin` (Rust 2024, MSRV 1.88, MIT); from
  seleniumbase-rs it is `seleniumbase_rs::plugin_api`, feature `plugins`.
- Required: `fn name(&self) -> &str`. Hooks: `before_launch`, `on_browser`,
  `on_page_created`, `on_close`.
- By name: `Configurable`, `Descriptor`, `PluginSpec`, `Registry`.
- Tests: `seleniumbase-rs-plugin-testing` (`RecordingPage`, `RecordingBrowser`,
  `Call`, `block_on`).
- Pin: every dependency on the plugins repository at
  `rev = "628991af0e6c5711298e3bd3692dc2974887401a"`.

</Info>

<Visibility for="agents">

Summary for AI agents. Crate `seleniumbase-rs-plugin` (git https://github.com/MustCodeAl/seleniumbase-rs-plugins, rev 628991af0e6c5711298e3bd3692dc2974887401a), re-exported by seleniumbase-rs as `seleniumbase_rs::plugin_api` (feature `plugins`). Items: `Plugin` (trait; only `name` required; hooks `before_launch(&mut LaunchSettings)`, `on_browser(&dyn BrowserHandle)`, `on_page_created(&dyn PageHandle)`, `on_close()`; declarations `dependencies`, `requirements`, `metadata`, `per_launch`, `packages`), `Result<T>`, `PluginError`, `PluginSet`, `AuditRow`, `LaunchContext`, `LaunchSettings`, `ProfileView`, `Engine`, `BrowserFamily`, `WebRtcPolicy`, `BrowserHandle`, `PageHandle`, `Requirements`, `Package`, `Configurable`, `Descriptor`, `PluginSpec`, `Registry`, `Metadata`, `Category`, `Phase`, `Seals`, `Budgets`, `Capability`, `CapabilitySet`, `HostScope`, `NetScope`, `PathScope`, `RawKind`, `Risk`, `Enforcement`, `ParseCapabilityError`, `HostInfo`, `HostKind`, module `switch_policy`, and the `async_trait` macro. Test crate `seleniumbase-rs-plugin-testing`: `RecordingPage`, `RecordingBrowser`, `Call`, `block_on`. Host side in seleniumbase-rs: `LaunchOptionsBuilder::plugins`/`plugins_named`/`plugin_selection`, `BrowserConfig::with_plugins`/`with_plugins_named`/`with_plugin_selection`, `Browser::connect_with_plugins`, `plugin_host::builtin_registry`/`build`/`build_with`, `config::plugins::PluginSelection`, `PLUGINS_ENV` (`SB_PLUGINS`), `cli::plugin_cmd`, `SeleniumBaseError::Plugin`.

</Visibility>

## Depend on it

A plugin crate depends on the API crate only, never on seleniumbase-rs, so it
keeps working as seleniumbase-rs changes. A program that runs plugins depends
on seleniumbase-rs with the feature `plugins` and on the plugin crates it uses.

<CodeGroup>

```toml A plugin crate
[dependencies]
seleniumbase-rs-plugin = { git = "https://github.com/MustCodeAl/seleniumbase-rs-plugins", rev = "628991af0e6c5711298e3bd3692dc2974887401a" }
serde_json = "1"

[dev-dependencies]
seleniumbase-rs-plugin-testing = { git = "https://github.com/MustCodeAl/seleniumbase-rs-plugins", rev = "628991af0e6c5711298e3bd3692dc2974887401a" }
```

```toml A program
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["plugins"] }
seleniumbase-rs-plugin-hello = { path = "../seleniumbase-rs-plugin-hello" }
```

```bash cargo add
cargo add seleniumbase-rs-plugin --git https://github.com/MustCodeAl/seleniumbase-rs-plugins --rev 628991af0e6c5711298e3bd3692dc2974887401a
cargo add --dev seleniumbase-rs-plugin-testing --git https://github.com/MustCodeAl/seleniumbase-rs-plugins --rev 628991af0e6c5711298e3bd3692dc2974887401a
```

</CodeGroup>

<Warning>

seleniumbase-rs takes `seleniumbase-rs-plugin` and
`seleniumbase-rs-plugins-builtin` from one git URL at one `rev`,
`628991af0e6c5711298e3bd3692dc2974887401a`. Cargo counts the same repository
under another reference (a branch, a tag, another `rev`) as another source, so a
plugin crate pinned differently builds a second copy of the API, and its
`Plugin` is not the trait seleniumbase-rs calls. See
[Cargo features](/plugins/selecting#cargo-features).

</Warning>

## Two paths to the same items

| In | Write | Needs |
|---|---|---|
| A plugin crate | `use seleniumbase_rs_plugin::...` | the API crate |
| A program that uses seleniumbase-rs | `use seleniumbase_rs::plugin_api::...` | seleniumbase-rs with the feature `plugins` |

`seleniumbase_rs::plugin_api` is a re-export of the whole crate
(`pub use seleniumbase_rs_plugin as plugin_api`), so the two paths name the same
types. It is not the module `seleniumbase_rs::plugins`, which holds the
test-reporting plugins.

## Stability

The API crate is on 0.1.x and changes by addition. Items marked
`#[non_exhaustive]` can grow:

- enums `PluginError`, `Engine`, `BrowserFamily`, `ProfileView`, `WebRtcPolicy`,
  `Category`, `Phase`, `Capability` and `HostKind`: a `match` on one needs a
  wildcard arm;
- structs `LaunchSettings`, `Metadata`, `Budgets` and `AuditRow`: their public
  fields can be read and assigned, but code outside the crate cannot build one
  with a struct literal. Start from `LaunchSettings::default()`,
  `Metadata::new`, `Budgets::default()`.

## Pages

<Columns cols={2}>

<Card title="The Plugin trait" icon="plug" href="/plugins/api/plugin-trait">

`Plugin` and each of its hooks, `Result`, `PluginError` and `async_trait`.

</Card>

<Card title="Launch settings" icon="rocket" href="/plugins/api/launch">

`LaunchContext`, `LaunchSettings`, `ProfileView`, `Engine`, `BrowserFamily`, `WebRtcPolicy`.

</Card>

<Card title="Handles" icon="window-maximize" href="/plugins/api/handles">

`BrowserHandle` and `PageHandle`, and what each engine does behind them.

</Card>

<Card title="PluginSet" icon="layer-group" href="/plugins/api/plugin-set">

Building a set, grants and trust, the check, the audit, the order, `AuditRow`.

</Card>

<Card title="Configuration" icon="sliders" href="/plugins/api/configuration">

`Configurable`, `Descriptor`, `PluginSpec` and `Registry`: plugins built from a name and options.

</Card>

<Card title="Metadata" icon="id-card" href="/plugins/api/metadata">

The `Metadata` builder, `Category`, `Phase`, `Seals` and `Budgets`.

</Card>

<Card title="Capabilities" icon="key" href="/plugins/api/capabilities">

`Capability`, `CapabilitySet`, scopes, risk, `HostInfo`, refused switches and features.

</Card>

<Card title="Requirements and packages" icon="list-check" href="/plugins/api/requirements">

`Requirements` and `Package`: what a plugin needs from the browser, and scripts and styles for pages.

</Card>

<Card title="The seleniumbase-rs host" icon="server" href="/plugins/api/host">

Cargo features, the launch builders, `plugin_host`, the request guard and `SeleniumBaseError::Plugin`.

</Card>

<Card title="Selection by name" icon="toggle-on" href="/plugins/api/selection">

`PluginSelection`, `SB_PLUGINS`, the settings loaders and `cli::plugin_cmd`.

</Card>

<Card title="Testing" icon="flask" href="/plugins/api/testing">

`RecordingPage`, `RecordingBrowser`, `Call` and `block_on`.

</Card>

</Columns>

## Module map

The API crate, `crates/seleniumbase-rs-plugin/src/` in the plugins repository:

| File | Public items | Page |
|---|---|---|
| `lib.rs` | `Plugin`, `Result`, `PluginError`, `async_trait` | [The Plugin trait](/plugins/api/plugin-trait) |
| `lib.rs` | `LaunchContext`, `LaunchSettings`, `ProfileView`, `Engine`, `BrowserFamily`, `WebRtcPolicy` | [Launch settings](/plugins/api/launch) |
| `lib.rs` | `BrowserHandle`, `PageHandle` | [Handles](/plugins/api/handles) |
| `lib.rs` | `PluginSet`, `AuditRow` | [PluginSet](/plugins/api/plugin-set) |
| `lib.rs` | `Requirements`, `Package` | [Requirements and packages](/plugins/api/requirements) |
| `config.rs` | `Configurable`, `Descriptor`, `PluginSpec`, `Registry` | [Configuration](/plugins/api/configuration) |
| `metadata.rs` | `Metadata`, `Category`, `Phase`, `Seals`, `Budgets` | [Metadata](/plugins/api/metadata) |
| `capability.rs` | `Capability`, `CapabilitySet`, `HostScope`, `NetScope`, `PathScope`, `RawKind`, `Risk`, `Enforcement`, `ParseCapabilityError` | [Capabilities](/plugins/api/capabilities) |
| `host.rs` | `HostInfo`, `HostKind` | [Capabilities](/plugins/api/capabilities#hostinfo) |
| `switch_policy.rs` | module `switch_policy`: `REFUSED_SWITCHES`, `REFUSED_FEATURES`, `switch_name`, `refusal`, `feature_refused` | [Capabilities](/plugins/api/capabilities#switch_policy) |
| `order.rs` | nothing public: the resolver behind the run order | [PluginSet](/plugins/api/plugin-set#run-order) |

The testing crate, `crates/seleniumbase-rs-plugin-testing/src/lib.rs`:
`RecordingPage`, `RecordingBrowser`, `Call`, `block_on`
([Testing](/plugins/api/testing)).

The host side, in seleniumbase-rs:

| Path | Public items | Page |
|---|---|---|
| `src/lib.rs` | `plugin_api`, `builtin_plugins` (re-exports) | [Host](/plugins/api/host) |
| `src/sb_cdp/launch.rs` | `LaunchOptionsBuilder::plugins`, `plugins_named`, `plugin_selection` | [Host](/plugins/api/host) |
| `src/sb_cdp/browser.rs` | `Browser::connect_with_plugins` | [Host](/plugins/api/host) |
| `src/browser/config.rs` | `BrowserConfig::with_plugins`, `with_plugins_named`, `with_plugin_selection`, `plugins` | [Host](/plugins/api/host) |
| `src/plugin_host/` | `builtin_registry`, `build`, `build_with`; the request guard | [Host](/plugins/api/host) |
| `src/error.rs` | `SeleniumBaseError::Plugin` | [Host](/plugins/api/host) |
| `src/config/plugins.rs` | `PluginSelection`, `PLUGINS_ENV` | [Selection](/plugins/api/selection) |
| `src/config/settings.rs` | `Settings::load_plugin_selection`, `load_plugin_table`, `Settings::plugins` | [Selection](/plugins/api/selection) |
| `src/cli/plugin_cmd.rs` | `PluginChoice`, `PluginSource`, `PluginOrigin`, `check_selection`, `check_selection_with`, `check_origins_with`, `apply_to_config`, `list`, `list_with`, `info`, `info_with` | [Selection](/plugins/api/selection) |

## Guides

<Columns cols={2}>

<Card title="Plugins" icon="puzzle-piece" href="/plugins/overview">

The official plugins and how to install them.

</Card>

<Card title="Writing your own" icon="pen" href="/plugins/writing-your-own">

A plugin crate from an empty folder to a tested plugin turned on by name.

</Card>

<Card title="Plugin architecture" icon="sitemap" href="/plugins/architecture">

Categories, capabilities, ordering, and what the host enforces today.

</Card>

<Card title="Choosing plugins" icon="list" href="/plugins/selecting">

Cargo features, `--plugin`, the `[plugins]` table and `SB_PLUGINS`.

</Card>

<Card title="Browser Plugins guide" icon="book" href="/tutorials/plugins">

When each hook runs on each engine, and what happens when a plugin fails.

</Card>

</Columns>
