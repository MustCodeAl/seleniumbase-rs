#!/bin/sh
# Runs a command in a private copy of the repository, inside the verification image.
#
# The image (docker/Dockerfile.verify) uses this script as its entry point. The repository is
# mounted read-only at /src; the script copies it to /work (without .git, target or tool state)
# so that tests which write beside their sources cannot touch the checkout, and then runs the
# command it was given from /work. Modification times survive the copy, so Cargo does not see
# every file as changed on the next run.
#
# From the repository root (the numbers are the ones this project was verified with):
#
#   docker build -f docker/Dockerfile.verify -t seleniumbase-rs:verify-tools .
#   docker volume create sbrs-verify-target
#   docker volume create sbrs-verify-registry
#   docker run --rm --memory=4g --shm-size=1g -e CARGO_BUILD_JOBS=2 \
#       -v "$PWD":/src:ro -v sbrs-verify-target:/target \
#       -v sbrs-verify-registry:/usr/local/cargo/registry \
#       seleniumbase-rs:verify-tools \
#       cargo test --features "mcp-server test-util scrape" --no-fail-fast
#
# A real-browser test binary, one at a time:
#
#   ... seleniumbase-rs:verify-tools \
#       cargo test --features "mcp-server test-util scrape" --test sb_cdp_chrome \
#       -- --ignored --test-threads=1
#
# The few tests that need a headed browser (the measurement tools) get a virtual display with
# `-e SBRS_XVFB=1`:
#
#   ... -e SBRS_XVFB=1 seleniumbase-rs:verify-tools \
#       cargo test --features "mcp-server test-util scrape" --test scrape_capture_chrome \
#       -- --ignored --test-threads=1
#
# The image sets CI=true and SB_NO_SANDBOX=1 because Chromium refuses to start its sandbox as
# root in a container (the real-browser tests read CI), and SB_HEADLESS=true because the
# container has no display.
set -eu

src=${SBRS_SRC:-/src}
work=${SBRS_WORK:-/work}

if [ ! -f "$src/Cargo.toml" ]; then
    echo "verify-linux: mount the repository at $src (docker run -v \"\$PWD\":$src:ro ...)" >&2
    exit 64
fi

# SBRS_XVFB=1 starts a virtual display on :99 for the tests that need a headed browser. (xvfb-run
# is not used: it waits for a signal that a process started in the background may be ignoring,
# and then never returns.)
if [ "${SBRS_XVFB:-}" = "1" ]; then
    Xvfb :99 -screen 0 1280x1024x24 -nolisten tcp > /dev/null 2>&1 &
    DISPLAY=:99
    export DISPLAY
    waited=0
    while [ ! -S /tmp/.X11-unix/X99 ] && [ "$waited" -lt 100 ]; do
        sleep 0.1
        waited=$((waited + 1))
    done
fi

mkdir -p "$work"
tar -C "$src" \
    --exclude=./.git \
    --exclude=./target \
    --exclude=./book \
    --exclude=./.claude \
    --exclude=./.tokensave \
    --exclude=./.serena \
    -cf - . | tar -C "$work" -xf -
cd "$work"
exec "$@"
