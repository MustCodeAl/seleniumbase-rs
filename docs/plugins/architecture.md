---
title: "Plugin Architecture"
sidebarTitle: "Architecture"
---

<Badge color="orange">Proposed</Badge>

This page summarizes the design of the plugin platform, version 2. The full
record, with the checks behind each decision, is
[ADR 0001 in the plugins repository](https://github.com/MustCodeAl/seleniumbase-rs-plugins/blob/main/docs/adr/0001-plugin-architecture.md)
(number 005 in this repository's series).

<Note>

The ADR is still **proposed**: its own status line has not changed. Most of its
plan is built in the plugins repository and used by seleniumbase-rs: the API
crate with metadata, capabilities and the registry, the hooks of phases 2 and 3
(navigation, the request router, browser contexts, DevTools events, tests and
tasks) and ten built-in plugins (see [The built-in plugins](#the-built-in-plugins),
[What is built](#what-is-built) and
[What core does with it](#what-core-does-with-it)). What is not built is a plan,
not a promise of dates.

</Note>

## The design in short

<Visibility for="agents">

Summary for AI exports: the plugin platform keeps one `Plugin` trait with default hook bodies, and the API crate stays on 0.1.x and changes only by addition. A plugin may describe itself with `metadata()` (category, phase, needed capabilities, ordering edges). Capabilities are checked where a plugin reaches the browser. For a native Rust plugin they are a contract and not a sandbox; only WebAssembly modules are sandboxed. The ADR is still proposed; its phase 1, the hooks of phases 2 and 3 and ten built-in plugins (`shield`, `block-resources`, `crx`, `fetcher`, `extractor`, `batcher`, `crawler`, `browser`, `imap`, `tor`) are built, and the store, lockfile, manifest plugins and WASM ABI 2 are not.

</Visibility>

- One `Plugin` trait with more hooks, all with default bodies. The API crate
  stays on 0.1.x and changes only by addition, so existing plugins keep
  working as "legacy" plugins.
- A plugin may describe itself with `metadata()`: a category, a phase, the
  capabilities it needs, and ordering edges.
- Capabilities are declared by each plugin and checked when the set is built; for a native Rust plugin they are a contract, not a sandbox (see [Capabilities](/plugins/api/capabilities)).
- Built-in plugins ship inside the prebuilt binaries and are turned on by name.
- Third-party code that is not compiled in comes as WebAssembly or as a TOML
  manifest. Native code is never loaded at run time.

## Five categories

| Category | For | Runs in phase |
|---|---|---|
| Environment | Presets, runtime, switches, proxies, headless | Prepare |
| Testing | Assertions, lifecycle, reporters, recording fakes | Observe |
| Extension | CRX and unpacked extensions, background workers | Normal |
| Core | Interception, network logging, resource blocking | Normal |
| Privacy | Hardening, telemetry, WebRTC, cookie isolation | Enforce |

## Capabilities

A capability is one thing a plugin may do, written as a stable string such as
`launch.args`, `net.observe` or `page.script:<hosts>`. A scoped enum, not a
bit set, because some capabilities carry a scope (hosts, paths).

| Risk | Examples |
|---|---|
| <Badge color="green">Low</Badge> | `launch.args`, `page.script:<hosts>`, `net.observe`, `test.hooks` |
| <Badge color="blue">Medium</Badge> | `launch.prefs`, `launch.headless`, `page.evaluate`, `net.intercept`, `contexts` |
| <Badge color="orange">High</Badge> | `page.script:*`, `cookies.*`, `net.bodies`, `launch.proxy`, `launch.extensions`, `host.fs.*` |
| <Badge color="red">Critical</Badge> | `raw:*`, `host.process`, `cdp:Target`, `cdp:Browser` |

How a set grants them:

- `PluginSet::with` grants what the plugin declares, except Critical ones.
- `with_trusted` must name each Critical capability.
- `with_grants` gives exactly the grants it is passed, as the command line does
  from a lockfile.
- A strict set refuses plugins that declare no metadata.

<Warning>

Capabilities are a contract at the API surface. For a native Rust plugin they
are **not** a sandbox: it can still call `std` directly. Only WebAssembly
modules are sandboxed.

</Warning>

## Lifecycle and ordering

Hooks run in this order of precedence: the plugin's phase (`Prepare`,
`Normal`, `Enforce`, `Observe`, `Last`), then `requires`, `after` and `before`
edges, then insertion order. A set of plugins without metadata therefore keeps
today's order, with `RUN_LAST` plugins at the end. A launch field a plugin
*seals* cannot later be changed to a different value by another plugin. An
error in a launch or page hook stops that launch or tab.

## Distribution

| Who | Install | Enable |
|---|---|---|
| Rust, built-in | A feature such as `plugin-shield` | Builder call, or a name in `[plugins]` / `SB_PLUGINS` |
| Rust, third-party | `cargo add` the crate | `PluginSet::with`, `LaunchOptionsBuilder::plugins` or `BrowserConfig::with_plugins`, or by name through a `Registry` |
| Prebuilt `sbrs` and `sbrs-mcp` | Nothing, built-ins are compiled in | `--plugin NAME`, `--plugin-opt NAME.KEY=<TOML>`; `sbrs plugin list` and `sbrs plugin info NAME` show them |
| WASM or manifest (planned) | `sbrs plugin add` into a content-addressed store, not built yet: `sbrs plugin` has only `list` and `info` | By the name it declares |

WASM and manifest plugins are pinned by SHA-256 in a lockfile and their
capabilities are approved at install. A project's own lock holds pins only,
never grants. WASM ABI 2 puts a JSON manifest in a custom section and runs the
module once at load; event mode (ABI 2.1) comes later.

## The built-in plugins

The ADR specifies seven built-ins that exercise the platform: **shield** and
**tor** (privacy), **fetcher** and **extractor** (core), **batcher**
(environment), **crawler** (testing) and **browser** (extension). They are built,
with three more: **block-resources**, **crx** and **imap**. Each has a page:

| Plugin | Category | What it does |
|---|---|---|
| [shield](/plugins/shield) | Privacy | Hardens the browser's privacy settings |
| [block-resources](/plugins/block-resources) | Core | Stops pages from loading images, fonts or media |
| [crx](/plugins/crx) | Extension | Loads packed `.crx` extensions |
| [fetcher](/plugins/fetcher) | Core | Request rules, response records and a fetch task |
| [extractor](/plugins/extractor) | Core | Finds and extracts structured data from pages, as a task |
| [batcher](/plugins/batcher) | Environment | Runs a list of jobs in tabs, with retries and a resume file |
| [crawler](/plugins/crawler) | Testing | Crawls a site you own and reports broken pages |
| [browser](/plugins/browser) | Extension | Loads extensions from `.zip` and `.crx` files and folders |
| [imap](/plugins/imap) | Testing | Waits for a verification mail and reads its code or link |
| [tor](/plugins/tor) | Privacy | Routes the browser through a Tor daemon you run |

## What is built

Phase 1 is in the API crate `seleniumbase-rs-plugin`, all additive, and the
hooks of the later phases are added to it the same way:

| Piece | File |
|---|---|
| `Capability`, `CapabilitySet` and string forms | `capability.rs` |
| `Category`, `Phase`, `Seals`, `Metadata` | `metadata.rs` |
| Ordering of a set | `order.rs` |
| The refused-switch list, applied to every plugin | `switch_policy.rs` |
| `HostInfo`, which capabilities each host supports | `host.rs` |
| `Configurable`, `Descriptor`, `Registry`, `PluginSpec` | `config.rs` |
| `Plugin::metadata()`, `Plugin::per_launch()`, `PluginSet::strict()`, `with_trusted`, `with_grants`, `audit()` | `lib.rs` |
| `Navigation`, `NavDecision`: `before_navigate` and `after_navigate` | `nav.rs` |
| `NetworkInterest`, `NetRule`, `RequestFilter`, `RequestView`, `ResponseView`: the network declarations and `on_request`, `on_response` | `network.rs` |
| `ContextSettings`, `ContextHandle`: browser contexts | `context.rs` |
| `Task`, `JobRunner`, `Sink`, `Prompt`, `TaskContext`: work started by name | `task.rs` |
| `TestInfo`, `TestOutcome`, `TestVerdict`: `on_test_end` | `testhook.rs` |
| `Clock`, `Timer`: sleeps and time limits | `timer.rs` |
| `Guarded`, `cdp_policy`: the guarded handles | `guard.rs` |

Next to the API crate there is the `seleniumbase-rs-plugins-builtin` crate
(one cargo feature per plugin, and `registry()`) and the ten plugins listed in
[The built-in plugins](#the-built-in-plugins). How a program turns them on is in
[Choosing plugins](/plugins/selecting).

Not part of this page's claim: the store, manifests and WASM ABI 2 belong to the
later work. The plan, from the ADR, and where it stands:

| Phase | Delivers | State |
|---|---|---|
| 0 | Security hotfix: one refused-switch list for all plugins | Built (`switch_policy.rs`) |
| 1 | Metadata, registry, CLI and config surface, release features, shield | Built |
| 2 | CDP lifecycle hooks, the shared request router, fetcher, extractor, tor | Built for the pure CDP engine |
| 3 | Tasks, test hooks, store and lockfile, manifest plugins, WASM ABI 2.0 | Tasks and test hooks are built; the store, lockfile, manifest plugins and WASM ABI 2.0 are not |
| 4 | WebDriver parity, ABI 2.1 events, CRX3 signature checks | Not built: the WebDriver hosts offer no request interception, browser contexts, tasks or test hooks, and the signature of a `.crx` is not verified |

To declare metadata in your own plugin, see
[Writing your own plugin](/plugins/writing-your-own#declaring-metadata-and-capabilities).

## What core does with it

The plugins repository listed seven things seleniumbase-rs had to do before
the new API was useful. They are done, in both engines unless an item says
otherwise:

<AccordionGroup>

<Accordion title="1. The API enums are `#[non_exhaustive]`" icon="circle-check">

A match on `BrowserFamily`, `PluginError` or a WebRTC policy has a wildcard arm. `PluginError` variants that name a plugin (`Unsupported`, `Capability`, `Undeclared`, `Conflict`, `Sealed`) now carry its name into `SeleniumBaseError::Plugin`, and the hint explains the new refusals.

</Accordion>

<Accordion title="2. Pooled browsers shared per-launch state" icon="circle-check">

`PluginSet::for_launch()` runs before every check, so each browser, each attach and each WebDriver session gets a fresh instance of a plugin that offers `per_launch`; a shared set no longer shares state, and `on_close` reaches the instance that started.

</Accordion>

<Accordion title="3. The host was always the CDP engine" icon="circle-check">

`check_with(&LaunchContext, &HostInfo)`: `HostInfo::cdp()` for the pure CDP engine, `HostInfo::webdriver(family)` for chromedriver, geckodriver and Safari. A server that answers `browserName: tauri` is re-checked as `HostInfo::embedded()` once the session exists, and a plugin that needs more than a webview gives ends the session.

</Accordion>

<Accordion title="4. New `LaunchSettings` fields were dropped" icon="circle-check">

`host_resolver_rules` is one `--host-resolver-rules`; `disable_features` and `enable_features` are one switch each, merged with the library's own `Translate` (and with a switch the configuration already has); `webrtc_policy` is `--force-webrtc-ip-handling-policy`; `disable_quic` is `--disable-quic`. The profile is shown to plugins before they run, as persistent when `user_data_dir` is set. On Firefox and Safari a plugin that sets one of the Chromium-only fields is refused.

</Accordion>

<Accordion title="5a. `Network.setBlockedURLs` replaced the tab's list" icon="circle-check">

The host keeps one blocklist per tab (per window with chromedriver): the library's ad-block list, the tracker hosts of a fingerprint with `block_trackers`, and every pattern a plugin sent. Each raw send adds to it, and the union goes out. With chromedriver a window keeps its union when the session switches away and back.

</Accordion>

<Accordion title="5b. The `Fetch` domain had many owners" icon="circle-check">

A plugin's `Fetch.*` command, and the older `Network.setRequestInterception` that pauses requests the same way, are refused, on the browser and on a tab. The library's own `Fetch.enable` for proxy passwords is not a plugin's and stays.

</Accordion>

<Accordion title="6. Extension ids were dropped" icon="circle-check">

`Browser::extension_ids()` keeps what `Extensions.loadUnpacked` answered, in load order.

</Accordion>

<Accordion title="7. Two lists of refused switches" icon="circle-check">

A test reads `FORBIDDEN_SWITCHES` of `src/evasion/evasions.rs` and fails if one is missing from `seleniumbase_rs_plugin::switch_policy::REFUSED_SWITCHES`.

</Accordion>

</AccordionGroup>

### How these were checked

On the scripted mock browser of the pure CDP engine (`Browser::launch_mocked`)
and on a mock WebDriver server. They show which commands the host sends, in
which order, and what a plugin is refused.

<Warning>

**No real Chrome ran in this change**, so nothing here shows that Chrome accepts
the merged switches or enforces a merged blocklist in iframes, workers and
popups; the plugins' own docs say the same.

</Warning>

### Limits

- The guard on raw commands is two rules: `Fetch.*` and
  `Network.setRequestInterception` are refused, and `Network.setBlockedURLs`
  is merged. Other commands go to the browser as sent.
  A plugin that reaches `Fetch` by wrapping it in `Target.sendMessageToTarget`
  is not stopped yet; the guarded handles of phase 2 check every command.
- The blocklist is a union: patterns only get added. A plugin cannot take
  another plugin's pattern back. Phase 2 keeps one list per plugin.
- A plugin that declares no metadata is treated as holding the legacy
  capabilities, `raw:cdp` among them, which the host of Firefox, Safari and a
  Tauri webview cannot give. Checked strictly, every such plugin would be
  refused there, though they have always run (their `send` fails when called).
  So a WebDriver set that holds one is checked as for Chromium's WebDriver, and
  skips the embedded re-check; a set of declared plugins only gets the
  browser's own host. A mixed set therefore finds a declared plugin's missing
  DevTools channel when it calls `send`, not at the check.
- The Tauri re-check reads the name `tauri` from the session's capabilities, as
  tauri-plugin-sbrs-automation answers it. Another embedded webview server is
  seen as the browser the configuration names.
