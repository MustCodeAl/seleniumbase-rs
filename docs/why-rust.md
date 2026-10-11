# Why Rust?

**Short answer.** Choose `seleniumbase-rs` if your tests or tooling already live
in Rust, if you want to ship the runner as one native binary, or if you want the
pure CDP engine and the agent tools that upstream Python SeleniumBase does not
have. Do not choose it because you expect faster tests: the browser sets the
pace (see [Performance](performance.md)).

| You are... | Then |
|---|---|
| A Rust team testing a Rust service | Use it: share types and run tests with `cargo test` |
| Shipping a test runner to CI or to people who do not write Rust | Use it: `sbase` is one binary |
| Building AI-agent tooling around a browser | Use it: the MCP servers explain failures and inspect pages |
| A Python team with a large SeleniumBase suite | Stay on Python unless you have another reason |
| Wanting speed from the same tests | Measure first; expect little change |

This page explains when the crate is the right choice and when another stack may
be a better fit. It does not claim Rust is universally better.

## What this crate is

`seleniumbase-rs` is a Rust port of the Python SeleniumBase framework (it tracks
4.55.2) and of its MCP server. It keeps the productive `BaseCase` API and
anti-detection features, but runs on Rust's async runtime, type system, and
native distribution model. It has two engines: a WebDriver engine built on
`thirtyfour`, and a pure Chrome DevTools Protocol engine (`sb_cdp`) that needs
no driver at all. Every API of the upstream project has a recorded status in
[the parity table](parity.md): 861 implemented, 375 composed from existing Rust
calls, 112 not applicable (for example pytest plumbing) and 17 still planned.
That table is a claim, not a measurement; the composed and not-applicable
entries have not been independently reviewed yet.

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

Optional features such as `tui`, `gui`, `playwright`, `s3`, `azure`, `gcp`,
`mcp-server`, `turso`, `full-tracing`, and the in-progress `scrape` and
`impersonate` are compiled in only when you enable them. The default feature set
is empty, so default builds stay small, and you can audit exactly which
dependencies are included for each target (`cargo deny check` and `cargo audit`
run against the lockfile).

## What the Rust port adds beyond Python

These exist in this crate and not in upstream Python SeleniumBase. The pure CDP
engine, the input engine, the pool, interception and the identity application
were checked against a real Chrome 155; the agent tools have unit tests and
three real-Chrome tests (`diagnose_environment` ran only against a mock):

- **A pure CDP engine with no driver.** `Browser`, `Page` and `Locator` talk to
  Chrome directly, so there is no chromedriver version to match. A mock browser
  lets tests run without Chrome.
- **A behavioural input engine.** Curved pointer paths and human typing pace
  (`Page::human`).
- **An async browser pool.** `BrowserPool` and `Lease` hand out isolated,
  recycled browser contexts with shared session state.
- **Request interception and per-context proxies.** Block, fulfil or modify
  requests per page, give each context its own proxy (including password
  protected ones), and shield WebRTC leaks.
- **Coherent randomised identities.** `Fingerprint::randomized(os, seed)` draws a
  whole machine and place. The tables are plausible, not measured from real
  traffic, and nothing has been run against a live bot-detection service.
- **Tools for AI agents that develop and debug tests.** The MCP servers answer a
  failed call with the page state, a hint and the next tools to try, and offer
  page inspection, selector diagnosis, a console and failed-request view, an
  accessibility audit, a debug bundle, an environment check and an export of the
  session as a Rust test.
- **Errors for people.** Every error has a hint, `sbase` exits with a status that
  follows the error's category, and failed tests carry their page and evidence
  into the reports.

Not there yet: the impersonating HTTP session, the MITM proxy and the combined
stealth session are under construction behind the `impersonate` feature. The
TLS (JA3/JA4) and HTTP/2 (Akamai) client exists for one measured browser and
system (Chrome 155 on macOS). See [Stealth Scraping](tutorials/stealth_scraping.md)
and [the plan](design/stealth_parity_plan.md).

## Evidence from this crate

These are things the type system and the toolchain did here, not general claims
about Rust:

- **Secrets cannot leak by formatting.** A `Proxy` with credentials never shows
  its password in `Debug`, `Display` or the command line, and the scraping types
  (`RedactedUrl`, the egress key) redact on construction and are never
  serialised with the secret. Tests assert this.
- **A new case breaks the build, not the run.** The challenge detector's
  `ChallengeKind` and the mitigation policy's `Action` are enums matched
  exhaustively; adding a variant makes every `match` that ignores it fail to
  compile.
- **Bad option combinations fail before launch.** `LaunchOptions::builder()
  .build()` rejects `incognito` together with `guest`, or `use_chromium` with an
  explicit `executable_path`, before Chrome starts.
- **No `unsafe`.** The crate has no `unsafe` blocks, so memory-safety bugs would
  have to come from a dependency.
- **Testing without a browser.** `Browser::new_mocked` swaps the Chrome
  connection for a scripted one behind the same types, so most of the pure CDP
  engine is tested without launching Chrome. The default run has 1,600 passing
  tests; the real-Chrome ones are `#[ignore]`d and run one binary at a time.
- **Dependencies are auditable.** `cargo deny check` and `cargo audit` run
  against the lockfile (currently no vulnerabilities, 3 allowed unmaintained
  warnings).

Rust did not prevent every bug. During this port the WebDriver path sent a proxy
login as an extra header to every site, which is a logic error the compiler
cannot see; a review found it and a test now guards it. Timing-dependent tests
also flaked until they were made to wait for what they check.

## What it costs

- **Compile time and memory.** A full check, lint and test run takes many
  minutes on an 8 GB machine, which is why the project gates once per batch.
- **A learning curve.** Test authors need to be comfortable with Rust, `async`
  and the borrow checker.
- **A smaller ecosystem.** Python SeleniumBase has far more examples and
  plugins; Python plugins cannot be reused here.
- **Unfinished parts.** The impersonating HTTP session, the MITM proxy and the
  combined stealth session exist but are young and not a stable API (tested
  against local servers and a local Chrome, not live services), only one browser
  and system is measured for TLS and HTTP/2, and the parity table's `composed`
  and `not-applicable` entries are still to be independently reviewed.

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

