---
title: "Design: Rust API Guidelines and Pragmatic Rust audit"
sidebarTitle: "API guidelines audit"
---

Status: audit note, written before any fix, on branch `lane/api-audit` (from `404208aa`, 2026-11-06).
It is read-only on code: no cargo command was run (no clippy, no rustdoc, no doctests), so every
statement comes from reading the source through the code index, from literal-text counts, and
from the two guideline books. Every `file:line` below is from that commit. The five areas that the
thirtyfour lanes are reworking are marked [in progress elsewhere](#in-progress-elsewhere) and are
not judged here.

## What was checked, and against what

* The [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html):
  54 items, fetched on 2026-11-06.
* The [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/) of Microsoft,
  version 2026.6 (book generated 2026-10-26): 90 guidelines, read from the single-file form at
  `agents/all.txt`. The lint set under M-STATIC-VERIFICATION is quoted from it below.
* Cargo's `[workspace.lints]` and `[lints] workspace = true`, `#[expect]` on lint groups, and the
  `clippy.toml` keys `allow-unwrap-in-tests`, `allow-expect-in-tests` and `allow-panic-in-tests`
  were checked on Context7 (`/websites/doc_rust-lang_cargo`, `/websites/doc_rust-lang_stable_reference`,
  `/rust-lang/rust-clippy`). The Rust Style Guide was looked at only through `rustfmt.toml`.
  Effective Rust was not read item by item.

How the numbers were made, and what they cannot say:

* A count is a literal-text search over `src/`, not `crates/`, not `tests/`, and not
  `src/mcp/dev/api_index.json` (it repeats every signature). It includes items declared `pub` in
  private modules and in `#[cfg(test)]` modules, so it is an upper bound on the exported surface.
* The surface in numbers (`src/`): 452 Rust files and 203,711 lines (tokei); `pub fn` 1,910,
  `pub async fn` 936, `pub struct` 425, `pub enum` 122, `pub trait` 15, `pub mod` 203,
  `#[non_exhaustive]` 70, `#[must_use` 1,065. `BaseCase` alone has about 700 public methods
  (452 `pub async fn` and 251 `pub fn` in the `src/api/base_case*` files).
* `module_api` was run on `src/error.rs` and `src/cookie.rs` only: for `BaseCase` its output is
  too large to read. The other modules were walked through `lib.rs`, the `mod.rs` files, literal
  counts and the representative types named below (`BaseCase`, `BrowserSession`, `BrowserConfig`,
  `Browser`, `Page`, `Cookie`, `Settings`, `RunSettings`, `Fingerprint` and its builder, the
  capability traits, the macros, the retry and shutdown helpers, the `testing` fixtures).
  `evasion`, `mcp`, `plugins`, `core`, `storage`, `profile_payloads`, `artifacts`, `plugin_host`
  and the packages under `crates/` were sampled, not walked item by item.
* Statuses: met, partly (some items meet it, some do not), not met, n/a (the crate has nothing
  the item is about), not checked. "Affected" is a verified count or a lower bound; "not counted"
  means only a compiler lint can count it, which is lane 1 and 2 below.

## Summary: Rust API Guidelines

Tally of the 54 items: met 26, partly 17, not met 3, n/a 7, not checked 1.

| Item | Status | Affected | In one line |
|---|---|---|---|
| C-CASE | met | 0 | rustc's naming lints are on by default and the gate runs clippy with `-D warnings`; no counter-example in the files read. |
| C-CONV | met | 2 | 13 `as_`, 4 `into_` and at least 30 `to_` methods fit their prefixes; `Browser::name` and `DriverMode::name` are the two odd names (`as_str` elsewhere). |
| C-GETTER | not met | 104 | 35 `pub fn get_` and 69 `pub async fn get_` (78 of them on `BaseCase`). |
| C-ITER | met | 0 | `ProxyList` has `iter` and `IntoIterator for &ProxyList` (`src/config/proxy_list.rs:518,535`). |
| C-ITER-TY | n/a | 0 | No public iterator type. |
| C-FEATURE | met | 0 | Feature names carry no `use-` or `with-`. |
| C-WORD-ORDER | partly | not counted | Error variants and proxy types mix word orders (F11). |
| C-COMMON-TRAITS | partly | 2 or more | `BaseCase` and `BrowserSession` have no `Debug` (F2); the rest needs the lint. |
| C-CONV-TRAITS | met | 0 | `FromStr` and `Display` on `Browser` and `DriverMode`; `From` impls on the selector types. |
| C-COLLECT | n/a | 0 | No public collection type needs `FromIterator`. |
| C-SERDE | met | 0 | `Cookie`, `BrowserConfig`, `RunSettings`, the fingerprint file derive serde. |
| C-SEND-SYNC | partly | 3 pinned | Pinned for `Browser`, `CdpInbox`, `Page` only (F14). |
| C-GOOD-ERR | partly | 10 types | `SeleniumBaseError` is `Error + Send + Sync + 'static` (pinned); 31 of 34 variants keep no source (F4). |
| C-NUM-FMT | n/a | 0 | No numeric formatting traits wanted. |
| C-RW-VALUE | n/a | 0 | No reader or writer taken by value. |
| C-EVOCATIVE | met | 0 | The `sb_*` macro names say what they do. |
| C-MACRO-ATTR | partly | 1 | `sb_test!` takes a closure-looking binder (the name `sb`, then a block) that is not a closure (F7). |
| C-ANYWHERE | met | 0 | The expression macros work wherever an expression does. |
| C-MACRO-VIS | n/a | 0 | No macro defines a public item with a caller-chosen visibility. |
| C-MACRO-TY | met | 0 | Fragments are `expr` and `ident`. |
| C-CRATE-DOC | met | 0 | `src/lib.rs:1-39`: summary, quick start, helpers, where the stealth layer went. |
| C-EXAMPLE | partly | 280 of about 2,846 | 280 `# Examples` sections for about 2,846 `pub` functions; 11 `ignore` fences (F3). |
| C-QUESTION-MARK | met | 0 | The examples read use `?`; not machine-checked. |
| C-FAILURE | partly | 894 of 2,580 | 894 `# Errors` against 2,580 `-> Result<` lines (an upper bound); 4 `# Panics` (F3). |
| C-LINK | met | 0 | Intra-doc links throughout the files read. |
| C-METADATA | partly | 5 fields | No `documentation`, `readme`, `keywords`, `categories` or `homepage` (F13). |
| C-RELNOTES | met | 0 | `CHANGELOG.md` exists (content not read). |
| C-HIDDEN | met | 2 | Two `#[doc(hidden)]` items: `config/test_options.rs:684`, `testing/mod.rs:66`. |
| C-SMART-PTR | met | 0 | No method on a smart pointer type; `Arc` is returned only by crate-private functions. |
| C-CONV-SPECIFIC | met | 0 | `as_webdriver_element`, `into_webdriver_element` name the type. |
| C-METHOD | met | 0 | Functions with an obvious receiver are methods (sampled). |
| C-NO-OUT | met | 0 | The `&mut Vec` parameters found mutate in place. |
| C-OVERLOAD | n/a | 0 | No operator overloading. |
| C-DEREF | not met | 3 | `MasterQA`, `SbCase`, `SbCdpCase` deref to `BaseCase` (F6). |
| C-CTOR | partly | 3 | `BaseCase::new` starts a browser; `with_` is both a setter and a constructor (F15). |
| C-INTERMEDIATE | met | 0 | `Page`, `Locator`, `Cookies` are cheap handles. |
| C-CALLER-CONTROL | met | 0 | No `&String`, `&Vec` or `&PathBuf` parameter; the 6 `&String` are `From<&String>` impls. |
| C-GENERIC | partly | not counted | Filesystem paths taken as `&str` (F9). |
| C-OBJECT | met | 0 | The four capability traits are object safe through `async_trait`. |
| C-NEWTYPE | partly | up to 370 | 269 `css: &str` and 101 `selector: &str` parameters (public or not), though `Selector` exists (F9). |
| C-CUSTOM-TYPE | partly | 9 or more | `bool` parameters where an enum fits, `uc: bool` for `DriverMode` (F9). |
| C-BITFLAG | n/a | 0 | No flag set. |
| C-BUILDER | partly | 4 plus 4 | Four real builders; four other construction styles (F15). |
| C-VALIDATE | partly | 3 | Public fields on `Cookie`, `BrowserConfig`, `Settings` skip validation at the set (F8). |
| C-DTOR-FAIL | met | 0 | `BrowserSession::quit` and `Browser::close` report failure; `Drop` swallows it. |
| C-DTOR-BLOCK | met | 0 | `Drop` spawns the cleanup (`common/shutdown.rs:322`, `sb_cdp/browser.rs:262`). |
| C-DEBUG | partly | 2 or more | Same as C-COMMON-TRAITS (F2). |
| C-DEBUG-NONEMPTY | met | 0 | `Cookie` and `BrowserConfig` print fields and hide secrets. |
| C-SEALED | not met | 6 | Six traits nobody should implement are open (F6). |
| C-STRUCT-PRIVATE | partly | 3 or more | `Cookie` 8, `BrowserConfig` 9 public fields; 70 `#[non_exhaustive]` for 547 structs and enums (F8). |
| C-NEWTYPE-HIDE | met | 0 | `SelectorBuf`, `FrameRef`, `WindowRef` hide their representation. |
| C-STRUCT-BOUNDS | not checked | n/a | No search can find derive bounds on generic structs. |
| C-STABLE | partly | 4 | Public dependencies at 0.x: thirtyfour 0.38, rstest 0.28, rstest-bdd 0.6, the plugin crate 0.1 (F13). |
| C-PERMISSIVE | met | 0 | MIT (`Cargo.toml:20`); `deny.toml` exists (not re-run). |

## Summary: Pragmatic Rust Guidelines

Tally of the 90 guidelines: met 20, partly 29, not met 7, n/a 11, not checked 23. The 23 not
checked are the 11 performance guidelines, the 3 application guidelines (they concern
`crates/cli`), and 9 single ones marked below.

| Guideline | Status | Affected | In one line |
|---|---|---|---|
| M-DESIGN-FOR-AI | met | 0 | `llms.txt`, `AGENTS.md`, `CLAUDE.md`, `context7.json`, the `dev` MCP server. |
| M-NO-META-DESIGN-DOCUMENTATION | partly | 681 lines | 681 doc lines say "Python's"; history notes at `src/lib.rs:32-39` and `src/error.rs:10-11` (F18). |
| M-RUST-SHAPED | partly | about 700 | `BaseCase` keeps Python's flat shape; `sb_cdp` is the Rust-shaped engine. |
| M-SINGLE-ITEM-PATH | not met | about 86 | The crate root re-exports about 86 names that also live in their modules (F12). |
| M-TAUTOLOGICAL-TESTS | not checked | n/a | Tests were not read. |
| M-PANIC-CONTINUATION, M-PANIC-IS-STOP, M-PANIC-MESSAGE | not checked | n/a | Needs the production `unwrap`/`expect`/`panic` list (lane 1). |
| M-PANIC-ON-BUG | partly | 3 | `BrowserSession::driver` panics on an expected state; two macro arms panic (F10). |
| M-UNSAFE-IMPLIES-UB, M-UNSAFE | met | 0 | No `unsafe` in the library; two calls in test code (`src/api/master_qa/wrapper_tests.rs:36,48`). |
| M-UNSOUND | not checked | n/a | Not audited. |
| M-CANONICAL-DOCS | partly | see C-FAILURE | Same gaps as C-FAILURE and C-EXAMPLE (F3). |
| M-DOC-INLINE | partly | 0 of 26 | `sb_cdp/mod.rs:110-177` marks every `pub use`; `src/lib.rs:201-246` marks none. |
| M-FIRST-DOC-SENTENCE | partly | not counted | Enforced only where the lint set is on (5 module trees). |
| M-MODULE-DOCS | partly | 3 | `common/mod.rs`, `utils/mod.rs`, `utilities/mod.rs` have no `//!`. |
| M-EXAMPLE-OVER-PROC | met | 0 | Only `macro_rules!`. |
| M-MACRO-HELPERS, M-MACRO-VERSION-PIN, M-PROC-IMPL, M-PROC-IMPLIED-ITEMS | n/a | 0 | No proc macro, no third-party helper item. |
| M-MACRO-LAST-RESORT | not met | 27 | 27 exported macros; those read are mostly a single method call (F7). |
| M-MACRO-MAIN-CRATE | partly | 1 | `sb_test!` expands to `#[tokio::test]`, which needs `tokio` in scope. |
| M-MACROS-DONT-LIE | partly | 2 or more | `sb_open!` returns a `Result` or panics by arity (F7). |
| M-CARGO-WORKSPACE | partly | 1 | `[workspace.package]` is used; no `[workspace.dependencies]`, no `[workspace.lints]` (F1, F13). |
| M-CRATES-FLAT-FOLDER | not met | 1 | The library is the root package, the others sit in `crates/`. |
| M-CRATES-IN-WORKSPACE | met | 0 | All three members are listed. |
| M-LATEST-EDITION | met | 0 | Edition 2024 (`Cargo.toml:18`). |
| M-MSRV | not met | 1 | `rust-version = "1.99"` is the newest stable; the guideline wants a few versions behind (F13). |
| M-DOCUMENTED-MAGIC | met | 0 | Constants carry their reason (`src/api/run_settings.rs:30-60`); sampled. |
| M-LINT-OVERRIDE-EXPECT | partly | 15 | 15 `#[allow(` against 25 `#[expect(` (F19). |
| M-LOG-STRUCTURED | not checked | n/a | `tracing` is used with named fields at `src/error.rs:543`; the rest not read. |
| M-PUBLIC-DEBUG | partly | 2 or more | See C-DEBUG. |
| M-PUBLIC-DISPLAY | not checked | n/a | Not audited. |
| M-REGULAR-FN | not checked | n/a | Not audited. |
| M-SHORT-NAMES | not checked | n/a | Not audited. |
| M-SMALLER-CRATES | not met | 1 | One crate holds two engines, the evasion layer, the MCP servers, a Python-to-Rust translator, the CLI half, reports and plugin hosts (F12). |
| M-STATIC-VERIFICATION | partly | see F1 | Gates exist; the lint set is a subset on 5 module trees. |
| M-UPSTREAM-GUIDELINES | n/a | 0 | This note. |
| M-WEASEL-WORDS | partly | 5 | `core::log_helper`, `core::report_helper`; the grab-bag modules `common`, `utils`, `utilities`. |
| M-FEATURES-ADDITIVE | partly | 1 | `BrowserConfig` gains a public field under feature `plugins` (F8). |
| M-OOBE | met | 0 | Drivers and browsers are downloaded on demand. |
| M-SYS-CRATES | n/a | 0 | No `-sys` crate. |
| M-DONT-LEAK-TYPES | partly | see list | thirtyfour types (in progress elsewhere); `serde_json::Value` and `tokio` through deliberate re-exports. |
| M-ESCAPE-HATCHES | met | 0 | `Element::as_webdriver_element` and `into_webdriver_element` (`src/browser/element.rs:143,149`), `Page::browser`. |
| M-FOREIGN-REEXPORTS | not met | 4 | `tokio`, `drivers`, `json`, `testing::rstest`: deliberate, see decision D8. |
| M-IMPL-ASREF | partly | not counted | `path: &str` and `file_path: &str` (F9). |
| M-IMPL-IO, M-IMPL-RANGEBOUNDS | n/a | 0 | Nothing takes a reader or a range. |
| M-TYPES-SEND | partly | 3 pinned | See C-SEND-SYNC. |
| M-AVOID-STATICS | partly | 21 | 20 `OnceLock` and 1 `LazyLock` caches; the registries in `common::shutdown` and `browser::reuse` (F17). |
| M-BUILD-RESULT | met | 4 | The four builders validate in `build()`. |
| M-INTEGRATION-TESTS | met | 0 | `tests/` holds the offline, chrome and contract binaries. |
| M-LOG-NOT-PRINT | partly | 3 | Three `eprintln!` in library code (F17). |
| M-MOCKABLE-SYSCALLS | partly | 1 | `sb_cdp::Browser::new_mocked` (feature `test-util`); the WebDriver engine's mock was not checked as exported. |
| M-NO-GLOB-REEXPORTS | partly | 1 | `src/profile_payloads/mod.rs:55`: `pub use profile::*;`. |
| M-STRONG-TYPES-GUARD | partly | 3 | Public fields (C-VALIDATE). |
| M-STRONG-TYPES | partly | see C-NEWTYPE | Strings for selectors and paths, `bool` for modes. |
| M-TEST-UTIL | met | 0 | Feature `test-util` (`Cargo.toml:194`). |
| M-ASYNC-FN | partly | 84 | 84 `#[async_trait` sites, boxing every call of the capability traits (F16). |
| M-AVOID-WRAPPERS | met | 0 | No `Arc`, `Rc` or `Box` in the public signatures read. |
| M-BALANCED-MODULES | partly | 4 | Files of 2,844, 2,836, 2,711 and 2,342 lines (tests included). |
| M-COLLECTION-TRAITS | met | 0 | `ProxyList`. |
| M-DI-HIERARCHY | met | 0 | Concrete types first; `dyn` only for `CdpTransport`-style seams. |
| M-ERRORS-CANONICAL-STRUCTS | not met | 1 | `SeleniumBaseError` is a 34-variant enum (F4). |
| M-ESSENTIAL-FN-INHERENT | met | 0 | `BaseCase` methods are inherent; the traits delegate (`src/api/traits.rs:107-130`). |
| M-FROM-ERROR | partly | 3 | `ResultExt::sb_context` and two `From` impls flatten to strings (F4). |
| M-INIT-BUILDER | met | 4 | Fingerprint, launch options, pool options, behavior. |
| M-INIT-CASCADED | met | 0 | `BrowserConfig` holds `BrowserSettings`, `SessionSettings`, `RunSettings`. |
| M-NO-PRELUDE | met | 0 | No `prelude` module. |
| M-PARAMETER-CONSISTENCY | partly | 2 | Selector first in `type_text`, text first in `assert_text` (F16). |
| M-SERVICES-CLONE | partly | 2 | `Browser` and `Page` are `Clone`; `BaseCase` and `BrowserSession` are single-owner. |
| M-SIMPLE-ABSTRACTIONS | met | 0 | No nested generic wrappers in the public types read. |
| M-FFI-NAMING, M-FFI-TRANSLATES, M-ISOLATE-DLL-STATE | n/a | 0 | The library has no FFI. |

## The lint set today

The Microsoft lint set is held on five module trees, each by a block of attributes in `src/lib.rs`:

| Module | Lines | Condition |
|---|---|---|
| `cookie` (private, re-exported) | 55-69 | always |
| `mcp` | 78-92 | feature `mcp-server` |
| `ocr` | 95-109 | feature `ocr` |
| `sb_cdp` | 153-167 | always |
| `visual` | 179-193 | feature `visual` |

Not under it: `api`, `artifacts`, `browser`, `cli`, `common`, `config`, `core`, `drivers`, `error`,
`evasion`, `js_code`, `json`, `macros`, `plugin_host`, `plugins`, `profile_payloads`, `resources`,
`storage`, `testing`, `tracing_util`, `utilities`, `utils`, and the packages `crates/cli`,
`crates/playwright`, `crates/own-imports`. That is most of the 452 files. A literal search for
`lints` in the manifests finds nothing, and `clippy.toml` holds only `doc-valid-idents`.

The block is also smaller than the guideline. Its 12 entries are `clippy::pedantic`, six of the
21 restriction lints (`allow_attributes_without_reason`, `clone_on_ref_ptr`, `map_err_ignore`,
`semicolon_outside_block`, `unused_result_ok`, `too_long_first_doc_paragraph`) and five of the 7
compiler lints (`missing_debug_implementations`, `redundant_imports`, `redundant_lifetimes`,
`trivial_numeric_casts`, `unused_lifetimes`). It lacks `ambiguous_negative_literals` and
`unsafe_op_in_unsafe_fn`, and the other 15 restriction lints.

One comment is misplaced: `src/lib.rs:151` says "held to for this module" above `pub mod evasion;`
(152), but the attribute block at 153-166 belongs to `pub mod sb_cdp;` (167). `evasion` is not
under the set.

## Findings

Each finding names the items, the proposed fix, and whether it breaks the public API.

### F1. No workspace lint set (M-STATIC-VERIFICATION, M-CARGO-WORKSPACE)

* Where: every manifest; `src/lib.rs:53-193`; `clippy.toml`.
* Fix: one `[workspace.lints]` in `Cargo.toml` with the guideline's table, and `[lints]` with
  `workspace = true` in the root package and in the three members. Cargo reads both on Context7
  (`[workspace.lints]` is inherited as `[lints] workspace = true`; `priority = -1` lets the
  explicit lints override the groups). The table, as the guideline gives it:

```toml
[workspace.lints.rust]
ambiguous_negative_literals = "warn"
missing_debug_implementations = "warn"
redundant_imports = "warn"
redundant_lifetimes = "warn"
trivial_numeric_casts = "warn"
unsafe_op_in_unsafe_fn = "warn"
unused_lifetimes = "warn"

[workspace.lints.clippy]
cargo = { level = "warn", priority = -1 }
complexity = { level = "warn", priority = -1 }
correctness = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
perf = { level = "warn", priority = -1 }
style = { level = "warn", priority = -1 }
suspicious = { level = "warn", priority = -1 }
allow_attributes_without_reason = "warn"
as_pointer_underscore = "warn"
assertions_on_result_states = "warn"
clone_on_ref_ptr = "warn"
deref_by_slicing = "warn"
disallowed_script_idents = "warn"
empty_drop = "warn"
empty_enum_variants_with_brackets = "warn"
empty_structs_with_brackets = "warn"
fn_to_numeric_cast_any = "warn"
if_then_some_else_none = "warn"
map_err_ignore = "warn"
redundant_type_annotations = "warn"
renamed_function_params = "warn"
semicolon_outside_block = "warn"
undocumented_unsafe_blocks = "warn"
unnecessary_safety_comment = "warn"
unnecessary_safety_doc = "warn"
unneeded_field_pattern = "warn"
unused_result_ok = "warn"
too_long_first_doc_paragraph = "warn"
literal_string_with_formatting_args = "allow"
```

* Project extras on top of the guideline (our rules, not the book's): `unsafe_code = "deny"` (the
  library has no `unsafe`; the two test calls take `#[expect(unsafe_code, reason = "...")]`), and
  `unwrap_used`, `expect_used`, `panic` at `warn`, with `allow-unwrap-in-tests = true`,
  `allow-expect-in-tests = true` and `allow-panic-in-tests = true` in `clippy.toml`. That turns
  the rule in `CLAUDE.md` ("`unwrap()` only in tests") into a check; today nothing enforces it, and
  the 3,836 `.unwrap()`, 532 `.expect(` and 130 `panic!(` in `src/` cannot be split into
  production and test by a text search.
* Staging: the set will not be clean on 203,711 lines. For each module that is not yet clean, put
  `#[expect(clippy::pedantic, reason = "lane N")]` on its `pub mod` line in `src/lib.rs`; a lint
  group expectation is fulfilled when any lint in the group fires (Rust Reference on Context7), and
  `unfulfilled_lint_expectations` then tells the lane that finishes a module to delete the line.
  The five existing blocks and the misplaced comment go away in the same commit. Check on the
  first module that Clippy's groups behave like rustc's before relying on it.
* `rustfmt.toml` pins `style_edition = "2021"` while the crate is edition 2024 and nothing says
  why; decision D7.
* Breaking: no.

### F2. `Debug` is missing on the two main types (C-DEBUG, M-PUBLIC-DEBUG)

* `BaseCase` (`src/api/base_case.rs:60`) and `BrowserSession` (`src/browser/session/mod.rs:49`)
  have no derive and no `impl Debug`; 30 other types in `src` (outside `sb_cdp`, `mcp`, `cli`) do,
  by hand: `MasterQA` (`src/api/master_qa.rs:462`), `Sb` (`src/testing/fixture.rs:157`), `Element`,
  `BrowserConfig` (`src/browser/config.rs:265`).
* Fix: a hand-written `Debug` that prints the test name, the engine and the mode and ends with
  `finish_non_exhaustive()`, as `MasterQA` does. The exact list of other gaps comes from
  `missing_debug_implementations` once lane 1 turns it on for the whole crate.
* Breaking: no.

### F3. Documentation gaps (C-EXAMPLE, C-FAILURE, M-CANONICAL-DOCS, M-MODULE-DOCS, M-DOC-INLINE)

* `BaseCase` has no doc comment at all (`src/api/base_case.rs:58-60`: a constant, a blank line,
  `pub struct BaseCase {`). The index's `doc_coverage` reports it among 8 undocumented public
  items in `src`; this one was confirmed by reading.
* The variants `Chrome`, `Chromium`, `Edge`, `Firefox` of `Browser` have no doc
  (`src/browser/config.rs:47-50`), the rest have.
* `src/common/mod.rs`, `src/utils/mod.rs` and `src/utilities/mod.rs` have no `//!`.
* The first line of `BrowserSession::driver` is "WebDriver interaction: `connect`."
  (`src/browser/session/mod.rs:85`), which says nothing about the method.
* `# Errors` is written 894 times, `-> Result<` appears 2,580 times; `# Panics` 4 times;
  `# Examples` 280 times. `clippy::missing_errors_doc` and `missing_panics_doc` (both pedantic)
  give the exact gaps in lane 1.
* 11 doc fences are `ignore`, most of them the macros in `src/macros.rs` (`:12,87,99,111,158`) and
  `src/evasion/patcher.rs:10`; they are never compiled. Use `no_run` (97 fences already do).
* `src/lib.rs:201-246` has 26 `pub use` statements without `#[doc(inline)]`, while
  `src/sb_cdp/mod.rs:110-177` marks every one; the root page of the docs therefore lists links, not
  items.
* Fix: a doc for `BaseCase` and the four variants, module docs for the three roots, then the lint
  output of lane 1 for `# Errors` and `# Panics`; `#[doc(inline)]` on the root re-exports.
* Breaking: no.

### F4. The error type (C-GOOD-ERR, M-ERRORS-CANONICAL-STRUCTS, M-FROM-ERROR)

* `SeleniumBaseError` (`src/error.rs:18-202`) is one `#[non_exhaustive]` enum of 34 variants.
  Fifteen are a bare `String` (`InvalidSelector`, `WaitTimeout`, `CdpDriver`, `Playwright`,
  `InvalidConfig`, `AssertionFailed`, `Unsupported`, `Gui`, `Screenshot`, `Pdf`,
  `Authentication`, `PythonMigration`, `Skipped`, `TestLifecycle`, `Database`); fifteen are struct
  variants of `String` fields (three add a number: `count`, `status`, `attempts`); one is a unit
  (`SessionNotStarted`). Only three variants keep a source: `WebDriver`, `Io`, `Json`
  (`#[from]`). A caller of a `Database(String)` error has nothing but the text to go on.
* `ResultExt::sb_context` (`src/error.rs:1054-1067`) is documented as "without losing the
  original error source", but it formats the error into `InvalidConfig(String)`: the source is
  lost, and every context becomes a configuration error whatever failed.
* `From<PluginError>` (`:230-258`) and `From<turso::Error>` (`:260-265`) call `to_string()`; the
  source is dropped.
* `Skipped` (`:183`) is a test-control signal carried as an error (`is_skipped`, `:482`).
* Nine other error types exist beside it, of different shapes: `FingerprintError`,
  `ScaffoldError` (`src/cli/scaffold.rs:50`), `FilterError` (`src/cli/scripts/run_filter.rs:127`),
  `SpecError` (`src/mcp/dev/spec.rs:93`), `ResourceError` (`src/mcp/resources.rs:127`),
  `ToolError` (`src/mcp/tool.rs:158`), and the logging-plugin errors at
  `src/plugins/azure_logging_plugin.rs:22`, `gcp_logging_plugin.rs:22`, `s3_logging_plugin.rs:15`.
  `sb_cdp::TransportError` was not read. Builders disagree on the error: `Fingerprint` builds with
  `FingerprintError` (`src/evasion/fingerprint.rs:1856`), the other three with
  `SeleniumBaseError`.
* `{:?}` is hand-written and prints the message plus the hint (`:209-228`); that is documented at
  `:15-17` and kept.
* Fix, small: keep the source in `sb_context` (a new `Context` variant or a boxed source), and
  keep the source in the two `From` impls with `#[source]`. Breaking: low.
* Fix, large: the canonical shape of M-ERRORS-CANONICAL-STRUCTS, a struct with a `kind()` enum, a
  source and a hint, and `Skipped` leaves the error type. Breaking: yes, and it waits for the
  thirtyfour driver-errors lane (the `WebDriver` variant, `src/error.rs:76`).

### F5. The `get_` prefix (C-GETTER)

* 104 `get_` functions: 35 `pub fn` and 69 `pub async fn`, 78 of them in the `BaseCase` files
  (`get_text`, `get_title`, `get_url`, `get_window_position` at `src/api/base_case.rs:480`).
  Others: `get_active_css_js`, `get_capture_js`, `get_recorder_js` (`src/evasion/js/`),
  `get_master_time` (`src/core/log_helper.rs:291`), `html::get_text` and `get_attribute`.
* `BaseCase::get_browser` (`src/api/base_case_impls/base_case_impl_extra.rs:1840`) returns a clone
  of the whole `BrowserConfig`, not a `Browser`; the index describes it as "the configured browser
  type". The name promises a cheap read and a different type.
* `Browser::name` and `DriverMode::name` should be `as_str`, like the 13 other `as_str`.
* The prefix is the Python names (`parity/api.toml`), so this is decision D2, not a quick rename.
  The cheap part: fix `get_browser`, the `*_js` getters and `name`.
* Breaking: yes.

### F6. Open traits and `Deref` for inheritance (C-SEALED, C-DEREF)

* Open and meant to stay closed: `BrowserApi`, `ElementApi`, `AssertionApi`, `ScreenshotApi`
  (`src/api/traits.rs:37,62,81,99`; implemented for `BaseCase` and `sb_cdp::Page` only),
  `ResultExt` (`src/error.rs:1056`, one blanket impl) and `Contains` (`src/api/membership.rs:21`).
  The other nine `pub trait` are extension points (`EvasionProvider`, `CdpTransport`, `RunSink`,
  `Finalizer`, `TestPlugin`, `PageEvidence`, `PromptPage`, `LineInput`, `Closeable`) and stay open.
* `impl Deref` and `DerefMut` for `MasterQA` (`src/api/master_qa.rs:446-460`), `SbCase`
  (`src/testing/bdd/mod.rs:203-215`) and `SbCdpCase` (`:326`) make a wrapper look like `BaseCase`.
* Fix: a private `Sealed` supertrait on the six; replace the three `Deref` impls with an explicit
  `case()` and `case_mut()` (`Scenario` already has both, and the `SbCase` impls call them,
  `src/testing/bdd/mod.rs:207,213`).
* Breaking: yes (callers of `qa.open(..)` write `qa.case_mut().open(..)`).

### F7. Macros (M-MACRO-LAST-RESORT, M-MACROS-DONT-LIE, C-MACRO-ATTR)

* `src/macros.rs` exports 26 macros (`selector!`, `sb_test!`, `sb_open!`, `sb_click!`, `sb_type!`,
  ..., `sb_with_timeout!`, `sb_config!`, `uc_config!`, `cdp_config!`, `fingerprint!`) and
  `src/config/test_options.rs:683-685` a 27th, `sb_test_options!`, which is `#[doc(hidden)]`. The ones read
  (`sb_open!`, `sb_click!`, `sb_type!`, `sb_hover!`, `sb_assert_visible!`) are one method call with
  `.await` added; `uc_config!` and `cdp_config!` are `BrowserConfig::default().with_mode(..)`.
* An optional last argument changes the type: `sb_open!(sb, url)` is a `Result`, `sb_open!(sb, url,
  "msg")` is `()` and panics (`:90-95`); the same arm shape is in `sb_click!`, `sb_type!`,
  `sb_hover!` and `sb_assert_visible!` (`:234-239`, whose own example ends in `?`). `sb_test!`
  ends in `.expect("test failed")` (`:80`).
* `sb_config!` lists seven fixed orders of three options (`:314-344`); an eighth setting needs a
  new arm.
* `sb_with_timeout!` drops the error of restoring the timeout (`let _ =`, `:418`).
* Fix: keep `sb_test!` and, if the owner wants them, `selector!` and `fingerprint!`; delete the
  single-call wrappers and the `_config!` macros; if any wrapper stays, one arity, one type.
* Breaking: yes. Decision D3.

### F8. Evolution of the public types (C-STRUCT-PRIVATE, C-VALIDATE, M-FEATURES-ADDITIVE)

* `BrowserConfig` (`src/browser/config.rs:202-248`) has 9 public fields, no `#[non_exhaustive]`,
  and one of them, `plugins`, exists only with feature `plugins` (`:245-247`). A crate that builds
  the struct with a literal compiles until another crate in the build turns the feature on.
  Features must be additive; this one is not.
* `Cookie` has 8 public fields (`src/cookie.rs:62-85`) and `Settings` public tables
  (`src/config/settings.rs:100-135`); both are deliberate (the cookie note and the settings note),
  but neither is `#[non_exhaustive]`, so adding a field breaks every literal.
* `Browser` and `DriverMode` (`src/browser/config.rs:44-76,150-175`) are not `#[non_exhaustive]`,
  though a browser was added recently (`Comet`). The `SeleniumBaseError`, `SameSite` and 68
  other items are.
* Fix: `#[non_exhaustive]` on the config structs and the two enums; the `plugins` field becomes
  unconditional (a zero-size `BrowserPlugins` when the feature is off).
* Breaking: yes.

### F9. Strings, booleans and paths where a type fits (C-NEWTYPE, C-CUSTOM-TYPE, C-GENERIC)

* Selectors: 269 `css: &str` and 101 `selector: &str` parameters, although `Selector` and
  `SelectorBuf` exist (`src/utils/selectors.rs`) and `sb_cdp::Page::locator` already takes
  `impl Into<SelectorBuf>` (`src/sb_cdp/page.rs:133`). The name `css` is also wrong when the
  string is an XPath.
* Booleans: `uc: bool` where `DriverMode` exists (`src/api/recorder.rs:312,364`,
  `src/api/recorder/generate.rs:152`); `to_html(show_notes: bool, interval, end_marker: bool)`
  (`src/api/presentation.rs:446`); `Chart::to_html(include_libs: bool)` (`src/api/chart.rs:526`);
  `make_runnable(path, resign: bool)` (`src/browser/driver_access.rs:69`);
  `convert(input, force: bool)` (`src/cli/scripts/sb_convert.rs:72`); `install_addon(path,
  temporary: bool)` (`src/api/base_case_impls/base_case_impl_addon.rs`) and
  `Locator::click_with_offset(x, y, center: bool)` (`src/sb_cdp/locator.rs`). Nine sites; builder
  toggles such as `Cookie::http_only(mut self, bool)` are fine and not counted.
* Paths: `file_path: &str` (`src/api/base_case.rs:2267`), `path: &str`
  (`src/api/base_case_impls/base_case_impl_addon.rs:112`) and `filename: &str`, while `&Path` is
  used elsewhere (`make_runnable`, `convert`), so the style is mixed. A text search for
  `path: &str` finds 32 lines, but it also matches `xpath: &str` and URL paths, so the exact
  count needs the review of the lane.
* Durations are right: 83 `timeout: Duration` against one `default_timeout: f64`
  (`src/mcp/webdriver.rs:593`).
* Fix: `impl AsRef<Path>` for filesystem paths; enums for the booleans; for selectors, a lane of
  its own that moves `BaseCase` to `impl Into<SelectorBuf>` the way `Page::locator` does (it is
  the largest change in this note: about 370 signatures).
* Breaking: yes.

### F10. Panics on expected states (M-PANIC-ON-BUG)

* `BrowserSession::driver` (`src/browser/session/mod.rs:93-99`) panics after `quit`, on a direct
  CDP attachment and after a failed `reconnect`: states a caller can reach without a bug. Its
  sibling `webdriver` (`:116`) returns the right error.
* The message arms of the `sb_*!` macros and `sb_test!` panic by design (F7).
* There are 4 `# Panics` sections, so the rest is undocumented or absent; lane 1's
  `missing_panics_doc` and the `panic` lint list them.
* Fix: remove `driver()` in favour of `webdriver()` once the drivers lane lands.
* Breaking: yes. Blocked by the drivers module.

### F11. Names that collide or drift (C-WORD-ORDER, M-WEASEL-WORDS)

* `seleniumbase_rs::Browser` (`src/lib.rs:211`) is the enum of browser kinds;
  `seleniumbase_rs::sb_cdp::Browser` (`src/sb_cdp/mod.rs:113`) is the engine; the fingerprint has a
  third, `BrowserType` (`src/lib.rs:223`).
* `seleniumbase_rs::Key` is `sb_cdp::Key` (`src/lib.rs:236`), while `src/api/base_case.rs:48`
  imports thirtyfour's `Key` for its own use: two types of one name in one crate.
* Six types for one concept, a proxy: `sb_cdp::Proxy`, `evasion::fingerprint::ProxyConfig`,
  `browser::options::ProxySettings`, `browser::options::proxy::ProxyAddress`,
  `config::proxy_list::ProxyEntry`, `profile_payloads::ProfileProxy`.
* Four names for plugins: `plugins` (reporting plugins), `plugin_host`, `plugin_api`,
  `builtin_plugins`; the doc at `src/lib.rs:119` has to say "Not to be confused with `plugins`".
* Three grab-bag modules, `common`, `utils`, `utilities`, and two `*_helper` modules
  (`core::log_helper`, `core::report_helper`).
* Two CDP page APIs: `api::cdp_page` and `api::cdp_driver` ("prefer `crate::sb_cdp`",
  `src/api/mod.rs:9`) and `sb_cdp::Page`.
* Error variants mix orders: `ElementNotFound`, `StaleElement`, `InvalidSelector`,
  `BrowserLaunch`, `WaitTimeout`.
* Fix: rename the enum `Browser` to `BrowserKind`; one `Proxy` type with conversions; fold
  `common`, `utils` and `utilities` into named modules; delete or hide `api::cdp_page` and
  `cdp_driver` after a deprecation-free removal (no users yet).
* Breaking: yes.

### F12. The crate root and the size of the crate (M-SINGLE-ITEM-PATH, M-SMALLER-CRATES, M-BALANCED-MODULES, M-NO-GLOB-REEXPORTS)

* `src/lib.rs:201-246` re-exports about 86 names, 27 of them from `evasion::fingerprint`, and the
  27 macros sit at the root as well. Each is reachable by two paths. `Cookie` has two public
  paths (`seleniumbase_rs::Cookie` and `sb_cdp::Cookie`).
* 203 `pub mod` in `src`; `browser` alone exposes 15, including plumbing
  (`downloader`, `driver_access`, `driver_install`, `launcher`, `virtual_display`).
* `src/cli/` is "the library half of the command line": 204 `pub fn` in a library that has no
  parser. `utilities::importer` holds a Python-to-Rust translator (`translate.rs`, 2,836 lines)
  with `import_python` at the root.
* `src/profile_payloads/mod.rs:55` is the one glob re-export found in the `mod.rs` files and
  `lib.rs`.
* Fix: narrow to `pub(crate)` everything the `tests/contract_*.rs` files and the stealth, Tauri and
  plugin crates do not import; keep one path per item; replace the glob. A crate split (importer,
  CLI half, MCP servers) is decision D6.
* Breaking: yes.

### F13. Manifest (C-METADATA, C-STABLE, M-MSRV, M-CARGO-WORKSPACE)

* `[package]` (`Cargo.toml:23-30`) has `description`, `license`, `repository`; it lacks
  `documentation`, `readme`, `keywords`, `categories`, `homepage`.
* Feature `error-backtrace` (`:183`) is empty and nothing reads it; `docs/DEVELOPER_GUIDE.md:493`
  says so, `README.md:515` and `CONTRIBUTING.md:300` say it "captures backtraces". A feature that
  does nothing and is documented as doing something.
* `rust-version = "1.99"` is the newest stable (`:19`); M-MSRV asks for a few versions behind.
  `CLAUDE.md` says this is deliberate, so it is decision D1.
* No `[workspace.dependencies]`: versions are repeated per member (F1 adds the lints; this adds the
  dependencies).
* Public dependencies below 1.0: `thirtyfour` 0.38 (`:82`), `rstest` 0.28 (`:164`), `rstest-bdd`
  0.6 (`:166`), and `seleniumbase-rs-plugin` 0.1 from a git revision (`:155`), re-exported as
  `plugin_api`. The `semver` task in `mise.toml:210` (cargo-semver-checks against a tag) exists,
  which is the right gate.
* Breaking: removing `error-backtrace` breaks anyone who enables it; nobody is known to.

### F14. `Send` and `Sync` are pinned for three types (C-SEND-SYNC, M-TYPES-SEND)

* `tests/contract_tauri.rs:40-42` pins `Browser`, `CdpInbox`, `Page`; `tests/contract_stealth.rs:244-257`
  pins two error types and `Debug + Clone + PartialEq` for five values. `BaseCase`, `Locator`,
  `BrowserPool`, `Lease`, `Cookies`, `RunSettings`, `Sb`, `SbCdp` are not pinned, and `BaseCase`
  is the type every `tokio::spawn` user meets.
* Fix: one test file with an `assert_send_sync` over every public handle type.
* Breaking: no.

### F15. Constructors and builders (C-CTOR, C-BUILDER, M-INIT-BUILDER)

* `BaseCase::new` is `async` and starts a browser (`src/api/base_case.rs:131`), where
  `sb_cdp::Browser::launch` says what it does; `BaseCase::with_session` is a constructor
  (`:161`), while `with_*` elsewhere is a setter (`BrowserConfig::with_headless`,
  `Page::with_timeout`).
* Four real builders with a fallible `build()` (`BehaviorBuilder`, `FingerprintBuilder`,
  `LaunchOptionsBuilder`, `PoolOptionsBuilder`) and four other styles: 80 `with_*` on
  `BrowserConfig`, consuming setters named like the field on `Cookie`, `RunSettings::with_*`,
  public fields on `Settings`. The builder note (`/design/bon_builders`) settled the first two;
  this audit keeps that decision.
* Fix: `BaseCase::connect(config)` for `new`, `BaseCase::from_session` for `with_session`;
  nothing else.
* Breaking: yes, small.

### F16. Capability traits (M-ASYNC-FN, M-PARAMETER-CONSISTENCY)

* All four use `#[async_trait]` (`src/api/traits.rs:36,61,80,98`), 84 sites in all. That boxes
  every call and fixes the futures' bounds; native `async fn` in traits drops the boxing but not
  the object safety. Decision D5.
* Receivers disagree for the same kind of call: `refresh(&self)` and `get_title(&mut self)`
  (`:45,54`), `get_url(&mut self)` (`:57`), `get_text(&mut self, ..)` (`:73`) read state and take
  `&mut self`.
* Parameter order: `type_text(css, text)` (`:70`) but `assert_text(expected, css)` (`:91`); the
  doc says Python does so, but one trait cannot teach two orders.
* Fix: `&self` for reads; settle one order for the Rust names.
* Breaking: yes.

### F17. Printing, statics, global effects (M-LOG-NOT-PRINT, M-AVOID-STATICS)

* `eprintln!` in library code: `src/api/master_qa.rs:431`, `src/browser/driver_access.rs:288`,
  `src/mcp/http.rs:842`. Use `tracing`.
* 20 `OnceLock` and 1 `LazyLock`; the registry `CLEANUPS` (`src/common/shutdown.rs:308`) and the
  reuse registry. Public functions with process-wide effect sit at the root: `install_tls_provider`,
  `init_tracing*` (5), `shutdown_reused_sessions`. They are documented; they are also the reason
  two tests in one process can disturb each other.
* Fix: `tracing` for the three prints; leave the rest (they are the design) and list them in the
  crate docs.
* Breaking: no.

### F18. Docs that tell the design history (M-NO-META-DESIGN-DOCUMENTATION)

* `src/lib.rs:32-39` (where the stealth layer went), `:110` ("ADR 004"), `src/error.rs:10-11`
  ("Existing tuple variants are preserved for backwards compatibility", which the no-shims rule
  contradicts), and 681 lines that explain behaviour by "Python's ...".
* Fix: keep the Python references that the parity table needs; move history to the design notes
  and the changelog.
* Breaking: no.

### F19. Lint overrides (M-LINT-OVERRIDE-EXPECT)

* 15 `#[allow(` in `src` against 25 `#[expect(`. Of the 14 the search listed, nine are one-line
  with no reason: `src/api/recorder/cdp.rs:409`, `src/browser/driver_access.rs:199`,
  `src/config/test_options.rs:487`, `src/evasion/behavior.rs:130,392,403,631,645`,
  `src/evasion/humanize.rs:107`; five more open a multi-line attribute (`src/mcp/export.rs:307`,
  `failure.rs:390`, `journal.rs:228`, `src/sb_cdp/session_store.rs:129,162`) whose reasons were not
  read.
* Fix: `#[expect(.., reason = "..")]` for each, in the same lane as the module's pedantic pass.
* Breaking: no.

## In progress elsewhere

Not judged here: driver errors (`SeleniumBaseError::WebDriver`, `src/error.rs:76`), network
conditions (`#![expect(deprecated)]` for `NetworkConditions`, `src/api/base_case.rs:1-6`), the
DevTools client (`evasion::cdp::CdpClient`, `BrowserSession::cdp*`), capability types
(`browser::options::capabilities`), and the `drivers` module (`src/drivers.rs`). Findings that
depend on them: F4 large, F10, and M-DONT-LEAK-TYPES. `Element::as_webdriver_element` is an
escape hatch by design and counts as M-ESCAPE-HATCHES met.

## Fix lanes

In this order; each is one concern, with the files it mostly touches. None is started.

1. **The Microsoft lint set for the whole workspace through `[workspace.lints]`** (F1, F19).
   Files: `Cargo.toml`, `crates/cli/Cargo.toml`, `crates/playwright/Cargo.toml`,
   `crates/own-imports/Cargo.toml`, `src/lib.rs`, `clippy.toml`. Not breaking. Output: the exact
   lists for lanes 2 to 4 (`missing_debug_implementations`, `missing_errors_doc`,
   `missing_panics_doc`, `unwrap_used`).
2. **`Debug` and `Send + Sync` pins** (F2, F14). Files: `src/api/base_case.rs`,
   `src/browser/session/mod.rs`, whatever the lint lists, one new test file. Not breaking.
3. **Doc gaps** (F3). Files: `src/api/base_case.rs`, `src/browser/config.rs`, `src/common/mod.rs`,
   `src/utils/mod.rs`, `src/utilities/mod.rs`, `src/lib.rs`, `src/macros.rs` (fences). Not breaking.
4. **Manifest and tooling** (F13, F1 rustfmt): metadata, `[workspace.dependencies]`,
   `error-backtrace`, after decisions D1 and D7. Files: the four manifests, `rustfmt.toml`.
   Breaking only for the feature.
5. **Keep the source in errors** (F4 small). Files: `src/error.rs` and its callers. Low breaking.
6. **Print, glob, small hygiene** (F17, F12 glob). Files: `src/api/master_qa.rs`,
   `src/browser/driver_access.rs`, `src/mcp/http.rs`, `src/profile_payloads/mod.rs`. Not breaking.
7. **Sealed traits and no `Deref`** (F6). Files: `src/api/traits.rs`, `src/error.rs`,
   `src/api/membership.rs`, `src/api/master_qa.rs`, `src/testing/bdd/mod.rs`, their tests. Breaking.
8. **`non_exhaustive` and the unconditional `plugins` field** (F8). Files:
   `src/browser/config.rs`, `src/cookie.rs`, `src/config/settings.rs`, and every struct literal
   (use `constructors` to list them first). Breaking.
9. **Booleans to enums, paths to `AsRef<Path>`** (F9 without selectors). Files: the sites in F9.
   Breaking.
10. **Macros** (F7), after D3. Files: `src/macros.rs`, `src/lib.rs`, `tests/`, `examples/`,
    `docs/`. Breaking.
11. **Names** (F11, F5 cheap part, F15): `BrowserKind`, one `Proxy`, `connect` and `from_session`,
    `as_str`, `get_browser`. Files: `src/browser/config.rs`, `src/lib.rs`,
    `src/evasion/fingerprint.rs`, `src/browser/options/`, `src/config/proxy_list.rs`,
    `src/profile_payloads/profile.rs`, `src/api/base_case.rs`. Breaking. Use `tokensave_rename`.
12. **Trait receivers and parameter order** (F16), with D5. Files: `src/api/traits.rs` and the
    `BaseCase` and `Page` impls. Breaking.
13. **Public module tree** (F12): `pub(crate)` pass, one path per item. Files: `src/lib.rs`,
    `mod.rs` files, the contract tests. Breaking; run `mise run semver` and the contract tests.
14. **Error redesign** (F4 large), after the thirtyfour driver-errors lane. Files: `src/error.rs`
    and every constructor call. Breaking, large.
15. **Selector typing** (F9 selectors): `BaseCase` on `impl Into<SelectorBuf>`. Up to 370
    signatures (public or not); do it with `ast_grep_rewrite`. Breaking, large.
16. **`get_` names** (F5), only if D2 says yes; with the translator tables
    (`src/utilities/importer/translate.rs`, `src/mcp/dev/api_index.json`) and `parity/api.toml`.
    Breaking, large.
17. **`panic` to `Result` for `driver()`** (F10), after the drivers lane. Files:
    `src/browser/session/mod.rs`. Breaking.
18. **Docs without history** (F18). Files: wherever the search lists. Not breaking.

## Decisions for the owner

* D1. MSRV: keep the newest stable, or a few versions behind (M-MSRV)?
* D2. Keep the Python `get_*` names on `BaseCase`, or rename about 104 to Rust names?
* D3. Keep any of the 27 `sb_*` macros, or cut to `sb_test!`?
* D4. Is `Skipped` an error? Is the canonical error struct wanted at all?
* D5. `async_trait` or native async functions in the four traits?
* D6. Split the crate (translator, CLI half, MCP servers) or keep one?
* D7. `style_edition = "2021"`: intended?
* D8. The deliberate re-exports (`tokio`, `json`, `testing::rstest`, `drivers`): an accepted
  deviation from M-FOREIGN-REEXPORTS? The imports note in `CLAUDE.md` says yes; this audit lists
  them so the choice is written down.

## Not checked

* Anything a compiler lint reports: the full list of types without `Debug`, `Clone`, `Eq`, `Hash`;
  missing `# Errors` and `# Panics`; production `unwrap`, `expect`, `panic`; unused doc links.
* Whether each public type is `Send + Sync` beyond the pinned three.
* The 11 performance guidelines, the 3 application guidelines for `crates/cli`, M-UNSOUND,
  M-TAUTOLOGICAL-TESTS, M-LOG-STRUCTURED beyond one site, M-PUBLIC-DISPLAY, M-REGULAR-FN,
  M-SHORT-NAMES, C-STRUCT-BOUNDS.
* The item-level API of `evasion`, `mcp`, `plugins`, `core`, `storage`, `profile_payloads`, the
  packages under `crates/`, and the features `gui`, `tui`, `turso`, `ocr`, `visual`, `pdf`.
* Whether `cargo-semver-checks` is clean on the current tree (not run).

## Sources

* [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
* [Pragmatic Rust Guidelines 2026.6](https://microsoft.github.io/rust-guidelines/) (all guidelines
  in one file: `agents/all.txt`)
* Cargo reference, workspaces and the `lints` table; the Rust Reference on lint attributes; the
  Clippy lint configuration book (all read on Context7).
