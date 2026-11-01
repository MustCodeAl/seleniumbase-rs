---
title: "Navigation"
sidebarTitle: "Navigation"
description: "Reference for Navigation, NavOrigin, NavDecision, NavOutcome and the before_navigate and after_navigate hooks: how a plugin cancels or redirects a navigation the program starts."
---

What a plugin may do with a navigation the program starts, and how it learns
that the navigation ended. Items are in `seleniumbase_rs_plugin`, defined in
`crates/seleniumbase-rs-plugin/src/nav.rs` of
[seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)
at `fea3634e2ebcaafce63a244df8d4adb53e3ec002` (crate 0.1.3). The host calls
[`PluginSet::before_navigate`](#pluginsetbefore_navigate) before `Page::goto`
and the MCP `navigate` tool, and
[`PluginSet::after_navigate`](#pluginsetafter_navigate) when that navigation
ends. A link click or a script is not one of these: it arrives as a request
with [`RequestView::navigation`](/plugins/api/network#requestview) set.

<RequestExample>

```rust Cancel or redirect
use seleniumbase_rs_plugin::{
    async_trait, Capability, Category, Metadata, NavDecision, Navigation, PageHandle, Plugin,
    Result,
};

pub struct Gate;

#[async_trait]
impl Plugin for Gate {
    fn name(&self) -> &str {
        "gate"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Privacy).capability(Capability::PageNavigate)
    }

    async fn before_navigate(
        &self,
        _page: &dyn PageHandle,
        navigation: &Navigation,
    ) -> Result<NavDecision> {
        let url = navigation.url.to_ascii_lowercase();
        if url.starts_with("file:") {
            return Ok(NavDecision::Cancel("file URLs are refused".into()));
        }
        if let Some(rest) = url.strip_prefix("http://") {
            return Ok(NavDecision::Redirect(format!("https://{rest}")));
        }
        Ok(NavDecision::Continue)
    }
}
```

```rust The set applies the decision
use seleniumbase_rs_plugin::{NavOrigin, Navigation, PluginSet};
use seleniumbase_rs_plugin_testing::{block_on, RecordingPage};

let page = RecordingPage::new();
let set = PluginSet::new().with(Gate)?;
let asked = Navigation::new("http://example.com/docs", NavOrigin::Program);
let going = block_on(set.before_navigate(&page, &asked))?;
assert_eq!(going.url, "https://example.com/docs");
```

</RequestExample>

## MAX_REDIRECTS

<Badge color="gray">const</Badge> `seleniumbase_rs_plugin::MAX_REDIRECTS`

```rust
pub const MAX_REDIRECTS: usize = 5;
```

The most redirects plugins may apply to one navigation. The next redirect
after that is [`PluginError::Hook`](/plugins/api/plugin-trait#pluginerror)
from `before_navigate`, and the navigation does not start.

## NavOrigin

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::NavOrigin`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NavOrigin {
    Program,
    Mcp,
}
```

Who started a navigation. `Program` is the library or the command line. `Mcp`
is an MCP client. A `match` needs a wildcard arm.

## Navigation

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::Navigation`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Navigation {
    pub url: String,
    pub origin: NavOrigin,
}
```

A navigation about to start. Build one with `new` or `program`. Code outside
the crate cannot use a struct literal.

<ParamField body="url" type="String" required>

The address to open. After a redirect this is the address the later plugins
see, not the one the program first asked for.

</ParamField>

<ParamField body="origin" type="NavOrigin" required>

Who asked for it.

</ParamField>

<ResponseField name="Navigation::new" type="fn">

```rust
pub fn new(url: impl Into<String>, origin: NavOrigin) -> Self
```

A navigation to `url` asked for by `origin`.

</ResponseField>

<ResponseField name="Navigation::program" type="fn">

```rust
pub fn program(url: impl Into<String>) -> Self
```

A navigation to `url` asked for by the program (`NavOrigin::Program`).

</ResponseField>

## NavDecision

<Badge color="orange">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::NavDecision`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NavDecision {
    Continue,
    Redirect(String),
    Cancel(String),
}
```

What one plugin decides. The default hook returns `Continue`.

<ResponseField name="Continue" type="unit variant">

Go on, to the address the plugin was shown.

</ResponseField>

<ResponseField name="Redirect(String)" type="tuple variant">

Open this address instead. The plugin needs [`Capability::PageNavigate`](/plugins/api/capabilities).
Only `http`, `https`, `about:blank` and `data:` addresses are accepted
(compared without regard to case). A refused redirect is
`PluginError::Hook` and the navigation does not start. A redirect is applied
at once, so each later plugin sees the new address. A redirect to the address
already in force does not count toward [`MAX_REDIRECTS`](#max_redirects).

</ResponseField>

<ResponseField name="Cancel(String)" type="tuple variant">

Do not navigate. The string is the reason and reaches the caller as
[`PluginError::NavigationCancelled`](#pluginerrornavigationcancelled). The
first cancel wins; later plugins are not asked.

</ResponseField>

## NavOutcome

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge> `seleniumbase_rs_plugin::NavOutcome`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct NavOutcome {
    pub url: String,
    pub status: Option<u16>,
    pub error: Option<String>,
}
```

How a navigation ended. The host builds one and passes it to
`after_navigate`.

<ParamField body="url" type="String" required>

The address that was opened.

</ParamField>

<ParamField body="status" type="Option&lt;u16&gt;">

The HTTP status of the document, when the host knows it.

</ParamField>

<ParamField body="error" type="Option&lt;String&gt;">

Why the navigation failed, such as `net::ERR_NAME_NOT_RESOLVED`.

</ParamField>

<ResponseField name="NavOutcome::new" type="fn">

```rust
pub fn new(url: impl Into<String>) -> Self
```

A navigation to `url` with no status and no error.

</ResponseField>

<ResponseField name="NavOutcome::status" type="fn">

```rust
pub fn status(self, status: u16) -> Self
```

Sets the HTTP status.

</ResponseField>

<ResponseField name="NavOutcome::failed" type="fn">

```rust
pub fn failed(self, error: impl Into<String>) -> Self
```

Marks the navigation as failed.

</ResponseField>

<ResponseField name="NavOutcome::ok" type="fn">

```rust
pub fn ok(&self) -> bool
```

Whether the page loaded: no error, and a status below 400 when one is known.
A missing status with no error counts as loaded.

</ResponseField>

## Plugin::before_navigate

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
async fn before_navigate(
    &self,
    page: &dyn PageHandle,
    navigation: &Navigation,
) -> Result<NavDecision> {
    Ok(NavDecision::Continue)
}
```

The program is about to navigate `page`. Return [`NavDecision`](#navdecision).
An error cancels the navigation the same way a refused redirect does: the
host does not send the command. Redirecting needs
`Capability::PageNavigate`; cancelling does not.

## Plugin::after_navigate

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
async fn after_navigate(
    &self,
    page: &dyn PageHandle,
    outcome: &NavOutcome,
) -> Result<()> {
    Ok(())
}
```

A navigation the program started has ended. An error is logged. Every plugin
still runs.

## PluginSet::before_navigate

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
pub async fn before_navigate(
    &self,
    page: &dyn PageHandle,
    navigation: &Navigation,
) -> Result<Navigation>
```

Asks every plugin, in [run order](/plugins/api/plugin-set#run-order), and
returns the navigation to carry out. The first `Cancel` wins and no command
is sent. A `Redirect` replaces the address before the next plugin is asked.
At most [`MAX_REDIRECTS`](#max_redirects) changes of address are applied.

<ResponseField name="return" type="Result&lt;Navigation&gt;">

The navigation to open, after redirects. `Err(PluginError::NavigationCancelled)`
when a plugin cancelled it, or `Err(PluginError::Hook)` when a hook failed or
a redirect was refused. The navigation does not start.

</ResponseField>

## PluginSet::after_navigate

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
pub async fn after_navigate(
    &self,
    page: &dyn PageHandle,
    outcome: &NavOutcome,
) -> Vec<PluginError>
```

Tells every plugin how the navigation ended. Every plugin runs even when one
fails. The returned errors are for the caller to log; the method itself does
not fail.

## PluginError::NavigationCancelled

<Badge color="orange">enum variant</Badge> `seleniumbase_rs_plugin::PluginError`

```rust
NavigationCancelled {
    plugin: String,
    url: String,
    reason: String,
}
```

A plugin cancelled a navigation the program asked for. `url` is the address
after earlier plugins' redirects, not necessarily the one the program first
named. Text: `navigation to URL cancelled by plugin PLUGIN: REASON`.
