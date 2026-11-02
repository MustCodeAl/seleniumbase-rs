# Release binaries, built locally

`build-binaries.sh` builds `sbrs` and `sbrs-mcp` on an Apple silicon Mac for three of the targets
`.github/workflows/release.yml` builds, packs them with the same names and contents, writes
`SHA256SUMS`, and checks that each binary runs. It uploads nothing: attach the archives to the
release by hand.

```bash
mise run release-binaries               # all three targets, then pack and check
mise run release-binaries linux-arm64   # one step: linux-arm64, linux-x86_64, macos-x86_64, pack or check
```

| Target | Built | Checked |
|---|---|---|
| `aarch64-unknown-linux-gnu` | in a `rust:1.99-bookworm` container | runs in `debian:bookworm-slim` |
| `x86_64-unknown-linux-gnu` | in the same arm64 container with Debian's x86_64 cross linker, or under emulation (`RELEASE_X86_MODE=emulate`, hours) | runs in an x86_64 `debian:bookworm-slim` under OrbStack's emulation |
| `x86_64-apple-darwin` | on the Mac (`rustup target add x86_64-apple-darwin` is run for you) | runs under Rosetta 2, when it is installed |

The macOS arm64 and Windows archives are not built here.

## What it does

1. Exports the commit (`HEAD`, or `RELEASE_COMMIT`) with `git archive`, so uncommitted changes are
   never built, and reads the version from it. The archives are
   `seleniumbase-rs-v<version>-<target>.tgz`, which is what `cargo binstall` and mise's `github:`
   backend look for.
2. Builds each target with `cargo build --release --locked --features mcp-server,pdf,plugins-builtin`,
   one at a time. Before each build it refuses (exit 75) when another cargo build is running or the
   disk has less than `RELEASE_MIN_FREE_GB` (10) free.
3. Generates the completions with the Linux arm64 binary, as the release workflow does with its own.
4. Packs each target: `sbrs`, `sbrs-mcp`, `completions/`, `warp/workflows/`, `README.md` and
   `LICENSE`, with the binaries at the archive's root.
5. Writes `SHA256SUMS` in the same format as `sha256sum`.
6. Checks each archive: the sums, that `sbrs` is built for the right CPU (`file`), and that
   `sbrs --version` and `sbrs-mcp --help` run on the target's platform. The result is in
   `<output>/v<version>-checks.txt`.

The output is `~/.cache/sbrs-release/v<version>/` (or `RELEASE_OUT`): the archives and
`SHA256SUMS` beside the `bin/` and `completions/` folders they were made from. Upload only the
`.tgz` files and `SHA256SUMS`.

## Settings

| Variable | Default | |
|---|---|---|
| `RELEASE_COMMIT` | `HEAD` | the commit to build |
| `RELEASE_OUT` | `~/.cache/sbrs-release` | where everything goes |
| `RELEASE_X86_MODE` | `cross` | `emulate` builds the x86_64 Linux binaries under emulation instead |
| `RELEASE_MIN_FREE_GB` | `10` | refuse a build below this much free disk |
| `CARGO_BUILD_JOBS` | `2` | build jobs, in the containers and on the Mac |
| `DOCKER` | `docker` | the docker command; a wrapper that is a shell alias must be named here |

## Things to know

- **Disk.** The volumes `sbrs-release-registry`, `sbrs-release-rustup-arm64` (and `-amd64` with
  `emulate`) and `sbrs-release-target-<target>` keep the downloads and the compiled work between
  runs. A release build of one target takes a few GB. Remove them with `docker volume rm` when done.
- **Cross or emulate.** `cross` links the x86_64 binaries with Debian's `x86_64-linux-gnu-gcc` in
  the arm64 container, at native speed. `emulate` runs the whole build in an x86_64 container, as the
  GitHub runner would, but OrbStack emulates every compiler process.
- **glibc.** The Linux binaries are linked on Debian 12 (glibc 2.36), older than the release
  workflow's Ubuntu 24.04, so they run on at least as many systems.
- **Rosetta.** Without Rosetta 2 the macOS Intel binary is built and its CPU type checked, but it is
  not run; the check says so. `softwareupdate --install-rosetta` installs it.
