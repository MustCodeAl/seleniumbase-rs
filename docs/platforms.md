# Operating systems

What differs between macOS, Linux and Windows when you build and run
`seleniumbase-rs`. The CI workflow (`.github/workflows/ci.yml`) builds and tests
on all three; day-to-day development and the project's own checks run on macOS
(Apple silicon), so macOS is the best-trodden path.

## Which Chrome it finds

The library looks in the usual install locations, and you can always name one:

| System | Looked at | Override |
|---|---|---|
| macOS | `/Applications/Google Chrome.app`, `~/Applications/Google Chrome.app`, Chromium, Chrome Canary | `SB_CHROME_BIN` or `CHROME_BIN` |
| Linux | `/usr/bin/google-chrome` and the Chromium packages' usual names | `SB_CHROME_BIN` or `CHROME_BIN` |
| Windows | `Program Files`, `Program Files (x86)` and the per-user `Google\Chrome\Application\chrome.exe` | `SB_CHROME_BIN` or `CHROME_BIN` |

For the WebDriver engine, `sbase install` downloads the chromedriver that matches
your Chrome's major version, and `CHROMEDRIVER_PATH` points at one you already
have. The pure CDP engine (`sb_cdp`) needs no chromedriver on any system.

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
- **Running as root or in a container** usually needs `--no-sandbox`:
  set `SB_NO_SANDBOX=1`, or call `LaunchOptionsBuilder::no_sandbox(true)`. That
  weakens Chrome's isolation, so use it only in a container you control.
- **No display.** Run headless (`SB_HEADLESS=true`). To see a window on a server,
  run under a virtual display such as Xvfb. The `gui` feature (native dialogs and
  GUI automation) links GTK and X11 and is off by default; the `tui` feature has
  no such requirement.
- **Shared memory.** In Docker, Chromium can crash under load with the default
  `/dev/shm`; start the container with `--shm-size=1g` (general Chromium advice).
- **Wayland.** The `gui` feature's input automation targets X11; the project has
  not tested it under Wayland.

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
