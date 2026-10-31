---
title: "Gherkin scenarios with rstest-bdd"
sidebarTitle: "Behave and Gherkin"
---

Gherkin feature files run as ordinary `cargo test` tests through
[rstest-bdd](https://docs.rs/rstest-bdd), with the browser as a fixture. This
replaces the in-house `seleniumbase_rs::behave` runner, which no longer exists.
Product owners, QA engineers and developers share the `.feature` files, and the
steps are Rust functions that use the same `sb` API as any other test.

<Info>

**What you will learn**

- How to write a `.feature` file and bind a scenario to a test function.
- How steps use the browser (`sb`), what the built-in steps already do, and what happens when a step fails.
- How to run scenarios from `cargo test`, `sbrs test` and `sbrs behave-gui`.
- How to move from the removed `seleniumbase_rs::behave` module.

</Info>

<Visibility for="agents">

Summary for AI exports: enable the features `testing` and `bdd`; the test crate also lists `rstest-bdd` (its macros read it from the manifest; no file names it) and nothing else, because the macros, the `rstest` attribute and the Tokio runtime are re-exported. A scenario is `#[scenario(path = "...", name = "...")] #[tokio::test(flavor = "multi_thread")] async fn name(#[future(awt)] sb: SbCase) {}` with `use seleniumbase_rs::testing::bdd::{given, scenario, sb, then, when, SbCase}`, `use seleniumbase_rs::testing::rstest` and `use seleniumbase_rs::tokio`; steps are `#[given]`, `#[when]`, `#[then]` async functions taking `sb: &mut SbCase`. A failed step panics; the fixture then writes `latest_logs/<test id>/` and closes the browser. `sb_cdp: SbCdpCase` is the pure CDP version. `harness = ...` cannot be used with an async scenario in rstest-bdd 0.6. The 98 built-in actions of SeleniumBase's behave library (`Open "url"`, `Click "selector"`, `Assert text "text" in "selector"`, ...) are registered when a scenario launches an `SbCase`, so a feature file that uses only them needs no step functions. `sbrs test` options such as `-k`, `-m`, `--co`, `--pdb` and `--headless` apply to scenarios; `sbrs behave-options` maps behave's `-D` options.

</Visibility>

## Set up

```toml Cargo.toml
[dev-dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["testing", "bdd"] }
# The Gherkin macros look this crate up in the manifest of the crate they expand in
# and panic when it is missing. No file below names it.
rstest-bdd = "0.6"
```

The step and scenario macros, the `rstest` attribute they expand to and the Tokio
runtime all come from `seleniumbase_rs`, so the `use` lines below name no other
crate. `rstest-bdd` is the one line that cannot go: see
[Imports and dependencies](/tutorials/getting_started#the-lines-that-stay).

## A feature, its steps and its scenario

<CodeGroup>

```gherkin tests/features/login.feature
Feature: Login

  Background:
    Given the login page is displayed

  Scenario: Valid login
    When I sign in as "demo"
    Then the title is "Welcome"

  Scenario Outline: Several users
    When I sign in as "<user>"
    Then the title is "<title>"

    Examples:
      | user  | title   |
      | demo  | Welcome |
      | admin | Admin   |
```

```rust tests/login.rs
use seleniumbase_rs::testing::bdd::{given, sb, scenario, then, when, SbCase};
// The `scenario` attribute expands to paths that start with `rstest::`.
use seleniumbase_rs::testing::rstest;
use seleniumbase_rs::tokio;
// `Result` is written out in full below: the steps' macros name `std`'s `Result`.

#[scenario(path = "tests/features/login.feature", name = "Valid login")]
#[tokio::test(flavor = "multi_thread")]
async fn valid_login(#[future(awt)] sb: SbCase) {}

#[scenario(path = "tests/features/login.feature", name = "Several users")]
#[tokio::test(flavor = "multi_thread")]
async fn several_users(#[future(awt)] sb: SbCase) {}

#[given("the login page is displayed")]
async fn login_page(sb: &mut SbCase) -> seleniumbase_rs::Result<()> {
    sb.open("https://example.com/login").await?;
    sb.breakpoint("Paused at the login page.").await
}

#[when("I sign in as {user:string}")]
async fn sign_in(sb: &mut SbCase, user: String) -> seleniumbase_rs::Result<()> {
    sb.type_text("#user", &user).await
}

#[then("the title is {title:string}")]
async fn title_is(sb: &mut SbCase, title: String) -> seleniumbase_rs::Result<()> {
    sb.assert_title(&title).await
}
```

</CodeGroup>

`SbCase` derefs to `BaseCase`, so every `sb.` method is there. The fixture
launches the browser from the run configuration (the settings file and the
`SB_*` variables that `sbrs test` sets) and closes it when the scenario ends,
whatever happened. For a pure CDP browser use `sb_cdp: SbCdpCase`, which derefs
to a `Page`.

<Note>

Write the fixture as `#[future(awt)] sb: SbCase`. Plain `#[future]` does not
compile with rstest-bdd 0.6, and `harness = ...` cannot be combined with an
`async fn` scenario, so there is no harness to add.

</Note>

## Built-in steps

Python's `behave` package ships a library of browser steps, so a feature file
can drive a page without any step code. The same library comes with the `bdd`
feature: 98 actions, each under every wording it has in Python. A scenario that
uses only them needs just the binding:

<CodeGroup>

```gherkin tests/features/shop.feature
Feature: Shop

  Scenario: Open the shop
    Given Open "https://example.com/"
    When Type text "demo" into "#user"
    And Click 'button#go'
    Then Assert title "Example Domain"
    And Assert text "Hello" in "#msg"
```

```rust tests/shop.rs
use seleniumbase_rs::testing::bdd::{sb, scenario, SbCase};
use seleniumbase_rs::testing::rstest;
use seleniumbase_rs::tokio;

#[scenario(path = "tests/features/shop.feature", name = "Open the shop")]
#[tokio::test(flavor = "multi_thread")]
async fn open_the_shop(#[future(awt)] sb: SbCase) {}
```

</CodeGroup>

Launching an `SbCase` links the built-in steps into the test binary, and your own
`#[given]`, `#[when]` and `#[then]` functions can sit beside them in the same
crate. The wording follows Python's:

- A step has no keyword of its own. `Given`, `When`, `Then`, `And` and `But`
  all reach the same step.
- A quoted argument is written in double or single quotes, so `Open "https://a.test"`,
  `Open 'https://a.test'` and `User opens URL "https://a.test"` are the same
  step. Text arguments understand the escapes `\n`, `\t`, `\"`, `\'` and `\\`.
- A step that waits for something waits 10 seconds, Python's `LARGE_TIMEOUT`.

| Actions | They cover |
|---|---|
| Navigation | open, refresh, back and forward, new windows and drivers, alerts, cookies and storage, downloads |
| Elements | click, type, select, hover, drag, check boxes, keys, the JavaScript and jQuery variants |
| Assertions | elements, text, the title and the URL, and the waits |
| Deferred assertions | `Deferred assert ...` steps and `Process deferred asserts` |
| Frames | iframes and their content |
| Other | sleep, screenshots, page source and PDF, demo mode, CDP mode |

`seleniumbase_rs::testing::bdd::steps::builtin_steps()` lists every action with its
patterns, and `matching_actions("Click 'button#go'")` tells which actions read a
line of text (exactly one for a well-formed step, none when no built-in step
reads it).

<Note>

The built-in steps act on an `SbCase` (WebDriver). A pure CDP scenario
(`sb_cdp: SbCdpCase`) has none, so write its steps with `#[given]`, `#[when]` and
`#[then]`. `Download '...' to downloads` takes the file name from the end of the
URL, because the browser saves the file under that name.

</Note>

## When a step fails

A step that returns `Err` or panics fails the scenario. Before the browser
closes, with the page still open, the fixture writes
`latest_logs/<test id>/screenshot.png`, `page_source.html` and
`basic_test_info.txt` (`--no-screenshot`, `--list-fail-page`, `--pdb` and
`--final-debug` work as in any other test).

<Warning>

A scenario's steps run once. `--reruns` does not apply, and `--time-limit` is the
browser's own limit (the next open, wait or action fails once it has run out),
not a hard stop. Use the `multi_thread` flavor of `#[tokio::test]`: with the
default one, a connection that a step opens late is driven by the test's
runtime, which waits while the browser closes.

</Warning>

## Tags, Background, outlines and tables

These are rstest-bdd's: `Background` steps run before each scenario, an outline
becomes one test per example row, a step can take a data table with
`#[datatable]`, and `scenarios!("tests/features", tags = "@smoke and not @slow")`
binds every scenario of a folder at once. See the
[rstest-bdd guide](https://github.com/leynos/rstest-bdd/blob/main/docs/users-guide.md).

## From the command line

`cargo test login` runs the scenarios whose test name contains `login`;
`sbrs test -k login` does the same with the run options of `sbrs test`, and every
other flag of [`sbrs test`](/help/test_options) works on scenarios too:

```bash Terminal
sbrs test --co                          # list the scenarios as tests, run nothing
sbrs test -m smoke --headless           # the tests in a module named smoke
sbrs test -k "login or signup" --env qa --dashboard
sbrs test --pdb                         # stop in a prompt when a step fails
```

Behave's own command line maps onto these flags, and `sbrs behave-options`
prints the table: `-D headless` is `sbrs test --headless`, `-D env=qa` is
`sbrs test --env qa`, `--dry-run` is `--co`, `--stop` is `--runner nextest`, and
`--tags` is `scenarios!(dir, tags = "...")` at compile time or `-m` with a module
name. `-q` (`--no-snippets`) has no meaning, because rstest-bdd checks every step
at compile time. The [test options page](/help/test_options#the-behave-options)
has the whole table.

`sbrs behave-gui` (build with the `tui` feature) lists the feature files under
`features/`, runs the scenarios you mark with `cargo test` and shows the result
of each. See [Behave Commander](/help/behave-gui).

`sbrs behave-gui --new` needs no feature: it prompts for a feature name, title,
scenarios and steps and writes `features/<name>.feature`.

## Moving from `seleniumbase_rs::behave`

| Removed | Now |
|---|---|
| `StepRegistry`, `AsyncStepRegistry`, `register_step` | `#[given]`, `#[when]`, `#[then]` functions |
| `run_feature_file`, `run_feature_file_async` | `#[scenario(path = "...")]` or `scenarios!("dir")` on a test function |
| `RunFilter` | `scenarios!(.., tags = "...")`, or `cargo test <name>` |
| `register_common_browser_steps` | the [built-in browser steps](#built-in-steps), registered when a scenario launches an `SbCase` |
| `BaseCase` behind an `Arc<Mutex<..>>` | `sb: &mut SbCase` in a step |
| `without_session` placeholders | `Browser::new_mocked()` with `SbCdpCase::launch_with_browser` (feature `test-util`) |
