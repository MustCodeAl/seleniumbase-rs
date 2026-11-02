#!/bin/sh
# Builds the release archives of sbrs and sbrs-mcp on an Apple silicon Mac for Linux arm64, Linux
# x86_64 (both in containers) and macOS Intel, packs each the way .github/workflows/release.yml names
# it, writes SHA256SUMS and checks that each binary runs on its platform. It uploads nothing.
# See README.md. Usage: build-binaries.sh [all|linux-arm64|linux-x86_64|macos-x86_64|pack|check]
set -eu
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
docker=${DOCKER:-docker}
out=${RELEASE_OUT:-$HOME/.cache/sbrs-release}
min_free_gb=${RELEASE_MIN_FREE_GB:-10}
jobs=${CARGO_BUILD_JOBS:-2}
# cross: build the x86_64 Linux binaries in an arm64 container with a cross linker (minutes);
# emulate: build them in an x86_64 container under OrbStack's emulation (hours).
x86_mode=${RELEASE_X86_MODE:-cross}
rust_image=rust:1.99-bookworm
run_image=debian:bookworm-slim
features=mcp-server,pdf,plugins-builtin
linux_arm=aarch64-unknown-linux-gnu
linux_x86=x86_64-unknown-linux-gnu
mac_x86=x86_64-apple-darwin

say() { printf '%s\n' "$*"; }
die() {
  printf 'build-binaries: %s\n' "$*" >&2
  exit 1
}

for tool in git jq tar shasum "$docker"; do
  command -v "$tool" > /dev/null 2>&1 || die "needs $tool on the PATH"
done
case "$x86_mode" in cross | emulate) ;; *) die "RELEASE_X86_MODE must be cross or emulate" ;; esac

commit=$(git -C "$repo" rev-parse --verify "${RELEASE_COMMIT:-HEAD}^{commit}")
short=$(printf '%s' "$commit" | cut -c1-12)
if [ -z "${RELEASE_COMMIT:-}" ] && [ -n "$(git -C "$repo" status --porcelain)" ]; then
  say "note: the checkout has uncommitted changes; this builds the commit $short, not them"
fi

# The commit's tree, for the builds, the version and the files that go into each archive.
src="$out/src-$short"
rm -rf "$src"
mkdir -p "$src"
git -C "$repo" archive "$commit" | tar -x -C "$src"
version=$(cargo metadata --no-deps --format-version 1 --manifest-path "$src/Cargo.toml" |
  jq -r '.packages[] | select(.name == "seleniumbase-rs-cli") | .version')
[ -n "$version" ] || die "could not read the version of seleniumbase-rs-cli"
tag="v$version"
dist="$out/$tag"
mkdir -p "$dist/bin" "$dist/completions"
say "commit $short, version $tag, output $dist"

# One release build at a time, and never on a nearly full disk.
guard() {
  free_gb=$(df -Pk "$out" | awk 'NR == 2 { print int($4 / 1048576) }')
  if [ "$free_gb" -lt "$min_free_gb" ]; then
    say "build-binaries: ${free_gb} GB free, under ${min_free_gb} GB: not starting a build" >&2
    exit 75
  fi
  if pgrep -x cargo > /dev/null 2>&1 || pgrep -x rustc > /dev/null 2>&1; then
    say "build-binaries: another cargo build is running; start this one after it" >&2
    exit 75
  fi
}

# Builds in a fresh container from the commit's tree, piped in on standard input. The volumes keep
# the registry, the toolchain and the compiled work between runs.
container_build() {
  platform=$1 target=$2 prepare=$3
  guard
  mkdir -p "$dist/bin/$target"
  say "building $target in a $platform container"
  git -C "$repo" archive "$commit" | "$docker" run --rm -i --platform "$platform" \
    -v sbrs-release-registry:/usr/local/cargo/registry \
    -v "sbrs-release-rustup-${platform#linux/}":/usr/local/rustup \
    -v "sbrs-release-target-$target":/src/target \
    -v "$dist/bin/$target":/out \
    -e CARGO_BUILD_JOBS="$jobs" -e CARGO_INCREMENTAL=0 -e RUSTUP_AUTO_INSTALL=1 \
    -w /src "$rust_image" sh -c "set -e
      tar -x -C /src
      $prepare
      cargo build --release --locked --target $target --package seleniumbase-rs-cli \
        --bin sbrs --bin sbrs-mcp --features $features
      cp target/$target/release/sbrs target/$target/release/sbrs-mcp /out/"
}

linux_arm64() {
  container_build linux/arm64 "$linux_arm" ":"
}

linux_x86_64() {
  if [ "$x86_mode" = emulate ]; then
    container_build linux/amd64 "$linux_x86" ":"
  else
    container_build linux/arm64 "$linux_x86" "
      apt-get update -qq
      apt-get install -y -qq --no-install-recommends gcc-x86-64-linux-gnu libc6-dev-amd64-cross > /dev/null
      rustup target add $linux_x86
      export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc
      export CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc AR_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ar"
  fi
}

# On the Mac itself, from the commit's tree; the toolchain is the one rust-toolchain.toml names.
macos_x86_64() {
  guard
  mkdir -p "$dist/bin/$mac_x86"
  say "building $mac_x86 on this Mac"
  (cd "$src" &&
    rustup target add "$mac_x86" &&
    CARGO_BUILD_JOBS="$jobs" CARGO_INCREMENTAL=0 cargo build --release --locked --target "$mac_x86" \
      --package seleniumbase-rs-cli --bin sbrs --bin sbrs-mcp --features "$features" &&
    cp "target/$mac_x86/release/sbrs" "target/$mac_x86/release/sbrs-mcp" "$dist/bin/$mac_x86/")
}

# The completions the release workflow generates, from the Linux arm64 binary.
completions() {
  bin="$dist/bin/$linux_arm"
  [ -x "$bin/sbrs" ] || die "the completions come from the Linux arm64 binary: build linux-arm64 first"
  for pair in bash:sbrs.bash zsh:_sbrs fish:sbrs.fish powershell:_sbrs.ps1 fig:sbrs.ts usage:sbrs.usage.kdl; do
    shell=${pair%%:*} file=${pair#*:}
    "$docker" run --rm --platform linux/arm64 -v "$bin":/release:ro "$run_image" \
      /release/sbrs completions "$shell" > "$dist/completions/$file"
  done
}

# Packs every target that has both binaries; the binaries sit at the archive's root.
pack() {
  [ -s "$dist/completions/sbrs.usage.kdl" ] || completions
  rm -f "$dist"/*.tgz "$dist/SHA256SUMS"
  for target in "$linux_arm" "$linux_x86" "$mac_x86"; do
    [ -x "$dist/bin/$target/sbrs" ] && [ -x "$dist/bin/$target/sbrs-mcp" ] || continue
    name="seleniumbase-rs-$tag-$target"
    stage="$out/stage/$name"
    rm -rf "$stage"
    mkdir -p "$stage/completions" "$stage/warp/workflows"
    cp "$dist/bin/$target/sbrs" "$dist/bin/$target/sbrs-mcp" "$stage/"
    cp "$dist/completions/"* "$stage/completions/"
    cp "$src"/.warp/workflows/*.yaml "$stage/warp/workflows/"
    cp "$src/README.md" "$src/LICENSE" "$stage/"
    # COPYFILE_DISABLE keeps macOS tar from adding ._ files for extended attributes.
    COPYFILE_DISABLE=1 tar -C "$stage" -czf "$dist/$name.tgz" .
    say "packed $name.tgz"
  done
  (cd "$dist" && set -- ./*.tgz && [ -e "$1" ] || die "nothing to pack: build a target first")
  (cd "$dist" && shasum -a 256 -- *.tgz > SHA256SUMS && cat SHA256SUMS)
}

# Unpacks each archive and runs its binaries on their platform: the Linux ones in a Debian
# container (arm64 natively, x86_64 under emulation), the macOS one under Rosetta.
check() {
  [ -s "$dist/SHA256SUMS" ] || die "nothing to check: pack first"
  (cd "$dist" && shasum -a 256 -c SHA256SUMS)
  failed=0 skipped=0
  report="$out/$tag-checks.txt"
  : > "$report"
  for archive in "$dist"/*.tgz; do
    name=$(basename "$archive" .tgz)
    target=${name#"seleniumbase-rs-$tag-"}
    unpacked="$out/check/$name"
    rm -rf "$unpacked"
    mkdir -p "$unpacked"
    tar -xzf "$archive" -C "$unpacked"
    kind=$(file -b "$unpacked/sbrs")
    case "$target" in
      "$linux_arm") platform=linux/arm64 want="ARM aarch64" ;;
      "$linux_x86") platform=linux/amd64 want="x86-64" ;;
      "$mac_x86") platform=macos want="Mach-O 64-bit executable x86_64" ;;
      *) platform=unknown want="?" ;;
    esac
    case "$kind" in
      *"$want"*) ;;
      *)
        say "$target: FAILED: sbrs is '$kind', not $want" | tee -a "$report"
        failed=$((failed + 1))
        continue
        ;;
    esac
    if [ "$platform" = macos ]; then
      if ! arch -x86_64 /usr/bin/true > /dev/null 2>&1; then
        say "$target: NOT RUN: Rosetta 2 is not installed (softwareupdate --install-rosetta); the file is $kind" | tee -a "$report"
        skipped=$((skipped + 1))
        continue
      fi
      if line=$(arch -x86_64 "$unpacked/sbrs" --version) &&
        arch -x86_64 "$unpacked/sbrs-mcp" --help > /dev/null; then
        say "$target: ok: $line" | tee -a "$report"
      else
        say "$target: FAILED to run" | tee -a "$report"
        failed=$((failed + 1))
      fi
    elif line=$("$docker" run --rm --platform "$platform" -v "$unpacked":/release:ro "$run_image" \
      sh -c '/release/sbrs --version && /release/sbrs-mcp --help > /dev/null'); then
      say "$target: ok: $line" | tee -a "$report"
    else
      say "$target: FAILED to run in a $platform container" | tee -a "$report"
      failed=$((failed + 1))
    fi
  done
  say "checks: $failed failed, $skipped not run (report: $report)"
  [ "$failed" -eq 0 ]
}

case "${1:-all}" in
  linux-arm64) linux_arm64 ;;
  linux-x86_64) linux_x86_64 ;;
  macos-x86_64) macos_x86_64 ;;
  pack) pack ;;
  check) check ;;
  all) linux_arm64 && linux_x86_64 && macos_x86_64 && pack && check ;;
  *)
    say "usage: build-binaries.sh [all|linux-arm64|linux-x86_64|macos-x86_64|pack|check]" >&2
    exit 2
    ;;
esac
