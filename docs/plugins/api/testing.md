---
title: "Testing Plugins"
sidebarTitle: "Testing"
description: "Reference for seleniumbase-rs-plugin-testing: RecordingPage and RecordingBrowser, recording fakes of the plugin handles, the Call they record, and block_on, to test a plugin's hooks without a browser or an async runtime."
---

`seleniumbase-rs-plugin-testing` has recording fakes of `PageHandle` and
`BrowserHandle`: they write down every command, script and expression a
plugin sends, so a test checks what a hook did, without a browser. It is in the
plugins repository, `crates/seleniumbase-rs-plugin-testing/src/lib.rs`, and
depends only on the API crate. To run the real launch steps of seleniumbase-rs
against a scripted browser instead, see the *Mock browser* tab of
[Test it without a browser](/plugins/writing-your-own#test-it-without-a-browser).

<RequestExample>

```rust Record a tab
use seleniumbase_rs_plugin::PluginSet;
use seleniumbase_rs_plugin_block_resources::BlockResources;
use seleniumbase_rs_plugin_testing::{block_on, Call, RecordingPage};
use serde_json::json;

#[test]
fn a_new_tab_gets_the_network_domain_and_the_patterns() {
    let page = RecordingPage::new();
    let set = PluginSet::new().with(BlockResources::patterns(["*.png"])).unwrap();
    block_on(set.on_page_created(&page)).unwrap();
    assert_eq!(
        page.calls(),
        [
            Call::Send("Network.enable".into(), json!({})),
            Call::Send("Network.setBlockedURLs".into(), json!({ "urls": ["*.png"] })),
        ]
    );
}
```

```rust Make a command fail
#[test]
fn a_failed_command_stops_the_tab_and_names_the_plugin() {
    let page = RecordingPage::new().failing("Network.setBlockedURLs");
    let set = PluginSet::new().with(BlockResources::media()).unwrap();
    let error = block_on(set.on_page_created(&page)).unwrap_err().to_string();
    assert!(error.contains("block-resources") && error.contains("on_page_created"));
}
```

```rust Answer a command
use seleniumbase_rs_plugin::PageHandle;
use seleniumbase_rs_plugin_testing::{block_on, Call, RecordingPage};
use serde_json::json;

let page = RecordingPage::new().replying("evaluate", json!("Mozilla/5.0 HeadlessChrome/126.0"));
let agent = block_on(page.evaluate("navigator.userAgent")).unwrap();
assert_eq!(agent, json!("Mozilla/5.0 HeadlessChrome/126.0"));
assert_eq!(page.calls(), [Call::Evaluate("navigator.userAgent".into())]);
```

```rust Another engine or browser
use seleniumbase_rs_plugin::{BrowserFamily, Engine, PluginSet};
use seleniumbase_rs_plugin_testing::{block_on, RecordingBrowser, RecordingPage};

let page = RecordingPage::on(Engine::WebDriver);
let browser = RecordingBrowser::of(Engine::WebDriver, BrowserFamily::Firefox);
let set = PluginSet::new().with(Hello).unwrap();
block_on(set.on_browser(&browser)).unwrap();
block_on(set.on_page_created(&page)).unwrap();
assert!(browser.calls().is_empty());
```

</RequestExample>

<ResponseExample>

```text Make a command fail
plugin `block-resources` failed in `on_page_created`: Network.setBlockedURLs failed (fake)
```

</ResponseExample>

## Depend on it

```toml Cargo.toml
[dev-dependencies]
seleniumbase-rs-plugin-testing = { git = "https://github.com/MustCodeAl/seleniumbase-rs-plugins", rev = "628991af0e6c5711298e3bd3692dc2974887401a" }
```

Take it at the same `rev` as `seleniumbase-rs-plugin`, so both name one copy of
the API crate.

## Call

<Badge color="orange">enum</Badge> `seleniumbase_rs_plugin_testing::Call`

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    Send(String, Value),
    InitScript(String),
    Evaluate(String),
}
```

One thing a plugin asked a fake handle to do. Compare `calls()` with a list of
them.

<ResponseField name="Call" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Send(String, Value)" type="tuple variant">

A DevTools Protocol command with its parameters, from `send` of either fake.

</ResponseField>

<ResponseField name="InitScript(String)" type="tuple variant">

A script for every new document, from `RecordingPage::add_init_script`. The set
records a plugin's packages this way too, as their `init_script()`.

</ResponseField>

<ResponseField name="Evaluate(String)" type="tuple variant">

An expression to evaluate, from `RecordingPage::evaluate`.

</ResponseField>

</Expandable>

</ResponseField>

## RecordingPage

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin_testing::RecordingPage`

```rust
#[derive(Debug)]
pub struct RecordingPage { /* private: engine, recorder */ }

impl Default for RecordingPage { /* RecordingPage::new() */ }

#[async_trait]
impl PageHandle for RecordingPage { /* ... */ }
```

A fake tab that records what a plugin asks of it. Every method of
`PageHandle` records its call first, then answers:

| Method | Records | Answers |
|---|---|---|
| `engine()` | nothing | the engine it was made with |
| `send(method, params)` | `Call::Send(method, params)` | the reply set for `method`, or `null` |
| `add_init_script(source)` | `Call::InitScript(source)` | `Ok(())` |
| `evaluate(expression)` | `Call::Evaluate(expression)` | the reply set for `"evaluate"`, or `null` |

A method made to fail with `failing` answers
`PluginError::Other("METHOD failed (fake)")` instead, after recording.

## RecordingPage::new

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn new() -> Self
```

<ResponseField name="return" type="RecordingPage">

A fake tab of the pure CDP engine (`Engine::Cdp`); also `RecordingPage::default()`.

</ResponseField>

## RecordingPage::on

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn on(engine: Engine) -> Self
```

<ParamField body="engine" type="Engine" required>

The engine `engine()` reports, such as `Engine::WebDriver`.

</ParamField>

<ResponseField name="return" type="RecordingPage">

A fake tab of `engine`.

</ResponseField>

## RecordingPage::failing

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn failing(self, method: &str) -> Self
```

<ParamField body="method" type="&str" required>

A DevTools method name for `send`, or `"add_init_script"` or `"evaluate"`.

</ParamField>

<ResponseField name="return" type="RecordingPage">

The fake, with `method` answering `PluginError::Other("METHOD failed
(fake)")`. One method fails at a time: a second call replaces the first.

</ResponseField>

## RecordingPage::replying

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn replying(self, method: &str, reply: Value) -> Self
```

<ParamField body="method" type="&str" required>

A DevTools method name for `send`, or `"evaluate"`.

</ParamField>

<ParamField body="reply" type="serde_json::Value" required>

What it answers. Each method has its own reply; a later one replaces an
earlier one.

</ParamField>

<ResponseField name="return" type="RecordingPage">

The fake, with `method` answering `reply` (otherwise `null`).

</ResponseField>

## RecordingPage::calls

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn calls(&self) -> Vec<Call>
```

<ResponseField name="return" type="Vec<Call>">

Everything the plugins asked for, in order: a copy, so the fake keeps
recording.

</ResponseField>

## RecordingBrowser

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin_testing::RecordingBrowser`

```rust
#[derive(Debug)]
pub struct RecordingBrowser { /* private: engine, family, recorder */ }

impl Default for RecordingBrowser { /* RecordingBrowser::new() */ }

#[async_trait]
impl BrowserHandle for RecordingBrowser { /* ... */ }
```

A fake browser that records what a plugin asks of it. `engine()` and
`family()` report what it was made with; `send(method, params)` records
`Call::Send(method, params)` and answers `null`. It has no `failing` or
`replying`.

## RecordingBrowser::new

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn new() -> Self
```

<ResponseField name="return" type="RecordingBrowser">

A fake Chromium browser on the pure CDP engine; also
`RecordingBrowser::default()`.

</ResponseField>

## RecordingBrowser::of

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn of(engine: Engine, family: BrowserFamily) -> Self
```

<ParamField body="engine" type="Engine" required>

The engine it reports.

</ParamField>

<ParamField body="family" type="BrowserFamily" required>

The family it reports, such as `BrowserFamily::Firefox`.

</ParamField>

<ResponseField name="return" type="RecordingBrowser">

A fake browser of `family` on `engine`.

</ResponseField>

## RecordingBrowser::calls

<Badge color="green">fn</Badge>

```rust
#[must_use]
pub fn calls(&self) -> Vec<Call>
```

<ResponseField name="return" type="Vec<Call>">

Everything the plugins asked for, in order.

</ResponseField>

## block_on

<Badge color="green">fn</Badge> `seleniumbase_rs_plugin_testing::block_on`

```rust
pub fn block_on<F: Future>(future: F) -> F::Output
```

<ParamField body="future" type="impl Future" required>

A future to run, such as `set.on_page_created(&page)`.

</ParamField>

<ResponseField name="return" type="F::Output">

Its output.

</ResponseField>

Runs a future to completion on the current thread, for tests and examples that
do not want a runtime: it polls with a no-op waker and yields the thread between
polls. The fakes never wait, so nothing else is needed. It drives no I/O and no
timers: a future that needs a runtime's reactor, such as tokio's, does not
complete under it. Test those against the mock browser of seleniumbase-rs with
`#[tokio::test]`.
