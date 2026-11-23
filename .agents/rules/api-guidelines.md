---
description: "Comprehensive Rust API guidelines, design rules (R1-R50), and Python parity enforcement"
globs: ["**/*.rs", "parity/*", "docs/*", "crates/**/*"]
always_on: true
---

# Comprehensive Rust API Guidelines & Design Rules (R1 - R50)

This repository strictly enforces the 50 design rules from `docs/design/api_guidelines_audit.md` (synthesizing Rust API Guidelines, Microsoft Pragmatic Rust Guidelines, Google Rust Style Guide, Idiomatic Rust Snippets, and ANSSI Secure Rust Guidelines). All coding agents MUST adhere to these rules without exception.

---

## The 50 Mandatory Design Rules (R1 - R50)

- **R1 (Naming & Vocabulary):** Names follow standard library style, in one word order (`verb_noun`), short, saying what the item is, with no obscure abbreviations. Never delete letters within words (e.g., use `recording`, not `rec`; `web_security`, not `ws`).
- **R2 (Standard Conversions):** Follow standard conversion prefixes: `as_` for cheap borrowed views, `to_` for costly conversions/allocations, `into_` for consuming self conversions. Conversions naming a specific type state it (`as_webdriver_element`, `into_webdriver_element`).
- **R3 (Getters and Setters - STRICT C-GETTER / Decision D2):** A getter is `foo()` or `foo_mut()`, a setter `set_foo(T)`. `take_foo()` clears and returns the old value; `replace_foo(T)` sets and returns the old value. **NO `get_` PREFIX:** Python SeleniumBase getters (`get_text`, `get_title`, `get_current_url`, `get_cookies`, `get_attribute`, `get_value`, etc.) are mapped to Rust methods WITHOUT `get_` (`text`, `title`, `current_url`, `cookies`, `attribute`, `value`, etc.). Redundant `get_*` aliases are strictly prohibited.
- **R4 (Constructors):** Constructors are `new()`, `new_with_*()`, or `try_new()` when fallible returning a `Result`. A constructor does what its name says. An options struct implements `Default`. Value conversions beside a `From` impl are `from_foo()`.
- **R5 (Builders):** A builder is a separate type (`FooBuilder`) whose setters are named directly after fields (no `with_` prefixes), taking and returning `self`. The `.build()` method validates invariants and returns `Result`. `with_*` is reserved strictly for constructors (e.g. `Vec::with_capacity`), never builder setters.
- **R6 (Caller Control & Parameter Flexibility):** Parameters take the widest borrowed or generic form: `&str`, `&[T]`, `&T`, `Option<&T>`, `impl AsRef<Path>`, and `impl Into<Selector>` where callers gain. Keep generic functions focused on conversion and delegating to a non-generic inner body. Parameter orders must remain consistent across related methods (`assert_text`, `type_text`).
- **R7 (Strong Typing over Primitives):** Use named types (enums, newtypes), not primitive `String`s or `bool`s, where values have semantic meaning. Named fields over tuples. Newtypes over aliases when specialized APIs or traits are needed.
- **R8 (Iterators):** Functions return iterators unless building a collection is necessary or callers need ownership. Collection types implement standard collection traits (`IntoIterator`).
- **R9 (Standard Derives):** Public value types derive standard traits: `Debug`, `Clone`, `Default`, `Eq`, `Hash` (for potential map keys), and `serde` where stored. Handles and state types must be `Send + Sync`.
- **R10 (Standard Conversion Traits):** Implement `FromStr`, `From`, `Display`, `Default`, and `IntoIterator` rather than custom look-alike methods (`from_str`, `to_str`). Operators are used only for standard mathematical meanings.
- **R11 (Trait Design & Sealing):** Traits exist for generic programming or necessary seams (`CdpTransport`). Traits not intended for outside implementation MUST be sealed (`C-SEALED`). NEVER abuse `Deref` for inheritance or polymorphism. Libraries prefer generics over `dyn`.
- **R12 (Native Async Trait Methods):** Trait methods use native `async fn` or return `impl Future<Output = ..> + Send` unless the trait is genuinely used as `dyn`.
- **R13 (Extensibility & Struct Privacy):** Keep fields private or annotate types with `#[non_exhaustive]` on public structs and enums that may grow. Structs initialized with `..Default::default()` stay exhaustive with `Default`. Features must be purely additive. `const fn` only if it remains const. `#[doc(hidden)]` is private.
- **R14 (Typed Errors):** Recoverable failures MUST return a typed `Result<T, SeleniumBaseError>` keeping underlying sources (`#[source]`). Use `Option` where cause is obvious. NEVER return `Result<_, String>` or unconstrained `Box<dyn Error>`. One canonical error struct with `kind()`.
- **R15 (Panic Discipline):** Panics report bugs or violated internal invariants, never expected external states. In production code, use `.expect("reason")` over `.unwrap()`, and document all panic conditions in `# Panics`. `unwrap()` is strictly restricted to test code.
- **R16 & F22 (No Panic Continuation / Lock Recovery):** Production code must not catch panics and continue execution (`catch_unwind` is prohibited outside specific, documented runner boundaries). Never silently recover poisoned mutexes (`PoisonError::into_inner`).
- **R17 & F24 (Safe Code First):** No `unsafe` in library code. Any unavoidable `unsafe` in tests MUST have an explanatory `// SAFETY:` comment justifying safety invariants.
- **R18 (Comprehensive Documentation):** Every public item has doc comments explaining what the code does not already say. Include `# Errors` for fallible functions, `# Panics` if it can panic, and `# Examples` for types and method families.
- **R19 (Code-Centric Docs):** Documentation describes current Rust code and behavior, not historical changes or Python quirks (except in explicit parity tables).
- **R20 (Macros as Last Resort):** Prefer functions over macros. Macros must not change return types based on argument count.
- **R21 (Single Item Paths & Clean Modules):** Exactly one canonical path per exported item. No glob imports (`use foo::*`), except `use super::*;` in tests. Non-leaf modules only declare submodules and re-exports.
- **R22 (Crate Structure):** A crate holds one concern; crates sit flat in the workspace folder.
- **R23 (Workspace Manifest & Stability):** Complete package metadata, latest edition, unified workspace manifest, no unstable features (`#![feature]`).
- **R24 (Workspace Lint Set & Zero Warnings):** Code must pass `cargo clippy --all-targets -D warnings` and `cargo fmt --all -- --check`. Lint overrides require `#[expect(lint, reason = "...")]`.
- **R25 (No Unstructured Output):** The library does not use `println!` or `eprintln!`. All diagnostics and logging use `tracing` (`info!`, `warn!`, `debug!`, `error!`). No code runs before `main` (no `ctor`).
- **R26 (Testable Dependency Injection):** Methods accept parameters directly rather than reading hidden environment variables (`std::env`) or ambient filesystem state.
- **R27 (Non-blocking Drop):** `Drop` must neither block nor fail silently. Async cleanup goes to spawned background tasks (`tokio::spawn` or graceful shutdown registry).
- **R28 (No Smart Pointers in Public Signatures):** Do not expose `Arc<T>`, `Rc<T>`, or `Box<dyn Trait>` in public API signatures. Services and client handles are lightweight, cheap-to-clone structs wrapping internal primitives.
- **R29 (Foreign Type Encapsulation):** Foreign types stay out of public APIs, wrapped in our own facade/adapter types with explicit escape hatches.
- **R30 (Idiomatic Implementation):** Use pattern matching over check-then-unwrap; `format!` over string concatenation; list every variant of `non_exhaustive` enums.
- **R31 (No FFI):** No FFI or `-sys` crates.
- **R32 (Associated Types):** Use associated types on traits when an implementor picks one related type; type parameters only when a type implements a trait multiple times.
- **R33 (No Library Process Exits):** Library code in `src/` must NEVER call `std::process::exit` or print errors directly. CLI process exits belong exclusively in `crates/cli`.
- **R34 (Typestate Pattern):** Encode strict lifecycles or mandatory builder parameters into type parameters so invalid states fail to compile. Transitions consume `self`.
- **R35 (RAII Resource Cleanup):** Types owning external resources (browser, driver, session, pool) release them in `Drop`. Scoped state uses guard values that restore state on drop.
- **R36 (Event Channels over Callbacks):** Event streams and observers use channels (`tokio::sync::mpsc`), where dropping the receiver unsubscribes, rather than `Arc<dyn Observer>` callback lists.
- **R37 (Avoid Public Boxed Trait Strategies):** Avoid factories or strategies returning/storing `Box<dyn Trait>` in public APIs. Use enums or generic closures.
- **R38 (Facades & Adapters):** Facades own subsystems and delegate; adapters wrap foreign types and implement our traits while keeping foreign types private.
- **R39 (Enum Polymorphism):** Use closed enums and `match` instead of open trait objects or visitor patterns for closed sets of kinds.
- **R40 (Never Ignore Results):** Avoid ignoring `Result` or compiler warnings (`let _ =`, `.ok()`, blanket `allow`). Handle errors or use `#[expect(.., reason = "..")]`.
- **R41 (Iterator Adapters over Index Loops):** Avoid manual indexing loops (`for i in 0..len`); use iterator adapters, `windows`, and `get`.
- **R42 (Minimize Mutability):** Avoid `mut` where bindings do not change, and `&mut self` where `&self` is sufficient.
- **R43 (Lifetime Elision):** Avoid explicit lifetimes where elision gives the same signature; no unused lifetime parameters.
- **R44 (Avoid Lossy Casts):** Avoid lossy `as` casts and `transmute`; use `TryFrom` or checked conversions.
- **R45 (Arithmetic Safety):** Arithmetic with user or external input that could overflow must use `checked_`, `saturating_`, or `wrapping_` methods.
- **R46 (No Leaks by Design):** No `mem::forget`, `Box::leak`, or uninitialized memory in library code.
- **R47 (Explicit Security Erasure):** Sensitive data (credentials, secret tokens, private keys) must be explicitly cleared/zeroized; do not rely solely on implicit `Drop`.
- **R48 (Vetted Dependencies):** Direct dependencies justified, audited with `cargo audit` and `cargo deny check`, no unvetted unsafe in dependencies.
- **R49 (Plain Build Environment):** Stable toolchain (`rustc 1.99`), compiler environment variables at defaults.
- **R50 (FFI Readiness):** Inapplicable until foreign boundaries exist; if introduced, safe wrapping and no panics across FFI are mandatory.

---

## Python Parity & Audit Verification

- **Always verify against parity documentation:**
  - Before writing code or claiming a Python SeleniumBase feature is missing, inspect `parity/api.toml`, `docs/parity.md`, or [https://mustcodeal-seleniumbase-rs.docs7.io/parity](https://mustcodeal-seleniumbase-rs.docs7.io/parity).
  - Use `parity_lookup` and `find_api` in the `dev` MCP server (`sbrs-mcp --server dev`).
  - Python features are tracked with their explicit idiomatic Rust equivalents (e.g., Python `get_cookies` is Rust `cookies`).
- **Update generated indexes on API changes:**
  - When changing public methods of `BaseCase` or the CDP engine, always update `src/mcp/dev/api_index.json` using:
    `SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index`
