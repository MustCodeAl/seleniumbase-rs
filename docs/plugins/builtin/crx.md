---
title: "crx Plugin (Browser Extensions)"
sidebarTitle: "crx"
---

The single `seleniumbase-rs` plugin for loading Chromium browser extensions from packed `.crx` (CRX3 and CRX2), `.zip` archives, and unpacked Manifest V3 folders, with optional service-worker readiness waits.

<Badge color="blue">Chromium only</Badge> <Badge color="blue">Launch only</Badge> <Badge color="blue">Feature: plugin-crx</Badge>

<Card color="#1a85d4" title="At a glance" icon="puzzle-piece">

- **Name:** `crx`
- **Formats:** CRX3, CRX2, `.zip`, and unpacked extension folders (`manifest_version: 3` enforced by default)
- **Worker waits:** optional `wait_for_workers` + `worker_timeout_secs` (`1..=30`, default `10`)
- **Unpacks to:** per-launch temporary folder (prefix `sbrs-crx-`), deleted when the browser closes
- **Guards:** 512 MiB unpacked limit, path-traversal check, 1 MiB `manifest.json` cap, refuses `--headless=old`
- **Composes with `browser`:** zero conflict; combine `--plugin crx --plugin browser` freely

</Card>

<Warning>

The cryptographic signature in a `.crx` header is **not** verified: load only extensions you trust.

</Warning>

## Use it

<Tabs>

<Tab title="sbase_config.toml" icon="gear">

```toml sbase_config.toml
[plugins]
enable = ["crx"]

[plugins.crx]
extension_zip = ["extensions/dark-reader.crx", "extensions/helper.zip"]
extension_dir = ["my-unpacked-extension"]
wait_for_workers = true
worker_timeout_secs = 15
```

</Tab>

<Tab title="Rust code" icon="code">

```rust,no_run
use seleniumbase_rs_plugin::PluginSet;
use seleniumbase_rs_plugin_crx::{CrxConfig, CrxExtensions};

let config = CrxConfig::builder()
    .zip("extensions/dark-reader.crx")
    .dir("my-unpacked-extension")
    .wait_for_workers(true)
    .build()?;
let plugins = PluginSet::strict().with(CrxExtensions::from_config(config)?)?;
Ok::<(), seleniumbase_rs_plugin::PluginError>(())
```

</Tab>

<Tab title="sbrs CLI" icon="terminal">

```sh Terminal
sbrs --plugin crx \
  --plugin-opt 'crx.extension_dir=["my-unpacked-extension"]' \
  --plugin-opt 'crx.wait_for_workers=true' \
  devtools
```

</Tab>

</Tabs>

## Options

| Option | Type | Default | Description |
|---|---|---|---|
| `extension_zip` | `string[]` or comma-separated `string` | `[]` | `.crx` (CRX3/CRX2) or `.zip` archives unpacked into per-launch temp dirs |
| `files` | `string[]` or comma-separated `string` | `[]` | Legacy alias for packed archive paths; merged before `extension_zip` |
| `extension_dir` | `string[]` or comma-separated `string` | `[]` | Unpacked Manifest V3 folders loaded in place without modification |
| `wait_for_workers` | `bool` | `false` | Wait in `on_browser` until every extension with `background.service_worker` reports a running worker |
| `worker_timeout_secs` | `u64` (`1..=30`) | `10` | Timeout in seconds for `wait_for_workers` |
| `require_manifest_v3` | `bool` | `true` | Enforce `"manifest_version": 3` on loaded extensions |

## Low-level helpers

`seleniumbase_rs_plugin_crx::archive(&[u8])` strips a CRX3 or CRX2 header and returns the inner zip slice, and `unpack(&[u8], &Path)` extracts a CRX archive into a directory with zip-bomb and path-traversal checks.
