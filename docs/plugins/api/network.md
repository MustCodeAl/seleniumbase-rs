---
title: "Network"
sidebarTitle: "Network"
description: "Reference for request filters, rules, RequestView, responses and DevTools events: what a plugin declares and what PluginSet does with a paused request."
---

What a plugin may see and change on the network. The host owns interception.
A plugin states what it cares about in [`Plugin::network`](#pluginnetwork);
the set reads that once, when the plugin is added. Items are in
`seleniumbase_rs_plugin`, defined in
`crates/seleniumbase-rs-plugin/src/network.rs` of
[seleniumbase-rs-plugins](https://github.com/MustCodeAl/seleniumbase-rs-plugins)
at `fea3634e2ebcaafce63a244df8d4adb53e3ec002` (crate 0.1.3).

Rules run without calling the plugin: block a URL, or add request headers.
`observe` filters name the responses [`Plugin::on_response`](#pluginon_response)
sees. `intercept` filters name the requests
[`Plugin::on_request`](#pluginon_request) may block. The host asks
[`PluginSet::interest`](#pluginsetinterest) which requests to pause, so the
browser stops only the matching ones.

What a plugin may declare follows its capabilities. Rules and intercept
filters need [`Capability::NetIntercept`](/plugins/api/capabilities). Observe
filters need `Capability::NetObserve`. Response bodies need
`Capability::NetBodies`. The headers `Cookie`, `Authorization`,
`Proxy-Authorization` and `Set-Cookie` need `Capability::NetSensitive`;
without it their values read [`REDACTED`](#redacted).

A document load the page starts itself is a request with
[`RequestView::navigation`](#requestview) set. A navigation the program
starts (`Page::goto`, the MCP `navigate` tool) is a
[navigation hook](/plugins/api/navigation), not this one.

<RequestExample>

```rust Block ads and watch documents
use seleniumbase_rs_plugin::{
    async_trait, Capability, Category, Metadata, NetRule, NetworkInterest, Plugin,
    RequestDecision, RequestFilter, RequestView, ResourceKind, Result,
};

pub struct Ads;

#[async_trait]
impl Plugin for Ads {
    fn name(&self) -> &str {
        "ads"
    }

    fn metadata(&self) -> Metadata {
        Metadata::new(Category::Privacy).capabilities([
            Capability::NetIntercept,
            Capability::NetObserve,
            Capability::NetBodies,
        ])
    }

    fn network(&self) -> NetworkInterest {
        NetworkInterest::none()
            .rule(NetRule::block(
                RequestFilter::any().url("*://ads.example/*"),
            ))
            .rule(NetRule::set_headers(
                RequestFilter::any().url("https://api.example/*"),
                [("X-From", "plugin")],
            ))
            .intercept(RequestFilter::any().kinds([ResourceKind::Document]))
            .observe(
                RequestFilter::any()
                    .kinds([ResourceKind::Xhr])
                    .max_body(4096),
            )
    }

    async fn on_request(&self, request: &RequestView) -> Result<RequestDecision> {
        if request.navigation && request.url.starts_with("file:") {
            return Ok(RequestDecision::Block("local files stay closed".into()));
        }
        Ok(RequestDecision::Continue)
    }
}
```

```rust What the set decides
use seleniumbase_rs_plugin::{
    PluginSet, RequestDecision, RequestView, ResourceKind,
};
use seleniumbase_rs_plugin_testing::block_on;

let set = PluginSet::new().with(Ads)?;
let ad = RequestView::new(
    "https://ads.example/banner.js",
    "GET",
    ResourceKind::Script,
);
let decision = block_on(set.on_request(&ad));
assert!(matches!(decision, RequestDecision::Block(_)));

let page = RequestView::new("file:///tmp/x.html", "GET", ResourceKind::Document)
    .navigation(true);
let decision = block_on(set.on_request(&page));
assert!(matches!(decision, RequestDecision::Block(_)));

let api = RequestView::new("https://api.example/v1", "GET", ResourceKind::Fetch);
assert_eq!(
    set.extra_headers(&api).get("X-From").map(String::as_str),
    Some("plugin")
);
```

</RequestExample>

## ResourceKind

<Badge color="blue">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::ResourceKind`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceKind {
    Document,
    Stylesheet,
    Image,
    Media,
    Font,
    Script,
    Xhr,
    Fetch,
    WebSocket,
    Other,
}
```

What a request is for. `Document` is a page or frame, `Media` is audio or
video, `Xhr` is an `XMLHttpRequest`, `Fetch` is a `fetch()` call, and
`WebSocket` is the handshake. A `match` outside the crate needs a wildcard
arm.

## RequestFilter

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::RequestFilter`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RequestFilter {
    pub url: Option<String>,
    pub kinds: Vec<ResourceKind>,
    pub methods: Vec<String>,
    pub max_body: Option<usize>,
}
```

Which requests a rule or a hook is for. A filter that says nothing matches
every request; each field that is set narrows it. Build one with
`RequestFilter::any()` and the methods below. Code outside the crate cannot
use a struct literal.

<ParamField body="url" type="Option<String>">

A glob over the whole URL. `*` matches any run of characters, including
none, and `?` matches one character. `None` matches every URL.

</ParamField>

<ParamField body="kinds" type="Vec<ResourceKind>">

The kinds of resource. Empty matches every kind.

</ParamField>

<ParamField body="methods" type="Vec<String>">

The HTTP methods, compared without regard to case. Empty matches every method.

</ParamField>

<ParamField body="max_body" type="Option<usize>">

For an `observe` filter, the most response-body bytes the plugin wants.
`None` means no body. It only matters to a plugin that holds
`Capability::NetBodies`.

</ParamField>

<ResponseField name="any" type="fn() -> Self">

A filter that matches every request. This is `Default`.

</ResponseField>

<ResponseField name="url" type="fn(self, glob: impl Into<String>) -> Self">

Narrows the filter to URLs that match `glob`.

</ResponseField>

<ResponseField name="kinds" type="fn(self, kinds: impl IntoIterator<Item = ResourceKind>) -> Self">

Narrows the filter to these kinds of resource.

</ResponseField>

<ResponseField name="methods" type="fn(self, methods) -> Self">

Narrows the filter to these HTTP methods. `methods` is any iterator of
items that convert into `String`.

</ResponseField>

<ResponseField name="max_body" type="fn(self, bytes: usize) -> Self">

Asks for up to `bytes` of each response body.

</ResponseField>

<ResponseField name="matches" type="fn(&self, request: &RequestView) -> bool">

Whether `request` is one the filter selects.

</ResponseField>

## RuleAction

<Badge color="blue">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::RuleAction`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuleAction {
    Block,
    SetHeaders(BTreeMap<String, String>),
}
```

What the host does with a request a rule matches. `Block` fails the request.
`SetHeaders` adds those headers. The plugin is not called.

## NetRule

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::NetRule`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct NetRule {
    pub filter: RequestFilter,
    pub action: RuleAction,
}
```

A rule the host runs without calling the plugin. It needs
`Capability::NetIntercept`. Build one with `NetRule::block` or
`NetRule::set_headers`.

<ResponseField name="block" type="fn(filter: RequestFilter) -> Self">

A rule that blocks the requests `filter` selects.

</ResponseField>

<ResponseField name="set_headers" type="fn(filter: RequestFilter, headers) -> Self">

A rule that adds `headers` to the requests `filter` selects. Each pair
converts into `(String, String)`.

</ResponseField>

## NetworkInterest

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::NetworkInterest`

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct NetworkInterest {
    pub rules: Vec<NetRule>,
    pub observe: Vec<RequestFilter>,
    pub intercept: Vec<RequestFilter>,
}
```

What a plugin wants to know about the network.
[`Plugin::network`](#pluginnetwork) returns one; the set copies it when the
plugin is added.

<ParamField body="rules" type="Vec<NetRule>">

Rules the host runs. The plugin is never called for them.

</ParamField>

<ParamField body="observe" type="Vec<RequestFilter>">

Requests whose responses go to `Plugin::on_response`.

</ParamField>

<ParamField body="intercept" type="Vec<RequestFilter>">

Requests that go to `Plugin::on_request`.

</ParamField>

<ResponseField name="none" type="fn() -> Self">

No interest in the network. This is the default.

</ResponseField>

<ResponseField name="is_empty" type="fn(&self) -> bool">

Whether nothing is declared.

</ResponseField>

<ResponseField name="rule" type="fn(self, rule: NetRule) -> Self">

Adds a rule.

</ResponseField>

<ResponseField name="observe" type="fn(self, filter: RequestFilter) -> Self">

Adds a filter for the responses to observe.

</ResponseField>

<ResponseField name="intercept" type="fn(self, filter: RequestFilter) -> Self">

Adds a filter for the requests to intercept.

</ResponseField>

## RequestView

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::RequestView`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RequestView {
    pub url: String,
    pub method: String,
    pub kind: ResourceKind,
    pub headers: BTreeMap<String, String>,
    pub navigation: bool,
    pub page: Option<PageId>,
    pub context: Option<ContextId>,
}
```

A request, as a plugin sees it. Build one with `RequestView::new` and the
methods below.

<ParamField body="url" type="String" required>

The address.

</ParamField>

<ParamField body="method" type="String" required>

The HTTP method, such as `GET`.

</ParamField>

<ParamField body="kind" type="ResourceKind" required>

What the request is for.

</ParamField>

<ParamField body="headers" type="BTreeMap<String, String>" required>

The request headers. Sensitive ones are replaced with [`REDACTED`](#redacted)
unless the plugin holds `Capability::NetSensitive`.

</ParamField>

<ParamField body="navigation" type="bool" required>

Whether the request loads a document the page navigates to. A link click or
a script sets this. A navigation the program starts is a
[navigation hook](/plugins/api/navigation).

</ParamField>

<ParamField body="page" type="Option<PageId>">

The tab that sent it, when the host knows.

</ParamField>

<ParamField body="context" type="Option<ContextId>">

The context of that tab, when it has one.

</ParamField>

<ResponseField name="new" type="fn(url, method, kind: ResourceKind) -> Self">

A request for `url` of `kind`, with no headers. `url` and `method` convert
into `String`.

</ResponseField>

<ResponseField name="header" type="fn(self, name, value) -> Self">

Adds a header. Both arguments convert into `String`.

</ResponseField>

<ResponseField name="navigation" type="fn(self, navigation: bool) -> Self">

Marks the request as a navigation.

</ResponseField>

<ResponseField name="page" type="fn(self, page: PageId) -> Self">

Sets the tab that sent it.

</ResponseField>

<ResponseField name="context" type="fn(self, context: ContextId) -> Self">

Sets the context of the tab.

</ResponseField>

<ResponseField name="redacted" type="fn(&self) -> Self">

A copy whose sensitive header values read [`REDACTED`](#redacted).

</ResponseField>

## REDACTED

<Badge color="gray">const</Badge> `seleniumbase_rs_plugin::REDACTED`

```rust
pub const REDACTED: &str = "[redacted]";
```

The value that stands in for a header the plugin may not read. The names are
`Cookie`, `Authorization`, `Proxy-Authorization` and `Set-Cookie`, compared
without regard to case.

## RequestDecision

<Badge color="blue">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::RequestDecision`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RequestDecision {
    Continue,
    Block(String),
}
```

What a plugin decides about a request. `Continue` lets it go. `Block` fails
it; the string says why. [`PluginSet::on_request`](#pluginseton_request)
puts the plugin's name in front of that string.

## Outcome

<Badge color="blue">enum</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::Outcome`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    Ok,
    Failed,
    Blocked { by: String },
}
```

How a request ended. `Ok` means it got a response. `Failed` means the
network failed it. `Blocked` means a plugin or the host blocked it, and
`by` names who.

## ResponseView

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::ResponseView`

```rust
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ResponseView<'a> {
    pub request: &'a RequestView,
    pub status: u16,
    pub mime: &'a str,
    pub body: Option<&'a [u8]>,
    pub truncated: bool,
    pub outcome: Outcome,
}
```

A response, as a plugin sees it.

<ParamField body="request" type="&RequestView" required>

The request it answers. Sensitive headers follow the same rule as
[`RequestView`](#requestview).

</ParamField>

<ParamField body="status" type="u16" required>

The HTTP status. `0` when there was no response.

</ParamField>

<ParamField body="mime" type="&str" required>

The MIME type, such as `text/html`.

</ParamField>

<ParamField body="body" type="Option<&[u8]>">

The body, only for a plugin that holds `Capability::NetBodies` and an
observe filter with a `max_body`, and cut to that size. The set uses the
largest `max_body` among the filters that matched.

</ParamField>

<ParamField body="truncated" type="bool" required>

Whether the body was cut.

</ParamField>

<ParamField body="outcome" type="Outcome" required>

How the request ended.

</ParamField>

<ResponseField name="new" type="fn(request: &RequestView, status: u16, mime: &str) -> Self">

A response with `status` and `mime` to `request`, with no body and
`Outcome::Ok`.

</ResponseField>

<ResponseField name="body" type="fn(self, body: &[u8]) -> Self">

Sets the body the host read.

</ResponseField>

<ResponseField name="truncated" type="fn(self, truncated: bool) -> Self">

Marks the body as already cut by the host.

</ResponseField>

<ResponseField name="outcome" type="fn(self, outcome: Outcome) -> Self">

Sets how the request ended.

</ResponseField>

## ProtocolEvent

<Badge color="blue">struct</Badge> <Badge color="gray" stroke>non_exhaustive</Badge>
`seleniumbase_rs_plugin::ProtocolEvent`

```rust
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ProtocolEvent {
    pub method: String,
    pub params: Value,
}
```

A Chrome DevTools Protocol event, for the plugins that asked for its name in
[`Plugin::protocol_events`](#pluginprotocol_events). `method` is the event
name, such as `Log.entryAdded`. `params` is its parameters. `Value` is
`serde_json::Value`, which a plugin crate depends on directly.

<ResponseField name="new" type="fn(method: impl Into<String>, params: Value) -> Self">

The event `method` with `params`.

</ResponseField>

## Plugin::network

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
fn network(&self) -> NetworkInterest {
    NetworkInterest::none()
}
```

What the plugin wants to know about the network. The set reads it once, when
the plugin is added. The default is nothing.

## Plugin::protocol_events

<Badge color="green">fn</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
fn protocol_events(&self) -> &[&str] {
    &[]
}
```

The names of the DevTools events the plugin wants in
[`on_protocol_event`](#pluginon_protocol_event), such as `Log.entryAdded`.
The default is none.

Events in the `Target` and `Browser` domains name other tabs. They are
delivered only when the plugin holds `Capability::Cdp` for that domain.
`Capability::Cdp` for `Target` or `Browser` is critical, so
[`PluginSet::with`](/plugins/api/plugin-set) does not grant it;
[`PluginSet::with_trusted`](/plugins/api/plugin-set) does.
`Capability::Raw(RawKind::Cdp)` covers every domain.

## Plugin::on_request

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
async fn on_request(&self, request: &RequestView) -> Result<RequestDecision> {
    Ok(RequestDecision::Continue)
}
```

A request the plugin named in `intercept` is paused. Reaches only plugins
that hold `Capability::NetIntercept`. An error blocks the request, and so
does a hook that runs past the set's time limit. The default lets the
request go.

## Plugin::on_response

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
async fn on_response(&self, response: &ResponseView<'_>) -> Result<()> {
    Ok(())
}
```

A response the plugin named in `observe` arrived. Reaches only plugins that
hold `Capability::NetObserve`. An error is logged. The default does nothing.
This hook never changes the response.

## Plugin::on_protocol_event

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge> <Badge color="gray" stroke>provided</Badge>

```rust
async fn on_protocol_event(
    &self,
    browser: &dyn BrowserHandle,
    event: &ProtocolEvent,
) -> Result<()> {
    Ok(())
}
```

A DevTools event the plugin named in `protocol_events` arrived. An error is
logged. The default does nothing.

## PluginSet::interest

<Badge color="green">fn</Badge>

```rust
pub fn interest(&self) -> NetworkInterest
```

What the host has to watch for. The plugins' declarations are merged in run
order and cut down to what each plugin may use.

- `rules` are those of plugins that hold `Capability::NetIntercept`, blocking
  rules first, then the others.
- `intercept` filters are those of the same plugins.
- `observe` filters are those of plugins that hold `Capability::NetObserve`.
  The `max_body` of a plugin without `Capability::NetBodies` is dropped.

## PluginSet::protocol_events

<Badge color="green">fn</Badge>

```rust
pub fn protocol_events(&self) -> Vec<String>
```

The event names the host has to deliver. They are the names the plugins
asked for, sorted, without the `Target` and `Browser` events of a plugin
that does not hold the matching `Capability::Cdp`.

## PluginSet::on_request

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
pub async fn on_request(&self, request: &RequestView) -> RequestDecision
```

Decides what happens to a request the host paused. Any block wins, whatever
the order.

For each plugin that holds `Capability::NetIntercept`, in run order:

- a blocking rule that matches blocks the request, even when `on_request` is
  not called;
- `on_request` runs when one of that plugin's `intercept` filters matches;
- a hook that returns an error, or that times out, blocks too.

The `Block` message starts with the name of the first plugin, in run order,
that blocked. A rule writes `{name}: blocked by a network rule`. A hook
writes `{name}: {reason}`. A timeout writes
`{name}: on_request timed out after {limit:?}`.

## PluginSet::extra_headers

<Badge color="green">fn</Badge>

```rust
pub fn extra_headers(&self, request: &RequestView) -> BTreeMap<String, String>
```

The headers the `SetHeaders` rules of plugins that hold
`Capability::NetIntercept` add to `request`. A later plugin's value wins for
the same header name.

## PluginSet::on_response

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
pub async fn on_response(&self, response: &ResponseView<'_>) -> Vec<PluginError>
```

Gives a response to each plugin that holds `Capability::NetObserve` and has
an `observe` filter that matches. The body reaches only plugins that hold
`Capability::NetBodies`, cut to the largest `max_body` of the filters that
matched. Every matching plugin runs even when one fails. The method returns
the failures and does not fail itself.

## PluginSet::on_protocol_event

<Badge color="green">fn</Badge> <Badge color="gray" stroke>async</Badge>

```rust
pub async fn on_protocol_event(
    &self,
    browser: &dyn BrowserHandle,
    event: &ProtocolEvent,
) -> Vec<PluginError>
```

Gives a protocol event to each plugin that asked for its name and is allowed
to receive it. Every such plugin runs even when one fails. The method
returns the failures and does not fail itself.
