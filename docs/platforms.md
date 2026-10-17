---
title: "Operating systems"
---

# Operating systems

What differs between macOS, Linux and Windows when you build and run
`seleniumbase-rs`. The CI workflow (`.github/workflows/ci.yml`) is set up to
build and test on all three, but has not been able to run yet; day-to-day
development and the project's own checks run on macOS (Apple silicon), so macOS
is the best-trodden path, and Windows 11 was checked by hand (see
[Windows](#windows)). The test suite was also run
on Linux (arm64, Debian 12, Chromium 154) in a container; what passed, and what
was not run, is under [Verified on Linux](#verified-on-linux).

The packages each system and each feature needs, with the commands that install
them (Homebrew, apt, dnf, winget), are in
[Building and installing](building.md#what-you-need).

## Which Chrome it finds

The library looks in the usual install locations, and you can always name one:

<Tabs>

<Tab title="macOS">

- **Looked at:** `/Applications/Google Chrome.app`, `~/Applications/Google Chrome.app`, Chromium, Chrome Canary
- **Override:** `SB_CHROME_BIN` or `CHROME_BIN`

</Tab>

<Tab title="Linux">

- **Looked at:** `google-chrome`, `google-chrome-stable`, `chromium` and `chromium-browser` in `/usr/bin` (and `/snap/bin/chromium`), then the same names on the `PATH`
- **Override:** `SB_CHROME_BIN`, `CHROME_BIN` or `CHROMIUM_BIN`

</Tab>

<Tab title="Windows">

- **Looked at:** `Program Files`, `Program Files (x86)` and the per-user `Google\Chrome\Application\chrome.exe`
- **Override:** `SB_CHROME_BIN` or `CHROME_BIN`

</Tab>

</Tabs>

For the WebDriver engine, `sbrs install` downloads the chromedriver that matches
your Chrome's major version, and `CHROMEDRIVER_PATH` points at one you already
have. The pure CDP engine (`sb_cdp`) needs no chromedriver on any system.

The WebDriver engine looks for `downloaded_drivers/chromedriver` in the working
directory first and uses it when its major version fits the browser; otherwise it
downloads the matching [Chrome for Testing](https://googlechromelabs.github.io/chrome-for-testing/)
driver. That download is `linux64` on x86-64 Linux and `linux-arm64` on arm64
Linux, which Chrome for Testing publishes from Chrome 153 on; for an older Chrome
on arm64, or any other Linux architecture, the error says that no build is
published and where to put a driver. A driver from your distribution works the
same way: link it into that folder, as the repository's `Dockerfile` does, and no
network is needed:

```bash
mkdir -p downloaded_drivers && ln -s /usr/bin/chromedriver downloaded_drivers/chromedriver
```

### Firefox and Edge

A Firefox session starts **geckodriver** and an Edge session **msedgedriver**,
as Python SeleniumBase does. Each is looked for in `downloaded_drivers/` (right
in it, or in the newest `geckodriver-v<version>/` or
`msedgedriver-<version>/` folder there) and then on the `PATH`, so a Homebrew or
distribution geckodriver is used as it is. When none is found:

| Driver | Downloaded from | Which build |
|---|---|---|
| geckodriver | `github.com/mozilla/geckodriver/releases` | The newest release; one geckodriver drives a range of Firefox versions, so it is not matched to Firefox. `sbrs get geckodriver 0.36.0` pins one. |
| msedgedriver | `msedgedriver.microsoft.com` | The newest build for the installed Edge's major version (`LATEST_RELEASE_<major>_<OS>`), or the stable one when Edge cannot be found. A driver on disk or on the `PATH` for another Edge major version is replaced; `with_driver_version` pins one. |

The archives (tar.gz for geckodriver on Linux and macOS, zip otherwise) are
capped at 64 MB, unpacked without writing outside their folder, and made
runnable the way chromedriver is: the execute bit, and on macOS no quarantine
flag and, if the system kills the driver as it starts, a new ad-hoc signature.
Edge is looked for at `/Applications/Microsoft Edge.app` on macOS,
`Microsoft\Edge\Application\msedge.exe` under `Program Files`,
`Program Files (x86)` and the per-user folder on Windows, and
`microsoft-edge`, `microsoft-edge-stable` or `/opt/microsoft/msedge/msedge` on
Linux. Microsoft publishes no msedgedriver for arm64 Linux.

The session's options go in `moz:firefoxOptions` (headless, arguments,
preferences, the user agent and locale as preferences, `browser_binary_path` as
the Firefox program and `user_data_dir` as `-profile`) and `ms:edgeOptions` (the
same options Chrome gets). Chrome-only settings such as extensions, mobile
emulation and the CDP and UC modes have no Firefox
equivalent and are not sent; the session proxy is not passed to Firefox either.

## macOS

- **A freshly written or replaced executable is slow the first time.** macOS
  scans it on first run, which can take seconds. The library writes a replaced
  chromedriver as a new file because macOS kills a signed binary that is
  overwritten in place; if a driver is "killed" or refused, run `sbrs install`
  again and see [Common problems](help/common_problems.md).
- **Gatekeeper and quarantine.** A browser or driver downloaded by another tool
  can carry a quarantine flag; clear it with
  `xattr -d com.apple.quarantine <file>` if macOS refuses to open it.
- **Tests that fake `java` or other scripts** must run the script once before
  timing it, for the same first-run reason.
- **Memory.** On an 8 GB machine keep `CARGO_BUILD_JOBS` low and run one cargo
  command at a time.

## Linux

- **Install a browser from your distribution** (`chromium` and, for the
  WebDriver engine, `chromium-driver` on Debian and Ubuntu) or use Google
  Chrome's own package. The repository's `Dockerfile` shows the Debian setup.
- **Running as root or in a container** usually needs `--no-sandbox`. The
  WebDriver engine always passes it on Linux. For the pure CDP engine set
  `SB_NO_SANDBOX` (it is read as set or not set: `SB_NO_SANDBOX=0` also turns the
  sandbox off), or call `LaunchOptionsBuilder::no_sandbox(true)`. The real-browser
  tests of this repository read `CI` instead and pass `--no-sandbox` when it is
  set.

  <Warning>

  `--no-sandbox` weakens Chrome's isolation, so use it only in a container you control.

  </Warning>

- **No display.** Run headless (`SB_HEADLESS=true`, which `sbrs open` and the other
  browser commands read, or `--headless` before the subcommand). To
  see a window on a server, or to run the few tests that need a headed Chrome (the
  measurement tools), run under a virtual display such as Xvfb. Start `Xvfb` in the
  background and export `DISPLAY`; `xvfb-run` can wait forever for a signal that a
  background process ignores. The `gui` feature (native dialogs and GUI
  automation) links GTK and X11 and is off by default; the `tui` feature has no
  such requirement.
- **Shared memory.** In Docker, Chromium can crash under load with the default
  `/dev/shm`; start the container with `--shm-size=1g` (general Chromium advice).
- **Wayland.** The `gui` feature's input automation targets X11; the project has
  not tested it under Wayland.

### Verified on Linux

Run on 2026-10-11 to 12 with `docker/Dockerfile.verify` and `docker/verify-linux.sh`
(commands in [Building and installing](building.md#run-the-tests-in-a-container)).
The host was an Apple-silicon Mac, so the containers were
**linux/arm64** on **Debian 12 (bookworm)** with **Rust 1.99** and **Chromium
154.0.8037.92** from Debian's `chromium` and `chromium-driver` packages. Cargo ran
with `CARGO_BUILD_JOBS=2` in a container limited to 4 GB.

<Info>

These runs predate 0.3.0: the feature set still had `scrape`, so the counts below
include the tests of the stealth layer, which has since moved to
[seleniumbase-rs-stealth](stealth/overview.md)
(its `impersonate` runs, the measured-persona checks and a flaky JA3 test fixed
then are no longer part of this crate).

</Info>

| Run | Result |
|---|---|
| Library tests, `--features "mcp-server test-util scrape"` | 2112 passed, 0 failed, 16 ignored |
| The other test binaries of that feature set (68) | 406 passed, 0 failed, 114 ignored (the real-browser tests, ignored by default) |
| Doc tests, same features | 182 passed, 0 failed, 10 ignored |
| Real Chromium 154: `browser_smoke` and 17 `*_chrome` binaries, one at a time with `-- --ignored --test-threads=1` | 112 passed, 0 failed (headless, except the 2 of `scrape_capture_chrome`, now in seleniumbase-rs-stealth, which ran headed on a virtual display) |
| `cargo test --test dockerfile_skeleton` | 9 passed |

What the runs showed:

- **A real bug, fixed:** the chromedriver download used the x86-64 `linux64` build on
  every Linux machine, so an arm64 container failed with "chromedriver exited as it
  started". It now asks for `linux-arm64`: `sbrs install` in the arm64 image
  fetches an aarch64 executable, and `browser_smoke` (7 tests) starts Chromium
  through a driver it downloaded that way.
- **Container limits, not bugs:** the doc tests need `-- --test-threads=2` in 4 GB
  (with eight threads the linker was killed for lack of memory in 66 of them); a
  headed browser needs a display, which Xvfb gives; Chromium needs `--no-sandbox`
  as root.

<Note>

Not run: x86-64 Linux, Google Chrome (only Debian's Chromium), other Linux
distributions, the `playwright`, `gui`, `tui`, `s3`, `azure`, `turso` and `gcp`
features, `stealth_audit_network` (it needs outside sites), `playwright_chrome`, and
the macOS-only `driver_repair_macos`. Whether the pure CDP engine starts as the
non-root `sbrs` user of the image with the sandbox on was not tried.

</Note>

## Windows

**Status:** on Windows 11 x64 (Rust 1.99, Chrome 154 and 155, Edge 155, Firefox
153) formatting, clippy, the whole test suite and the real-browser tests pass,
and the Tauri example builds its `.msi` and setup `.exe`. (The stealth layer,
then the `impersonate` feature, built and passed there too; it now lives in
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
with its CMake, NASM and LLVM requirements.) Install steps:
[Building and installing](building.md#what-you-need).

**What works differently**

- The Chrome and Edge version comes from the program file's own version
  (`chrome.exe --version` prints nothing on Windows). While an update waits for a
  restart, that is still the version that runs, so the right driver is fetched.
- Download folders are given to Chrome as plain `C:\...` paths.
- Build in a short folder (`C:\src\seleniumbase-rs`) if a deep `target\` path
  hits the 260-character limit. In Rust code, use raw strings for Windows paths.

<Warning>

**Not there yet on Windows**

- If the program that started a browser is killed, the browser can keep
  running (macOS and Linux end it; Windows needs a job object). A driver whose
  session fails, or that is dropped, is ended.
- The tests' leftover-process checks list processes with `sysinfo` now, so
  they run on Windows too, but they have not been run on a Windows PC yet.
- Measured fingerprints exist for Chrome 155 on macOS only; the TLS and HTTP/2
  tables refuse other systems with `NotMeasured`.

</Warning>

## WSL and remote machines

Run the Linux steps inside WSL. A browser inside WSL has no display unless you
use WSLg or run headless. To drive a browser on another machine, run
`sbrs-mcp` or your tests there, or point the WebDriver engine at a
Selenium Grid (`sbrs grid`) with `BrowserConfig::webdriver_url`.

## Check the setup on any system

```bash
sbrs doctor       # Chrome and chromedriver versions, and whether they match
sbrs install      # fetch the matching chromedriver
```

From an AI agent, the MCP tool `diagnose_environment` reports the same plus the
platform, the redacted `SB_*` settings and the output directory.
