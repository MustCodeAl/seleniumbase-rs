---
title: "Launch Settings"
sidebarTitle: "Launch settings"
description: "Reference for LaunchContext, LaunchSettings, ProfileView, Engine, BrowserFamily and WebRtcPolicy: what a plugin is checked against and what before_launch may change, with what each engine fills in and applies."
---

What a plugin learns about a browser before it starts, and what it may change.
The set checks every plugin against a `LaunchContext`; `before_launch` gets the
browser's `LaunchSettings`, which the host turns into the engine's own options
after every plugin has run. Items are in `seleniumbase_rs_plugin`, defined in
`crates/seleniumbase-rs-plugin/src/lib.rs`.

<RequestExample>

```rust Change the start-up settings
use seleniumbase_rs_plugin::{
    async_trait, Capability, Category, LaunchSettings, Metadata, Plugin, Result, WebRtcPolicy,
};
use serde_json::json;

pub struct Quiet;

#[async_trait]
impl Plugin for Quiet {
    fn name(&self) -> &str {
        "quiet"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Privacy).capabilities([
            Capability::LaunchArgs,
            Capability::LaunchPrefs,
            Capability::LaunchNetwork,
        ])
    }

    async fn before_launch(&self, launch: &mut LaunchSettings) -> Result<()> {
        launch.args.push("--lang=en-US".into());
        launch.disable_features.push("Translate".into());
        launch.prefs.insert("intl.accept_languages".into(), json!("en-US,en"));
        launch.webrtc_policy = Some(WebRtcPolicy::DisableNonProxiedUdp);
        launch.disable_quic = true;
        Ok(())
    }
}
```

```rust Settings in a test
use std::path::PathBuf;

use seleniumbase_rs_plugin::{LaunchSettings, PluginSet, ProfileView};
use seleniumbase_rs_plugin_testing::block_on;

// LaunchSettings is #[non_exhaustive]: start from the default.
let mut settings = LaunchSettings::default();
settings.set_profile(ProfileView::Persistent(PathBuf::from("/tmp/profile")));

let set = PluginSet::new().with(Quiet)?;
block_on(set.before_launch(&mut settings))?;
assert_eq!(settings.args, ["--lang=en-US"]);
assert!(settings.disable_quic);
```

```rust Check a context
use seleniumbase_rs_plugin::{BrowserFamily, Engine, HostInfo, LaunchContext, PluginSet};

let context = LaunchContext {
    engine: Engine::WebDriver,
    family: BrowserFamily::Chromium,
    headless: true,
    connecting: false,
};
let set = PluginSet::new().with(Quiet)?;
set.check_with(&context, &HostInfo::webdriver(context.family))?;
```

</RequestExample>

<ResponseExample>

```text WebDriver, Chromium: extra_args after the hook
--lang=en-US
--disable-features=Translate
--force-webrtc-ip-handling-policy=disable_non_proxied_udp
--disable-quic
```

</ResponseExample>

## LaunchContext

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::LaunchContext`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchContext {
    pub engine: Engine,
    pub family: BrowserFamily,
    pub headless: bool,
    pub connecting: bool,
}
```

What the set checks a plugin against before a browser starts, in
[`PluginSet::check`](/plugins/api/plugin-set) and `check_with`. It is not
`#[non_exhaustive]`, so a test builds one with a struct literal.

<ParamField body="engine" type="Engine" required>

The engine that will run the browser.

</ParamField>

<ParamField body="family" type="BrowserFamily" required>

The browser's family. `Requirements::CHROMIUM_ONLY` refuses any other than
`Chromium`.

</ParamField>

<ParamField body="headless" type="bool" required>

Whether the browser runs without a window. `Requirements::HEADFUL` refuses
`true`.

</ParamField>

<ParamField body="connecting" type="bool" required>

Whether the program attaches to a running browser instead of starting one.
`Requirements::LAUNCH_ONLY` refuses `true`.

</ParamField>

What the hosts of seleniumbase-rs pass:

| Where | `engine` | `family` | `headless` | `connecting` |
|---|---|---|---|---|
| Pure CDP, `Browser::launch` | `Cdp` | `Chromium` | the options' headless setting | `false` |
| Pure CDP, `Browser::connect_with_plugins` | `Cdp` | `Chromium` | whether the user agent of `/json/version` holds `HeadlessChrome` | `true` |
| WebDriver, `BrowserSession::connect` | `WebDriver` | from `BrowserConfig::browser` | the `headless` setting; always `false` for Safari | `true` when the WebDriver URL is not the local default |
| WebDriver, a server that answers `browserName: tauri` | `WebDriver` | `Embedded` | `false` | `true` |

A launch checks the set twice, before and after `before_launch`, because a
plugin may change what another was checked against, such as headless mode.

## LaunchSettings

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::LaunchSettings`

```rust
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct LaunchSettings {
    pub args: Vec<String>,
    pub prefs: serde_json::Map<String, Value>,
    pub extensions: Vec<PathBuf>,
    pub proxy: Option<String>,
    pub headless: bool,
    pub host_resolver_rules: Vec<String>,
    pub webrtc_policy: Option<WebRtcPolicy>,
    pub disable_features: Vec<String>,
    pub enable_features: Vec<String>,
    pub disable_quic: bool,
    // private: profile: ProfileView
}
```

The start-up settings a plugin may change in
[`before_launch`](/plugins/api/plugin-trait). After each plugin,
`PluginSet::before_launch` compares the settings with what they were and refuses
a change the plugin may not make, as `PluginError::Capability` or
`PluginError::Sealed`. Outside the crate, start from `LaunchSettings::default()`
and assign the fields.

<ParamField body="args" type="Vec<String>">

Command-line switches for the browser, such as `--lang=de`. Adding needs
`launch.args`; entries may not be removed. A switch of
[`REFUSED_SWITCHES`](/plugins/api/capabilities#switch_policy), and a
`--disable-features=` or `--enable-features=` that names a refused feature,
is refused unless the plugin holds `raw:switches`.

</ParamField>

<ParamField body="prefs" type="serde_json::Map<String, Value>">

Browser preferences, as in Chrome's `Preferences` file. Changing needs
`launch.prefs`; a key that is there may not be removed (`it removed a
preference another plugin set`). The pure CDP engine reads a dotted name such
as `intl.accept_languages` as a path, as `LaunchOptionsBuilder::preference`
does.

</ParamField>

<ParamField body="extensions" type="Vec<PathBuf>">

Unpacked extension folders to load. Adding needs `launch.extensions`; entries
may not be removed.

</ParamField>

<ParamField body="proxy" type="Option<String>">

The proxy server, such as `http://127.0.0.1:8080`. Changing needs
`launch.proxy`. The host shows the server only: a password in the launch
options is never shown to plugins, and it stays when they leave the proxy as
it is. `Seals::PROXY` can seal it.

</ParamField>

<ParamField body="headless" type="bool">

Whether the browser runs without a window. Changing needs `launch.headless`.
`Seals::HEADLESS` can seal it.

</ParamField>

<ParamField body="host_resolver_rules" type="Vec<String>">

Host-resolver rules; the host joins them into one `--host-resolver-rules`
switch. Changing needs `launch.network`; entries may not be removed.

</ParamField>

<ParamField body="webrtc_policy" type="Option<WebRtcPolicy>">

The WebRTC IP-handling policy. Changing needs `launch.network`.
`Seals::WEBRTC` can seal it.

</ParamField>

<ParamField body="disable_features" type="Vec<String>">

Features to disable; the host merges them into one `--disable-features`
switch. Adding needs `launch.args`; entries may not be removed;
`IsolateOrigins` and `site-per-process` are refused unless the plugin holds
`raw:switches`.

</ParamField>

<ParamField body="enable_features" type="Vec<String>">

Features to enable, merged into one `--enable-features` switch. The same rules
as `disable_features`.

</ParamField>

<ParamField body="disable_quic" type="bool">

Whether to turn QUIC off (`--disable-quic`). Changing needs `launch.network`.

</ParamField>

<Note>

`CapabilitySet::legacy()`, the grants of a plugin that declares no metadata,
holds `launch.args`, `launch.prefs`, `launch.headless`, `launch.proxy` and
`launch.extensions` but not `launch.network`: such a plugin may not change
`host_resolver_rules`, `webrtc_policy` or `disable_quic`.

</Note>

### What each engine fills in and applies

| Host | Fills in | Applies |
|---|---|---|
| Pure CDP | Every field from the `LaunchOptions`: extra arguments, preferences, extensions, the proxy's server, headless, host rules, features, QUIC, WebRTC policy, and the profile | Every field back into the options. A proxy that does not parse is `SeleniumBaseError::InvalidConfig`. `WebRtcPolicy::Default` adds no switch |
| WebDriver, Chromium | `extra_args`; the `prefs` of `goog:chromeOptions` (`ms:edgeOptions` for Edge); the folders of `extension_dir`; the proxy without its password; headless. The other fields start empty | `args` become `extra_args`; host rules, features, WebRTC policy and QUIC become one switch each, merged with one already there; extensions become `extension_dir` (each path UTF-8 and without a comma) |
| WebDriver, Firefox | The arguments and preferences of the Firefox options; headless | Arguments and preferences. Host rules, WebRTC policy, features, QUIC, extensions and a proxy are refused; a preference must be a boolean, a whole number or text |
| WebDriver, Safari | Headless only, always `false` | Any change of arguments, preferences, extensions, proxy, headless or the Chromium-only fields is refused |

A refusal on Firefox or Safari is `SeleniumBaseError::Plugin` (`a plugin's
before_launch asked for what Firefox cannot do here: ...`). The guide has the
details per browser: [The WebDriver engine](/tutorials/plugins#the-webdriver-engine).

## LaunchSettings::profile

<Badge color="green">fn</Badge> `seleniumbase_rs_plugin::LaunchSettings::profile`

```rust
#[must_use]
pub fn profile(&self) -> &ProfileView
```

<ResponseField name="return" type="&ProfileView">

The browser profile this launch uses, read-only for plugins.

</ResponseField>

The hosts show `ProfileView::Persistent(folder)` when the launch names a
profile folder (`user_data_dir`), and `ProfileView::Ephemeral` otherwise.

## LaunchSettings::set_profile

<Badge color="green">fn</Badge> `seleniumbase_rs_plugin::LaunchSettings::set_profile`

```rust
pub fn set_profile(&mut self, profile: ProfileView)
```

<ParamField body="profile" type="ProfileView" required>

The profile of the launch.

</ParamField>

For the host, before the plugins run. A plugin that calls it is refused after
its hook: `PluginError::Capability` on `profile`, `the profile is read-only for
plugins`.

## ProfileView

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::ProfileView`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfileView {
    #[default]
    Ephemeral,
    Persistent(PathBuf),
}
```

<ResponseField name="ProfileView" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Ephemeral" type="unit variant" default="default">

A fresh profile thrown away at close.

</ResponseField>

<ResponseField name="Persistent(PathBuf)" type="tuple variant">

A profile kept in this folder.

</ResponseField>

</Expandable>

</ResponseField>

## Engine

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::Engine`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Engine {
    Cdp,
    WebDriver,
}
```

Which of seleniumbase-rs's engines runs the browser. A plugin reads it from
`LaunchContext::engine`, `BrowserHandle::engine` and `PageHandle::engine`.

<ResponseField name="Engine" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Cdp" type="unit variant">

The pure Chrome DevTools Protocol engine (`sb_cdp`).

</ResponseField>

<ResponseField name="WebDriver" type="unit variant">

The WebDriver engine (`BaseCase`).

</ResponseField>

</Expandable>

</ResponseField>

## BrowserFamily

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::BrowserFamily`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BrowserFamily {
    Chromium,
    Firefox,
    Safari,
    Embedded,
}
```

The family of the browser a plugin runs with.

<ResponseField name="BrowserFamily" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Chromium" type="unit variant">

Chrome, Chromium, Edge, Brave, Opera. The WebDriver engine also counts Comet,
Chrome for Testing and Chrome Headless Shell here; the pure CDP engine always
reports `Chromium`.

</ResponseField>

<ResponseField name="Firefox" type="unit variant">

Firefox.

</ResponseField>

<ResponseField name="Safari" type="unit variant">

Safari.

</ResponseField>

<ResponseField name="Embedded" type="unit variant">

A webview embedded in an app, such as Tauri's.

</ResponseField>

</Expandable>

</ResponseField>

The WebDriver engine refuses plugins for Internet Explorer, which is none of
these families, with `SeleniumBaseError::Plugin`.

## WebRtcPolicy

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::WebRtcPolicy`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum WebRtcPolicy {
    Default,
    DefaultPublicInterfaceOnly,
    DefaultPublicAndPrivateInterfaces,
    DisableNonProxiedUdp,
}
```

Chrome's WebRTC IP-handling policies, the value of
`LaunchSettings::webrtc_policy`. Both engines turn it into one
`--force-webrtc-ip-handling-policy` switch.

<ResponseField name="WebRtcPolicy" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Default" type="unit variant">

Chrome's default. No switch is added.

</ResponseField>

<ResponseField name="DefaultPublicInterfaceOnly" type="unit variant">

Use only the default public interface:
`--force-webrtc-ip-handling-policy=default_public_interface_only`.

</ResponseField>

<ResponseField name="DefaultPublicAndPrivateInterfaces" type="unit variant">

Use the public and private default interfaces:
`--force-webrtc-ip-handling-policy=default_public_and_private_interfaces`.

</ResponseField>

<ResponseField name="DisableNonProxiedUdp" type="unit variant">

Send UDP only through the proxy:
`--force-webrtc-ip-handling-policy=disable_non_proxied_udp`.

</ResponseField>

</Expandable>

</ResponseField>

A policy added to the API later, which this version of seleniumbase-rs cannot
apply, is `SeleniumBaseError::InvalidConfig`.
