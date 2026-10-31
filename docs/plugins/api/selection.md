---
title: "Selection by Name"
sidebarTitle: "Selection"
description: "Reference for turning plugins on by name in seleniumbase-rs: PluginSelection, SB_PLUGINS, the settings loaders, and the command-line helpers of cli::plugin_cmd (PluginChoice, the checks, and the text of sbrs plugin list and info)."
---

A program turns built-in plugins, and plugins of its own registry, on by name:
from the `[plugins]` table of the settings file, the `SB_PLUGINS` variable,
the command-line flags `--plugin` and `--plugin-opt`, or code. The names and
options are a `PluginSelection`; the launch builders and
`plugin_host::build_with` turn it into a `PluginSet`
([Host](/plugins/api/host)). The user guide is
[Choosing plugins](/plugins/selecting). `PluginSelection` and `PLUGINS_ENV`
are always compiled, so a settings file with a `[plugins]` table parses in
every build; the items marked "Feature: plugins" need that feature.

<RequestExample>

```rust The settings file
use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::sb_cdp::LaunchOptions;

// sbase_config.toml:
//   [plugins]
//   enable = ["shield"]
//   [plugins.shield]
//   level = "strict"
let settings = Settings::load()?; // the file, then SB_PLUGINS on top
let options = LaunchOptions::builder()
    .plugin_selection(&settings.plugins)
    .build()?;
```

```rust In code
use seleniumbase_rs::config::plugins::PluginSelection;
use serde_json::json;

let selection = PluginSelection::new()
    .enable("shield")
    .enable("block-resources")
    .with_options("block-resources", json!({ "preset": "media" }));
assert_eq!(selection.to_list(), "shield,block-resources");
```

```rust Command-line flags
use seleniumbase_rs::config::plugins::PluginSelection;

let mut selection = PluginSelection::from_list("shield");
selection.apply_flags(&["crx"], &[r#"shield.level="strict""#])?;
assert_eq!(selection.enabled(), ["shield", "crx"]);
assert_eq!(selection.options_of("shield").unwrap()["level"], "strict");
```

```rust A command-line program
use std::path::Path;

use seleniumbase_rs::cli::plugin_cmd::{self, PluginChoice};
use seleniumbase_rs::plugin_api::Descriptor;
use seleniumbase_rs::plugin_host;

// What your parser collected from --plugin NAME and --plugin-opt NAME.KEY=VALUE.
let plugin_flags = ["hello"];
let option_flags = [r#"hello.message="hi""#];

let choice = PluginChoice::load(None::<&Path>, &plugin_flags, &option_flags)?;
let registry = plugin_host::builtin_registry().with(Descriptor::of::<Hello>());
plugin_cmd::check_selection_with(&registry, choice.selection())?;
plugin_cmd::check_origins_with(&registry, choice.selection(), choice.origins())?;
choice.log();
let plugins = plugin_host::build_with(&registry, choice.selection())?;
```

```rust What a plugin declares
use seleniumbase_rs::cli::plugin_cmd;
use seleniumbase_rs::config::plugins::PluginSelection;
use seleniumbase_rs::plugin_host;

let registry = plugin_host::builtin_registry();
let text = plugin_cmd::info_with(&registry, "block-resources", &PluginSelection::new())?;
print!("{text}");
```

</RequestExample>

<ResponseExample>

```text info_with
block-resources 0.1.3
Stops pages from loading images, fonts, media or URLs you name

category      core
enabled       no
phase         normal
needs         a Chromium-based browser
capabilities  cdp:Network  medium  enforced
optional      -
requires      -
conflicts     -
runs after    -
runs before   -
seals         -
options       none set; give them as [plugins.block-resources] in the settings file, or on the command line as --plugin-opt 'block-resources.KEY=VALUE'
```

```text check_origins_with
invalid configuration: plugin `crx` asks for host.fs.write:temp, which the settings file in the current folder cannot grant by itself: a project you clone would decide. Name the plugin yourself, with `--plugin crx` or `SB_PLUGINS=crx`
```

```text apply_flags
invalid configuration: the value of `--plugin-opt shield.level=strict` is not a TOML value (...); write a string with quotes, as --plugin-opt 'shield.level="strict"'
```

</ResponseExample>

## PluginSelection

<Badge color="blue">struct</Badge> `seleniumbase_rs::config::plugins::PluginSelection`

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSelection {
    // private: `enable`, the names in order, and every other key of the
    // table, the options of the plugin of that name
}
```

The plugins to turn on, by name, and their options. It is the `[plugins]` table
of the settings file (the `plugins` object of a JSON settings file): `enable`
lists the names in the order their plugins are added, and a table named like a
plugin holds that plugin's options.

```toml sbase_config.toml
[plugins]
enable = ["shield", "block-resources"]

[plugins.shield]
level = "strict"
```

The same value comes from `SB_PLUGINS`, from the flags through `apply_flags`,
or from code with `enable` and `with_options`. Options are kept as JSON values.

## PluginSelection::new

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn new() -> Self
```

<ResponseField name="return" type="PluginSelection">

A selection that turns nothing on.

</ResponseField>

## PluginSelection::from_list

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn from_list(list: &str) -> Self
```

<ParamField body="list" type="&str" required>

A comma-separated list of names, as `SB_PLUGINS` holds it, such as
`shield,block-resources`.

</ParamField>

<ResponseField name="return" type="PluginSelection">

The selection of those names, with no options. Spaces around a name and empty
entries are ignored: `" shield, ,block-resources,"` gives
`["shield", "block-resources"]`.

</ResponseField>

## PluginSelection::enable

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn enable(mut self, name: impl Into<String>) -> Self
```

<ParamField body="name" type="impl Into<String>" required>

A plugin name.

</ParamField>

<ResponseField name="return" type="PluginSelection">

The selection with `name` after the plugins already enabled. A name enabled
twice is kept twice, and the build then refuses it (`already in the set`).

</ResponseField>

## PluginSelection::with_options

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn with_options(mut self, name: impl Into<String>, options: Value) -> Self
```

<ParamField body="name" type="impl Into<String>" required>

The plugin the options are for.

</ParamField>

<ParamField body="options" type="serde_json::Value" required>

What the plugin's option table holds, usually a JSON object.

</ParamField>

<ResponseField name="return" type="PluginSelection">

The selection with these options, replacing earlier options of `name`.

</ResponseField>

The plugin must also be enabled: options for a plugin that is not are an error
when the selection is built.

## PluginSelection::set_enabled_from_list

<Badge color="green">fn</Badge>

```rust
pub fn set_enabled_from_list(&mut self, list: &str)
```

<ParamField body="list" type="&str" required>

A comma-separated list, as `SB_PLUGINS` holds it.

</ParamField>

Replaces the enabled names with the list, as `SB_PLUGINS` does. The options of
the plugins the list still names stay; the options of the others are dropped,
so they do not fail the build as stray options. An empty list turns every
plugin off.

## PluginSelection::set_enabled_keeping_options

<Badge color="green">fn</Badge>

```rust
pub fn set_enabled_keeping_options(&mut self, list: &str)
```

<ParamField body="list" type="&str" required>

A comma-separated list.

</ParamField>

`set_enabled_from_list` that keeps every option table. A program that applies
command-line flags after the variable uses it, so a plugin the flags turn on
again keeps the options of the settings file, and calls `prune_unused` once the
flags are applied.

## PluginSelection::prune_unused

<Badge color="green">fn</Badge>

```rust
pub fn prune_unused(&mut self)
```

Drops the options of the plugins that are not enabled.

## PluginSelection::extend

<Badge color="green">fn</Badge>

```rust
pub fn extend(&mut self, other: &Self)
```

<ParamField body="other" type="&PluginSelection" required>

Another selection.

</ParamField>

Adds the plugins of `other` after this selection's, and takes its options over
those of the same name. `LaunchOptionsBuilder::plugin_selection` uses it.

## PluginSelection::enable_once

<Badge color="green">fn</Badge>

```rust
pub fn enable_once(&mut self, name: impl Into<String>)
```

<ParamField body="name" type="impl Into<String>" required>

A plugin name.

</ParamField>

Adds `name` after the plugins already enabled, unless it is enabled already, in
which case it keeps its place.

## PluginSelection::apply_flags

<Badge color="green">fn</Badge>

```rust
pub fn apply_flags<N, O>(&mut self, plugins: &[N], options: &[O]) -> Result<(), SeleniumBaseError>
where
    N: AsRef<str>,
    O: AsRef<str>,
```

<ParamField body="plugins" type="&[impl AsRef<str>]" required>

The values of every `--plugin NAME`, in order.

</ParamField>

<ParamField body="options" type="&[impl AsRef<str>]" required>

The values of every `--plugin-opt NAME.KEY=VALUE`, in order.

</ParamField>

<ResponseField name="return" type="Result<(), SeleniumBaseError>">

`SeleniumBaseError::InvalidConfig` for a plugin name that is empty or holds a
space, a tab or a comma; an option that is not `NAME.KEY=VALUE` with a TOML
`VALUE`; an option of a plugin that nothing turns on; and the same option given
twice.

</ResponseField>

Applies the command line to a selection read from the settings file and
`SB_PLUGINS`, so the precedence is the command line over `SB_PLUGINS` over the
file. Each `--plugin` name is added with `enable_once`. Each `--plugin-opt`
sets one option of one plugin: `NAME` ends at the first `.`, `KEY` at the first
`=`, and `VALUE` is a TOML value (so a string is quoted:
`shield.level="strict"`). It replaces the same key from the file and keeps the
plugin's other options. A TOML date, or a float with no JSON form, is refused.

## PluginSelection::to_list

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn to_list(&self) -> String
```

<ResponseField name="return" type="String">

The enabled names joined with commas: the value of `SB_PLUGINS` that enables
the same plugins, the inverse of `from_list`.

</ResponseField>

## PluginSelection::enabled

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn enabled(&self) -> &[String]
```

<ResponseField name="return" type="&[String]">

The names of the plugins to enable, in order.

</ResponseField>

## PluginSelection::options_of

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn options_of(&self, name: &str) -> Option<&Value>
```

<ParamField body="name" type="&str" required>

A plugin name.

</ParamField>

<ResponseField name="return" type="Option<&Value>">

The options given for that plugin, if any.

</ResponseField>

## PluginSelection::is_empty

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn is_empty(&self) -> bool
```

<ResponseField name="return" type="bool">

Whether the selection turns nothing on and holds no options.

</ResponseField>

## PluginSelection::ensure_supported

<Badge color="green">fn</Badge>

```rust
pub fn ensure_supported(&self) -> Result<(), SeleniumBaseError>
```

<ResponseField name="return" type="Result<(), SeleniumBaseError>">

`Ok(())` when the build has the feature `plugins` or the selection is empty;
otherwise `SeleniumBaseError::InvalidConfig` naming what was asked for and
saying to rebuild with `--features plugins-builtin` (or `--features plugins`
for the API alone), or to remove the `[plugins]` table and `SB_PLUGINS`.

</ResponseField>

A program that asked for privacy hardening and silently runs without it is
worse than one that stops: `Settings::load` calls it after `SB_PLUGINS` is
applied.

## PLUGINS_ENV

<Badge color="gray">const</Badge> `seleniumbase_rs::config::plugins::PLUGINS_ENV`

```rust
pub const PLUGINS_ENV: &str = "SB_PLUGINS";
```

The environment variable that names the plugins to turn on, as a
comma-separated list such as `shield,block-resources`. It replaces the names of
the file's `enable`, and carries names only. Unlike the other `SB_*`
variables, an empty value is an answer: `SB_PLUGINS=` turns every plugin off.

## Settings::plugins

<Badge color="blue">field</Badge> `seleniumbase_rs::config::settings::Settings::plugins`

```rust
#[serde(default, skip_serializing_if = "PluginSelection::is_empty")]
pub plugins: PluginSelection,
```

The `[plugins]` table of the settings file with `SB_PLUGINS` on top, as
`Settings::load` and `Settings::load_from` read it. A build without the
feature `plugins` refuses a non-empty selection when it loads the settings.
Turn it into plugins with `LaunchOptionsBuilder::plugin_selection` or
`BrowserConfig::with_plugin_selection`.

## Settings::load_plugin_selection

<Badge color="green">fn</Badge> `seleniumbase_rs::config::settings::Settings::load_plugin_selection`

```rust
pub fn load_plugin_selection<P: AsRef<Path>>(
    path: Option<P>,
) -> Result<PluginSelection, SeleniumBaseError>
```

<ParamField body="path" type="Option<impl AsRef<Path>>" required>

A settings file (TOML for a name that ends in `.toml`, JSON otherwise), or
`None` for the global file of the current folder: the first of
`sbase_config.toml`, `.sbase_config.toml` and `.sbase_config` that exists.

</ParamField>

<ResponseField name="return" type="Result<PluginSelection, SeleniumBaseError>">

The `[plugins]` table of the file, with `SB_PLUGINS` on top. With `None`
and no settings file in the folder, the table is empty. `SeleniumBaseError::InvalidConfig` when the
file cannot be read or parsed, or its table is not a plugin selection.

</ResponseField>

Only the plugin part of the file is read, so a value that is wrong for a
browser (`headless = "maybe"`) or an invalid `SB_HEADLESS` does not stop a
program that only needs the plugins. Whether this build can run plugins is not
checked: call `ensure_supported` on the result, or build it.

## Settings::load_plugin_table

<Badge color="green">fn</Badge> `seleniumbase_rs::config::settings::Settings::load_plugin_table`

```rust
pub fn load_plugin_table<P: AsRef<Path>>(
    path: Option<P>,
) -> Result<PluginSelection, SeleniumBaseError>
```

<ParamField body="path" type="Option<impl AsRef<Path>>" required>

As for `load_plugin_selection`.

</ParamField>

<ResponseField name="return" type="Result<PluginSelection, SeleniumBaseError>">

The `[plugins]` table of the file and nothing else: `SB_PLUGINS` is not read.

</ResponseField>

For a program that applies `SB_PLUGINS` and the command line itself, and so
can tell which plugins the file turned on, as `PluginChoice::load` does.

## PluginChoice

<Badge color="blue">struct</Badge> `seleniumbase_rs::cli::plugin_cmd::PluginChoice`

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PluginChoice { /* private: selection, origins */ }
```

The plugins an operator chose for a program: what the settings file,
`SB_PLUGINS` and the command line turn on and configure, and where each name
came from. `sbrs`, `sbrs devtools` and `sbrs-mcp` build one, so the precedence
is the same for all of them.

## PluginChoice::load

<Badge color="green">fn</Badge>

```rust
pub fn load<P, N, O>(
    config: Option<P>,
    plugins: &[N],
    options: &[O],
) -> Result<Self, SeleniumBaseError>
where
    P: AsRef<Path>,
    N: AsRef<str>,
    O: AsRef<str>,
```

<ParamField body="config" type="Option<impl AsRef<Path>>" required>

The file `--config` names, or `None` for the settings file of the current
folder.

</ParamField>

<ParamField body="plugins" type="&[impl AsRef<str>]" required>

The `--plugin` values.

</ParamField>

<ParamField body="options" type="&[impl AsRef<str>]" required>

The `--plugin-opt` values.

</ParamField>

<ResponseField name="return" type="Result<PluginChoice, SeleniumBaseError>">

The choice, or `SeleniumBaseError::InvalidConfig` for a file that cannot be
read and for a flag `apply_flags` refuses.

</ResponseField>

Reads the `[plugins]` table of the file (`load_plugin_table`), replaces the
names with `SB_PLUGINS` when it is set (keeping the options), applies the flags,
and then, when the variable was set, drops the options of plugins nothing turns
on. Each enabled name's origin is the strongest place that named it: the
command line, then `SB_PLUGINS`, then the file.

## PluginChoice::selection

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn selection(&self) -> &PluginSelection
```

<ResponseField name="return" type="&PluginSelection">

The plugins to turn on and their options.

</ResponseField>

## PluginChoice::into_selection

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn into_selection(self) -> PluginSelection
```

<ResponseField name="return" type="PluginSelection">

The selection, to hand to a builder.

</ResponseField>

## PluginChoice::origins

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn origins(&self) -> &[PluginOrigin]
```

<ResponseField name="return" type="&[PluginOrigin]">

The enabled plugins in order, each with where its name was given.

</ResponseField>

## PluginChoice::check

<Badge color="green">fn</Badge>

```rust
pub fn check(&self) -> Result<(), SeleniumBaseError>
```

<ResponseField name="return" type="Result<(), SeleniumBaseError>">

`check_selection`, then (feature `plugins`) `check_origins_with` against the
built-in registry; `SeleniumBaseError::InvalidConfig` naming the mistake.

</ResponseField>

The check a program makes before it starts a browser. A program with a registry
of its own calls `check_selection_with` and `check_origins_with` with it
instead.

## PluginChoice::log

<Badge color="green">fn</Badge>

```rust
pub fn log(&self)
```

Logs one `INFO` line for each enabled plugin with the place that enabled it,
such as `plugin enabled plugin=shield source="the settings file in the
current folder"`, so a file in the folder cannot turn one on unseen.

## PluginSource

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs::cli::plugin_cmd::PluginSource`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginSource {
    ProjectFile,
    ConfigFile,
    Environment,
    CommandLine,
}

impl PluginSource {
    #[must_use]
    pub fn describe(self) -> &'static str;
}
```

Where the name of a plugin was given. The file is told apart by how it was
found, because a file the operator names with `--config` is theirs, and the
`sbase_config.toml` of the current folder may be a cloned project's.

<ResponseField name="PluginSource" type="enum">

<Expandable title="variants, with describe()" defaultOpen>

<ResponseField name="ProjectFile" type="unit variant">The `[plugins]` table of the settings file found in the current folder: `the settings file in the current folder`.</ResponseField>

<ResponseField name="ConfigFile" type="unit variant">The `[plugins]` table of the file `--config` names: `the settings file named by --config`.</ResponseField>

<ResponseField name="Environment" type="unit variant">The `SB_PLUGINS` variable: `SB_PLUGINS`.</ResponseField>

<ResponseField name="CommandLine" type="unit variant">A `--plugin` flag: `--plugin`.</ResponseField>

</Expandable>

</ResponseField>

## PluginOrigin

<Badge color="blue">struct</Badge> `seleniumbase_rs::cli::plugin_cmd::PluginOrigin`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginOrigin {
    pub name: String,
    pub source: PluginSource,
}
```

<ParamField body="name" type="String">

The plugin's name.

</ParamField>

<ParamField body="source" type="PluginSource">

Where the name came from; the strongest place wins.

</ParamField>

## check_selection

<Badge color="green">fn</Badge> `seleniumbase_rs::cli::plugin_cmd::check_selection`

```rust
pub fn check_selection(selection: &PluginSelection) -> Result<(), SeleniumBaseError>
```

<ParamField body="selection" type="&PluginSelection" required>

The selection to check.

</ParamField>

<ResponseField name="return" type="Result<(), SeleniumBaseError>">

`SeleniumBaseError::InvalidConfig` (exit status 2) naming the mistake.

</ResponseField>

Checks a selection the way a command-line program should before it starts a
browser: this build can run plugins (`ensure_supported`), and, with the feature
`plugins`, `check_selection_with` against the built-in registry. Without the
feature it refuses any non-empty selection.

## check_selection_with

<Badge color="green">fn</Badge> <Badge color="blue">Feature: plugins</Badge> `seleniumbase_rs::cli::plugin_cmd::check_selection_with`

```rust
pub fn check_selection_with(
    registry: &Registry,
    selection: &PluginSelection,
) -> Result<(), SeleniumBaseError>
```

<ParamField body="registry" type="&Registry" required>

The plugins that can be named.

</ParamField>

<ParamField body="selection" type="&PluginSelection" required>

The selection to check.

</ParamField>

<ResponseField name="return" type="Result<(), SeleniumBaseError>">

`SeleniumBaseError::InvalidConfig` for an unknown name (``no plugin named
`NAME` (known: ...); `sbrs plugin list` shows them``), and for every error
`plugin_host::build_with` returns, as ``plugin `NAME`: REASON``.

</ResponseField>

## check_origins_with

<Badge color="green">fn</Badge> <Badge color="blue">Feature: plugins</Badge> `seleniumbase_rs::cli::plugin_cmd::check_origins_with`

```rust
pub fn check_origins_with(
    registry: &Registry,
    selection: &PluginSelection,
    origins: &[PluginOrigin],
) -> Result<(), SeleniumBaseError>
```

<ParamField body="registry" type="&Registry" required>

The plugins that can be named.

</ParamField>

<ParamField body="selection" type="&PluginSelection" required>

`PluginChoice::selection`: each plugin is built with its options, to read
what the built plugin declares.

</ParamField>

<ParamField body="origins" type="&[PluginOrigin]" required>

`PluginChoice::origins`.

</ParamField>

<ResponseField name="return" type="Result<(), SeleniumBaseError>">

`SeleniumBaseError::InvalidConfig` naming the plugin, the capabilities and how
to name the plugin yourself.

</ResponseField>

Refuses a plugin named **only** by the settings file of the current folder
(`PluginSource::ProjectFile`) when it lists `launch.proxy`, `artifacts.write`
or a `host.*` capability, in `capabilities` or `optional`, either in its
descriptor's metadata (`describe()`) or in the metadata of the plugin built
with its options from `selection`: an option can add a capability, as the
crawler's `state_file` and the batcher's `resume_file` add `host.fs.*` for
their file. `--plugin`, `SB_PLUGINS` and a file named with
`--config` count as the operator choosing. A name the registry does not have is
left to `check_selection_with`.

## apply_to_config

<Badge color="green">fn</Badge> `seleniumbase_rs::cli::plugin_cmd::apply_to_config`

```rust
pub fn apply_to_config(
    config: BrowserConfig,
    selection: &PluginSelection,
) -> Result<BrowserConfig, SeleniumBaseError>
```

<ParamField body="config" type="BrowserConfig" required>

The configuration of a WebDriver session.

</ParamField>

<ParamField body="selection" type="&PluginSelection" required>

The plugins to give it.

</ParamField>

<ResponseField name="return" type="Result<BrowserConfig, SeleniumBaseError>">

`BrowserConfig::with_plugin_selection` with the feature `plugins`; without it,
`SeleniumBaseError::InvalidConfig` for a non-empty selection instead of leaving
it out of the program.

</ResponseField>

## list

<Badge color="green">fn</Badge> `seleniumbase_rs::cli::plugin_cmd::list`

```rust
#[must_use]
pub fn list(selection: &PluginSelection) -> String
```

<ParamField body="selection" type="&PluginSelection" required>

What the file, the variable and the flags turn on.

</ParamField>

<ResponseField name="return" type="String">

What `sbrs plugin list` prints: `list_with` of the built-in registry, or, in a
build without the feature, `No plugins: this build has no built-in plugin;
rebuild with --features plugins-builtin (...)`.

</ResponseField>

## list_with

<Badge color="green">fn</Badge> <Badge color="blue">Feature: plugins</Badge> `seleniumbase_rs::cli::plugin_cmd::list_with`

```rust
#[must_use]
pub fn list_with(registry: &Registry, selection: &PluginSelection) -> String
```

<ParamField body="registry" type="&Registry" required>

The plugins to list, in registry order.

</ParamField>

<ParamField body="selection" type="&PluginSelection" required>

What is turned on.

</ParamField>

<ResponseField name="return" type="String">

A table with the columns `NAME`, `CATEGORY`, `ENABLED` and `SUMMARY`, the names
the selection enables that the registry does not have (`Plugins enabled but
not in this build: ...`), and a line on how to turn one on.

</ResponseField>

## info

<Badge color="green">fn</Badge> `seleniumbase_rs::cli::plugin_cmd::info`

```rust
pub fn info(name: &str, selection: &PluginSelection) -> Result<String, SeleniumBaseError>
```

<ParamField body="name" type="&str" required>

A plugin name.

</ParamField>

<ParamField body="selection" type="&PluginSelection" required>

What is turned on, with the options in effect.

</ParamField>

<ResponseField name="return" type="Result<String, SeleniumBaseError>">

What `sbrs plugin info NAME` prints: `info_with` of the built-in registry.
Without the feature, always `SeleniumBaseError::InvalidConfig`.

</ResponseField>

## info_with

<Badge color="green">fn</Badge> <Badge color="blue">Feature: plugins</Badge> `seleniumbase_rs::cli::plugin_cmd::info_with`

```rust
pub fn info_with(
    registry: &Registry,
    name: &str,
    selection: &PluginSelection,
) -> Result<String, SeleniumBaseError>
```

<ParamField body="registry" type="&Registry" required>

The plugins that can be named.

</ParamField>

<ParamField body="name" type="&str" required>

The plugin to describe.

</ParamField>

<ParamField body="selection" type="&PluginSelection" required>

What is turned on, with the options in effect.

</ParamField>

<ResponseField name="return" type="Result<String, SeleniumBaseError>">

The description, or `SeleniumBaseError::InvalidConfig` for a name the registry
does not have, listing the ones it has.

</ResponseField>

The text (see the ResponseExample) gives the name and version, the summary,
then one row each for `category`, `enabled`, `phase` (the declared phase, or
the category's), `needs` (the requirements in words), `capabilities` and
`optional` (each with its risk and `enforced` or `declared only`), `requires`,
`conflicts`, `runs after`, `runs before`, `seals` and `options`. The metadata
comes from the descriptor (`describe()`); `needs` comes from the plugin built
with the selection's options, or with no options when it refuses them, and the
`options` row then quotes the plugin's own message. A note on `declared only`
ends the text when a capability is declared only.
