#!/bin/sh
# Builds the repository's Dockerfile for Linux, then runs the test suite and the real-Chrome tests
# in a container. See README.md. Usage: run.sh [build|suite|chrome|all]
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
commit=${LINUX_CHECK_COMMIT:-$(git -C "$repo" rev-parse HEAD)}
logs=${LINUX_CHECK_LOGS:-$HOME/.cache/sbrs-linux-check}
mkdir -p "$logs"
# The docker command (a local wrapper that is an alias is not on the PATH for this script), and
# extra flags for docker build, such as that wrapper's --no-slim.
docker=${DOCKER:-docker}
build_flags=${LINUX_CHECK_BUILD_FLAGS:-}

in_container() {
  "$docker" run --rm --shm-size=1g \
    -v sbrs-linux-check-work:/work \
    -v sbrs-linux-check-cargo:/usr/local/cargo/registry \
    -v "$here":/scripts:ro \
    -v "$logs":/logs \
    -w /work \
    -e CARGO_BUILD_JOBS=2 -e CARGO_INCREMENTAL=0 -e CARGO_PROFILE_DEV_DEBUG=0 \
    -e RUST_BACKTRACE=0 -e CI=1 -e SB_HEADLESS=true \
    sbrs-linux-check:test "$@"
}

# shellcheck disable=SC2086 # build_flags holds several words on purpose
build() {
  (cd "$repo" &&
    "$docker" build $build_flags --target builder --build-arg CARGO_BUILD_JOBS=2 -t sbrs-linux-check:builder . &&
    "$docker" build $build_flags --build-arg CARGO_BUILD_JOBS=2 -t sbrs-linux-check:final .) || return 1
  "$docker" run --rm sbrs-linux-check:final --version || return 1
  "$docker" build $build_flags -t sbrs-linux-check:test -f "$here/Dockerfile.test" "$here"
}

setup() {
  in_container sh /scripts/setup.sh "$commit"
}

suite() {
  setup || return 1
  in_container sh -c 'cargo test --no-fail-fast --features "mcp-server test-util testing bdd" > /logs/suite.log 2>&1'
  echo "suite exit=$? (log: $logs/suite.log)"
}

chrome() {
  setup || return 1
  in_container sh /scripts/chrome.sh
  echo "summary: $logs/chrome_summary.txt"
}

case "${1:-all}" in
  build) build ;;
  suite) suite ;;
  chrome) chrome ;;
  all) build && suite && chrome ;;
  *)
    echo "usage: run.sh [build|suite|chrome|all]" >&2
    exit 2
    ;;
esac
