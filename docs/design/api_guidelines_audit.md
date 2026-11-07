---
title: "Design: Rust API Guidelines and Pragmatic Rust audit"
sidebarTitle: "API guidelines audit"
---

Status: audit note, written before any fix, on branch `lane/api-audit` (from `404208aa`, 2026-11-06).
It is read-only on code: no cargo command was run (no clippy, no rustdoc, no doctests), so every
statement comes from reading the source through the code index, from literal-text counts, and
from the two guideline books. Every `file:line` below is from that commit (the second pass, below,
re-read the lines it cites). The five areas that the thirtyfour lanes are reworking are marked
[in progress elsewhere](#in-progress-elsewhere) and are not judged here.

Two passes made this note. The first walked the root, `api`, `browser`, `sb_cdp`, `error`,
`cookie`, the macros and the manifests. The second spot-checked the first pass against the code
(35 claims: 26 stood and 9 were corrected, listed in
[What the second pass changed](#what-the-second-pass-changed)), walked the modules the first pass
had only sampled (`storage`, `plugin_host`, `artifacts`, `plugins`, `profile_payloads`, `core`,
`testing`, `utilities`, `crates/playwright`), added two findings (F20 and F21) and two fix lanes,
and moved two checklist items from met to partly.

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
* `module_api` was run on `src/error.rs`, `src/cookie.rs`, `src/storage`, `src/plugin_host`,
  `src/artifacts.rs` and `crates/playwright/src`: for `BaseCase` its output is too large to read.
  The other modules were walked through `lib.rs`, the `mod.rs` files, literal counts and the
  representative types named below (`BaseCase`, `BrowserSession`, `BrowserConfig`, `Browser`,
  `DriverMode`, `Page`, `Cookie`, `Settings`, `RunSettings`, `Fingerprint` and its builder, the
  capability traits, the macros, the retry and shutdown helpers, the `testing` fixtures, the
  `S3LoggingPlugin`, `ProfileParams`, `IdeCommand`, the `Drop` impls). `evasion` (outside
  `fingerprint`), `mcp`, `config` (outside `Settings`), `utils`, `common` and `crates/cli` were
  sampled, not walked item by item: [Not checked](#not-checked) lists what is left.
* `doc_coverage` was tried and is not used for counts: it reported 80 or more undocumented items
  in 14 files, and the two files read (`src/api/jqc.rs:98-116`, `src/api/chart.rs:106-139`) have a
  doc comment on every item it listed (a `///` above `#[default]` reads as missing).
* Statuses: met, partly (some items meet it, some do not), not met, n/a (the crate has nothing
  the item is about), not checked. "Affected" is a verified count or a lower bound; "not counted"
  means only a compiler lint can count it, which is lane 1 and 2 below.

## Summary: Rust API Guidelines

Tally of the 54 items: met 24, partly 19, not met 3, n/a 7, not checked 1 (the first pass had met 26
and partly 17; C-METHOD and C-DTOR-BLOCK moved to partly in the second).

| Item | Status | Affected | In one line |
|---|---|---|---|
| C-CASE | met | 0 | rustc's naming lints are on by default and the gate runs clippy with `-D warnings`; no counter-example in the files read. |
| C-CONV | met | 4 | 13 `as_`, 4 `into_` and at least 30 `to_` methods fit their prefixes; four `const fn name(self) -> &'static str` are the odd names where `as_str` is used elsewhere (`Browser::name` and `DriverMode::name` in `src/browser/config.rs:91,169`, `src/browser/chrome_for_testing.rs:84`, `src/browser/options/proxy.rs:120`). |
| C-GETTER | not met | 104 | 35 `pub fn get_` and 69 `pub async fn get_` (78 of them on `BaseCase`). |
| C-ITER | met | 0 | `ProxyList` has `iter` and `IntoIterator for &ProxyList` (`src/config/proxy_list.rs:518,535`). |
| C-ITER-TY | n/a | 0 | No public iterator type. |
| C-FEATURE | met | 0 | Feature names carry no `use-` or `with-`. |
| C-WORD-ORDER | partly | not counted | Error variants and proxy types mix word orders (F11). |
| C-COMMON-TRAITS | partly | 4 or more | `BaseCase` and `BrowserSession` have no `Debug`; `Browser` and `DriverMode` are `Copy` enums without `Hash` (F2); the rest needs the lint. |
| C-CONV-TRAITS | met | 0 | `FromStr` and `Display` on `Browser` and `DriverMode`; `From` impls on the selector types. |
| C-COLLECT | n/a | 0 | No public collection type needs `FromIterator`. |
| C-SERDE | met | 0 | `Cookie`, `BrowserConfig`, `RunSettings`, the fingerprint file derive serde. |
| C-SEND-SYNC | partly | 3 pinned | Pinned for `Browser`, `CdpInbox`, `Page` only (F14). |
| C-GOOD-ERR | partly | 10 types, 6 functions | `SeleniumBaseError` is `Error + Send + Sync + 'static` (pinned); 31 of 34 variants keep no source; 4 public functions return `Result<_, String>` and 2 return `Box<dyn Error>` without `Send + Sync` (F4). |
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
| C-METHOD | partly | 3 | Three unit structs (`S3LoggingPlugin`, `AzureLoggingPlugin`, `GcpLoggingPlugin`) hold one receiver-less `upload_file` each and nothing else (F20); the rest was sampled. |
| C-NO-OUT | met | 0 | The `&mut Vec` parameters found mutate in place. |
| C-OVERLOAD | n/a | 0 | No operator overloading. |
| C-DEREF | not met | 3 | `MasterQA`, `SbCase`, `SbCdpCase` deref to `BaseCase` (F6). |
| C-CTOR | partly | 3 | `BaseCase::new` starts a browser; `with_` is both a setter and a constructor (F15). |
| C-INTERMEDIATE | met | 0 | `Page`, `Locator`, `Cookies` are cheap handles. |
| C-CALLER-CONTROL | met | 0 | No `&String`, `&Vec` or `&PathBuf` parameter; the 6 `&String` are `From<&String>` impls. |
| C-GENERIC | partly | not counted | Filesystem paths taken as `&str` (F9). |
| C-OBJECT | met | 0 | The four capability traits are object safe through `async_trait`. |
| C-NEWTYPE | partly | up to 371 | 270 `css: &str` and 101 `selector: &str` parameters (public or not), though `Selector` exists (F9). |
| C-CUSTOM-TYPE | partly | 9 or more | `bool` parameters where an enum fits, `uc: bool` for `DriverMode` (F9). |
| C-BITFLAG | n/a | 0 | No flag set. |
| C-BUILDER | partly | 4 plus 4 | Four real builders; four other construction styles (F15). |
| C-VALIDATE | partly | 3 | Public fields on `Cookie`, `BrowserConfig`, `Settings` skip validation at the set (F8). |
| C-DTOR-FAIL | met | 0 | `BrowserSession::quit` and `Browser::close` report failure; `Drop` swallows it. |
| C-DTOR-BLOCK | partly | 3 | `BrowserSession` and the `sb_cdp` handles spawn their cleanup; `DriverProcess` and `XvfbProcess` kill and `wait()` in `drop`, and `TestRecorder` writes report files in `drop` (F21). |
| C-DEBUG | partly | 2 or more | Same as C-COMMON-TRAITS (F2). |
| C-DEBUG-NONEMPTY | met | 0 | `Cookie` and `BrowserConfig` print fields and hide secrets. |
| C-SEALED | not met | 6 | Six traits nobody should implement are open (F6); `crates/playwright` seals `PlaywrightTarget` correctly, the pattern to copy. |
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
| M-MODULE-DOCS | partly | 4 | `common/mod.rs`, `config/mod.rs`, `utils/mod.rs`, `utilities/mod.rs` have no `//!`. |
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
| M-LOG-NOT-PRINT | partly | 3 | Three `eprintln!` in library code (F17). The five `println!` in `BaseCase` and the `rich_helper` print functions are documented prints, not counted. |
| M-MOCKABLE-SYSCALLS | partly | 9 | The settings code takes an injected lookup (`Settings::load_with`, `from_vars`); three `upload_file`, `artifacts::logs_root`, `find_system_chrome` and four cookie-file methods read the environment or the working directory directly (F20). `sb_cdp::Browser::new_mocked` (feature `test-util`) is the engine-level mock; the WebDriver engine's was not checked as exported. |
| M-NO-GLOB-REEXPORTS | partly | 1 | `src/profile_payloads/mod.rs:55`: `pub use profile::*;`. |
| M-STRONG-TYPES-GUARD | partly | 3 | Public fields (C-VALIDATE). |
| M-STRONG-TYPES | partly | see C-NEWTYPE | Strings for selectors and paths, `bool` for modes, keyword strings in `ProfileParams` (F9). |
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

## The ten findings that matter most

Ranked by how many callers meet the problem and by how hard it is to fix later.

1. **No lint set in the manifests (F1).** The Pragmatic Rust set is held on 5 of about 27 module
   trees by attributes in `src/lib.rs`; `[workspace.lints]` does not exist, so over 4,000 lines with `.unwrap()`,
   600 with `.expect(` and 160 with `panic!(` in `src` are unchecked and every count in this note that says
   "not counted" waits on it.
2. **Selectors are `&str` in about 370 signatures (F9)** although `Selector` and `SelectorBuf`
   exist and `Page::locator` already takes `impl Into<SelectorBuf>`; the parameter is called `css`
   even when it holds an XPath.
3. **104 `get_` functions (F5, C-GETTER)**, 78 on `BaseCase`, the one checklist item with a
   count in the hundreds. It follows the Python names, so it is decision D2, not a quick fix.
4. **27 exported macros, mostly one method call (F7)**; in five of them an optional last argument
   changes the type from a `Result` to `()` that panics.
5. **The error type keeps almost no source (F4):** 31 of 34 variants hold only text,
   `ResultExt::sb_context` turns every error into `InvalidConfig`, and public functions return
   `Result<_, String>` (4) or `Box<dyn Error>` without `Send + Sync` (2).
6. **Open traits and `Deref` for inheritance (F6):** six traits that nobody should implement are
   open, and `MasterQA`, `SbCase`, `SbCdpCase` deref to `BaseCase`. `crates/playwright` shows the
   sealed form.
7. **`BrowserConfig` is not additive (F8):** a public field exists only under feature `plugins`,
   and no config struct, `Browser` or `DriverMode` is `#[non_exhaustive]`; `src/api` has no
   `#[non_exhaustive]` at all.
8. **Two paths per item and one crate for everything (F12):** about 86 root re-exports, 203
   `pub mod`, a Python translator and the library half of the CLI inside the library.
9. **`BaseCase` and `BrowserSession` have no `Debug` (F2)**, no doc comment on `BaseCase` (F3), and
   `Send + Sync` is pinned for 3 types of the public handles (F14).
10. **The logging plugins (F20):** unit structs with a receiver-less `upload_file` that reads its
    credentials from the environment, and an error enum of `String`s.


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
  the 4,357 lines with `.unwrap()`, 632 with `.expect(` and 164 with `panic!(` in `src/` (a
  literal search in the second pass; the first pass counted 3,836, 532 and 130 with another scope,
  so read them as an order of magnitude) cannot be split into production and test by a text
  search.
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
  have no derive and no `impl Debug` (there is no `Debug for BaseCase` or `Debug for BrowserSession`
  anywhere in `src`). 26 hand-written `impl Debug` exist in `api`, `browser`, `testing`, `core`,
  `plugins`, `config`, `common`, `evasion` and `storage`: `MasterQA` (`src/api/master_qa.rs:462`),
  `Sb` (`src/testing/fixture.rs:157`), `Element` (`src/browser/element.rs:77`), `BrowserConfig`
  (`src/browser/config.rs:265`), and the ones that keep a credential out of the output
  (`ProxyAddress`, `PacUrl`, `VaultKey`).
* `Browser` and `DriverMode` (`src/browser/config.rs:44,153`) derive `Clone, Copy, Debug, Default,
  Deserialize, Eq, PartialEq, Serialize` and no `Hash`, so neither can key a `HashMap`; `ChartType`
  (`src/api/chart.rs:107`) is the same, while `JqcTheme` (`src/api/jqc.rs:99`) has `Hash`. No lint
  reports a missing `Hash`, `Eq` or `Clone`, so lane 2 reads the public `Copy` and `Eq` types once.
* Fix: a hand-written `Debug` that prints the test name, the engine and the mode and ends with
  `finish_non_exhaustive()`, as `MasterQA` does. The exact list of other gaps comes from
  `missing_debug_implementations` once lane 1 turns it on for the whole crate. Derive `Hash` on the
  `Copy` enums above.
* Breaking: no.

### F3. Documentation gaps (C-EXAMPLE, C-FAILURE, M-CANONICAL-DOCS, M-MODULE-DOCS, M-DOC-INLINE)

* `BaseCase` has no doc comment at all (`src/api/base_case.rs:58-60`: a constant, a blank line,
  `pub struct BaseCase {`), confirmed by reading. Its fields are private, so the struct needs one
  summary line and no more.
* The variants `Chrome`, `Chromium`, `Edge`, `Firefox` of `Browser` have no doc
  (`src/browser/config.rs:47-50`), the rest have.
* `src/common/mod.rs`, `src/config/mod.rs`, `src/utils/mod.rs` and `src/utilities/mod.rs` have no
  `//!`, and `src/utilities/mod.rs` has no doc on any of its six `pub mod` lines.
* Public items read in the second pass without a doc: `S3LoggingPlugin::new`
  (`src/plugins/s3_logging_plugin.rs:40`) and the six variants of `S3UploadError` (`:16-29`), the
  three fields of `IdeCommand` (`src/utilities/selenium_ide.rs:8-12`), the fields of `ProfileParams`
  and the struct `Parameters` (`src/profile_payloads/profile.rs:14-53`). `# Errors` is also missing
  on `parse_ide_file` and `parse_ide_html` (`src/utilities/selenium_ide.rs:15,33`).
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
* Fix: a doc for `BaseCase` and the four variants, module docs for the four roots, then the lint
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
* Four public functions return `Result<_, String>`: `find_examples`
  (`src/cli/scripts/all_scripts.rs:155`), `SbRecorder::check` and `check_in`
  (`src/cli/scripts/sb_recorder.rs:103,112`) and `validate_file_name` (`:151`), whose docs say
  "the message Python prints". `String` is not an `Error`, so `?` into `SeleniumBaseError` or
  `Box<dyn Error>` needs a `map_err`. A search for `, String> {` found 65 lines in `src`, nearly all
  `Result<_, String>` (two are a `BTreeMap<String, String>`) and most of them private parsers
  (`api/recorder`, `utilities/importer`, `config/plugins`); five lines past the first 60 were not
  listed, so four is a lower bound for the public ones.
* `utilities::selenium_ide::parse_ide_file` and `parse_ide_html`
  (`src/utilities/selenium_ide.rs:15-17,33`) return `Box<dyn std::error::Error>`, which is not
  `Send + Sync`, so a caller on a spawned task cannot `?` it into an error that is.
  `parse_ide_html` cannot fail (it ends in `Ok(commands)`, `:53`).
* The logging plugins have their own enum of `String` payloads: `S3UploadError`
  (`src/plugins/s3_logging_plugin.rs:16-29`; `Bucket` and `Upload` hold `error.to_string()`,
  `:80,85`). It is not `#[non_exhaustive]`, so a new variant breaks every `match` (the Azure and
  GCP enums were not re-read).
* `{:?}` is hand-written and prints the message plus the hint (`:209-228`); that is documented at
  `:15-17` and kept.
* Fix, small: keep the source in `sb_context` (a new `Context` variant or a boxed source), and
  keep the source in the two `From` impls with `#[source]`; give the six functions above a typed
  error (decision D9); let the plugin errors keep their source and take `#[non_exhaustive]`
  (lane 7). Breaking: low.
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
* Four `const fn name(self) -> &'static str` (`Browser`, `DriverMode`,
  `src/browser/chrome_for_testing.rs:84`, `src/browser/options/proxy.rs:120`) should be `as_str`,
  like the 13 other `as_str`.
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
  `src/testing/bdd/mod.rs:207,213`). The sealing pattern is already in the repository:
  `PlaywrightTarget: sealed::Sealed`, with the `sealed` module private
  (`crates/playwright/src/target.rs:36-61`).
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
  other items are. `src/api` has none: the enums `ChartType`, `JqcTheme` and `InspectionSeverity`
  and the records `ChartPoint`, `ChartSeries` and `InspectionIssue` have public fields or
  variants and are exhaustive, as are `IdeCommand` (`src/utilities/selenium_ide.rs:8`) and
  `ProfileParams` with its siblings (`src/profile_payloads/profile.rs`). `storage` does it right:
  `RunSummary` and `FlakyTest` are `#[non_exhaustive]` (`src/storage/results.rs:144,178`).
* Fix: `#[non_exhaustive]` on the config structs, the two enums and the `api`, `utilities` and
  `profile_payloads` types listed above; the `plugins` field becomes
  unconditional (a zero-size `BrowserPlugins` when the feature is off).
* Breaking: yes.

### F9. Strings, booleans and paths where a type fits (C-NEWTYPE, C-CUSTOM-TYPE, C-GENERIC)

* Selectors: 270 `css: &str` and 101 `selector: &str` parameters, although `Selector` and
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
* Keywords: `ProfileParams` keeps `browser_type`, `os_type` and `automation` as `String`s
  (`src/profile_payloads/profile.rs:14-39`), and `ProfileFlags` (`:60`) holds `webrtc_masking`,
  `proxy_masking`, `audio_masking` and others as keyword strings (`"mask"`, `"natural"`,
  `"auto"`, or a literal such as `"1920x1080x24"`, per its doc). The shape mirrors an external
  tool's JSON, which justifies strings on the wire; the typed settings exist (the conversion
  `to_fingerprint` makes them), so a bad keyword is found at the conversion, not at the parse.
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
* Feature `error-backtrace` (`:183`) is empty and nothing reads it. `docs/DEVELOPER_GUIDE.md:493`,
  `docs/CONTRIBUTING.md:518`, `docs/ABI_API.md:181` and `docs/contributing/debugging.md:28` say so;
  `README.md:515`, `CONTRIBUTING.md:300` and `docs/building.md:585` say it captures backtraces
  ("a backtrace on every error"). A feature that does nothing and is documented as doing
  something.
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
  every call and fixes the futures' bounds; native `async fn` in traits drops the boxing but the
  traits stop being dyn compatible (a method with an `async fn` is not dispatchable from a trait
  object: Rust Reference, "Dyn Compatibility", read on Context7). Decision D5. The repository has
  the native shape in one place already: the sealed `Sealed::attachment` of
  `crates/playwright/src/target.rs:46` returns `impl Future<Output = ..> + Send`.
* Receivers disagree for the same kind of call: `refresh(&self)` and `get_title(&mut self)`
  (`:45,54`), `get_url(&mut self)` (`:57`), `get_text(&mut self, ..)` (`:73`) read state and take
  `&mut self`.
* Parameter order: `type_text(css, text)` (`:70`) but `assert_text(expected, css)` (`:91`); the
  doc says Python does so, but one trait cannot teach two orders.
* Fix: `&self` for reads; settle one order for the Rust names.
* Breaking: yes.

### F17. Printing, statics, global effects (M-LOG-NOT-PRINT, M-AVOID-STATICS)

* `eprintln!` in library code: `src/api/master_qa.rs:431`, `src/browser/driver_access.rs:288`,
  `src/mcp/http.rs:842`. Use `tracing`. The three `eprintln!` in `src/cli/scripts/rich_helper.rs`
  (`:180,188,196`) are the `print_notice`, `print_warning` and `print_error` functions, and the five
  `println!` in `BaseCase` (`base_case_impl_links.rs:218,221`, `base_case_impl_parity.rs:281`,
  `base_case_impl_charts.rs:501`, `base_case_impl_tours.rs:459`) are documented behaviour (a method
  that prints, Python's `post_message` fallback, a prompt to close a window): none is a finding.
* 20 `OnceLock`, 1 `LazyLock` and a `static Mutex` registry keyed by thread (`ACTIVE_TEST_LOGS`,
  `src/artifacts.rs:89`); the registry `CLEANUPS` (`src/common/shutdown.rs:308`) and the
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

* 15 `#[allow(` in `src` against 25 `#[expect(`, and no crate-level `#![allow(` in the library.
  Ten are one-line with no reason: `src/api/recorder/cdp.rs:409`, `src/browser/driver_access.rs:199`,
  `src/config/test_options.rs:487`, `src/evasion/behavior.rs:130,392,403,631,645`,
  `src/evasion/humanize.rs:107`, `src/utils/translate/master_dict.rs:80`; five more open a multi-line attribute (`src/mcp/export.rs:307`,
  `failure.rs:390`, `journal.rs:228`, `src/sb_cdp/session_store.rs:129,162`) whose reasons were not
  read.
* Fix: `#[expect(.., reason = "..")]` for each, in the same lane as the module's pedantic pass.
* Breaking: no.

### F20. Plugins and path helpers take their inputs from the environment (C-METHOD, M-MOCKABLE-SYSCALLS, M-DI-HIERARCHY)

* `S3LoggingPlugin`, `AzureLoggingPlugin` and `GcpLoggingPlugin` (`src/plugins/s3_logging_plugin.rs:37`,
  `azure_logging_plugin.rs:41`, `gcp_logging_plugin.rs:40`) are unit structs. `new()` returns the
  empty value and the one real function, `upload_file`, takes no `self` (`s3:56`, `azure:58`,
  `gcp:50`). The S3 doc says "It has no hooks of its own", so the type is a namespace, with
  `Default` and `new` to look like a plugin.
* `upload_file` reads its credentials from the environment on every call (`AWS_ACCESS_KEY_ID`,
  `AWS_SECRET_ACCESS_KEY` and `AWS_ENDPOINT_URL` at `s3:62,64,72`; `AZURE_BLOB_URL` at `azure:59`;
  `GCS_ACCESS_TOKEN` at `gcp:55`). A caller cannot pass a credential, two uploads in one process
  cannot use two accounts, and a test can only fake one with `set_var`, which is `unsafe` in
  edition 2024 (the only `unsafe` in `src` is such a test, `src/api/master_qa/wrapper_tests.rs:36,48`).
  The settings code shows the right shape: `Settings::load_with(path, lookup)` and
  `from_settings_with(settings, lookup)` take the lookup as an argument
  (`src/config/settings.rs:455,466`, `src/core/test_run.rs:178`).
* The same shape in other public functions: `artifacts::logs_root` (`src/artifacts.rs:79`,
  `SB_REPORT_ROOT`; documented), `evasion::find_system_chrome` (`src/evasion/patcher.rs:403`,
  `CHROME_BIN` and `CHROMIUM_BIN`; documented, and re-exported at the crate root) and four
  cookie-file sites that resolve a name against `std::env::current_dir()`
  (`src/api/base_case.rs:581,641`, `src/api/base_case_impls/base_case_impl_parity.rs:411,423`).
* Fix: an upload target struct per store (bucket, region, endpoint, credential in a type that does
  not print), `fn upload(&self, path, key)`, and one named constructor `from_env()` so today's
  behaviour stays one call; a `logs_root_with(lookup)` and a `find_system_chrome_with(lookup)` like
  the settings code; an optional directory for the cookie-file methods.
* Breaking: yes for the three plugins (no known user); no for the rest if the old functions stay.

### F21. `Drop` that blocks or does I/O (C-DTOR-BLOCK, C-DTOR-FAIL)

* `DriverProcess::drop` calls `kill()`, which calls `child.kill()` and then `child.wait()`
  (`src/browser/launcher.rs:50-51,55-59`); `XvfbProcess::drop` calls `stop()`, which does the same
  (`src/browser/virtual_display.rs:141-151`). A `kill` followed by a `wait` returns quickly, but
  it is a blocking system call, and `BrowserSession::drop` runs `process.kill()` inside the task
  it spawns (`src/browser/session/lifecycle.rs:188-190`), that is on a Tokio worker.
* `TestRecorder::drop` (`src/core/test_run.rs:1416-1437`) writes the failure info and records the
  result in the run's reports when a test ends without finishing, which is file I/O in a
  destructor, and during a panic. Failures are logged with `tracing::warn!` and not lost, so
  C-DTOR-FAIL holds.
* Met: `BrowserSession::drop` (`lifecycle.rs:158-193`) gives the browser to
  `common::shutdown::spawn_cleanup` because "`drop` cannot await", and says so in its doc; the
  first pass found the same in `sb_cdp/browser.rs:262`. Of the 21 `impl Drop` in `src`, five were
  read (the four above and that one) and 16 were not; one of the 21 is in a test.
* Fix: say in the docs of `DriverProcess`, `XvfbProcess` and `TestRecorder` that dropping waits or
  writes; run the `wait` of `BrowserSession::drop` on `tokio::task::spawn_blocking`; let
  `TestRecorder` do its writes in an explicit `finish` (it has the `finished` flag) and leave
  `drop` to log. Check the other 16 with the lane.
* Breaking: no.

## In progress elsewhere

Not judged here: driver errors (`SeleniumBaseError::WebDriver`, `src/error.rs:76`), network
conditions (`#![expect(deprecated)]` for `NetworkConditions`, `src/api/base_case.rs:1-6`), the
DevTools client (`evasion::cdp::CdpClient`, `BrowserSession::cdp*`), capability types
(`browser::options::capabilities`), and the `drivers` module (`src/drivers.rs`). Findings that
depend on them: F4 large, F10, and M-DONT-LEAK-TYPES. `Element::as_webdriver_element` is an
escape hatch by design and counts as M-ESCAPE-HATCHES met.

## Fix lanes

In this order; each is one concern, with the files it mostly touches. None is started. Every
lane ends with the repository's gate (`cargo fmt --check`, clippy with `-D warnings`, the tests,
`cargo doc` with `-D warnings`); a breaking lane also updates the `tests/contract_*.rs` files in
the same commit and runs `mise run contracts` and `mise run semver`. The order puts first the
lanes that cost little and tell the rest (1 to 4), then the lanes that break little or nothing
(5 to 8), then the breaking lanes from the smaller to the larger (9 to 19), and the history
clean-up last.

1. **The Microsoft lint set for the whole workspace through `[workspace.lints]`** (F1, F19).
   Files: `Cargo.toml`, `crates/cli/Cargo.toml`, `crates/playwright/Cargo.toml`,
   `crates/own-imports/Cargo.toml`, `src/lib.rs`, `clippy.toml`. Not breaking. Output: the exact
   lists for lanes 2 to 4 and 8 (`missing_debug_implementations`, `missing_errors_doc`,
   `missing_panics_doc`, `unwrap_used`). Delete the five attribute blocks and the misplaced
   comment in `src/lib.rs` in the same commit; put `#[expect(clippy::pedantic, reason = "lane N")]`
   on each module that is not yet clean, so a later lane finds its line by the
   `unfulfilled_lint_expectations` warning.
2. **`Debug`, `Hash` and `Send + Sync` pins** (F2, F14). Files: `src/api/base_case.rs`,
   `src/browser/session/mod.rs`, `src/browser/config.rs`, `src/api/chart.rs`, whatever the lint
   lists, one new test file. Not breaking.
3. **Doc gaps** (F3, and the doc half of F21). Files: `src/api/base_case.rs`,
   `src/browser/config.rs`, `src/common/mod.rs`, `src/config/mod.rs`, `src/utils/mod.rs`,
   `src/utilities/mod.rs`, `src/lib.rs`, `src/macros.rs` (fences), `src/plugins/`,
   `src/utilities/selenium_ide.rs`, `src/profile_payloads/profile.rs`, `src/browser/launcher.rs`,
   `src/browser/virtual_display.rs`. Not breaking.
4. **Manifest and tooling** (F13, F1 rustfmt): metadata, `[workspace.dependencies]`,
   `error-backtrace` and the three pages that describe it wrongly, after decisions D1 and D7.
   Files: the four manifests, `rustfmt.toml`, `README.md`, `CONTRIBUTING.md`, `docs/building.md`.
   Breaking only for the feature.
5. **Keep the source in errors** (F4 small): `sb_context`, the two `From` impls, `#[non_exhaustive]`
   on the plugin errors, and typed errors for `find_examples`, `SbRecorder::check`, `check_in`,
   `validate_file_name`, `parse_ide_file` and `parse_ide_html` after decision D9. Files:
   `src/error.rs` and its callers, `src/cli/scripts/all_scripts.rs`, `src/cli/scripts/sb_recorder.rs`,
   `src/utilities/selenium_ide.rs`. Low breaking.
6. **Print, glob, small hygiene** (F17, F12 glob). Files: `src/api/master_qa.rs`,
   `src/browser/driver_access.rs`, `src/mcp/http.rs`, `src/profile_payloads/mod.rs`. Not breaking.
7. **The logging plugins and the path helpers take their inputs** (F20): an upload target per store
   with `from_env()`, `logs_root_with`, `find_system_chrome_with`, a directory for the cookie-file
   methods. Files: `src/plugins/s3_logging_plugin.rs`, `azure_logging_plugin.rs`,
   `gcp_logging_plugin.rs`, `src/artifacts.rs`, `src/evasion/patcher.rs`,
   `src/api/base_case.rs`, `src/api/base_case_impls/base_case_impl_parity.rs`. Breaking only for
   the three plugin types.
8. **`Drop` that blocks or writes** (F21, the code half): `spawn_blocking` for the `wait`, an
   explicit `finish` for `TestRecorder`, and the check of the 16 `impl Drop` not read. Files:
   `src/browser/launcher.rs`, `src/browser/virtual_display.rs`,
   `src/browser/session/lifecycle.rs`, `src/core/test_run.rs`. Not breaking.
9. **Sealed traits and no `Deref`** (F6). Files: `src/api/traits.rs`, `src/error.rs`,
   `src/api/membership.rs`, `src/api/master_qa.rs`, `src/testing/bdd/mod.rs`, their tests. Breaking.
10. **`non_exhaustive` and the unconditional `plugins` field** (F8). Files:
    `src/browser/config.rs`, `src/cookie.rs`, `src/config/settings.rs`, the `api`, `utilities` and
    `profile_payloads` types of F8, and every struct literal (use `constructors` to list them
    first). Breaking.
11. **Booleans to enums, paths to `AsRef<Path>`, keywords to enums** (F9 without selectors). Files:
    the sites in F9. Breaking.
12. **Macros** (F7), after D3. Files: `src/macros.rs`, `src/lib.rs`, `tests/`, `examples/`,
    `docs/`. Breaking.
13. **Names** (F11, F5 cheap part, F15): `BrowserKind`, one `Proxy`, `connect` and `from_session`,
    `as_str` for the four `name`, `get_browser`. Files: `src/browser/config.rs`, `src/lib.rs`,
    `src/evasion/fingerprint.rs`, `src/browser/options/`, `src/config/proxy_list.rs`,
    `src/profile_payloads/profile.rs`, `src/api/base_case.rs`,
    `src/browser/chrome_for_testing.rs`. Breaking. Use `tokensave_rename`.
14. **Trait receivers and parameter order** (F16), with D5. Files: `src/api/traits.rs` and the
    `BaseCase` and `Page` impls. Breaking.
15. **Public module tree** (F12): `pub(crate)` pass, one path per item. Files: `src/lib.rs`,
    `mod.rs` files, the contract tests. Breaking; run `mise run semver` and the contract tests.
16. **Error redesign** (F4 large), after the thirtyfour driver-errors lane. Files: `src/error.rs`
    and every constructor call. Breaking, large.
17. **Selector typing** (F9 selectors): `BaseCase` on `impl Into<SelectorBuf>`. About 371
    signatures (public or not); do it with `ast_grep_rewrite`. Breaking, large.
18. **`get_` names** (F5), only if D2 says yes; with the translator tables
    (`src/utilities/importer/translate.rs`, `src/mcp/dev/api_index.json`) and `parity/api.toml`.
    Breaking, large.
19. **`panic` to `Result` for `driver()`** (F10), after the drivers lane. Files:
    `src/browser/session/mod.rs`. Breaking.
20. **Docs without history** (F18). Files: wherever the search lists. Not breaking.

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
* D9. Keep `String` as the error where the text is Python's message (`SbRecorder::check`,
  `validate_file_name`, `find_examples`), or give them an error type that displays the same text
  (lane 5)?

## What the second pass changed

The second pass re-read the code behind 35 claims of the first pass. No cargo command was run.

Stood as written (26):

1. The five lint blocks (`src/lib.rs:55-69,78-92,95-109,153-167,179-193`) and the misplaced
   comment (`:151`).
2. The 26 `pub use` of `src/lib.rs:201-246`, 27 of the names from `evasion::fingerprint`.
3. `Browser` (`:211`), `BrowserType` (`:223`) and `Key` (`:236`) at the crate root.
4. The three `Deref` impls (`src/api/master_qa.rs:446`, `src/testing/bdd/mod.rs:203,326`).
5. No `Debug` for `BaseCase` and `BrowserSession`.
6. No doc on `BaseCase` (`src/api/base_case.rs:58-60`).
7. The four undocumented `Browser` variants (`src/browser/config.rs:47-50`).
8. `BrowserSession::driver`: the first doc line, the panic and the `webdriver()` sibling
   (`src/browser/session/mod.rs:85-116`).
9. 35 `pub fn get_` and 69 `pub async fn get_`.
10. `BaseCase::get_browser` returns a clone of the `BrowserConfig`.
11. `ResultExt::sb_context` formats into `InvalidConfig` (`src/error.rs:1054-1067`).
12. The empty `error-backtrace` feature (`Cargo.toml:183`).
13. The three `eprintln!` (`master_qa.rs:431`, `driver_access.rs:288`, `mcp/http.rs:842`).
14. 26 macros in `src/macros.rs` and `sb_test_options!` as the 27th.
15. `sb_open!` changes its type by arity (`src/macros.rs:91-95`) and `sb_test!` ends in `.expect`
    (`:80`).
16. The four traits: `#[async_trait]` at `src/api/traits.rs:36,61,80,98`, `&mut self` getters, the
    order of `type_text` and `assert_text`.
17. `BrowserConfig`: nine public fields, `plugins` behind a feature, no `#[non_exhaustive]`
    (`src/browser/config.rs:201-248`).
18. `Browser` and `DriverMode` are not `#[non_exhaustive]`.
19. The manifest: edition 2024, `rust-version` 1.99, MIT, the `[package]` fields, no `lints` table.
20. `unsafe` only in two test lines (`src/api/master_qa/wrapper_tests.rs:36,48`).
21. `pub use profile::*;` (`src/profile_payloads/mod.rs:55`).
22. The counts 1,910 `pub fn`, 936 `pub async fn`, 203 `pub mod`, 894 `# Errors`, 25 `#[expect(`,
    15 `#[allow(`.
23. `browser` exposes 15 `pub mod`, plumbing included (`src/browser/mod.rs`).
24. `src/api/mod.rs:9` says to prefer `crate::sb_cdp`.
25. The hand-written `Debug` of `BrowserConfig` keeps secrets out (`src/browser/config.rs:262-270`).
26. `FromStr` on `Browser` and `Display` on `DriverMode` (`src/browser/config.rs:122-143,178-180`).

Corrected (9):

1. "`doc_coverage` reports 8 undocumented public items": it reports 80 or more, and the two files
   read have a doc on each listed item. The count is dropped.
2. "30 other types have a hand-written `Debug`": 26 in the directories searched.
3. The counts of `.unwrap()`, `.expect(` and `panic!(`: 4,357, 632 and 164 in this scope, not
   3,836, 532 and 130.
4. 270 `css: &str`, not 269.
5. `#[allow(` without a reason: ten one-line, not nine (`src/utils/translate/master_dict.rs:80`).
6. M-MODULE-DOCS: four modules, not three (`src/config/mod.rs`).
7. C-CONV: four `name` methods, not two.
8. `error-backtrace`: three pages say it works, not two (`docs/building.md:585`); four say it does
   not.
9. F16: native `async fn` in traits drops the boxing but also the dyn compatibility; the first pass
   wrote "but not the object safety".

Moved from met to partly: C-METHOD (F20) and C-DTOR-BLOCK (F21). Added: `Hash` to C-COMMON-TRAITS,
the `Result<_, String>` and `Box<dyn Error>` signatures to C-GOOD-ERR, and the missing `//!` of
`config/mod.rs`.


## Not checked

* Anything a compiler lint reports: the full list of types without `Debug`, `Clone`, `Eq`, `Hash`;
  missing `# Errors` and `# Panics`; production `unwrap`, `expect`, `panic`; unused doc links.
* Whether each public type is `Send + Sync` beyond the pinned three.
* The 11 performance guidelines, the 3 application guidelines for `crates/cli`, M-UNSOUND,
  M-TAUTOLOGICAL-TESTS, M-LOG-STRUCTURED beyond one site, M-PUBLIC-DISPLAY, M-REGULAR-FN,
  M-SHORT-NAMES, C-STRUCT-BOUNDS.
* The item-level API of `evasion` outside `fingerprint`, `mcp`, `config` outside `Settings`,
  `utils`, `common`, `sb_cdp` beyond `Browser`, `Page` and `Cookie`, `crates/cli` (a binary
  package) and the features `gui`, `tui`, `ocr`, `visual`, `pdf`. `crates/own-imports` is
  tests only and has no public item.
* 16 of the 21 `impl Drop` in `src` (F21), and the Azure and GCP error enums.
* Whether `cargo-semver-checks` is clean on the current tree (not run).

## Sources

* [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
* [Pragmatic Rust Guidelines 2026.6](https://microsoft.github.io/rust-guidelines/) (all guidelines
  in one file: `agents/all.txt`)
* Cargo reference, workspaces and the `lints` table; the Rust Reference on lint attributes; the
  Clippy lint configuration book (all read on Context7).
