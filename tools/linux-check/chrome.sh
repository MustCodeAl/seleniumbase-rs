#!/bin/sh
# Runs the real-Chrome integration tests headless and writes one summary line per binary.
# Tests that open a real site are left out: local fixture pages only.
# `chrome` is one binary in the library and one in the command-line package, so each
# invocation names the package. Cargo's --skip is a substring of the test path.
cd /work || exit 1
FEATURES="mcp-server test-util testing bdd"
CLI_FEATURES="mcp-server test-util"
SUMMARY=/logs/chrome_summary.txt
: > "$SUMMARY"
mkdir -p /logs/chrome

run() {
  name="$1"
  shift
  timeout 1800 "$@" > "/logs/chrome/$name.log" 2>&1
  code=$?
  echo "$name exit=$code $(grep '^test result' "/logs/chrome/$name.log" | tail -1)" >> "$SUMMARY"
  pkill -f chromium 2>/dev/null
  pkill -f chromedriver 2>/dev/null
}

echo "captcha_live_chrome NOT RUN: opens a real site" >> "$SUMMARY"
echo "examples_parity_chrome NOT RUN: opens a real site" >> "$SUMMARY"
echo "client_hints_chrome NOT RUN: needs a branded Google Chrome, which has no Linux arm64 build" >> "$SUMMARY"
echo "cli_fingerprint_chrome NOT RUN: opens a real site" >> "$SUMMARY"

run library_chrome cargo test -p seleniumbase-rs --features "$FEATURES" --test chrome -- --ignored --test-threads=1 \
  --skip captcha_live_chrome \
  --skip examples_parity_chrome \
  --skip client_hints_chrome \
  --skip typing_lands_in_a_pure_cdp_page_after_a_login_with_a_leaked_password \
  --skip typing_lands_in_a_webdriver_page_after_a_login_with_a_leaked_password

run browser_smoke cargo test -p seleniumbase-rs --features "$FEATURES" --test browser_smoke -- --ignored --test-threads=1

run cli_chrome cargo test -p seleniumbase-rs-cli --features "$CLI_FEATURES" --test chrome -- --ignored --test-threads=1 \
  --skip cli_fingerprint_chrome

run playwright_chrome cargo test -p seleniumbase-rs-playwright --test playwright_chrome -- --ignored --test-threads=1
echo DONE >> "$SUMMARY"
