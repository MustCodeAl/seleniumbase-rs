# Binary Patching

Chromedriver and similar Chromium drivers embed static signatures that bot
detection scripts can read at runtime. The most famous are the `cdc_` variables
and quoted `$cdc_` strings that the driver injects into every page. The
`ChromedriverPatcher` edits the executable on disk before launch so those
markers are never injected in the first place.

This page covers both driver-level patching (`ChromedriverPatcher`) and
browser-level patching (`ChromeBinaryPatcher`), plus the engine spoofing
arguments that hide additional automation markers.

Nothing here runs by itself for a chromedriver: UC mode does not patch the
driver, so patch it yourself (or use the [pure CDP engine](./pure_cdp.md), which
has no driver to patch).

## What you will learn

- Which markers `ChromedriverPatcher` removes.
- How to use `EnginePatch` presets and backups.
- How to patch from Rust code and from the CLI.
- How to combine patching with fingerprints and UC mode.
- Safety and licensing considerations.

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

`restore` returns an error if there is no `.orig` file.

## Engine spoofing arguments

Binary patching removes the injected JavaScript markers. You can further reduce
the engine-level automation fingerprint by passing extra Chromium flags. The
helper `engine_spoofing_args()` returns a list of flags such as
`--disable-blink-features=AutomationControlled` and disables background
networking, default apps, and similar telemetry.

Add them to a [`BrowserConfig`](crate::BrowserConfig):

```rust
use seleniumbase_rs::{engine_spoofing_args, BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_extra_args(engine_spoofing_args());
```

Or apply them through [`StealthOptions`](crate::stealth::options::StealthOptions):

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

For the strongest defense, combine binary patching, engine spoofing args, a
matching [`Fingerprint`](crate::Fingerprint), and UC mode.

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

```bash
sbrs patch-chrome --path "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
sbrs patch-chrome --path /usr/bin/google-chrome --cache-dir /tmp/sb-chrome-patches
```

Without `--cache-dir` the CLI caches under `sb-chrome-patches` in the platform
cache directory. `sbrs doctor` reports the Chrome binary it finds, where the
patched copy would be, and whether your chromedriver still has automation markers.

## Safety and licensing

Only patch executables that you own or have permission to modify. Patching a
system-installed driver or a binary belonging to another user may violate
licenses, local policies, or terms of service. Keep a backup (the default
`EnginePatch` presets create one automatically) and test the patched binary in a
non-production environment first.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `needs_patch()` still true after patching | Driver was replaced by a newer version | Re-run the patcher after driver downloads (`sbrs install` downloads a fresh, unpatched one). |
| Browser crashes after Chrome binary patch | Patch incompatible with Chromium version | Delete the cached copy under `chrome-patches/` and use the original browser; the original is never modified. |
| Driver is killed as it starts on macOS | Its signature no longer matches | `patch` signs it again; if it still fails, run `codesign --force --sign - <driver>`. |
| `restore` fails with "no .orig backup found" | Used `EnginePatch::no_backup()` | Re-download the original binary before patching again. |
| Bot detection still triggers | TLS or behavioral signals leak | Combine patching with `Fingerprint`, UC mode, and proxy egress alignment. |
