---
title: "browser Plugin"
sidebarTitle: "browser"
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugin-browser</Badge>

Loads browser extensions into the Chromium browser that seleniumbase-rs starts,
from `.zip` and `.crx` files and from unpacked folders. It is the plugin form of
Python SeleniumBase's `--extension-zip` and `--extension-dir`, with the same
option names and the same meaning.

<Note>

An extension is code that runs inside your browser, on every page. The plugin
loads the files you name and nothing else; it does not verify the signature of a
`.crx` file, so load only files you trust, as you would install them by hand.
Only Manifest V3 extensions are loaded. It needs a Chromium browser that the
program starts, not one it attaches to. It cannot share a set with
[crx](/plugins/crx), and a settings file in the current folder cannot turn it on
by itself: the operator names it, see [Who may turn it on](#who-may-turn-it-on).

</Note>

<Visibility for="agents">

Summary for AI exports: browser is a built-in extension plugin (feature `plugin-browser`, name `browser`) that loads extensions into a Chromium browser the program starts. Options live in `[plugins.browser]`: `extension_zip` (`.zip` or `.crx` files) and `extension_dir` (unpacked extension folders), each a list of paths or one text with the paths separated by commas, both default empty. An unknown option is an error, so the options of the design that are not built yet (`wait_for_workers`, `worker_timeout_secs`, `session`, `save_on_close`, `interactive`, `pause_message`) are refused, not ignored. Archives are unpacked into a temporary folder that belongs to one browser; folders are loaded where they are. The archives come first, then the folders, each list in the order given; the same path twice is loaded once. Every extension needs a `manifest.json` with `manifest_version` 3. Everything is checked before the browser starts: a missing path, a file of the wrong kind, a bad archive or a manifest that is not Manifest V3 fails the launch and the message names the option and the path. The signature of a `.crx` is not verified. It conflicts with `crx` (naming both is refused with "plugin `browser` cannot be combined with plugin `crx`"). It declares `launch.extensions`, `host.fs.read` for the paths and `host.fs.write:temp`, so only the operator can turn it on (`--plugin browser`, `SB_PLUGINS=browser`), never a settings file in the current folder alone. Nothing in it has run in a real Chrome.

</Visibility>

## Turn it on

browser ships with seleniumbase-rs behind one cargo feature, and is switched on
by its name. See [Choosing plugins](/plugins/selecting) for every way to do that.

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["plugin-browser"] }
```

Then switch it on by name and give it the extensions:

<Tabs>

<Tab title="sbase_config.toml" icon="gear">

```toml sbase_config.toml
[plugins.browser]
extension_zip = ["extensions/dark-reader.crx"]
extension_dir = ["my-extension"]
```

The table gives the options; the plugin itself is turned on by name too. A
settings file in the current folder cannot turn it on alone: name it with
`--plugin browser` or `SB_PLUGINS=browser`.

</Tab>

<Tab title="SB_PLUGINS" icon="terminal">

With no code, set `SB_PLUGINS=browser`, with the options in `[plugins.browser]`
of a settings file or in `--plugin-opt`.

</Tab>

<Tab title="sbrs command line" icon="terminal">

```sh Terminal
sbrs --plugin browser \
  --plugin-opt 'browser.extension_dir=["my-extension"]' \
  devtools
```

The value is a TOML value, so a list is written with brackets and quotes. As in
Python, one text with the paths separated by commas works too:
`--plugin-opt 'browser.extension_zip="a.zip,b.crx"'`.

</Tab>

<Tab title="Rust code" icon="code">

In code, through the registry of built-in plugins:

```rust
use seleniumbase_rs::config::plugins::PluginSelection;
use seleniumbase_rs::json::json;
use seleniumbase_rs::sb_cdp::LaunchOptions;

let selection = PluginSelection::new().enable("browser").with_options(
    "browser",
    json!({ "extension_zip": ["extensions/dark-reader.crx"], "extension_dir": ["my-extension"] }),
);
let options = LaunchOptions::builder()
    .headless(true)
    .plugin_selection(&selection)
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

Or build the plugin yourself:

```rust
use seleniumbase_rs::builtin_plugins::browser::{BrowserSession, SessionConfig};
use seleniumbase_rs::plugin_api::PluginSet;

let config = SessionConfig::new()
    .with_zip("extensions/dark-reader.crx")
    .with_dir("my-extension");
let plugins = PluginSet::new().with(BrowserSession::new(config)?)?;
# Ok::<(), seleniumbase_rs::plugin_api::PluginError>(())
```

</Tab>

</Tabs>

## What it does

In `before_launch`:

1. If an option is set and the launch has an explicit `--headless=old` switch, it
   refuses the launch: Chrome's old headless mode would lose the extensions
   without a word.
2. It resolves every path and checks it, in this order: the archives of
   `extension_zip`, then the folders of `extension_dir`, each list in the order
   you gave it, after any folder another plugin added. The same file or folder
   named twice is loaded once.
3. An archive is read as what its first bytes say, not what its name says: a
   CRX2 or CRX3 file (`Cr24`) or a ZIP file. It is unpacked, with the code of the
   [crx](/plugins/crx) plugin, into a temporary folder that belongs to one browser
   (prefix `sbrs-browser-`). Entries that would land outside the folder are
   refused, and so is an archive that unpacks to more than 512 MiB.
4. An unpacked folder is loaded where it is, and is never changed or deleted.
5. Every extension, archive or folder, must have a `manifest.json` of at most
   1 MiB with `"manifest_version": 3`. The check reads plain JSON: a byte order
   mark is accepted, a manifest with `//` comments is refused as not valid JSON.
6. It adds the folders to the launch settings. It adds no switch: seleniumbase-rs
   joins them into Chrome's `--load-extension` list, or loads them over the
   protocol where branded Chrome ignores that switch. Python does not add
   `--disable-extensions-except` either, and neither does the plugin.

In `on_close` it drops its temporary folders, which deletes them. Each browser of
a pool gets a copy of the plugin of its own, so closing one browser deletes only
its own folders.

## Options

`[plugins.browser]` in a settings file, or
`--plugin-opt 'browser.extension_dir=["my-extension"]'`. An option that is not
listed is an error.

<ParamField body="extension_zip" type="string[] or string" default="[]">

`.zip` or `.crx` files: a list, or one text with the paths separated by commas
(Python's form). An empty text means none.

</ParamField>

<ParamField body="extension_dir" type="string[] or string" default="[]">

Unpacked extension folders: a list, or one text with the paths separated by
commas. A folder whose path holds a comma is refused, because Chrome's list of
extension folders is split there.

</ParamField>

Not built yet: the design also has `wait_for_workers`, `worker_timeout_secs`,
`session`, `save_on_close`, `interactive` and `pause_message`. They need hooks the
plugin API does not have (a clock in `on_browser`, a folder for saved sessions),
so they are refused as unknown options instead of being accepted and ignored,
for example ``plugin configuration: browser: unknown option `wait_for_workers` ``.

## Messages

A mistake in the paths stops the launch before the browser starts. The message
names the option and the path as you wrote it, in the form
``browser: OPTION `PATH`: WHAT``:

| WHAT | When |
|---|---|
| `does not exist` | The path is not there |
| `cannot be read: ERROR` | The path or the file cannot be read |
| `is a folder; use extension_dir for an unpacked extension` | A folder in `extension_zip` |
| `is a file; use extension_zip for a .zip or .crx file` | A file in `extension_dir` |
| `is not a regular file` | In `extension_zip`, something that is neither a file nor a folder |
| `is larger than 512 MiB` | An archive over the limit |
| `is neither a CRX nor a ZIP file` | The first bytes are neither |
| `cannot be unpacked: ERROR` or the unpacker's own message | A bad archive, or an entry outside the folder |
| `manifest.json is missing` | No manifest in the folder or the archive |
| `manifest.json is larger than 1 MiB` | A manifest that cannot be one |
| `manifest.json is not valid JSON: ERROR` | A manifest with a comment, say |
| `manifest.json is not a JSON object` | A manifest that is a list or a text |
| `manifest.json has no manifest_version; only Manifest V3 extensions are loaded` | No version |
| `manifest_version is 2; only Manifest V3 extensions are loaded` | Any version but 3 |

Nothing is kept when one extension is refused: the folders unpacked so far are
deleted. Other messages:

| Mistake | Message |
|---|---|
| An empty entry in a list | `` plugin configuration: browser: `extension_zip` has an empty entry `` |
| A folder with a comma in its path | `` plugin configuration: browser: `extension_dir` entry `PATH` holds a comma, which Chrome's list of extension folders cannot carry; rename the folder `` |
| `--headless=old` with extensions | `` browser: extensions cannot be loaded in Chrome's old headless mode (--headless=old); use the new headless mode, which supports them `` |
| `crx` in the same set | `` plugin `browser` cannot be combined with plugin `crx` `` |

## Who may turn it on

The plugin declares `host.fs.write:temp` (to unpack archives) and, with paths,
`host.fs.read` for them. A plugin that declares a `host.*` capability and is named
only by the settings file of the current folder is refused, with exit status 2:

```text
error: invalid configuration: plugin `browser` asks for host.fs.write:temp, which the settings file in the current folder cannot grant by itself: a project you clone would decide. Name the plugin yourself, with `--plugin browser` or `SB_PLUGINS=browser`
```

`--plugin browser`, `SB_PLUGINS=browser` and a file named with `--config` count as
the person choosing ([why](/plugins/selecting#what-a-project-file-may-not-turn-on-alone)).
For an MCP server the operator chooses too: `sbrs-mcp --plugin browser` with the
options, and the model has no argument that adds a plugin.

## What it may do

`sbrs plugin info browser` shows what the plugin declares. It is an `extension`
plugin for Chromium browsers that the program starts (`Requirements::CHROMIUM_ONLY
| Requirements::LAUNCH_ONLY`):

| Capability | Why |
|---|---|
| `launch.extensions` | It adds extension folders to the launch settings |
| `host.fs.read` for the paths you configured | It reads the archives, the manifests and the folders |
| `host.fs.write:temp` | It unpacks archives into a temporary folder; the configured plugin asks for it only when you configured archives |

It declares that it cannot share a set with `crx`: naming both builds the set, and
the check every launch makes refuses the pair, whichever name comes first, before
a browser starts. The `Extension` category runs in the `Normal` phase and
`Privacy` in the later `Enforce` phase, so a privacy plugin such as
[tor](/plugins/tor) sees the launch after this plugin, whatever order the plugins
were added in.

## Test it

The plugin crate depends on the plugin API only, so a test needs no browser. The
plugin's own tests drive it through a plugin set, with archives built in memory
and a temporary folder for each test:

```rust
use seleniumbase_rs_plugin::{LaunchSettings, Plugin};
use seleniumbase_rs_plugin_browser::{BrowserSession, SessionConfig};
use seleniumbase_rs_plugin_testing::block_on;

// `my-extension` is a folder with a Manifest V3 `manifest.json`.
let plugin = BrowserSession::new(SessionConfig::new().with_dir("my-extension"))?;
let mut launch = LaunchSettings::default();
block_on(plugin.before_launch(&mut launch))?;
assert_eq!(launch.extensions.len(), 1);
# Ok::<(), seleniumbase_rs_plugin::PluginError>(())
```

## Limits

<Warning>

**Nothing here has run in a real Chrome.** The plugin's tests use a plugin set,
archives built in memory and temporary folders; they never start a browser. What
Chrome does with the folders, and the `--load-extension` list itself, belong to
seleniumbase-rs, and its tests of that part run on a scripted mock browser (see
[What has and has not been verified](/tutorials/plugins#what-has-and-has-not-been-verified)). Check that your
extension is active in the browser (open its page, or `chrome://extensions`)
before you rely on it.

</Warning>

- **Headless.** Chrome's old headless mode cannot load extensions; its new mode
  (`--headless=new`, which seleniumbase-rs starts) can. Python loads the
  extensions you name whatever the mode, and so does this plugin; the plugin API
  has one `headless` flag and cannot tell the two modes apart. The only case it
  refuses is an explicit `--headless=old` switch.
- **Launch only.** A browser the program attaches to cannot be given extensions
  at start-up, so it is refused.
- **Conflicts with `crx`.** One plugin loads extensions for each set. This one is
  a superset of `crx` (it also takes `.zip` files and folders and checks the
  manifest), so pick it or `crx`, not both.
- **Manifest V3 only.** A Manifest V2 extension is refused with the message above,
  not loaded.
- **The signature of a `.crx` is not verified.** Load only files you trust.
- **It is not a sandbox.** Like every native plugin, browser is Rust code that
  runs with your program's rights, and an extension it loads runs in your browser.
