# Binary Patching

Chromedriver and similar Chromium drivers embed static signatures that bot
detection scripts can read at runtime. The most famous are the `cdc_` variables
and quoted `$cdc_` strings that the driver injects into every page. The
`ChromedriverPatcher` edits the executable on disk before launch so those
markers are never injected in the first place.

This page covers both driver-level patching (`ChromedriverPatcher`) and
browser-level patching (`ChromeBinaryPatcher`), plus the engine spoofing
arguments that hide additional automation markers.

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
* `{window.cdc_...;}` initialization blocks.

## `ChromedriverPatcher` API

```rust
use seleniumbase_rs::{ChromedriverPatcher, EnginePatch};

let patcher = ChromedriverPatcher::new("/path/to/chromedriver");

// Check whether known markers are still present.
if patcher.needs_patch()? {
    // balanced = conservative patches with a .orig backup
    patcher.patch(EnginePatch::balanced())?;
}
```

`EnginePatch` presets:

| Preset | Behavior |
|---|---|
| `EnginePatch::balanced()` | Scrubs `cdc_` props, randomizes prefixes, removes webdriver markers; creates a backup. |
| `EnginePatch::all()` | Everything in `balanced()` plus replacement of `{window.cdc...;}` blocks. |
| `EnginePatch::no_backup()` | Same as `all()` but skips the `.orig` backup. Useful for ephemeral CI binaries. |

## One-shot helper

If you only need the default patch, use the `patch_chromedriver` function:

```rust
use seleniumbase_rs::patch_chromedriver;

patch_chromedriver("/path/to/chromedriver")?;
```

It is equivalent to `ChromedriverPatcher::new(path).patch(EnginePatch::all())`.

## Backup and restore

By default `EnginePatch::balanced()` and `EnginePatch::all()` create a backup at
`<binary>.orig`. You can restore the original binary later:

```rust
use seleniumbase_rs::ChromedriverPatcher;

let patcher = ChromedriverPatcher::new("/path/to/chromedriver");
patcher.restore()?; // copies .orig back over the patched binary
```

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

let mut opts = StealthOptions::default();
opts.extra_args = engine_spoofing_args();

let mut caps = DesiredCapabilities::chrome();
opts.apply_to(&mut caps)?;
assert!(caps.args().contains(&"--disable-blink-features=AutomationControlled".into()));
```

For the strongest defense, combine binary patching, engine spoofing args, a
matching [`Fingerprint`](crate::Fingerprint), and UC mode.

## Chrome/Chromium binary patching

`ChromeBinaryPatcher` copies the browser executable to a cache directory and
applies byte-level patches so no JavaScript override is needed for some
automation markers. Because the browser binary itself returns the modified
values, page scripts cannot detect the spoof through `toString()` or property
descriptor inspection.

```rust
use seleniumbase_rs::{ChromeBinaryPatcher, EnginePatch};

let patcher = ChromeBinaryPatcher::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
    .with_cache_dir("/tmp/sb-chrome-patches");
let patched = patcher.patch(EnginePatch::chrome_binary())?;
println!("Patched binary: {}", patched.display());
```

When `native_spoofing` is enabled in a `Fingerprint`, the launcher automatically
patches the Chrome binary and passes the patched copy to chromedriver.

You can also patch from the CLI:

```bash
sbase patch-chrome --path /Applications/Google Chrome.app/Contents/MacOS/Google Chrome
sbase patch-chrome --path /usr/bin/google-chrome --cache-dir /tmp/sb-chrome-patches
```

## Safety and licensing

Only patch executables that you own or have permission to modify. Patching a
system-installed driver or a binary belonging to another user may violate
licenses, local policies, or terms of service. Keep a backup (the default
`EnginePatch` presets create one automatically) and test the patched binary in a
non-production environment first.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `needs_patch()` still true after patching | Driver was replaced by a newer version | Re-run the patcher after driver downloads. |
| Browser crashes after Chrome binary patch | Patch incompatible with Chromium version | Restore from `.orig` and use the matching engine version. |
| Backup missing | Used `EnginePatch::no_backup()` | Re-download the original binary before patching again. |
| Bot detection still triggers | TLS or behavioral signals leak | Combine patching with `Fingerprint`, UC mode, and proxy egress alignment. |
