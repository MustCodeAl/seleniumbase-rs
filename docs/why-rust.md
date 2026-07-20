# Why Rust?

Choose `seleniumbase-rs` when the test harness itself benefits from strong
compile-time checks, predictable resource ownership, native binaries, and
integration with a Rust application or workspace.

## Practical advantages

| Area | Rust advantage | Important limit |
|------|----------------|-----------------|
| Refactoring | Types and compiler errors expose many broken call sites before a test runs | Selectors and page behavior still fail at runtime |
| Concurrency | Tokio tasks and explicit shared state support controlled parallelism | Browsers remain expensive external processes |
| Cleanup | Ownership makes lifecycle boundaries visible | Async browser cleanup cannot run from `Drop` |
| Distribution | `cargo install` produces a native CLI without a Python or Node runtime | Browser and driver installation are still required |
| Dependencies | Cargo lockfiles and feature flags provide reproducible dependency selection | Dependencies still require auditing and updates |
| Integration | Tests can share typed models and helpers with Rust services | Cross-language teams may prefer their existing stack |

## When Python or JavaScript may fit better

Python SeleniumBase has the mature upstream ecosystem and a large collection
of existing tests. JavaScript and TypeScript can be a natural fit for teams
already using browser-first tooling. Migration has a cost, so choose Rust for
specific engineering benefits rather than assuming that one language makes
every browser test faster or safer.

Read the focused discussions on [security](security.md),
[reliability](reliability.md), and [performance](performance.md).

