---
title: "codec::header_spec"
sidebarTitle: "HeaderSpec"
description: "Header names, order and values per request shape in seleniumbase-rs-codec: HeaderSpec, HeaderValue placeholders, HeaderIdentity, RequestShape, the HTTP/2 priority table and HeaderSpecError."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

A `HeaderSpec` says which headers a request carries, in which order, with which
capitalisation, for two request shapes: a top-level navigation
(`RequestShape::Document`) and a script-issued request (`RequestShape::Fetch`).
Most values are literal. The ones that belong to the caller's identity are
placeholders, resolved at request time from a `HeaderIdentity`: the `User-Agent`,
`Accept-Language`, the client hints and the cookie. Because the caller supplies
them, any of them can change without touching the spec, and any header can be
replaced by name with `HeaderIdentity::with_override` or `HeaderSpec::with_header`.

A spec is built from a captured header list (`HeaderSpec::from_captured`), or
written by hand as plain data and checked with `HeaderSpec::validate`. Names keep
the capitalisation they were captured with (HTTP/1 shows it); an HTTP/2 sender
lowercases them. The HTTP/2 `priority` header is not part of the lists: it depends
on what the request is for, so it is looked up by the `Sec-Fetch-Dest` value with
`priority_for` and goes last in the header block.

<RequestExample>

```rust Render a captured list
use seleniumbase_rs_stealth::codec::header_spec::{HeaderIdentity, HeaderSpec, RequestShape};

fn main() -> Result<(), seleniumbase_rs_stealth::codec::header_spec::HeaderSpecError> {
    let captured = [("Accept", "*/*"), ("User-Agent", "Agent/1.0"), ("Host", "example.test")];
    let spec = HeaderSpec::from_captured(&captured, &captured)?;
    let identity = HeaderIdentity::new("Agent/2.0");
    let headers = spec.render(RequestShape::Fetch, &identity)?;
    // `Host` belongs to the connection; the user agent is the caller's.
    println!("{headers:?}");
    Ok(())
}
```

```rust The measured priority table
use seleniumbase_rs_stealth::evasion::fingerprint::OsType;
use seleniumbase_rs_stealth::codec::emulation;
use seleniumbase_rs_stealth::codec::BrowserKind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chrome = emulation::get(BrowserKind::Chrome, OsType::Macos)?;
    println!("{:?}", chrome.headers.priority_for("document"));
    println!("{:?}", chrome.headers.priority_for("empty"));
    Ok(())
}
```

</RequestExample>

<ResponseExample>

```text Render a captured list
[("Accept", "*/*"), ("User-Agent", "Agent/2.0")]
```

```text The measured priority table
Some("u=0, i")
Some("u=1, i")
```

</ResponseExample>

## `HeaderSpec`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeaderSpec {
    #[serde(default)]
    pub document: Vec<(String, HeaderValue)>,
    #[serde(default)]
    pub fetch: Vec<(String, HeaderValue)>,
    #[serde(default)]
    pub priority_by_dest: Vec<(String, String)>,
}
```

The header names, their order and their values, per request shape.

<ResponseField name="document" type="Vec<(String, HeaderValue)>">

Headers of a navigation, in order.

</ResponseField>

<ResponseField name="fetch" type="Vec<(String, HeaderValue)>">

Headers of a script-issued request, in order.

</ResponseField>

<ResponseField name="priority_by_dest" type="Vec<(String, String)>">

The HTTP/2 `priority` header value for each `Sec-Fetch-Dest` value. The measured
Chrome 155 table maps `document` to `u=0, i`, `empty` to `u=1, i`, `image` to
`i` and `style` to `u=0`.

</ResponseField>

### `HeaderSpec::from_captured`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn from_captured<N: AsRef<str>, V: AsRef<str>>(
    document: &[(N, V)],
    fetch: &[(N, V)],
) -> Result<Self, HeaderSpecError>
```

Builds a spec from the headers of one captured navigation and one captured fetch.
Pseudo-headers, the transport headers of `TRANSPORT_HEADERS` and the credential
headers are dropped. The identity headers become placeholders. A `priority` header
(an HTTP/2 capture) moves to the priority table, keyed by the same request's
`Sec-Fetch-Dest`. Fails when a name or value is invalid, a header repeats, or a
`priority` header has no `Sec-Fetch-Dest` to belong to.

### `HeaderSpec::validate`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn validate(&self) -> Result<(), HeaderSpecError>
```

Checks a spec written by hand or read from a file, and returns the first problem
found: an invalid or duplicate name, a pseudo-header, a reserved header (the
transport and credential headers, and `priority`), a bad value, or a list or
table that is too long. A name must be an HTTP token of at most `MAX_NAME_LEN`
bytes; a value has no control character except a tab, no space or tab at either
end, and at most `MAX_VALUE_LEN` bytes.

### `HeaderSpec::headers`, `render`, `priority_for`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn headers(&self, shape: RequestShape) -> &[(String, HeaderValue)]
pub fn render(
    &self,
    shape: RequestShape,
    identity: &HeaderIdentity,
) -> Result<Vec<(String, String)>, HeaderSpecError>
pub fn priority_for(&self, dest: &str) -> Option<&str>
```

`headers` is the list of a shape. `render` gives the headers to send, in order,
with the placeholders filled from `identity`; an override in the identity wins,
and a placeholder the identity has no value for leaves its header out.
`InvalidValue` when a resolved value is not a valid header value. `priority_for`
is the `priority` value for requests whose `Sec-Fetch-Dest` is `dest`, matched
without regard to case.

### `HeaderSpec::with_header`, `without_header`, `with_priority`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn with_header(
    &self,
    shape: RequestShape,
    name: &str,
    value: HeaderValue,
) -> Result<Self, HeaderSpecError>
pub fn without_header(&self, shape: RequestShape, name: &str) -> Self
pub fn with_priority(&self, dest: &str, value: &str) -> Result<Self, HeaderSpecError>
```

Copies with one change. `with_header` sets the header `name` for `shape`:
replaced in place (keeping its capitalisation and position) when the list has
it, appended when it does not. `without_header` removes it (any capitalisation).
`with_priority` sets the `priority` value for `dest`. The two that can fail check
the result.

## `HeaderValue`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HeaderValue {
    Literal(String),
    UserAgent,
    SecChUa,
    SecChUaMobile,
    SecChUaPlatform,
    AcceptLanguage,
    Cookie,
}
```

The value of one header in a `HeaderSpec`. In JSON a literal is
`{"literal": "1"}` and a placeholder is its snake-case name, such as
`"user_agent"` or `"sec_ch_ua"`.

<ResponseField name="HeaderValue" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Literal(String)">

Exactly this text.

</ResponseField>

<ResponseField name="UserAgent">

The identity's `User-Agent`.

</ResponseField>

<ResponseField name="SecChUa">

The identity's `Sec-CH-UA`; the header is left out when the identity has none.

</ResponseField>

<ResponseField name="SecChUaMobile">

The identity's `Sec-CH-UA-Mobile`; left out when the identity has none.

</ResponseField>

<ResponseField name="SecChUaPlatform">

The identity's `Sec-CH-UA-Platform`; left out when the identity has none.

</ResponseField>

<ResponseField name="AcceptLanguage">

The identity's `Accept-Language`; left out when the identity has none.

</ResponseField>

<ResponseField name="Cookie">

The identity's cookie; left out when the identity has none.

</ResponseField>

</Expandable>

</ResponseField>

## `HeaderIdentity`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Clone, PartialEq, Eq)]
pub struct HeaderIdentity {
    pub user_agent: String,
    pub accept_language: Option<String>,
    pub sec_ch_ua: Option<String>,
    pub sec_ch_ua_mobile: Option<String>,
    pub sec_ch_ua_platform: Option<String>,
    pub cookie: Option<String>,
    pub overrides: Vec<(String, String)>,
}

impl HeaderIdentity {
    pub fn new(user_agent: impl Into<String>) -> Self;
    pub fn with_accept_language(mut self, value: impl Into<String>) -> Self;
    pub fn with_client_hints(
        mut self,
        brands: impl Into<String>,
        mobile: impl Into<String>,
        platform: impl Into<String>,
    ) -> Self;
    pub fn with_cookie(mut self, value: impl Into<String>) -> Self;
    pub fn with_override(mut self, name: impl Into<String>, value: impl Into<String>) -> Self;
    pub fn from_captured<N: AsRef<str>, V: AsRef<str>>(headers: &[(N, V)]) -> Option<Self>;
}
```

The caller's values for the placeholders of a `HeaderSpec`. The cookie is a
secret, so `Debug` shows it and the override values only as `<redacted>`.

<ResponseField name="user_agent" type="String">

The `User-Agent` value.

</ResponseField>

<ResponseField name="accept_language, sec_ch_ua, sec_ch_ua_mobile, sec_ch_ua_platform, cookie" type="Option<String>">

The values of `Accept-Language`, `Sec-CH-UA`, `Sec-CH-UA-Mobile`,
`Sec-CH-UA-Platform` and `Cookie`, when the request has one.

</ResponseField>

<ResponseField name="overrides" type="Vec<(String, String)>">

Header values that replace whatever the spec says, by case-insensitive name; the
last one given wins.

</ResponseField>

`new` makes an identity with a user agent and nothing else. `with_client_hints`
sets the three client-hint headers a browser sends on every request.
`from_captured` reads the identity a captured header list shows, or `None` when it
has no `User-Agent`.

## `RequestShape`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RequestShape {
    Document,
    Fetch,
}
```

What a request is, for the purpose of choosing its headers: a top-level
navigation, or a request made by a script (`fetch`, `XMLHttpRequest`).

## `HeaderSpecError`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HeaderSpecError {
    InvalidName(String),
    InvalidValue { name: String },
    PseudoHeader(String),
    ReservedHeader(String),
    DuplicateName(String),
    DuplicateDestination(String),
    TooManyHeaders,
    TooManyDestinations,
    PriorityWithoutDestination,
}
```

Why a header spec or a header value was refused: a name that is empty, too long
or not an HTTP token; a value with control characters, edge whitespace, or too
long (the error names the header, never the value); a pseudo-header, which belongs
to the HTTP/2 spec; a connection-level or credential header, or `priority`; the
same header twice in one list; the same `Sec-Fetch-Dest` twice in the priority
table; a list longer than `MAX_HEADERS`; a table longer than
`MAX_PRIORITY_DESTINATIONS`; or a captured `priority` header in a list that has no
`Sec-Fetch-Dest` to key it by.

## Constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const MAX_HEADERS: usize = 128;
pub const MAX_NAME_LEN: usize = 256;
pub const MAX_VALUE_LEN: usize = 8192;
pub const MAX_PRIORITY_DESTINATIONS: usize = 32;
pub const TRANSPORT_HEADERS: [&str; 9] = [
    "host",
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authorization",
    "transfer-encoding",
    "upgrade",
    "content-length",
    "te",
];
pub const CREDENTIAL_HEADERS: [&str; 1] = ["authorization"];
```

The most headers in one list; the longest name and value in bytes; the most
`Sec-Fetch-Dest` values with a `priority` of their own; the headers that belong to
the connection (the transport sets them); and the headers that carry credentials.
A profile never holds a credential header: a capture may have seen one, and the
caller adds it to the requests that need it.

## Related

<Columns cols={2}>

<Card color="#1a85d4" title="Emulation" icon="database" href="/stealth/api/codec-emulation">

The measured `headers` of a browser.

</Card>

</Columns>
