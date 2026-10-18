---
title: "Binary Patching"
---

Chromedriver and similar Chromium drivers embed static signatures that bot
detection scripts can read at runtime. The most famous are the `cdc_` variables
and quoted `$cdc_` strings that the driver injects into every page. The
`ChromedriverPatcher` edits the executable on disk before launch so those
markers are never injected in the first place.

This page covers both driver-level patching (`ChromedriverPatcher`) and
browser-level patching (`ChromeBinaryPatcher`), plus the engine spoofing
arguments that hide additional automation markers.

<Note>

Nothing here runs by itself for a chromedriver: UC mode does not patch the
driver, so patch it yourself (or use the [pure CDP engine](/tutorials/pure_cdp), which
has no driver to patch).

</Note>

<Info>

**What you will learn**

- Which markers `ChromedriverPatcher` removes.
- How to use `EnginePatch` presets and backups.
- How to patch from Rust code and from the CLI.
- How to combine patching with fingerprints and UC mode.
- Safety and licensing considerations.

</Info>

<Visibility for="agents">

Summary for AI exports: `ChromedriverPatcher::new(path)` removes the `cdc_` and webdriver markers from a chromedriver binary (`needs_patch()`, then `patch(EnginePatch::balanced())`; `restore()` copies the `.orig` backup back). `ChromeBinaryPatcher` patches a cached copy of the browser and never modifies the original. The CLI has `sbrs patch-chromedriver --path` and `sbrs patch-chrome --path`. Patch only executables you own or may modify.

</Visibility>

## What is patched

* `cdc_<22 alphanum>_` property assignments such as
  `window.cdc_..._Array = window.Array`.
* Quoted `$cdc_...` string prefixes.
* `__webdriver`, `__selenium`, and `__driver` globals.
* `{window.cdc_...;}` initialization blocks (the `all()` preset).
* Hard-coded `ChromeDriver/<version>` strings, and `navigator.webdriver = true`
  assignments (the `all()` preset).

## `ChromedriverPatcher` API

```rust,no_run
use seleniumbase_rs::{ChromedriverPatcher, EnginePatch};

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let patcher = ChromedriverPatcher::new("/path/to/chromedriver");

// Check whether known markers are still present.
if patcher.needs_patch()? {
    // balanced = conservative patches with a .orig backup
    patcher.patch(EnginePatch::balanced())?;
}
# Ok(())
# }
```

`EnginePatch` presets:

| Preset | Behavior |
|---|---|
| `EnginePatch::balanced()` | Scrubs `cdc_` props, randomizes prefixes, removes webdriver markers; creates a backup. |
| `EnginePatch::all()` | Everything in `balanced()` plus replacement of `{window.cdc...;}` blocks, the `ChromeDriver/<version>` strings and `navigator.webdriver` assignments; creates a backup. |
| `EnginePatch::aggressive()` | Currently the same set as `all()`. |
| `EnginePatch::no_backup()` | Same as `all()` but skips the `.orig` backup. Useful for ephemeral CI binaries. |
| `EnginePatch::chrome_binary()` | The set for the browser executable, used by `ChromeBinaryPatcher` (see below). Not meant for chromedriver. |

The patch is written to a temporary file next to the driver and renamed over it,
keeping the original permissions, so a driver that is running does not make it
fail and an interrupted patch cannot leave a half-written binary. On macOS,
changing a signed binary breaks its signature and the system then kills it as
it starts, so `patch` signs the driver again ad hoc afterwards (see
`seleniumbase_rs::browser::driver_access`).

## One-shot helper

If you only need the default patch, use the `patch_chromedriver` function, which
lives in the `stealth::patcher` module (it is not re-exported at the crate root):

```rust,no_run
use seleniumbase_rs::stealth::patcher::patch_chromedriver;

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
patch_chromedriver("/path/to/chromedriver")?;
# Ok(())
# }
```

It is equivalent to `ChromedriverPatcher::new(path).patch(EnginePatch::all())`,
and `sbrs patch-chromedriver --path /path/to/chromedriver` calls it.

## Backup and restore

By default `EnginePatch::balanced()` and `EnginePatch::all()` create a backup at
`<binary>.orig`. Only the first patch makes the backup: patching again never
replaces it with an already-patched file, so `restore` always returns the
original. You can restore the original binary later:

```rust,no_run
use seleniumbase_rs::ChromedriverPatcher;

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let patcher = ChromedriverPatcher::new("/path/to/chromedriver");
patcher.restore()?; // copies .orig back over the patched binary
# Ok(())
# }
```

<Note>

`restore` returns an error if there is no `.orig` file.

</Note>

## Engine spoofing arguments

Binary patching removes the injected JavaScript markers. You can further reduce
the engine-level automation fingerprint by passing extra Chromium flags. The
helper `engine_spoofing_args()` returns a list of flags such as
`--disable-blink-features=AutomationControlled` and disables background
networking, default apps, and similar telemetry.

<Tabs>

<Tab title="BrowserConfig" icon="gear">

Add them to a `BrowserConfig`:

```rust
use seleniumbase_rs::{engine_spoofing_args, BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_extra_args(engine_spoofing_args());
```

</Tab>

<Tab title="StealthOptions" icon="sliders">

Or apply them through `StealthOptions`:

```rust
use seleniumbase_rs::stealth::options::StealthOptions;
use seleniumbase_rs::engine_spoofing_args;
use thirtyfour::{DesiredCapabilities, BrowserCapabilitiesHelper};

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let mut opts = StealthOptions::default();
opts.extra_args = engine_spoofing_args();

let mut caps = DesiredCapabilities::chrome();
opts.apply_to(&mut caps)?;
assert!(caps.args().contains(&"--disable-blink-features=AutomationControlled".into()));
# Ok(())
# }
```

</Tab>

</Tabs>

For the strongest defense, combine binary patching, engine spoofing args, a
matching `Fingerprint`, and UC mode.

## Chrome/Chromium binary patching

`ChromeBinaryPatcher` copies the browser executable to a cache directory and
applies byte-level patches so no JavaScript override is needed for some
automation markers (`HeadlessChrome` in the user agent, `__webdriver`-style
globals, `navigator.webdriver = true` assignments). The original browser is
never modified. Because the browser binary itself returns the modified
values, page scripts cannot detect the spoof through `toString()` or property
descriptor inspection.

```rust,no_run
use seleniumbase_rs::{ChromeBinaryPatcher, EnginePatch};

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let patcher = ChromeBinaryPatcher::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
    .with_cache_dir("/tmp/sb-chrome-patches");
let patched = patcher.patch(EnginePatch::chrome_binary())?;
println!("Patched binary: {}", patched.display());
# Ok(())
# }
```

The patched copy goes under the cache directory in
`seleniumbase-rs/chrome-patches/<hash>/`, where the hash is derived from the
source path and its modification time. A copy that already exists is reused, so a
second call is instant and a changed Chrome gets a new copy. On macOS the copy's
code signature is removed (with `codesign --remove-signature`) so the system
accepts it.

When `native_spoofing` is enabled in a `Fingerprint` and
`BrowserConfig::browser_binary_path` is not set, the launcher automatically
patches the Chrome binary and passes the patched copy to chromedriver. It starts
from `SB_CHROME_BIN`, or else a Chrome it finds on the system (`CHROME_BIN`,
`CHROMIUM_BIN`, the usual install paths, then `PATH`), and caches the copy in
`SB_PATCH_CACHE_DIR` (default: the platform cache directory).

You can also patch from the CLI. Quote a path that contains spaces:

```bash Terminal
sbrs patch-chrome --path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
sbrs patch-chrome --path /usr/bin/google-chrome --cache-dir /tmp/sb-chrome-patches
```

Without `--cache-dir` the CLI caches under `sb-chrome-patches` in the platform
cache directory. `sbrs doctor` reports the Chrome binary it finds, where the
patched copy would be, and whether your chromedriver still has automation markers.

## Safety and licensing

<Warning>

Only patch executables that you own or have permission to modify. Patching a
system-installed driver or a binary belonging to another user may violate
licenses, local policies, or terms of service. Keep a backup (the default
`EnginePatch` presets create one automatically) and test the patched binary in a
non-production environment first.

</Warning>

## Troubleshooting

<AccordionGroup>

<Accordion title="needs_patch() still true after patching" icon="rotate">

**Likely cause:** Driver was replaced by a newer version.

**Fix:** Re-run the patcher after driver downloads (`sbrs install` downloads a fresh, unpatched one).

</Accordion>

<Accordion title="Browser crashes after Chrome binary patch" icon="bug">

**Likely cause:** Patch incompatible with Chromium version.

**Fix:** Delete the cached copy under `chrome-patches/` and use the original browser; the original is never modified.

</Accordion>

<Accordion title="Driver is killed as it starts on macOS" icon="apple">

**Likely cause:** Its signature no longer matches.

**Fix:** `patch` signs it again; if it still fails, run `codesign --force --sign - <driver>`.

</Accordion>

<Accordion title='restore fails with "no .orig backup found"' icon="clock-rotate-left">

**Likely cause:** Used `EnginePatch::no_backup()`.

**Fix:** Re-download the original binary before patching again.

</Accordion>

<Accordion title="Bot detection still triggers" icon="user-secret">

**Likely cause:** TLS or behavioral signals leak.

**Fix:** Combine patching with `Fingerprint`, UC mode, and proxy egress alignment.

</Accordion>

</AccordionGroup>

## Next steps

<Columns cols={3}>

<Card title="Pure CDP" icon="bolt" href="/tutorials/pure_cdp">

An engine with no driver to patch.

</Card>

<Card title="Undetected (UC) mode" icon="user-secret" href="/tutorials/uc_mode">

Combine patching with UC mode.

</Card>

<Card title="Fingerprint & Stealth Profiles" icon="fingerprint" href="/tutorials/fingerprint_stealth">

Combine patching with a matching `Fingerprint`.

</Card>

</Columns>
