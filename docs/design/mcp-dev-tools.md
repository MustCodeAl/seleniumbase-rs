---
title: "Design: the dev MCP server, tools for developing, debugging and using the library"
---

# Design: the `dev` MCP server

Status: accepted (2026-10-18).

## Why

The three MCP servers (`cdp`, `driver`, `sb`) drive a browser. A model that
writes tests with this library, runs `sbrs`, or finds out why a session fails
needs other things too: which method does what, how a Python call maps to Rust,
what a flag means, what an error means and how to fix it, whether the machine
and the settings are fine. None of that needs a browser.

Every tool costs context in every session that loads the server (`sb` has 104
tools, and some clients stop near 40 MCP tools in all). So these tools are not
added to the browser servers. They are a fourth server, `dev`, that starts no
browser:

```bash
sbrs-mcp --server dev
```

A user installs `dev` plus one browser server. The browser servers stay as they
are, except that the `driver` server gets the session debugging tools the other
two already have (see the end).

## The dev server

Twelve tools. The tools that need nothing from the machine except the
embedded data are read-only and open-world false.

| Tool | Input | Result | Effect |
|---|---|---|---|
| `find_api` | `query`, `engine` (`basecase`, `cdp`, `any`), `limit` | entries: name, owner type, signature, first paragraph of the doc comment, `is_async`, `deprecated` | read-only |
| `parity_lookup` | `python_name` | the Python SeleniumBase 4.55.2 name, status, Rust path, notes; close matches when there is no exact one | read-only |
| `python_to_rust` | `code`, `source` (`auto`, `seleniumbase`, `selenium`), `test_name` | the Rust test and the importer's diagnostics | read-only |
| `scaffold` | `kind` (`test`, `suite`), `name`, `url`, `basic` | the files `sbrs mkfile` / `sbrs mkdir` create, as path and content; nothing is written | read-only |
| `cli_help` | `command` | with no command, the commands and global flags; with one, its usage, flags, defaults and aliases | read-only |
| `list_plugins` | none | the built-in plugins of this build | read-only |
| `plugin_info` | `name` | what one plugin declares and may do | read-only |
| `install_driver` | `browser` (`chrome`, `edge`, `firefox`), `version` | the path and version installed | downloads, open-world |
| `explain_error` | `message`, `category` | category, the library's hint, causes and fix steps, the exit status `sbrs` uses | read-only |
| `check_config` | `path` | every problem in the settings file and the `SB_*` variables, and what the settings would start | read-only |
| `check_fingerprint` | `json` | whether a fingerprint or a profile payload parses, and its errors and warnings | read-only |
| `diagnose_environment` | none | Chrome, driver, versions, output folder, features of this build | read-only |

`diagnose_environment` is the tool the `cdp` and `sb` servers already have; the
`dev` server registers the same code.

### Where each answer comes from

- **`find_api`** searches an index generated from the source. A test parses
  `src/api/base_case.rs`, `src/api/base_case_impls/*.rs` and `src/sb_cdp/*.rs`
  with `syn` and writes `src/mcp/dev/api_index.json`; the server embeds the file
  with `include_str!`. A second test fails when the committed file differs from
  a fresh one, and `SB_UPDATE_API_INDEX=1 cargo test` rewrites it (the same
  scheme as `SB_UPDATE_COMPLETIONS` for the usage spec). Ranking: exact name,
  then name contains the query, then every word of the query in the name, then
  every word in name, owner or doc. The signature is the text of the source,
  not a re-print, so it reads as written.
- **`parity_lookup`** reads `parity/api.toml`, embedded.
- **`python_to_rust`** calls `utilities::python_importer::import_python`. The
  Python is parsed, not run. The input is capped at 256 KiB.
- **`scaffold`** calls `Suite::files()` and `TestFile::render()`, the same
  templates the commands write, through the same name validation
  (`RelativeName`). `sbrs mkchart` and `sbrs mkpres` write HTML straight to a
  file and have no renderer, so they are not offered.
- **`cli_help`** reads `docs/help/cli_reference.md`, embedded and cut by
  heading. That page is generated from `completions/sbrs.usage.kdl` by a test of
  the CLI crate, so it cannot drift from the parser. The alternative, parsing the
  KDL with `usage-lib`, would add that crate and its dependencies to the library
  for the `mcp-server` feature (the CLI crate only has it as a dev-dependency);
  the rendered page already holds every flag, default, alias and choice.
- **`list_plugins`, `plugin_info`** call `cli::plugin_cmd::list` and `info`.
  Without the feature `plugins-builtin` they say the build has no plugin and
  how to get them (`--features plugins-builtin`).
- **`install_driver`** calls `cli::scripts::sb_install::install_driver`, which
  `sbrs install` calls, so it fills the same `downloaded_drivers/` folder of the
  server's working directory. The downloader is a parameter of the server, so
  the tests install through a fake one and never reach the network.
- **`explain_error`** rebuilds a `SeleniumBaseError` of the category (and the
  payload the message carries) and returns its `category()`, `hint()` and
  `exit_code()`, then the matching sections of `docs/help/common_problems.md`,
  embedded and cut by heading.
- **`check_config`** loads the settings as `sbrs` does (the file, then the
  `SB_*` variables, then validation) and reports every problem instead of the
  first, then the runtime variables (`RuntimeConfig::from_env_lenient`) and the
  plugin selection (`check_selection`). It starts nothing.
- **`check_fingerprint`** parses the JSON as a `Fingerprint`, or as a profile
  payload (`ProfileParams::to_fingerprint`), and returns what
  `Fingerprint::validate` finds. `validate_fingerprint` of the `sb` server uses
  the same function for its report.

### Safety

- Every tool but `install_driver` is read-only, and says so in its annotations.
  `install_driver` is not read-only, replaces a driver of another version in the
  cache, and reaches the network, so its annotations are
  `readOnly=false, destructive=true, idempotent=true, openWorld=true`
  (the new `Effect::Fetch`).
- `scaffold` writes nothing; the model writes the files with its own tools, which
  the client already guards.
- `check_config` reads one file. The path must be relative, inside the folder
  the server runs in, with no `..`. The values it prints are redacted the way
  `diagnose_environment` redacts them (credentials in a proxy URL, anything
  named like a secret), and a parse error is cut to its first line, since the
  TOML parser's message quotes the offending line.
- `python_to_rust` and `check_fingerprint` read text the model sends; both bound
  its size and neither runs it.
- `install_driver` takes the browser and version from a closed set and a parsed
  `DriverVersion`; no URL or path comes from the model.
- A result says what it did. An answer from a build without a feature says so; a
  search that finds nothing says it searched the index of this version.

## Debugging, by symptom

| Symptom | Tool |
|---|---|
| "element not found", a wait times out | `diagnose_selector`, `suggest_selectors`, `inspect_page` (browser server) |
| The page is blank or wrong | `inspect_page`, `get_console_messages`, `get_failed_requests`, `capture_debug_bundle` |
| The driver and Chrome disagree ("session not created") | `diagnose_environment`, then `install_driver` |
| A site blocks the session | `check_fingerprint`, `validate_fingerprint`, `audit_page` |
| An error text you do not understand | `explain_error` |
| A setting is ignored or refused | `check_config` |
| A flag or command | `cli_help` |

## The driver server

The `driver` server mirrors upstream's 26 `Driver()` tools. It also gets the
tools below, which `sb` already has and which run on `BaseCase` unchanged: they
are the same `ToolDef`s, registered by name from the shared table in
`devtools.rs`, not copies.

`inspect_page`, `suggest_selectors`, `diagnose_selector`, `get_console_messages`,
`capture_debug_bundle`, `get_page_info`, `export_session_as_test` and
`diagnose_environment`.

`get_failed_requests` and `audit_page` stay with `cdp` and `sb`. The WebDriver
engine has no network events to record, so `get_failed_requests` can only read
the performance timeline there; the `driver` server is kept at 34 tools.

## Not in scope

No new CLI flag. `sbrs-mcp` reads the server name through `Profile::from_str`,
so `--server dev` works once `Profile::Dev` exists; only the help text of the
flag names the new value.
