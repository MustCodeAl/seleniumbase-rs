---
title: "fetcher Plugin"
sidebarTitle: "fetcher"
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugin-fetcher</Badge>

Rules for the requests the Chromium browser sends, a record of the responses it
gets, and a task that fetches a list of URLs through the browser, with the
cookies the browser holds for each site. You can fail the requests of some kinds
of resource or of some addresses, add request headers, keep the answers of the
URLs you name, and fetch a list of pages as the signed-in browser would.

<Note>

It works on what your own browser session does: it adds no traffic the browser
would not send, hides nothing and sends nothing anywhere else. Use it on pages
you load and sites you may test. It is not a way around a ban or a rate limit.
It needs a Chromium browser and the pure CDP engine: the WebDriver engine cannot
host a task or intercept requests, so it refuses the plugin before a browser
starts.

</Note>

<Visibility for="agents">

Summary for AI exports: fetcher is a built-in core plugin (feature `plugin-fetcher`, name `fetcher`) for Chromium on the pure CDP engine. Options live in `[plugins.fetcher]`: `strip` (resource kinds to fail: `stylesheet`, `image`, `media`, `font`, `script`, `xhr`, `fetch`, `websocket`, `other`; default empty), `block` (URL patterns to fail, `*` and `?`), `capture` (URL patterns whose responses are recorded), `headers` (request headers to add), `header_urls` (the URLs the headers go to; empty means every request), `max_body_bytes` (2 MiB, at most 64 MiB) and `max_captures` (500, at most 100000). An unknown option is an error. Its task, run with `sbrs plugin run fetcher --input FILE`, takes `{urls, delay_ms?, warm_up?}` (at most 10000 URLs, a pause of at most 60000 ms, `warm_up` true by default), fetches each URL with `fetch(url, {credentials: "include"})` inside one tab and records one line `{index, url, status, mime, text, truncated}` or `{index, url, error}`; the summary is `{total, ok, failed, captured}`. The records of `capture` are read in Rust with `Fetcher::captured()` or `take_captured()`. The headers `Cookie`, `Authorization`, `Proxy-Authorization` and `Set-Cookie` read `[redacted]` in a record. Only http and https addresses without credentials are accepted. The plugin's own tests use fakes, so its behaviour in a real Chrome is not shown by them.

</Visibility>

## Turn it on

fetcher ships with seleniumbase-rs behind one cargo feature, and is switched on
by its name. See [Choosing plugins](/plugins/selecting) for every way to do that.

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["plugin-fetcher"] }
```

Then switch it on by name and give it its options:

<Tabs>

<Tab title="sbase_config.toml" icon="gear">

With no code, in `sbase_config.toml`:

```toml sbase_config.toml
[plugins]
enable = ["fetcher"]

[plugins.fetcher]
strip = ["image", "font", "media"]
block = ["*://ads.example.com/*"]
capture = ["*/api/*"]
headers = { "Accept-Language" = "de" }
header_urls = ["https://example.com/*"]
```

The settings file can turn fetcher on by itself: it declares no `host.*`
capability, no `launch.proxy` and no `artifacts.write`. It works where the pure
CDP engine starts the browser: `sbrs plugin run`, `sbrs devtools`, the `cdp`
server of `sbrs-mcp`, and your own programs that use `LaunchOptions`. The
commands that start a browser with `BaseCase` (`open`, `smoke`, ...) use the
WebDriver engine, which refuses the plugin.

</Tab>

<Tab title="SB_PLUGINS" icon="terminal">

With no code, set `SB_PLUGINS=fetcher`. The options still come from
`[plugins.fetcher]` or from `--plugin-opt`.

</Tab>

<Tab title="sbrs command line" icon="terminal">

The task is run by name. `plugin run` turns the plugin on for that run, so
`--plugin fetcher` is not needed:

```sh Terminal
sbrs plugin run fetcher --input urls.json --out pages.jsonl
```

A global option sets one of the plugin's options. Put the whole option in single
quotes so that the shell keeps the quotes TOML needs:

```sh Terminal
sbrs --plugin-opt 'fetcher.max_body_bytes=500000' \
  --plugin-opt 'fetcher.strip=["image","font"]' \
  plugin run fetcher --input urls.json
```

`urls.json` is the input of the task, see [The task](#the-task). To apply the
rules to a browser that `sbrs devtools` starts, name the plugin yourself:
`sbrs --plugin fetcher --plugin-opt 'fetcher.strip=["image"]' devtools`.

</Tab>

<Tab title="Rust code" icon="code">

In code, through the registry of built-in plugins:

```rust
use seleniumbase_rs::config::plugins::PluginSelection;
use seleniumbase_rs::json::json;
use seleniumbase_rs::sb_cdp::LaunchOptions;

let selection = PluginSelection::new()
    .enable("fetcher")
    .with_options("fetcher", json!({ "strip": ["image", "font"], "capture": ["*/api/*"] }));
let options = LaunchOptions::builder()
    .headless(true)
    .plugin_selection(&selection)
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

`build()` reports a mistake in the name or the options as
`SeleniumBaseError::Plugin`. To read the records of `capture`, keep a handle on
the plugin; see [Records](#records).

</Tab>

</Tabs>

## Rules

The plugin declares its rules once, and seleniumbase-rs runs them itself: the
plugin is not called for a request that a rule decides.

1. `strip` fails the requests of the kinds of resource it lists. Documents
   cannot be stripped, only blocked by address with `block`.
2. `block` fails the requests whose address matches a pattern. `*` matches any
   run of characters and `?` one character.
3. `headers` adds request headers, to every request, or only to those whose
   address matches a pattern of `header_urls`.
4. `capture` records the responses whose address matches a pattern, with the
   start of the body.

A failed request ends with Chrome's `ERR_BLOCKED_BY_CLIENT`. `strip` and `block`
overlap with [block-resources](/plugins/block-resources): that one is a preset
for the usual cases, this one is for rules of your own and for the records.

## Options

`[plugins.fetcher]` in a settings file. An option that is not listed is an
error, so a typo cannot go unnoticed.

<ParamField body="strip" type="string[]" default="[]">

Kinds of resource to fail: `stylesheet`, `image`, `media`, `font`, `script`,
`xhr`, `fetch`, `websocket` or `other`. Documents cannot be stripped.

</ParamField>

<ParamField body="block" type="string[]" default="[]">

Address patterns to fail (`*` any run of characters, `?` one).

</ParamField>

<ParamField body="capture" type="string[]" default="[]">

Address patterns whose responses are recorded.

</ParamField>

<ParamField body="headers" type="table" default="{}">

Request headers to add, such as `{ "Accept-Language" = "de" }`: at most 64. The
names must be header names; `Host`, `Content-Length`, `Transfer-Encoding`,
`Connection`, `Upgrade`, `Expect`, `TE` and `Trailer` cannot be set. A value
may not hold a line break and is at most 8192 bytes.

</ParamField>

<ParamField body="header_urls" type="string[]" default="[]">

Address patterns the `headers` go to. Empty means every request, third parties
included: give patterns when a header carries a secret.

</ParamField>

<ParamField body="max_body_bytes" type="integer" default="2097152">

The most bytes kept of one captured body and of one fetched body (2 MiB). `0`
keeps none. At most 67108864 (64 MiB).

</ParamField>

<ParamField body="max_captures" type="integer" default="500">

The most records kept; the oldest go first. At most 100000. A `capture` list
with `max_captures = 0` is an error.

</ParamField>

A pattern is bad when it is empty or holds a space or a control character: no
address does, so it could never match.

## Records

A record is `{url, method, status, mime, text or base64, truncated, outcome,
headers, at_ms}`:

| Field | Meaning |
|---|---|
| `url`, `method` | The address and the HTTP method |
| `status` | The HTTP status; `0` when there was no response |
| `mime` | The MIME type, such as `application/json` |
| `text` | The body as text, for a text type (`text/*` or a type with `json`, `xml`, `javascript`, `ecmascript`, `urlencoded` or `svg` in it) and for a body that is valid UTF-8 |
| `base64` | The body in base64, for anything else |
| `truncated` | Whether the body was cut at `max_body_bytes` |
| `outcome` | `ok`, `failed` or `blocked` |
| `headers` | The request headers |
| `at_ms` | Milliseconds since the Unix epoch when the response arrived |

The plugin does not hold `net.sensitive`, so `Cookie`, `Authorization`,
`Proxy-Authorization` and `Set-Cookie` read `[redacted]` in every record. A
response that announces a `Content-Length` of more than 16 MiB gets no body,
whatever `max_body_bytes` says, because seleniumbase-rs does not read it.

The log lives in the plugin object. Build the plugin yourself and share it with
the set to read it:

```rust
use std::sync::Arc;

use seleniumbase_rs::builtin_plugins::fetcher::{Fetcher, FetcherConfig};
use seleniumbase_rs::plugin_api::PluginSet;

let fetcher = Arc::new(Fetcher::new(FetcherConfig {
    capture: vec!["*/api/*".into()],
    ..FetcherConfig::default()
})?);
let plugins = PluginSet::new().with_shared(fetcher.clone())?;
// ... give `plugins` to LaunchOptions::builder().plugins(plugins), browse, then:
for record in fetcher.take_captured() {
    println!("{} {} {}", record.method, record.url, record.status);
}
# Ok::<(), seleniumbase_rs::plugin_api::PluginError>(())
```

`captured()` returns the log without emptying it, `take_captured()` empties it,
`captured_len()` counts it and `dropped()` counts the records that were pushed out
because the log was full. From the command line the log is not written anywhere:
the summary of the task has only the number of records, `captured`.

## The task

The task fetches a list of addresses in one tab, one after the other, with
`fetch(url, { credentials: "include" })` inside the page. The request therefore
obeys that page's cross-origin rules, carries the cookies the browser holds for
the site, and goes through the proxy and privacy plugins like any other request.
The address travels to the page as a JSON literal and is never pasted into the
script. Only `http` and `https` addresses without credentials are accepted.

```json urls.json
{"urls": ["https://example.com/a", "https://example.com/b"], "delay_ms": 500, "warm_up": true}
```

<ParamField body="urls" type="string[]" required>

The addresses, at most 10000.

</ParamField>

<ParamField body="delay_ms" type="integer" default="0">

A pause between two fetches, at most 60000.

</ParamField>

<ParamField body="warm_up" type="boolean" default="true">

The tab first goes to the front page of a site (`https://example.com/`) before
the first fetch from it, so the fetch is same-origin and carries the site's
cookies. `false` fetches from wherever the tab is.

</ParamField>

```sh Terminal
sbrs plugin run fetcher --input urls.json --out pages.jsonl
```

Each address writes one line:

```json pages.jsonl
{"index": 0, "url": "https://example.com/a", "status": 200, "mime": "text/html", "text": "<!doctype html>...", "truncated": false}
{"index": 1, "url": "https://example.com/b", "error": "net::ERR_FAILED"}
```

and the task returns, on standard error, `{"total": 2, "ok": 1, "failed": 1, "captured": 0}`.
An address that fails is a line with an `error`, and the others go on. A missing
capability stops the task. The tab the task opened is closed whatever happened.

An address that is not a plain web address fails in its own line, with the
plugin's message, for example
``plugin configuration: fetcher: `ftp://c.test/4` is not an http or https address``.
The input itself is checked before a tab opens:

| Input | Message |
|---|---|
| No `urls`, or an empty list | ``plugin configuration: fetcher: the task input needs `urls`, a list of http(s) addresses`` |
| More than 10000 addresses | ``plugin configuration: fetcher: too many urls (at most 10000)`` |
| A `delay_ms` over 60000 | ``plugin configuration: fetcher: `delay_ms` is at most 60000`` |
| Another key | ``plugin configuration: fetcher: bad task input: ...`` with the key named |

Other plugins can use the same fetch as a job: `{"url": "https://example.com/api/items", "max_bytes": 100000}`
gives `{"url", "status", "mime", "text", "truncated"}`, and `max_bytes` never
exceeds `max_body_bytes`. [batcher](/plugins/batcher) calls it with its `runner`
option set to `fetcher`.

## What it may do

`sbrs plugin info fetcher` shows what the plugin declares. It is a `core`
plugin for Chromium browsers:

| Capability | Why |
|---|---|
| `net.intercept` | It declares rules (`strip`, `block`, `headers`) that seleniumbase-rs runs |
| `net.observe` | It watches the responses that `capture` names |
| `task` | It is run by name, by `sbrs plugin run fetcher` |
| `page.evaluate` | The task runs `fetch` inside a page |
| `page.navigate` | The task opens the front page of a site before it fetches from it |
| `net.bodies` (optional) | Without it a record has no body |

It does not ask for `net.sensitive`, for a host capability or for a browser
launch capability.

## What seleniumbase-rs does for it

The plugin only declares and records. The pure CDP engine applies the
declarations:

| Plugin declares | The host |
|---|---|
| Rules | One request router per tab owns `Fetch`; a plugin never gets `Fetch.*` itself. A blocked request is failed with `BlockedByClient` and reported as the event `cdp.plugin.request.failed` whose `reason` starts with the plugin's name |
| `headers` | The headers are added to the request, under the headers of a rule of the program's own (`Page::intercept`), which win |
| `capture` | Chrome pauses the response only for the addresses the filters name; the router reads the body up to `max_body_bytes` and hands the response to the plugin |

A rule of the program's own that fulfils or blocks a request is final and the
plugin is not asked. When both the program and the plugins block, any block wins.

## Test it

The plugin crate depends on the plugin API only, so a test needs no browser.
`seleniumbase-rs-plugin-testing` has `drive_request` and `drive_response`, which
run a request or a response through a set the way the host does:

```rust
use seleniumbase_rs_plugin::{Configurable, PluginSet, RequestView, ResourceKind};
use seleniumbase_rs_plugin_fetcher::Fetcher;
use seleniumbase_rs_plugin_testing::drive_request;
use serde_json::json;

let fetcher = Fetcher::configure(&json!({ "strip": ["image"] }))?;
let set = PluginSet::strict().with(fetcher)?;
let image = RequestView::new("https://a.test/x.png", "GET", ResourceKind::Image);
assert!(matches!(drive_request(&set, image), seleniumbase_rs_plugin::RequestDecision::Block(_)));
# Ok::<(), seleniumbase_rs_plugin::PluginError>(())
```

For the task, `VecSink` collects the lines, `RecordingPage::evaluating(text, reply)`
answers the page script, and `ManualTimer` stands in for the clock; the plugin's
own `tests/fetcher.rs` is the example.

## Limits

<Warning>

**Nothing here has run in a real Chrome from this documentation.** The plugin's
own tests use the recording fakes of `seleniumbase-rs-plugin-testing`, and the
checks in seleniumbase-rs that run without a browser show how the command line
refuses what it should. The path from `sbrs plugin run fetcher` through the
built-in registry and the task host to a real headless Chrome has a test of its
own, `crates/cli/tests/cli_plugins_chrome.rs`, which is `#[ignore]`d. Run it
before you rely on the plugin:

```sh Terminal
cargo test -p seleniumbase-rs-cli --test cli_plugins_chrome -- --ignored --test-threads=1
```

</Warning>

- **Pure CDP engine and Chromium only.** The WebDriver engine refuses the
  plugin before a browser starts, with a message that names the plugin and the
  capability the host lacks, such as ``plugin `fetcher` needs `task`, which WebDriver with Chromium does not support``.
  With `capture` set it names `net.observe`, and with rules (`strip`, `block`,
  `headers`) and no `capture` it names `net.intercept`. With Firefox or Safari
  the message says the plugin needs a Chromium-based browser.
- **The fetch obeys the page.** A `fetch` from the tab follows the cross-origin
  rules of the page it runs in, so an address of another site fails unless that
  site allows it. That is why the task goes to the front page of each site
  first.
- **Text and JSON, not files.** A fetched body is decoded as UTF-8.
- **Redaction.** The headers of a record never show the cookie or the
  authorization of the request, because the plugin does not hold `net.sensitive`.
- **A header you add reaches third parties** unless `header_urls` limits it. Give
  patterns when a header carries a secret.
- **It is not a sandbox.** Like every native plugin, fetcher is Rust code that
  runs with your program's rights.
