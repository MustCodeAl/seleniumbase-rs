---
title: "The Plugin Trait"
sidebarTitle: "Plugin trait"
description: "Reference for the Plugin trait of seleniumbase-rs-plugin: name, dependencies, requirements, metadata, per_launch, packages, the four hooks, the Result alias, PluginError and the async_trait macro."
---

A plugin is a type that implements `Plugin`. `name` is the only method without
a default; every hook does nothing unless you override it. Items are in
`seleniumbase_rs_plugin` (`seleniumbase_rs::plugin_api` from seleniumbase-rs),
defined in `crates/seleniumbase-rs-plugin/src/lib.rs`. A walk-through is in
[Writing your own](/plugins/writing-your-own); when each hook runs on each
engine is in [When the hooks run](/tutorials/plugins#when-the-hooks-run).

<RequestExample>

```rust A plugin with hooks
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use seleniumbase_rs_plugin::{
    async_trait, Capability, Category, LaunchSettings, Metadata, PageHandle, Plugin,
    Requirements, Result,
};
use serde_json::json;

/// German pages in Berlin time; counts the tabs it prepared.
#[derive(Default)]
pub struct Berlin {
    tabs: AtomicUsize,
}

#[async_trait]
impl Plugin for Berlin {
    fn name(&self) -> &str {
        "berlin"
    }

    fn requirements(&self) -> Requirements {
        Requirements::CHROMIUM_ONLY
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Environment)
            .version(env!("CARGO_PKG_VERSION"))
            .summary("German pages in Berlin time")
            .capability(Capability::LaunchArgs)
            .capability(Capability::Cdp("Emulation".into()))
    }

    /// A fresh counter for each browser of a pool.
    fn per_launch(&self) -> Option<Arc<dyn Plugin>> {
        Some(Arc::new(Self::default()))
    }

    async fn before_launch(&self, launch: &mut LaunchSettings) -> Result<()> {
        launch.args.push("--lang=de".into());
        Ok(())
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
        page.send(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": "Europe/Berlin" }),
        )
        .await?;
        self.tabs.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
```

```rust Fail a hook
use seleniumbase_rs_plugin::{LaunchSettings, PluginError, ProfileView, Result};

// In `impl Plugin for NoProfile`:
async fn before_launch(&self, launch: &mut LaunchSettings) -> Result<()> {
    if let ProfileView::Persistent(folder) = launch.profile() {
        return Err(PluginError::other(format!(
            "refusing the persistent profile {}",
            folder.display()
        )));
    }
    Ok(())
}
```

```rust Read an error
use seleniumbase_rs_plugin::PluginError;

match set.check(&context) {
    Ok(()) => {}
    Err(PluginError::Requirement { plugin, reason }) => eprintln!("{plugin}: {reason}"),
    Err(PluginError::MissingDependency { plugin, missing }) => {
        eprintln!("add the crate that provides `{missing}`, which `{plugin}` needs");
    }
    // PluginError is #[non_exhaustive]: keep a wildcard arm.
    Err(other) => return Err(other),
}
```

</RequestExample>

<ResponseExample>

```text Hook
plugin `berlin` failed in `on_page_created`: Emulation.setTimezoneOverride failed (fake)
```

```text Requirement
plugin `berlin` cannot run here: it needs a Chromium-based browser
```

```text MissingDependency
plugin `captcha` needs plugin `stealth`: add the crate that provides it to the set
```

</ResponseExample>

## Plugin

<Badge color="purple">trait</Badge> `seleniumbase_rs_plugin::Plugin`

```rust
#[async_trait]
pub trait Plugin: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn dependencies(&self) -> &[&str] { &[] }
    fn requirements(&self) -> Requirements { Requirements::empty() }
    fn metadata(&self) -> Metadata { Metadata::legacy() }
    fn per_launch(&self) -> Option<Arc<dyn Plugin>> { None }
    fn packages(&self) -> Vec<Package> { Vec::new() }
    async fn before_launch(&self, _launch: &mut LaunchSettings) -> Result<()> { Ok(()) }
    async fn on_browser(&self, _browser: &dyn BrowserHandle) -> Result<()> { Ok(()) }
    async fn on_page_created(&self, _page: &dyn PageHandle) -> Result<()> { Ok(()) }
    async fn on_close(&self) -> Result<()> { Ok(()) }
}
```

The trait is object safe through `#[async_trait]`: a set keeps plugins as
`Arc<dyn Plugin>`. Write `#[async_trait]` on every `impl Plugin` too. The
supertraits `Send + Sync + 'static` let one plugin serve several browsers and
tasks at once, so keep mutable state behind atomics or a `Mutex`.

| Method | Kind | Default | Called by |
|---|---|---|---|
| `name` | declaration | none, required | every `PluginSet` method |
| `dependencies` | declaration | `&[]` | the [run order](/plugins/api/plugin-set#run-order) and the check |
| `requirements` | declaration | `Requirements::empty()` | `PluginSet::check_with`, the run order |
| `metadata` | declaration | `Metadata::legacy()` | the check, the run order, grants, `audit` |
| `per_launch` | declaration | `None` | `PluginSet::for_launch` |
| `packages` | declaration | no packages | the check, `PluginSet::on_page_created` |
| `before_launch` | hook | does nothing | `PluginSet::before_launch` |
| `on_browser` | hook | does nothing | `PluginSet::on_browser` |
| `on_page_created` | hook | does nothing | `PluginSet::on_page_created` |
| `on_close` | hook | does nothing | `PluginSet::on_close` |

<Note>

The set calls the declarations again whenever it orders, checks or audits the
plugins, so return the same values every time and keep them cheap.

</Note>

## Plugin::name

<Badge color="green">fn</Badge> <Badge color="red" stroke>required</Badge>

```rust
fn name(&self) -> &str;
```

<ResponseField name="return" type="&str" required>

The plugin's unique, lower-case name, such as `block-resources`.

</ResponseField>

A set refuses a second plugin with the same name
(`PluginError::DuplicateName`). Other plugins refer to this name in
`dependencies`, `requires`, `conflicts_with`, `after` and `before`. A plugin
that is also [`Configurable`](/plugins/api/configuration#configurable) returns
`Self::NAME`, the name a registry builds it by.

## Plugin::dependencies

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
fn dependencies(&self) -> &[&str] {
    &[]
}
```

<ResponseField name="return" type="&[&str]" default="&[]">

The names of the plugins this one needs in the same set.

</ResponseField>

Each name joins the plugin's `requires` edges from
[`Metadata::requires`](/plugins/api/metadata): the named
plugin must be in the set, or the check fails with
`PluginError::MissingDependency`, and it runs before this one.

## Plugin::requirements

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
fn requirements(&self) -> Requirements {
    Requirements::empty()
}
```

<ResponseField name="return" type="Requirements" default="Requirements::empty()">

Conditions the plugin needs, combined with `|`: `CHROMIUM_ONLY`,
`LAUNCH_ONLY`, `HEADFUL`, `RUN_LAST`. See
[Requirements](/plugins/api/requirements#requirements).

</ResponseField>

The check refuses a browser that does not fit with
`PluginError::Requirement`. `RUN_LAST` refuses nothing; it puts the plugin in
the phase `Last`.

## Plugin::metadata

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
fn metadata(&self) -> Metadata {
    Metadata::legacy()
}
```

<ResponseField name="return" type="Metadata" default="Metadata::legacy()">

What the plugin declares: category, version, summary, capabilities, ordering
edges, phase, seals and budgets. See [Metadata](/plugins/api/metadata).

</ResponseField>

The default, `Metadata::legacy()`, has no category and the legacy capability
set (`raw:cdp` among them). Such a plugin is "undeclared": a plain set runs it,
a strict set refuses it with `PluginError::Undeclared`. The command line, the
MCP server and every set built from names are strict.

## Plugin::per_launch

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
fn per_launch(&self) -> Option<Arc<dyn Plugin>> {
    None
}
```

<ResponseField name="return" type="Option<Arc<dyn Plugin>>" default="None">

A fresh instance for one browser, or `None` to share this one.

</ResponseField>

[`PluginSet::for_launch`](/plugins/api/plugin-set)
replaces the plugin with the instance it returns, keeping its grants, so
browsers launched from one set (a pool) do not share the plugin's state and
`on_close` reaches the instance that started. Both engines call `for_launch`
before every launch and every attach. The fresh instance must have the same
`name`; one with another name is ignored and the shared instance is kept.

## Plugin::packages

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
fn packages(&self) -> Vec<Package> {
    Vec::new()
}
```

<ResponseField name="return" type="Vec<Package>" default="empty">

JavaScript and CSS to load into every new tab. See
[Package](/plugins/api/requirements#package).

</ResponseField>

Every package must say where it loads (`only_on` or `everywhere`), or the check
refuses the plugin with `PluginError::Requirement` (``one of its packages says
neither `only_on` nor `everywhere` ``). For each new tab, the set adds the plugin's
packages with `PageHandle::add_init_script`, then calls its `on_page_created`.
A package that fails is reported as `PluginError::Hook` with the hook
`packages`.

## Plugin::before_launch

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
async fn before_launch(&self, _launch: &mut LaunchSettings) -> Result<()> {
    Ok(())
}
```

<ParamField body="launch" type="&mut LaunchSettings" required>

The start-up settings of the browser about to start, as the host filled them
in and earlier plugins changed them. See
[LaunchSettings](/plugins/api/launch#launchsettings).

</ParamField>

<ResponseField name="return" type="Result<()>">

`Ok(())` to go on; an error stops the launch.

</ResponseField>

Runs before the browser starts, in run order. It does not run when the program
attaches to a running browser. After each plugin the set checks what it
changed: each change needs its capability, list fields only grow, a refused
switch or feature needs `raw:switches`, the profile is read-only, and a field
another plugin sealed keeps its value. The rules are under
[PluginSet::before_launch](/plugins/api/plugin-set).

## Plugin::on_browser

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
async fn on_browser(&self, _browser: &dyn BrowserHandle) -> Result<()> {
    Ok(())
}
```

<ParamField body="browser" type="&dyn BrowserHandle" required>

The running browser: its engine, its family and browser-level DevTools
commands. See [BrowserHandle](/plugins/api/handles#browserhandle).

</ParamField>

<ResponseField name="return" type="Result<()>">

`Ok(())` to go on; an error stops the launch or the attach.

</ResponseField>

Runs once the browser runs, whether the program started it or attached to it.

## Plugin::on_page_created

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
async fn on_page_created(&self, _page: &dyn PageHandle) -> Result<()> {
    Ok(())
}
```

<ParamField body="page" type="&dyn PageHandle" required>

The new tab: its engine, DevTools commands to it, scripts for every document
and evaluation. See [PageHandle](/plugins/api/handles#pagehandle).

</ParamField>

<ResponseField name="return" type="Result<()>">

`Ok(())` to go on; an error stops the tab.

</ResponseField>

Runs for every tab the library attaches to, before any command of the program
runs in it. The plugin's own packages are already added.

## Plugin::on_close

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
async fn on_close(&self) -> Result<()> {
    Ok(())
}
```

<ResponseField name="return" type="Result<()>">

An error is collected, not raised: the browser closes anyway.

</ResponseField>

Runs when the browser is about to close. Every plugin runs even when one fails;
[`PluginSet::on_close`](/plugins/api/plugin-set) returns
the failures, and both engines log each one at `WARN`
(`cdp.plugin.close.failed`, `webdriver.plugin.close.failed`). Use it to clean
up, such as deleting temporary folders.

## Hooks added in 0.1.3

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

Every one has a default body, so a plugin written for 0.1.0 compiles and
behaves as before. A hook behind a capability reaches only plugins whose
`metadata()` holds it. When and where each runs, with examples, is in
[More hooks](/tutorials/plugins#more-hooks).

```rust
fn network(&self) -> NetworkInterest;                 // rules and filters; default: none
fn protocol_events(&self) -> &[&str];                 // DevTools events for on_protocol_event
fn task(&self) -> Option<&dyn Task>;                  // needs Capability::Task
fn job_runner(&self) -> Option<&dyn JobRunner>;

async fn before_context(&self, settings: &mut ContextSettings) -> Result<()>;      // Capability::Contexts
async fn on_context_created(&self, context: &dyn ContextHandle) -> Result<()>;     // Capability::Contexts
async fn on_context_closed(&self, id: &ContextId) -> Result<()>;                   // Capability::Contexts
async fn before_navigate(&self, page: &dyn PageHandle, navigation: &Navigation)
    -> Result<NavDecision>;                                                        // redirect: Capability::PageNavigate
async fn after_navigate(&self, page: &dyn PageHandle, outcome: &NavOutcome) -> Result<()>;
async fn on_request(&self, request: &RequestView) -> Result<RequestDecision>;     // Capability::NetIntercept
async fn on_response(&self, response: &ResponseView<'_>) -> Result<()>;           // Capability::NetObserve
async fn on_protocol_event(&self, browser: &dyn BrowserHandle, event: &ProtocolEvent)
    -> Result<()>;
async fn before_close(&self, browser: &dyn BrowserHandle) -> Result<()>;
async fn on_test_start(&self, test: &TestInfo) -> Result<()>;                      // Capability::TestHooks
async fn on_test_end(&self, test: &TestInfo, outcome: &TestOutcome, page: Option<&dyn PageHandle>)
    -> Result<TestVerdict>;                                                        // Capability::TestHooks
```

An error from `before_context`, `on_context_created` or `before_navigate` stops
what it guards, and one from `on_request` blocks the request; the others are
logged. A panic in any hook, old or new, becomes `PluginError::Panicked`, and a
set with a timer (`PluginSet::with_timer`) ends a hook that runs past its limit
with `PluginError::Timeout`. The full reference, with every type above, is the
crate's rustdoc (`cargo doc -p seleniumbase-rs-plugin --open`).


## Result

<Badge color="gray">type</Badge> `seleniumbase_rs_plugin::Result`

```rust
pub type Result<T> = std::result::Result<T, PluginError>;
```

The result of every hook and of the fallible `PluginSet` and `Registry`
methods. In a program that also uses `std::result::Result`, import it under
another name: `use seleniumbase_rs::plugin_api::Result as PluginResult;`.

## PluginError

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::PluginError`

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PluginError {
    DuplicateName(String),
    MissingDependency { plugin: String, missing: String },
    Requirement { plugin: String, reason: String },
    Hook { plugin: String, hook: &'static str, message: String },
    Cycle(Vec<String>),
    Conflict { a: String, b: String },
    Sealed { field: &'static str, a: String, b: String },
    Unsupported { plugin: String, capability: String, host: &'static str },
    Capability { plugin: String, capability: String, reason: String },
    Undeclared(String),
    Config(String),
    Other(String),
}
```

Why a plugin, or the set, refused to go on. It implements `std::error::Error`
and `Display`; seleniumbase-rs turns it into
[`SeleniumBaseError::Plugin`](/plugins/api/host)
with `From`. The `Display` text of each variant is shown below, with each
field in capitals.

<ResponseField name="PluginError" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="DuplicateName(String)" type="tuple variant">

Two plugins in a set have the same name. Raised by `PluginSet::with` and its
siblings. Text: ``a plugin named `NAME` is already in the set``.

</ResponseField>

<ResponseField name="MissingDependency" type="struct variant">

A plugin needs another one that is not in the set. Raised by the check.

<Expandable title="fields">

<ResponseField name="plugin" type="String">The plugin that has the dependency.</ResponseField>

<ResponseField name="missing" type="String">The name of the plugin it needs.</ResponseField>

</Expandable>

Text: ``plugin `PLUGIN` needs plugin `MISSING`: add the crate that provides it to the set``.

</ResponseField>

<ResponseField name="Requirement" type="struct variant">

A plugin cannot run with this browser or launch. Raised by the check, from
`requirements` and `packages`.

<Expandable title="fields">

<ResponseField name="plugin" type="String">The plugin that refused.</ResponseField>

<ResponseField name="reason" type="String">What it needs that is missing.</ResponseField>

</Expandable>

Text: ``plugin `PLUGIN` cannot run here: REASON``.

</ResponseField>

<ResponseField name="Hook" type="struct variant">

A hook failed. The set wraps any other error a hook returns in this variant;
an error that already is a `Hook` passes through unchanged.

<Expandable title="fields">

<ResponseField name="plugin" type="String">The plugin whose hook failed.</ResponseField>

<ResponseField name="hook" type="&'static str">`before_launch`, `on_browser`, `packages`, `on_page_created` or `on_close`.</ResponseField>

<ResponseField name="message" type="String">The `Display` text of the error the hook returned.</ResponseField>

</Expandable>

Text: ``plugin `PLUGIN` failed in `HOOK`: MESSAGE``.

</ResponseField>

<ResponseField name="Cycle(Vec<String>)" type="tuple variant">

The plugins' ordering edges contain a cycle; the names are those that could not
be ordered. Raised by the check. Text: `these plugins wait for each other: a, b`.

</ResponseField>

<ResponseField name="Conflict" type="struct variant">

Two plugins that cannot be combined are in the set. Raised by the check.

<Expandable title="fields">

<ResponseField name="a" type="String">The plugin that declares the conflict.</ResponseField>

<ResponseField name="b" type="String">The plugin it conflicts with.</ResponseField>

</Expandable>

Text: ``plugin `A` cannot be combined with plugin `B` ``.

</ResponseField>

<ResponseField name="Sealed" type="struct variant">

A sealed launch field was changed to a different value. Raised by
`PluginSet::before_launch`.

<Expandable title="fields">

<ResponseField name="field" type="&'static str">`proxy`, `webrtc_policy` or `headless`.</ResponseField>

<ResponseField name="a" type="String">The plugin that sealed it.</ResponseField>

<ResponseField name="b" type="String">The plugin that tried to change it.</ResponseField>

</Expandable>

Text: ``plugin `B` cannot change `FIELD`, which plugin `A` sealed``.

</ResponseField>

<ResponseField name="Unsupported" type="struct variant">

The host cannot provide a capability the plugin requires. Raised by
`PluginSet::check_with`.

<Expandable title="fields">

<ResponseField name="plugin" type="String">The plugin.</ResponseField>

<ResponseField name="capability" type="String">The capability, as its string (`cdp:Network`).</ResponseField>

<ResponseField name="host" type="&'static str">`HostInfo::name()`, such as `WebDriver with Firefox or Safari`.</ResponseField>

</Expandable>

Text: ``plugin `PLUGIN` needs `CAPABILITY`, which HOST does not support``.

</ResponseField>

<ResponseField name="Capability" type="struct variant">

A required capability was not granted, or `before_launch` broke a rule about
launch settings.

<Expandable title="fields">

<ResponseField name="plugin" type="String">The plugin.</ResponseField>

<ResponseField name="capability" type="String">The capability (`launch.proxy`), or the setting (`args`, `prefs`, `profile`, ...).</ResponseField>

<ResponseField name="reason" type="String">Why it was refused, such as `it is required but not granted`.</ResponseField>

</Expandable>

Text: ``plugin `PLUGIN` is not allowed to use `CAPABILITY`: REASON``.

</ResponseField>

<ResponseField name="Undeclared(String)" type="tuple variant">

A strict set refuses a plugin that declares no metadata. Raised by the check.
Text: ``plugin `NAME` declares no metadata, which a strict set requires``.

</ResponseField>

<ResponseField name="Config(String)" type="tuple variant">

A wrong plugin name or wrong options. Returned by `Configurable::configure`
and `Registry::build`. Text: `plugin configuration: MESSAGE`.

</ResponseField>

<ResponseField name="Other(String)" type="tuple variant">

Any other failure, with a message for the user. Made by
`PluginError::other` (below), and by the hosts' handles when a
command fails. Text: the message itself.

</ResponseField>

</Expandable>

</ResponseField>

## PluginError::other

<Badge color="green">fn</Badge> `seleniumbase_rs_plugin::PluginError::other`

```rust
pub fn other(message: impl Into<String>) -> Self
```

<ParamField body="message" type="impl Into<String>" required>

What went wrong, for the user.

</ParamField>

<ResponseField name="return" type="PluginError">

`PluginError::Other(message)`.

</ResponseField>

The usual way for a hook to fail. The set adds the plugin's name and the hook
when it wraps the error in `PluginError::Hook`.

## async_trait

<Badge color="yellow">macro</Badge> `seleniumbase_rs_plugin::async_trait`

```rust
pub use async_trait::async_trait;
```

The `async_trait` attribute macro of the crate `async-trait` (0.1), re-exported
so a plugin crate needs no dependency of its own for it. Put
`#[async_trait]` on every `impl Plugin`, `impl BrowserHandle` and
`impl PageHandle`.

## See also

<Columns cols={2}>

<Card title="PluginSet" icon="layer-group" href="/plugins/api/plugin-set">

How the set orders the plugins and calls these hooks.

</Card>

<Card title="Testing" icon="flask" href="/plugins/api/testing">

Run the hooks against recording fakes, without a browser.

</Card>

</Columns>
