---
title: "Manage chromedriver with mise"
sidebarTitle: "chromedriver with mise"
---

[mise](https://mise.jdx.dev) can install, upgrade and remove chromedriver for you,
and the WebDriver engine runs the driver it manages. This page shows the plugin
in `contrib/mise/mise-chromedriver`, how a session finds the driver, and what it
does not do.

<Info>

**What you will learn**

- How to install the plugin and a chromedriver with it.
- How to upgrade and remove the driver.
- Where a session looks for a driver, and where mise comes in.
- Which platforms the plugin supports.

</Info>

<Visibility for="agents">

Summary for AI exports: link the plugin with `mise plugins link chromedriver ./contrib/mise/mise-chromedriver`, then `mise use chromedriver@151` (the major version of your Chrome; `latest` is usually a Canary build). `mise upgrade chromedriver` installs a newer build of the pinned major (mise prunes the old one after a day, or at once with `mise prune chromedriver`) and `mise uninstall chromedriver@VERSION` removes one. A session started by `sbrs` or the library runs `mise which chromedriver` in the working directory when `downloaded_drivers/chromedriver` is missing or is for another Chrome (and, with `--driver-version keep`, when no driver is in `downloaded_drivers/` or on the `PATH`), and uses the file it prints when it exists, can run and fits the installed Chrome. No mise, a failing mise, a mise that takes more than 5 seconds and a bad answer are all silent. There is no setting for it. `--driver-version` with a version always downloads that build. `sbrs doctor` and `sbrs prompt` do not ask mise. The plugin lists builds from Chrome for Testing, and installs only what the index names on storage.googleapis.com over https.

</Visibility>

## Install the plugin and a driver

The plugin is a folder of Lua files. Link it from a checkout of this repository,
under the name `chromedriver`, then pin the major version of your Chrome:

```bash Terminal
mise plugins link chromedriver ./contrib/mise/mise-chromedriver
mise ls-remote chromedriver 151
mise use chromedriver@151
mise exec -- chromedriver --version
```

`mise use` installs the newest build of Chrome 151 that has a chromedriver for
your platform and writes `chromedriver = "151"` under `[tools]` in `mise.toml`.
Pin the major, not `latest`: the list holds every build Chrome for Testing has
published, Canary builds included, and a chromedriver only drives the Chrome of
its own major version.

To share the plugin with a team, copy the folder into a repository of its own
and install it from there as mise's plugin pages describe, with a `[plugins]`
entry in `mise.toml`.

## Upgrade and remove

```bash Terminal
mise outdated                  # shows 151 with the build installed and the newest one
mise upgrade chromedriver      # installs the newest 151; mise prunes the old build after a day
mise prune chromedriver        # removes the builds no mise.toml selects, now
mise uninstall chromedriver@151.0.7922.71
mise uninstall --all chromedriver
```

## Where a session looks for a driver

With the default WebDriver URL, a session first uses a driver that is already
listening there. Otherwise the launcher looks for a chromedriver in this order
and stops at the first that fits:

1. `downloaded_drivers/chromedriver`, when its major version is the installed
   Chrome's, or when either version cannot be read.
2. The chromedriver mise manages for the working directory, found with
   `mise which chromedriver`, under the same version check.
3. A download for the installed Chrome into `downloaded_drivers/`.

`--driver-version keep` takes any chromedriver in `downloaded_drivers/` or on the
`PATH` first, then the one mise manages, and downloads only when there is none.
A pinned `--driver-version` (`151`, `stable`, a full build) downloads that build
and does not ask mise. Brave, Comet, Chrome for Testing, chrome-headless-shell and a
session that downloads the driver through a proxy follow the version of their own
browser in the same way, so they do not ask mise either.

Asking mise is cheap and silent. Nothing is logged above debug level, and the
launcher goes on to the next place, when:

- there is no `mise` on the `PATH`;
- `mise which chromedriver` exits with a non-zero status, for example because
  no chromedriver is installed for the directory;
- mise does not answer within 5 seconds;
- the path it prints does not exist, is not a file the system can run, or is not
  a full path.

mise is started with an argument list, never through a shell. The answer depends
on the working directory, so a project's `mise.toml` decides which build runs.
A driver for another Chrome than the installed one is passed over with a warning.

<Warning>

`sbrs doctor` and `sbrs prompt` look for a driver in `CHROMEDRIVER_PATH`,
`downloaded_drivers/` and the `PATH` and do not ask mise, so they can report
no chromedriver while a session runs the one mise manages. If your shell runs
`mise activate`, the driver is on the `PATH` and they find it there.

</Warning>

## The driver stays as mise installed it

The launcher runs the driver where mise put it and does not patch it. The one
thing it can change is what it does for every driver the system refuses to
start: add the execute bit, remove the macOS quarantine flag, and sign a driver
that macOS kills as it starts. A driver that mise installed started on macOS
without any of that (the file's SHA-256, mode and time were the same after the
run). To use a patched driver, make a copy
and patch the copy: `sbrs install uc_driver` does that in `downloaded_drivers/`,
and `sbrs patch-chromedriver --path FILE` patches the file it is given in place,
so give it a copy and not the file in mise's folder.

## Platforms

Linux x64, Linux arm64, macOS Intel and Apple silicon, Windows x64 and Windows
x86. Chrome for Testing publishes chromedriver for Linux arm64 only from build
153 on; on that platform the plugin lists those builds, and asking for an older
one fails with a message that says so. Any other platform, and Linux with musl
(Alpine), is refused instead of being given a binary that would not run: install
the system's chromedriver package there. The plugin's own README lists what it
checks and what it does not: the index has no checksums, so the download is
protected by https alone.
