---
title: "Requirements and Packages"
sidebarTitle: "Requirements and packages"
description: "Reference for Requirements, the conditions a plugin needs from the browser (CHROMIUM_ONLY, LAUNCH_ONLY, HEADFUL, RUN_LAST), and Package, the JavaScript and CSS a plugin loads into pages, with the init script each package becomes."
---

Two declarations of the [Plugin trait](/plugins/api/plugin-trait) that the set
checks before anything starts: `requirements` says what browser a plugin can
run with, and `packages` lists the scripts and styles it loads into every new
tab. Both items are in `seleniumbase_rs_plugin`, defined in
`crates/seleniumbase-rs-plugin/src/lib.rs`.

<RequestExample>

```rust Requirements
use seleniumbase_rs_plugin::{async_trait, Plugin, Requirements};

pub struct Unpacker;

#[async_trait]
impl Plugin for Unpacker {
    fn name(&self) -> &str {
        "unpacker"
    }

    // Extensions load only into a Chromium browser this program starts.
    fn requirements(&self) -> Requirements {
        Requirements::CHROMIUM_ONLY | Requirements::LAUNCH_ONLY
    }
}
```

```rust A pinned script on some sites
use seleniumbase_rs_plugin::Package;

fn packages() -> Vec<Package> {
    vec![
        Package::script_url("https://cdn.jsdelivr.net/npm/intro.js@7.2.0/intro.min.js")
            .integrity("sha384-...") // the file's real SRI hash
            .only_on(["example.com"]),
        Package::style_url("https://cdn.jsdelivr.net/npm/intro.js@7.2.0/minified/introjs.min.css")
            .only_on(["example.com"]),
    ]
}
```

```rust Inline code everywhere
use seleniumbase_rs_plugin::Package;

fn packages() -> Vec<Package> {
    vec![
        Package::script("window.__plugin = true;").everywhere(),
        Package::style("body { zoom: 0.9 }").everywhere(),
    ]
}
```

```rust Read the init script
use seleniumbase_rs_plugin::Package;

let script = Package::style("body { zoom: 0.9 }").everywhere().init_script();
assert!(script.unwrap().contains("zoom: 0.9"));
// A package that does not say where it loads has no init script.
assert!(Package::script("window.x = 1").init_script().is_none());
```

</RequestExample>

<ResponseExample>

```js style(...).everywhere()
(() => { const hosts = null; if (hosts && !hosts.some((h) => location.hostname === h || location.hostname.endsWith('.' + h))) return; const add = () => { const el = document.createElement('style'); el.textContent = "body { zoom: 0.9 }"; (document.head || document.documentElement).appendChild(el); }; if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', add, { once: true }); else add(); })();
```

```js script(...).only_on(example.com)
(() => { const hosts = ["example.com"]; if (hosts && !hosts.some((h) => location.hostname === h || location.hostname.endsWith('.' + h))) return; window.x = 1
})();
```

</ResponseExample>

## Requirements

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::Requirements`

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Requirements(/* private u8 */);

impl Requirements {
    pub const RUN_LAST: Self;
    pub const HEADFUL: Self;
    pub const LAUNCH_ONLY: Self;
    pub const CHROMIUM_ONLY: Self;
    pub const fn empty() -> Self;
    pub const fn contains(self, other: Self) -> bool;
}

impl std::ops::BitOr for Requirements { type Output = Self; }
```

Conditions a plugin needs, combined with `|`, returned by
`Plugin::requirements`. `PluginSet::check_with` compares them with the
[LaunchContext](/plugins/api/launch#launchcontext) and refuses a browser that
does not fit with `PluginError::Requirement`, before anything starts. A
refusal is an error, not a warning.

<ResponseField name="Requirements" type="flags">

<Expandable title="constants" defaultOpen>

<ResponseField name="Requirements::CHROMIUM_ONLY" type="const">

Refuses browsers that are not Chromium-based (`family` other than
`Chromium`): `it needs a Chromium-based browser`. Say it when the plugin needs
`send`, or a script that runs before the page's own.

</ResponseField>

<ResponseField name="Requirements::LAUNCH_ONLY" type="const">

Refuses a browser the program attaches to instead of starting
(`connecting: true`), such as one attached with
`Browser::connect_with_plugins` or on a remote WebDriver server: `it only works
with a browser the program starts, not one it attaches to`.

</ResponseField>

<ResponseField name="Requirements::HEADFUL" type="const">

Refuses a headless browser: `it needs a browser with a window, not a headless
one`.

</ResponseField>

<ResponseField name="Requirements::RUN_LAST" type="const">

Refuses nothing. The plugin runs in the phase `Last`, after the plugins
without this flag, whatever its metadata says.

</ResponseField>

</Expandable>

</ResponseField>

`sbrs plugin info` prints them under `needs` as `a Chromium-based browser`,
`a browser it starts itself, not one it attaches to`, `a window, not headless`
and `to run after the plugins without this flag`.

## Requirements::empty

<Badge color="green">fn</Badge> <Badge color="gray" stroke>const</Badge>

```rust
#[must_use]
pub const fn empty() -> Self
```

<ResponseField name="return" type="Requirements">

No requirements; also `Requirements::default()` and the default of
`Plugin::requirements`.

</ResponseField>

## Requirements::contains

<Badge color="green">fn</Badge> <Badge color="gray" stroke>const</Badge>

```rust
#[must_use]
pub const fn contains(self, other: Self) -> bool
```

<ParamField body="other" type="Requirements" required>

The flags to look for.

</ParamField>

<ResponseField name="return" type="bool">

Whether every flag of `other` is set.

</ResponseField>

## Package

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::Package`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package { /* private: kind, integrity, hosts */ }
```

JavaScript or CSS that a plugin loads into pages, returned by
`Plugin::packages`. Make one with a constructor, then say where it loads with
`only_on` or `everywhere`: the check refuses a plugin with a package that does
neither (``one of its packages says neither `only_on` nor `everywhere` ``), so a
plugin never loads a script into every site by accident. Pin the version in a
URL and add `integrity` so the browser checks the file.

For every new tab, `PluginSet::on_page_created` hands each package's
`init_script()` to `PageHandle::add_init_script`, before the plugin's own
`on_page_created`. Declare the matching capability, `page.script:HOSTS` or
`page.script:*`; this version does not compare the packages with it.

| Constructor | Loads | When, in each matching document |
|---|---|---|
| `Package::script(code)` | inline JavaScript | as soon as the document starts, before the page's own scripts |
| `Package::style(css)` | an inline `style` element | at `DOMContentLoaded`, or at once if the document has loaded |
| `Package::script_url(url)` | a `script` element with `src`, `crossOrigin = 'anonymous'` and the integrity | as `style` |
| `Package::style_url(url)` | a `link rel="stylesheet"` element, `crossOrigin = 'anonymous'` and the integrity | as `style` |

The element goes into `document.head`, or the document element when there is
no head. A document matches when `location.hostname` equals one of the hosts or
ends with `.` and one of them; with `everywhere`, every document matches.

## Package::script_url

<Badge color="green">fn</Badge>

```rust
pub fn script_url(url: impl Into<String>) -> Self
```

<ParamField body="url" type="impl Into<String>" required>

The script's URL. Pin the version in it.

</ParamField>

<ResponseField name="return" type="Package">

A script loaded from `url`, with no hosts yet.

</ResponseField>

## Package::style_url

<Badge color="green">fn</Badge>

```rust
pub fn style_url(url: impl Into<String>) -> Self
```

<ParamField body="url" type="impl Into<String>" required>

The stylesheet's URL.

</ParamField>

<ResponseField name="return" type="Package">

A stylesheet loaded from `url`, with no hosts yet.

</ResponseField>

## Package::script

<Badge color="green">fn</Badge>

```rust
pub fn script(code: impl Into<String>) -> Self
```

<ParamField body="code" type="impl Into<String>" required>

JavaScript run in every matching document before the page's own scripts.

</ParamField>

<ResponseField name="return" type="Package">

An inline script, with no hosts yet.

</ResponseField>

## Package::style

<Badge color="green">fn</Badge>

```rust
pub fn style(css: impl Into<String>) -> Self
```

<ParamField body="css" type="impl Into<String>" required>

CSS added to every matching document.

</ParamField>

<ResponseField name="return" type="Package">

An inline stylesheet, with no hosts yet.

</ResponseField>

## Package::integrity

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn integrity(mut self, hash: impl Into<String>) -> Self
```

<ParamField body="hash" type="impl Into<String>" required>

The Subresource Integrity hash of the file at the URL, such as `sha384-...`.

</ParamField>

<ResponseField name="return" type="Package">

The package with the hash; the browser refuses a file that does not match. It
applies to `script_url` and `style_url`; inline code has no file to check.

</ResponseField>

## Package::only_on

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn only_on<I, S>(mut self, hosts: I) -> Self
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
```

<ParamField body="hosts" type="impl IntoIterator<Item = impl Into<String>>" required>

Host names, such as `["example.com"]`. Each also covers its subdomains. They
are compared as written with `location.hostname`, which browsers keep in lower
case.

</ParamField>

<ResponseField name="return" type="Package">

The package, loading only on these hosts. It replaces an earlier `only_on` or
`everywhere`.

</ResponseField>

## Package::everywhere

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn everywhere(mut self) -> Self
```

<ResponseField name="return" type="Package">

The package, loading on every site. It replaces an earlier `only_on`.

</ResponseField>

## Package::init_script

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn init_script(&self) -> Option<String>
```

<ResponseField name="return" type="Option<String>">

The script that loads the package into a document, for
`PageHandle::add_init_script`; `None` when the package does not say where it
loads.

</ResponseField>

Every value (URL, hash, CSS, host list) goes into the script as a JSON
literal, so a quote in it cannot break the script; inline `script` code is
inserted as written, followed by a line break. The ResponseExample shows two
results.
