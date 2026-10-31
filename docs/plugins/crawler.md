---
title: "crawler Plugin"
sidebarTitle: "crawler"
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugin-crawler</Badge>

Crawls a site from its links and reports what is broken: pages that do not load
or answer an error status, addresses that must be reachable and are not, and
errors in the browser console. A test can fail on that report. The report uses
the wording of Python SeleniumBase's `assert_no_404_errors`.

<Note>

Use it only on sites you own or have permission to test. It follows plain
`<a href>` links with `http` and `https` addresses. It never clicks, submits a
form or runs a page's scripts on purpose. It reads `robots.txt` and obeys it
unless you turn that off, waits between pages and stops at a page limit. It
needs a Chromium browser and the pure CDP engine: the WebDriver engine cannot
host a task, so it refuses the plugin before a browser starts.

</Note>

<Visibility for="agents">

Summary for AI exports: crawler is a built-in testing plugin (feature `plugin-crawler`, name `crawler`) for Chromium on the pure CDP engine, for sites the operator owns or may test. Options live in `[plugins.crawler]`: `start` (addresses), `scope` (`same-origin` by default, `same-site`, or `{ hosts = [...] }`), `include` and `exclude` (patterns), `max_depth` (3, at most 100), `max_pages` (500, from 1 to 50000), `delay_ms` (1000, at most 60000), `respect_robots` (true), `max_links` (1000, from 1 to 100000), `state_file` (none) and the table `expect` with `max_status` (399), `must_reach` (empty) and `no_console_errors` (false). An unknown option or a value out of range is an error. Its task, run with `sbrs plugin run crawler --input FILE`, takes `{start?, resume?}` and crawls breadth first, one navigation at a time, never leaving the scope. It records one line for each page (`index`, `url`, `depth`, `from`, `status`, `title`, `links`, `queued`, or `error`, or `skipped: "robots.txt"`) and returns the report `{pages, edges, broken, js_errors, js_errors_total, unreached, robots_blocked, truncated, passed, summary, resumed}`. The summary uses the wording of Python's `assert_no_404_errors`. As a test plugin it fails the next `on_test_end` with the summary unless the report passed. Nothing in it is shown to work in a real Chrome by tests that run by default.

</Visibility>

## Turn it on

crawler ships with seleniumbase-rs behind one cargo feature, and is switched on
by its name. See [Choosing plugins](/plugins/selecting) for every way to do that.

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["plugin-crawler"] }
```

Then switch it on by name and give it the site:

<Tabs>

<Tab title="sbase_config.toml" icon="gear">

With no code, in `sbase_config.toml`:

```toml sbase_config.toml
[plugins]
enable = ["crawler"]

[plugins.crawler]
start = ["https://staging.example.com/"]
scope = "same-origin"
exclude = ["/logout*", "*?print=1"]
max_depth = 4
max_pages = 200
delay_ms = 500
state_file = "crawl-state.json"

[plugins.crawler.expect]
must_reach = ["https://staging.example.com/pricing"]
no_console_errors = true
```

The settings file can turn crawler on by itself: it declares no `host.*`
capability, no `launch.proxy` and no `artifacts.write`. A `[plugins.crawler]`
table can come from a project you cloned, so look at `state_file` before you run
the task of a project you did not write: a state file that belongs to a crawl
with other options is ignored and overwritten.

</Tab>

<Tab title="SB_PLUGINS" icon="terminal">

With no code, set `SB_PLUGINS=crawler`. The options still come from
`[plugins.crawler]` or from `--plugin-opt`.

</Tab>

<Tab title="sbrs command line" icon="terminal">

The task is run by name. `plugin run` turns the plugin on for that run, so
`--plugin crawler` is not needed:

```sh Terminal
sbrs plugin run crawler --input crawl.json --out pages.jsonl
```

A global option sets one of the plugin's options; put the whole option in
single quotes so that the shell keeps the quotes TOML needs:

```sh Terminal
sbrs --plugin-opt 'crawler.max_pages=50' \
  --plugin-opt 'crawler.exclude=["/logout*"]' \
  plugin run crawler --input crawl.json
```

`crawl.json` is the input of the task, see [The task](#the-task).

</Tab>

<Tab title="Rust code" icon="code">

In code, through the registry of built-in plugins:

```rust
use seleniumbase_rs::config::plugins::PluginSelection;
use seleniumbase_rs::json::json;
use seleniumbase_rs::sb_cdp::LaunchOptions;

let selection = PluginSelection::new().enable("crawler").with_options(
    "crawler",
    json!({ "start": ["https://staging.example.com/"], "max_pages": 20 }),
);
let options = LaunchOptions::builder()
    .headless(true)
    .plugin_selection(&selection)
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

`build()` reports a mistake in the name or the options as
`SeleniumBaseError::Plugin`. To run the task from Rust, use
`plugin_host::run_task(options, "crawler", &input, &sink, &prompt)`.

</Tab>

</Tabs>

## Options

`[plugins.crawler]` in a settings file. An option that is not listed is an
error, and so is a value out of range, for example
``plugin configuration: crawler: `max_pages` must be 1 to 50000``. Each list
(`start`, `scope.hosts`, `include`, `exclude`, `expect.must_reach`) holds at most
1000 entries.

<ParamField body="start" type="string[]" default="[]">

Where the crawl starts: `http` or `https` addresses. The task input may give
`start` instead; one of them is required.

</ParamField>

<ParamField body="scope" type="string or table" default="same-origin">

`"same-origin"` (the scheme, host and port of a start address), `"same-site"`
(the host of a start address and its subdomains, any scheme and port; the
registrable part is a rule of thumb and not the public suffix list, so
`example.co.uk` counts as one site) or `{ hosts = ["example.com"] }` (those hosts
and their subdomains).

</ParamField>

<ParamField body="include" type="string[]" default="[]">

Patterns a followed link must match, if there are any (`*` any run of
characters, `?` one). A pattern that starts with `/` is matched against the path
and the query, any other against the whole address. Start addresses are not
filtered.

</ParamField>

<ParamField body="exclude" type="string[]" default="[]">

Patterns of links not to follow, matched like `include`.

</ParamField>

<ParamField body="max_depth" type="integer" default="3">

How many links deep the crawl goes; the start addresses are depth 0. At most 100.

</ParamField>

<ParamField body="max_pages" type="integer" default="500">

The most pages loaded, from 1 to 50000.

</ParamField>

<ParamField body="delay_ms" type="integer" default="1000">

The pause between two navigations, at most 60000. A `Crawl-delay` in robots.txt
can make it longer, but never more than a minute.

</ParamField>

<ParamField body="respect_robots" type="boolean" default="true">

Read robots.txt and obey it. `false` is for sites you own or may test.

</ParamField>

<ParamField body="max_links" type="integer" default="1000">

The most links read from one page, from 1 to 100000.

</ParamField>

<ParamField body="state_file" type="string" default="none">

A JSON file that keeps the crawl so that it can be resumed, see
[Resuming](#resuming).

</ParamField>

<ParamField body="expect.max_status" type="integer" default="399">

A page above it is broken, from 100 to 599. The default makes every 4xx and 5xx
broken; Python SeleniumBase counts only 404. A page that does not load counts as
404, as in Python.

</ParamField>

<ParamField body="expect.must_reach" type="string[]" default="[]">

Addresses the crawl must load. One that it did not load, or that was broken, is
`unreached`.

</ParamField>

<ParamField body="expect.no_console_errors" type="boolean" default="false">

Console errors fail the report. They are recorded either way.

</ParamField>

Other mistakes in the options, each reported as ``plugin configuration: crawler: `` followed by:

| Mistake | Message |
|---|---|
| A start or `must_reach` address that is not http or https | `` `X` in `start` is not an http or https address `` |
| `scope = { hosts = [] }` | `` `scope.hosts` needs at least one host `` |
| A host with a scheme, a port or a path | `` bad host `X` in `scope.hosts` (a name such as `example.com`, with no scheme, port or path) `` |
| An empty pattern, or one with a space | `` bad glob `X` in `include` `` |
| `max_depth` over 100 | `` `max_depth` is at most 100 `` |
| `delay_ms` over 60000 | `` `delay_ms` is at most 60000 `` |
| `max_links` out of range | `` `max_links` must be 1 to 100000 `` |
| `expect.max_status` out of range | `` `expect.max_status` must be 100 to 599 `` |
| A list with more than 1000 entries | `` `start` holds at most 1000 entries `` |

## The task

The input is `{"start": ["https://staging.example.com/"], "resume": true}`. Both
keys are optional; without `start` the option of that name is used.

```json crawl.json
{"start": ["https://staging.example.com/"], "resume": true}
```

```sh Terminal
sbrs plugin run crawler --input crawl.json --out pages.jsonl
```

One tab does the work, breadth first, one navigation at a time:

1. The address is checked against the robots.txt of its site, which is opened in
   the tab once for each site like any page (so a proxy and the privacy plugins
   apply). A 4xx answer means no rules; a 5xx answer, or none, closes the site to
   the crawl. A group for `sbrs` wins over `*`, the longest rule wins, `Allow`
   wins a tie, and `*` and a trailing `$` work in a rule.
2. The page is opened. One script reads its title, its HTTP status (from the
   navigation timing entry, else from the response the host reported) and its
   `a[href]` links. A page with `<meta name="robots" content="nofollow">` is read
   but its links are not followed.
3. The links are normalised (lower-case scheme and host, default port and
   fragment dropped, dot segments resolved) and queued unless they are known,
   outside the scope, filtered by `include` or `exclude`, or deeper than
   `max_depth`.

A link outside the scope is never navigated. While a crawl runs, the plugin
cancels such a navigation with the error ``navigation to X cancelled by plugin crawler: X is outside the crawl scope``.
A page that redirects out of the scope is recorded, and its links are not
followed.

Each page writes one line:

```json pages.jsonl
{"index": 0, "url": "https://staging.example.com/", "depth": 0, "from": null, "status": 200, "title": "Home", "links": 12, "queued": 9}
{"index": 3, "url": "https://staging.example.com/old", "depth": 1, "from": "https://staging.example.com/", "status": 404, "title": "Not found", "links": 0, "queued": 0}
{"index": 4, "url": "https://staging.example.com/x", "depth": 2, "from": "https://staging.example.com/a", "error": "net::ERR_CONNECTION_RESET"}
{"url": "https://staging.example.com/admin", "depth": 1, "from": "https://staging.example.com/", "skipped": "robots.txt"}
```

A line may also carry `final_url` (after a redirect), `nofollow` or `error`. The
task returns the report, on standard error:

```json
{"pages": 31, "edges": 120, "broken": [{"url": "https://staging.example.com/old", "from": "https://staging.example.com/", "status": 404}],
 "js_errors": [], "js_errors_total": 0, "unreached": [], "robots_blocked": 1, "truncated": false,
 "passed": false, "summary": "Broken link detected:\nhttps://staging.example.com/old", "resumed": false}
```

`summary` uses the wording of Python's `assert_no_404_errors`: `Broken link
detected:` or `Broken links detected:`, then the addresses sorted, one on each
line. A crawl without a broken page reads `Crawled 3 pages: no broken links`, and
one that loaded no page reads `No page was crawled` and does not pass. The report
keeps the first 200 console errors and counts the rest in `js_errors_total`.

These stop the task before a page is loaded:

| Problem | Message |
|---|---|
| No `start` in the input or the options | `` plugin configuration: crawler: give `start` addresses in the options or in the task input `` |
| More than 1000 start addresses | `plugin configuration: crawler: too many start addresses (at most 1000)` |
| A start address that is not http or https | `` plugin configuration: crawler: `X` is not an http or https address `` |
| A start address outside the scope | `` plugin configuration: crawler: the start address `X` is outside the crawl scope `` |
| Another key in the input | `plugin configuration: crawler: bad task input: ...` |
| A second run on the same plugin while one is running | `crawler: a crawl is already running` |

## As a test plugin

The plugin hears the end of a test. After a crawl, the next `on_test_end` fails
the test with the summary unless the report passed: no broken page, every
`must_reach` address loaded, and no console error when `no_console_errors` is
set. A report judges one test. In Rust, `Crawler::report()` returns the report of
the last finished crawl. The hooks that make this work, `before_navigate`,
`on_response`, `on_protocol_event` and `on_test_end`, are described under
[More hooks](/tutorials/plugins#more-hooks).

## Resuming

With `state_file` the crawl is written to `FILE.tmp` and renamed over the file
every few pages and at the end. A later run goes on from it when the options that
decide which pages are crawled (`start`, `scope`, `include`, `exclude`,
`max_depth`, `respect_robots`) are the same. A file of another crawl is ignored,
the result says `state_ignored`, and it is overwritten. `max_pages`, `delay_ms`
and `expect` may change, so a crawl that stopped at the page limit can be resumed
with a higher one. `"resume": false` in the input starts over.

```json crawl-state.json
{"version": 1, "config_hash": "9f2c1d0a7b3e4a55", "visited": [], "frontier": [["https://staging.example.com/a", 1]],
 "edges": [["https://staging.example.com/", "https://staging.example.com/a"]], "broken": [], "pages": 1, "blocked": []}
```

## What it may do

`sbrs plugin info crawler` shows what the plugin declares. It is a `testing`
plugin for Chromium browsers:

| Capability | Why |
|---|---|
| `task` | It is run by name, by `sbrs plugin run crawler` |
| `page.navigate` | It opens the pages |
| `page.evaluate` | One script reads the title, the status and the links |
| `net.observe` | It watches the documents it loads, to learn their status |
| `test.hooks` | It fails the test whose crawl did not pass |
| `host.fs.read`, `host.fs.write` for one file | Only with `state_file`: the file and the `.tmp` file beside it, and no other |

It holds no `net.intercept`, `net.bodies` or `net.sensitive`. It asks the host for
the DevTools events `Runtime.exceptionThrown` and `Log.entryAdded`, from which it
reads the console errors.

## Test it

The plugin crate depends on the plugin API only, so a test needs no browser:

```rust
use seleniumbase_rs_plugin::{Configurable, PluginSet};
use seleniumbase_rs_plugin_crawler::Crawler;
use serde_json::json;

let crawler = Crawler::configure(&json!({
    "start": ["https://example.com/"],
    "max_pages": 20,
}))?;
let set = PluginSet::strict().with(crawler)?;
assert_eq!(set.task_names(), ["crawler"]);
# Ok::<(), seleniumbase_rs_plugin::PluginError>(())
```

The plugin's own `tests/crawler.rs` drives a whole crawl against a fake tab.

## Limits

<Warning>

**It has not been shown to work in a real Chrome by tests that run by default.**
The plugin's tests drive a whole crawl against the recording fakes of
`seleniumbase-rs-plugin-testing`. The plugin crate also has an `#[ignore]`d test,
`crawler_chrome`, that crawls a small local site in headless Chrome; the plugins
repository's CHANGELOG records that it passed, with a stand-in tab and not the
host's pure CDP engine. The checks in seleniumbase-rs show how the command line
refuses what it should and do not crawl. Run the crawl on a site you own before
you rely on it.

</Warning>

- **Pure CDP engine and Chromium only.** The WebDriver engine refuses the
  plugin before a browser starts, with a message that names the plugin and the
  capability the host lacks, such as ``plugin `crawler` needs `net.observe`, which WebDriver with Chromium does not support``.
- **Pages only.** Images, scripts and stylesheets are not checked. A link outside
  the scope is neither followed nor checked; Python sends a HEAD request for every
  link, and this crawler never leaves the scope.
- **One crawl at a time for each plugin.** A second `run` fails with
  `crawler: a crawl is already running`.
- **Bounded memory.** At most 200000 addresses are queued and 200000 links
  recorded as `edges`, and the report keeps the first 200 console errors.
- **The report is returned, not saved.** Writing it as an artifact waits for an
  artifact API in the plugin host, so the plugin does not declare
  `artifacts.write`.
- **Console errors need the host.** They come from `Runtime.exceptionThrown` and
  `Log.entryAdded` events that the host has to deliver to the plugin.
- **The site is untrusted.** Titles, addresses and console messages in the report
  are text from the pages. Do not let a model act on instructions found in them.
- **It is not a sandbox.** Like every native plugin, crawler is Rust code that
  runs with your program's rights.
