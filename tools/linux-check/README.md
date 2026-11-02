# Linux check

Builds the repository's `Dockerfile` for the machine's Linux architecture, then runs the test suite
and the real-Chrome tests in a container, headless, one binary at a time. On an Apple silicon Mac
with OrbStack that is Linux arm64.

```bash
mise run linux-check            # build, suite, then Chrome tests
mise run linux-check suite      # one step: build, suite or chrome
```

The commit under test is the checkout's `HEAD`, or `LINUX_CHECK_COMMIT`. The container clones it from
GitHub, so it must be pushed. Logs go to `~/.cache/sbrs-linux-check`, or `LINUX_CHECK_LOGS`:
`suite.log`, `chrome_summary.txt` (one line per binary) and `chrome/<binary>.log`.

The volumes `sbrs-linux-check-work` and `sbrs-linux-check-cargo` keep the clone and the compiled
work, so a second run reuses them. Delete them with `docker volume rm` to start clean. They hold about
15 GB. Check `df -h /` first.

## What differs from a normal Linux machine

- The tests run as root with `CI=1`, and `Dockerfile.test` adds `--no-sandbox` to every Chromium
  start: the sandbox cannot start in an unprivileged container.
- The browser is Debian's `chromium`, with Debian's `chromedriver` linked at
  `downloaded_drivers/chromedriver`, as the runtime stage of the `Dockerfile` does it.
- The images are built with `--no-slim`, so this does not check the Slim pass of a local Docker wrapper.
- No display: a test that needs a headed browser fails here.

## What it leaves out

These open real sites, and this check uses local fixture pages only:

- `captcha_live_chrome`: a real Cloudflare-protected site, for a person to run on purpose.
- `examples_parity_chrome`: seleniumbase.io and saucedemo.com.
- `cli_fingerprint_chrome`: example.com.
- The two `..._after_a_login_with_a_leaked_password` tests in `password_prompt_chrome`: saucedemo.com.
