---
title: "Design: a mise plugin for chromedriver"
---

# Design: a mise plugin for chromedriver

Status: proposal.

A [mise](https://mise.jdx.dev) plugin that installs, upgrades and removes
chromedriver, including a patched UC build. Versions come from the Chrome for
Testing JSON this crate already uses. The plugin does not live in this
repository; this page says what it would be and what the library must change
before a mise-installed driver is the one a session runs.

## What the docs call the plugin

mise has two Lua plugin shapes that can install a binary. Pages used here:

- [Plugins](https://mise.jdx.dev/plugins.html) (types, `[plugins]` and `[tools]`)
- [Tool plugins](https://mise.jdx.dev/tool-plugin-development.html) (one tool)
- [Backend plugins](https://mise.jdx.dev/backend-plugin-development.html) (a family of tools under one prefix)
- [Plugin Lua reference](https://mise.jdx.dev/plugin-lua-modules.html) (which hook files decide the type)

A tool plugin is one tool. chromedriver and the patched UC build are two
tools, so this is a **backend plugin**: users write `sbrs:chromedriver` and
`sbrs:uc-chromedriver` after installing the plugin under the name `sbrs`.

The hook files, and the names mise documents for them:

| Hook | File | Required | When mise calls it |
| --- | --- | --- | --- |
| `BackendListVersions` | `hooks/backend_list_versions.lua` | yes | `mise ls-remote`, and resolving `latest` |
| `BackendInstall` | `hooks/backend_install.lua` | yes | install one version |
| `BackendExecEnv` | `hooks/backend_exec_env.lua` | yes | put that install on the environment |
| `BackendUninstall` | `hooks/backend_uninstall.lua` | no | before mise deletes the install directory |

`metadata.lua` holds the plugin name and version. `mise plugins link sbrs ./mise-sbrs` is the local try-out from the backend-plugin page. A project pins it with:

```toml
[plugins]
sbrs = "https://github.com/example/mise-sbrs#v1"

[tools]
"sbrs:chromedriver" = "latest"
```

Upgrade is a new install of another version (`mise upgrade`, or changing the version in `mise.toml`). mise then calls `BackendInstall` for the new version and `BackendUninstall` for the old one. Removal is `mise uninstall sbrs:chromedriver@<version>`, which calls `BackendUninstall` and deletes the install directory.

## Where the versions come from

The same Chrome for Testing documents `src/browser/chrome_for_testing.rs` and `src/browser/downloader.rs` already fetch:

- `https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json` for `latest` (and stable, beta, dev, canary)
- `https://googlechromelabs.github.io/chrome-for-testing/latest-versions-per-milestone-with-downloads.json` for a milestone such as `131`

`BackendListVersions` reads those documents and returns the chromedriver version strings (the Chrome version the driver belongs to). `BackendInstall` downloads the chromedriver archive for the host (`linux64`, `mac-arm64`, `mac-x64`, `win64`, `win32`) from the URL in that JSON, checks the download, and extracts the `chromedriver` binary into the install directory mise passed in. It does not reimplement the crate's downloader.

## The patched UC variant

`sbrs:uc-chromedriver` is the same version list. Its install does two steps, in order:

1. `sbrs install chromedriver <version>`, which is the crate's own download into `downloaded_drivers/`.
2. The crate's patcher, `seleniumbase_rs::evasion::patcher::patch_chromedriver` (`src/evasion/patcher.rs`), which calls `ChromedriverPatcher::new(path).patch(EnginePatch::all())`.

Lua cannot call that function. There is no `sbrs` subcommand that patches a chromedriver today (`sbrs patch-chrome` patches the browser, not the driver). The UC hook therefore needs a small command, proposed as `sbrs patch-chromedriver <path>`, whose only job is `patch_chromedriver`. The hook then copies the patched binary into the mise install directory and does not leave the session depending on `downloaded_drivers/`.

Until that command exists, the UC tool cannot be installed by the plugin. Do not shell out to a private Rust test or copy the patch bytes into Lua.

## What the library must change

A default Chrome session does not look on `PATH`. `ensure_chromedriver_binary` in `src/browser/launcher.rs` uses only `downloaded_drivers/chromedriver` (or `chromedriver.exe`), relative to the working directory. When that file is missing or the major version does not match Chrome, it downloads another one into the same folder.

`find_downloaded_or_on_path` and `find_driver_program` in the same file do look on `PATH`, but only after `binaries::DOWNLOADS_DIR` (`"downloaded_drivers"`, `src/browser/binaries.rs`). The default plan (`ChromedriverPlan::Default`) never calls them. So a driver mise put on `PATH` is not the one `sbrs` runs for Chrome.

`SB_DRIVER_PATH_DIR` is not that lookup. `sbrs install -p` copies a driver into the directory it names (else `/usr/local/bin`). Nothing reads it back when a session starts.

Add **`SB_DRIVER_DIR`**: a directory of driver binaries. `ensure_chromedriver_binary`, `find_downloaded_or_on_path` and `find_driver_program` search it before `downloaded_drivers`. When it is unset, behaviour stays as it is. The plugin's `BackendExecEnv` sets `SB_DRIVER_DIR` to the install's bin directory (and may also prepend that directory to `PATH`, which helps every other program and does not, by itself, change the default Chrome plan).

Do not teach `src/browser/downloader.rs` about mise. The download stays the plugin's, or `sbrs install`, and the session only needs to find the binary.
