---
title: "PluginSet"
sidebarTitle: "PluginSet"
description: "Reference for PluginSet, the plugins a program uses: building a set, default grants and trust, strict sets, the check against a browser and a host, the audit, the run order, the hook runners and the errors each one returns."
---

A `PluginSet` holds the plugins a program uses, with the capabilities each was
granted. It checks them against the browser about to start, orders them, and
calls their hooks; seleniumbase-rs calls the set, so a program usually only
builds one and hands it to a launch builder ([Host](/plugins/api/host)). It is
`seleniumbase_rs_plugin::PluginSet`, defined in
`crates/seleniumbase-rs-plugin/src/lib.rs`; the run order comes from
`order.rs`.

<RequestExample>

```rust Build and check
use seleniumbase_rs_plugin::{BrowserFamily, Engine, LaunchContext, PluginSet};

let set = PluginSet::new().with(Berlin::default())?.with(NoImages)?;
set.check(&LaunchContext {
    engine: Engine::Cdp,
    family: BrowserFamily::Chromium,
    headless: true,
    connecting: false,
})?;
// Berlin is an Environment plugin, so it runs first.
assert_eq!(set.names(), ["berlin", "no-images"]);
```

```rust Trust a Critical capability
use seleniumbase_rs_plugin::{Capability, PluginSet, RawKind};

// A plugin that requires raw:cdp fails the check when added with `with`.
let set = PluginSet::new().with_trusted(Inspector, &[Capability::Raw(RawKind::Cdp)])?;
```

```rust Exact grants
use seleniumbase_rs_plugin::{Capability, CapabilitySet, PluginSet};

let grants: CapabilitySet = ["launch.args", "cdp:Network"]
    .into_iter()
    .map(str::parse::<Capability>)
    .collect::<Result<_, _>>()?;
let set = PluginSet::strict().with_grants(NoImages, grants)?;
```

```rust Audit
for row in set.audit() {
    println!(
        "{:<12} {:<14} {:?} granted={} {:?}",
        row.plugin,
        row.capability.to_string(),
        row.risk,
        row.granted,
        row.enforcement
    );
}
```

```rust Run the hooks yourself
use seleniumbase_rs_plugin::LaunchSettings;
use seleniumbase_rs_plugin_testing::{block_on, RecordingBrowser, RecordingPage};

let set = set.for_launch();
let mut settings = LaunchSettings::default();
block_on(set.before_launch(&mut settings))?;
block_on(set.on_browser(&RecordingBrowser::new()))?;
block_on(set.on_page_created(&RecordingPage::new()))?;
for failure in block_on(set.on_close()) {
    eprintln!("{failure}");
}
```

</RequestExample>

<ResponseExample>

```text Audit
berlin       launch.args    Low granted=true Enforced
berlin       cdp:Emulation  Medium granted=true Enforced
no-images    cdp:Network    Medium granted=true Enforced
```

```text Not granted
plugin `inspector` is not allowed to use `raw:cdp`: it is required but not granted
```

```text Not supported
plugin `no-images` needs `cdp:Network`, which WebDriver with Firefox or Safari does not support
```

```text Strict set
plugin `old` declares no metadata, which a strict set requires
```

</ResponseExample>

## PluginSet

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::PluginSet`

```rust
#[derive(Clone, Default)]
pub struct PluginSet { /* private: the plugins with their grants, and strict */ }

impl fmt::Debug for PluginSet { /* prints the names in run order and `strict` */ }
```

The plugins a program uses, in the order their hooks run. An error from
`before_launch`, `on_browser` or `on_page_created` stops there and is returned:
a plugin that did not run must not leave a browser without what it adds.
Cloning a set clones `Arc`s: the plugins themselves are shared.

| Method | Does |
|---|---|
| `new`, `strict` | An empty set; a strict one refuses undeclared plugins |
| `with`, `with_shared` | Add a plugin with the default grants |
| `with_trusted` | Add a plugin with the default grants and named Critical ones |
| `with_grants`, `with_grants_shared` | Add a plugin with exactly these grants |
| `is_empty`, `names` | Read the set |
| `for_launch` | The set for one browser, with fresh per-launch instances |
| `check`, `check_with` | Check every plugin before anything starts |
| `audit` | Every declared capability with its risk and grant |
| `before_launch`, `on_browser`, `on_page_created`, `on_close` | Run a hook of every plugin, in order |

## PluginSet::new

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn new() -> Self
```

<ResponseField name="return" type="PluginSet">

An empty set, the same as `PluginSet::default()`. It runs plugins that declare
no metadata.

</ResponseField>

## PluginSet::strict

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn strict() -> Self
```

<ResponseField name="return" type="PluginSet">

An empty strict set, which refuses plugins that declare no metadata.

</ResponseField>

A strict set accepts an undeclared plugin when you add it, and refuses it at the
check with `PluginError::Undeclared`. The command line, the MCP server,
`Registry::plugin_set` and `plugin_host::build_with` use strict sets.

## PluginSet::with

<Badge color="green">fn</Badge>

```rust
pub fn with(self, plugin: impl Plugin) -> Result<Self>
```

<ParamField body="plugin" type="impl Plugin" required>

The plugin to add. It is moved into an `Arc`.

</ParamField>

<ResponseField name="return" type="Result<PluginSet>">

The set with the plugin at the end of the insertion order, or
`PluginError::DuplicateName` when a plugin with the same name is in it.

</ResponseField>

Grants the plugin what it declares, except Critical capabilities:

| The plugin | Gets |
|---|---|
| Declares metadata (`Metadata::new`) | `capabilities` and `optional`, without the Critical ones (`raw:*`, `host.process`, `cdp:Target`, `cdp:Browser`) |
| Declares nothing (`Metadata::legacy`) | `CapabilitySet::legacy()`: `launch.args`, `launch.prefs`, `launch.headless`, `launch.proxy`, `launch.extensions`, `page.script:*`, `page.evaluate` and `raw:cdp` |

A plugin that requires a Critical capability therefore fails the check
(`it is required but not granted`) unless it is added with `with_trusted` or
`with_grants`.

## PluginSet::with_shared

<Badge color="green">fn</Badge>

```rust
pub fn with_shared(self, plugin: Arc<dyn Plugin>) -> Result<Self>
```

<ParamField body="plugin" type="Arc<dyn Plugin>" required>

A plugin shared with other sets, such as one a `Registry` built.

</ParamField>

<ResponseField name="return" type="Result<PluginSet>">

As `with`, with the same default grants.

</ResponseField>

## PluginSet::with_trusted

<Badge color="green">fn</Badge>

```rust
pub fn with_trusted(self, plugin: impl Plugin, trusted: &[Capability]) -> Result<Self>
```

<ParamField body="plugin" type="impl Plugin" required>

The plugin to add.

</ParamField>

<ParamField body="trusted" type="&[Capability]" required>

Capabilities to grant besides the default grants. They may be Critical.

</ParamField>

<ResponseField name="return" type="Result<PluginSet>">

As `with`.

</ResponseField>

The way to run a plugin that requires `raw:cdp`, `raw:switches`,
`host.process`, `cdp:Target` or `cdp:Browser`. A set built from names never
grants those, so such a plugin is added in code.

## PluginSet::with_grants

<Badge color="green">fn</Badge>

```rust
pub fn with_grants(self, plugin: impl Plugin, grants: CapabilitySet) -> Result<Self>
```

<ParamField body="plugin" type="impl Plugin" required>

The plugin to add.

</ParamField>

<ParamField body="grants" type="CapabilitySet" required>

Exactly the capabilities to grant, as the command line will from a lockfile.
See [CapabilitySet](/plugins/api/capabilities#capabilityset).

</ParamField>

<ResponseField name="return" type="Result<PluginSet>">

As `with`.

</ResponseField>

The default grants do not apply: a declared plugin gets only `grants`, and an
undeclared one does not get the legacy set unless `grants` holds it.

## PluginSet::with_grants_shared

<Badge color="green">fn</Badge>

```rust
pub fn with_grants_shared(self, plugin: Arc<dyn Plugin>, grants: CapabilitySet) -> Result<Self>
```

<ParamField body="plugin" type="Arc<dyn Plugin>" required>

A shared plugin.

</ParamField>

<ParamField body="grants" type="CapabilitySet" required>

Exactly the capabilities to grant.

</ParamField>

<ResponseField name="return" type="Result<PluginSet>">

As `with`. Every other way of adding a plugin ends here.

</ResponseField>

## PluginSet::is_empty

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn is_empty(&self) -> bool
```

<ResponseField name="return" type="bool">

Whether the set has no plugins. Both engines skip every plugin step for an
empty set.

</ResponseField>

## PluginSet::for_launch

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn for_launch(&self) -> Self
```

<ResponseField name="return" type="PluginSet">

The set for one browser: each plugin whose
[`per_launch`](/plugins/api/plugin-trait) returns an instance with the same
name is replaced by it. Grants and strictness are kept.

</ResponseField>

Both engines call it before every launch and every attach, and use the result
for every later hook of that browser, so pooled browsers launched from one set
do not share a plugin's state.

## PluginSet::names

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn names(&self) -> Vec<&str>
```

<ResponseField name="return" type="Vec<&str>">

The plugins' names, in the order their hooks run (see [Run order](#run-order)).

</ResponseField>

## PluginSet::check

<Badge color="green">fn</Badge>

```rust
pub fn check(&self, context: &LaunchContext) -> Result<()>
```

<ParamField body="context" type="&LaunchContext" required>

The browser about to start. See [LaunchContext](/plugins/api/launch#launchcontext).

</ParamField>

<ResponseField name="return" type="Result<()>">

As `check_with`.

</ResponseField>

`check_with(context, &HostInfo::cdp())`: the check on a host that supports
every capability.

## PluginSet::check_with

<Badge color="green">fn</Badge>

```rust
pub fn check_with(&self, context: &LaunchContext, host: &HostInfo) -> Result<()>
```

<ParamField body="context" type="&LaunchContext" required>

The browser about to start.

</ParamField>

<ParamField body="host" type="&HostInfo" required>

What the host supports: `HostInfo::cdp()`, `HostInfo::webdriver(family)` or
`HostInfo::embedded()`. See [HostInfo](/plugins/api/capabilities#hostinfo).

</ParamField>

<ResponseField name="return" type="Result<()>">

`Ok(())`, or the first error found. Nothing has started either way.

</ResponseField>

The checks, in this order:

1. The run order resolves: every `requires` and `dependencies` name is in the
   set (`MissingDependency`), no plugin conflicts with another that is present
   (`Conflict`), and the edges have no cycle (`Cycle`).
2. Then, for each plugin in run order:
   1. a strict set refuses it when it declares no metadata (`Undeclared`);
   2. its requirements and packages (`Requirement`, with one of these reasons):
      `it needs a browser with a window, not a headless one` (`HEADFUL`),
      `it only works with a browser the program starts, not one it attaches to`
      (`LAUNCH_ONLY`), `it needs a Chromium-based browser` (`CHROMIUM_ONLY`),
      `one of its packages says neither only_on nor everywhere`;
   3. each capability in its metadata's `capabilities`: the host supports it
      (`Unsupported`), and the set granted it (`Capability`, `it is required but
      not granted`). Capabilities in `optional` are never checked.

The engines call it before `before_launch` and again after it; an attach is
checked once. The WebDriver
engine passes `HostInfo::webdriver(family)`, except that a set holding an
undeclared plugin is checked as for Chromium; see
[Limits](/plugins/architecture#limits).

## PluginSet::audit

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn audit(&self) -> Vec<AuditRow>
```

<ResponseField name="return" type="Vec<AuditRow>">

One row for every capability each plugin requires or optionally uses, plugin
by plugin in run order, each plugin's capabilities sorted and without
repeats.

</ResponseField>

`granted` says whether the plugin's grants cover the capability, and
`enforcement` is the capability's enforcement for a native plugin. An
undeclared plugin's rows are the legacy set, with `category: None`.

## PluginSet::before_launch

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
pub async fn before_launch(&self, launch: &mut LaunchSettings) -> Result<()>
```

<ParamField body="launch" type="&mut LaunchSettings" required>

The settings the host filled in. Each plugin sees the changes of the ones
before it.

</ParamField>

<ResponseField name="return" type="Result<()>">

The first hook error as `PluginError::Hook`, or the first rule broken as
`PluginError::Capability` or `PluginError::Sealed`. Later plugins do not run.

</ResponseField>

Lets every plugin change the start-up settings, in run order, and checks each
plugin's change right after its hook:

| Change | Needs | Refused with |
|---|---|---|
| An entry removed from `args`, `disable_features`, `enable_features`, `host_resolver_rules` or `extensions` | never allowed | `list settings only grow; it removed an entry` |
| New `args` | `launch.args` | `it changed a setting without the capability` |
| A new arg in `REFUSED_SWITCHES`, or a `--disable-features=`/`--enable-features=` arg naming a refused feature | `raw:switches` | ``the switch `--NAME` is refused for every plugin``, or ``the feature `NAME` may not be changed by a plugin`` |
| New `disable_features` or `enable_features` | `launch.args`; a refused feature needs `raw:switches` too | `that feature may not be changed by a plugin` |
| `host_resolver_rules`, `webrtc_policy` or `disable_quic` changed | `launch.network` | `it changed a setting without the capability` |
| A key removed from `prefs` | never allowed | `it removed a preference another plugin set` |
| `prefs` changed | `launch.prefs` | `it changed a setting without the capability` |
| `extensions` changed | `launch.extensions` | as above |
| `headless` changed | `launch.headless` | as above |
| `proxy` changed | `launch.proxy` | as above |
| The profile changed | never allowed | `the profile is read-only for plugins` |
| A sealed `proxy`, `webrtc_policy` or `headless` set to another value | never allowed | `PluginError::Sealed` |

A field is sealed by the first plugin that changes it while its metadata holds
the matching seal (`Seals::PROXY`, `Seals::WEBRTC`, `Seals::HEADLESS`); a later
plugin may set the same value again, not another one. See
[Seals](/plugins/api/metadata#seals).

## PluginSet::on_browser

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
pub async fn on_browser(&self, browser: &dyn BrowserHandle) -> Result<()>
```

<ParamField body="browser" type="&dyn BrowserHandle" required>

The running browser.

</ParamField>

<ResponseField name="return" type="Result<()>">

The first hook error, as `PluginError::Hook`.

</ResponseField>

Tells every plugin, in run order, that the browser runs.

## PluginSet::on_page_created

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
pub async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()>
```

<ParamField body="page" type="&dyn PageHandle" required>

The new tab.

</ParamField>

<ResponseField name="return" type="Result<()>">

The first error, as `PluginError::Hook` with the hook `packages` or
`on_page_created`.

</ResponseField>

Plugin by plugin, in run order: adds the plugin's packages with
`page.add_init_script(&package.init_script())`, then calls its
`on_page_created`. A package that says neither `only_on` nor `everywhere` is
skipped here; the check has already refused it.

## PluginSet::on_close

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
pub async fn on_close(&self) -> Vec<PluginError>
```

<ResponseField name="return" type="Vec<PluginError>">

Every failure, each as `PluginError::Hook` with the hook `on_close`; empty
when every plugin closed cleanly.

</ResponseField>

Tells every plugin, in run order, that the browser is about to close. Every
plugin runs even when one fails; the caller logs the failures.

## AuditRow

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::AuditRow`

```rust
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct AuditRow {
    pub plugin: String,
    pub category: Option<Category>,
    pub capability: Capability,
    pub risk: Risk,
    pub granted: bool,
    pub enforcement: Enforcement,
}
```

One row of `PluginSet::audit`.

<ParamField body="plugin" type="String">

The plugin's name.

</ParamField>

<ParamField body="category" type="Option<Category>">

Its category; `None` for a plugin that declares no metadata.

</ParamField>

<ParamField body="capability" type="Capability">

The capability, from the plugin's `capabilities` or `optional`.

</ParamField>

<ParamField body="risk" type="Risk">

`capability.risk()`: `Low`, `Medium`, `High` or `Critical`.

</ParamField>

<ParamField body="granted" type="bool">

Whether the plugin's grants cover it.

</ParamField>

<ParamField body="enforcement" type="Enforcement">

`capability.enforcement(false)`: whether the host enforces it for a native
plugin, or the plugin only declares it.

</ParamField>

## Run order

Every method that runs hooks, and `names` and `audit`, goes through the plugins
in one order, resolved from:

1. **The phase** of each plugin: `Last` for a plugin with
   `Requirements::RUN_LAST`; otherwise its metadata's `phase`; otherwise its
   category's default phase; otherwise `Normal`. Phases run in the order
   `Prepare`, `Normal`, `Enforce`, `Observe`, `Last`
   ([Phase](/plugins/api/metadata#phase)).
2. **The edges**: each name in `requires` or `dependencies` runs before the
   plugin (and must be present); each present plugin named in `after` runs
   before it; each present plugin named in `before` runs after it. An `after`
   or `before` naming an absent plugin is ignored. Edges win over phases: a
   `Prepare` plugin that runs `after` a `Normal` one comes after it.
3. **The insertion order**, among plugins that are ready at the same time and
   share a phase.

The resolver is Kahn's algorithm, always taking the ready plugin with the
lowest phase, then the lowest insertion index. So a set of plugins that declare
nothing runs in the order they were added, with the `RUN_LAST` ones at the
end. When the order cannot be resolved (a missing `requires`, a conflict or a
cycle), `names` and the hook runners fall back to insertion order with the
`RUN_LAST` plugins at the end; `check` reports the error.
