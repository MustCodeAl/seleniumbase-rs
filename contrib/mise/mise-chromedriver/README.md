# mise-chromedriver

A [mise](https://mise.jdx.dev) tool plugin that installs, upgrades and removes
chromedriver from [Chrome for Testing](https://googlechromelabs.github.io/chrome-for-testing/).
`sbrs` and the library find the driver it installs with `mise which chromedriver`;
see `docs/tutorials/mise_chromedriver.md`.

It is a tool plugin because it installs one tool. mise's plugin pages say to write
a tool plugin when no backend can install the tool and a backend plugin only for a
family of tools. As a tool plugin it returns the download and mise does the rest:
it fetches the archive, unpacks it, and runs nothing that the archive contains.

## Use it

```sh
mise plugins link chromedriver ./contrib/mise/mise-chromedriver
mise ls-remote chromedriver 151        # the builds of Chrome 151
mise use chromedriver@151              # install the newest 151 and pin it in mise.toml
mise exec -- chromedriver --version
mise upgrade chromedriver              # a newer build of the pinned major
mise uninstall chromedriver@151.0.7922.71
```

The link name is the tool name. Linked as `chromedriver`, the plugin is used
instead of the registry's `chromedriver` entry.

Pin the major version of your Chrome (`151`), not `latest`: the index lists every
build, Canary builds included, so `latest` is usually newer than any Chrome you
have, and a chromedriver only drives the Chrome of its own major version.

## What it installs

- The versions are the builds in `known-good-versions-with-downloads.json` that
  have a chromedriver for the platform of the machine mise runs on, newest first.
  Builds before 115 have none.
- The download is the URL the index gives for that build, which must be `https`
  on `storage.googleapis.com` and end in `.zip`; any other URL stops the install.
- The index publishes no checksums, so the plugin returns none and mise has
  nothing to check the archive against: only the https connection to Google's
  storage protects the download. `mise lock` records the URL and the SHA-256 of
  what mise downloaded, but mise 2026.10.25 installed a build whose recorded
  checksum had been changed by hand, even with `--locked`, so the lockfile is not
  a check of the archive.
- The archive's one folder is unpacked into the install directory, so
  `chromedriver` sits at its top: `mise which chromedriver` prints
  `.../installs/chromedriver/<version>/chromedriver` (`<version>` is `151` when
  `mise.toml` pins `151`).

Platforms: Linux x64, Linux arm64, macOS Intel, macOS Apple silicon, Windows
x64 and Windows x86. Chrome for Testing builds chromedriver for Linux arm64 only
from 153 on, so a Linux arm64 machine lists only those builds, and locking an
older build for that platform fails with a message that says so. Any other
platform, and Linux with musl (Alpine), is refused with an error instead of
being given a binary that would not run; install the system's chromedriver
package there and put it on the `PATH`.

## Files

- `metadata.lua`: the plugin's name and version.
- `hooks/available.lua`: `mise ls-remote` and version resolution.
- `hooks/pre_install.lua`: the download for one version.
- `hooks/env_keys.lua`: puts the install directory on the `PATH`.
- `lib/cft.lua`: the index, the platform names and the check of the URL.
- `test/cft_test.lua`: checks of `lib/cft.lua` that need neither mise nor the
  network: `cd contrib/mise/mise-chromedriver && luajit test/cft_test.lua`.

mise may print that a build "has no release date": the index has no dates, so
mise's `minimum_release_age` cannot be checked and the build is allowed. Add the
tool to `minimum_release_age_excludes` to silence it.
