---
title: "Design: the sbrs CLI on usage, in its own workspace crate"
---

Status: accepted (2026-10-17): derives, package `seleniumbase-rs-cli`, work
starts after the fingerprint API v2 lands. Replaces the earlier plan of a
hand-written KDL spec parsed at runtime.

## Goal

1. `sbrs` parses its command line with [usage](https://usage.jdx.dev) instead of
   clap, and the usage spec (`completions/sbrs.usage.kdl`) is generated from the
   Rust declaration, so the parser, help, shell completions, the Fig spec and the
   CLI reference page cannot drift apart.
2. The CLI and its dependencies leave the library package. Someone who adds
   `seleniumbase-rs` as a dependency no longer compiles clap, completion
   generators or terminal UI code.
3. The command line does not change: every command, flag, alias, default and
   exit status stays as it is in 0.4.0.

## Why derives, not a hand-written spec

usage 6 (`usage = { package = "usage-rs", version = "6" }`) has typed derives that
mirror clap's: `#[derive(usage::Cli)]`, `#[derive(usage::Subcommands)]`,
`#[derive(usage::Args)]`, `#[derive(usage::ValueEnum)]`, with `Cli::parse()`,
`Cli::parse_from(argv)` and `Cli::to_kdl()`. Its guide "Migrating from clap" maps
the attributes one to one (`#[arg(...)]` becomes `#[usage(...)]`, `default_value`
becomes `default`, `value_delimiter` becomes `delimiter`, `conflicts_with` becomes
`conflicts("--flag")`).

A hand-written KDL file parsed at runtime would give an untyped map of strings,
move every type check from the compiler to runtime and need a second
description of each command for `run()`. The derive keeps the types and makes
the KDL a generated artefact, checked by a test.

## What `sbrs.rs` uses today (counted 2026-10-17)

- 182 `#[arg(...)]`: `long`, `short`, `help`, `value_name`, `default_value(_t)`,
  `value_enum`, `value_delimiter`, `last = true` (2), `conflicts_with(_all)` (7),
  `requires` (1), `value_parser = clap::value_parser!(Language)` (1).
- `#[command(...)]`: 8 `visible_alias`, `subcommand`, and the root with `name`,
  `version`, `about`, `long_about`, `before_long_help = LOGO`.
- Derives: 1 `Parser`, 1 `Subcommand`, 2 `clap::Args`, 6 `ValueEnum`.
- `clap_complete` (the `completions` command) and `clap_complete_fig`, plus
  `clap_usage` (dev) to write the KDL.

## Decisions for the gaps the usage guide lists

| Gap | Decision |
|---|---|
| Permissive defaults (unknown flags parse as values, last-one-wins) | Root gets `unknown_flags = "error"` and `args_override_self = false`, as strict as clap. `unknown_flags` reaches every command; `args_override_self` is per command, so the `Args` structs set it too and the inline variants cannot (see *Done* below) |
| `value_parser!(Language)` | `Language: FromStr` (it already parses) |
| `last = true` (args after `--`) | Use usage's equivalent if it has one; otherwise a `Vec<String>` positional with the double-dash setting. Verified by the existing tests of `sbrs grid` and `sbrs run` |
| `before_long_help = LOGO` | `before_long_help` is supported |
| Prefix inference | Not used today; nothing to do |
| Help layout | Accept usage's layout. Tests that compare help text are updated to the new text once, by review |

## Workspace split (one repository)

```
Cargo.toml                 workspace root and the library package seleniumbase-rs (unchanged path)
crates/cli/Cargo.toml      package seleniumbase-rs-cli: bins sbrs and sbrs-mcp
crates/cli/src/            sbrs.rs (from src/cli/bin), mcp_server.rs, report.rs, logo
crates/cli/tests/          every test that runs CARGO_BIN_EXE_sbrs or sbrs-mcp
```

- The library keeps `src/cli/` modules that have no CLI-only dependencies
  (`commands`, `scaffold`, `scripts`, `devtools`) because the MCP server and the
  golden-template doctests use them. Only the binaries and their dependencies
  move.
- CLI-only dependencies move to `crates/cli`: usage (was clap, clap_complete,
  clap_complete_fig: 15 crates in a default build of the library today), and the
  `tui` and `gui` front ends if their code is only reached from `sbrs`.
- The library stays at the repository root, so `seleniumbase-rs = { git = ... }`
  keeps working unchanged.
- Install: `cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli`
  (features `pdf`, `mcp-server` on by default in the CLI crate). The Homebrew
  formula changes `cargo install` to `--path crates/cli` at the next release.
- Dockerfile skeleton step, CI matrix, release workflow, justfile, Fig workflow
  and the install lines in the docs move with it.

## Order of work

1. Split (mechanical, clap unchanged): move the binaries, their tests and
   their dependencies; nothing else changes. Gate: fmt, clippy for the workspace
   with `--all-features` minus `turso`, nextest, doctests, deny, mdbook, MDX,
   Dockerfile skeleton test. One commit series, one sonnet agent.
   *Done:* the binaries, their 19 tests and clap moved to `crates/cli`; the
   workspace names both packages in `default-members`, the CLI package has
   `pdf` and `mcp-server` on by default, and `sbrs --version` still prints
   `(seleniumbase-rs)`. The library's default build lost the 15 crates of clap and
   its completion generators. `ratatui`, `crossterm`, `enigo` and `rfd` stayed in
   the library on purpose: the commander and the dialogs (`src/cli/scripts`,
   `src/api/dialog`) are library code that `BaseCase` and the MCP server reach, so
   the CLI package only forwards the `tui` and `gui` features.
2. Migrate `crates/cli` from clap to usage derives. `completions/sbrs.usage.kdl`
   is written by `Cli::to_kdl()` in a test that fails when the committed file is
   stale; shell completions and the Fig spec are generated from that KDL. The
   0.4.0 spec is the oracle: the new KDL may differ only in formatting and in the
   "@generated" line, which the agent shows as a diff in its report. One sonnet
   agent, after step 1 is merged.
   *Done:* `sbrs` and `sbrs-mcp` are declared with `usage` 6.12 (the package
   `usage-rs`), and clap, `clap_complete`, `clap_complete_fig` and `clap_usage` are
   gone from the workspace. `completions/sbrs.usage.kdl`, the CLI reference page
   `docs/help/cli_reference.md` (rendered by `usage-lib`, a dev-dependency) and the
   Fig spec `docs/completions/fig-sbrs.ts` are written by tests that fail when a file
   is stale. The shell scripts of `sbrs completions` ask `sbrs` for the candidates
   (usage's own scripts); `completions fig` has its own small generator,
   `crates/cli/src/fig.rs`, over the same tables, because usage 6 has a Fig
   generator only in the `usage` tool and 7.0 removed it. Against the 0.4.0 binary,
   250 of 256 command lines act the same; the other six are three differences the
   spec cannot close: usage 6.12 lets an inline struct variant repeat a flag (the
   rule is per command and only an `Args` struct or the root can set it), drops the
   `=value` of a plain switch (so every switch is `bool_value` and `--headless=false`
   means false), and prints the root help for `help nosuch`. `crates/cli/src/strict.rs`
   closed all three afterwards: it reads the parser's events before the real parse and
   refuses them with status 2, as clap did. The help is laid out by
   usage (alphabetical command list, `Flags:`, a final period on short
   descriptions, `[aliases: x]`); the help tests that read the layout changed.
3. Docs: the CLI reference page generated from the spec, the install lines,
   CHANGELOG (Breaking: the install command and package name).

## Measures of done

- `sbrs --help`, every `sbrs <command> --help` and the KDL list the same
  commands, flags, aliases and defaults as 0.4.0.
- A default build of the library has no clap, no usage and no completion crates
  (`cargo tree -p seleniumbase-rs -e normal` shows none).
- All existing CLI tests pass unchanged except help-text snapshots.

## Not in this change

- Renaming commands or flags, or changing which commands take a URL as a
  positional and which as `--url`. That is an API change of its own; if wanted,
  it gets its own table and review after the migration, when the spec makes the
  whole surface visible in one file.
