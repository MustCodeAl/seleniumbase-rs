---
title: "Metadata"
sidebarTitle: "Metadata"
description: "Reference for Metadata, what a plugin declares about itself (category, version, summary, capabilities, ordering edges, phase, seals and budgets), and for Category, Phase, Seals and Budgets."
---

A plugin says what it is and what it needs by returning a `Metadata` from
`Plugin::metadata` (and, for a configurable plugin, from
`Configurable::describe`). The set orders, checks, grants and audits plugins
from it. Items are in `seleniumbase_rs_plugin`, defined in
`crates/seleniumbase-rs-plugin/src/metadata.rs`. The design is on
[Plugin architecture](/plugins/architecture).

<RequestExample>

```rust Declare metadata
use seleniumbase_rs_plugin::{Capability, Category, HostScope, Metadata, Seals};

fn metadata() -> Metadata {
    Metadata::new(Category::Privacy)
        .version(env!("CARGO_PKG_VERSION"))
        .summary("Keeps WebRTC from leaking local addresses")
        .capabilities([
            Capability::LaunchNetwork,
            Capability::PageScript(HostScope::Everywhere),
        ])
        .optional(Capability::Cdp("Network".into()))
        .seal(Seals::WEBRTC)
}
```

```rust Order against other plugins
use seleniumbase_rs_plugin::{Category, Metadata, Phase};

fn metadata() -> Metadata {
    Metadata::new(Category::Core)
        .requires("block-resources") // must be in the set, runs before
        .conflicts_with("crx") // may not be in the set
        .after("shield") // runs after it, when present
        .before("recorder") // runs before it, when present
        .phase(Phase::Observe) // instead of Core's Normal
}
```

```rust Budgets
use std::time::Duration;

use seleniumbase_rs_plugin::{Budgets, Category, Metadata};

// Budgets is #[non_exhaustive]: start from the default.
let mut budgets = Budgets::default();
budgets.before_launch = Some(Duration::from_secs(5));
budgets.on_page_created = Some(Duration::from_millis(500));
let meta = Metadata::new(Category::Environment).budget(budgets);
```

```rust Read it back
use seleniumbase_rs_plugin::{Category, Metadata, Phase};

let meta = Metadata::new(Category::Privacy);
assert!(meta.is_declared());
assert_eq!(meta.phase, None);
assert_eq!(Category::Privacy.default_phase(), Phase::Enforce);
assert!(!Metadata::legacy().is_declared());
```

</RequestExample>

## Metadata

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::Metadata`

```rust
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Metadata {
    pub category: Option<Category>,
    pub version: Cow<'static, str>,
    pub summary: Cow<'static, str>,
    pub capabilities: CapabilitySet,
    pub optional: CapabilitySet,
    pub requires: Vec<Cow<'static, str>>,
    pub conflicts: Vec<Cow<'static, str>>,
    pub after: Vec<Cow<'static, str>>,
    pub before: Vec<Cow<'static, str>>,
    pub phase: Option<Phase>,
    pub seals: Seals,
    pub budgets: Budgets,
}
```

What a plugin declares about itself. Build one with `Metadata::new` and the
builder methods; outside the crate the fields can be read and assigned, not
listed in a struct literal.

<ParamField body="category" type="Option<Category>">

The plugin's category; `None` for a legacy plugin, which declares nothing.

</ParamField>

<ParamField body="version" type="Cow<'static, str>" default="empty">

The plugin's version, such as `env!("CARGO_PKG_VERSION")`. Shown by
`sbrs plugin info`.

</ParamField>

<ParamField body="summary" type="Cow<'static, str>" default="empty">

One line about what it does. Shown by `sbrs plugin list` and `sbrs plugin info`.

</ParamField>

<ParamField body="capabilities" type="CapabilitySet" default="empty">

What it needs. The check fails when the host cannot give one or the set did not
grant it. See [Capabilities](/plugins/api/capabilities).

</ParamField>

<ParamField body="optional" type="CapabilitySet" default="empty">

What it uses when granted. Granted by default like `capabilities`, minus the
Critical ones, and listed by the audit, but never checked.

</ParamField>

<ParamField body="requires" type="Vec<Cow<'static, str>>" default="empty">

Plugins that must be in the set and run before it.

</ParamField>

<ParamField body="conflicts" type="Vec<Cow<'static, str>>" default="empty">

Plugins that may not be in the set.

</ParamField>

<ParamField body="after" type="Vec<Cow<'static, str>>" default="empty">

Plugins it runs after, when they are present.

</ParamField>

<ParamField body="before" type="Vec<Cow<'static, str>>" default="empty">

Plugins it runs before, when they are present.

</ParamField>

<ParamField body="phase" type="Option<Phase>" default="None">

`None`: the category's default phase.

</ParamField>

<ParamField body="seals" type="Seals" default="Seals::empty()">

The scalar launch fields it seals against later change.

</ParamField>

<ParamField body="budgets" type="Budgets" default="Budgets::default()">

Time limits it asks for; stored, not enforced yet.

</ParamField>

## Metadata::new

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn new(category: Category) -> Self
```

<ParamField body="category" type="Category" required>

What kind of job the plugin does.

</ParamField>

<ResponseField name="return" type="Metadata">

A declared plugin of `category`, with an empty version and summary, no
capabilities, no edges, no phase of its own, no seals and no budgets.

</ResponseField>

## Metadata::legacy

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn legacy() -> Self
```

<ResponseField name="return" type="Metadata">

An undeclared plugin: `category: None` and `capabilities:
CapabilitySet::legacy()` (`launch.args`, `launch.prefs`, `launch.headless`,
`launch.proxy`, `launch.extensions`, `page.script:*`, `page.evaluate`,
`raw:cdp`), the rest as in `new`.

</ResponseField>

The default of `Plugin::metadata`. A plain set runs such a plugin with the
legacy grants and in the `Normal` phase; a strict set refuses it.

## Metadata::is_declared

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn is_declared(&self) -> bool
```

<ResponseField name="return" type="bool">

Whether the plugin declared its metadata: `category.is_some()`.

</ResponseField>

## Metadata::version

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn version(mut self, version: impl Into<Cow<'static, str>>) -> Self
```

<ParamField body="version" type="impl Into<Cow<'static, str>>" required>

The version, such as `env!("CARGO_PKG_VERSION")`.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with this version.

</ResponseField>

## Metadata::summary

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn summary(mut self, summary: impl Into<Cow<'static, str>>) -> Self
```

<ParamField body="summary" type="impl Into<Cow<'static, str>>" required>

One line about what the plugin does.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with this summary.

</ResponseField>

## Metadata::capability

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn capability(mut self, capability: Capability) -> Self
```

<ParamField body="capability" type="Capability" required>

A capability the plugin needs; added to `capabilities`.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with the capability.

</ResponseField>

## Metadata::capabilities

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn capabilities(mut self, capabilities: impl IntoIterator<Item = Capability>) -> Self
```

<ParamField body="capabilities" type="impl IntoIterator<Item = Capability>" required>

Capabilities the plugin needs; each is added to `capabilities`.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with the capabilities.

</ResponseField>

## Metadata::optional

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn optional(mut self, capability: Capability) -> Self
```

<ParamField body="capability" type="Capability" required>

A capability the plugin uses if granted; added to `optional`.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with the optional capability.

</ResponseField>

A plugin that can work without it handles the error where it is missing, such
as `send` on a browser without the DevTools Protocol.

## Metadata::requires

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn requires(mut self, name: impl Into<Cow<'static, str>>) -> Self
```

<ParamField body="name" type="impl Into<Cow<'static, str>>" required>

A plugin that must be in the set and run before this one.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with the edge. A missing plugin fails the check with
`PluginError::MissingDependency`.

</ResponseField>

## Metadata::conflicts_with

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn conflicts_with(mut self, name: impl Into<Cow<'static, str>>) -> Self
```

<ParamField body="name" type="impl Into<Cow<'static, str>>" required>

A plugin that may not be in the same set; added to `conflicts`.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with the conflict. Both in one set fail the check with
`PluginError::Conflict`.

</ResponseField>

## Metadata::after

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn after(mut self, name: impl Into<Cow<'static, str>>) -> Self
```

<ParamField body="name" type="impl Into<Cow<'static, str>>" required>

A plugin this one runs after, when it is in the set.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with the edge. An absent plugin is ignored.

</ResponseField>

## Metadata::before

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn before(mut self, name: impl Into<Cow<'static, str>>) -> Self
```

<ParamField body="name" type="impl Into<Cow<'static, str>>" required>

A plugin this one runs before, when it is in the set.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with the edge. An absent plugin is ignored.

</ResponseField>

## Metadata::phase

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn phase(mut self, phase: Phase) -> Self
```

<ParamField body="phase" type="Phase" required>

The phase to run in, instead of the category's default.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with `phase: Some(phase)`. `Requirements::RUN_LAST` still puts the
plugin in `Last`.

</ResponseField>

## Metadata::seal

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn seal(mut self, seals: Seals) -> Self
```

<ParamField body="seals" type="Seals" required>

Fields to seal, combined with `|`. They are added to the seals already set.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with the seals.

</ResponseField>

## Metadata::budget

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn budget(mut self, budgets: Budgets) -> Self
```

<ParamField body="budgets" type="Budgets" required>

The time limits; they replace earlier ones.

</ParamField>

<ResponseField name="return" type="Metadata">

The metadata with these budgets.

</ResponseField>

## Category

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::Category`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Category {
    Environment,
    Testing,
    Extension,
    Core,
    Privacy,
}
```

What kind of job a plugin does. It serializes in lower case (`"privacy"`), the
word `sbrs plugin list` and `sbrs plugin info` print.

<ResponseField name="Category" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Environment" type="default phase: Prepare">

Presets, runtime, switches, proxies, headless.

</ResponseField>

<ResponseField name="Testing" type="default phase: Observe">

Assertions, lifecycle, reporters, recording fakes.

</ResponseField>

<ResponseField name="Extension" type="default phase: Normal">

Extensions and their workers.

</ResponseField>

<ResponseField name="Core" type="default phase: Normal">

Interception, network logging, resource blocking.

</ResponseField>

<ResponseField name="Privacy" type="default phase: Enforce">

Hardening, telemetry, WebRTC, cookie isolation.

</ResponseField>

</Expandable>

</ResponseField>

## Category::default_phase

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn default_phase(self) -> Phase
```

<ResponseField name="return" type="Phase">

The phase plugins of this category run in unless they say otherwise:
`Environment` → `Prepare`, `Extension` and `Core` → `Normal`, `Privacy` →
`Enforce`, `Testing` → `Observe`.

</ResponseField>

## Phase

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::Phase`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Phase {
    Prepare,
    Normal,
    Enforce,
    Observe,
    Last,
}
```

When, relative to other plugins, a plugin runs. The variants are ordered as
declared, and the set runs lower phases first; ordering edges can override it
([Run order](/plugins/api/plugin-set#run-order)).

<ResponseField name="Phase" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Prepare" type="unit variant">First: set the environment up.</ResponseField>

<ResponseField name="Normal" type="unit variant">The default.</ResponseField>

<ResponseField name="Enforce" type="unit variant">After the others, so a privacy plugin has the last word.</ResponseField>

<ResponseField name="Observe" type="unit variant">Only watches the result.</ResponseField>

<ResponseField name="Last" type="unit variant">Last. A plugin with `Requirements::RUN_LAST` is always here.</ResponseField>

</Expandable>

</ResponseField>

## Seals

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::Seals`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Seals(/* private u8 */);

impl Seals {
    pub const PROXY: Self;
    pub const CONTEXT_PROXY: Self;
    pub const WEBRTC: Self;
    pub const HEADLESS: Self;
    pub const FULFILL: Self;
    pub const fn empty() -> Self;
    pub const fn contains(self, other: Self) -> bool;
}

impl std::ops::BitOr for Seals { type Output = Self; }
```

The scalar launch fields a plugin may seal against later change, combined with
`|`. When a plugin whose metadata holds a seal changes that field in
`before_launch`, a later plugin may set it again to the same value but not to
another one (`PluginError::Sealed`). Two plugins that seal one field to
different values cannot run together.

<ResponseField name="Seals" type="flags">

<Expandable title="constants" defaultOpen>

<ResponseField name="Seals::PROXY" type="const">

`LaunchSettings::proxy`. Checked by `PluginSet::before_launch`.

</ResponseField>

<ResponseField name="Seals::CONTEXT_PROXY" type="const">

The proxy of new browser contexts. Stored and shown by `sbrs plugin info`
(`context-proxy`); no hook of this version checks it.

</ResponseField>

<ResponseField name="Seals::WEBRTC" type="const">

`LaunchSettings::webrtc_policy`. Checked by `PluginSet::before_launch`.

</ResponseField>

<ResponseField name="Seals::HEADLESS" type="const">

`LaunchSettings::headless`. Checked by `PluginSet::before_launch`.

</ResponseField>

<ResponseField name="Seals::FULFILL" type="const">

The one plugin that answers requests. Stored and shown (`fulfill`); no hook of
this version checks it.

</ResponseField>

</Expandable>

</ResponseField>

<ResponseField name="Seals::empty()" type="const fn() -> Seals">

No seals; also `Seals::default()`.

</ResponseField>

<ResponseField name="contains(self, other)" type="const fn(Seals, Seals) -> bool">

Whether every seal of `other` is set.

</ResponseField>

## Budgets

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::Budgets`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Budgets {
    pub before_launch: Option<Duration>,
    pub on_page_created: Option<Duration>,
}
```

Time limits a plugin asks for. This version stores them; no code reads, caps
or enforces them yet (the API's own comment plans a timer for a later phase).

<ParamField body="before_launch" type="Option<Duration>" default="None">

For `before_launch`.

</ParamField>

<ParamField body="on_page_created" type="Option<Duration>" default="None">

For `on_page_created`.

</ParamField>
