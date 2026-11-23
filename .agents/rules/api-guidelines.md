---
description: "Comprehensive Rust API guidelines, design rules (R1-R50), and Python parity enforcement"
globs: ["**/*.rs", "parity/*", "docs/*", "crates/**/*"]
always_on: true
---

# Comprehensive Rust API Guidelines & Design Rules (R1 - R50)

This repository follows the strict unified ruleset established in `docs/design/api_guidelines_audit.md` (combining Rust API Guidelines, Microsoft Pragmatic Rust Guidelines, Google Rust Style Guide, Idiomatic Rust Snippets, and ANSSI Secure Rust Guidelines). All coding agents MUST adhere to these rules without exception.

---

## 1. Naming & Conversions (R1 - R3, R10, C-CASE, C-CONV, C-GETTER)

- **R1 (Naming & Vocabulary):** Names follow standard library style in one word order (`verb_noun`), short, clear, with no obscure abbreviations. Do not delete letters within words (e.g., use `recording`, not `rec`; `web_security`, not `ws`).
- **R2 (Conversions):** 
  - `as_` for cheap borrowed views (`as_str()`, `as_bytes()`, `as_webdriver_element()`).
  - `to_` for costly conversions/allocations (`to_string()`, `to_path_buf()`).
  - `into_` for consuming self conversions (`into_string()`, `into_inner()`).
- **R3 (Getters and Setters - STRICT C-GETTER / Decision D2):**
  - **NO `get_` PREFIX:** A getter is `foo()` or `foo_mut()`, NEVER `get_foo()`.
  - Setters are `set_foo(T)` on mutable handles, or named after the field on builders.
  - `take_foo()` clears and returns the old value; `replace_foo(T)` sets and returns the old value.
  - Python SeleniumBase `get_*` methods (`get_text`, `get_title`, `get_current_url`, `get_cookies`, `get_attribute`, `get_value`, etc.) are mapped to Rust methods WITHOUT `get_` (`text`, `title`, `current_url`, `cookies`, `attribute`, `value`, etc.).
  - NEVER introduce redundant `get_*` aliases that violate idiomatic Rust conventions.
- **R10 (Standard Conversion Traits):** Implement `FromStr`, `From`, `Display`, `Default`, and `IntoIterator` rather than custom look-alike methods (e.g. `from_str` or `to_str` methods).

---

## 2. Types, Constructors & Builders (R4 - R7, R13, R34, C-CTOR, C-BUILDER, C-NEWTYPE)

- **R4 (Constructors):** Constructors are `new()`, `new_with_*()`, or `try_new()` when fallible returning a `Result`. A constructor does what its name says; an options struct implements `Default`.
- **R5 (Builders):** Builders are separate types (`FooBuilder`) whose setters are named after the field (no `with_` prefix), taking and returning `self`. The `.build()` method validates invariants and returns `Result`. `with_*` is reserved strictly for constructors (like `Vec::with_capacity`), not builder setters.
- **R6 (Caller Control & Parameter Types):** Parameters take the widest borrowed or generic form: `&str`, `&[T]`, `&T`, `Option<&T>`, `impl AsRef<Path>`, and `impl Into<Selector>` where callers benefit. Parameter order must remain consistent across related methods.
- **R7 (Strong Typing over Strings & Bools):** Use dedicated enums or newtypes instead of bare `bool` flags or primitive `String`s when a parameter has semantic meaning (avoid stringly-typed APIs).
- **R13 (Future Extensibility & Struct Privacy):** Keep fields private or annotate types with `#[non_exhaustive]` on public structs and enums that may grow. Structs initialized with `..Default::default()` stay exhaustive with `Default`. Features must be purely additive.
- **R34 (Typestate Pattern):** Encode strict lifecycles or mandatory builder parameters into the type system where invalid operations can be prevented at compile time.

---

## 3. Standard Traits & Trait Design (R8, R9, R11, R12, R32, R39)

- **R8 (Iterators):** Functions return iterators unless building a collection is necessary or callers require ownership. Collection types implement standard collection traits.
- **R9 (Standard Derives):** Public types derive standard traits: `Debug`, `Clone`, `Default`, `PartialEq`, `Eq`, and `Hash` (for map keys). Handles and state types must be `Send + Sync`.
- **R11 (Trait Design & Sealing):** Traits exist for generic programming or necessary seams (`CdpTransport`). Traits not intended for external implementation MUST be sealed. NEVER abuse `Deref` for inheritance/polymorphism.
- **R12 (Native Async Trait Methods):** Trait methods use native `async fn` or return `impl Future<Output = ..> + Send` unless the trait genuinely requires `dyn` object safety.
- **R32 (Associated Types):** Use associated types when an implementor picks one related type; use generic parameters only when a type may implement a trait multiple times.
- **R39 (Enum Polymorphism):** Use closed enums and `match` instead of open trait objects or visitor patterns for closed sets of kinds.

---

## 4. Error Handling & Panics (R14 - R16, R33, C-GOOD-ERR, M-PANIC-*)

- **R14 (Typed Errors):** Recoverable failures MUST return a typed `Result<T, SeleniumBaseError>` preserving the underlying source (`#[source]`). NEVER return `Result<_, String>` or unconstrained `Box<dyn Error>`.
- **R15 (Panic Discipline):** Panics report bugs or violated internal invariants, never expected external states. In production code, use `.expect("reason")` over `.unwrap()`, and document all panic conditions in `# Panics`. `unwrap()` is restricted to tests.
- **R16 & F22 (No Panic Continuation / Lock Recovery):** Production code must not catch panics and continue execution (`catch_unwind` is prohibited outside specific, documented runner boundaries). Never silently recover poisoned mutexes (`PoisonError::into_inner`).
- **R33 (Library Exit Prohibition):** Library code in `src/` must NEVER call `std::process::exit` or print errors directly to stdout/stderr. CLI process exits belong exclusively in `crates/cli`.

---

## 5. Memory, RAII, Concurrency & Security (R17, R27, R28, R35 - R37, R45 - R47)

- **R17 & F24 (Safe Code First):** No `unsafe` in library code. Any unavoidable `unsafe` in tests MUST have an explanatory `// SAFETY:` comment justifying safety invariants.
- **R27 & R35 (Non-blocking RAII Drop):** `Drop` must neither block nor fail silently. Async cleanup is spawned onto tasks (`tokio::spawn` or graceful shutdown registry).
- **R28 & R37 (No Smart Pointers in Public Signatures):** Do not expose `Arc<T>`, `Rc<T>`, or `Box<dyn Trait>` in public API signatures. Services and client handles should be lightweight, cheap-to-clone structs wrapping internal primitives.
- **R36 (Event Channels over Observer Callbacks):** Event streams and observers use channels (`tokio::sync::mpsc`), where dropping the receiver unsubscribes, rather than `Arc<dyn Observer>` callback lists.
- **R45 (Arithmetic Safety):** Arithmetic with user or external input that could overflow must use `checked_`, `saturating_`, or `wrapping_` methods.
- **R46 (No Leaks by Design):** No `mem::forget`, `Box::leak`, or uninitialized memory in library code.
- **R47 (Explicit Security Erasure):** Sensitive data (credentials, secret tokens, private keys) must be explicitly cleared/zeroized; do not rely solely on implicit `Drop`.

---

## 6. Architecture, Modules, Documentation & Lints (R18 - R26, R48, R49)

- **R18 (Comprehensive Documentation):** Every public item has documentation explaining its purpose and behavior. Include `# Errors` for fallible functions, `# Panics` if it can panic, and `# Examples` for types and method families.
- **R19 (Code-Centric Docs):** Documentation describes current Rust code and behavior, not historical changes or Python quirks (except in explicit parity tables).
- **R20 (Macros as Last Resort):** Prefer functions over macros. Macros must not change return types based on argument count.
- **R21 (Single Item Paths & Clean Modules):** Exactly one canonical path per exported item. No glob imports (`use foo::*`), except `use super::*;` in tests. Non-leaf modules only declare submodules and re-exports.
- **R24 (Workspace Lint Set & Zero Warnings):** Code must pass `cargo clippy --all-targets -D warnings` and `cargo fmt --all -- --check`. Lint overrides require `#[expect(lint, reason = "...")]`.
- **R25 (No Unstructured Output):** The library does not use `println!` or `eprintln!`. All diagnostics and logging use `tracing` (`info!`, `warn!`, `debug!`, `error!`).
- **R26 (Testable Dependency Injection):** Methods accept parameters directly rather than reading hidden environment variables or ambient filesystem state.

---

## 7. Python Parity & Audit Verification Rules

- **Always verify against parity documentation:**
  - Before writing code or claiming a Python SeleniumBase feature is missing, inspect `parity/api.toml`, `docs/parity.md`, or [https://mustcodeal-seleniumbase-rs.docs7.io/parity](https://mustcodeal-seleniumbase-rs.docs7.io/parity).
  - Use the `parity_lookup` and `find_api` tools in the `dev` MCP server (`sbrs-mcp --server dev`).
  - Python features are tracked with their explicit idiomatic Rust equivalents (e.g., Python `get_cookies` is Rust `cookies`).
- **Update generated indexes on API changes:**
  - When changing public methods of `BaseCase` or the CDP engine, always update `src/mcp/dev/api_index.json` using:
    `SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index`
