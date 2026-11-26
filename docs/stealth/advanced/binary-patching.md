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

Summary for AI exports: `ChromedriverPatcher::new(path)` inspects (`inspect()` -> `BinaryMarkerReport`) and removes `cdc_`, webdriver/DOM/extra globals, CDP flags, and optional `sourceURL` markers from a driver binary (`needs_patch()`, `verify_clean()`, `patch(spec)`, `restore()`). `ChromeBinaryPatcher` inspects and patches cached copies of the Chrome binary (`patched_path_for(&spec)`) and companion libraries (`patch_bundle(spec)` -> `PatchedBrowserBundle`), never modifying originals. Equal-length custom rules use `BinaryPatchRule::try_new` / `neutralize`. CLI: `sbrs patch-chromedriver --path [--inspect|--restore|--preset balanced|all|aggressive|chrome-binary]` and `sbrs patch-chrome --path [--inspect] [--preset ...] [--with-companions] [--cache-dir ...]`.

</Visibility>

## What is patched

* `cdc_<22 alphanum>_` property assignments (`window.cdc_..._Array = window.Array`).
* Quoted `$cdc_...` string prefixes and raw NUL-terminated `$cdc_<22 alphanum>_` tokens.
* `__webdriver`, `__selenium`, and `__driver` globals.
* `{window.cdc_...;}` initialization blocks (`all()` / `aggressive()`).
* Hard-coded `ChromeDriver/<version>` / `EdgeDriver/<version>` strings and `navigator.webdriver = true` / `!0` assignments (`all()` / `aggressive()`).
* Extended globals (`__fxdriver`, `__nightmare`, `_phantom`, `callPhantom`, `calledSelenium`, `$wdc_`, `__lastWatir`), `domAutomation` / `domAutomationController`, `HeadlessChrome`, `kAutomationControlled`, `enable-automation`, `webdriver-active`, `AutomationControlled`, and CDP evaluation script tokens (`__puppeteer_evaluation_script__`, `__playwright_evaluation_script__`).
* Automation `//# sourceURL=` comments naming `webdriver`, `cdc_`, `pptr:`, `puppeteer`, or `playwright` (`EnginePatch::aggressive()`).
* User-defined equal-length byte replacements via `BinaryPatchRule`.

## `ChromedriverPatcher` API

```rust,no_run
use seleniumbase_rs::{ChromedriverPatcher, EnginePatch, PatchPreset};

fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let patcher = ChromedriverPatcher::new("/path/to/chromedriver");

    let report = patcher.inspect()?;
    println!("{report}"); // format (ELF/Mach-O/PE), size, sha256, and per-category MarkerCounts

    if patcher.needs_patch()? {
        patcher.patch(EnginePatch::from_preset(PatchPreset::All))?;
        assert!(patcher.verify_clean()?);
    }
    Ok(())
}
```

`EnginePatch` presets (and `PatchPreset`):

| Preset | `PatchPreset` | Behavior |
|---|---|---|
| `EnginePatch::balanced()` | `PatchPreset::Balanced` | Scrubs `cdc_` props, randomizes prefixes, removes core webdriver markers; creates `.orig` backup. |
| `EnginePatch::all()` | `PatchPreset::All` | Adds `{window.cdc...;}` blocks, `ChromeDriver`/`EdgeDriver` versions, `navigator.webdriver`, `HeadlessChrome`, `domAutomation`, extended driver globals, `kAutomationControlled`, `enable-automation`, `webdriver-active`, and CDP evaluation markers; creates `.orig` backup. |
| `EnginePatch::aggressive()` | `PatchPreset::Aggressive` | Everything in `all()` plus neutralizing automation `//# sourceURL=` comments; creates `.orig` backup. |
| `EnginePatch::no_backup()` | — | Same flags as `all()` without creating `.orig` backup (CI/ephemeral binaries). |
| `EnginePatch::chrome_binary()` | `PatchPreset::ChromeBinary` | Browser binary preset for cached copies (`ChromeBinaryPatcher`), skipping chromedriver-only `cdc_` / version passes and disabling `.orig` backup. |

Custom equal-length rules preserve Mach-O, ELF, and PE section offsets:

```rust,no_run
use seleniumbase_rs::{BinaryPatchRule, ChromedriverPatcher, EnginePatch};

fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let spec = EnginePatch::aggressive()
        .custom_rule(BinaryPatchRule::neutralize(b"customAutomationFlag")?);
    ChromedriverPatcher::new("/path/to/chromedriver").patch(spec)?;
    Ok(())
}
```

The patch is written to a temporary file next to the driver and renamed over it,
keeping the original permissions, so a driver that is running does not make it
fail and an interrupted patch cannot leave a half-written binary. On macOS,
changing a signed binary breaks its signature and the system then kills it as
it starts, so `patch` signs the driver again ad hoc afterwards (see
`seleniumbase_rs::browser::driver_access`).

## One-shot helper

If you only need the default patch, use the `patch_chromedriver` function, which
lives in the `undetected::patcher` module (it is not re-exported at the crate root):

```rust,no_run
use seleniumbase_rs::undetected::patcher::patch_chromedriver;

fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    patch_chromedriver("/path/to/chromedriver")?;
    Ok(())
}
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

fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let patcher = ChromedriverPatcher::new("/path/to/chromedriver");
    patcher.restore()?; // copies .orig back over the patched binary
    Ok(())
}
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

let config = BrowserConfig::builder()
    .mode(DriverMode::Uc)
    .extra_args(engine_spoofing_args())
    .build()?;
```

</Tab>

<Tab title="StealthOptions" icon="sliders">

Or apply them through `StealthOptions`, whose `args()` is the launch it builds:

```rust
use seleniumbase_rs::browser::options::StealthOptions;
use seleniumbase_rs::engine_spoofing_args;

fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let mut opts = StealthOptions::default();
    opts.extra_args = engine_spoofing_args();
    assert!(opts
        .args()?
        .contains(&"--disable-blink-features=AutomationControlled".to_owned()));
    Ok(())
}
```

</Tab>

</Tabs>

For the strongest defense, combine binary patching, engine spoofing args, a
matching `Fingerprint`, and UC mode.

## Chrome/Chromium binary patching

`ChromeBinaryPatcher` copies the browser executable to a cache directory and
applies byte-level patches so no JavaScript override is needed for engine
markers (`HeadlessChrome` in the user agent, `__webdriver`-style globals,
`domAutomation` / `domAutomationController`, `navigator.webdriver` assignments,
`kAutomationControlled`, `enable-automation`, and CDP evaluation script names).
The original browser is never modified. Because the browser binary itself returns
the modified values, page scripts cannot detect the spoof through `toString()` or
property descriptor inspection.

```rust,no_run
use seleniumbase_rs::{ChromeBinaryPatcher, EnginePatch};

fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let patcher = ChromeBinaryPatcher::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
        .cache_dir("/tmp/sb-chrome-patches");
    let report = patcher.inspect()?;
    println!("{report}");

    // Patch primary executable only:
    let patched = patcher.patch(EnginePatch::chrome_binary())?;
    println!("Patched binary: {}", patched.display());

    // Or also discover and patch companion libraries (macOS Framework / Windows DLL / Linux .so):
    let bundle = patcher.patch_bundle(EnginePatch::aggressive())?;
    println!("Executable: {}, companion: {:?}", bundle.executable.display(), bundle.companion);
    Ok(())
}
```

The patched copy goes under the cache directory in
`seleniumbase-rs/chrome-patches/<hash>/`, where `patched_path_for(&spec)` hashes
the source path, its modification time, and the full `EnginePatch` fingerprint
(presets and custom rules) so distinct configurations never collide. A copy that
already exists is reused, so a second call is instant and an updated Chrome
gets a new copy. On macOS the copy's code signature is removed (with
`codesign --remove-signature`) so the system accepts it.

When `native_spoofing` is enabled in a `Fingerprint` and
`BrowserConfigBuilder::browser_binary_path` is not set, the launcher automatically
patches the Chrome binary and passes the patched copy to chromedriver. It starts
from `SB_CHROME_BIN`, or else a Chrome it finds on the system (`CHROME_BIN`,
`CHROMIUM_BIN`, the usual install paths, then `PATH`), and caches the copy in
`SB_PATCH_CACHE_DIR` (default: the platform cache directory).

You can inspect, patch, or restore from the CLI. Quote paths containing spaces:

```bash Terminal
sbrs patch-chromedriver --path /path/to/chromedriver --inspect
sbrs patch-chromedriver --path /path/to/chromedriver --preset aggressive
sbrs patch-chromedriver --path /path/to/chromedriver --restore
sbrs patch-chrome --path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --inspect --with-companions
sbrs patch-chrome --path /usr/bin/google-chrome --preset chrome-binary --cache-dir /tmp/sb-chrome-patches
sbrs patch-chrome --path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --with-companions
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

<Card color="#1a85d4" title="Pure CDP" icon="bolt" href="/tutorials/pure_cdp">

An engine with no driver to patch.

</Card>

<Card color="#199f43" title="Undetected (UC) mode" icon="user-secret" href="/tutorials/uc_mode">

Combine patching with UC mode.

</Card>

<Card color="#d47628" title="Fingerprint & Stealth Profiles" icon="fingerprint" href="/stealth/introduction">

Combine patching with a matching `Fingerprint`.

</Card>

</Columns>
