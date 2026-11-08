---
title: "Design: API rules audit (Rust API Guidelines, Pragmatic Rust, Google, Idiomatic Rust Snippets)"
sidebarTitle: "API guidelines audit"
---

Status: audit note, written before any fix, on branch `lane/api-audit` (from `404208aa`, 2026-11-06),
and extended on 2026-11-07 on `lane/api-audit-google` (from `2327ffb8`). The four sources the crate
follows, the Rust API Guidelines, Microsoft's Pragmatic Rust Guidelines, Google's Rust style
guide and Idiomatic Rust Snippets (`IRS:`, added 2026-11-08), are one set of rules here, [The rules we follow](#the-rules-we-follow); every finding and
every fix lane names the guides it comes from. The note is read-only on code: no cargo command was
run (no clippy, no rustdoc, no doctests), so every statement comes from reading the source through
the code index, from literal-text counts, and from the three guideline books. A `file:line` is from
`404208aa` (the second pass re-read the lines it cites) unless it belongs to the third pass, whose
lines are from `2327ffb8`. The five areas that the thirtyfour lanes are reworking are marked
[in progress elsewhere](#in-progress-elsewhere) and are not judged here. The owner's decisions of
2026-11-07 are in [Decisions for the owner](#decisions-for-the-owner): all twelve are taken.

Three passes made this note. The first walked the root, `api`, `browser`, `sb_cdp`, `error`,
`cookie`, the macros and the manifests. The second spot-checked the first pass against the code
(35 claims: 26 stood and 9 were corrected, listed in
[What the second pass changed](#what-the-second-pass-changed)), walked the modules the first pass
had only sampled (`storage`, `plugin_host`, `artifacts`, `plugins`, `profile_payloads`, `core`,
`testing`, `utilities`, `crates/playwright`), added two findings (F20 and F21) and two fix lanes,
and moved two checklist items from met to partly.

The third pass (2026-11-07) read Google's Rust style guide, searched the code again for each of its
rules, merged them with the other two guides into one set of rules, tagged every finding with the
guides it comes from, added three findings (F22 to F24), corrected two first-pass claims and put the
work the owner's decisions need into the fix lanes (see
[What the third pass changed](#what-the-third-pass-changed)).

## What was checked, and against what

* The [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html):
  54 items, fetched on 2026-11-06.
* The [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/) of Microsoft,
  version 2026.6 (book generated 2026-10-26): 90 guidelines, read from the single-file form at
  `agents/all.txt`. The lint set under M-STATIC-VERIFICATION is quoted from it below.
* [Google's Rust style guide](https://github.com/google/styleguide/blob/gh-pages/rustguide.md)
  (`rustguide.md` on `gh-pages`, fetched 2026-11-07): 61 rule sections, 6 of them for Chromium or
  Fuchsia only (small crates, flat file layout, the import macro, no `async`/`.await`, no `std`
  threads, `fuchsia_sync::Mutex`). It has no ids, so a rule is named by the title of its section
  (`G: getters and setters`). The sections on errors, getters and setters, standard traits,
  documentation and unsafe code were read on Context7 (`/google/styleguide`, through the connector
  because the plugin's quota had run out); the file was then fetched from the raw page and every
  section that applies to a library was read (the sections on iterator chains and on assertion
  messages only by their titles). Its own scope rule: for library code its API rules
  are binding, for application code (`crates/cli`) they are a strong recommendation.
* Cargo's `[workspace.lints]` and `[lints] workspace = true`, `#[expect]` on lint groups, and the
  `clippy.toml` keys `allow-unwrap-in-tests`, `allow-expect-in-tests` and `allow-panic-in-tests`
  were checked on Context7 (`/websites/doc_rust-lang_cargo`, `/websites/doc_rust-lang_stable_reference`,
  `/rust-lang/rust-clippy`). The Rust Style Guide was looked at only through `rustfmt.toml`.
  Effective Rust was not read item by item. For the third pass the `disallowed-methods` key of
  `clippy.toml` (fields `path`, `reason`, `replacement`) was checked on the same Context7 entry of
  Clippy.

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
  means only a compiler lint can count it, which is lane 1 and 2 below. `accepted` is a fifth
  status for a deviation the owner decided to keep (D1, D8, D11, D12; D2 and D10 were reversed on 2026-11-08).
* The third pass counts lines of `src/` without `crates/` and without `src/mcp/dev/api_index.json`
  (a path filter `src/` also matches `crates/*/src/`, so the exclusion is explicit). A count is a
  number of lines, not of items, and is an upper bound unless the text says it was read.

## The rules we follow

The three guides overlap on most rules and disagree on a few. The crate follows one set. Each rule
below appears once, with its ids in every guide it comes from (`C-` the Rust API Guidelines, `M-`
Pragmatic Rust, `G:` Google's guide, named by the title of its section), and the fourth column says
what the crate adopts where the guides differ, and why. From then on the adopted form is the rule: a
later lane does not choose between guides again.

Four principles settled the differences:

1. When two rules can both be met, the stricter one wins (glob re-exports, R21).
2. A rule that a lint checks wins over one a reviewer has to remember: each rule says its lint
   where there is one, and lane 1 turns it on.
3. Library code is bound by the API rules of all three guides; application code (`crates/cli`) takes
   them as strong recommendations. That is Google's own scope rule and the line Microsoft draws with
   its application guidelines.
4. A deviation needs its reason where it is made (`#[expect(.., reason = "..")]`) or here, as a
   decision. The owner's decisions of 2026-11-07 are the accepted deviations.

Status is the crate's status against the adopted rule, in the words used below, plus `accepted`.
The ids that no rule row uses are n/a or not checked and stay only in the per-id tables further
down: C-ITER-TY, C-NUM-FMT, C-RW-VALUE, C-BITFLAG, C-MACRO-VIS and C-STRUCT-BOUNDS,
M-TAUTOLOGICAL-TESTS and the performance and application guidelines.

Tally of the 44 rules: met 3, partly 18, accepted 2 (R16, R29; R3 and R5 were accepted until the
owner reversed D2 and D10 on 2026-11-08), not met 6, not checked 14 (R32 to R44 are new, see [The fourth source](#the-fourth-source)), n/a 1.

| # | Rule we follow | Sources | Where the guides differ: what we adopt | Status | Affected | Finding |
|---|---|---|---|---|---|---|
| R1 | Names follow the standard library style, in one word order, and say what the item is; short, but with no abbreviation that an outsider cannot read. | C-CASE, C-WORD-ORDER; M-WEASEL-WORDS, M-SHORT-NAMES; G: naming (methods, lifetimes, generic parameters), avoid abbreviations | M-SHORT-NAMES prefers abbreviations (`CallbackFn`); Google refuses those that delete letters inside a word or that people outside the project do not know. Adopt: at most two short words, no module name as a prefix, and an abbreviation only when it is common outside the project (`Fn`, `Ip`, `Css`), so `rec` and `ws` go. | partly | 6 `BrowserConfig` setters and 1 accessor, `uc`, `jqc`, the names of F11 | F11 |
| R2 | `as_` is a cheap borrow, `to_` costs, `into_` consumes; a conversion that names its type says so. | C-CONV, C-CONV-SPECIFIC; G: naming methods (`wrong_self_convention`) | Agree. | met | 4 `name` methods that should be `as_str` | F5 |
| R3 | A getter is `foo()` or `foo_mut()`, a setter `set_foo(T)`; `take_foo()` clears and returns the old value, `replace_foo(T)` sets and returns it; there is no `get_` prefix. | C-GETTER; M-RUST-SHAPED; G: getters and setters | Agree on the rule. D2 is reversed (2026-11-08): `BaseCase` getters drop `get_` (`get_text` becomes `text`); the Python names stay findable through `parity/api.toml`, the Python importer and `parity_lookup`. | not met (104, 78 on `BaseCase`) | 104 `get_`; 14 public `set_*`, 13 of them fit; `set_header` and `replace_body` consume `self` | F5, F15 |
| R4 | Constructors are `new`, `new_with_foo`, `try_new` when they return a `Result`, and `from_foo` beside a `From` impl; a constructor does what its name says; an options struct implements `Default`. | C-CTOR; G: naming initialization functions, pair `From` with constructors, options structs implement `Default`; IRS: associated function `new` | Agree. | partly | 10 `new` return a `Result` and `try_new` has 0; 5 constructors are named `with_*`; `BaseCase::new` starts a browser | F15 |
| R5 | A builder is a separate type whose setters are named after the field and take and return `self`; `build()` validates; `with_*` names a constructor only. | C-BUILDER; M-INIT-BUILDER, M-INIT-CASCADED, M-BUILD-RESULT; G: closure accessors; IRS: builder pattern (method chaining) | Adopt (D10 replaced, 2026-11-08): setters are named after the field, on a separate builder type (`BrowserConfig::builder().headless(true).build()`), as C-BUILDER and IRS's builder idiom do, so there is no conflict with IRS; `build()` still validates, which is the stricter rule where IRS lets it return unchecked. `with_*` stays only for constructors, as in std's `Vec::with_capacity`; Google's closure meaning of `with_foo(f)` is not used. | not met (the bon builders lane does it) | 153 public `with_*` (about 80 on `BrowserConfig`), 0 closure accessors | F15 |
| R6 | A parameter takes the widest borrowed or generic form: `&str`, `&[T]`, `&T`, `Option<&T>`, `impl AsRef<Path>`, and `impl Into<String>` where the call site gains; the same parameters come in the same order. | C-CALLER-CONTROL, C-GENERIC; M-IMPL-ASREF, M-IMPL-IO, M-IMPL-RANGEBOUNDS, M-PARAMETER-CONSISTENCY; G: flexible references, accepting `Into`, iterators vs slices in parameters; IRS: `&str` over `String` (string vs str anti-pattern), borrowing instead of cloning | C-GENERIC and M-IMPL-ASREF ask for generics; Google weighs `Into` against code size and says application code almost never takes it. Adopt: generics at the library boundary, each kept to the conversion and a call to a non-generic body; concrete types in `crates/cli`. | partly | 0 `&Vec<`, 0 public `&Option<`, 253 `impl Into<` lines, paths as `&str`, 2 methods with the order reversed (`type_text`, `assert_text`) | F9, F16 |
| R7 | A named type, not a `String` or a `bool`, where the value has a meaning; named fields, not tuples; a newtype, not an alias, when the type needs an API of its own. | C-NEWTYPE, C-CUSTOM-TYPE, C-NEWTYPE-HIDE; M-STRONG-TYPES; G: enum over bool, named fields over tuples, newtypes vs type aliases; IRS: newtype pattern (`UserId` and `AccountId` over `u64`, `From` between wrappers), stringly typed anti-pattern (F9) | Agree. | partly | 371 selector parameters, 9 `bool` sites, 1 tuple alias | F9 |
| R8 | A function returns an iterator unless it builds the collection or callers need one; a collection type implements the collection traits. | C-ITER, C-COLLECT; M-COLLECTION-TRAITS; G: iterators vs owned collections in return types; IRS: iterator adapters, `iter`/`iter_mut`/`into_iter` | Agree. | met in the files read, not counted | The functions read build a `Vec` from a browser answer, which is the guide's own exception | none |
| R9 | A public type derives what it can: `Debug`, `Clone`, `Default`, `Eq` and `Hash` if it may be a map key, serde where it is stored; handles are `Send` and `Sync`. | C-COMMON-TRAITS, C-DEBUG, C-DEBUG-NONEMPTY, C-SEND-SYNC, C-SERDE; M-PUBLIC-DEBUG, M-PUBLIC-DISPLAY, M-TYPES-SEND; G: important traits, `#[derive]` over manual impls | Agree. | partly | `BaseCase` and `BrowserSession` without `Debug`; 3 `Copy` enums without `Hash`; 3 types pinned `Send` and `Sync` | F2, F14 |
| R10 | The standard trait, not a look-alike method (`FromStr`, `From`, `Display`, `Default`, `IntoIterator`); an operator only for its own meaning. | C-CONV-TRAITS, C-OVERLOAD; G: known standard library traits (`should_implement_trait`), no overloading for non-mathematical purposes | Google asks for the trait and refuses an operator used as sugar, and the two meet in `not()`. Adopt: the method, with the reason written in `#[expect]`. | met | 31 `fn from_str(` lines (the 15 read are `FromStr` impls) and no `pub fn from_str(`; 2 recorded exceptions, `not()` at `src/sb_cdp/expect.rs:92,316` | none |
| R11 | A trait exists for generic programming, for an `impl` or `dyn` parameter, or as an extension (its name ends in `Ext`); a trait nobody should implement is sealed; no `Deref` for inheritance; a library takes generics, not `dyn`. | C-SEALED, C-DEREF, C-OBJECT; M-DI-HIERARCHY, M-ESSENTIAL-FN-INHERENT; G: trait vs inherent impl, extension traits, generics vs `dyn`; IRS: builder by method chaining over boxed trait-object decorators; IRS: extension traits (`StringExt`, a trait the user must import), enum polymorphism, visitor as enum and `match`, decorator as method chaining, deref abuse anti-pattern (F6) | C-OBJECT keeps a trait object safe where that helps; Google prefers generics. Adopt: `dyn` only for seams such as `CdpTransport`. | not met | 6 open traits, 3 `Deref` impls | F6 |
| R12 | Trait methods are native `async fn` unless the trait is used as `dyn`. | M-ASYNC-FN; G: generics vs `dyn` | Adopt D5; the dyn compatibility of C-OBJECT is given up where nothing uses `dyn`. | partly | 84 `#[async_trait` sites | F16 |
| R13 | A public type can grow: private fields or `#[non_exhaustive]`; fields checked where they are set; features only add; `const fn` only if it stays const; `#[doc(hidden)]` is private. | C-STRUCT-PRIVATE, C-VALIDATE, C-HIDDEN; M-FEATURES-ADDITIVE, M-STRONG-TYPES-GUARD; G: `#[non_exhaustive]` on enums and on structs, `const fn`, `doc(hidden)` | C-STRUCT-PRIVATE wants private fields; Google lets a struct keep public fields when callers build it with `..Default::default()`. Adopt: structs built that way (`Settings`, `BrowserSettings`, `ProxyConfig`) stay exhaustive with `Default`; structs built by `with_*` (`BrowserConfig`, `Cookie`) take `#[non_exhaustive]`; an enum that may grow takes it. | partly | `BrowserConfig` has 9 public fields and one behind a feature; 70 `#[non_exhaustive]` for 547 structs and enums; 85 `pub const fn` | F8, F13 |
| R14 | A failure the caller can recover from is a typed `Result` with a source; `Option` where the cause is obvious; never `Result<_, String>`; one canonical error. | C-GOOD-ERR; M-ERRORS-CANONICAL-STRUCTS, M-FROM-ERROR; G: reporting errors (`Option`, `Result`, `panic!`), `Copy` error types; IRS: custom error type with `Display`, `Error` and `From` | M-ERRORS-CANONICAL-STRUCTS wants a struct with `kind()`; C-GOOD-ERR and Google accept an enum. Adopt the struct (D4). | partly | 31 of 34 variants keep no source; 4 `Result<_, String>`; 2 `Box<dyn Error>` | F4 |
| R15 | A panic reports a bug or a broken invariant, never an expected state; where one is needed it is `expect` with the reason, not `unwrap`, and it is documented. | M-PANIC-ON-BUG, M-PANIC-MESSAGE; G: reporting errors, document panic cases, `expect` over `unwrap`, pattern matching over check-then-unwrap; IRS: unwrap abuse, panic in libraries anti-patterns (F22) | `CLAUDE.md` says `unwrap()` only in tests; Google prefers `expect` where a panic is needed. Adopt both: `unwrap_used` and `panic` warn in production code, `expect_used` stays off, `missing_panics_doc` is on. | partly | 4,357 `.unwrap()`, 632 `.expect(` and 164 `panic!(` lines, tests included; 4 `# Panics` | F10, F7, F1 |
| R16 | A panic is not caught and a poisoned lock is not recovered, except where the reason is written. | M-PANIC-CONTINUATION, M-PANIC-IS-STOP; G: don't call `catch_unwind`, handling poison | The three agree. Adopt D11 (three production `catch_unwind`, no more) and D12 (one helper for poison). | accepted, narrowly | 3 production `catch_unwind`, 34 `PoisonError::into_inner` | F22 |
| R17 | No `unsafe` unless there is no safe way; each block has a `// SAFETY:` comment. | M-UNSAFE, M-UNSAFE-IMPLIES-UB, M-UNSOUND; G: unsafe code; IRS: unnecessary unsafe anti-pattern (F24) | Agree. | partly: none in the library, 10 of 11 test blocks lack the comment | 11 blocks, all in test code | F24 |
| R18 | Every public item has a doc that says what the code does not; a module has `//!`; a function that fails or panics says so; examples cover each type and each family of methods. | C-CRATE-DOC, C-EXAMPLE, C-FAILURE, C-LINK, C-QUESTION-MARK; M-CANONICAL-DOCS, M-FIRST-DOC-SENTENCE, M-MODULE-DOCS, M-DOC-INLINE, M-DOCUMENTED-MAGIC, M-DESIGN-FOR-AI; G: when and how to write documentation | C-EXAMPLE wants an example on every item; Google wants enough of them "somewhere" to show how the crate works. Adopt Google's, because `BaseCase` has about 700 methods; `# Errors` stays on every fallible function (M-CANONICAL-DOCS); a variant whose name says it may go undocumented (Google). | partly | 280 `# Examples` for about 2,846 `pub` functions; 894 `# Errors` against 2,580 `Result` returns; 4 modules without `//!`; no `missing_docs` | F3 |
| R19 | Docs describe the code, not its history or its Python origin, except where the parity table needs it. | M-NO-META-DESIGN-DOCUMENTATION | One guide. | partly | 681 lines | F18 |
| R20 | A macro only where a function cannot do the job; no macro changes its result type by its arity; documentation is written at the item, not generated. | C-EVOCATIVE, C-MACRO-ATTR, C-ANYWHERE, C-MACRO-TY; M-MACRO-LAST-RESORT, M-MACROS-DONT-LIE, M-EXAMPLE-OVER-PROC, M-MACRO-MAIN-CRATE; G: macros, document in a straightforward manner | Agree. D3 cuts the `sb_*` macros to `sb_test!`. | not met, fix decided | 27 exported and 16 internal `macro_rules!` | F7 |
| R21 | One path per item; no glob imports, except `use super::*` in a test module; no prelude; a non-leaf module only declares and re-exports; a file is named after its main item. | M-SINGLE-ITEM-PATH, M-NO-GLOB-REEXPORTS, M-NO-PRELUDE, M-BALANCED-MODULES; G: glob imports, non-leaf modules, name files after the primary item | Google allows `pub use submodule::*` in a non-leaf module; M-NO-GLOB-REEXPORTS does not. Adopt the stricter. | not met | about 86 root re-exports with a second path; 1 glob re-export; 13 types defined in 7 `mod.rs` files | F12 |
| R22 | A crate holds one concern; the crates sit flat in the workspace folder. | M-SMALLER-CRATES, M-CRATES-FLAT-FOLDER, M-CRATES-IN-WORKSPACE; G: prefer small crates, flat file layout (both Chromium only) | Google's two rules are Chromium's and allow a large foundational library. Adopt Microsoft's split, designed in `docs/design/core_split.md` (D6). | not met, decided | 1 library with two engines, the evasion layer, the MCP servers, a translator and the CLI half | F12 |
| R23 | Complete metadata; the latest edition; one workspace manifest; no unstable feature. | C-METADATA, C-STABLE, C-PERMISSIVE, C-RELNOTES, C-FEATURE; M-CARGO-WORKSPACE, M-LATEST-EDITION, M-MSRV, M-OOBE; G: no unstable features | M-MSRV asks for a few versions behind; D1 keeps the newest stable. | partly; M-MSRV accepted | 5 missing `[package]` fields; the empty feature `error-backtrace`; 0 `#![feature(` | F13 |
| R24 | One lint set for the workspace; a deviation is `#[expect]` with a reason; the formatter decides the layout. | M-STATIC-VERIFICATION, M-LINT-OVERRIDE-EXPECT; G: formatting tool, opting out of rustfmt, the Clippy lints it names | Agree. | partly | 5 module trees under the set, of about 27; 15 `#[allow(`; `style_edition = "2021"` (D7: 2024 later) | F1, F19 |
| R25 | The library does not print, keeps global state small, runs nothing before `main`, and logs in a structured way. | M-LOG-NOT-PRINT, M-LOG-STRUCTURED, M-AVOID-STATICS; G: do not run code before `main()`; IRS: lazy initialization and singleton with `OnceLock`, no `unsafe` | Agree. | partly | 3 `eprintln!`; 21 statics; 0 `#[ctor` | F17 |
| R26 | Inputs come from arguments, not from the environment, so the code can be tested; a function with a clear receiver is a method. | C-METHOD; M-MOCKABLE-SYSCALLS, M-TEST-UTIL, M-INTEGRATION-TESTS | Agree. | partly | 9 sites | F20 |
| R27 | `Drop` neither blocks nor fails silently; cleanup that needs `await` goes to a task. | C-DTOR-FAIL, C-DTOR-BLOCK; IRS: RAII, lock guards | One guide, plus IRS's RAII. | partly | 3 types | F21 |
| R28 | No `Arc`, `Rc` or `Box` in a public signature; a service is a cheap `Clone` handle; borrow instead of cloning. | C-SMART-PTR, C-INTERMEDIATE; M-AVOID-WRAPPERS, M-SERVICES-CLONE, M-SIMPLE-ABSTRACTIONS; G: avoid `Rc` and `Arc`, prefer borrows over deep copies; IRS: excessive cloning anti-pattern, observer by channel, unnecessary smart pointers anti-pattern (F23), flyweight (an `Arc` cache kept inside, never in a signature), mediator (its `Arc<Mutex<..>>` loses to this rule) | Google lets `Arc` stand where borrowing is "unreasonably complex"; Microsoft keeps it out of the public API. Adopt: `Arc` inside is a judgment call, never in a signature. | partly | 2 constructors, 5 closure aliases and 3 future aliases; 168 `Arc<` lines inside | F23 |
| R29 | Foreign types stay out of the API, with an escape hatch where the user needs one. | M-DONT-LEAK-TYPES, M-ESCAPE-HATCHES, M-FOREIGN-REEXPORTS; IRS: adapter, facade, proxy | One guide, plus IRS's wrappers that keep the wrapped type private. | accepted (D8) for the four re-exports; thirtyfour is in progress elsewhere | 4 re-exports | none |
| R30 | Implementation habits: a type written where inference hides it, pattern matching over check-then-unwrap, every variant of a `non_exhaustive` enum listed, a message on an assertion, a loop where a chain reads worse, `format!` over `+` and over `str::replace` for templates. | G: implementation concerns; IRS: poor pattern matching, iterator and manual-indexing anti-patterns (see R41) | One guide, plus IRS. | not checked item by item | 3 `for_each`; 42 `.replace("` lines, 8 read (runtime text, not templates) | none |
| R31 | Not for this crate: FFI, `-sys` crates, the Chromium, Android and Fuchsia rules, the 11 performance guidelines and the 3 application guidelines (for `crates/cli`). | M-FFI-NAMING, M-FFI-TRANSLATES, M-ISOLATE-DLL-STATE, M-SYS-CRATES, M-UPSTREAM-GUIDELINES, M-MACRO-HELPERS, M-MACRO-VERSION-PIN, M-PROC-IMPL, M-PROC-IMPLIED-ITEMS; G: small crates, flat layout, import macro, no `async`/`.await`, no `std` threads, `fuchsia_sync::Mutex`, test framework per codebase | None. | n/a | none | none |
| R32 | A trait whose implementor picks one related type uses an associated type, not an extra type parameter on every user; a parameter only where one type may implement the trait several times. | IRS: associated types | New in IRS; the three guides name no rule. Adopt for the 15 `pub trait`s and any new one; it fits C-SEALED and R11. IRS also shows a default associated type (`type Output = String;`); that is an unstable feature, so R23 wins and it is not adopted. | not checked | the 15 `pub trait`s | none |
| R33 | A library function returns `Result` and propagates with `?` through `From` impls; it never calls `process::exit` and never prints the error to end the program; that choice belongs to `crates/cli`. | IRS: `?` propagation, unwrap abuse, panic in libraries (graceful `exit(1)` in `main` only; F22) | R14 and R15 cover the error type and the panic; IRS adds the exit. Adopt: no `process::exit` in `src/`; where IRS maps errors to `String` with `map_err(format!)`, R14 wins (never `Result<_, String>`). | not checked | `process::exit` in `src/` not counted | none |
| R34 | A builder with required fields, and a handle with a strict lifecycle, encode the state in a type parameter (typestate, `PhantomData` markers) so that `build()` or an invalid call does not compile; a transition consumes `self`. | IRS: type state pattern, builder with type states | New in IRS. Adopt where a field is required (the bon builders lane gives this for required members) and for lifecycles with an invalid order. R5 still holds: `build()` validates what the types cannot say. IRS's `build()` calls `unwrap`; R15 wins (no `unwrap` in production). | not checked | builders with a required field, handles with a lifecycle: not counted | none |
| R35 | A type that owns a resource (browser, driver, session, pool) releases it in `Drop`, so an early return or an error still cleans up; scoped state is a guard value that restores it on drop. | IRS: RAII, `Drop`, lock guards | New in IRS as a pattern; C-DTOR-* only limit what `Drop` may do. Adopt with R27 on top: `Drop` neither blocks nor fails silently, async cleanup goes to a task, and (R25) it prints nothing, which IRS's example does. | not checked | the 21 `impl Drop` in `src` (F21) | F21 |
| R36 | Events and observers go through a channel (`mpsc`, `tokio::sync`): the subscriber owns a receiver and dropping it unsubscribes; no `Arc<dyn Observer>` list in a public signature. | IRS: message passing, mediator, traditional observer vs channels | IRS shows both and prefers the channel. Adopt the channel; the `Arc<dyn Observer>` form loses to R28 (no `Arc` in a signature) and R11 (generics, not `dyn`). | not checked | plugin observers and event streams: not counted | F23 |
| R37 | Avoid: a factory or a strategy that returns or stores `Box<dyn Trait>` in the public API. A factory is an enum or a generic constructor; a strategy is a generic parameter or a closure. | IRS: factory method, abstract factory, strategy, chain of responsibility, bridge, mediator (all with `Box<dyn ...>`) | IRS's own examples use `Box<dyn Shape>` and `Box<dyn Strategy>`; under the page's principles the stricter rule wins, so R11 and R28 beat them. `dyn` stays only for seams such as `CdpTransport`. | not checked | the `Box<dyn` sites of R28 (2 constructors, 5 closure aliases, 3 future aliases) | F23 |
| R38 | A facade is one type that owns its subsystems and delegates, and an adapter is a type that wraps a foreign type and implements our trait; the wrapped type stays private. | IRS: facade, adapter, proxy | New as a pattern name; it fits `BaseCase` over `sb_cdp`, the Playwright bridge and R29's escape hatch. IRS's proxy example prints a log line; R25 wins (the library does not print). | not checked | `BaseCase`, `PlaywrightSidecar`: not judged here | none |
| R39 | Use when the kinds of a thing are a closed set: an enum and `match` (enum polymorphism), with the operations as methods or functions, instead of a trait hierarchy or a visitor trait; the enum is `#[non_exhaustive]` if it may grow. | IRS: enum polymorphism, visitor | IRS itself says a visitor is better replaced by enum and `match`, and keeps a visitor trait object for plugin-style outside operations "rarely". Adopt the enum; the trait-object visitor loses to R11. | not checked | enums such as `Spoof`, `DriverMode`, `BrowserKind`: not judged | F6 |
| R40 | Avoid: ignoring a `Result` or a compiler warning (`let _ =`, `.ok()`, an `allow` to silence it). A fallible call is propagated or handled; a lint is satisfied, or `#[expect]` with a reason. | IRS: ignoring warnings anti-pattern | Agrees with R24 and R14; no conflict. | not checked | `let _ =` and `.ok()` sites: not counted | F1, F19 |
| R41 | Avoid: an index loop (`for i in 0..len` with `v[i + 1]`) and a chain of intermediate `Vec`s where one iterator chain does it; use `windows`, `get`, adapters. | IRS: manual indexing, iterator anti-patterns | Library code only; Google's "chains that read worse" (R30) still wins where a loop is clearer. | not checked | 3 `for_each` known; index loops not counted | none |
| R42 | Avoid: `mut` where a binding does not change, and `&mut self` where `&self` is enough; a public method takes `&mut self` only when it changes the value. | IRS: excessive mutability anti-pattern | Applies to the 700 `BaseCase` methods and to local code; no other guide names it. | not checked | public `&mut self` receivers: not counted | none |
| R43 | Avoid: explicit lifetimes where elision gives the same signature; a lifetime parameter that no output uses. | IRS: improper lifetime handling anti-pattern | Applies to public signatures; Clippy's `needless_lifetimes` is the lint to turn on in lane 1 (R24). | not checked | not counted | F1 |
| R44 | Avoid: forcing a type (`as` casts that lose value, `transmute`) and a function whose signature promises more than it returns (divides by a count that may be zero); return `Option` or `Result`, convert with `TryFrom`. | IRS: fighting the type system anti-pattern | Fits R7, R14 and R15; no conflict. | not checked | `as` casts in public code: not counted | F9, F10 |

## Coverage by item: Rust API Guidelines

The item-level evidence behind the rule table: every `C-` id once, with its count. The rule table maps
each id to its rule (R1 to R31) and to the Google rule that shares it.

Tally of the 54 items: met 24, partly 19, not met 3, n/a 7, not checked 1 (the first pass had met 26
and partly 17; C-METHOD and C-DTOR-BLOCK moved to partly in the second; the third moved none).

| Item | Status | Affected | In one line |
|---|---|---|---|
| C-CASE | met | 0 | rustc's naming lints are on by default and the gate runs clippy with `-D warnings`; no counter-example in the files read. |
| C-CONV | met | 4 | 13 `as_`, 4 `into_` and at least 30 `to_` methods fit their prefixes; four `const fn name(self) -> &'static str` are the odd names where `as_str` is used elsewhere (`Browser::name` and `DriverMode::name` in `src/browser/config.rs:91,169`, `src/browser/chrome_for_testing.rs:84`, `src/browser/options/proxy.rs:120`). |
| C-GETTER | not met | 104 | 35 `pub fn get_` and 69 `pub async fn get_` (78 of them on `BaseCase`, where D2 is reversed: the prefix goes). |
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

## Coverage by item: Pragmatic Rust Guidelines

The item-level evidence behind the rule table: every `M-` id once, with its count.

Tally of the 90 guidelines: met 19, partly 32, not met 5, accepted 2 (M-MSRV, M-FOREIGN-REEXPORTS),
n/a 11, not checked 21. The 21 not checked are the 11 performance guidelines, the 3 application
guidelines (they concern `crates/cli`), and 7 single ones marked below. The third pass moved
M-AVOID-WRAPPERS from met to partly, M-PANIC-CONTINUATION and M-PANIC-IS-STOP from not checked to
partly, and M-MSRV and M-FOREIGN-REEXPORTS from not met to accepted (the first pass had met 20,
partly 29, not met 7, not checked 23).

| Guideline | Status | Affected | In one line |
|---|---|---|---|
| M-DESIGN-FOR-AI | met | 0 | `llms.txt`, `AGENTS.md`, `CLAUDE.md`, `context7.json`, the `dev` MCP server. |
| M-NO-META-DESIGN-DOCUMENTATION | partly | 681 lines | 681 doc lines say "Python's"; history notes at `src/lib.rs:32-39` and `src/error.rs:10-11` (F18). |
| M-RUST-SHAPED | partly | about 700 | `BaseCase` keeps Python's flat shape; `sb_cdp` is the Rust-shaped engine. |
| M-SINGLE-ITEM-PATH | not met | about 86 | The crate root re-exports about 86 names that also live in their modules (F12). |
| M-TAUTOLOGICAL-TESTS | not checked | n/a | Tests were not read. |
| M-PANIC-CONTINUATION, M-PANIC-IS-STOP | partly | 3 and 34 | Library code should not catch a panic and go on, and a panic is not to be assumed caught: 3 production `catch_unwind` and 34 `PoisonError::into_inner` (F22, D11, D12). Google has both rules. |
| M-PANIC-MESSAGE | not checked | n/a | Needs the production `unwrap`/`expect`/`panic` list (lane 1). |
| M-PANIC-ON-BUG | partly | 3 | `BrowserSession::driver` panics on an expected state; two macro arms panic (F10). |
| M-UNSAFE-IMPLIES-UB, M-UNSAFE | met | 0 | No `unsafe` in the library; 11 blocks in test code, 2 in `src/api/master_qa/wrapper_tests.rs:36,48` and 9 in `tests/` (the first pass counted `src` only), 10 of them without the `SAFETY:` comment that Google asks for (F24). |
| M-UNSOUND | not checked | n/a | Not audited. |
| M-CANONICAL-DOCS | partly | see C-FAILURE | Same gaps as C-FAILURE and C-EXAMPLE (F3). |
| M-DOC-INLINE | partly | 0 of 26 | `sb_cdp/mod.rs:110-177` marks every `pub use`; `src/lib.rs:201-246` marks none. |
| M-FIRST-DOC-SENTENCE | partly | not counted | Enforced only where the lint set is on (5 module trees). |
| M-MODULE-DOCS | partly | 4 | `common/mod.rs`, `config/mod.rs`, `utils/mod.rs`, `utilities/mod.rs` have no `//!`. |
| M-EXAMPLE-OVER-PROC | met | 0 | Only `macro_rules!`. |
| M-MACRO-HELPERS, M-MACRO-VERSION-PIN, M-PROC-IMPL, M-PROC-IMPLIED-ITEMS | n/a | 0 | No proc macro, no third-party helper item. |
| M-MACRO-LAST-RESORT | not met | 27 | 27 exported macros; those read are mostly a single method call (F7). Google ranks macros after functions too; D3 cuts the `sb_*` macros to `sb_test!`. |
| M-MACRO-MAIN-CRATE | partly | 1 | `sb_test!` expands to `#[tokio::test]`, which needs `tokio` in scope. |
| M-MACROS-DONT-LIE | partly | 2 or more | `sb_open!` returns a `Result` or panics by arity (F7). |
| M-CARGO-WORKSPACE | partly | 1 | `[workspace.package]` is used; no `[workspace.dependencies]`, no `[workspace.lints]` (F1, F13). |
| M-CRATES-FLAT-FOLDER | not met | 1 | The library is the root package, the others sit in `crates/`. Decided in `docs/design/core_split.md` (D6). |
| M-CRATES-IN-WORKSPACE | met | 0 | All three members are listed. |
| M-LATEST-EDITION | met | 0 | Edition 2024 (`Cargo.toml:18`). |
| M-MSRV | accepted | 1 | `rust-version = "1.99"` is the newest stable; the guideline wants a few versions behind (F13). Decision D1 keeps the newest stable. |
| M-DOCUMENTED-MAGIC | met | 0 | Constants carry their reason (`src/api/run_settings.rs:30-60`); sampled. |
| M-LINT-OVERRIDE-EXPECT | partly | 15 | 15 `#[allow(` against 25 `#[expect(` (F19). |
| M-LOG-STRUCTURED | not checked | n/a | `tracing` is used with named fields at `src/error.rs:543`; the rest not read. |
| M-PUBLIC-DEBUG | partly | 2 or more | See C-DEBUG. |
| M-PUBLIC-DISPLAY | not checked | n/a | Not audited. |
| M-REGULAR-FN | not checked | n/a | Not audited. |
| M-SHORT-NAMES | not checked | n/a | Read in the third pass: it prefers abbreviations where Google refuses them (rule R1 adopts a middle form); the crate's names were not counted against it. |
| M-SMALLER-CRATES | not met | 1 | One crate holds two engines, the evasion layer, the MCP servers, a Python-to-Rust translator, the CLI half, reports and plugin hosts (F12). The split is designed in `docs/design/core_split.md` (D6). |
| M-STATIC-VERIFICATION | partly | see F1 | Gates exist; the lint set is a subset on 5 module trees. |
| M-UPSTREAM-GUIDELINES | n/a | 0 | This note. |
| M-WEASEL-WORDS | partly | 5 | `core::log_helper`, `core::report_helper`; the grab-bag modules `common`, `utils`, `utilities`. |
| M-FEATURES-ADDITIVE | partly | 1 | `BrowserConfig` gains a public field under feature `plugins` (F8). |
| M-OOBE | met | 0 | Drivers and browsers are downloaded on demand. |
| M-SYS-CRATES | n/a | 0 | No `-sys` crate. |
| M-DONT-LEAK-TYPES | partly | see list | thirtyfour types (in progress elsewhere); `serde_json::Value` and `tokio` through deliberate re-exports. |
| M-ESCAPE-HATCHES | met | 0 | `Element::as_webdriver_element` and `into_webdriver_element` (`src/browser/element.rs:143,149`), `Page::browser`. |
| M-FOREIGN-REEXPORTS | accepted | 4 | `tokio`, `drivers`, `json`, `testing::rstest`: deliberate, decision D8 accepts them. |
| M-IMPL-ASREF | partly | not counted | `path: &str` and `file_path: &str` (F9). |
| M-IMPL-IO, M-IMPL-RANGEBOUNDS | n/a | 0 | Nothing takes a reader or a range. |
| M-TYPES-SEND | partly | 3 pinned | See C-SEND-SYNC. |
| M-AVOID-STATICS | partly | 21 | 20 `OnceLock` and 1 `LazyLock` caches; the registries in `common::shutdown` and `browser::reuse` (F17). |
| M-BUILD-RESULT | met | 4 | The four builders validate in `build()`. |
| M-INTEGRATION-TESTS | met | 0 | `tests/` holds the offline, chrome and contract binaries. |
| M-LOG-NOT-PRINT | partly | 3 | Three `eprintln!` in library code (F17). The five `println!` in `BaseCase` and the `rich_helper` print functions are documented prints, not counted. |
| M-MOCKABLE-SYSCALLS | partly | 9 | The settings code takes an injected lookup (`Settings::load_with`, `from_vars`); three `upload_file`, `artifacts::logs_root`, `find_system_chrome` and four cookie-file methods read the environment or the working directory directly (F20). `sb_cdp::Browser::new_mocked` (feature `test-util`) is the engine-level mock; the WebDriver engine's was not checked as exported. |
| M-NO-GLOB-REEXPORTS | partly | 1 | `src/profile_payloads/mod.rs:55`: `pub use profile::*;`. Google allows it in a non-leaf module; rule R21 adopts the stricter. |
| M-STRONG-TYPES-GUARD | partly | 3 | Public fields (C-VALIDATE). |
| M-STRONG-TYPES | partly | see C-NEWTYPE | Strings for selectors and paths, `bool` for modes, keyword strings in `ProfileParams` (F9). |
| M-TEST-UTIL | met | 0 | Feature `test-util` (`Cargo.toml:194`). |
| M-ASYNC-FN | partly | 84 | 84 `#[async_trait` sites, boxing every call of the capability traits (F16). |
| M-AVOID-WRAPPERS | partly | 10 | 2 public constructors take `Arc<TestRun>`, 5 public aliases name a boxed closure and 3 a boxed future (F23; the first pass read the signatures of other modules and wrote met). |
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

Ranked by how many callers meet the problem and by how hard it is to fix later. The letters after a
finding name the guides it comes from: C the Rust API Guidelines, M Pragmatic Rust, G Google's
guide.

1. **No lint set in the manifests (F1; M, G).** The Pragmatic Rust set is held on 5 of about 27 module
   trees by attributes in `src/lib.rs`; `[workspace.lints]` does not exist, so over 4,000 lines with `.unwrap()`,
   600 with `.expect(` and 160 with `panic!(` in `src` are unchecked and every count in this note that says
   "not counted" waits on it.
2. **Selectors are `&str` in about 370 signatures (F9; C, M, G)** although `Selector` and `SelectorBuf`
   exist and `Page::locator` already takes `impl Into<SelectorBuf>`; the parameter is called `css`
   even when it holds an XPath.
3. **104 `get_` functions (F5; C, G)**, 78 on `BaseCase`, the one checklist item with a
   count in the hundreds. The owner reversed D2 on 2026-11-08: `BaseCase` drops the prefix
   too (`get_text` becomes `text`), and the Python names stay findable through `parity/api.toml`,
   the Python importer and `parity_lookup`.
4. **27 exported macros, mostly one method call (F7; C, M, G)**; in five of them an optional last
   argument changes the type from a `Result` to `()` that panics. Decision D3 (taken) cuts the
   `sb_*` macros to `sb_test!`.
5. **The error type keeps almost no source (F4; C, M, G):** 31 of 34 variants hold only text,
   `ResultExt::sb_context` turns every error into `InvalidConfig`, and public functions return
   `Result<_, String>` (4) or `Box<dyn Error>` without `Send + Sync` (2).
6. **Open traits and `Deref` for inheritance (F6; C, M, G):** six traits that nobody should implement are
   open, and `MasterQA`, `SbCase`, `SbCdpCase` deref to `BaseCase`. `crates/playwright` shows the
   sealed form.
7. **`BrowserConfig` is not additive (F8; C, M, G):** a public field exists only under feature
   `plugins`, and no config struct, `Browser` or `DriverMode` is `#[non_exhaustive]`; `src/api` has
   no `#[non_exhaustive]` at all. The settings tables that callers build with `..Default::default()`
   stay exhaustive on purpose (Google's exception, rule R13).
8. **Two paths per item and one crate for everything (F12; M, G):** about 86 root re-exports, 203
   `pub mod`, a Python translator and the library half of the CLI inside the library.
9. **`BaseCase` and `BrowserSession` have no `Debug` (F2; C, M, G)**, no doc comment on `BaseCase`
   (F3; C, M, G), and `Send + Sync` is pinned for 3 types of the public handles (F14; C, M).
10. **The logging plugins (F20; C, M):** unit structs with a receiver-less `upload_file` that reads its
    credentials from the environment, and an error enum of `String`s.


## Findings

Each finding names the items, the guides it comes from (in its heading), the proposed fix, and
whether it breaks the public API. F1 to F21 are the findings of the first two passes with the
Google rules merged in; F22 to F24 are new in the third pass.

### F1. No workspace lint set (M-STATIC-VERIFICATION, M-CARGO-WORKSPACE; Google: lints it names, `missing_docs`, formatting)

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
  library has no `unsafe`; the 11 test blocks take `#[expect(unsafe_code, reason = "...")]`), and
  `unwrap_used` and `panic` at `warn`, with `allow-unwrap-in-tests = true` and
  `allow-panic-in-tests = true` in `clippy.toml`; `expect_used` stays off, because Google's guide
  prefers `expect` with a reason to `unwrap` where a panic is needed (rule R15) and
  `missing_panics_doc` then documents each one. That turns
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
  why; decision D7 (taken: move to 2024, later, as the last lane).
* Google (rule R24): the guide asks for `missing_docs` on public APIs, and the lint's name occurs
  nowhere in the repository (0 hits), so lane 1 adds it to the table. The guide names these Clippy
  lints: `should_implement_trait`, `wrong_self_convention`, `new_without_default`,
  `len_without_is_empty`, `ptr_arg`, `borrowed_box` and `exhaustive_enums`; lane 1 reads the group
  of each on Clippy's list and adds those the table above lacks (`exhaustive_enums` is not in it).
  No `allow` of `new_without_default` exists (0 hits) and `should_implement_trait` is expected
  twice, with a reason (`src/sb_cdp/expect.rs:89,313`).
* Panics (D11, D12, F22): `clippy.toml` gets `disallowed-methods` entries (fields `path`, `reason`,
  `replacement`, read on Context7) for `std::panic::catch_unwind`, for the `catch_unwind` method of
  `futures_util`'s `FutureExt` and for `std::sync::PoisonError::into_inner`, so a new use fails the
  gate unless it carries `#[expect(clippy::disallowed_methods, reason = "...")]`. Clippy reports a
  path that does not exist, so each entry is checked on the first run.
* The guide also says to format with the tool, to opt out with `#[rustfmt::skip]` only for manual
  alignment or generated code, and not to use unstable features: `rustfmt.toml` is the one key above
  (the default width of 100 is the guide's) and the repository has no `#![feature(` (0 hits).
* Breaking: no.

### F2. `Debug` is missing on the two main types (C-DEBUG, M-PUBLIC-DEBUG; Google: important traits)

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
* Google (rule R9): "if you think it is likely that a type will be used as a map key ... implement
  `Hash` and `Eq`", which is the missing `Hash` above; `Clone` for "most types ... unless they
  represent some external resource", so `BaseCase` and `BrowserSession`, which own a browser,
  rightly have none (`Browser` and `Page` are the cheap `Clone` handles of M-SERVICES-CLONE);
  `Copy` "whenever it is reasonably clear that the type will continue to be `Copy`". The guide also
  says to derive where a manual impl could be a derive; that was not counted.
* Breaking: no.

### F3. Documentation gaps (C-EXAMPLE, C-FAILURE, M-CANONICAL-DOCS, M-MODULE-DOCS, M-DOC-INLINE; Google: documentation)

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
* Google (rule R18): "add documentation comments to public APIs, potentially using the `missing_docs`
  lint" (not on today, F1). On examples: "enough functionality should be covered in examples
  *somewhere* to give an idea of how the crate works", so the target is one example per type and
  per family of methods, not the 2,566 functions without one that C-EXAMPLE read literally would
  ask for (`BaseCase` alone has about 700 methods). "Always document panic cases": the 4
  `# Panics` and `missing_panics_doc`, above. "It is generally okay to omit documentation on
  individual enum variants, especially if their name is self-explanatory": the four `Browser`
  variants may stay as they are. "Comments should say something that the code does not already
  say": the first line of `BrowserSession::driver` above fails it; none other was counted, because
  only reading finds them.
* The 7 `#[doc = concat!(` attributes (`src/api/recorder.rs:975`, `src/cli/scripts/sb_mkchart.rs:348`,
  `sb_mkdir.rs:644,662`, `sb_mkfile.rs:903`, `sb_mkpres.rs:456`, `src/utilities/python_importer.rs:563`):
  the one read, `src/cli/scripts/sb_mkdir.rs:640-668`, is a `cfg(doctest)` harness over golden
  files, not user documentation, so the guide's "document things in a straightforward manner ... rather than
  generating documentation from macros" does not touch it; the other six were not read.
* Fix: a doc for `BaseCase` (the four `Browser` variants may stay undocumented, see above), module
  docs for the four roots, one example per type and per family of methods, then the lint
  output of lane 1 for `# Errors` and `# Panics`; `#[doc(inline)]` on the root re-exports.
* Breaking: no.

### F4. The error type (C-GOOD-ERR, M-ERRORS-CANONICAL-STRUCTS, M-FROM-ERROR; Google: reporting errors)

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
  (lane 5). Breaking: low.
* Fix, large: the canonical shape of M-ERRORS-CANONICAL-STRUCTS, a struct with a `kind()` enum, a
  source and a hint, and `Skipped` leaves the error type. Breaking: yes, and it waits for the
  thirtyfour driver-errors lane (the `WebDriver` variant, `src/error.rs:76`).
* Google (rule R14): "use `Option` for simple failures where the cause is obvious", `Result` "for
  recoverable failures where the caller needs context", and `panic!` for bugs and violated
  invariants. By it `parse_ide_html`, which cannot fail, returns its value and no `Result`; the four
  `Result<_, String>` give a caller no failure mode to tell apart, which is what the guide's
  `Result` is for; and `Skipped` is not a failure at all. The guide's advice that error types
  implement `Copy` "whenever possible" does not apply: the variants carry `String`s.
* Decisions taken: D4, `Skipped` leaves the error type and the canonical struct is adopted (lane 18,
  after the driver-errors lane); D9, the six functions get a typed error that displays the text they
  return today (lane 5).

### F5. The `get_` prefix (C-GETTER; Google: getters and setters, naming methods)

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
* The prefix is the Python names (`parity/api.toml`). Decision D2 was reversed on 2026-11-08: the
  `BaseCase` getters (78 of the 104) drop `get_`; the Python names stay findable through
  `parity/api.toml`, the Python importer and `parity_lookup`.
  What this finding still asks for: the other 26 `get_*`, `get_browser`, the `*_js` getters and
  `name` (the cheap part, lane 15).
* Google (rule R3) spells the whole family: `foo()` returns `&T` (or `T` if it is `Copy`),
  `foo_mut()` returns `&mut T`, `take_foo()` clears the value and returns the old one, `set_foo(T)`
  takes the value, `replace_foo(T)` sets it and returns the old one. Of the 14 public `set_*` in
  `src`, 13 fit (a `&mut self` method or a free function that returns nothing; `set_time_limit`
  returns a `Result`); `set_header(self, ..) -> Self` (`src/sb_cdp/intercept.rs:251`) is a builder
  step named like a setter, and `replace_body(self, ..) -> Self` (`:277`) returns `Self`, not the
  old value. There is no public `take_*` and no public `*_mut` getter (0 hits); the `case_mut()` of
  F6 is the first one to be added, in the guide's form. Rename the two builder steps `with_header`
  and `with_body` (F15, lane 15).
* Google also says to avoid `.clone()` "as a substitute for borrowing": `get_browser` clones the
  whole `BrowserConfig` (above) where a `&BrowserConfig` would do.
* Breaking: yes for the other 26 and for the renames; yes for `BaseCase` too (D2 reversed).

### F6. Open traits and `Deref` for inheritance (C-SEALED, C-DEREF; Google: trait vs inherent impl, extension traits)

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
* Google (rules R11, R12): define a trait "to enable generic programming", to accept or return an
  `impl Trait` or `dyn Trait`, or as an extension trait, and "don't use traits merely to organize
  code". The four capability traits pass: generic bounds name them (`E: BrowserApi + ElementApi +
  AssertionApi`, `src/api/traits.rs:20` and `tests/offline/sb_cdp_mock.rs:1183`). `Contains` is an
  input trait implemented on foreign types (`[T]`, `[T; N]`, `Vec<T>` and, by its doc, `str` and
  `String`), which the guide expects and does not call an extension trait; `ResultExt` carries the
  `Ext` suffix the guide asks for. "In libraries, prefer accepting generics ... over `dyn Trait`":
  `dyn BrowserApi` occurs nowhere in the repository (14 hits for the name, none `dyn`); the other
  three traits were not searched.
* Decision D5 (taken): native `async fn` where no `dyn` is used (F16, lane 16). The guide's
  `foo_mut()` is the name for the `case_mut()` above.
* Breaking: yes (callers of `qa.open(..)` write `qa.case_mut().open(..)`).

### F7. Macros (M-MACRO-LAST-RESORT, M-MACROS-DONT-LIE, C-MACRO-ATTR; Google: macros)

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
* Google (rule R20): prefer, in order, regular functions, types and traits, then `macro_rules!`,
  then other code generation, then `proc_macro`, because macros make tools and large-scale changes
  harder. Beside the 27 exported macros `src` has 16 internal `macro_rules!` helpers
  (`option_methods!` in `src/api/jqc.rs:830`, `text_contains!` in `src/api/membership.rs:45`,
  `accessors!` and `setters!` in `src/evasion/fingerprint.rs:700,1463`, `audit_each!` in
  `src/evasion/providers/mod.rs:746`, and others in `src/config/`, `src/api/recorder/calls.rs` and
  a test). `setters!` defines public builder methods but takes each doc comment from its call site
  (`$(#[$doc:meta])*`, `:1464`), so the docs stay written where the guide wants them; the cost is
  the tooling one.
* Fix: decision D3 (taken) cuts the `sb_*` macros to `sb_test!` (and its `#[doc(hidden)]` helper
  `sb_test_options!`). The four macros that are not named `sb_*` follow the same rule: `uc_config!`
  and `cdp_config!` go (they are `BrowserConfig::default().with_mode(..)`), and lane 14 reads the
  expansions of `selector!` and `fingerprint!` and deletes either one if a function does the job.
* Breaking: yes.

### F8. Evolution of the public types (C-STRUCT-PRIVATE, C-VALIDATE, M-FEATURES-ADDITIVE; Google: non_exhaustive)

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
* Google (rule R13) asks for `#[non_exhaustive]` on an enum of a public API that may change (not on
  an enum that describes a fixed set, or two states) and on a struct with public fields, with one
  exception that matters here: you "want to enable your users to use functional record update (FRU)
  struct initialization syntax" (`..Default::default()`), because a `#[non_exhaustive]` struct
  "can't be constructed with the struct literal syntax" outside its crate. Callers build the
  settings tables that way: `tests/chrome/bdd_chrome.rs:13-18` builds `Settings { browser:
  BrowserSettings { headless: true, ..Default::default() }, ..Settings::default() }` and the
  stealth contract builds `ProxyConfig { host, ..Default::default() }`
  (`tests/contract_stealth.rs:200-203,332-335`); `..Default::default()` stands on 24 lines of the
  repository outside the changelog. So `Settings`, `BrowserSettings`, `ProxyConfig` and the like
  stay exhaustive with `Default`, and a field added to them breaks only a literal that does not
  end in `..Default::default()`; `BrowserConfig`, `Cookie` and the `api` records are built by
  `with_*` and `new`, so they take the attribute.
* Also Google: list every variant in a `match` over a `non_exhaustive` enum (an implementation
  habit, not counted).
* Fix: `#[non_exhaustive]` on `BrowserConfig`, `Cookie`, the two enums and the `api`, `utilities`
  and `profile_payloads` types listed above, and not on the tables built with
  `..Default::default()`; the `plugins` field becomes unconditional (a zero-size `BrowserPlugins`
  when the feature is off).
* Breaking: yes.

### F9. Strings, booleans and paths where a type fits (C-NEWTYPE, C-CUSTOM-TYPE, C-GENERIC; Google: flexible references, `Into`, enum over bool, named fields)

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
* Google (rules R6, R7): its table says `&str` for `&String`, `&[T]` for `&Vec<T>` and `Option<&T>`
  for `&Option<T>`. `src` has no `&Vec<` (0) and three `&Option<`, none public (a serde `serialize`
  helper, `src/api/run_settings.rs:126`; a private helper, `src/evasion/capture.rs:406`; a closure,
  `src/mcp/dev/diagnose.rs:685`), and the six `&String` are `From<&String>` impls, so the table is
  met. The guide also says to use `enum` parameters instead of `bool` where the purpose is not
  obvious: the nine sites above.
* Google on `Into`: "consider accepting `impl Into<T>` ... if call-site ergonomics are paramount",
  balanced against "code bloat concerns", and application code "should almost never" use it. `src`
  has 253 lines with `impl Into<` (147 `impl Into<String>`), which fits the first half;
  `crates/cli`, being application code, takes concrete types. The selector lane turns about 370
  signatures into generic functions: it keeps each one to the conversion and a call to a
  non-generic body, so they do not become 370 copies of their bodies (lane 19).
* Google: "prefer types with named fields over tuples". `pub type GeneratedFile = (RelativeName,
  String);` (`src/cli/scaffold.rs:345`) names a pair by position; make it a struct with named
  fields. Other tuple returns were not counted. The other `pub type` aliases read (15 in all) are
  boxed closures and futures (F23), `Result<T>` (`src/error.rs:6`) and plain renamings such as
  `HtmlInspection`.
* Google on iterators: library code "should accept iterators or `impl IntoIterator` if it only
  needs to iterate" and should prefer returning iterators; `&[&str]` and `&[String]` parameters
  were not counted, and the functions read return a `Vec` built from a browser answer, which the
  guide allows.
* Breaking: yes.

### F10. Panics on expected states (M-PANIC-ON-BUG; Google: panic for bugs, `expect` over `unwrap`)

* `BrowserSession::driver` (`src/browser/session/mod.rs:93-99`) panics after `quit`, on a direct
  CDP attachment and after a failed `reconnect`: states a caller can reach without a bug. Its
  sibling `webdriver` (`:116`) returns the right error.
* The message arms of the `sb_*!` macros and `sb_test!` panic by design (F7).
* There are 4 `# Panics` sections, so the rest is undocumented or absent; lane 1's
  `missing_panics_doc` and the `panic` lint list them.
* Fix: remove `driver()` in favour of `webdriver()` once the drivers lane lands.
* Google (rule R15): `panic!` is "to report bugs and violated invariants", and `Result` is for
  failures "the caller can reasonably anticipate"; `driver()` is the second kind, which the fix
  above follows. "If a panic is necessary because an invariant cannot be expressed in the type
  system, prefer `expect` over `unwrap` to document the reason", and prefer pattern matching to
  check-then-unwrap: lane 1 turns on `unwrap_used` and `panic` for production code and leaves
  `expect_used` off (F1), so each production `.unwrap()` that remains becomes `expect("reason")`
  or an `if let`. The guide adds that some codebases avoid panics altogether; this one does not.
* Related: F22 (panics that are caught).
* Breaking: yes. Blocked by the drivers module.

### F11. Names that collide or drift (C-WORD-ORDER, M-WEASEL-WORDS; Google: avoid abbreviations)

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
* Google (rule R1): do not use abbreviations "that someone outside your project is unlikely to
  know" and do not "abbreviate by deleting letters within a word". Six `BrowserConfig` setters do:
  `with_rec_sb_cdp`, `with_rec_print`, `with_rec_behave`, `with_rec_sb_mgr`
  (`src/browser/config.rs:772,796,802,808`; `rec` is recording and `sb` the project's own prefix)
  and `with_disable_ws`, `with_enable_ws` (`:940,946`), where `ws` stands for web security, as the
  enum behind them says (`LaunchSwitch::DisableWebSecurity`), though most readers will see
  WebSocket; the accessor `rec_sb_cdp()` (`:779`) has the same name. The names come from
  SeleniumBase's flags, which the doc comments keep (`--rec-sb-cdp`, `--disable-ws`);
  `with_esc_end` stays, since Esc is the name of the key. M-SHORT-NAMES, read in the third pass,
  asks for abbreviations (`CallbackFn`), the opposite; rule R1 takes the middle, short names and an
  abbreviation only when it is common outside the project. Fix: spell `ws` as `web_security` and
  `rec` as `recording` (lane 15); the flag spellings stay in the doc comments. Other cases are
  `uc: bool` (F9) and the module `jqc`, not counted further. **Done (2026-11-08):** the six setters are renamed (`with_recording_*`, `with_disable_web_security`, `with_enable_web_security`); the accessor `rec_sb_cdp()` is not renamed yet.
* Breaking: yes.

### F12. The crate root and the size of the crate (M-SINGLE-ITEM-PATH, M-SMALLER-CRATES, M-BALANCED-MODULES, M-NO-GLOB-REEXPORTS; Google: glob imports, non-leaf modules, small crates)

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
* Google (rule R21): "the only items in non-leaf modules should be module declarations and
  re-exports" (a small amount of code that ties things together is fine). 13 `pub struct` are
  defined in 7 `mod.rs` files of `src`: `src/browser/session/mod.rs:49` (`BrowserSession`),
  `src/browser/options/mod.rs` (3), `src/evasion/providers/mod.rs` (3), `src/mcp/cdp/mod.rs:60`,
  `src/mcp/dev/mod.rs:55`, `src/testing/bdd/mod.rs` (2) and `src/utilities/importer/mod.rs` (2).
  Move each into a file named after the type (the guide: name files and leaf modules after the
  primary user-defined item) and re-export it; the paths stay, so this does not break (lane 6). The
  files read are otherwise named that way (`base_case.rs`, `cookie.rs`, `master_qa.rs`).
* Google on glob imports: "don't use glob imports", except `use super::*` for inline modules and
  `pub use submodule::*` for non-leaf modules, and "defining preludes should be avoided". `src` has
  341 lines ending in `::*;`: 331 are `use super::*;`, which the guide allows; 9 import a third-party
  prelude (3 `base64::prelude::*` at `src/api/base_case.rs:8`, `src/api/cdp_page.rs:16` and
  `src/sb_cdp/breakpoint.rs:141`; 6 `httpmock::prelude::*` at `src/browser/chrome_for_testing.rs:722`,
  `downloader.rs:268`, `driver_install.rs:675` and `src/cli/scripts/sb_grid.rs:556,582,607`), which
  the guide leaves to the reviewer; and 1 is the re-export above, which the guide allows and
  M-NO-GLOB-REEXPORTS does not. Rule R21 adopts the stricter, so lane 6 replaces it.
* Google's "prefer small crates" and "flat file layout" are marked for Chromium and allow "a large
  foundational library"; they add nothing to the split, which D6 (taken) designs in
  `docs/design/core_split.md`.
* Breaking: yes for the module tree; no for the `mod.rs` moves and the glob.

### F13. Manifest (C-METADATA, C-STABLE, M-MSRV, M-CARGO-WORKSPACE; Google: no unstable features, `const fn`)

* `[package]` (`Cargo.toml:23-30`) has `description`, `license`, `repository`; it lacks
  `documentation`, `readme`, `keywords`, `categories`, `homepage`.
* Feature `error-backtrace` (`:183`) is empty and nothing reads it. `docs/DEVELOPER_GUIDE.md:493`,
  `docs/CONTRIBUTING.md:518`, `docs/ABI_API.md:181` and `docs/contributing/debugging.md:28` say so;
  `README.md:515`, `CONTRIBUTING.md:300` and `docs/building.md:585` say it captures backtraces
  ("a backtrace on every error"). A feature that does nothing and is documented as doing
  something.
* `rust-version = "1.99"` is the newest stable (`:19`); M-MSRV asks for a few versions behind.
  `CLAUDE.md` says this is deliberate, so it was decision D1 (taken: keep the newest stable; the
  `msrv` CI job and the `Dockerfile` base image move with it).
* No `[workspace.dependencies]`: versions are repeated per member (F1 adds the lints; this adds the
  dependencies).
* Public dependencies below 1.0: `thirtyfour` 0.38 (`:82`), `rstest` 0.28 (`:164`), `rstest-bdd`
  0.6 (`:166`), and `seleniumbase-rs-plugin` 0.1 from a git revision (`:155`), re-exported as
  `plugin_api`. The `semver` task in `mise.toml:210` (cargo-semver-checks against a tag) exists,
  which is the right gate.
* Google (rule R23): "don't use unstable features": none (0 `#![feature(` in the repository). "Mark
  a public function as `const` only if you expect it to remain `const`": 85 `pub const fn` in
  `src`, among them the four `name` methods (F5); none was read for that question, and turning one
  into a plain function breaks callers, so lane 15 reads those four before it renames them.
* Breaking: removing `error-backtrace` breaks anyone who enables it; nobody is known to.

### F14. `Send` and `Sync` are pinned for three types (C-SEND-SYNC, M-TYPES-SEND)

* `tests/contract_tauri.rs:40-42` pins `Browser`, `CdpInbox`, `Page`; `tests/contract_stealth.rs:244-257`
  pins two error types and `Debug + Clone + PartialEq` for five values. `BaseCase`, `Locator`,
  `BrowserPool`, `Lease`, `Cookies`, `RunSettings`, `Sb`, `SbCdp` are not pinned, and `BaseCase`
  is the type every `tokio::spawn` user meets.
* Fix: one test file with an `assert_send_sync` over every public handle type.
* Breaking: no.

### F15. Constructors and builders (C-CTOR, C-BUILDER, M-INIT-BUILDER; Google: initialization functions, closure accessors)

* `BaseCase::new` is `async` and starts a browser (`src/api/base_case.rs:131`), where
  `sb_cdp::Browser::launch` says what it does; `BaseCase::with_session` is a constructor
  (`:161`), while `with_*` elsewhere is a setter (`BrowserConfig::with_headless`,
  `Page::with_timeout`).
* Four real builders with a fallible `build()` (`BehaviorBuilder`, `FingerprintBuilder`,
  `LaunchOptionsBuilder`, `PoolOptionsBuilder`) and four other styles: 80 `with_*` on
  `BrowserConfig`, consuming setters named like the field on `Cookie`, `RunSettings::with_*`,
  public fields on `Settings`. The builder note (`/design/bon_builders`) settled the first two;
  this audit keeps that decision.
* Google (rule R4) names initialization functions: "`new()`, or `new_with_foo()` if there is more
  than one. If the constructor returns a `Result`, use `try_new()`", and `from_foo()` for a
  value-to-value conversion with a `From` impl. `src` has 83 `pub fn new(`, at least 10 of which
  return a `Result` on their first line (`src/api/gui.rs:174`, `src/api/master_qa.rs:244`,
  `src/browser/options/extensions.rs:42`, `src/cli/scaffold.rs:144`,
  `src/cli/scripts/sb_mkchart.rs:146`, `sb_mkdir.rs:171`, `sb_mkfile.rs:238`, `sb_mkpres.rs:247`,
  `src/core/allure.rs:423`, `src/plugins/reports.rs:215`); `try_new` has 0 hits (a `new` whose
  signature spans lines was not read). Five constructors carry the name of a setter:
  `BaseCase::with_session` (`src/api/base_case.rs:161`), `with_sources` (`src/core/allure.rs:433`),
  `with_config` (`src/evasion/providers/mod.rs:105`), `with_run` (`src/plugins/reports.rs:221`) and
  `with_launcher` (`src/sb_cdp/pool.rs:324`).
* Google (rule R5) gives `with_foo(f)` another meaning: it calls `f` with a borrow of the `foo`
  property (and `with_mut_foo` with a mutable borrow), for a property inside a `Mutex` or
  `RefCell`, or when a change needs follow-up work and `set_foo` cannot be used. Here `with_*` is
  the setter that takes and returns `self`: 153 public items in `src` (151 `pub fn` and 2
  `pub const fn`, about 80 of them on `BrowserConfig`), and none reads or changes a property through a
  closure. The one `with_*` that takes a closure, `FingerprintBuilder::with_policy`
  (`src/evasion/fingerprint.rs:1771`), stores a transform that `build()` runs. Decision D10
  (replaced on 2026-11-08): setters lose `with_` and are named after the field on a separate
  builder type (the bon builders lane); `with_*` stays only for constructors, as in `Vec::with_capacity`,
  so the five constructors above keep their names and `set_header`/`replace_body` (F5) become
  setters of a builder or `set_*`.
* Google also asks that an options struct implement `Default` and that an important `From` impl
  have a constructor beside it: `BrowserConfig`, `Settings`, `BrowserSettings` and `ProxyConfig`
  have a `Default` (the tests above build them with it) and no `allow` of `new_without_default`
  exists (F1); the `From` impls were not read for the second point.
* Fix: `BaseCase::connect(config)` for `new`; the `with_*` constructors keep their names (the
  `new_with_*` rename is dropped with D10); `try_new` for the fallible `new` above (`BaseCase::new` is `async` and
  starts a browser, so it takes `connect` instead); `with_header` and `with_body` for `set_header`
  and `replace_body`; nothing else.
* Breaking: yes, small.

### F16. Capability traits (M-ASYNC-FN, M-PARAMETER-CONSISTENCY; Google: generics vs `dyn`)

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
* Decision D5 (taken): native `async fn` in the traits where nothing uses them as `dyn`. `dyn
  BrowserApi` occurs nowhere in the repository (F6); lane 16 searches the other three first and
  keeps `#[async_trait]` only on a trait that is used as `dyn`. Google (rule R11) agrees: a library
  prefers generics to `dyn`.
* Breaking: yes.

### F17. Printing, statics, global effects (M-LOG-NOT-PRINT, M-AVOID-STATICS; Google: no code before `main`)

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
* Google (rule R25): "do not cause code to run before `main()`", for example through the `ctor`
  crate: no `#[ctor` (0 hits) and no `ctor` dependency (the only text matches in the manifests are
  feature names such as `plugin-extractor`). Met.
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

### F22. Panics that are caught, locks that are recovered (M-PANIC-CONTINUATION, M-PANIC-IS-STOP; Google: no `catch_unwind`, handling poison)

* The three guides agree. M-PANIC-CONTINUATION: library code "should not attempt to catch a panic
  and continue execution". M-PANIC-IS-STOP: it is not valid to "assume panics will be caught, even
  by your own code". Google: `panic::catch_unwind` "is prohibited in first-party Rust code", except
  as a defense in depth that is never relied upon, and one must "never reach inside a poisoned
  mutex to get at its insides, no matter how tempting the use case may seem".
* Production `catch_unwind`: three sites. `src/api/runner.rs:179` is the test runner: it catches the
  panic of a failing test, takes the evidence, closes the browser, keeps the payload and raises it
  again (`Ended::finish`; its test says "the panic is raised again", `:337-338`), which is the shape
  M-PANIC-CONTINUATION allows. `src/api/pdf.rs:164` (`guarded`) turns a panic inside the PDF
  parser, which has paths that panic on a malformed file (its doc, `:156-160`), into
  `SeleniumBaseError::Pdf` and goes on. `src/sb_cdp/plugin_hooks.rs:343` turns the panic of a
  plugin task into a plugin error and goes on. The other 8 lines with `catch_unwind` (11 in all)
  are the import `src/api/pdf.rs:29` and 7 tests: `src/api/runner.rs:337`,
  `src/plugins/observer.rs:652` and `src/testing/lifecycle.rs:985` were read;
  `src/testing/lifecycle.rs:1074`, `tests/chrome/bdd_chrome.rs:73`,
  `tests/offline/bdd_scenarios.rs:179` and `tests/offline/reporting_run.rs:414` are tests by their
  files.
* Poisoned locks: 34 lines with `PoisonError::into_inner` in `src` (for example
  `src/artifacts.rs:97,106,154,432`, `src/browser/reuse.rs:125,134,372,381,389`,
  `src/common/shutdown.rs:314`, `src/config/proxy_list.rs:198`, `src/core/allure.rs:450`,
  `src/api/recorder/output.rs:189`, `src/cli/plugin_cmd.rs:939`); `.lock().unwrap()` stands on 84
  lines, tests included and not split. The code does not say why it recovers; the probable reason
  is the one D11 keeps, that the runner goes on after a failed test, so the process-wide registries
  (cleanup list, session reuse, test logs) must still answer for the next one.
* Fix, taken as D11 and D12: no new `catch_unwind`. The three sites stay, each with a one-line
  comment that says why, and `disallowed-methods` (F1) makes a fourth one fail the gate. Poison
  goes through one crate-private helper in `common` that locks and, on poison, returns the guard:
  the single place where the guide's rule is knowingly broken, with the reason written once. The 34
  sites call it, and `std::sync::PoisonError::into_inner` is disallowed everywhere else.
* Breaking: no.

### F23. Wrappers in public signatures (M-AVOID-WRAPPERS, M-SERVICES-CLONE; Google: avoid `Rc` and `Arc`, generics vs `dyn`) [done, except `Session<S>`]

* Done: `TestRun` is a `Clone` handle (`TestRun::current`, `open`, `resume` return it; `RunFinalizer::new`, `ReportsPlugin::with_run`, `Sb::report_to` take it); each boxed alias carries a doc line that says why it is boxed. Not done: `Session<S>` is still the Tokio guard type (its doc says so).

* The first pass wrote "no `Arc`, `Rc` or `Box` in the public signatures read"; the third pass
  found these. Two public constructors take `Arc<TestRun>`: `with_run` (`src/plugins/reports.rs:221`)
  and `new` (`src/core/test_run.rs:1470`). Public aliases name a boxed closure: `Clock` and
  `IdSource` (`src/core/allure.rs:85,88`), `Console` (`src/core/test_run.rs:96`) and `Installer`
  (`src/mcp/dev/cli.rs:32`) are `Arc<dyn Fn ..>`, and `ProgressCallback` (`src/mcp/client.rs:182`) is
  `Box<dyn FnMut ..>`; three name a boxed future (`InstallFuture`, `src/mcp/dev/cli.rs:25`;
  `ResourceFuture`, `src/mcp/resources.rs:130`; `ToolFuture`, `src/mcp/tool.rs:429`); and
  `Session<S>` (`src/mcp/host.rs:180`) is a Tokio guard type, `OwnedMappedMutexGuard<Option<S>, S>`,
  which also leaks a dependency's type (M-DONT-LEAK-TYPES).
* The rules: M-AVOID-WRAPPERS keeps `Rc`, `Arc` and `Box` out of public APIs unless the pointer is
  fundamental to the API or benchmarks justify it; M-SERVICES-CLONE wants a heavy service to be a
  cheap `Clone` handle that shares its state, as `Browser` and `Page` are; Google prefers unique
  ownership and borrowing to `Rc` and `Arc` unless that would be "unreasonably complex", a judgment
  call for review (`src` has 168 lines with `Arc<` and 1 with `Rc<`, `src/api/html.rs:35`, not
  judged site by site), and, in libraries, generics to `dyn Trait`.
* Fix: `TestRun` becomes a `Clone` handle that holds its own `Arc` (if it is not one already), so
  the two constructors take a `TestRun`; `Session<S>` is hidden behind a type of our own. The
  closure and future aliases stay: a callback that a struct stores has to be boxed, or the struct
  takes a type parameter; each alias gets a doc line that says so.
* Breaking: yes, small.

### F24. `unsafe` in tests without a safety comment (M-UNSAFE; Google: unsafe code)

* The library has no `unsafe`. Test code has 11 `unsafe` blocks: `src/api/master_qa/wrapper_tests.rs:36,48`,
  `tests/chrome/masterqa_chrome.rs:81,129,139,156`, `tests/offline/sb_reports_env.rs:26,28,30,32`
  and `tests/no_sandbox_env.rs:31`. Ten are one-line `std::env::set_var` or `remove_var` calls
  (both are `unsafe` in edition 2024); the eleventh has a comment about the environment, and it is
  the only one with a safety comment (`:30`: "this binary has one test, and nothing else reads the
  environment meanwhile"). The first pass counted `src` only and found two.
* Google: "add a safety comment starting with `SAFETY:` that explains why the code is correct", and
  a technical justification; its other rules (a safe abstraction, a separate crate) do not apply,
  since the library has no `unsafe`. M-UNSAFE asks for a valid reason, and the lint
  `undocumented_unsafe_blocks` is in the F1 table and will list the ten.
* Fix: a `// SAFETY:` line on each of the ten (lane 3). F20's `logs_root_with(lookup)` takes the
  environment out of most of these tests, which removes the blocks (lane 7).
* Breaking: no.

## In progress elsewhere

Not judged here: driver errors (`SeleniumBaseError::WebDriver`, `src/error.rs:76`), network
conditions (`#![expect(deprecated)]` for `NetworkConditions`, `src/api/base_case.rs:1-6`), the
DevTools client (`evasion::cdp::CdpClient`, `BrowserSession::cdp*`), capability types
(`browser::options::capabilities`), and the `drivers` module (`src/drivers.rs`). Findings that
depend on them: F4 large, F10, and M-DONT-LEAK-TYPES. `Element::as_webdriver_element` is an
escape hatch by design and counts as M-ESCAPE-HATCHES met.

## Fix lanes

In this order; each is one concern, with the files it mostly touches and the guides it serves (C the
Rust API Guidelines, M Pragmatic Rust, G Google's guide). None is started. Every
lane ends with the repository's gate (`cargo fmt --check`, clippy with `-D warnings`, the tests,
`cargo doc` with `-D warnings`); a breaking lane also updates the `tests/contract_*.rs` files in
the same commit and runs `mise run contracts` and `mise run semver`. The order puts first the
lanes that cost little and tell the rest (1 to 4), then the lanes that break little or nothing
(5 to 9), then the breaking lanes from the smaller to the larger (10 to 20), the history
clean-up (21) and, last, the formatting-only commit (22). The owner's decisions of 2026-11-07 shaped
the list: D2 removed the lane that renamed the `get_*` of `BaseCase` (reversed on 2026-11-08: lane 15 renames them now), D3 made the macro lane a
deletion, D4 and D9 fixed the error lanes, D5 the trait lane, and D7 the last lane; D10 to D12 added
or filled lanes 9, 14 and 15.

1. **The lint set for the whole workspace through `[workspace.lints]`** (F1, F19; M-STATIC-VERIFICATION,
   M-LINT-OVERRIDE-EXPECT; G: lints it names, `missing_docs`, `expect` over `unwrap`).
   Files: `Cargo.toml`, `crates/cli/Cargo.toml`, `crates/playwright/Cargo.toml`,
   `crates/own-imports/Cargo.toml`, `src/lib.rs`, `clippy.toml`. Not breaking. Output: the exact
   lists for lanes 2 to 4 and 8 (`missing_debug_implementations`, `missing_errors_doc`,
   `missing_panics_doc`, `unwrap_used`, `missing_docs`, `undocumented_unsafe_blocks`). Adds the
   `disallowed-methods` entries of D11 and D12 (F22) and the Google lints the table lacks. Delete
   the five attribute blocks and the misplaced comment in `src/lib.rs` in the same commit; put
   `#[expect(clippy::pedantic, reason = "lane N")]` on each module that is not yet clean, so a
   later lane finds its line by the `unfulfilled_lint_expectations` warning.
2. **`Debug`, `Hash` and `Send + Sync` pins** (F2, F14; C-DEBUG, C-COMMON-TRAITS, C-SEND-SYNC;
   M-PUBLIC-DEBUG, M-TYPES-SEND; G: important traits). Files: `src/api/base_case.rs`,
   `src/browser/session/mod.rs`, `src/browser/config.rs`, `src/api/chart.rs`, whatever the lint
   lists, one new test file. Not breaking.
3. **Doc gaps and safety comments** (F3, the doc half of F21, F24; C-EXAMPLE, C-FAILURE;
   M-CANONICAL-DOCS, M-MODULE-DOCS, M-UNSAFE; G: documentation, unsafe code). One example per type
   and per family of methods, not per method (rule R18); `# Errors` and `# Panics` from the lint
   output; a `// SAFETY:` line on the ten test blocks that lack one. Files: `src/api/base_case.rs`,
   `src/browser/config.rs`, `src/common/mod.rs`, `src/config/mod.rs`, `src/utils/mod.rs`,
   `src/utilities/mod.rs`, `src/lib.rs`, `src/macros.rs` (fences), `src/plugins/`,
   `src/utilities/selenium_ide.rs`, `src/profile_payloads/profile.rs`, `src/browser/launcher.rs`,
   `src/browser/virtual_display.rs`, the test files of F24. Not breaking.
4. **Manifest and tooling** (F13, F1 rustfmt; C-METADATA; M-CARGO-WORKSPACE; G: no unstable
   features): metadata, `[workspace.dependencies]`, `error-backtrace` and the three pages that
   describe it wrongly. D1 is taken (the MSRV stays the newest stable) and D7 moved to lane 22.
   Files: the four manifests, `README.md`, `CONTRIBUTING.md`, `docs/building.md`. Breaking only for
   the feature.
5. **Keep the source in errors** (F4 small; C-GOOD-ERR; M-FROM-ERROR; G: reporting errors):
   `sb_context`, the two `From` impls, `#[non_exhaustive]` on the plugin errors, and typed errors
   that display the text of today for `find_examples`, `SbRecorder::check`, `check_in`,
   `validate_file_name`, `parse_ide_file` and `parse_ide_html` (D9, taken); `parse_ide_html`, which
   cannot fail, returns its value. Files: `src/error.rs` and its callers,
   `src/cli/scripts/all_scripts.rs`, `src/cli/scripts/sb_recorder.rs`,
   `src/utilities/selenium_ide.rs`. Low breaking.
6. **Print, glob, `mod.rs`, small hygiene** (F17, F12 glob and non-leaf modules; M-LOG-NOT-PRINT,
   M-NO-GLOB-REEXPORTS; G: glob imports, non-leaf modules, name files after the primary item):
   `tracing` for the three prints, the glob re-export replaced, and the 13 types of 7 `mod.rs`
   files moved to files named after them and re-exported. Files: `src/api/master_qa.rs`,
   `src/browser/driver_access.rs`, `src/mcp/http.rs`, `src/profile_payloads/mod.rs`, the 7 `mod.rs`
   files of F12. Not breaking.
7. **The logging plugins and the path helpers take their inputs** (F20; C-METHOD;
   M-MOCKABLE-SYSCALLS): an upload target per store with `from_env()`, `logs_root_with`,
   `find_system_chrome_with`, a directory for the cookie-file methods. Files:
   `src/plugins/s3_logging_plugin.rs`, `azure_logging_plugin.rs`, `gcp_logging_plugin.rs`,
   `src/artifacts.rs`, `src/evasion/patcher.rs`, `src/api/base_case.rs`,
   `src/api/base_case_impls/base_case_impl_parity.rs`. Breaking only for the three plugin types.
8. **`Drop` that blocks or writes** (F21, the code half; C-DTOR-BLOCK): `spawn_blocking` for the
   `wait`, an explicit `finish` for `TestRecorder`, and the check of the 16 `impl Drop` not read.
   Files: `src/browser/launcher.rs`, `src/browser/virtual_display.rs`,
   `src/browser/session/lifecycle.rs`, `src/core/test_run.rs`. Not breaking.
9. **Panics that are caught, locks that are recovered** (F22; M-PANIC-CONTINUATION,
   M-PANIC-IS-STOP; G: no `catch_unwind`, handling poison; D11 and D12): a one-line comment and
   an `#[expect(clippy::disallowed_methods, reason = "...")]` on the three production
   `catch_unwind`, the crate-private lock helper in `common`, the 34 lock sites on it. Files:
   `src/api/runner.rs`, `src/api/pdf.rs`, `src/sb_cdp/plugin_hooks.rs`, `src/common/` (the
   helper), and the lock sites (`src/artifacts.rs`, `src/browser/reuse.rs`,
   `src/common/shutdown.rs`, `src/config/proxy_list.rs`, `src/core/allure.rs`,
   `src/api/recorder/output.rs`, `src/cli/plugin_cmd.rs`, and the rest of the 34). Not breaking.
10. **Sealed traits and no `Deref`** (F6; C-SEALED, C-DEREF; M-ESSENTIAL-FN-INHERENT; G: trait vs
    inherent impl, `foo_mut`). Files: `src/api/traits.rs`, `src/error.rs`, `src/api/membership.rs`,
    `src/api/master_qa.rs`, `src/testing/bdd/mod.rs`, their tests. Breaking.
11. **Wrappers out of public signatures** (F23; M-AVOID-WRAPPERS, M-SERVICES-CLONE; G: avoid `Rc`
    and `Arc`): `TestRun` as a `Clone` handle, `Session<S>` hidden, a doc line on each boxed alias.
    Files: `src/core/test_run.rs`, `src/plugins/reports.rs`, `src/core/allure.rs`, `src/mcp/host.rs`,
    the call sites. Breaking, small.
12. **`non_exhaustive` and the unconditional `plugins` field** (F8; C-STRUCT-PRIVATE, C-VALIDATE;
    M-FEATURES-ADDITIVE; G: `non_exhaustive` with its record-update exception): on `BrowserConfig`,
    `Cookie`, `Browser`, `DriverMode` and the `api`, `utilities` and `profile_payloads` types of F8,
    and not on `Settings`, `BrowserSettings`, `ProxyConfig` and the other tables built with
    `..Default::default()`. Files: `src/browser/config.rs`, `src/cookie.rs`, the types of F8, and
    every struct literal (use `constructors` to list them first). Breaking.
13. **Booleans to enums, paths to `AsRef<Path>`, keywords to enums, the tuple alias** (F9 without
    selectors; C-CUSTOM-TYPE, C-GENERIC, C-NEWTYPE; M-IMPL-ASREF, M-STRONG-TYPES; G: enum over bool,
    flexible references, named fields). Files: the sites in F9, `src/cli/scaffold.rs`. Breaking.
14. **Macros** (F7; C-MACRO-ATTR; M-MACRO-LAST-RESORT, M-MACROS-DONT-LIE; G: macros), with D3
    taken: delete every `sb_*` macro but `sb_test!`, delete `uc_config!` and `cdp_config!`, read
    `selector!` and `fingerprint!` and delete either if a function does the job. Files:
    `src/macros.rs`, `src/lib.rs`, `tests/`, `examples/`, `docs/`. Breaking.
15. **Names** (F11, F5 cheap part, F15; C-GETTER, C-CTOR, C-WORD-ORDER; M-SHORT-NAMES,
    M-WEASEL-WORDS; G: getters and setters, initialization functions, avoid abbreviations):
    `BrowserKind`, one `Proxy`, `connect`, `try_new` for the fallible `new`, `as_str` for the four `name`,
    `get_browser` and the `*_js` getters, `with_header` and `with_body`, the six abbreviated
    `BrowserConfig` setters and the accessor `rec_sb_cdp`. The `BaseCase::get_*` names go too (D2 reversed); the `new_with_*` renames are dropped (D10); builder setters move to the bon builders lane.
    Files: `src/browser/config.rs`, `src/lib.rs`, `src/evasion/fingerprint.rs`,
    `src/browser/options/`, `src/config/proxy_list.rs`, `src/profile_payloads/profile.rs`,
    `src/api/base_case.rs`, `src/browser/chrome_for_testing.rs`, `src/sb_cdp/intercept.rs`, the
    `new` that return a `Result` (F15). Breaking. Use `tokensave_rename`.
16. **Trait receivers, parameter order and native async** (F16; M-ASYNC-FN,
    M-PARAMETER-CONSISTENCY; G: generics vs `dyn`), with D5 taken: native `async fn` in the four
    traits where nothing uses them as `dyn`. Files: `src/api/traits.rs` and the `BaseCase` and
    `Page` impls. Breaking.
17. **Public module tree** (F12; M-SINGLE-ITEM-PATH, M-BALANCED-MODULES): `pub(crate)` pass, one
    path per item. Files: `src/lib.rs`, `mod.rs` files, the contract tests. Breaking; run
    `mise run semver` and the contract tests. The crate split is not part of it: D6 designs it in
    `docs/design/core_split.md`.
18. **Error redesign** (F4 large; M-ERRORS-CANONICAL-STRUCTS; G: reporting errors), with D4
    taken: the canonical struct, `Skipped` out of the error type, after the thirtyfour driver-errors
    lane. Files: `src/error.rs` and every constructor call. Breaking, large.
19. **Selector typing** (F9 selectors; C-NEWTYPE; M-STRONG-TYPES; G: the code-size caveat of
    `Into`): `BaseCase` on `impl Into<SelectorBuf>`, each generic function kept to the conversion
    and a call to a non-generic body. About 371 signatures (public or not); do it with
    `ast_grep_rewrite`. Breaking, large.
20. **`panic` to `Result` for `driver()`** (F10; M-PANIC-ON-BUG; G: panic for bugs), after the
    drivers lane. Files: `src/browser/session/mod.rs`. Breaking.
21. **Docs without history** (F18; M-NO-META-DESIGN-DOCUMENTATION). Files: wherever the search
    lists. Not breaking.
22. **`style_edition = "2024"`** (D7, taken, later; G: the formatting tool decides the layout):
    one formatting-only commit after every other lane has merged, so it conflicts with none of
    them. Files: `rustfmt.toml` and what `cargo fmt` changes. Not breaking.

## Decisions for the owner

All twelve are taken (2026-11-07). D1 to D3 and D5 to D8 are the owner's own; D4 and D9 to D12 were
taken on the recommendation written under each.

* D1. **Taken (owner): the MSRV stays the newest stable.** `rust-version = "1.99"`; the `msrv` CI job
  and the `Dockerfile` base image move with it, as `CLAUDE.md` says. M-MSRV is an accepted deviation.
* D2. **REVERSED (owner, 2026-11-08): `BaseCase` getters drop `get_`** (`get_text` becomes `text`; 78
  of the 104). The Python names stay findable through `parity/api.toml`, the Python importer and
  `parity_lookup`. All 104 `get_*`, the four `name` methods and `get_browser` are renamed in lane 15.
  Direction: the pure CDP engine (`sb_cdp`) is the Rust-native API and `BaseCase` is the
  Python-familiar layer built on it, also Rust-named; the rule set applies to every public API in
  every repository (core, playwright, the stealth crates, plugins, the Tauri plugin).
* D3. **Taken (owner): cut the `sb_*` macros to `sb_test!`.** The four macros not named `sb_*`
  follow the same rule on the recommendation of this page, which the owner accepted: `uc_config!`
  and `cdp_config!` go, and `selector!` and `fingerprint!` are kept only if a function cannot do
  their job (lane 14).
* D4. **Taken (recommendation accepted): `Skipped` leaves the error type and the canonical error
  struct is adopted** (a struct with a `kind()` enum, a source and a hint), after the thirtyfour
  driver-errors lane (lane 18). Why: a skipped test is an outcome, not a failure, and the three
  guides ask for a typed error with a source (Google: `Result` is for failures the caller has to
  tell apart). Cost: breaking and large; every `matches!` on `SeleniumBaseError` and every
  constructor call changes, and `is_skipped` moves to the test outcome.
* D5. **Taken (owner): native `async fn` in the four capability traits where nothing uses them as
  `dyn`.** `dyn BrowserApi` occurs nowhere in the repository; lane 16 searches the other three
  first and keeps `#[async_trait]` only on a trait that is used as `dyn`.
* D6. **Taken (owner): the crate split is designed in `docs/design/core_split.md`** (not on this
  branch). This page does not repeat it and lane 17 does not split crates. M-SMALLER-CRATES and
  M-CRATES-FLAT-FOLDER are decided there; Google's two crate rules are Chromium's.
* D7. **Taken (owner): `style_edition` moves to 2024, later**: lane 22, a formatting-only commit
  after every other lane.
* D8. **Taken (owner): the deliberate re-exports (`tokio`, `json`, `testing::rstest`, `drivers`)
  are an accepted deviation from M-FOREIGN-REEXPORTS.** The imports note in `CLAUDE.md` is the
  reason; this audit lists them so the choice is written down.
* D9. **Taken (recommendation accepted): typed errors that display the same text.** The message
  stays Python's (`SbRecorder::check`, `validate_file_name`, `find_examples`); a type carries it, so
  `?` works and a caller can match (lane 5).
* D10. **REPLACED (owner, 2026-11-08): builder setters lose `with_` and are named after the field on
  a separate builder type** (`BrowserConfig::builder().headless(true).build()`, C-BUILDER, agreeing
  with IRS's builder idiom), done by the bon builders lane; `build()` still validates (R5);
  `with_*` stays only for constructors, as in std's `Vec::with_capacity`; the `new_with_*` rename of
  F15 is dropped. The text below is the earlier decision, kept for the record.
  Earlier text: keep `with_*` as the setter that takes and returns
  `self`. Google's guide gives `with_foo(f)` the meaning "call `f` with a borrow of `foo`" (and
  `with_mut_foo(f)` for a mutable borrow), for a property inside a `Mutex` or `RefCell` or when a
  change needs follow-up work that `set_foo` cannot do. The crate and `seleniumbase-rs-stealth` use
  `with_*` for builder setters (`BrowserConfig::with_*`, `RunSettings::with_*`,
  `ProxyList::with_rotation`: 153 public items in `src`).
  Option A, keep: nothing is renamed, and no `with_*` here takes a closure to read or change a
  property, so the two meanings never meet in one type. The cost is that a reader who knows
  Google's meaning has to learn ours, and a closure accessor, if one is ever needed, cannot be
  called `with_*`. Option B, adopt Google's meaning: the 153 setters need another name, and the
  candidates collide: `foo(self, v)` clashes with the getter `foo()` that Google and C-GETTER ask
  for on the same property, and `set_foo(&mut self, v)` ends the chaining that every example, doc
  page, test and the stealth crates use (1,327 lines of the repository contain `.with_`, an upper
  bound that includes other crates' methods). B renames the public API of several repositories at
  once and ends with a worse name. Recommendation: A, with three rules: `with_*` is only a setter
  that consumes and returns `self` (the five `with_*` constructors become `new_with_*` and
  `set_header` and `replace_body` become `with_header` and `with_body`: lane 15); a closure
  accessor is never named `with_*`; the exception stands in the rule table (R5).
* D11. **Taken (recommendation accepted): `catch_unwind` stays at the three production sites, and
  nowhere else.** The runner's use ends in a re-raised panic, the shape the three guides allow; the
  PDF guard and the plugin-task guard turn the panic of third-party code into an error and go on,
  which the guides discourage, and the alternatives are worse: a parser that panics on a malformed
  file cannot be fixed from here, and a plugin task is someone else's code. Each site gets a
  one-line comment and `disallowed-methods` makes a fourth one fail the gate (lanes 1 and 9). Cost:
  three documented deviations from three guides. The alternative, running the parser and the plugin
  tasks in a child process so that a crash is a dead process and not a caught panic, is much more
  code for the same result.
* D12. **Taken (recommendation accepted): poison is recovered in one place.** A crate-private helper
  in `common` locks and, on poison, returns the guard; the 34 sites call it and
  `PoisonError::into_inner` is disallowed elsewhere. It is still a deviation from Google's rule,
  but it is one, with its reason written once, where the 34 are now unexplained. The reason
  (probable, the code does not say): the runner goes on after a failed test (D11), so the
  process-wide registries must still work for the next one. The alternative is a non-poisoning mutex
  from a crate, a new dependency (the manifests have none); it can replace the helper later without
  touching the 34 sites.
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


## What the third pass changed

The third pass (2026-11-07, on the tree of `2327ffb8`) read Google's Rust style guide and searched the
code for each of its rules, and re-read five rules of Pragmatic Rust (M-PANIC-CONTINUATION,
M-PANIC-IS-STOP, M-AVOID-WRAPPERS, M-SERVICES-CLONE, M-SHORT-NAMES). No cargo command was run.

Corrected (2), both claims of the first pass:

1. M-AVOID-WRAPPERS, "no `Arc`, `Rc` or `Box` in the public signatures read": wrong for `core`,
   `plugins` and `mcp`, where 2 constructors and 8 aliases carry them (F23). Met moved to partly.
2. M-UNSAFE, "two calls in test code": the count covered `src` only; `tests/` has 9 more blocks
   (F24).

Moved: M-PANIC-CONTINUATION and M-PANIC-IS-STOP from not checked to partly (their text is quoted in
F22); M-MSRV and M-FOREIGN-REEXPORTS from not met to accepted (D1, D8); M-SHORT-NAMES stays not
checked, but it was read and it conflicts with Google on abbreviations (R1).

Added: the rule table (R1 to R31); the guides each finding comes from, in every finding heading and
in the ten that matter most; three findings (F22 to F24); lanes 9, 11 and 22; and the Google rules
merged into F1 to F13 and F15 to F17. What Google's guide asks and the other two do not: no
`catch_unwind` and no recovered poison (F22), the record-update exception to `non_exhaustive` (F8),
`new_with_foo` and `try_new` (F15), `foo_mut`, `take_foo` and `replace_foo` (F5), no abbreviations
that delete letters (F11), non-leaf modules without definitions (F12), named fields over tuples
(F9), and `SAFETY:` comments (F24).

Decisions: the owner took D1 to D3 and D5 to D8, and accepted this page's recommendation for D4 and
D9 to D12; they are marked taken above, and their work is in lanes 1, 5, 9, 14, 15, 16, 18 and 22.

## The fourth source

On 2026-11-08 the owner added Idiomatic Rust Snippets (`IRS:`, github.com/saltukalakus/idiomatic-rust-snippets, read on Context7). It has no ids; a rule is named by the title of its snippet. It tags R4, R5, R6, R8, R11, R14, R15 and R28, adds R32 and R33 (status not checked: the code is not judged against them yet; that is the next audit pass), and no longer conflicts with R5 after the owner's D10 replacement (plain setters; `build()` still validates). Its snippets on builders, `new`, `&str`, cloning, iterators, `Display`/`Error`/`From` and `?` agree with the other three; its remaining snippets are language lessons, not API rules.

A second round of four queries (newtype, typestate, RAII and `Drop`, extension traits and conversion traits) tagged R7 (newtype pattern), R11 (extension traits) and R27 (RAII) and added R34 (typestate) and R35 (RAII owners and guards), both not checked. What the source did not return for the fourth query: `Default`, `From`/`Into` as a parameter rule, `AsRef`, `Cow` and sealed traits (only the `From` between newtype wrappers and a default associated type, which is unstable, came back). The crate's rules on those stay with the other three guides.

A third round (design patterns, three queries) tagged R25 (`OnceLock` lazy initialization), R28 (observer by channel) and R29 (adapter, facade, proxy) and added R36 (observer by channel), R37 (avoid `Box<dyn>` factories and strategies) and R38 (facade and adapter), all not checked. That list was incomplete: Context7 had missed pages. A direct read of the repository (`gh api`, pages under `src/patterns` and `src/anti-patterns`) in a fourth round covered every pattern and anti-pattern it holds: the creational, structural and behavioural pages (singleton, abstract method, chain of responsibility, enum polymorphism, mediator, message passing, visitor, bridge, decorator, flyweight) are tagged to R5, R11, R25, R28, R36 and R37 or have R39, and each anti-pattern is tagged or has a row (R40 to R44; stringly typed to R7 and F9, deref abuse to R11 and F6, panic in libraries to R15, R33 and F22, unnecessary smart pointers to R28 and F23, unnecessary unsafe to R17). The rust-unofficial Rust Design Patterns book was queried, then dropped at the owner's correction, and is not a source of this page.

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
* Google's guide, what was not checked: how types and functions are imported (unqualified types,
  module-qualified functions, trait imports scoped to a function), type inference and turbofish,
  assertion messages, the length of iterator chains (3 `for_each` found), `format!` against
  `str::replace` (42 `.replace("` lines, 8 read), lifetime and generic-parameter names,
  `#[must_use]` (1,065 sites, none judged), which `Clone` and `Copy` impls are missing, `.clone()`
  used to dodge a borrow, `&[T]` parameters that could take an iterator, functions that return a
  `Vec` where an iterator would do, whether each of the 168 `Arc<` lines is needed, and how big the
  definitions in the 7 `mod.rs` files are. The Chromium, Android and Fuchsia rules are n/a.

## Sources

* [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
* [Pragmatic Rust Guidelines 2026.6](https://microsoft.github.io/rust-guidelines/) (all guidelines
  in one file: `agents/all.txt`, read again for the third pass)
* [Google's Rust style guide](https://github.com/google/styleguide/blob/gh-pages/rustguide.md)
  (`/google/styleguide` on Context7; the file fetched from the raw page and read on 2026-11-07)
* [Idiomatic Rust Snippets](https://github.com/saltukalakus/idiomatic-rust-snippets) (`IRS:`;
  `/saltukalakus/idiomatic-rust-snippets` on Context7, three queries on 2026-11-08: API design and
  naming, error handling, builders and iterators; only the snippets those returned were read; four more queries on 2026-11-08 for the second round)
* Cargo reference, workspaces and the `lints` table; the Rust Reference on lint attributes; the
  Clippy lint configuration book (all read on Context7).
