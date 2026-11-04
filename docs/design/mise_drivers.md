---
title: "Design: a mise plugin for chromedriver"
---

# Design: a mise plugin for chromedriver

Status: built for the plain chromedriver: a tool plugin in
`contrib/mise/mise-chromedriver` and a `mise which chromedriver` lookup in the
launcher (`src/browser/mise_driver.rs`). The patched UC build is not built.

A [mise](https://mise.jdx.dev) plugin that installs, upgrades and removes
chromedriver. Versions come from the Chrome for Testing JSON this crate already
uses. How to use it: [Manage chromedriver with mise](/building#manage-chromedriver-with-mise).
This page records what was chosen and why, and what is left.

## A tool plugin

mise has several Lua plugin shapes. Pages used here:

- [Plugins](https://mise.jdx.dev/plugins.html) (types, `[plugins]` and `[tools]`)
- [Tool plugins](https://mise.jdx.dev/tool-plugin-development.html) (one tool)
- [Backend plugins](https://mise.jdx.dev/backend-plugin-development.html) (a family of tools under one prefix)
- [Plugin Lua reference](https://mise.jdx.dev/plugin-lua-modules.html) (the modules, `RUNTIME`, and which hook files decide the type)

The first draft of this page chose a backend plugin (`sbrs:chromedriver` and
`sbrs:uc-chromedriver`), because it counted two tools. Only one is built, and
mise's pages say to write a tool plugin for one tool and a backend plugin for a
family. A tool plugin also gives mise the download: the plugin returns a URL,
and mise fetches the archive, unpacks it and runs nothing from it. A backend
plugin installs by itself, and mise does not look at what it downloads.

| Hook | File | What it does here |
| --- | --- | --- |
| `Available` | `hooks/available.lua` | `mise ls-remote` and resolving `151` or `latest`: the builds with a chromedriver for this platform, newest first |
| `PreInstall` | `hooks/pre_install.lua` | the URL of the archive for one version |
| `EnvKeys` | `hooks/env_keys.lua` | puts the install directory on the `PATH` (the archive has no `bin/`) |

`mise plugins link chromedriver ./contrib/mise/mise-chromedriver` is the local
try-out. Upgrade is `mise upgrade chromedriver`, and removal is
`mise uninstall chromedriver@VERSION`; mise deletes the install directory, so
there is no uninstall hook. The commands were run against a throwaway
`MISE_DATA_DIR`, `MISE_CONFIG_DIR` and `MISE_CACHE_DIR`.

## Where the versions come from

`known-good-versions-with-downloads.json` in the Chrome for Testing site: every
build, with the archives of each platform. One document gives both the versions
and the URLs, where the draft named two (`last-known-good-versions-with-downloads.json` for
`latest` and `latest-versions-per-milestone-with-downloads.json` for a major).
Builds before 115 have no chromedriver. The file lists builds oldest first, and
`Available` turns that around.

The URL comes from the index and is never built by the plugin. It must be
`https` on `storage.googleapis.com`, the host the index names, and end in `.zip`.
The index has no checksums, so the plugin returns none.

Chrome for Testing does publish chromedriver for Linux arm64, from build
153.0.8001.0 on (142 of the 2555 builds with a chromedriver). The plugin lists
only those builds on a Linux arm64 machine and refuses an older one for that
platform. It refuses every other platform it has no archive for, and Linux with
musl, with a message that says what to install instead.

## What the library changed

A default Chrome session did not look on `PATH`. `ensure_chromedriver_binary` in
`src/browser/launcher.rs` used only `downloaded_drivers/chromedriver` (or
`chromedriver.exe`), relative to the working directory, and downloaded another
into the same folder when that file was missing or for another Chrome. So a
driver mise put on `PATH` was not the one `sbrs` ran for Chrome.

The first draft proposed a setting, `SB_DRIVER_DIR`, that the plugin would set
for the session. That is not built. The launcher asks mise instead, with no
setting: `mise which chromedriver` in the working directory, run with an
argument list and a 5 second limit. It comes after `downloaded_drivers/` and
before a download, and the file it names must exist, be runnable and fit the
installed Chrome. `--driver-version keep` tries it after `downloaded_drivers/`
and the `PATH`. A pinned `--driver-version` still downloads that build. No mise,
a failing mise, a slow one and a bad answer are silent. The download code does
not know about mise.

Not changed: the lookup that `sbrs doctor`, `sbrs prompt` and the `dev` MCP
server print (`lookup_chromedriver_in` in `src/resources/assets.rs`). It reads
`CHROMEDRIVER_PATH`, `downloaded_drivers/` and the `PATH` and does not start a
program, which keeps the shell prompt fast. The launcher itself does not read
`CHROMEDRIVER_PATH`.

## The patched UC variant (not built)

A `uc-chromedriver` tool would do two steps, in order:

1. `sbrs install chromedriver <version>`, which is the crate's own download into `downloaded_drivers/`.
2. The crate's patcher, `seleniumbase_rs::evasion::patcher::patch_chromedriver` (`src/evasion/patcher.rs`), which calls `ChromedriverPatcher::new(path).patch(EnginePatch::all())`.

Lua cannot call that function. `sbrs patch-chromedriver --path FILE` patches a
file in place, and a file in mise's install folder must not be patched in place:
the hook would patch a copy and put the copy in the install directory. Until
such a plugin exists, `sbrs install uc_driver` is the way to a patched driver.
Do not copy the patch bytes into Lua.
