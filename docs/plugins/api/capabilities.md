---
title: "Capabilities"
sidebarTitle: "Capabilities"
description: "Reference for Capability and its string forms, risk levels and enforcement, CapabilitySet, the scopes HostScope, NetScope and PathScope, RawKind, Risk, Enforcement, ParseCapabilityError, HostInfo and HostKind, and the switch_policy module of refused switches and features."
---

A capability is one thing a plugin may do, written as a stable string such as
`launch.args`, `cdp:Network` or `page.script:example.com`. A plugin declares the
ones it needs in its [metadata](/plugins/api/metadata); a
[PluginSet](/plugins/api/plugin-set) grants them and checks them against what
the host supports. Items are in `seleniumbase_rs_plugin`, defined in
`capability.rs`, `host.rs` and `switch_policy.rs` of
`crates/seleniumbase-rs-plugin/src/`.

<RequestExample>

```rust Parse and print
use seleniumbase_rs_plugin::{Capability, Risk};

let capability: Capability = "page.script:example.com,example.org".parse()?;
assert_eq!(capability.to_string(), "page.script:example.com,example.org");
assert_eq!(capability.risk(), Risk::Low);
assert_eq!("raw:cdp".parse::<Capability>()?.risk(), Risk::Critical);
assert!("nonsense".parse::<Capability>().is_err());
```

```rust Scopes cover narrower scopes
use seleniumbase_rs_plugin::{Capability, CapabilitySet, HostScope};

let wide: CapabilitySet = [Capability::PageScript(HostScope::Everywhere)]
    .into_iter()
    .collect();
assert!(wide.covers(&"page.script:a.com".parse::<Capability>()?));

let narrow: CapabilitySet = ["page.script:a.com".parse::<Capability>()?]
    .into_iter()
    .collect();
assert!(!narrow.covers(&"page.script:*".parse::<Capability>()?));
assert!(CapabilitySet::legacy().covers(&Capability::Cdp("Page".into())));
```

```rust What a host supports
use seleniumbase_rs_plugin::{BrowserFamily, Capability, HostInfo};

let firefox = HostInfo::webdriver(BrowserFamily::Firefox);
assert!(!firefox.supports(&Capability::Cdp("Network".into())));
assert!(HostInfo::cdp().supports(&Capability::NetIntercept));
assert_eq!(firefox.name(), "WebDriver with Firefox or Safari");
```

```rust Refused switches
use seleniumbase_rs_plugin::switch_policy::{feature_refused, refusal, switch_name};

assert_eq!(
    switch_name("--Host-Resolver-Rules=MAP * 1.2.3.4").as_deref(),
    Some("host-resolver-rules")
);
assert!(refusal("--no-sandbox").is_some());
assert!(refusal("--disable-features=Translate").is_none());
assert!(feature_refused("IsolateOrigins"));
```

</RequestExample>

<ResponseExample>

```text refusal("--no-sandbox")
the switch `--no-sandbox` is refused for every plugin
```

```text refusal("--enable-features=site-per-process:x/y")
the feature `site-per-process:x/y` may not be changed by a plugin
```

```text "nonsense".parse::<Capability>()
unknown capability `nonsense`
```

</ResponseExample>

<Warning>

For a native Rust plugin, capabilities are a declared contract, not a sandbox:
the plugin can still call `std` directly. This version checks the required
capabilities when the set is checked, and each change `before_launch` makes;
the request guard refuses `Fetch.*` and merges `Network.setBlockedURLs`. It
does not compare a plugin's `send`, `evaluate`, `add_init_script` or packages
with what it declared (see [Limits](/plugins/architecture#limits)). Only
[WebAssembly](/plugins/wasm) plugins are sandboxed.

</Warning>

## Capability

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::Capability`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Capability {
    LaunchArgs,
    LaunchPrefs,
    LaunchHeadless,
    LaunchNetwork,
    LaunchProxy,
    LaunchExtensions,
    LaunchProfile,
    PageScript(HostScope),
    PageEvaluate,
    PageNavigate,
    Cdp(Cow<'static, str>),
    NetObserve,
    NetSensitive,
    NetBodies,
    NetIntercept,
    NetFulfill,
    CookiesRead,
    CookiesWrite,
    StorageClear,
    Contexts,
    TestHooks,
    Task,
    ArtifactsWrite,
    HostNet(NetScope),
    HostFsRead(PathScope),
    HostFsWrite(PathScope),
    HostProcess,
    Clock,
    Random,
    State { persist: bool },
    Raw(RawKind),
}
```

One thing a plugin may do. Every variant has a string form (`Display` and
`FromStr`), a [risk](#risk), and an [enforcement](#enforcement). A
`CapabilitySet` keeps them sorted in the order of the variants above.

| Variant | String | Risk | WASM | Native |
|---|---|---|---|---|
| `LaunchArgs`: add browser switches (the refused list applies) | `launch.args` | Low | yes | enforced |
| `LaunchPrefs`: set browser preferences | `launch.prefs` | Medium | yes | enforced |
| `LaunchHeadless`: change headless mode | `launch.headless` | Medium | yes | enforced |
| `LaunchNetwork`: host rules, WebRTC policy, QUIC | `launch.network` | High | yes | enforced |
| `LaunchProxy`: set the proxy | `launch.proxy` | High | yes | enforced |
| `LaunchExtensions`: load extensions | `launch.extensions` | High | no | enforced |
| `LaunchProfile`: use the profile folder | `launch.profile` | Medium | no | enforced |
| `PageScript(HostScope::Hosts(..))`: scripts on these hosts | `page.script:a.com,b.com` | Low | yes | enforced |
| `PageScript(HostScope::Everywhere)`: scripts on every site | `page.script:*` | High | yes | enforced |
| `PageEvaluate`: evaluate JavaScript in pages | `page.evaluate` | Medium | yes | enforced |
| `PageNavigate`: navigate pages | `page.navigate` | Medium | yes | enforced |
| `Cdp(domain)`: use one DevTools domain | `cdp:Network` | Medium; Critical for `Target` and `Browser` | yes, unless Critical | enforced |
| `NetObserve`: observe requests and responses | `net.observe` | Low | yes | enforced |
| `NetSensitive`: see cookies and authorization headers | `net.sensitive` | High | yes | enforced |
| `NetBodies`: read response bodies | `net.bodies` | High | yes | enforced |
| `NetIntercept`: intercept and change requests | `net.intercept` | Medium | yes | enforced |
| `NetFulfill`: answer requests without the network | `net.fulfill` | High | yes | enforced |
| `CookiesRead` | `cookies.read` | High | yes | enforced |
| `CookiesWrite` | `cookies.write` | High | yes | enforced |
| `StorageClear` | `storage.clear` | Medium | yes | enforced |
| `Contexts`: create browser contexts | `contexts` | Medium | yes | enforced |
| `TestHooks`: hook test start and end | `test.hooks` | Low | yes | enforced |
| `Task`: run as a task | `task` | Medium | yes | enforced |
| `ArtifactsWrite`: write files to the run's output | `artifacts.write` | Medium | yes | enforced |
| `HostNet(NetScope::Loopback)` | `host.net:loopback` | High | no | declared only |
| `HostNet(NetScope::Any)` | `host.net:any` | High | no | declared only |
| `HostFsRead(scope)`: read host files | `host.fs.read:temp`, `host.fs.read:run-output`, `host.fs.read:/a,/b` | High | no | declared only |
| `HostFsWrite(scope)`: write host files | `host.fs.write:temp`, ... | High | no | declared only |
| `HostProcess`: start host processes | `host.process` | Critical | no | declared only |
| `Clock`: read the clock | `clock` | Low | yes | enforced |
| `Random`: use randomness | `random` | Low | yes | enforced |
| `State { persist: false }`: keep state for the run | `state` | Low | yes | enforced |
| `State { persist: true }`: keep state across runs | `state:persist` | Low | yes | enforced |
| `Raw(RawKind::Cdp)`: any DevTools command | `raw:cdp` | Critical | no | enforced |
| `Raw(RawKind::Switches)`: any switch not refused for everyone | `raw:switches` | Critical | no | enforced |

"WASM" is `wasm_grantable()`; "Native" is `enforcement(false)`.

## Capability::risk

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn risk(&self) -> Risk
```

<ResponseField name="return" type="Risk">

How dangerous the capability is, as in the table above.

</ResponseField>

`PluginSet::with` and `Registry::plugin_set` never grant a `Critical`
capability; `PluginSet::with_trusted` must name each one.

## Capability::wasm_grantable

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn wasm_grantable(&self) -> bool
```

<ResponseField name="return" type="bool">

Whether a WASM module or manifest may be granted it: not when it is
`Critical`, nor for `host.process`, `host.fs.read`, `host.fs.write`,
`host.net`, `launch.extensions` and `launch.profile`.

</ResponseField>

## Capability::enforcement

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn enforcement(&self, wasm: bool) -> Enforcement
```

<ParamField body="wasm" type="bool" required>

Whether the plugin is a WASM module (`true`) or native Rust (`false`).

</ParamField>

<ResponseField name="return" type="Enforcement">

`DeclaredOnly` for `host.net`, `host.fs.read`, `host.fs.write` and
`host.process` on a native plugin; `Enforced` otherwise.

</ResponseField>

`PluginSet::audit` and `sbrs plugin info` report `enforcement(false)`.

## Capability::covers

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn covers(&self, wanted: &Self) -> bool
```

<ParamField body="wanted" type="&Capability" required>

The capability a plugin needs.

</ParamField>

<ResponseField name="return" type="bool">

Whether holding `self` allows `wanted`.

</ResponseField>

Equal capabilities cover each other. Beyond that:

| Holding | Covers |
|---|---|
| `page.script:*` | every `page.script` |
| `page.script:HOSTS` | `page.script` of a subset of those hosts |
| `host.net:any` | `host.net:loopback` too |
| `raw:cdp` | every `cdp:DOMAIN` |
| `host.fs.read` or `host.fs.write` of declared paths | the same with a subset of the paths |

`raw:cdp` does not cover `raw:switches`, and `temp` or `run-output` cover only
themselves.

## Display and FromStr

<Badge color="blue">impl</Badge> `impl fmt::Display for Capability`, `impl FromStr for Capability`

```rust
impl fmt::Display for Capability { /* the string form */ }

impl FromStr for Capability {
    type Err = ParseCapabilityError;
    fn from_str(text: &str) -> Result<Self, Self::Err>;
}
```

The string forms in the table round-trip: `text.parse::<Capability>()?.to_string() == text`.
The grammar is `HEAD` or `HEAD:ARG`, split at the first `:`:

- `page.script:*` is every site; `page.script:a.com,b.com` is a host list
  (empty entries are dropped); `page.script:` alone is refused.
- `cdp:DOMAIN` needs a domain; the domain is kept as written.
- `host.fs.read:SCOPE` and `host.fs.write:SCOPE` take `temp`, `run-output`, or
  comma-separated paths.
- `host.net:` takes `loopback` or `any`; `state:` takes `persist`; `raw:` takes
  `cdp` or `switches`.
- Any other text is `ParseCapabilityError`.

## ParseCapabilityError

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::ParseCapabilityError`

```rust
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown capability `{0}`")]
pub struct ParseCapabilityError(pub String);
```

<ParamField body="0" type="String">

The text that does not parse.

</ParamField>

## CapabilitySet

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::CapabilitySet`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapabilitySet(/* private BTreeSet<Capability> */);

impl FromIterator<Capability> for CapabilitySet { /* ... */ }
```

A set of capabilities, sorted and without repeats: the `capabilities` and
`optional` of `Metadata`, and the grants of a plugin in a set. Build one with
`collect()`, `new` and `insert`, or `legacy`.

## CapabilitySet::new

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn new() -> Self
```

<ResponseField name="return" type="CapabilitySet">

The empty set.

</ResponseField>

## CapabilitySet::legacy

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn legacy() -> Self
```

<ResponseField name="return" type="CapabilitySet">

What a plugin that declares nothing is treated as having: `launch.args`,
`launch.prefs`, `launch.headless`, `launch.proxy`, `launch.extensions`,
`page.script:*`, `page.evaluate` and `raw:cdp`.

</ResponseField>

## CapabilitySet::insert

<Badge color="green">fn</Badge>

```rust
pub fn insert(&mut self, capability: Capability)
```

<ParamField body="capability" type="Capability" required>

The capability to add; a repeat changes nothing.

</ParamField>

## CapabilitySet::covers

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn covers(&self, wanted: &Capability) -> bool
```

<ParamField body="wanted" type="&Capability" required>

The capability a plugin needs.

</ParamField>

<ResponseField name="return" type="bool">

Whether some member covers it (`Capability::covers`).

</ResponseField>

## CapabilitySet::iter

<Badge color="green">fn</Badge>

```rust
pub fn iter(&self) -> impl Iterator<Item = &Capability>
```

<ResponseField name="return" type="impl Iterator<Item = &Capability>">

The members, in order.

</ResponseField>

## CapabilitySet::is_empty

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn is_empty(&self) -> bool
```

<ResponseField name="return" type="bool">

Whether the set is empty.

</ResponseField>

## CapabilitySet::union

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn union(&self, other: &Self) -> Self
```

<ParamField body="other" type="&CapabilitySet" required>

The other set.

</ParamField>

<ResponseField name="return" type="CapabilitySet">

The members of both.

</ResponseField>

## CapabilitySet::without_critical

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn without_critical(&self) -> Self
```

<ResponseField name="return" type="CapabilitySet">

The members that are not `Risk::Critical`: the default grants are
`capabilities.union(&optional).without_critical()`.

</ResponseField>

## HostScope

<Badge color="orange">enum</Badge> `seleniumbase_rs_plugin::HostScope`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HostScope {
    Hosts(Vec<String>),
    Everywhere,
}
```

Which sites a page script may run on: the scope of `Capability::PageScript`.

<ResponseField name="HostScope" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Hosts(Vec<String>)" type="tuple variant">Only these hosts and their subdomains (`page.script:a.com,b.com`). Low risk.</ResponseField>

<ResponseField name="Everywhere" type="unit variant">Every site (`page.script:*`). High risk.</ResponseField>

</Expandable>

</ResponseField>

## NetScope

<Badge color="orange">enum</Badge> `seleniumbase_rs_plugin::NetScope`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NetScope {
    Loopback,
    Any,
}
```

Which network peers a plugin may talk to from the host: the scope of
`Capability::HostNet`.

<ResponseField name="NetScope" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Loopback" type="unit variant">Loopback addresses only (`host.net:loopback`).</ResponseField>

<ResponseField name="Any" type="unit variant">Any address (`host.net:any`).</ResponseField>

</Expandable>

</ResponseField>

## PathScope

<Badge color="orange">enum</Badge> `seleniumbase_rs_plugin::PathScope`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PathScope {
    Temp,
    RunOutput,
    Declared(Vec<PathBuf>),
}
```

Which files a plugin may use: the scope of `Capability::HostFsRead` and
`HostFsWrite`.

<ResponseField name="PathScope" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Temp" type="unit variant">A temporary folder owned by the plugin (`temp`).</ResponseField>

<ResponseField name="RunOutput" type="unit variant">The run's output folder (`run-output`).</ResponseField>

<ResponseField name="Declared(Vec<PathBuf>)" type="tuple variant">These paths, comma-separated in the string form.</ResponseField>

</Expandable>

</ResponseField>

## RawKind

<Badge color="orange">enum</Badge> `seleniumbase_rs_plugin::RawKind`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RawKind {
    Cdp,
    Switches,
}
```

The kind of raw access a plugin has: the scope of `Capability::Raw`. Both are
Critical.

<ResponseField name="RawKind" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Cdp" type="unit variant">Any Chrome DevTools Protocol command (`raw:cdp`).</ResponseField>

<ResponseField name="Switches" type="unit variant">Any browser switch that is not refused for everyone (`raw:switches`): it lifts `REFUSED_SWITCHES` and `REFUSED_FEATURES` for the plugin.</ResponseField>

</Expandable>

</ResponseField>

## Risk

<Badge color="orange">enum</Badge> `seleniumbase_rs_plugin::Risk`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Risk {
    Low,
    Medium,
    High,
    Critical,
}
```

How dangerous a capability is. Ordered from `Low` to `Critical`.

<ResponseField name="Risk" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Low" type="unit variant">Harmless to the user's data and network.</ResponseField>

<ResponseField name="Medium" type="unit variant">Changes browser behaviour in the plugin's own area.</ResponseField>

<ResponseField name="High" type="unit variant">Can read or redirect user data or the network.</ResponseField>

<ResponseField name="Critical" type="unit variant">Never granted without being named, and never to WASM.</ResponseField>

</Expandable>

</ResponseField>

## Enforcement

<Badge color="orange">enum</Badge> `seleniumbase_rs_plugin::Enforcement`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Enforcement {
    Enforced,
    DeclaredOnly,
}
```

Whether the host stops a plugin that goes beyond a capability, as
`Capability::enforcement` classifies it.

<ResponseField name="Enforcement" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Enforced" type="unit variant">The API surface or a sandbox stops it. `sbrs plugin info` prints `enforced`.</ResponseField>

<ResponseField name="DeclaredOnly" type="unit variant">The plugin promises; a native Rust plugin can still use `std`. `sbrs plugin info` prints `declared only`.</ResponseField>

</Expandable>

</ResponseField>

## HostInfo

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::HostInfo`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostInfo { /* private: kind */ }
```

What the host that runs a set supports, passed to
[`PluginSet::check_with`](/plugins/api/plugin-set) so the set can refuse a
plugin whose required capabilities the host cannot provide
(`PluginError::Unsupported`).

| Capabilities | `cdp()` | `webdriver(Chromium)` | `webdriver(Firefox or Safari)` | `embedded()` |
|---|---|---|---|---|
| `page.script`, `page.evaluate`, `test.hooks`, `clock`, `random` | yes | yes | yes | yes |
| `cdp:*`, `raw:*` | yes | yes | no | no |
| `net.*`, `contexts` | yes | no | no | no |
| Every other capability (`launch.*`, `page.navigate`, `cookies.*`, `storage.clear`, `task`, `artifacts.write`, `host.*`, `state`) | yes | yes | yes | no |

## HostInfo::cdp

<Badge color="green">fn</Badge> <Badge color="gray" stroke>const</Badge>

```rust
#[must_use]
pub const fn cdp() -> Self
```

<ResponseField name="return" type="HostInfo">

The pure CDP engine, which supports everything. `PluginSet::check` uses it.

</ResponseField>

## HostInfo::webdriver

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn webdriver(family: BrowserFamily) -> Self
```

<ParamField body="family" type="BrowserFamily" required>

The browser's family.

</ParamField>

<ResponseField name="return" type="HostInfo">

`HostKind::WebDriverChromium` for `Chromium`, `HostKind::WebDriverOther` for
any other family.

</ResponseField>

## HostInfo::embedded

<Badge color="green">fn</Badge> <Badge color="gray" stroke>const</Badge>

```rust
#[must_use]
pub const fn embedded() -> Self
```

<ResponseField name="return" type="HostInfo">

A Tauri webview. The WebDriver engine re-checks a set against it when the
server answers `browserName: tauri`.

</ResponseField>

## HostInfo::kind

<Badge color="green">fn</Badge> <Badge color="gray" stroke>const</Badge>

```rust
#[must_use]
pub const fn kind(&self) -> HostKind
```

<ResponseField name="return" type="HostKind">

The kind of host.

</ResponseField>

## HostInfo::name

<Badge color="green">fn</Badge> <Badge color="gray" stroke>const</Badge>

```rust
#[must_use]
pub const fn name(&self) -> &'static str
```

<ResponseField name="return" type="&'static str">

A short name for messages: `the CDP engine`, `WebDriver with Chromium`,
`WebDriver with Firefox or Safari`, `an embedded webview`.

</ResponseField>

## HostInfo::supports

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn supports(&self, capability: &Capability) -> bool
```

<ParamField body="capability" type="&Capability" required>

A capability a plugin requires.

</ParamField>

<ResponseField name="return" type="bool">

Whether the host can provide it, as in the table under [HostInfo](#hostinfo).

</ResponseField>

## HostKind

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::HostKind`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HostKind {
    Cdp,
    WebDriverChromium,
    WebDriverOther,
    Embedded,
}
```

<ResponseField name="HostKind" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Cdp" type="unit variant">The pure CDP engine: everything.</ResponseField>

<ResponseField name="WebDriverChromium" type="unit variant">WebDriver with a Chromium browser.</ResponseField>

<ResponseField name="WebDriverOther" type="unit variant">WebDriver with Firefox or Safari.</ResponseField>

<ResponseField name="Embedded" type="unit variant">A Tauri webview.</ResponseField>

</Expandable>

</ResponseField>

## switch_policy

<Badge color="gray">mod</Badge> `seleniumbase_rs_plugin::switch_policy`

The one list of browser switches that no plugin may add, and of the features no
plugin may turn on or off. `PluginSet::before_launch` applies it to every
plugin, legacy ones included, unless the plugin holds `raw:switches`.
seleniumbase-rs has a test (`tests/plugin_switch_policy.rs`) that fails when a
switch its own stealth layer forbids is missing from `REFUSED_SWITCHES`.

## REFUSED_SWITCHES

<Badge color="gray">const</Badge> `seleniumbase_rs_plugin::switch_policy::REFUSED_SWITCHES`

```rust
pub const REFUSED_SWITCHES: &[&str];
```

Switch names (no dashes, lower case) refused for every plugin:

| Kind | Switches |
|---|---|
| Process launchers | `renderer-cmd-prefix`, `utility-cmd-prefix`, `zygote-cmd-prefix`, `gpu-launcher`, `ppapi-plugin-launcher`, `plugin-launcher`, `browser-subprocess-path`, `nacl-gdb`, `nacl-gdb-script` |
| Sandbox and security | `no-sandbox`, `disable-setuid-sandbox`, `disable-gpu-sandbox`, `no-zygote`, `disable-web-security`, `ignore-certificate-errors`, `ignore-certificate-errors-spki-list`, `disable-site-isolation-trials`, `allow-file-access-from-files`, `unsafely-treat-insecure-origin-as-secure`, `allow-running-insecure-content`, `single-process`, `js-flags` |
| Network (use the `LaunchSettings` fields) | `proxy-server`, `proxy-pac-url`, `host-resolver-rules`, `host-rules` |
| Remote debugging | `remote-debugging-port`, `remote-debugging-address`, `remote-debugging-pipe`, `remote-allow-origins` |
| Extensions and profile | `load-extension`, `load-component-extension`, `disable-extensions-except`, `user-data-dir` |
| Logs and dumps | `log-net-log`, `net-log-capture-mode`, `enable-logging`, `log-file`, `disk-cache-dir`, `crash-dumps-dir` |

## REFUSED_FEATURES

<Badge color="gray">const</Badge> `seleniumbase_rs_plugin::switch_policy::REFUSED_FEATURES`

```rust
pub const REFUSED_FEATURES: &[&str] = &["isolateorigins", "site-per-process"];
```

Feature names (lower case) that may not be disabled or enabled, through
`--disable-features=`, `--enable-features=`, or the `disable_features` and
`enable_features` fields of `LaunchSettings`.

## switch_name

<Badge color="green">fn</Badge> `seleniumbase_rs_plugin::switch_policy::switch_name`

```rust
#[must_use]
pub fn switch_name(arg: &str) -> Option<String>
```

<ParamField body="arg" type="&str" required>

A command-line argument.

</ParamField>

<ResponseField name="return" type="Option<String>">

The switch name of `arg`: surrounding spaces, the leading `--` or `-` and any
`=value` removed, in lower case. `None` when `arg` is not a switch.

</ResponseField>

## refusal

<Badge color="green">fn</Badge> `seleniumbase_rs_plugin::switch_policy::refusal`

```rust
#[must_use]
pub fn refusal(arg: &str) -> Option<String>
```

<ParamField body="arg" type="&str" required>

A command-line argument a plugin adds.

</ParamField>

<ResponseField name="return" type="Option<String>">

Why `arg` is refused, or `None` when it is allowed: ``the switch `--NAME` is
refused for every plugin``, or, for a `--disable-features=` or
`--enable-features=` list naming a refused feature, ``the feature `NAME` may
not be changed by a plugin``. Every spelling counts: `-name`, upper case,
surrounding spaces.

</ResponseField>

## feature_refused

<Badge color="green">fn</Badge> `seleniumbase_rs_plugin::switch_policy::feature_refused`

```rust
#[must_use]
pub fn feature_refused(feature: &str) -> bool
```

<ParamField body="feature" type="&str" required>

A feature name, as in `--disable-features=`; anything after a `:` is ignored.

</ParamField>

<ResponseField name="return" type="bool">

Whether the feature is in `REFUSED_FEATURES`, compared in lower case.

</ResponseField>
