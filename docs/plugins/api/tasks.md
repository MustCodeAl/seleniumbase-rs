---
title: "Tasks"
sidebarTitle: "Tasks"
description: "Reference for Task, JobRunner, TaskContext, Sink, Prompt and Clock: work a plugin does when the user starts it, and the per-job piece a batcher calls."
---

Work a plugin does on its own, instead of reacting to the browser. A
[`Task`](#task) is a plugin the user starts by name with `sbrs plugin run`.
A [`JobRunner`](#jobrunner) is the smaller piece a batching plugin calls once
per job, on a page the caller opened. Items are in `seleniumbase_rs_plugin`,
defined in `crates/seleniumbase-rs-plugin/src/task.rs` of
[seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)
at `0666904475f4f1f422f8f426f169f6a0e533e824` (crate 0.1.3).
[`Clock`](#clock) lives in `timer.rs` in the same crate.

A task needs [`Capability::Task`](/plugins/api/capabilities).
[`PluginSet::with`](/plugins/api/plugin-set) grants it, because it is not
critical.

<RequestExample>

```rust A task that records its input
use seleniumbase_rs_plugin::{
    async_trait, Capability, Category, JobRunner, Metadata, PageHandle, Plugin, Result, Task,
    TaskContext,
};
use serde_json::{Value, json};

pub struct Echo;

#[async_trait]
impl Plugin for Echo {
    fn name(&self) -> &str {
        "echo"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Testing).capabilities([Capability::Task])
    }

    fn task(&self) -> Option<&dyn Task> {
        Some(self)
    }

    fn job_runner(&self) -> Option<&dyn JobRunner> {
        Some(self)
    }
}

#[async_trait]
impl Task for Echo {
    async fn run(&self, cx: &TaskContext<'_>) -> Result<Value> {
        cx.sink.record(cx.input)?;
        Ok(json!({"echoed": true}))
    }
}

#[async_trait]
impl JobRunner for Echo {
    async fn run_job(&self, _page: &dyn PageHandle, job: &Value) -> Result<Value> {
        Ok(job.clone())
    }
}
```

```rust Look the task up
use seleniumbase_rs_plugin::PluginSet;

let set = PluginSet::new().with(Echo)?;
assert_eq!(set.task_names(), ["echo"]);
let _task = set.task("echo")?;
let _runner = set.job_runner("echo")?;
```

</RequestExample>

## Sink

<Badge color="purple">trait</Badge> `seleniumbase_rs_plugin::Sink`

```rust
pub trait Sink: Send + Sync {
    fn record(&self, line: &Value) -> Result<()>;
}
```

Where a task writes its results, one JSON value at a time. On the command
line that is a JSONL file. In an MCP client it is progress. `record` returns
an error when the result could not be written. `Value` is
`serde_json::Value`.

## NullSink

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::NullSink`

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct NullSink;
```

A sink that drops everything. `record` returns `Ok(())`.

## Prompt

<Badge color="purple">trait</Badge> <Badge color="gray" stroke>async</Badge>
`seleniumbase_rs_plugin::Prompt`

```rust
#[async_trait]
pub trait Prompt: Send + Sync {
    async fn wait(&self, message: &str, page: &dyn PageHandle) -> Result<()>;
}
```

How a task asks a person to do something in the browser, such as signing in,
and waits until they are done. `page` is the tab they work in. An error
means nobody can be asked, or the wait ended.

## NoPrompt

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::NoPrompt`

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPrompt;
```

A prompt for a host where no person is at the keyboard. `wait` fails with
`PluginError::unsupported("waiting for a person")`.

## TaskContext

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::TaskContext`

```rust
#[non_exhaustive]
pub struct TaskContext<'a> {
    pub browser: &'a dyn BrowserHandle,
    pub input: &'a Value,
    pub sink: &'a dyn Sink,
    pub plugins: &'a PluginSet,
    pub clock: &'a Clock,
    pub prompt: &'a dyn Prompt,
}
```

What a task gets when it runs. `Debug` prints only `input`. Build one with
`TaskContext::new`; code outside the crate cannot use a struct literal.

<ParamField body="browser" type="&dyn BrowserHandle" required>

The browser to work with. See [Handles](/plugins/api/handles).

</ParamField>

<ParamField body="input" type="&Value" required>

The task's input, as the user gave it.

</ParamField>

<ParamField body="sink" type="&dyn Sink" required>

Where results go. Results written here stay written when the task fails.

</ParamField>

<ParamField body="plugins" type="&PluginSet" required>

The plugins of this run, so the task can reach a [`JobRunner`](#jobrunner)
by name.

</ParamField>

<ParamField body="clock" type="&Clock" required>

Sleeps, time limits and the current time. See [`Clock`](#clock).

</ParamField>

<ParamField body="prompt" type="&dyn Prompt" required>

How to ask a person for help.

</ParamField>

<ResponseField name="new" type="fn(browser, input, sink, plugins, clock, prompt) -> Self">

A context from its parts, in the field order above.

</ResponseField>

## Task

<Badge color="purple">trait</Badge> <Badge color="gray" stroke>async</Badge>
`seleniumbase_rs_plugin::Task`

```rust
#[async_trait]
pub trait Task: Send + Sync {
    async fn run(&self, cx: &TaskContext<'_>) -> Result<Value>;
}
```

Work a plugin does when the user starts it. The plugin needs
`Capability::Task`. `run` does the work and returns a summary. Results that
come one by one go to `cx.sink`. An error is whatever stopped the task; the
caller shows it.

## JobRunner

<Badge color="purple">trait</Badge> <Badge color="gray" stroke>async</Badge>
`seleniumbase_rs_plugin::JobRunner`

```rust
#[async_trait]
pub trait JobRunner: Send + Sync {
    async fn run_job(&self, page: &dyn PageHandle, job: &Value) -> Result<Value>;
}
```

One kind of job a batching plugin can hand out, such as fetching a URL.
`page` is a tab the caller opened and will close. An error means the job
failed. Offering a job runner does not require `Capability::Task`.

## Plugin::task

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
fn task(&self) -> Option<&dyn Task> {
    None
}
```

The work this plugin does when the user starts it by name. The default is
none. A plugin that returns `Some` still needs `Capability::Task` before
[`PluginSet::task`](#pluginsettask) will hand it out.

## Plugin::job_runner

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
fn job_runner(&self) -> Option<&dyn JobRunner> {
    None
}
```

A kind of job that a batching plugin can hand to this one. The default is
none.

## PluginSet::task

<Badge color="green">fn</Badge>

```rust
pub fn task(&self, name: &str) -> Result<&dyn Task>
```

The task of the plugin called `name`.

Returns [`PluginError::Config`](/plugins/api/plugin-trait#pluginerror) when
no plugin has that name, or the plugin offers no task. Returns
`PluginError::Capability` when the plugin offers a task and does not hold
`Capability::Task`.

## PluginSet::job_runner

<Badge color="green">fn</Badge>

```rust
pub fn job_runner(&self, name: &str) -> Result<&dyn JobRunner>
```

The job runner of the plugin called `name`. Returns `PluginError::Config`
when no plugin has that name, or the plugin offers no job runner. This
method does not check a capability.

## PluginSet::task_names

<Badge color="green">fn</Badge>

```rust
pub fn task_names(&self) -> Vec<&str>
```

The names of the plugins that offer a task, in run order. A name is listed
when `Plugin::task` returns `Some`, even if the plugin was not granted
`Capability::Task`. [`PluginSet::task`](#pluginsettask) still refuses that
plugin.

## Clock

<Badge color="blue">struct</Badge> `seleniumbase_rs_plugin::Clock`

```rust
pub struct Clock { /* private */ }

impl Clock {
    pub fn new(timer: Arc<dyn Timer>) -> Self;
    pub async fn sleep(&self, duration: Duration);
    pub fn timeout<F: Future>(
        &self,
        duration: Duration,
        future: F,
    ) -> impl Future<Output = Result<F::Output>>;
    pub fn now_ms(&self) -> u64;
}
```

What a task uses for sleeping, time limits and the current time. The API
crate has no runtime. seleniumbase-rs gives the set a timer that sleeps on
its own runtime (`PluginSet::with_timer`). A test gives it `ManualTimer`
from `seleniumbase-rs-plugin-testing`, which moves when the test says so.

`sleep` waits for `duration`. `now_ms` is milliseconds since the Unix epoch,
by the timer's clock. `timeout` runs `future` and gives up after `duration`.
The time starts when `timeout` is called, not when the result is first
polled. If the time runs out first, the future is `Err(PluginError::Timeout)`
and `future` is dropped.

## Timer

<Badge color="purple">trait</Badge> `seleniumbase_rs_plugin::Timer`

```rust
pub trait Timer: Send + Sync {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>>;
    fn now_ms(&self) -> u64;
}
```

A source of sleeps, so the API crate needs no async runtime. `sleep` is a
future that finishes after `duration`, counted from the call. `now_ms`
defaults to the system clock; a test timer returns its own time.

```rust A clock that times out on a manual timer
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs_plugin::{Clock, PluginError};
use seleniumbase_rs_plugin_testing::{ManualTimer, block_on};

let timer = Arc::new(ManualTimer::new());
let clock = Clock::new(timer.clone());
let waiting = clock.timeout(Duration::from_secs(5), std::future::pending::<()>());
timer.advance(Duration::from_secs(6));
assert!(matches!(block_on(waiting), Err(PluginError::Timeout(_))));
```
