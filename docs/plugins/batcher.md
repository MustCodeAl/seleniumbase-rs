---
title: "batcher Plugin"
sidebarTitle: "batcher"
---

<Badge color="orange">Early</Badge> <Badge color="blue">Feature: plugin-batcher</Badge>

Runs another plugin's jobs over a list of inputs: a few at a time, each in a tab
of its own, with a time limit, retries with a growing pause, and a resume file
that lets a second run skip what the first one finished. It writes one JSON
line for each job.

<Note>

It works on what your own browser session does: it opens tabs, hands each one to
a job runner and closes it. It adds no traffic of its own, hides nothing and
sends nothing anywhere else. Use it on pages you load and sites you may test.
It is not a test runner (the `-n` of `sbrs test` belongs to that runner); it
runs data jobs. It needs a Chromium browser and the pure CDP engine: the
WebDriver engine cannot host a task or give browser contexts, so it refuses the
plugin before a browser starts.

</Note>

<Visibility for="agents">

Summary for AI exports: batcher is a built-in environment plugin (feature `plugin-batcher`, name `batcher`) for Chromium on the pure CDP engine. Options live in `[plugins.batcher]`: `concurrency` (2, from 1 to 32), `chunk_size` (25, from 1 to 10000), `job_timeout_secs` (60, from 1 to 3600), `retries` (1, from 0 to 3), `backoff_ms` (500, at most 60000, doubled at each retry, no jitter), `runner` (`open`, or the name of a plugin with a job runner: `fetcher` or `extractor`), `max_jobs` (10000, from 1 to 1000000), `ordered` (false), `warm_up` (unset: on for the `fetcher` runner, off for the others) and `resume_file` (none). An unknown option or a value out of range is an error. Its task, run with `sbrs plugin run batcher --input FILE`, takes `{jobs: [...]}`; the shape of a job is the runner's. It runs the jobs in chunks, each chunk in a browser context of its own, each job in a tab of its own, and records one line `{chunk, index, job, outcome}` for each job with `outcome` `{status: "ok", attempts, result}`, `{status: "failed", attempts, error, denied?}` or `{status: "skipped"}`; the summary is `{total, ok, failed, skipped, retried, chunks}`. With `resume_file` it writes down the finished jobs by a hash and skips them in a second run. A job that a plugin refused is never retried. With `runner` set to a plugin, that plugin must be in the set (`--plugin fetcher`). The plugin's own tests use fakes; nothing in it is shown to work in a real Chrome.

</Visibility>

## Turn it on

batcher ships with seleniumbase-rs behind one cargo feature, and is switched on
by its name. See [Choosing plugins](/plugins/selecting) for every way to do that.

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs", features = ["plugin-batcher", "plugin-fetcher"] }
```

The second feature is for the `fetcher` runner of the examples below; the
default runner, `open`, needs nothing else. Then switch it on by name:

<Tabs>

<Tab title="sbase_config.toml" icon="gear">

With no code, in `sbase_config.toml`:

```toml sbase_config.toml
[plugins]
enable = ["fetcher", "batcher"]

[plugins.batcher]
concurrency = 4
chunk_size = 50
retries = 2
runner = "fetcher"
resume_file = "done.jsonl"
```

The settings file can turn batcher on by itself: the plugin declares no `host.*`
capability. Because a `[plugins.batcher]` table can come from a project you
cloned, look at `resume_file` before you run the task of a project you did not
write: the plugin creates that file and appends to it.

</Tab>

<Tab title="SB_PLUGINS" icon="terminal">

With no code, set `SB_PLUGINS=fetcher,batcher`. The options still come from
`[plugins.batcher]` or from `--plugin-opt`.

</Tab>

<Tab title="sbrs command line" icon="terminal">

The task is run by name. `plugin run` turns the named plugin on for that run, so
`--plugin batcher` is not needed. The plugin whose runner does the work is
another plugin and has to be named too:

```sh Terminal
sbrs plugin run batcher --input jobs.json --out results.jsonl \
  --plugin fetcher --plugin-opt 'batcher.runner="fetcher"'
```

With the default runner there is nothing else to name:

```sh Terminal
sbrs plugin run batcher --input jobs.json --out results.jsonl
```

`jobs.json` is the input of the task, see [The task](#the-task).

</Tab>

<Tab title="Rust code" icon="code">

In code, through the registry of built-in plugins:

```rust
use seleniumbase_rs::config::plugins::PluginSelection;
use seleniumbase_rs::json::json;
use seleniumbase_rs::sb_cdp::LaunchOptions;

let selection = PluginSelection::new()
    .enable("fetcher")
    .enable("batcher")
    .with_options("batcher", json!({ "concurrency": 4, "runner": "fetcher" }));
let options = LaunchOptions::builder()
    .headless(true)
    .plugin_selection(&selection)
    .build()?;
# Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
```

`build()` reports a mistake in the name or the options as
`SeleniumBaseError::Plugin`. To run the task from Rust, use
`plugin_host::run_task(options, "batcher", &input, &sink, &prompt)`.

</Tab>

</Tabs>

## The task

The input is a list of jobs. What a job looks like is up to the runner:

```json jobs.json
{"jobs": [{"url": "https://example.com/a"}, {"url": "https://example.com/b"}]}
```

| Runner | A job | The answer |
|---|---|---|
| `open` (the default) | `{"url": "https://..."}`, http or https without credentials | `{"title", "url"}`: the title, and the address the tab ended on |
| `fetcher` | `{"url", "max_bytes"?}` | `{"url", "status", "mime", "text", "truncated"}`, see [fetcher](/plugins/fetcher) |
| `extractor` | its own request, `{"url", "select"?, ...}` | `{"url", "title", "schema", "records", "problems"}`, see [extractor](/plugins/extractor) |

Each job writes one line:

```json results.jsonl
{"chunk": 0, "index": 0, "job": {"url": "https://example.com/a"}, "outcome": {"status": "ok", "attempts": 1, "result": {"title": "Example", "url": "https://example.com/a"}}}
{"chunk": 0, "index": 1, "job": {"url": "https://example.com/b"}, "outcome": {"status": "failed", "attempts": 2, "error": "net::ERR_NAME_NOT_RESOLVED"}}
{"chunk": null, "index": 2, "job": {"url": "https://example.com/c"}, "outcome": {"status": "skipped"}}
```

`index` is the place of the job in the input, so lines that come in the order the
jobs finish can be put back in order (or set `ordered`). `chunk` counts the
contexts of this run; a job that the resume file skipped has none. A failed job
has `"denied": true` when a plugin's policy refused it. The summary, written to
standard error, is

```json
{"total": 3, "ok": 1, "failed": 1, "skipped": 1, "retried": 1, "chunks": 1}
```

where `retried` counts the extra attempts of all jobs.

How it runs the jobs:

1. The jobs that the resume file marks as done are written first, as `skipped`.
2. The rest go to the runner in chunks of `chunk_size`. Each chunk gets a browser
   context of its own, which is closed with its tabs when the chunk is done.
   Chunks run one after the other.
3. In a chunk, at most `concurrency` jobs run at once. Each attempt of each job
   gets a tab of its own and the time limit `job_timeout_secs`; the tab is closed
   whatever happened.
4. With `warm_up`, the tab first goes to the front page of the job's site, so a
   `fetch` in the page is same-origin. It needs a `url` in the job; a job without
   one is run as it is.

These are checked before a browser context is made, and each message starts with
``plugin configuration: batcher:``:

| Input | Message |
|---|---|
| No `jobs`, or no input | `` the task input needs `jobs`, a list of jobs `` |
| An empty list | `` `jobs` is empty `` |
| More than `max_jobs` | `too many jobs` |
| Another key, or `jobs` that is not a list | `bad task input: ...` |

## Retries, time limits and denials

- A time limit, a failing hook and a browser error are tried again, up to
  `retries` times. The pause before the first retry is `backoff_ms`, each later
  one is twice as long, at most a minute. There is no jitter.
- A job that a plugin's policy refused is never tried again: a navigation a plugin
  cancelled, or a request it blocked (`ERR_BLOCKED_BY_CLIENT`). Neither is a bad
  job (a URL that is not http or https, a key the runner does not know).
- A job that fails is a line, and the others go on. A missing capability ends the
  run, because every job would fail the same way; so does a sink that cannot
  write. The context and its tabs are closed first. The lines already written
  stay.
- Dropping the task (Ctrl-C in `sbrs plugin run`) stops everything at once and
  nothing is tried again.

## The resume file

With `resume_file`, a job that finished well is written down after its line
reached the sink. A second run with the same jobs and the same runner skips those
jobs and runs the rest, the jobs that failed included. A crash between the line
and the entry runs the job again rather than losing its line.

The file is JSON lines, `{"key": "<32 hex digits>", "index": 3}`. The key is a
hash of the runner and the job: no job text is stored, so a job that carries a
token leaves none in the file. A job counts as the same job whatever the order of
its keys, but a different runner is a different job. A last line cut off by a
crash is ignored and dropped from the file; any other line that is not an entry
stops the run before a context is made. The file is created if it is missing and is never removed.

| Problem | Message |
|---|---|
| The file cannot be read, rewritten or opened (`WHAT` is `read`, `write` or `open`) | `` plugin configuration: batcher: cannot WHAT the resume file `PATH`: ERROR `` |
| A line that is not an entry | `` plugin configuration: batcher: line N of the resume file `PATH` is not an entry: ERROR `` |
| An entry cannot be written during the run | `` batcher: cannot write the resume file `PATH`: ERROR `` |

## Options

`[plugins.batcher]` in a settings file. An option that is not listed is an
error, and so is a value out of range, for example
``plugin configuration: batcher: `concurrency` is from 1 to 32``.

<ParamField body="concurrency" type="integer" default="2">

Jobs in flight at once, each in a tab of its own, from 1 to 32. Raise it when the
machine and the site can take it.

</ParamField>

<ParamField body="chunk_size" type="integer" default="25">

Jobs served by one browser context, from 1 to 10000. Then the context and its
tabs are closed and a fresh one is made, so that cookies do not pile up and a
privacy plugin such as [tor](/plugins/tor) can give each chunk its own circuit.

</ParamField>

<ParamField body="job_timeout_secs" type="integer" default="60">

The time limit of one attempt, from 1 to 3600.

</ParamField>

<ParamField body="retries" type="integer" default="1">

How often a failed job is tried again, from 0 to 3.

</ParamField>

<ParamField body="backoff_ms" type="integer" default="500">

The pause before the first retry, from 0 to 60000. Each later retry waits twice
as long, at most a minute.

</ParamField>

<ParamField body="runner" type="string" default="open">

`open`, or the name of the plugin whose job runner does the work: `fetcher` or
`extractor`. That plugin becomes a dependency and has to be in the set. A name
with a space, or `batcher` itself, is an error.

</ParamField>

<ParamField body="max_jobs" type="integer" default="10000">

The most jobs one run accepts, from 1 to 1000000.

</ParamField>

<ParamField body="ordered" type="boolean" default="false">

`true` writes the lines in the order of the jobs, `false` in the order they
finish.

</ParamField>

<ParamField body="warm_up" type="boolean" default="unset">

Whether a tab goes to the front page of the job's site (its `url`) before the
runner starts. Unset means on for the `fetcher` runner and off for every other.

</ParamField>

<ParamField body="resume_file" type="string" default="none">

A file that lists the jobs already done. It is created if it is missing. An empty
name is an error.

</ParamField>

If `runner` names a plugin that is not in the set, the check every launch makes
refuses it before a browser starts, with the plugin API's message: ``plugin `batcher` needs plugin `fetcher`: add the crate that provides it to the set``. If the
plugin has no job runner, the task stops with
``plugin configuration: batcher: runner `crx`: plugin `crx` offers no job runner``.

## What it may do

`sbrs plugin info batcher` shows what the plugin declares. It is an
`environment` plugin for Chromium browsers:

| Capability | Why |
|---|---|
| `task` | It is run by name, by `sbrs plugin run batcher` |
| `contexts` | It makes a browser context for each chunk |
| `page.navigate` | The `open` runner and the warm-up open pages |
| `page.evaluate` | The `open` runner reads the title |
| `host.fs.read`, `host.fs.write` for one path | Only with `resume_file`: the resume file is the only file it touches |

The runner sees a tab as its own plugin does: with that plugin's capabilities
and no more. The batcher's tabs are checked against the batcher's capabilities
too.

## Test it

The plugin crate depends on the plugin API only, so a test needs no browser:

```rust
use seleniumbase_rs_plugin::{Configurable, PluginSet};
use seleniumbase_rs_plugin_batcher::Batcher;
use serde_json::json;

let batcher = Batcher::configure(&json!({ "concurrency": 4, "retries": 2 }))?;
let set = PluginSet::strict().with(batcher)?;
assert_eq!(set.task_names(), ["batcher"]);
# Ok::<(), seleniumbase_rs_plugin::PluginError>(())
```

With `RecordingBrowser::with_contexts()` the fakes count the live contexts and
tabs, so a test can check that `concurrency` is kept; the plugin's own
`tests/run.rs` is the example.

## Limits

<Warning>

**Nothing here has run in a real Chrome.** The plugin's own tests use the
recording fakes of `seleniumbase-rs-plugin-testing` and the real `fetcher`. The
checks in seleniumbase-rs show how the command line refuses what it should
(a misspelt option, the WebDriver engine) and never start the task. Try it on a
small list of pages you own before you rely on it.

</Warning>

- **Pure CDP engine and Chromium only.** The WebDriver engine refuses the
  plugin before a browser starts, with a message that names the plugin and the
  capability the host lacks, such as ``plugin `batcher` needs `task`, which WebDriver with Chromium does not support``.
- **It is not a test runner.** It runs data jobs, one tab for each attempt.
- **The host makes the context.** Whether the `before_context` hook of another
  plugin runs for it, as [tor](/plugins/tor) needs for circuits of their own, is up
  to the host's `new_context`; this was not checked here.
- **Retries cost.** A retry is a new attempt in a new tab, so a job that has side
  effects on the site can have them twice. Keep `retries` at 0 for such jobs.
- **It is not a sandbox.** Like every native plugin, batcher is Rust code that
  runs with your program's rights.
