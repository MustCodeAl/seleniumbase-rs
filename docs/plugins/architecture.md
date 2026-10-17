---
title: "Plugin Architecture"
---

# Plugin architecture

This page summarizes the design of the plugin platform, version 2. The full
record, with the checks behind each decision, is
[ADR 0001 in the plugins repository](https://github.com/MustCodeAl/seleniumbase-rs-plugins/blob/main/docs/adr/0001-plugin-architecture.md)
(number 005 in this repository's series).

<Note>

The ADR is **proposed**. Only phase 1 of its plan has begun, in the plugin API
crate (see [What is built](#what-is-built)). Later phases are a plan, not a
promise of dates.

</Note>

## The design in short

- One `Plugin` trait with more hooks, all with default bodies. The API crate
  stays on 0.1.x and changes only by addition, so existing plugins keep
  working as "legacy" plugins.
- A plugin may describe itself with `metadata()`: a category, a phase, the
  capabilities it needs, and ordering edges.
- Capabilities are checked where a plugin reaches the browser, not trusted.
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
| Low | `launch.args`, `page.script:<hosts>`, `net.observe`, `test.hooks` |
| Medium | `launch.prefs`, `launch.headless`, `page.evaluate`, `net.intercept`, `contexts` |
| High | `page.script:*`, `cookies.*`, `net.bodies`, `launch.proxy`, `launch.extensions`, `host.fs.*` |
| Critical | `raw:*`, `host.process`, `cdp:Target`, `cdp:Browser` |

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
| Rust, third-party | `cargo add` the crate | `.plugin(...)`, or by name through a `Registry` |
| Prebuilt `sbrs` | Nothing, built-ins are compiled in | `--plugin NAME`, `--plugin-opt NAME.KEY=<TOML>` |
| WASM or manifest | `sbrs plugin add` into a content-addressed store | By the name it declares |

WASM and manifest plugins are pinned by SHA-256 in a lockfile and their
capabilities are approved at install. A project's own lock holds pins only,
never grants. WASM ABI 2 puts a JSON manifest in a custom section and runs the
module once at load; event mode (ABI 2.1) comes later.

## The seven example plugins

The ADR specifies seven built-ins that exercise the platform: **shield** and
**tor** (privacy), **fetcher** and **extractor** (core), **batcher**
(environment), **crawler** (testing) and **browser** (extension).

## What is built

Phase 1 starts in the API crate `seleniumbase-rs-plugin`, all additive:

| Piece | File |
|---|---|
| `Capability`, `CapabilitySet` and string forms | `capability.rs` |
| `Category`, `Phase`, `Seals`, `Metadata` | `metadata.rs` |
| Ordering of a set | `order.rs` |
| The refused-switch list, applied to every plugin | `switch_policy.rs` |
| `HostInfo`, which capabilities each host supports | `host.rs` |
| `Configurable`, `Descriptor`, `Registry`, `PluginSpec` | `config.rs` |
| `Plugin::metadata()`, `Plugin::per_launch()`, `PluginSet::strict()`, `with_trusted`, `with_grants`, `audit()` | `lib.rs` |

Not part of this page's claim: the command-line options, the prebuilt
binaries with built-ins, the store, manifests, WASM ABI 2 and the seven
plugins belong to the later work. The plan, from the ADR:

| Phase | Delivers |
|---|---|
| 0 | Security hotfix: one refused-switch list for all plugins |
| 1 | Metadata, registry, CLI and config surface, release features, shield |
| 2 | CDP lifecycle hooks, the shared request router, fetcher, extractor, tor |
| 3 | Tasks, test hooks, store and lockfile, manifest plugins, WASM ABI 2.0 |
| 4 | WebDriver parity, ABI 2.1 events, CRX3 signature checks |

To declare metadata in your own plugin, see
[Writing your own plugin](writing-your-own.md#declaring-metadata-and-capabilities).
