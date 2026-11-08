---
title: "wire::client"
sidebarTitle: "Client"
description: "WireFormat, WireFormatBuilder and WireFormat::client_builder."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: wire</Badge>

`client` turns a measured browser into a wreq client. `WireFormat` is the measurement as data, including the known gaps BoringSSL cannot send. `WireFormat::client_builder` builds a `ClientBuilder` from a profile. `WireFormatBuilder` lays a JA3 or an Akamai string over the measured profile. A browser or system with no measurement is refused.

See [Wire formats](/stealth/advanced/wire-formats) and [Limits](/scraping/advanced-stealthy/limits). Only Chrome 155.0.8059.40 on macOS is measured.

<RequestExample>

```rust The measured Chrome on this operating system
use seleniumbase_rs_stealth::codec::BrowserKind;
use seleniumbase_rs_stealth::evasion::fingerprint::OsType;
use seleniumbase_rs_stealth::wire::{WireFormatError, WireFormat};

fn profile() -> Result<WireFormat, WireFormatError> {
    WireFormat::for_system(BrowserKind::Chrome, OsType::Macos)
}
```

</RequestExample>

## `client`

### `WireFormatError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum WireFormatError {
    Ja3(#[from] Ja3Error),
    Ja4(#[from] Ja4Error),
    Peetprint(#[from] PeetprintError),
    Akamai(#[from] AkamaiError),
    Tls(#[from] TlsSpecError),
    Http2(#[from] H2SpecError),
    WireMeasurement(#[from] WireMeasurementError),
    Headers(#[from] HeaderSpecError),
    Convert(#[from] ConvertError),
    VersionMismatch {
            /// The major version the user agent claims.
            browser: u32,
            /// The major version that was measured.
            measured: u32,
        },
    Incomplete {
            /// What is missing.
            what: &'static str,
        },
    Fingerprint(&'static str),
    UnknownHeader(String),
    Build(String),
}
```

Why a profile or a client could not be built.

### `WireProtocol`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum WireProtocol {
    Http1,
    Http2,
}
```

The protocol a request is expected to travel on.

### `WireFormat`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct WireFormat
```

A measured browser as data: what to send on the wire and in the headers to look like it.

### `WireFormat` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn measured(kind: BrowserKind) -> Result<Self, WireFormatError>
pub fn for_system(kind: BrowserKind, os: OsType) -> Result<Self, WireFormatError>
pub fn from_measured(entry: &Measured) -> Result<Self, WireFormatError>
pub fn random(seed: u64) -> Result<Self, WireFormatError>
pub fn from_fingerprint(fingerprint: &Fingerprint) -> Result<Self, WireFormatError>
pub fn builder() -> WireFormatBuilder
pub fn validate(&self) -> Result<(), WireFormatError>
pub fn source(&self) -> &str
pub fn kind(&self) -> BrowserKind
pub fn os(&self) -> OsType
pub fn major(&self) -> u32
pub fn tls_spec(&self) -> &TlsSpec
pub fn h2_spec(&self) -> &H2Spec
pub fn header_spec(&self) -> &HeaderSpec
pub fn client_headers(&self) -> &ClientHeaders
pub fn known_gaps(&self) -> &[KnownGap]
pub fn dropped_settings(&self) -> &[(u16, u32)]
pub fn headers_for(
        &self,
        shape: RequestShape,
        cookie: Option<&str>,
        wire: WireProtocol,
    ) -> Result<Vec<(String, String)>, WireFormatError>
pub fn apply(
        &self,
        request: RequestBuilder,
        shape: RequestShape,
        cookie: Option<&str>,
        wire: WireProtocol,
    ) -> Result<RequestBuilder, WireFormatError>
pub fn client_builder(&self) -> Result<wreq::ClientBuilder, WireFormatError>
```


- `measured`: The newest measured entry of a browser.
- `for_system`: The newest measured entry of a browser on one operating system.
- `from_measured`: A profile from one measured entry.
- `random`: A profile for a seeded random identity on a measured system.
- `from_fingerprint`: A profile for the identity of a fingerprint (`ClientHeaders::from_fingerprint`: its user agent, languages and client hints, the hints computed from its Chrome major unless it sets its own), riding the measured TLS and HTTP/2 of its operating system. To change the language, localize the fingerprint first (`LocaleExt::localize`).
- `builder`: A builder that layers custom inputs over a measured base.
- `validate`: Checks that the specs can be sent and the headers are valid.
- `source`: Where the profile comes from, such as `chrome-155-macos`.
- `kind`: The browser.
- `os`: The operating system the browser was measured on.
- `major`: The major version that was measured.
- `tls_spec`: The TLS spec the client sends.
- `h2_spec`: The HTTP/2 spec the client sends.
- `header_spec`: The header lists.
- `client_headers`: The values of the identity placeholders: the `User-Agent` (`.user_agent`), the `Accept-Language` (`.accept_language`) and the client hints a request carries (a cookie is never part of it).
- `known_gaps`: What the measured hello has and `BoringSSL` cannot send: the difference between this client's hello and the browser's, which the wire test asserts and nothing else.
- `dropped_settings`: The SETTINGS of the measured HTTP/2 start that wreq cannot send and the profile dropped.
- `headers_for`: The headers of one request, in wire order, with the placeholders filled.
- `apply`: Sets the headers of one request on a wreq request: the rendered list in its order and spelling, in place of the client's default headers.
- `client_builder`: Starts a wreq client with the TLS, HTTP/2 and default headers of this profile.

## `client::custom`

### `WireFormatBuilder`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct WireFormatBuilder
```

Builds an [`WireFormat`] from a measured base and caller-supplied inputs.

### `WireFormatBuilder` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new() -> Self
pub fn base(mut self, kind: BrowserKind) -> Self
pub fn base_on(mut self, kind: BrowserKind, os: OsType) -> Self
pub fn ja3(self, ja3: &str) -> Result<Self, WireFormatError>
pub fn ja4_r(self, ja4_r: &str) -> Result<Self, WireFormatError>
pub fn ja4_ro(self, ja4_ro: &str) -> Result<Self, WireFormatError>
pub fn peetprint(self, peetprint: &str) -> Result<Self, WireFormatError>
pub fn signature_algorithms(self, ids: &[u16]) -> Result<Self, WireFormatError>
pub fn grease(self, enabled: bool) -> Result<Self, WireFormatError>
pub fn permute_extensions(self, enabled: bool) -> Result<Self, WireFormatError>
pub fn akamai(mut self, akamai: &str) -> Result<Self, WireFormatError>
pub fn fingerprint(mut self, fingerprint: &Fingerprint) -> Result<Self, WireFormatError>
pub fn client_headers(mut self, headers: ClientHeaders) -> Self
pub fn header(
        mut self,
        shape: RequestShape,
        name: impl Into<String>,
        value: HeaderSource,
    ) -> Self
pub fn without_header(mut self, shape: RequestShape, name: impl Into<String>) -> Self
pub fn header_order<I, S>(mut self, shape: RequestShape, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
pub fn priority(mut self, destination: impl Into<String>, value: impl Into<String>) -> Self
pub fn headers(mut self, spec: HeaderSpec) -> Self
pub fn build(mut self) -> Result<WireFormat, WireFormatError>
```


- `new`: A builder over the newest measured Chrome.
- `base`: Takes the newest measurement of `kind` as the base. Call it first: it discards the TLS and HTTP/2 inputs given before it.
- `base_on`: Takes the measurement of `kind` on `os` as the base; a pair with no capture is refused by the next input that needs the base, or by `build`. Call it first.
- `ja3`: Replaces the version, ciphers, extensions, groups and point formats with a JA3.
- `ja4_r`: Replaces the ciphers, extensions and signature algorithms with a `JA4_r` (sorted ids).
- `ja4_ro`: Replaces the ciphers, extensions and signature algorithms with a `JA4_ro` (wire order).
- `peetprint`: Replaces the ciphers, extensions, groups, versions, ALPN, signature algorithms, PSK modes and certificate compression with a peetprint.
- `signature_algorithms`: Replaces the signature algorithms.
- `grease`: Turns GREASE (RFC 8701) on or off in the hello: the extra cipher, group, version and signature algorithm values and the first and last extension that fingerprints strip.
- `permute_extensions`: Lets `BoringSSL` shuffle the extensions on every connection, as Chrome does, or pins their order. A JA3, `JA4_ro` or similar input pins the order; its JA3 is then exact.
- `akamai`: Replaces the HTTP/2 SETTINGS, connection window, PRIORITY frames and pseudo-header order with an Akamai fingerprint. The HEADERS priority fields stay the base's.
- `fingerprint`: Takes the identity (the `User-Agent`, the `Accept-Language` and the client hints) of a core fingerprint, as `ClientHeaders::from_fingerprint` reads it. Fails with `WireFormatError::Fingerprint` for a fingerprint without a user agent.
- `client_headers`: Takes the identity from `ClientHeaders` captured from a browser (`ClientHeaders::from_captured`) or written by hand, the other way to fill the client hints.
- `header`: Sets a header of one request shape: replaced in place (keeping its position and spelling) when the list has it, appended when it does not.
- `without_header`: Removes a header of one request shape.
- `header_order`: Puts the named headers of one request shape first, in this order; the others follow in their old order. A name that is not in the list is refused by `build`.
- `priority`: Sets the HTTP/2 `priority` header value for requests with this `Sec-Fetch-Dest`.
- `headers`: Replaces both header lists and the priority table.
- `build`: Applies the identity and header inputs, checks every spec, and returns the profile.

## Related

The guide is [Wire formats](/stealth/advanced/wire-formats). The overview is [wire](/stealth/api/wire).
