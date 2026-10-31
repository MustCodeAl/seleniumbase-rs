---
title: "Releasing a version"
sidebarTitle: "Releasing"
description: "The steps that turn main into a tagged release: versions, changelog, gate, tag, the macOS binary, the GitHub release and the Homebrew formula."
---

A release is cut from `main` by a maintainer, by hand. GitHub Actions cannot run for this
repository at the moment (billing), so the release workflow does not build binaries: the
only prebuilt binary is the macOS arm64 one built locally, and every other platform installs
from source (`cargo install --git ... --tag vX.Y.Z seleniumbase-rs-cli`) or through Homebrew.

<Warning>
Every push, tag, GitHub release and Homebrew commit is public. Run the whole gate on the
exact commit you tag, and never force-push.
</Warning>

<Steps>
<Step title="Start from a clean, current main">

```bash Terminal
git fetch origin
git switch main && git merge --ff-only origin/main
git status --short    # nothing but files you mean to keep out of the release
```

</Step>
<Step title="Set the version">

```bash Terminal
cargo set-version X.Y.Z -p seleniumbase-rs -p seleniumbase-rs-cli
```

`cargo set-version` (cargo-edit) also moves the CLI crate's path dependency on the library;
`Cargo.lock` follows. Update the version pins in `README.md` and look for the old version in
other docs (`rg -n 'OLD\.VERSION' -g '*.md' -g '*.toml' -g '*.rb'`, CHANGELOG files aside).

</Step>
<Step title="Close the changelog">

Rename `## [Unreleased]` in `CHANGELOG.md` to `## [X.Y.Z] - YYYY-MM-DD`, open a new empty
`## [Unreleased]` above it, then write the docs page from it:

```bash Terminal
python3 -I tools/docs7_changelog.py
```

</Step>
<Step title="Regenerate what is generated">

```bash Terminal
SB_UPDATE_COMPLETIONS=1 cargo test -p seleniumbase-rs-cli --bin sbrs usage_spec
SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index
python3 -I tools/parity/render_docs.py
```

Commit everything as `chore(release): X.Y.Z`.

</Step>
<Step title="Gate the release commit">

```bash Terminal
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all -- --check
cargo clippy --all-targets --features "mcp-server test-util" -- -D warnings
cargo nextest run --features "mcp-server test-util"
cargo test --doc --features "mcp-server test-util"
cargo audit && cargo deny check
trufflehog filesystem . --only-verified && ripsecrets
just mdx && mdbook build
```

</Step>
<Step title="Publish main and the tag">

```bash Terminal
C=$(git rev-parse HEAD)
git fetch origin && git merge-base --is-ancestor origin/main "$C"
git push origin "${C}:refs/heads/main"
git tag -a vX.Y.Z -m "seleniumbase-rs X.Y.Z" "$C"
git push origin vX.Y.Z
```

In zsh write `"${C}:refs/heads/main"`: `$C:r` would be read as a modifier.

</Step>
<Step title="Build the macOS binary">

```bash Terminal
cargo build --release --locked -p seleniumbase-rs-cli
```

Archive `target/release/sbrs` and `target/release/sbrs-mcp` with the completion scripts as
`seleniumbase-rs-vX.Y.Z-aarch64-apple-darwin.tgz`, and write its SHA-256 next to it.

</Step>
<Step title="Create the GitHub release">

```bash Terminal
gh release create vX.Y.Z --verify-tag --title "vX.Y.Z" --notes-file NOTES.md \
  seleniumbase-rs-vX.Y.Z-aarch64-apple-darwin.tgz seleniumbase-rs-vX.Y.Z-aarch64-apple-darwin.tgz.sha256
```

The notes are the version's CHANGELOG block, plus a line saying the prebuilt binary is for
macOS arm64 only and how to install elsewhere.

</Step>
<Step title="Update the Homebrew formula">

In the [homebrew-tap](https://github.com/MustCodeAl/homebrew-tap) repository, set `url` to the
tag's source archive and `sha256` to its checksum in `Formula/sbrs.rb`:

```bash Terminal
curl -sL https://github.com/MustCodeAl/seleniumbase-rs/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256
brew audit --strict --online Formula/sbrs.rb
brew install --build-from-source Formula/sbrs.rb && brew test sbrs
```

Commit `sbrs X.Y.Z` and push. `brew upgrade sbrs` then gives users the release.

</Step>
<Step title="Move the consumers">

The companion extension's end-to-end crate pins the library by tag: move it to `vX.Y.Z`.
The stealth workspace follows with `cargo update -p seleniumbase-rs` and its own gates.
Before the tag, run `just contracts --check-build` (see Consumer contracts): it compiles each
consumer against this checkout, so the pins can be moved with nothing left to break.

</Step>
</Steps>
