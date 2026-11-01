---
title: "mimic::convert"
sidebarTitle: "Convert"
description: "Plain functions from the codec specs to wreq and BoringSSL options."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: mimic</Badge>

`convert` maps a codec spec onto what BoringSSL and wreq can emit. `tls_options`, `http2_options` and `wreq_headers` do the mapping. `check_emittable` reports a spec that cannot be sent. `ConvertError` names the gap. This module and `client` are the ones that name wreq types.

See [Wire formats](/stealth/wire-formats).

<RequestExample>

```rust Can this TLS spec be sent?
use seleniumbase_rs_stealth::codec::TlsSpec;
use seleniumbase_rs_stealth::mimic::convert::{check_emittable, ConvertError};

fn can_send(spec: &TlsSpec) -> Result<(), ConvertError> {
    check_emittable(spec)
}
```

</RequestExample>

## `ConvertError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum ConvertError {
    UnsupportedAlpn(String),
    UnsupportedAlps(String),
    CipherName(u16),
    SigalgName(u16),
    GroupName(u16),
    KeyShare(u16),
    Tls13SuitesNotFirst,
    VersionGap(Vec<u16>),
    ExtensionAlwaysSent(u16),
    ExtensionVersion {
            /// The extension.
            id: u16,
            /// The version rule that forbids it.
            reason: &'static str,
        },
    PaddingUnavailable,
    PaddingAppended,
    GroupNeedsTls13(u16),
    CertCompression(u16),
    StreamId(u32),
    Weight(u16),
    Header {
            /// The header name.
            name: String,
            /// Why it was refused.
            reason: &'static str,
        },
}
```

Why a spec could not be turned into wreq options.

## Functions

### `check_emittable`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn check_emittable(spec: &TlsSpec) -> Result<(), ConvertError>
```

Checks the rules of this client stack that the spec's own check cannot know.

### `tls_options`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn tls_options(spec: &TlsSpec) -> Result<TlsOptions, ConvertError>
```

Turns a TLS spec into the options of a wreq client.

### `http2_options`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn http2_options(spec: &H2Spec) -> Result<Http2Options, ConvertError>
```

Turns an HTTP/2 spec into the options of a wreq client.

### `wreq_headers`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn wreq_headers(
    list: &[(String, String)],
    leading: &[&'static str],
) -> Result<(HeaderMap, OrigHeaderMap), ConvertError>
```

===== Headers ===== Turns a rendered header list into the default headers and the order and capitalisation wreq writes them in.

## Related

The guide is [Wire formats](/stealth/wire-formats). The overview is [mimic](/stealth/api/mimic).
