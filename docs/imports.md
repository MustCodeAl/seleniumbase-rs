---
title: "Imports and dependencies"
sidebarTitle: "Imports"
description: "What a test crate imports from seleniumbase_rs instead of thirtyfour, serde_json, rstest, rstest-bdd and tokio, which lines stay in Cargo.toml and why."
---

A project that uses `seleniumbase-rs` lists `seleniumbase-rs` in its `Cargo.toml`
and writes `use seleniumbase_rs::...` for everything the library's API takes or
returns. The documentation, the examples and the code that `sbrs mkfile`,
`sbrs mkdir` and the recorder generate follow the same rule, so a copied example
needs no second crate.

<Visibility for="agents">

Summary for AI exports: import from `seleniumbase_rs`, not from the crates it is built on. Tokio: `use seleniumbase_rs::tokio;` before `#[tokio::main]` or `#[tokio::test]`. rstest (feature `testing`): `use seleniumbase_rs::testing::{fixture, rstest};`. Gherkin (feature `bdd`): `use seleniumbase_rs::testing::bdd::{given, scenario, scenarios, skip, then, when};` plus `use seleniumbase_rs::testing::rstest;`, and `rstest-bdd = "0.6"` in the manifest (never named in code). WebDriver types: `use seleniumbase_rs::drivers::{By, WebElement, DesiredCapabilities, ...};`. JSON: `use seleniumbase_rs::json::{json, Value};`. Test plugins: `use seleniumbase_rs::plugins::observer::async_trait;`. Plugin crates, your own `serde` derives and your own `tracing` spans still need those crates.

</Visibility>

## What you import instead

| You need | Import | Instead of |
|---|---|---|
| The async runtime and `#[tokio::main]`, `#[tokio::test]` | `use seleniumbase_rs::tokio;` | `tokio = { version = "1", features = [...] }` |
| `#[rstest]` and `#[fixture]` (feature `testing`) | `use seleniumbase_rs::testing::{fixture, rstest};` | `rstest = "0.26"` |
| The `sb` and `sb_cdp` fixtures | `use seleniumbase_rs::testing::{sb, Sb, sb_cdp, SbCdp};` | nothing else |
| Gherkin steps and scenarios (feature `bdd`) | `use seleniumbase_rs::testing::bdd::{given, when, then, scenario, scenarios, skip};` and `use seleniumbase_rs::testing::rstest;` | `rstest-bdd-macros = "0.6"`, `rstest = "0.26"` |
| A selector or an element on the WebDriver engine | `use seleniumbase_rs::drivers::{By, WebElement};` | `thirtyfour` |
| Capabilities, keys, cookies, network conditions of the WebDriver engine | `use seleniumbase_rs::drivers::{DesiredCapabilities, Key, Cookie, NetworkConditions, ...};` | `thirtyfour` |
| A JSON value or the `json!` macro | `use seleniumbase_rs::json::{json, Value};` | `serde_json = "1"` |
| `#[async_trait]` for a `TestPlugin` or `PageEvidence` | `use seleniumbase_rs::plugins::observer::async_trait;` | `async-trait = "0.1"` |

`seleniumbase_rs::drivers` lists the `thirtyfour` types that appear in this
crate's signatures. [`BaseCase::find_element`](/api/traits) returns a
`seleniumbase_rs::drivers::WebElement`, so `root.find(By::Css("button"))` on
it needs only the `By` of the same module.

### Why a macro needs an import you never name

`#[tokio::test]` and `#[rstest]` are attribute macros that write code starting
with `tokio::` and `rstest::`. Those paths are looked up from the file that
uses the macro, so a crate without `tokio` or `rstest` in its manifest has
nothing to resolve them to. `use seleniumbase_rs::tokio;` and
`use seleniumbase_rs::testing::rstest;` put the crate under its own name in the
file, and the generated paths find it. The `rstest` import also brings the
attribute itself, which is why one line does both jobs. A test that uses
`#[scenario]` imports `rstest` for the same reason: the attribute expands to
`rstest::` paths.

```rust
use seleniumbase_rs::testing::{fixture, rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[fixture]
fn login_url() -> &'static str {
    "https://example.com/login"
}

#[rstest]
#[tokio::test]
async fn login_page_has_a_title(sb: Sb, login_url: &str) -> Result<()> {
    sb.run(async |sb| {
        sb.open(login_url).await?;
        sb.assert_title("Example Domain").await
    })
    .await
}
```

## The lines that stay

Some imports cannot move into `seleniumbase_rs`. Each one has a reason, and the
page that shows it says so.

| Line | Where | Why it stays |
|---|---|---|
| `rstest-bdd = "0.6"` in `Cargo.toml` | Gherkin test crates | The Gherkin macros read the manifest of the crate they expand in (through `proc-macro-crate`) and panic with `rstest-bdd crate not found` when it is missing. They take no crate path, so no re-export can replace the line. No `.rs` file names `rstest_bdd`. |
| `use serde::{Deserialize, Serialize};` | A type of your own that you serialize | `#[derive(Serialize, Deserialize)]` expands to paths into `serde`, and the derive is yours, not the library's. |
| `use tracing::{info, instrument};` | Your own spans and events | The library logs with `tracing`; your code that logs does too. `seleniumbase_rs::init_tracing` installs the subscriber. |
| `serde_json`, `tokio`, `async_trait` in a plugin crate | A crate that depends on `seleniumbase-rs-plugin` | A plugin crate never depends on `seleniumbase-rs`, so it has nothing to import from. The plugin API re-exports `async_trait`; JSON values come from `serde_json`. |
| `serde_json::Value` in `reply.json()` | The stealth HTTP client pages | `seleniumbase-rs-stealth` is a separate repository with its own manifest; `json()` deserializes into any type you choose. |
| `tokio` or `thirtyfour` in `Cargo.toml` | A program that needs a part the library does not enable or re-export | `seleniumbase_rs::tokio` has the features this crate turns on (`macros`, `rt-multi-thread`, `time`, `signal`, `sync`, `process`, `net`, `io-util`, `fs`), and `drivers` lists what the API itself uses. Another feature or another type means the crate itself. |
| `use serde_json::json;` in contributor pages | Tests inside this repository | They are code of the library, which depends on `serde_json`. |

The snippets that continue a page leave `use seleniumbase_rs::tokio;` out when
an earlier block of the page shows it. A snippet that starts a file shows it.

## How it is kept true

The crate `crates/own-imports` in the repository lists `seleniumbase-rs` and
`rstest-bdd` and nothing else. Its tests import only `seleniumbase_rs`: an
`#[rstest]` test with a fixture, a Gherkin scenario, `#[tokio::main]` and
`#[tokio::test]`, the `drivers` and `json` types, and `#[async_trait]`. A
doctest compiles the golden files of `sbrs mkfile`, `sbrs mkdir`, `sbrs mkpres`,
`sbrs mkchart` and the recorder there, so a generator that wrote an import of another crate fails
`cargo test -p seleniumbase-rs-own-imports`. `cargo test --test docs_current`
fails when a page gains a `use thirtyfour`, `use rstest`, `use rstest_bdd` or
`use serde_json` line outside the pages that the table above names.

<Note>

The `rstest` attribute finds its crate through its `crate-name` feature when a
test crate lists `rstest` itself, and through the import when it does not. If
some other dependency turns that feature on and the manifest has no `rstest`
line, the attribute stops with `rstest is present in Cargo.toml`: list `rstest`
in that crate, and the import keeps working beside it.

</Note>
