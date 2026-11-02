#!/bin/sh
# Runs each real-Chrome test binary by itself, headless, and writes one summary line per binary.
# The binaries and tests that open a real site are left out: local fixture pages only.
cd /work || exit 1
FEATURES="mcp-server test-util testing bdd"
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

for path in tests/*_chrome.rs tests/browser_smoke.rs crates/cli/tests/*_chrome.rs; do
  name=$(basename "$path" .rs)
  case "$name" in
    captcha_live_chrome|examples_parity_chrome|cli_fingerprint_chrome)
      echo "$name NOT RUN: opens a real site" >> "$SUMMARY"
      ;;
    password_prompt_chrome)
      run "$name" cargo test --features "$FEATURES" --test "$name" -- --ignored --test-threads=1 \
        --skip typing_lands_in_a_pure_cdp_page_after_a_login_with_a_leaked_password \
        --skip typing_lands_in_a_webdriver_page_after_a_login_with_a_leaked_password
      ;;
    *)
      run "$name" cargo test --features "$FEATURES" --test "$name" -- --ignored --test-threads=1
      ;;
  esac
done
run playwright_chrome cargo test -p seleniumbase-rs-playwright --test playwright_chrome -- --ignored --test-threads=1
echo DONE >> "$SUMMARY"
