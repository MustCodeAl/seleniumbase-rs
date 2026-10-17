---
title: "crx Plugin"
---

# crx

A seleniumbase-rs plugin that loads packed Chrome extensions (`.crx` files).

Chrome only loads unpacked extension folders from the command line. This
plugin takes `.crx` files (CRX3, the current format, and the older CRX2),
strips the signed header, unpacks the zip into a temporary folder and adds
the folder to the browser's start-up settings. The folders are deleted when
the browser closes.

```sh
cargo add seleniumbase-rs-plugin-crx --git https://github.com/MustCodeAl/seleniumbase-rs-plugins --rev 628991af0e6c5711298e3bd3692dc2974887401a
```

<Warning>

The signature in the header is **not** verified: load only files you trust,
as you would install them by hand.

</Warning>

## Use it

```rust,no_run
use seleniumbase_rs_plugin::PluginSet;
use seleniumbase_rs_plugin_crx::CrxExtensions;

let plugins = PluginSet::new().with(CrxExtensions::new(["extensions/dark-reader.crx"]))?;
# Ok::<(), seleniumbase_rs_plugin::PluginError>(())
```

Give the set to `LaunchOptionsBuilder::plugins`, as shown in the
[Browser Plugins guide](../tutorials/plugins.md#using-a-plugin).

## How it works

<Steps>

<Step title="Before the launch">

For each file, `before_launch` reads it, makes a temporary folder
(prefix `sbrs-crx-`), unpacks the zip archive inside the CRX file into it and
pushes the folder onto the launch settings' `extensions`. Unpacked archives above
512 MiB are refused, as a guard against zip bombs.

</Step>

<Step title="Loading">

The pure CDP engine loads the folders as extensions over the protocol, as
described in the [Browser Plugins guide](../tutorials/plugins.md#when-the-hooks-run).

</Step>

<Step title="On close">

`on_close` drops the folders, which deletes them.

</Step>

</Steps>

The plugin is named `crx`. It requires a Chromium browser that the plugin set
starts itself (`Requirements::CHROMIUM_ONLY | Requirements::LAUNCH_ONLY`), so it
cannot be used with a browser attached by `connect_with_plugins`.

## Errors

An unreadable file or an archive that cannot be unpacked stops the launch with
an error that names the file. `CrxError` says why a file could not be unpacked:

| Variant | Meaning |
|---|---|
| `CrxError::NotCrx` | The file does not start like a CRX file (`Cr24`), has an unknown version, or its header is cut short. |
| `CrxError::Archive` | The archive inside could not be read or written out. |

The public function `archive(&[u8])` returns the zip archive inside a CRX file
(CRX3 or CRX2), without its header, or `CrxError::NotCrx` when the bytes are not
a CRX file.
