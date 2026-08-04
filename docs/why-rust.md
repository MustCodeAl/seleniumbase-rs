# Why Rust?

This page explains when `seleniumbase-rs` is the right choice for your browser
automation work and when another stack may be a better fit. The goal is not to
claim that Rust is universally better, but to show which engineering problems it
solves well.

## What this crate is

`seleniumbase-rs` is a Rust port of the Python SeleniumBase framework. It keeps
the productive `BaseCase` API and anti-detection features, but runs on Rust's
async runtime, type system, and native distribution model. It uses `thirtyfour`
for WebDriver communication and adds its own CDP, stealth, and tooling layers.

## When Rust helps most

Choose `seleniumbase-rs` when the test harness itself benefits from strong
compile-time checks, predictable resource ownership, native binaries, or
integration with an existing Rust workspace.

### Refactoring safety

Browser tests are full of strings—URLs, selectors, and expected text. Rust
cannot catch a broken selector, but it can catch a renamed helper, a changed
assertion signature, or a mistyped method call. This turns a large class of
"works on my machine but fails in CI" bugs into compiler errors that you fix
before pushing.

For example, if you rename a helper from `login_as` to `sign_in_as`, every
broken call site is reported at compile time:

```text
error[E0599]: no method named `login_as` found for struct `BaseCase`
 --> tests/auth.rs:12:9
   |
12 |         sb.login_as("alice").await?;
   |         ^^^^^^^^ method not found in `BaseCase`
```

The browser still validates the page, but the harness no longer hides trivial
structural mistakes.

### Controlled concurrency

Rust's ownership model and Tokio make shared state explicit. This is valuable
when tests need to coordinate with each other, share connection pools, or run
in a controlled number of parallel browser processes. The `run_browser_test`
helper also makes cleanup deterministic: it awaits the test body and then quits
the WebDriver session, reporting cleanup failures instead of dropping them.

### Native distribution

The `sbase` CLI compiles to a single native binary. You do not need to manage a
Python virtual environment, resolve pip conflicts, or ship a Node runtime. This
is especially useful in CI/CD images, on restricted machines, or when tests are
run by operators who are not Rust developers.

### Shared codebase

If your services are already written in Rust, tests can reuse the same structs,
serializers, validation logic, and client code. Keeping test data models in sync
with production code becomes a compile-time guarantee instead of a manual task.

### Feature flags and dependency control

Optional features such as `playwright`, `s3`, `azure`, `gcp`, `mcp-server`, and
`full-tracing` are compiled in only when you enable them. Default builds stay
small, and you can audit exactly which dependencies are included for each target.

## What does not change

Rust does not make the browser faster. Page loads, JavaScript execution, network
round trips, and browser startup still dominate most end-to-end tests. A Rust
test that opens a heavy web page waits just as long as a Python or JavaScript
test opening the same page.

Rust also does not fix flaky selectors, race conditions in the application under
test, or unstable infrastructure. Those problems require the same test-design
discipline in every language.

## Practical comparison

| Area | Rust advantage | Important limit |
|------|----------------|-----------------|
| Refactoring | Compiler catches renamed methods and signature changes before CI | Selectors and page behavior still fail at runtime |
| Concurrency | Tokio tasks and explicit shared state support controlled parallelism | Browsers remain expensive external processes |
| Cleanup | `run_browser_test` makes session teardown deterministic | Async cleanup cannot run from `Drop` |
| Distribution | Single native binary with no Python or Node runtime | Browser and driver installation are still required |
| Dependencies | Cargo lockfiles and feature flags provide reproducible selection | Dependencies still require auditing and updates |
| Integration | Tests can share typed models with Rust services | Cross-language teams may prefer their existing stack |

## Compared to Python SeleniumBase

Python SeleniumBase has the largest existing example library and the most mature
upstream ecosystem. It is often the right choice when:

- Your team already writes Python.
- You rely on SeleniumBase Python plugins or a large existing test suite.
- You need the fastest path to a working test.

`seleniumbase-rs` is worth considering when you want:

- Compile-time guarantees across a growing test suite.
- A single native binary for CI or distribution.
- To share code with Rust services.
- Fine-grained control over dependencies via feature flags.

## Compared to JavaScript / TypeScript

JavaScript and TypeScript are convenient for frontend teams and have rich
browser-first tooling. They fit well when:

- Your team already uses Playwright, Cypress, or WebdriverIO.
- Tests need deep integration with frontend build tooling.

`seleniumbase-rs` offers:

- Compile-time correctness for missing `await`, stale imports, and renamed APIs.
- Deterministic resource cleanup through `run_browser_test`.
- A smaller runtime footprint because the compiled CLI does not pull in a Node
 runtime.

## How to decide

Use this checklist:

- [ ] Is the test suite large enough that refactoring mistakes are costly?
- [ ] Is the harness maintained by Rust developers or integrated with Rust services?
- [ ] Is native distribution or CI simplicity important?
- [ ] Do you need anti-detection features that are available in this crate?
- [ ] Are you willing to invest in porting or writing tests in Rust?

If most answers are yes, `seleniumbase-rs` is likely a good fit. If your team is
happy with Python or JavaScript and does not share code with Rust, staying in
that ecosystem may be more productive.

## Related reading

- [Security](security.md) — how Rust's safety properties affect browser automation.
- [Reliability](reliability.md) — test lifecycle, flaky behavior, and cleanup.
- [Performance](performance.md) — what to measure and what not to expect.

