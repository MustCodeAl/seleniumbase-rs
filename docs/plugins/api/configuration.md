---
title: "Configuration"
sidebarTitle: "Configuration"
description: "Reference for Configurable, Descriptor, PluginSpec and Registry: building plugins from a name and a TOML or JSON options value, as the settings file, SB_PLUGINS and the command line do."
---

A program turns a plugin on by name, with options from the settings file or the
command line, through four items of `seleniumbase_rs_plugin`, defined in
`crates/seleniumbase-rs-plugin/src/config.rs`. A plugin implements
`Configurable`; `Descriptor::of` describes it; a `Registry` holds descriptors
and builds plugins from `PluginSpec`s. seleniumbase-rs turns a
`PluginSelection` into specs for you ([Selection](/plugins/api/selection));
how options reach a plugin is in the
[Options reference](/plugins/writing-your-own#options-reference).

<RequestExample>

```rust A configurable plugin
use seleniumbase_rs_plugin::{
    async_trait, Capability, Category, Configurable, HostScope, Metadata, PageHandle, Plugin,
    PluginError, Result,
};
use serde_json::Value;

pub struct Hello {
    message: String,
}

impl Configurable for Hello {
    const NAME: &'static str = "hello";

    fn describe() -> Metadata {
        Metadata::new(Category::Testing)
            .version(env!("CARGO_PKG_VERSION"))
            .summary("Logs a line in the console of every page")
            .capability(Capability::PageScript(HostScope::Everywhere))
    }

    /// Options: `message`, the text to log.
    fn configure(options: &Value) -> Result<Self> {
        let default = || "hello from a plugin".to_owned();
        let message = match options {
            Value::Null => default(),
            Value::Object(table) => {
                if let Some(key) = table.keys().find(|key| key.as_str() != "message") {
                    return Err(PluginError::Config(format!("hello: unknown option '{key}'")));
                }
                match table.get("message") {
                    None => default(),
                    Some(Value::String(text)) => text.clone(),
                    Some(other) => {
                        return Err(PluginError::Config(format!(
                            "hello: 'message' must be a string, not {other}"
                        )));
                    }
                }
            }
            other => return Err(PluginError::Config(format!("hello: not a table: {other}"))),
        };
        Ok(Self { message })
    }
}

#[async_trait]
impl Plugin for Hello {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn metadata(&self) -> Metadata {
        Self::describe()
    }

    async fn on_page_created(&self, page: &dyn PageHandle) -> Result<()> {
        let script = format!("console.log({});", Value::from(self.message.as_str()));
        page.add_init_script(&script).await
    }
}
```

```rust A registry
use seleniumbase_rs_plugin::{Descriptor, Plugin, PluginSpec, Registry};
use serde_json::json;

let registry = Registry::new().with(Descriptor::of::<Hello>());
assert_eq!(registry.names(), ["hello"]);

let spec = PluginSpec::new("hello").options(json!({ "message": "hi" }));
let plugin = registry.build(&spec)?;
assert_eq!(plugin.name(), "hello");
```

```rust A strict set from specs
use seleniumbase_rs_plugin::PluginSpec;
use serde_json::json;

let set = registry.plugin_set(&[
    PluginSpec::new("hello").options(json!({ "message": "hi" })),
])?;
assert_eq!(set.names(), ["hello"]);
```

```rust With seleniumbase-rs
use seleniumbase_rs::config::plugins::PluginSelection;
use seleniumbase_rs::plugin_api::Descriptor;
use seleniumbase_rs::plugin_host;
use serde_json::json;

// The built-in plugins of this build, plus Hello.
let registry = plugin_host::builtin_registry().with(Descriptor::of::<Hello>());
let selection = PluginSelection::new()
    .enable("hello")
    .with_options("hello", json!({ "message": "hi" }));
let plugins = plugin_host::build_with(&registry, &selection)?;
```

</RequestExample>

<ResponseExample>

```text Unknown name
plugin configuration: no plugin named `nope` (known: hello)
```

```text Wrong option
plugin configuration: hello: unknown option 'mesage'
```

</ResponseExample>

## Configurable

<Badge color="purple">trait</Badge> `seleniumbase_rs_plugin::Configurable`

```rust
pub trait Configurable: Plugin + Sized {
    const NAME: &'static str;
    fn describe() -> Metadata;
    fn configure(options: &Value) -> Result<Self>;
}
```

A plugin that can be built from a name and a TOML or JSON options value. It is
what `Descriptor::of` needs, and so what every registry, the settings file,
`SB_PLUGINS`, `--plugin` and `sbrs plugin info` need. Its methods are not
`async`.

## Configurable::NAME

<Badge color="gray">const</Badge> <Badge color="red" stroke>required</Badge>

```rust
const NAME: &'static str;
```

The name the plugin is enabled by: the key of the registry, of `enable` and of
the options table. Return the same string from `Plugin::name`; the set detects
duplicates and resolves ordering edges by `Plugin::name`.

## Configurable::describe

<Badge color="green">fn</Badge> <Badge color="red" stroke>required</Badge>

```rust
fn describe() -> Metadata;
```

<ResponseField name="return" type="Metadata">

What the plugin declares, without building one. See [Metadata](/plugins/api/metadata).

</ResponseField>

A `Descriptor` stores it, so `sbrs plugin list`, `sbrs plugin info` and the
check of what a project's settings file may turn on read it. Return it from
`Plugin::metadata` too, adding only what depends on the options (as `crx` adds
the files it reads); the set checks what `Plugin::metadata` returns.

## Configurable::configure

<Badge color="green">fn</Badge> <Badge color="red" stroke>required</Badge>

```rust
fn configure(options: &Value) -> Result<Self>;
```

<ParamField body="options" type="&serde_json::Value" required>

The plugin's options: `Value::Null` when none were given; otherwise usually a
`Value::Object`, such as the `[plugins.NAME]` table of the settings file
converted to JSON, with each `--plugin-opt NAME.KEY=VALUE` setting one key.

</ParamField>

<ResponseField name="return" type="Result<Self>">

The plugin, or `PluginError::Config` when an option is wrong.

</ResponseField>

Refuse a key you do not know, so a typo does not turn a setting off without a
word. With serde, `#[serde(default, deny_unknown_fields)]` on an options struct
does that.

## Descriptor

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::Descriptor`

```rust
#[derive(Clone, Debug)]
pub struct Descriptor { /* private: name, metadata, build */ }
```

One plugin a registry can build: its name, what it declares, and a function
that builds it from options. The only way to make one is `Descriptor::of`.

## Descriptor::of

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn of<P: Configurable>() -> Self
```

<ParamField body="P" type="type parameter: Configurable" required>

The plugin type.

</ParamField>

<ResponseField name="return" type="Descriptor">

`P::NAME`, `P::describe()` (called once, now), and a builder that calls
`P::configure` and puts the plugin in an `Arc`.

</ResponseField>

## Descriptor::name

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn name(&self) -> &'static str
```

<ResponseField name="return" type="&'static str">

`P::NAME`.

</ResponseField>

## Descriptor::metadata

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn metadata(&self) -> &Metadata
```

<ResponseField name="return" type="&Metadata">

What `P::describe()` returned.

</ResponseField>

## PluginSpec

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::PluginSpec`

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct PluginSpec {
    pub name: String,
    pub options: Value,
}
```

A plugin to enable: its name and options.

<ParamField body="name" type="String" required>

The plugin's name, as `Configurable::NAME` spells it.

</ParamField>

<ParamField body="options" type="serde_json::Value" default="Value::Null">

Its options; `Value::Null` for none.

</ParamField>

## PluginSpec::new

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn new(name: impl Into<String>) -> Self
```

<ParamField body="name" type="impl Into<String>" required>

The plugin's name.

</ParamField>

<ResponseField name="return" type="PluginSpec">

A spec with no options (`Value::Null`).

</ResponseField>

## PluginSpec::options

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn options(mut self, options: Value) -> Self
```

<ParamField body="options" type="serde_json::Value" required>

The options, usually `json!(...)`. They replace earlier ones.

</ParamField>

<ResponseField name="return" type="PluginSpec">

The spec with these options.

</ResponseField>

## Registry

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::Registry`

```rust
#[derive(Clone, Debug, Default)]
pub struct Registry { /* private: descriptors, in the order added */ }
```

The plugins a program can enable by name. seleniumbase-rs's
`plugin_host::builtin_registry()` is the registry of the built-in plugins whose
cargo feature is on, in the order `shield`, `block-resources`, `crx`
([Host](/plugins/api/host)).

## Registry::new

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn new() -> Self
```

<ResponseField name="return" type="Registry">

An empty registry.

</ResponseField>

## Registry::with

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn with(mut self, descriptor: Descriptor) -> Self
```

<ParamField body="descriptor" type="Descriptor" required>

The plugin to add, usually `Descriptor::of::<P>()`.

</ParamField>

<ResponseField name="return" type="Registry">

The registry with the descriptor at the end. One with the same name is
removed first, so a later descriptor replaces an earlier one and takes the last
place.

</ResponseField>

## Registry::names

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn names(&self) -> Vec<&'static str>
```

<ResponseField name="return" type="Vec<&'static str>">

The known names, in the order added.

</ResponseField>

## Registry::get

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn get(&self, name: &str) -> Option<&Descriptor>
```

<ParamField body="name" type="&str" required>

A plugin name.

</ParamField>

<ResponseField name="return" type="Option<&Descriptor>">

The descriptor called `name`, or `None`.

</ResponseField>

## Registry::build

<Badge color="green">fn</Badge>

```rust
pub fn build(&self, spec: &PluginSpec) -> Result<Arc<dyn Plugin>>
```

<ParamField body="spec" type="&PluginSpec" required>

The name and options.

</ParamField>

<ResponseField name="return" type="Result<Arc<dyn Plugin>>">

The plugin. `PluginError::Config` for an unknown name (``no plugin named `NAME`
(known: a, b)``), or whatever `configure` returns for wrong options.

</ResponseField>

## Registry::plugin_set

<Badge color="green">fn</Badge>

```rust
pub fn plugin_set(&self, specs: &[PluginSpec]) -> Result<PluginSet>
```

<ParamField body="specs" type="&[PluginSpec]" required>

The plugins to enable, in insertion order.

</ParamField>

<ResponseField name="return" type="Result<PluginSet>">

A strict set of the plugins the specs name, or the first build or
duplicate-name error.

</ResponseField>

Each plugin is granted the `capabilities` and `optional` of its
`Plugin::metadata`, without the Critical ones, so a plugin built from a name
never holds `raw:*`, `host.process`, `cdp:Target` or `cdp:Browser`. A program
that uses seleniumbase-rs usually calls `plugin_host::build_with` instead,
which does the same from a `PluginSelection` and reports a mistake as
`SeleniumBaseError::Plugin` with the plugin's name.
