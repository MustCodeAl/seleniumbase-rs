---
title: "Choosing Plugins"
sidebarTitle: "Choose plugins"
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugins</Badge>

The official plugins ship with seleniumbase-rs, one cargo feature each, and you
turn them on **by name**, with no code: on the command line, in the settings
file, in an environment variable, or in a builder call. This page is that
surface. For writing a plugin of your own, see
[Writing your own](/plugins/writing-your-own).

<Note>

The prebuilt `sbrs` and `sbrs-mcp`, and `cargo install` of
`seleniumbase-rs-cli`, have the official plugins compiled in, so there is
nothing to install: `sbrs --plugin shield open https://example.com` (see
[On the command line](#on-the-command-line)). The rest of the page is for Rust
programs that load `Settings` and build their own browser. A build without the
`plugins` feature stops at `Settings::load` when the table or `SB_PLUGINS` names
a plugin; a build with it reads them and leaves the selection to the program.

</Note>

<Visibility for="agents">

Summary for AI exports: the official plugins (`shield`, `block-resources`, `crx`) are turned on by name, with no code. On the command line, `--plugin NAME` and `--plugin-opt NAME.KEY=VALUE` are global options, so they go before the command or after it, and both repeat (`sbrs --plugin shield open https://example.com`); `VALUE` is a TOML value, so `'shield.level="strict"'` is quoted. Other sources are `SB_PLUGINS=shield,block-resources` and `enable = [...]` in the `[plugins]` table of `sbase_config.toml`. Precedence, highest first: the command line, `SB_PLUGINS`, the settings file. In Rust, turn on the feature (`plugin-shield`, `plugin-block-resources`, `plugin-crx` or `plugins-builtin`) and use `plugins_named` or `plugin_selection`. A wrong name or option stops the run before a browser starts, with exit status 2. `sbrs plugin list` and `sbrs plugin info NAME` show the plugins and what they may do.

</Visibility>

## The short version

| You want | Do |
|---|---|
| shield for one `sbrs` command | `sbrs --plugin shield open https://example.com` |
| An option of a plugin on the command line | `--plugin-opt 'shield.level="strict"'` |
| The plugins and what they may do | `sbrs plugin list`, `sbrs plugin info shield` |
| shield in a Rust program | `features = ["plugin-shield"]`, then `.plugins_named(["shield"])` |
| Every official plugin | `features = ["plugins-builtin"]` |
| A plugin chosen by a file, not code | `[plugins] enable = ["shield"]` in `sbase_config.toml` |
| A plugin for one run | `SB_PLUGINS=shield` |
| A plugin from another crate | Add the crate, then `PluginSet::with` (see [Writing your own](/plugins/writing-your-own)) |

## Cargo features

| Feature | Turns on |
|---|---|
| `plugins` | The plugin API (`seleniumbase_rs::plugin_api`) and the hooks of both engines. No plugin of its own |
| `plugin-shield` | `plugins`, and the [shield](/plugins/shield) plugin |
| `plugin-block-resources` | `plugins`, and the [block-resources](/plugins/block-resources) plugin |
| `plugin-crx` | `plugins`, and the [crx](/plugins/crx) plugin |
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

```sh Terminal
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

```toml sbase_config.toml
[plugins]
enable = ["shield", "block-resources"]   # the order is the insertion order

[plugins.shield]
level = "strict"
extra_block_hosts = ["tracker.example"]
```

Options are the plugin's own; an option it does not know is an error. Load the
file with `Settings::load`, and `settings.plugins` holds the selection:

<Tabs>

<Tab title="Pure CDP engine" icon="bolt">

```rust,no_run
use seleniumbase_rs::config::settings::Settings;
use seleniumbase_rs::sb_cdp::LaunchOptions;

let settings = Settings::load()?;
let options = LaunchOptions::builder()
    .plugin_selection(&settings.plugins)
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

</Tab>

<Tab title="WebDriver engine" icon="server">

For the WebDriver engine, `BrowserConfig::with_plugin_selection`:

```rust,no_run
use seleniumbase_rs::config::settings::Settings;

let settings = Settings::load()?;
let config = settings
    .to_browser_config()
    .with_plugin_selection(&settings.plugins)?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

</Tab>

</Tabs>

## With `SB_PLUGINS`

`SB_PLUGINS` is a comma-separated list of names:

```sh Terminal
SB_PLUGINS=shield,block-resources cargo test
```

It replaces the `enable` list of the file. The options of a plugin the list still
names stay; the options of the others are dropped, so `SB_PLUGINS=` (set, but
empty) turns every plugin off and leaves nothing for the build to refuse. Spaces
around a name and empty entries are ignored. Like the other `SB_*` variables, it
is read by `Settings::load`, and it takes effect before the check for plugin
support, so in a build without the `plugins` feature `SB_PLUGINS=` still
overrides a `[plugins]` table of the file.

## On the command line

`sbrs` and `sbrs-mcp` apply the `[plugins]` table and `SB_PLUGINS` to every
browser they start, and take two options of their own. Both go **before** the
command, as the other global options of `sbrs` do, and both repeat:

<ParamField body="--plugin NAME">

Turns the plugin on, after the ones the file and `SB_PLUGINS` turn on. A name that is on already stays where it is.

</ParamField>

<ParamField body="--plugin-opt NAME.KEY=VALUE">

Sets one option of a plugin. `VALUE` is a TOML value, so a string is written with quotes.

</ParamField>

```sh Terminal
sbrs --plugin shield open https://example.com
sbrs --plugin shield --plugin-opt 'shield.level="strict"' open https://example.com
sbrs --plugin block-resources --plugin-opt 'block-resources.preset="media"' screenshot
sbrs --plugin shield --plugin-opt 'shield.extra_block_hosts=["tracker.example"]' devtools
```

Put the whole option in single quotes, so the shell keeps the double quotes TOML
needs. `shield.level=strict` is not a TOML value; `sbrs` says so and shows the
quoted spelling. `NAME` ends at the first `.`, and `KEY` is the option's name.

### Precedence

Highest first:

1. **The command line.** `--plugin` adds a name; `--plugin-opt` sets one key.
2. **`SB_PLUGINS`**, which replaces the names the file enables, and drops the
   options of the plugins it no longer names (after the flags are applied, so a
   `--plugin` that turns such a plugin back on keeps the options the file gave
   it).
3. **The `[plugins]` table of the settings file** (`--config FILE`, otherwise
   `sbase_config.toml` in the current folder).

A `--plugin-opt` replaces the same key from the variable or the file and keeps
the plugin's other options. With `level = "basic"` and `extra_block_hosts = [...]`
under `[plugins.shield]` in the file, `--plugin-opt 'shield.level="strict"'`
runs shield at the strict level with the file's hosts. The plugin has to be on:
an option for a plugin that nothing turns on is an error, not an option that is
quietly dropped. The same text is in `sbrs --help`.

### Which commands

| Command | What happens |
|---|---|
| Every command that starts a browser with `BaseCase` (`open`, `smoke`, `screenshot`, `record`, `replay`, `run-scenario`, `print-pdf`, ...) | The WebDriver engine runs the plugins (`BrowserConfig::with_plugin_selection`) |
| `sbrs devtools` | The pure CDP engine runs them (`LaunchOptionsBuilder::plugin_selection`) |
| `sbrs test` | Passes the names on to the tests as `SB_PLUGINS`, with the names the file and the variable already turn on, for a test that calls `with_plugin_selection(&settings.plugins)`. It refuses `--plugin-opt`, because the variable carries names only: put the options under `[plugins.NAME]` in the file |
| `sbrs-mcp` | Every browser the server starts gets them; see below |
| `sbrs plugin list`, `sbrs plugin info` | Show them; see below |
| Commands that start no browser (`version`, `doctor`, `install`, `mkfile`, ...) | Ignore the options, even a wrong one: `sbrs --plugin 'a b' version` prints the version |

`sbrs devtools`, `sbrs plugin list`, `sbrs plugin info` and `sbrs-mcp` read the
`[plugins]` table of the settings file and nothing else of it, so a variable
that is wrong for a browser (`SB_HEADLESS=maybe`) or a value of the file that is
(`headless = "maybe"`) does not stop them.

### What a project file may not turn on alone

<Warning>

The `sbase_config.toml` of the current folder may be a cloned project's, so it
cannot grant a plugin the powers that reach beyond the browser.

</Warning>

A plugin that declares `launch.proxy`, `artifacts.write` or a `host.*`
capability (see `sbrs plugin info NAME`) and is named **only** by that file is
refused, with exit status 2 and the way to name it yourself (`tunnel` is an
example, not a built-in plugin):

```text
error: invalid configuration: plugin `tunnel` asks for launch.proxy, which the settings file in the current folder cannot grant by itself: a project you clone would decide. Name the plugin yourself, with `--plugin tunnel` or `SB_PLUGINS=tunnel`
```

`--plugin NAME`, `SB_PLUGINS` and a file named with `--config` count as the
person choosing. The other powers a plugin can declare (`launch.args`,
`launch.network`, `launch.extensions`, `page.script`, `cdp:*`) are not held back:
the settings file can already set an extension folder itself. Of the three built-in
plugins, `crx` declares one: `host.fs.write:temp`, to unpack an extension into
the temporary folder. So a settings file in the current folder cannot turn
`crx` on by itself; name it with `--plugin crx` or `SB_PLUGINS=crx`. A settings file of
the person (in their home folder), which would be trusted like `--config`, is
not built yet.

Every active plugin is logged at `INFO` with the place that turned it on, so a
file in the folder cannot switch one on unseen:

```text
INFO plugin enabled plugin=shield source="the settings file in the current folder"
INFO plugin enabled plugin=crx source="--plugin"
```

### sbrs plugin list and sbrs plugin info

```text
$ sbrs plugin list
NAME             CATEGORY   ENABLED  SUMMARY
shield           privacy    no       Turns off browser telemetry, WebRTC address leaks and third-party cookies; keeps no profile (clear_between_tests and sweep_stale_profiles are not active yet)
block-resources  core       no       Stops pages from loading images, fonts, media or URLs you name
crx              extension  no       Loads .crx extension files into the browser it starts

Turn one on with --plugin NAME, SB_PLUGINS, or `enable` in the [plugins] table of the settings file. `sbrs plugin info NAME` shows what a plugin may do.
```

`ENABLED` says whether the file, the variable and the options together turn the
plugin on. A name that is enabled but not in the build is listed below the table.

```text
$ sbrs --plugin shield --plugin-opt 'shield.level="strict"' plugin info shield
shield 0.1.2
Turns off browser telemetry, WebRTC address leaks and third-party cookies; keeps no profile (clear_between_tests and sweep_stale_profiles are not active yet)

category      privacy
enabled       yes
phase         enforce
needs         a Chromium-based browser
capabilities  launch.args     low     enforced
              launch.prefs    medium  enforced
              launch.network  high    enforced
              page.script:*   high    enforced
              cdp:Network     medium  enforced
optional      -
requires      -
conflicts     -
runs after    -
runs before   -
seals         -
options       level = "strict"
```

`info` shows what the plugin declares: its category and phase, each capability
with its risk and whether the host enforces it (`declared only` means a native
Rust plugin promises and the host cannot stop it), what it needs from the
browser, its place among the other plugins, and the options in effect. When the
plugin refuses those options, or cannot start without any (`crx` needs its
`files`), the page says so with the plugin's own message. The plugin API of this
release does not describe the options a plugin accepts, so the page cannot list
them: each plugin's page does ([shield](/plugins/shield),
[block-resources](/plugins/block-resources), [crx](/plugins/crx)).

An unknown name exits with status 2 and lists the ones the build has:

```text
$ sbrs plugin info sheild
error: invalid configuration: no plugin named `sheild` (known: shield, block-resources, crx); `sbrs plugin list` shows them
```

### sbrs-mcp

`sbrs-mcp` takes the same two options, and reads the same table (in the folder
it runs in, and only that table) and variable, and logs the active plugins as
`sbrs` does. **The operator chooses the plugins, not the model**:
`start_browser` has no plugin argument, and an extra argument adds nothing. All
three servers (`cdp`, `driver`, `sb`) give every browser they start the plugins,
in the pure CDP and the WebDriver engine:

```sh Terminal
sbrs-mcp --server cdp --plugin shield
sbrs-mcp --server driver --plugin shield --plugin-opt 'shield.level="strict"'
```

```json MCP client config
{
  "mcpServers": {
    "seleniumbase": {
      "command": "sbrs-mcp",
      "args": ["--server", "cdp", "--plugin", "shield"]
    }
  }
}
```

A mistake in the plugins stops the server before it listens, with the same
message and exit status as `sbrs`, not at the model's first `start_browser`.

### Mistakes and exit status

Nothing is skipped. These are reported **before any browser starts**, with exit
status 2 (the status `sbrs` gives every value that does not parse):

| Mistake | What you get |
|---|---|
| `--plugin sheild`, `SB_PLUGINS=sheild`, or `sheild` under `enable` | An error that says no plugin has that name and lists the ones the build has |
| `--plugin-opt` for a plugin nothing turns on | The option, the plugin, and the three ways to turn it on |
| An option the plugin does not know (`shield.lvel`) | The plugin's own message, with its name |
| A value that is not TOML (`shield.level=strict`) | The text, and the quoted spelling to use |
| `--plugin-opt` that is not `NAME.KEY=VALUE`, an empty `--plugin`, a comma in a name | What the option has to look like |
| The same option twice (`--plugin-opt a.b=1 --plugin-opt a.b=2`) | Which one |

### Builds

`plugins-builtin` is a default feature of `seleniumbase-rs-cli`, as `pdf` and
`mcp-server` are, and the release workflow builds with it. The package also
passes `plugins`, `plugin-shield`, `plugin-block-resources` and `plugin-crx`
through to the library. A build with `--no-default-features` has no plugin
support and **fails closed**: `--plugin`, a `[plugins]` table, or `SB_PLUGINS`
naming a plugin stops a browser command with status 2 and the text
`rebuild with --features plugins-builtin`; `sbrs plugin list` says there are
none; `sbrs plugin info` fails. An empty `SB_PLUGINS=` still turns the file's
plugins off. The Docker image is built without the default features and has no
plugins.

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
silently runs without it is worse than one that stops. The command-line programs
report the same mistakes with exit status 2 (see
[Mistakes and exit status](#mistakes-and-exit-status)).

## Not here yet

- `sbrs plugin add|remove|enable|disable|verify|sync|audit`, and the global
  options `--no-plugins` and `--plugins-dir`.
- Plugins that are not compiled in: WebAssembly modules and TOML manifests, and
  the store and lockfile that pin them.
- A `plugins` argument for the MCP `start_browser` tool (a subset of what the
  operator allowed), a `list_plugins` tool and `run_plugin_task`.
- A list of the options a plugin accepts in `sbrs plugin info`; the plugin API
  does not describe options yet.
- The other official plugins of the design (tor, fetcher, extractor, browser,
  batcher, crawler).
