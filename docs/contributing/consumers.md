---
title: "Consumer contracts"
sidebarTitle: "Consumer contracts"
description: "The tests and the script that keep the repositories built on this crate (stealth, the companion extension, the plugins, the Tauri plugin, the Homebrew tap) in step with it: what a contract file holds, how to run the check, and what to do when one fails."
---

This crate has no users who can be told about a change beforehand, but it has five
repositories that build on it. Each one imports items from here, or runs the `sbrs` binary
built from here, and a change that is fine for this repository can stop one of them from
compiling the day it updates. A contract is a test in this repository that names such a
consumer and fails first.

| Consumer | Repository | Contract file | What it holds |
|---|---|---|---|
| Stealth layer | `seleniumbase-rs-stealth` | `tests/contract_stealth.rs` | its `seleniumbase_rs::` imports and the signatures its crates call |
| Companion extension | `seleniumbase-rs-companion` | `tests/contract_companion.rs` | the Rust its generators write, its golden programs, its end-to-end crate |
| Plugins | `seleniumbase-rs-plugins` | `tests/contract_plugins.rs` | what this crate uses of the plugin API, and the pin in `Cargo.toml` |
| Tauri plugin | `tauri-plugin-sbrs-automation` | `tests/contract_tauri.rs` | its `stealth` feature: the in-process DevTools channel |
| Homebrew tap | `homebrew-tap` | `crates/cli/tests/contract_homebrew.rs` | what the formula's `install` and `test do` rely on |

The plugins run the other way: this crate depends on that repository by git and a commit, and
that repository imports nothing from here. Its contract is what this crate uses of it.

## What a contract file holds

* **The imports, as the consumer writes them.** Every `use` line of the consumer that names an
  item of this crate is copied as it is, one block each, so the same name can be imported twice.
  Paths the consumer writes in code or in a rustdoc link are there too. If a change removes or
  renames one, the file stops compiling and the error names the item.
* **The calls, as the consumer makes them.** Beside the imports, a function mirrors a call site
  (the consumer's file is named above it), with the same argument and return types, and is compiled and
  never run. A changed signature breaks it. Where it costs nothing, a test runs the behavior the
  consumer's own tests rely on (a command answered through the inbox of a channel, the
  programs `ActionRecorder` writes, a set of plugins given to both launch paths, the real `sbrs`).
* **Nothing that starts a browser.** The contract files need no Chrome. The ones that run
  something run the mock transport, the library, or the `sbrs` binary.

Features follow the consumer. The stealth layer takes this crate with its default features,
which are none, so what it uses of `test-util` (`Browser::new_mocked`, `MockCtrl`) is behind that
feature in the file. The plugins contract is behind `plugins`, as the plugin API is.

## Run the check

`tests/contract_*.rs` run with the rest of the tests:

```bash Terminal
cargo test --features "mcp-server test-util testing" \
  --test contract_stealth --test contract_companion --test contract_plugins \
  --test contract_tauri --test contract_script
cargo test -p seleniumbase-rs-cli --test contract_homebrew
```

`just contracts` is the other half. It reads the sibling checkouts and fails, naming the path,
when a consumer uses a `seleniumbase_rs::` path that its contract file does not list, so a
contract cannot lag behind the repository it describes. It finds the checkouts beside this one
(`../seleniumbase-rs-stealth` and so on) or where these variables say:

| Variable | Checkout |
|---|---|
| `SBRS_STEALTH_DIR` | `seleniumbase-rs-stealth` |
| `SBRS_COMPANION_DIR` | `seleniumbase-rs-companion` |
| `SBRS_PLUGINS_DIR` | `seleniumbase-rs-plugins` |
| `SBRS_TAURI_DIR` | `tauri-plugin-sbrs-automation` |
| `SBRS_TAP_DIR` | `homebrew-tap` |

```bash Terminal
just contracts                  # every consumer
just contracts stealth tauri    # some of them
scripts/check-consumers.sh --list stealth   # the paths a consumer uses, one per line
```

Besides the paths it checks four things:

* the companion's golden programs (`tests/golden/` there) are byte for byte the copies in
  `tests/contract_companion/`;
* both plugin lines of `Cargo.toml` carry the same git URL and the same `rev`, and the `rev` is
  an ancestor of `main` in the plugins checkout (`git merge-base --is-ancestor`): two revisions
  would build two `seleniumbase-rs-plugin` crates, and the `Plugin` of a built-in would not be
  the one this crate calls;
* the paths this crate itself uses of the plugin crates are in `tests/contract_plugins.rs`;
* every string the Homebrew formula relies on (the arguments of its `shell_output`, what it
  `assert_match`es, its shells, the Warp workflows folder) is in `contract_homebrew.rs`.

A path counts when it is in code in the contract file; one in a comment does not.

### Compile the consumers

```bash Terminal
just contracts --check-build
```

For each consumer this runs `cargo check --tests` with this checkout in place of its git
dependency on this crate (a `--config` that patches `https://github.com/MustCodeAl/seleniumbase-rs`
to a path). It is for a release, not for every change: the stealth workspace builds BoringSSL
with cmake, the Tauri plugin builds Tauri, and the machine builds one at a time. Run it before
the tag, then move the consumers' pins.

## When one fails

<Steps>
<Step title="A contract file does not compile">

The error names an item and the consumer's file is named above the code that uses it. The change
you made breaks that consumer. Fix the contract file in the same commit, so that it pins the new
shape, and put what the consumer has to change in the commit message. There are no shims: this
crate has no users yet, and the consumer follows.

</Step>
<Step title="The script names a path">

The consumer uses a path its contract file does not list. Add it where the consumer writes it
(an import block, or a call mirrored from the call site) and run `just contracts` again. Find
the use with the path from `scripts/check-consumers.sh --list NAME`.

</Step>
<Step title="The goldens differ">

`just contracts-sync` copies the companion's golden programs again. Do it only after the
companion changed them on purpose: `action_recorder_still_writes_the_goldens` holds this crate's
`ActionRecorder` to the same files, so a copy that makes it fail means one of the two moved.

</Step>
<Step title="The plugin pin is wrong">

Push the plugins commit first, then set `rev` on both lines of `Cargo.toml` to it.
`tests/plugin_features.rs` fails when the two lines differ.

</Step>
</Steps>

A public item that a consumer imports needs its contract file updated in the same commit. A
change to an item no consumer imports needs nothing.

## Adding a consumer

Write `tests/contract_NAME.rs` with the imports and the calls, add the name and the places to
look to `scripts/check-consumers.sh` (`consumer_dir`, `contract_file`, `consumer_paths`), a fixture
test in `tests/contract_script.rs` if it reads something new, and a row to the table above.
