---
title: "Install sbrs with mise"
sidebarTitle: "sbrs with mise"
---

[mise](https://mise.jdx.dev) can install, pin and upgrade `sbrs` and `sbrs-mcp`
without a checkout of this repository. Two of its built-in backends do it, so
there is no plugin to link: `github:` downloads the release archive, and
`cargo:` builds the command line from a git tag.

<Info>

**What you will learn**

- How to install the release archive on a Mac with Apple silicon.
- How to build from a tag on every other platform.
- How one `mise.toml` picks the right backend on each machine.
- How to upgrade, remove, and get the shell completions.

</Info>

<Visibility for="agents">

Summary for AI exports: on macOS arm64, `mise use -g github:MustCodeAl/seleniumbase-rs` installs the release archive and puts `sbrs` and `sbrs-mcp` on the `PATH`; `mise ls-remote github:MustCodeAl/seleniumbase-rs` lists the versions, which are the release tags without the `v` (0.6.0 and 0.7.0 have an archive). `aarch64-apple-darwin` is the only archive the releases carry, so every other platform builds from a tag with `mise use -g 'cargo:https://github.com/MustCodeAl/seleniumbase-rs@tag:v0.7.0'`, which runs `cargo install --git` and needs Rust 1.99 and a C compiler; a git source lists no versions, so name the tag. A team `mise.toml` can hold both entries with `os = ["macos/arm64"]` on the `github:` one and `os = ["linux", "windows", "macos/x64"]` on the `cargo:` one (mise 2026.4.12 or newer). mise installs no completions; the archive holds them in `completions/` under `mise where github:MustCodeAl/seleniumbase-rs`, and `sbrs completions <shell>` writes them for any install.

</Visibility>

## On a Mac with Apple silicon

Each release on GitHub carries a prebuilt archive for `aarch64-apple-darwin`.
The `github:` backend lists the releases, picks that archive and puts both
binaries on the `PATH`:

```bash Terminal
mise ls-remote github:MustCodeAl/seleniumbase-rs   # 0.6.0, 0.7.0, ...
mise use -g github:MustCodeAl/seleniumbase-rs@0.7.0
sbrs --version
sbrs-mcp --version
```

A version is the release tag without its `v`. `mise use` without `@0.7.0` takes
the newest release. Drop `-g` to pin the version in the project's `mise.toml`
instead of your global one.

## On every other platform

No other platform has an archive yet (see
[Install without a checkout](/building#install-without-a-checkout)), so the
`github:` backend stops with an error there. The `cargo:` backend builds the
command line from a git tag instead, with `cargo install --git`:

```bash Terminal
mise use -g rust@1.99
mise use -g 'cargo:https://github.com/MustCodeAl/seleniumbase-rs@tag:v0.7.0'
```

It needs Rust 1.99 and a C compiler, and takes as long as a release build of the
command line. The repository has one package with binaries,
`seleniumbase-rs-cli`, so cargo finds it and installs `sbrs` and `sbrs-mcp`.
Add `features = ["tui"]` to the tool's options for the terminal UI. A git source
lists no versions, so `mise ls-remote` has nothing to offer for it: name the
tag, and change it to upgrade.

## One mise.toml for every machine

A team on mixed machines can keep both entries in one file. mise installs an
entry only on the platforms its `os` option names, so a Mac with Apple silicon
downloads the archive and everything else builds from the same tag:

```toml mise.toml
[tools]
"github:MustCodeAl/seleniumbase-rs" = { version = "0.7.0", os = ["macos/arm64"] }
"cargo:https://github.com/MustCodeAl/seleniumbase-rs" = { version = "tag:v0.7.0", os = ["linux", "windows", "macos/x64"] }
```

An `os` entry with an architecture, such as `macos/arm64`, needs mise 2026.4.12
or newer. Keep the two versions the same when you move to a new release.

## Upgrade and remove

```bash Terminal
mise outdated                                         # the github: entry, with the newest release
mise use -g github:MustCodeAl/seleniumbase-rs@latest
mise uninstall github:MustCodeAl/seleniumbase-rs@0.6.0
```

For the `cargo:` entry, write the new tag in `mise.toml` (or run `mise use`
with it) and mise builds it.

## Shell completions

mise installs no completions. The archive holds them in `completions/` (bash,
zsh, fish, PowerShell, a Fig spec and the usage spec), in the folder that
`mise where github:MustCodeAl/seleniumbase-rs` prints. `sbrs completions <shell>`
writes them for any install, the `cargo:` one included; see
[the CLI page](/tutorials/cli_usage).

## What was checked

On macOS arm64 with mise 2026.10.25: `mise ls-remote` lists 0.6.0 and 0.7.0,
`mise install` puts the 0.7.0 archive in place, `sbrs --version` and
`sbrs-mcp --version` print 0.7.0, and with the `mise.toml` above mise installs
the `github:` entry and skips the `cargo:` one. The `cargo:` route was not built
for this page; it is the same `cargo install --git` that
[Install without a checkout](/building#install-without-a-checkout) describes.

For chromedriver, see [Manage chromedriver with mise](/tutorials/mise_chromedriver).
