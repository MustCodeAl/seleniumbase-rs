# Operating systems

What differs between macOS, Linux and Windows when you build and run
`seleniumbase-rs`. The CI workflow (`.github/workflows/ci.yml`) builds and tests
on all three; day-to-day development and the project's own checks run on macOS
(Apple silicon), so macOS is the best-trodden path. The test suite was also run
on Linux (arm64, Debian 12, Chromium 154) in a container; what passed, and what
was not run, is under [Verified on Linux](#verified-on-linux).

## Which Chrome it finds

The library looks in the usual install locations, and you can always name one:

| System | Looked at | Override |
|---|---|---|
| macOS | `/Applications/Google Chrome.app`, `~/Applications/Google Chrome.app`, Chromium, Chrome Canary | `SB_CHROME_BIN` or `CHROME_BIN` |
| Linux | `google-chrome`, `google-chrome-stable`, `chromium` and `chromium-browser` in `/usr/bin` (and `/snap/bin/chromium`), then the same names on the `PATH` | `SB_CHROME_BIN`, `CHROME_BIN` or `CHROMIUM_BIN` |
| Windows | `Program Files`, `Program Files (x86)` and the per-user `Google\Chrome\Application\chrome.exe` | `SB_CHROME_BIN` or `CHROME_BIN` |

For the WebDriver engine, `sbase install` downloads the chromedriver that matches
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

## macOS

- **A freshly written or replaced executable is slow the first time.** macOS
  scans it on first run, which can take seconds. The library writes a replaced
  chromedriver as a new file because macOS kills a signed binary that is
  overwritten in place; if a driver is "killed" or refused, run `sbase install`
  again and see [Common problems](help/common_problems.md).
- **Gatekeeper and quarantine.** A browser or driver downloaded by another tool
  can carry a quarantine flag; clear it with
  `xattr -d com.apple.quarantine <file>` if macOS refuses to open it.
- **Tests that fake `java` or other scripts** must run the script once before
  timing it, for the same first-run reason.
- **Python library errors in tests.** If a test binary cannot load `libpython`,
  set `DYLD_LIBRARY_PATH` to your Python's `lib` directory (see
  `CONTRIBUTING.md`).
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
  set. That weakens Chrome's isolation, so use it only in a container you control.
- **No display.** Run headless (`SB_HEADLESS=true`; `sbase open` and the other
  browser commands read it only with `--config`, so pass `--headless` to them). To
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
The machine was an 8 GB Apple M1 Mac running OrbStack, so the containers were
**linux/arm64** on **Debian 12 (bookworm)** with **Rust 1.99** and **Chromium
154.0.8037.92** from Debian's `chromium` and `chromium-driver` packages. Cargo ran
with `CARGO_BUILD_JOBS=2` in a container limited to 4 GB.

| Run | Result |
|---|---|
| Library tests, `--features "mcp-server test-util scrape"` | 2112 passed, 0 failed, 16 ignored |
| The other test binaries of that feature set (68) | 406 passed, 0 failed, 114 ignored (the real-browser tests, ignored by default) |
| Doc tests, same features | 182 passed, 0 failed, 10 ignored |
| `--features "impersonate test-util" --lib impersonate` (builds BoringSSL) | 291 passed |
| `impersonate_http`, `impersonate_mitm`, `stealth_session`, `impersonate_wire`, `impersonate_capture_h2` | 83 + 49 + 32 + 39 + 6 = 209 passed, 0 failed (four more runs gave the same) |
| Real Chromium 154: `browser_smoke` and 17 `*_chrome` binaries, one at a time with `-- --ignored --test-threads=1` | 112 passed, 0 failed (headless, except the 2 of `scrape_capture_chrome`, which ran headed on a virtual display) |
| Real-Chrome tests of the measured macOS persona, 4 binaries (14 tests) | 14 pass: 1 ran, 13 skip themselves (below) |
| `cargo test --test dockerfile_skeleton` | 9 passed |

What the runs showed:

- **A real bug, fixed:** the chromedriver download used the x86-64 `linux64` build on
  every Linux machine, so an arm64 container failed with "chromedriver exited as it
  started". It now asks for `linux-arm64`: `sbase install` in the arm64 image
  fetches an aarch64 executable, and `browser_smoke` (7 tests) starts Chromium
  through a driver it downloaded that way.
- **A flaky test, fixed (not Linux-specific):** `a_custom_ja3_arrives_exactly_as_given`
  failed on about two runs in five, because BoringSSL pads a ClientHello of 256 to
  511 bytes to 512 and the random ECH GREASE size moves one variant across that
  limit. The test now asserts that rule.
- **Tests of the measured persona skip on Linux.** Only Chrome 155 on macOS is
  measured, so a persona for Linux is refused with `NotMeasured`, and Debian's
  Chromium 154 is not the measured major either. The four binaries
  `impersonate_capture_h2_chrome`, `impersonate_http_chrome`,
  `impersonate_mitm_chrome` and `stealth_session_chrome` print a note and return
  where the host system has no measurement (run with `-- --nocapture` to see it),
  so they pass without testing anything there. A macOS machine with another Chrome major still
  fails them, which is the drift check.
- **Container limits, not bugs:** the doc tests need `-- --test-threads=2` in 4 GB
  (with eight threads the linker was killed for lack of memory in 66 of them); the
  headed measurement tools need a display, which Xvfb gives; Chromium needs
  `--no-sandbox` as root.

Not run: x86-64 Linux, Google Chrome (only Debian's Chromium), other Linux
distributions, the `playwright`, `gui`, `tui`, `s3`, `azure`, `turso` and `gcp`
features, `stealth_audit_network` (it needs outside sites), `playwright_chrome`, and
the macOS-only `driver_repair_macos`. Whether the pure CDP engine starts as the
non-root `sbase` user of the image with the sandbox on was not tried.

## Windows

- Install Google Chrome normally; the library finds it under `Program Files`. CI
  installs a Chrome and chromedriver on Windows runners for the tests that need
  one.
- Use forward slashes or raw strings for paths in Rust code, and quote paths with
  spaces in shell commands.
- Long paths: build in a short directory (`C:\src\seleniumbase-rs`) if a deep
  `target\` path trips the 260-character limit.
- The `impersonate` feature builds BoringSSL and needs `cmake` and a C/C++
  toolchain (Visual Studio Build Tools). Only part of that feature is written (see
  [Stealth Scraping](tutorials/stealth_scraping.md)).
- Measured browser fingerprint data is macOS-only for now (Chrome 155); presets
  for other systems are plausible values, not measurements, and the TLS and
  HTTP/2 tables refuse any other system with `NotMeasured`.

## WSL and remote machines

Run the Linux steps inside WSL. A browser inside WSL has no display unless you
use WSLg or run headless. To drive a browser on another machine, run
`seleniumbase-mcp` or your tests there, or point the WebDriver engine at a
Selenium Grid (`sbase grid`) with `BrowserConfig::webdriver_url`.

## Check the setup on any system

```bash
sbase doctor       # Chrome and chromedriver versions, and whether they match
sbase install      # fetch the matching chromedriver
```

From an AI agent, the MCP tool `diagnose_environment` reports the same plus the
platform, the redacted `SB_*` settings and the output directory.
