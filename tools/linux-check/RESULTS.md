# Linux check: last results

Run on 2026-11-02 at commit `5e4576dc` (the code of `main` at `679160a3`, plus this folder).

## Machine

| | |
|---|---|
| Host | Apple M1, 8 GB, macOS 27.0, OrbStack |
| Container | Debian 12 (bookworm), Linux arm64, as root, `CI=1` |
| Toolchain | rustc 1.99.1 |
| Browser | Chromium 156.0.8078.73 and ChromeDriver 156.0.8078.73 (Debian packages), headless |

## Image

The repository's `Dockerfile` builds for Linux arm64, and `sbrs --version` in the final image prints
`sbrs 0.7.0 (seleniumbase-rs)`. The images were built with the owner's docker wrapper and
`--no-slim`.

## Suite

`cargo test --no-fail-fast --features "mcp-server test-util testing bdd"`: 199 test binaries,
**5,345 passed, 2 failed**, 382 ignored.

| Test | Cause | Status |
|---|---|---|
| `browser::driver_install::tests::a_listing_that_names_no_build_is_an_error_and_nothing_is_downloaded` | The test assumed Microsoft publishes an msedgedriver for the machine. It does not for linux-aarch64, so the install refuses before it reads the listing. Its two siblings already return early there. | Fixed on `friend/linux-test-fixes` |
| Doctest of `utilities::retry::RateLimiter` (`src/utilities/retry.rs`, line 277) | Timing: the third `reserve()` had to return more than 900 ms of a one-second window, so the three calls had to finish within 100 ms. Under load they did not. Alone, it passed 3 of 3 reruns. | Fixed on `friend/linux-test-fixes` |

The doctests took 7,239 seconds (about two hours) of the run: each of the 345 links its own binary,
with two build jobs.

## Real-Chrome tests

Each `*_chrome.rs` binary, `browser_smoke` and the playwright crate's `playwright_chrome`, one at a
time with `--ignored --test-threads=1`: 76 binaries run, **306 passed, 2 failed**. Three binaries
were not run because they open real sites (see the README). `ocr_chrome` and `visual_chrome` ran no
tests: their features are not in the feature list.

| Binary | Cause | Status |
|---|---|---|
| `client_hints_chrome` | Needs a branded Google Chrome; Chromium reports no `Google Chrome` brand. Its `#[ignore]` reason says so. | Environment. `chrome.sh` now leaves it out. |
| `sb_cdp_plugin_hooks_chrome` | `Page.navigate` got no answer within 30 seconds, once. | Not reproduced: passed 3 of 3 reruns in the same container. |

Everything else passed, including the recorder, frames, captcha fixtures, MCP servers, the CLI end
to end and the playwright bridge.

## Notes for the next run

- A docker wrapper that is a shell alias is not seen by `run.sh`; name it in `DOCKER` (see the
  README).
- The volumes need about 15 GB. The host had 21 GB free after the run.
