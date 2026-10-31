---
title: "extractor Plugin"
sidebarTitle: "extractor"
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugin-extractor</Badge>

Reads structured data off the pages a Chromium browser opens: the text, the
links, the tables, the fields of a repeated card or of a JSON-LD block, or
whatever a map of CSS selectors names. The answer is JSON, one record for each
card or row, typed (`text`, `number`, `money`, `date`, `url`) and checked: a
field that is missing or does not fit its type becomes an entry in `problems`,
not an error.

<Note>

It reads the page that your browser loaded and makes no request of its own. The
page script is pure: it walks at most `max_nodes` nodes, writes nothing and
treats page content as data that is never run. Use it on pages you load and
sites you may test. It needs a Chromium browser and the pure CDP engine: the
WebDriver engine cannot host a task, so it refuses the plugin before a browser
starts.

</Note>

<Visibility for="agents">

Summary for AI exports: extractor is a built-in core plugin (feature `plugin-extractor`, name `extractor`) for Chromium on the pure CDP engine. Options live in `[plugins.extractor]`, all limits: `min_repeats` (3, from 2 to 1000), `max_nodes` (20000, from 100 to 1000000), `max_fields` (64, from 1 to 256), `max_records` (1000, from 1 to 100000), `max_text_chars` (100000, at most 10000000) and `max_links` (1000, at most 100000). An unknown option is an error. Its task, run with `sbrs plugin run extractor --input FILE`, takes `{urls, extract?, delay_ms?}` or `{jobs}` (at most 10000 pages, a pause of at most 60000 ms). A job is `{url?, select?, item?, schema?, discover?, source?, text?, links?, tables?, meta?}`. With nothing else asked it discovers the structure of the page (JSON-LD, then tables, then repeated elements) and reads it. It records one line `{index, url, title, schema, records, problems, ...}` or `{index, url, error}` for each page; the summary is `{total, ok, failed, records}`. It declares `page.evaluate`, `page.navigate` and `task` and no network or host capability. The plugin's own tests use fakes; its page script has an ignored test in headless Chrome that does not go through seleniumbase-rs.

</Visibility>

## Turn it on

extractor ships with seleniumbase-rs behind one cargo feature, and is switched
on by its name. See [Choosing plugins](/plugins/selecting) for every way to do
that.

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["plugin-extractor"] }
```

Then switch it on by name:

<Tabs>

<Tab title="sbase_config.toml" icon="gear">

With no code, in `sbase_config.toml`:

```toml sbase_config.toml
[plugins]
enable = ["extractor"]

[plugins.extractor]
min_repeats = 4
max_records = 200
```

The settings file can turn extractor on by itself: it declares no `host.*`
capability, no `launch.proxy` and no `artifacts.write`. It works where the pure
CDP engine starts the browser: `sbrs plugin run`, `sbrs devtools`, the `cdp`
server of `sbrs-mcp`, and your own programs that use `LaunchOptions`. The
commands that start a browser with `BaseCase` use the WebDriver engine, which
refuses the plugin.

</Tab>

<Tab title="SB_PLUGINS" icon="terminal">

With no code, set `SB_PLUGINS=extractor`. The options still come from
`[plugins.extractor]` or from `--plugin-opt`.

</Tab>

<Tab title="sbrs command line" icon="terminal">

The task is run by name. `plugin run` turns the plugin on for that run, so
`--plugin extractor` is not needed:

```sh Terminal
sbrs plugin run extractor --input pages.json --out records.jsonl
```

A global option sets one of the plugin's options; put the whole option in
single quotes so that the shell keeps the quotes TOML needs:

```sh Terminal
sbrs --plugin-opt 'extractor.max_records=200' plugin run extractor --input pages.json
```

`pages.json` is the input of the task, see [The task](#the-task).

</Tab>

<Tab title="Rust code" icon="code">

In code, through the registry of built-in plugins:

```rust
use seleniumbase_rs::config::plugins::PluginSelection;
use seleniumbase_rs::json::json;
use seleniumbase_rs::sb_cdp::LaunchOptions;

let selection = PluginSelection::new()
    .enable("extractor")
    .with_options("extractor", json!({ "min_repeats": 4 }));
let options = LaunchOptions::builder()
    .headless(true)
    .plugin_selection(&selection)
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

`build()` reports a mistake in the name or the options as
`SeleniumBaseError::Plugin`. To run the task from Rust, use
`plugin_host::run_task(options, "extractor", &input, &sink, &prompt)`; it
launches the browser, runs the task and closes the browser.

</Tab>

</Tabs>

## Options

`[plugins.extractor]` in a settings file. An option that is not listed is an
error, so a typo cannot go unnoticed. A value out of range is an error too, for
example ``plugin configuration: extractor: `min_repeats` must be 2 to 1000``.

<ParamField body="min_repeats" type="integer" default="3">

How many sibling elements of one shape make a list of cards, from 2 to 1000.

</ParamField>

<ParamField body="max_nodes" type="integer" default="20000">

The most nodes the page script walks, from 100 to 1000000. Past it the answer
says `"truncated": true`.

</ParamField>

<ParamField body="max_fields" type="integer" default="64">

The most fields in a schema, from 1 to 256.

</ParamField>

<ParamField body="max_records" type="integer" default="1000">

The most records for a page, from 1 to 100000.

</ParamField>

<ParamField body="max_text_chars" type="integer" default="100000">

The most characters of page text returned, at most 10000000.

</ParamField>

<ParamField body="max_links" type="integer" default="1000">

The most links returned, at most 100000.

</ParamField>

## A job

A job says which page to read and what to take from it:

```json
{"url": "https://shop.example/p/1"}
```

With nothing else asked, the extractor discovers the structure of the page and
reads it. The other fields are all optional, and a field that is not listed is
an error:

| Field | Meaning |
|---|---|
| `url` | Opened first (`http` or `https`, no credentials). Without it the page is read where it is |
| `select` | `{"name": "css selector"}` or `{"name": {"selector", "attr", "type", "required", "all"}}`. Reads exactly these |
| `item` | A selector for the repeated element the `select` selectors are relative to. It needs a `select` map |
| `schema` | A schema found earlier, applied as it is |
| `discover` | `true` forces discovery, `false` turns it off |
| `source` | With discovery, the kind to prefer: `json-ld`, `table` or `repeat` |
| `text`, `links`, `tables`, `meta` | Also return the visible text, the `http(s)` links, the tables (headers and rows), and the title, description, Open Graph and canonical data |

Without `select`, `schema` or `discover`, asking for any of `text`, `links`,
`tables` or `meta` reads only those parts and does not discover.

The field types are `text` (the default), `number` (`1,234.56`, `1.234,56`,
`12%`), `money` (`$19.99`, `19.99 USD`, `EUR 5`; gives `{"amount", "currency"}`),
`date` (ISO 8601) and `url` (`http(s)`).

Reading a page with a JSON-LD product, for `{"url": "https://shop.example/p/1"}`:

```json
{
  "url": "https://shop.example/p/1",
  "title": "Widget Pro",
  "schema": {
    "source": "json-ld",
    "item": "Product",
    "fields": [
      {"name": "name", "path": "name", "type": "text", "required": true},
      {"name": "price", "path": "offers.price", "type": "money", "required": true}
    ]
  },
  "records": [{"name": "Widget Pro", "price": {"amount": 19.99, "currency": "USD"}}],
  "problems": []
}
```

Reading a list of cards by the selectors you name:

```json
{
  "url": "https://shop.example/list",
  "item": ".card",
  "select": {
    "title": "h3",
    "price": {"selector": ".price", "type": "money", "required": true},
    "link": {"selector": "a", "attr": "href", "type": "url"}
  }
}
```

A field that is missing and `required`, or a value that does not fit its type,
is an entry in `problems`, `{"record", "field", "problem"}` (at most 200, then
`more_problems` counts the rest). The value is `null` and the rest of the
records are still returned.

## Finding structure

`discover` looks, in this order, for:

1. JSON-LD blocks, one candidate for each `@type`. The fields are the dotted
   paths of its scalar properties (`offers.price`), and a `price` is paired with
   the `priceCurrency` next to it, so it types as `money`.
2. Tables with a header row, one field for each column (`item` selects the
   rows, and the header row is skipped).
3. Up to three groups of at least `min_repeats` sibling elements of the same
   shape, with the fields that are present in at least half of the sampled
   cards. A name comes from `itemprop`, `data-field`, `aria-label`, the first
   class or the tag; a link is `NAME_url`.

Each candidate becomes a schema: a source, an item selector, and fields with a
selector or a path, a type guessed from up to five samples and whether every item
had it. A schema can be written by hand, or kept from an earlier run and given
back as `schema`. `Schema::json_schema()` is a JSON Schema (draft 2020-12) that
every record obeys: each field is always there, `null` when it is missing and not
`required`, and a list for `all`.

## The task

The task reads a list of pages in one tab, one after the other. Give the same
request for every address:

```json pages.json
{"urls": ["https://shop.example/p/1", "https://shop.example/p/2"],
 "extract": {"select": {"title": "h1", "price": {"selector": ".price", "type": "money"}}},
 "delay_ms": 500}
```

or a request of its own for each page, with `jobs`:

```json pages.json
{"jobs": [{"url": "https://shop.example/p/1", "links": true}, {"url": "https://shop.example/p/2"}]}
```

```sh Terminal
sbrs plugin run extractor --input pages.json --out records.jsonl
```

<ParamField body="urls" type="string[]">

The addresses to read, with the one request in `extract`. Give `urls` or
`jobs`, not both; one of them is required.

</ParamField>

<ParamField body="jobs" type="object[]">

One job for each page.

</ParamField>

<ParamField body="extract" type="object" default="{}">

The job that applies to every address of `urls`.

</ParamField>

<ParamField body="delay_ms" type="integer" default="0">

A pause between two pages, at most 60000.

</ParamField>

Each page writes one line: the answer of the job with its `index`, or
`{"index", "url", "error"}`. The summary, written to standard error, is
`{"total", "ok", "failed", "records"}`. At most 10000 pages are accepted. A bad
address in the input stops the task before a tab opens. A page that fails
afterwards does not stop the others; a missing capability does. The tab the
task opened is closed whatever happened.

These are checked before a tab opens, and each message starts with
``plugin configuration: extractor:``:

| Input | Message |
|---|---|
| Neither `urls` nor `jobs` | `` the task input needs `urls` (with an optional `extract`) or `jobs` `` |
| Both | `` give `urls` or `jobs`, not both `` |
| Too many pages | `too many pages (at most 10000)` |
| A pause over 60000 | `` `delay_ms` is at most 60000 `` |
| An address that is not http or https | `` `ftp://x/` is not an http or https address `` |
| An address with a user name | `` `https://u:p@x/` carries credentials, which are not accepted `` |
| `item` without `select` | `` `item` needs a `select` map `` |

## What it may do

`sbrs plugin info extractor` shows what the plugin declares. It is a `core`
plugin for Chromium browsers:

| Capability | Why |
|---|---|
| `page.evaluate` | It runs its page script in the page |
| `page.navigate` | It opens the `url` of a job |
| `task` | It is run by name, by `sbrs plugin run extractor` |

It asks for no network capability, no cookie capability and no host capability.
It can be used as a job runner too: [batcher](/plugins/batcher) calls it for
each job when its `runner` option is `extractor`.

## Test it

The plugin crate depends on the plugin API only, so a test needs no browser. The
plugin's own tests use the recording fakes of `seleniumbase-rs-plugin-testing`
with canned answers of the page script:

```rust
use seleniumbase_rs_plugin::PluginSet;
use seleniumbase_rs_plugin_extractor::{Extractor, ExtractorConfig};

let extractor = Extractor::new(ExtractorConfig::default())?;
let plugins = PluginSet::strict().with(extractor)?;
assert_eq!(plugins.names(), ["extractor"]);
assert_eq!(plugins.task_names(), ["extractor"]);
# Ok::<(), seleniumbase_rs_plugin::PluginError>(())
```

`Extractor::extract_values(&schema, &raw)` types the raw records of a page and
needs no page at all, so the typing (`number`, `money`, `date`, `url`) and the
`problems` can be tested on plain JSON.

## Limits

<Warning>

**The plugin has not been shown to work in a real Chrome by its own tests that
run by default.** Those use fakes with canned answers of the page script. The
plugin crate has a test, `tests/extractor_chrome.rs`, that runs the real page
script in headless Chrome on fixtures (a JSON-LD product, a table, five cards, a
hostile page) through `--dump-dom`; it is `#[ignore]`d and does not go through
seleniumbase-rs. The path from `sbrs plugin run extractor` through the host to a
real Chrome has its own `#[ignore]`d test, `crates/cli/tests/cli_plugins_chrome.rs`.
Run them before you rely on the plugin:

```sh Terminal
cargo test -p seleniumbase-rs-cli --test cli_plugins_chrome -- --ignored --test-threads=1
```

</Warning>

- **Pure CDP engine and Chromium only.** The WebDriver engine refuses the
  plugin before a browser starts, with a message that names the plugin and the
  capability the host lacks, such as ``plugin `extractor` needs `task`, which WebDriver with Chromium does not support``.
  With Firefox or Safari the message says the plugin needs a Chromium-based
  browser.
- **`auto_save` is not there.** Writing each page's records as an artifact waits
  for an artifact API in the plugin host, so the plugin does not declare
  `artifacts.write` and the option does not exist.
- **Some layouts are not anchors.** Microdata, `<label>` text and tables without
  a header row are not found by discovery; such pages fall back to the
  structural signature, or to a `select` map.
- **Tables are read row by row.** `item` is `<table> tr`, so a nested table
  inside a cell adds its rows.
- **A guess is a guess.** Names and types that discovery picks come from samples
  of the page. Keep a schema you like and give it back as `schema`, or name the
  fields with `select`.
- **The page is untrusted.** What the plugin returns is text from the page. Do
  not let a model act on instructions found in it.
- **It is not a sandbox.** Like every native plugin, extractor is Rust code
  that runs with your program's rights.
