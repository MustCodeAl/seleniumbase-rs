# Syncing with upstream SeleniumBase

This crate follows the Python [SeleniumBase](https://github.com/seleniumbase/SeleniumBase)
and [seleniumbase-mcp](https://github.com/seleniumbase/seleniumbase-mcp)
releases. Keeping up is a short, mechanical loop, because the public API of
both is recorded as data and checked by a test.

The files involved:

| File | What it is |
| --- | --- |
| `parity/upstream.toml` | The upstream versions and commits the crate is synced to. |
| `parity/upstream-api.json` | The upstream public API, extracted by a script. Generated. |
| `parity/api.toml` | What each upstream name maps to here. Edited by people. |
| `tests/parity.rs` | Fails until the two above agree and every path exists. |
| `docs/parity.md` | A readable table of `api.toml`. Generated. |
| `tools/parity/` | The extractor, the seeder and the doc renderer (Python standard library only). |

## The loop

1. **Update the checkouts.** Pull the new release of SeleniumBase and
   seleniumbase-mcp into two local clones.
2. **Extract the API.**

   ```bash
   just parity-extract ~/src/SeleniumBase ~/src/seleniumbase-mcp
   ```

   The output is sorted and stable, so `git diff parity/upstream-api.json` is
   exactly what upstream added, removed or changed.
3. **Pin the release.** Set `version` and `commit` in `parity/upstream.toml` to
   match the extraction, and update `UPSTREAM_SELENIUMBASE_VERSION` in
   `src/lib.rs`.
4. **Classify the new names.**

   ```bash
   just parity-seed
   ```

   This adds an entry for every upstream name that has none: `implemented` if a
   Rust item has the same name, `composed` for known compositions, and
   otherwise `planned`. It never changes entries that already exist.
5. **Run the test.** `just parity-check` lists what is still wrong, as a
   to-do list: names with no entry, entries for names upstream removed, entries
   missing a `rust`, `reason` or `note`, and `rust` paths that do not exist.
6. **Work the list.** For each `planned` name, decide:
   - *Port it.* Write it the way a Rust library would (see below), add tests,
     then set `status = "implemented"` (or `"composed"`) with the `rust` path.
   - *Compose it.* If an existing Rust item already does the job, say which.
   - *Skip it.* Set `not-applicable` with a `reason` (for example Python-only
     test-harness plumbing).
7. **Regenerate the docs.** `just parity-docs` rewrites `docs/parity.md`.
8. **Update `ROADMAP.md`**, run the quality gate, and commit.

## Porting a feature

The Rust API is not a transliteration of the Python one.

- **One canonical name per capability.** Python's `type` is `type_text` here
  and there is no alias.
- **Use the type system.** A Python method with a mode string becomes an enum;
  several booleans become a builder; `None`-or-value becomes `Option`.
- **Fit the existing shape.** A `sb.cdp` method that takes a selector becomes a
  method on `Locator`; page-wide actions go on `Page`; browser-wide ones on
  `Browser`. A `BaseCase` method goes in the matching file under
  `src/api/base_case_impls/`.
- **Record differences.** If behaviour differs on purpose (0-based versus
  1-based indexes, JSON instead of pickle), add a `divergence` note.
- **Write the test first where you can.** Logic that needs no browser gets a
  plain `#[test]`; the mock browser (`Browser::new_mocked`) covers CDP
  commands; real-browser tests are `#[ignore]` and named in the roadmap.

## MCP tools

Each upstream MCP tool maps to a Rust tool of the same name in `mcp::cdp`,
`mcp::driver` or `mcp::sb`. A new tool is a `ToolDef` in that file (schema,
description, `Effect`, async handler); the parity test then checks it against
the manifest. See "Adding an MCP tool" in the developer guide.

## Watching for releases

`.github/workflows/upstream-watch.yml` runs weekly, compares the latest
SeleniumBase release with `parity/upstream.toml`, and opens an issue titled
"Upstream sync: SeleniumBase vX.Y.Z" when it is behind. The issue points here.
