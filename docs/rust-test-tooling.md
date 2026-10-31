---
title: "Writing Browser Tests"
sidebarTitle: "Write a test"
---

{/* layout-tree-component:start */}

export const LayoutTree = ({ children }) => (
  <div className="not-prose layout-tree" style={{ margin: "1.25rem 0", fontFamily: "var(--font-sans)", fontSize: "14px", color: "var(--text-secondary)" }}>
    <style>{`
      .layout-tree summary { list-style: none; }
      .layout-tree summary::-webkit-details-marker { display: none; }
      .layout-tree .lt-row:hover { background-color: var(--surface-subtle); }
      .layout-tree .lt-fold { color: #d5a910; }
      .dark .layout-tree .lt-fold { color: #ffd452; }
      .layout-tree .lt-fold-closed { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-open { display: none; }
      .layout-tree details:not([open]) summary .lt-fold-closed { display: flex; }
      .layout-tree .lt-hl { color: var(--primary); background-color: color-mix(in srgb, var(--primary) 8%, transparent); }
      .dark .layout-tree .lt-hl { color: var(--primary-light); }
      .layout-tree .lt-ic-gray { color: #84848a; }
      .dark .layout-tree .lt-ic-gray { color: #adadb1; }
      .layout-tree .lt-ic-blue { color: #1a85d4; }
      .dark .layout-tree .lt-ic-blue { color: #69b1ff; }
      .layout-tree .lt-ic-cyan { color: #1ca1c7; }
      .dark .layout-tree .lt-ic-cyan { color: #68cdf2; }
      .layout-tree .lt-ic-green { color: #199f43; }
      .dark .layout-tree .lt-ic-green { color: #5ecc71; }
      .layout-tree .lt-ic-indigo { color: #693acf; }
      .dark .layout-tree .lt-ic-indigo { color: #9d6afb; }
      .layout-tree .lt-ic-orange { color: #d47628; }
      .dark .layout-tree .lt-ic-orange { color: #ffa359; }
      .layout-tree .lt-ic-pink { color: #d32a61; }
      .dark .layout-tree .lt-ic-pink { color: #ff678d; }
      .layout-tree .lt-ic-purple { color: #a631be; }
      .dark .layout-tree .lt-ic-purple { color: #d568ea; }
      .layout-tree .lt-ic-red { color: #d52c36; }
      .dark .layout-tree .lt-ic-red { color: #ff6762; }
      .layout-tree .lt-ic-yellow { color: #d5a910; }
      .dark .layout-tree .lt-ic-yellow { color: #ffd452; }
    `}</style>
    {children}
  </div>
)

export const LayoutRow = ({ icon, name, note, highlight = false, button = false }) => (
  <div className={highlight ? "lt-row lt-hl" : "lt-row"} style={{ display: "flex", alignItems: "flex-start", gap: "6px", minHeight: "30px", padding: "5px 8px", margin: "0 2px", borderRadius: "6px", lineHeight: "20px", cursor: button ? "pointer" : "default" }}>
    <span style={{ width: "16px", height: "20px", flexShrink: 0, display: "flex", alignItems: "center", justifyContent: "center", color: highlight ? "inherit" : "var(--text-muted)" }}>{icon}</span>
    <span style={{ whiteSpace: "nowrap", fontFamily: "var(--font-mono, ui-monospace, SFMono-Regular, Menlo, monospace)", fontSize: "13px", fontWeight: 600, color: highlight ? "inherit" : "var(--text-primary)" }}>{name}</span>
    {note ? <span style={{ color: "var(--text-muted)", fontSize: "13px", fontStyle: "italic", marginLeft: "10px", opacity: 0.9 }}>{note}</span> : null}
  </div>
)

export const LayoutFolderIcon = ({ kind }) => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    {kind === "empty" ? (
      <path fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" d="M2 4.2A1.2 1.2 0 0 1 3.2 3h3.1c.32 0 .62.13.85.35L8.4 4.6h4.4A1.2 1.2 0 0 1 14 5.8v6a1.2 1.2 0 0 1-1.2 1.2H3.2A1.2 1.2 0 0 1 2 11.8z" />
    ) : kind === "open" ? (
      <g fill="currentColor">
        <path opacity=".5" d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H13a1.5 1.5 0 0 1 1.5 1.5V7H4.2a1.5 1.5 0 0 0-1.4.95L1.5 11.3z" />
        <path d="M4.2 8h10.6a.7.7 0 0 1 .65.95l-1.6 4.1a1.5 1.5 0 0 1-1.4.95H2.2a.7.7 0 0 1-.65-.95l1.25-3.2A1.5 1.5 0 0 1 4.2 8z" />
      </g>
    ) : (
      <g fill="currentColor">
        <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3.38a1.5 1.5 0 0 1 1.06.44L8.5 4H1.5z" />
        <path opacity=".6" d="M1.5 4.5V12A1.5 1.5 0 0 0 3 13.5h10a1.5 1.5 0 0 0 1.5-1.5V6A1.5 1.5 0 0 0 13 4.5z" />
      </g>
    )}
  </svg>
)

export const LayoutFileIcon = () => (
  <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
    <path fill="currentColor" opacity=".5" d="M8 1v3a3 3 0 0 0 3 3h3v5.5a2.5 2.5 0 0 1-2.5 2.5h-7A2.5 2.5 0 0 1 2 12.5v-9A2.5 2.5 0 0 1 4.5 1z" />
    <path fill="currentColor" d="M9.5 1a.5.5 0 0 1 .354.146l4 4A.5.5 0 0 1 14 5.5V6h-3a2 2 0 0 1-2-2V1z" />
  </svg>
)

export const LayoutDir = ({ name, note, open = false, highlight = false, children }) =>
  children ? (
    <details open={open}>
      <summary>
        <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><span className="lt-fold-open" style={{ display: "flex" }}><LayoutFolderIcon kind="open" /></span><span className="lt-fold-closed"><LayoutFolderIcon kind="closed" /></span></span>} name={name} note={note} highlight={highlight} button />
      </summary>
      <div style={{ marginLeft: "17.5px", borderLeft: "1px solid var(--border-subtle)", paddingLeft: "6px" }}>
        {children}
      </div>
    </details>
  ) : (
    <LayoutRow icon={<span className={highlight ? "" : "lt-fold"} style={{ display: "flex" }}><LayoutFolderIcon kind="empty" /></span>} name={name} note={note} highlight={highlight} />
  )

export const layoutIconColor = (name) => {
  const n = String(name).toLowerCase()
  const ext = n.includes(".") ? n.split(".").pop() : ""
  if (n === "dockerfile" || n.startsWith("dockerfile.") || n.startsWith("docker-compose")) return "blue"
  if (["rs", "json", "jsonc", "html", "htm", "zip", "gz", "tgz", "crx", "xpi", "swift", "svg"].includes(ext)) return "orange"
  if (["ts", "tsx", "mts", "cts", "py"].includes(ext)) return "blue"
  if (["js", "mjs", "cjs", "jsx"].includes(ext)) return "yellow"
  if (["md", "mdx", "sh", "bash", "zsh", "fish", "ps1"].includes(ext)) return "green"
  if (["css", "scss"].includes(ext)) return "indigo"
  if (["png", "jpg", "jpeg", "gif", "webp", "ico"].includes(ext)) return "pink"
  if (ext === "go") return "cyan"
  return "gray"
}

export const LayoutFile = ({ name, note, highlight = false }) => (
  <LayoutRow icon={<span className={highlight ? "" : "lt-ic-" + layoutIconColor(name)} style={{ display: "flex" }}><LayoutFileIcon /></span>} name={name} note={note} highlight={highlight} />
)

{/* layout-tree-component:end */}

`seleniumbase-rs` does not ship its own test runner. `cargo test` is the one
runner and [rstest](https://docs.rs/rstest) supplies the fixtures: with the
`testing` feature the library adds `sb` (WebDriver) and `sb_cdp` (pure CDP),
which do what Python's `sb` fixture does with the pytest options. `sbrs test`
is `cargo test` (or `cargo nextest run`) with pytest's flags. This keeps tests
idiomatic and lets you use the full Rust testing ecosystem.

This page maps common testing concepts from Python and JavaScript to Rust, shows
the recommended test shape, and explains how to organize a growing suite.

<Visibility for="agents">

Summary for AI exports: write a browser test as `#[rstest] #[tokio::test] async fn name(sb: Sb) -> Result<()> { sb.run(async |sb| { .. }).await }` with `use seleniumbase_rs::testing::{rstest, sb, Sb}` and `use seleniumbase_rs::{tokio, Result}`, and the feature `testing` as the only dev-dependency. The body is an async closure over `&mut BaseCase`; it may run again on a rerun, so it must not consume what it captures. For the pure CDP engine ask for `sb_cdp: SbCdp` and call `sb_cdp.run(async |page| { .. })`. The fixture reads the settings file and the `SB_*` variables, reruns, enforces a time limit, leaves `latest_logs/<test id>/` on a failure and closes the browser. `run_browser_test(BrowserConfig::default(), async |sb| { .. })` and `run_cdp_test` are the explicit forms that read no configuration; `sb_test!` generates the same wrapper. Run with `cargo test NAME`, `cargo nextest run` or `sbrs test` (`-k`, `-m`, `-n`, `--co`, `--reruns`, `--pdb`). Put integration tests in `tests/` and shared helpers in `tests/helpers/`.

</Visibility>

## Concept mapping

| Python or JavaScript concept | Rust equivalent |
|------------------------------|-----------------|
| `pytest` async plugin | `#[tokio::test]` |
| `def test(sb)` with the `sb` fixture | `#[rstest]` with `sb: Sb`, and `sb.run(..)` for the body |
| SeleniumBase `setUp` / `tearDown` | `Sb::run`, or `run_browser_test` |
| Test selection by name, `-k` and `-m` | `cargo test name`, or `sbrs test -k` and `-m` |
| Parallel test execution, `-n` | `cargo nextest run`, or `sbrs test -n` |
| `--reruns`, `--time-limit`, `--pdb` | the same flags of `sbrs test` |
| Feature-specific suites | Cargo features |
| Environment configuration | `BrowserConfig`, `sbase_config.toml`, `SB_*` env vars |
| Parameterized test cases | rstest's `#[case]` |

## Recommended test shape

Ask for the `sb` (WebDriver) or `sb_cdp` (pure CDP) fixture and hand the body of
the test to `run`. The library then does what Python's `sb` fixture does with the
pytest options: it reads the run configuration, reruns a failed test, stops one
that runs too long, leaves the evidence of a failure in `latest_logs/` and
closes the browser whatever happened. The one dev-dependency is the library with
the `testing` feature, which re-exports the `rstest` attribute and the Tokio
runtime:

```toml Cargo.toml
[dev-dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["testing"] }
```

<Tabs>

<Tab title="WebDriver (sb)" icon="flask">

```rust tests/home.rs
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn user_can_open_the_home_page(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_element("h1").await?;
        sb.assert_text("Example Domain", "h1").await
    })
    .await
}
```

The body is an async closure over `&mut BaseCase` that returns `Result<()>`, so
`?` converts any error with a `From` impl into it, and so do the assertions of
`BaseCase`. A panic in the body fails the test too, after the browser is closed.

</Tab>

<Tab title="Pure CDP (sb_cdp)" icon="bolt">

```rust tests/home_cdp.rs
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn user_can_open_the_home_page(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp
        .run(async |page| {
            page.goto("https://example.com").await?;
            page.locator("h1").expect().to_contain_text("Example Domain").await?;
            Ok(())
        })
        .await
}
```

The body gets the first `Page` of a browser that needs no driver. A `Page` has
no test data of its own, so read it with `sb_cdp.data()`.

</Tab>

</Tabs>

The body is called again on a rerun, so it must not move out of what it
captures. For the test data (`sb.env()`, `sb.data()`, `sb.var1()`, ...), the
evidence and the reports, see [Writing Tests with the sb
Fixture](/tutorials/writing_tests).

### Reruns, time limit and debugging

These are the options of pytest's plugins, as options of `sbrs test`, `SB_*`
variables and keys of the `[test]` table of the settings file:

| Python | Here | What it does |
|---|---|---|
| `--reruns N`, `--reruns-delay S` | `--reruns`, `--reruns-delay`, `settings.test.reruns` | Runs a failed test again with a new browser, after the delay |
| `--time-limit S` | `--time-limit`, `settings.test.time_limit` | Fails a test that runs longer, and closes its browser |
| `--pdb`, `--trace`, `--final-debug` | the same flags | A prompt with the browser open; see [Debug modes](/tutorials/debug_modes) |
| `--env`, `--data`, `--var1` | the same flags | The test data, read with `sb.env()`, `sb.data()`, `sb.var1()` |

Run the test:

```bash
cargo test user_can_open_the_home_page
sbrs test user_can_open_the_home_page --reruns 2
```

## Running the full suite

For small suites, the default test runner is fine. For larger suites, install and
use [cargo-nextest](https://nexte.st/). `sbrs test` runs either one and takes the
options of Python's pytest plugin (see [Test options](/help/test_options)):

<CodeGroup>

```bash cargo test
cargo test
```

```bash cargo nextest
cargo install cargo-nextest
cargo nextest run
```

```bash sbrs test
sbrs test                         # cargo test
sbrs test --runner nextest -n 4   # cargo nextest, four tests at a time
sbrs test -k "login or signup"    # select tests by name
sbrs test -m smoke                # select the tests in a module named smoke
sbrs test --co                    # list the tests, run nothing
```

</CodeGroup>

<Note>

Nextest runs each test as a separate process, which isolates browser failures
and makes parallel execution safer. The repository profile intentionally does
not retry failed tests, so flaky behavior is not hidden: use `--reruns N` on
the `sb` fixture's tests when you want a rerun. A debug prompt (`--pdb`,
`--trace`, `--final-debug`) needs the cargo runner, because nextest gives tests
no standard input; see [Debug modes](/tutorials/debug_modes).

</Note>

## Sharing helpers

As your suite grows, extract reusable helpers into modules or crates:

<View title="BaseCase (WebDriver)" icon="server">

```rust
// tests/helpers/mod.rs
use seleniumbase_rs::{BaseCase, Result};

pub async fn login(sb: &mut BaseCase, user: &str, pass: &str) -> Result<()> {
    sb.open("https://example.com/login").await?;
    sb.type_text("#username", user).await?;
    sb.type_text("#password", pass).await?;
    sb.click("#submit").await?;
    sb.assert_element_visible("#dashboard").await
}
```

`BaseCase::find_element` is still there when you want the `drivers::WebElement`
handle.

</View>

<View title="sb_cdp (pure CDP)" icon="bolt">

Use the capability traits (`BrowserApi`, `ElementApi`, `AssertionApi`,
`ScreenshotApi`) when you want helpers to work with any type that exposes the
same behavior:

```rust
use seleniumbase_rs::ElementApi;

async fn fill_email<E: ElementApi>(sb: &mut E, email: &str) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    sb.type_text("#email", email).await
}
```

Both `BaseCase` (WebDriver) and `sb_cdp::Page` (Pure CDP) implement the traits,
so the same helper runs on either. `ElementApi` no longer has `find_element`,
because its `drivers::WebElement` return type tied the trait to WebDriver.

</View>

## Parameterized tests

rstest's `#[case]` runs one body for several inputs. Each case is its own test,
with its own test id, its own folder of evidence and its own name for `-k`:

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[case("alice", "secret1")]
#[case("bob", "secret2")]
#[tokio::test]
async fn login_with_multiple_users(sb: Sb, #[case] user: &str, #[case] pass: &str) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com/login").await?;
        sb.type_text("#username", user).await?;
        sb.type_text("#password", pass).await?;
        sb.click("#submit").await?;
        sb.assert_element_visible("#dashboard").await
    })
    .await
}
```

## Test organization

A typical project layout looks like this:

<LayoutTree>
  <LayoutDir name="my_project" open>
    <LayoutFile name="Cargo.toml" />
    <LayoutFile name="sbase_config.toml" note="global defaults for every test" />
    <LayoutDir name="src">
      <LayoutFile name="main.rs" />
    </LayoutDir>
    <LayoutDir name="tests" open note="integration tests, each compiled as a separate binary">
      <LayoutDir name="helpers" note="shared helpers">
        <LayoutFile name="mod.rs" />
      </LayoutDir>
      <LayoutFile name="login.rs" />
      <LayoutFile name="checkout.rs" />
    </LayoutDir>
  </LayoutDir>
</LayoutTree>

Integration tests in `tests/` are compiled as separate binaries, which gives
each test its own browser process and reduces shared-state bugs.

## Configuration per test

For global defaults, use `sbase_config.toml`, `SB_*` environment variables or
the flags of `sbrs test` ([Test options](/help/test_options) lists them). See
[Settings and Configuration](/tutorials/settings_and_config) for details.

To change a setting for one test, which is the Rust form of Python's `SB(...)`
arguments, use `configure` on the fixture:

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Result};

#[rstest]
#[tokio::test]
async fn flaky_page(sb: Sb) -> Result<()> {
    let sb = sb.configure(|settings| {
        settings.headless = true;
        settings.test.reruns = 2;
        settings.test.time_limit = Some(60.0);
    });
    sb.run(async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_title_contains("Example").await
    })
    .await
}
```

For a test that must not depend on the environment at all, build a
`BrowserConfig` directly and use `run_browser_test`, which reads no
configuration:

```rust
use seleniumbase_rs::{run_browser_test, tokio, BrowserConfig, DriverMode, Result};

#[tokio::test]
async fn uc_smoke_test() -> Result<()> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(true);

    run_browser_test(config, async |sb| {
        sb.open("https://example.com").await?;
        sb.assert_title_contains("Example").await
    })
    .await
}
```

## Macros for shorter tests

The `sb_test!` macro generates a `#[tokio::test]` wrapper and the
`run_browser_test` boilerplate:

```rust
use seleniumbase_rs::{sb_test, BrowserConfig};

sb_test!(home_page_loads, BrowserConfig::default(), |sb| {
    sb.open("https://example.com").await?;
    sb.assert_text("Example Domain", "h1").await
});
```

This expands to the same shape as the manual example above. It is optional but
useful for linear tests where the reduced noise improves readability.

See [Macros](/tutorials/macros) for the full macro catalog.

## Common pitfalls

<AccordionGroup>

<Accordion title="Using the old Box::pin closure" icon="code">

`run_browser_test` takes an async closure now, so `|sb| Box::pin(async move { .. })`
does not compile ("implementation of `AsyncFnOnce` is not general enough"). Write
`async |sb| { .. }`.

</Accordion>

<Accordion title="Moving out of what the body captures" icon="recycle">

The body of `sb.run` is called again on a rerun, so it must not consume what it
captures. Borrow the case's parameters (`&str`, a `&String`) or clone them
inside the closure; a body that moves a captured `String` out by value does not
compile.

</Accordion>


<Accordion title="Mixing sync and async cleanup" icon="arrows-rotate">

`BaseCase::quit` is async. Do not rely on `Drop` for cleanup; use `Sb::run`
(or `run_browser_test`) or call `quit` explicitly.

</Accordion>

<Accordion title="Heavy setup in every test" icon="weight-hanging">

If every test logs in, consider a helper or a shared session strategy, but be
aware that shared browsers can leak state.

</Accordion>

<Accordion title="Too much parallelism" icon="gauge-high">

Browsers are expensive. Start with low concurrency and increase only after
measuring.

</Accordion>

</AccordionGroup>

## Testing checklist

- [ ] Tests ask for the `sb` or `sb_cdp` fixture and hand their body to `run`, so the browser closes whatever happens.
- [ ] Helpers are extracted into `tests/helpers/` or a shared crate.
- [ ] Configuration is externalized for environment differences (the settings file, `SB_*` variables, `sbrs test` flags).
- [ ] The suite is run with `sbrs test`, `cargo test` or `cargo nextest run`.
- [ ] A failing test leaves its screenshot, page source and test info in `latest_logs/<test id>/`: the fixture does it, so look there first.
