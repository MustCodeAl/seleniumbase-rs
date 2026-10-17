---
title: "Choosing Plugins"
---

# Choosing plugins

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugins</Badge>

The official plugins ship with seleniumbase-rs, one cargo feature each, and you
turn them on **by name**, with no code: in the settings file, in an environment
variable, or in a builder call. This page is that surface. For writing a plugin
of your own, see [Writing your own](writing-your-own.md).

<Note>

There are no command-line flags for this yet (`--plugin`, `--plugin-opt`,
`sbrs plugin list`), and the `sbrs` binary does **not** apply the `[plugins]`
table or `SB_PLUGINS` yet: it is about to move into a crate of its own, and its
plugin flags and handling come with the move. This page is for Rust programs
that load `Settings` and build their own browser. A build without the `plugins`
feature stops at `Settings::load` when the table or `SB_PLUGINS` names a plugin;
a build with it reads them and leaves the selection to the program.

</Note>

## The short version

| You want | Do |
|---|---|
| shield in a Rust program | `features = ["plugin-shield"]`, then `.plugins_named(["shield"])` |
| Every official plugin | `features = ["plugins-builtin"]` |
| A plugin chosen by a file, not code | `[plugins] enable = ["shield"]` in `sbase_config.toml` |
| A plugin for one run | `SB_PLUGINS=shield` |
| A plugin from another crate | Add the crate, then `PluginSet::with` (see [Writing your own](writing-your-own.md)) |

## Cargo features

| Feature | Turns on |
|---|---|
| `plugins` | The plugin API (`seleniumbase_rs::plugin_api`) and the hooks of both engines. No plugin of its own |
| `plugin-shield` | `plugins`, and the [shield](shield.md) plugin |
| `plugin-block-resources` | `plugins`, and the [block-resources](block-resources.md) plugin |
| `plugin-crx` | `plugins`, and the [crx](crx.md) plugin |
| `plugins-builtin` | All three above |

A build compiles only the plugin crates it asks for: `plugin-shield` does not
also compile the CRX unpacker. The plugins live in the
[plugins repository](https://github.com/MustCodeAl/seleniumbase-rs-plugins), and
this crate depends on two crates from it, the API and the aggregator of the
built-in plugins.

<Warning>

Both dependencies name **one** git URL and **one** commit (`rev` in
`Cargo.toml`). Cargo counts a git repository under two references (a branch, a
tag, a revision) as two sources, so a plugin crate added with a different
reference builds a second copy of `seleniumbase-rs-plugin`, and its `Plugin` is
not the trait this crate calls. The compiler then says the trait `Plugin` is not
implemented and mentions two versions of `seleniumbase_rs_plugin`. Add a plugin
crate with the same revision:

```sh
cargo add --git https://github.com/MustCodeAl/seleniumbase-rs-plugins \
    --rev 628991af0e6c5711298e3bd3692dc2974887401a \
    seleniumbase-rs-plugin-foo
```

A test in this repository (`tests/plugin_features.rs`) fails when the two
dependencies drift apart.

</Warning>

## In the settings file

The `[plugins]` table of `sbase_config.toml` (or the `plugins` object of a JSON
`.sbase_config`) lists the plugins to add, in order, and a table named like a
plugin holds that plugin's options:

```toml
[plugins]
enable = ["shield", "block-resources"]   # the order is the insertion order

[plugins.shield]
level = "strict"
extra_block_hosts = ["tracker.example"]
```

Options are the plugin's own; an option it does not know is an error. Load the
file with `Settings::load`, and `settings.plugins` holds the selection:

```rust,no_run
use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::sb_cdp::LaunchOptions;

let settings = Settings::load(None::<&str>)?;
let options = LaunchOptions::builder()
    .plugin_selection(&settings.plugins)
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

For the WebDriver engine, `BrowserConfig::with_plugin_selection`:

```rust,no_run
use seleniumbase_rs::config::settings::Settings;

let settings = Settings::load(None::<&str>)?;
let config = settings
    .to_browser_config()
    .with_plugin_selection(&settings.plugins)?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

## With `SB_PLUGINS`

`SB_PLUGINS` is a comma-separated list of names:

```sh
SB_PLUGINS=shield,block-resources cargo test
```

It replaces the `enable` list of the file. The options of a plugin the list still
names stay; the options of the others are dropped, so `SB_PLUGINS=` (set, but
empty) turns every plugin off and leaves nothing for the build to refuse. Spaces
around a name and empty entries are ignored. Like the other `SB_*` variables, it
is read by `Settings::load`, and it takes effect before the check for plugin
support, so in a build without the `plugins` feature `SB_PLUGINS=` still
overrides a `[plugins]` table of the file.

## In code

On the pure CDP engine, `LaunchOptionsBuilder` takes names beside the plugins you
build yourself:

```rust
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
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

`plugins_named` and `plugin_selection` only record the names. `build()` resolves
them and reports a mistake as `SeleniumBaseError::Plugin`. Named plugins are
added after the set given to `plugins(...)`, and run in the order their phases
give (privacy plugins run after the others). On the WebDriver engine,
`BrowserConfig::with_plugins_named` and `with_plugin_selection` do the same and
return the error at once, because a configuration has no `build()`.

A program with plugins of its own builds a registry:

```rust
use seleniumbase_rs::plugin_host;

// The official plugins this build has, by name. Add your own with
// `registry.with(Descriptor::of::<MyPlugin>())`, then:
let registry = plugin_host::builtin_registry();
let set = plugin_host::build_with(&registry, &Default::default())?;
assert!(set.is_empty());
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

A set built from names is **strict**: it refuses a plugin that declares no
metadata, and grants each plugin what it declares except Critical capabilities.

## Mistakes it catches

Nothing is skipped silently. Every case below stops the build, before a browser
starts:

| Mistake | What you get |
|---|---|
| A name no plugin has (`sheild`) | An error that says no plugin has that name, lists the names that are known, and names the cargo features that add the official ones |
| Options for a plugin that is not in `enable` (`[plugins.sheild]`, or `enabled = [...]`) | An error naming the table and saying it is not in `enable` |
| An option the plugin does not know | The plugin's own message, with the plugin's name |
| The same name twice | An error that says the plugin is already in the set |
| Names in a build without the `plugins` feature | `Settings::load` fails and says to rebuild with `--features plugins-builtin` |

The last row is deliberate: a program that asked for privacy hardening and
silently runs without it is worse than one that stops.

## Not here yet

- `--plugin`, `--plugin-opt` and `sbrs plugin list|info` (see the note at the top).
- Plugins that are not compiled in: WebAssembly modules and TOML manifests, and
  the store and lockfile that pin them.
- The `sbrs-mcp` server taking plugin names.
- The other official plugins of the design (tor, fetcher, extractor, browser,
  batcher, crawler).
