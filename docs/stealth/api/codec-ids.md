---
title: "codec::ids"
sidebarTitle: "Registries"
description: "IANA names of the TLS and HTTP/2 ids in seleniumbase-rs-codec (Registry and its tables), and the lists of what BoringSSL can emit and what the spec refuses."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_codec::ids` names the TLS and HTTP/2 values a browser
fingerprint is built from, and seeds what BoringSSL can emit in a client hello.
Each registry is a `Registry`: a static table of `(id, IANA name)` rows, looked up
in both directions. The `BORINGSSL_*` slices list the ids BoringSSL can emit; the
`*_CIPHERS`, `REFUSED_EXTENSIONS`, `UNSUPPORTED_GROUPS` and `ML_DSA_SIGALGS`
constants list the ids the spec refuses before a client exists. The numbers come
from the vendored BoringSSL headers of btls-sys 0.5.6, cross-checked against
rustls 0.23.45 and http2 0.5.20; the wire probe of the mimic crate decides what
the wire really carries.

<RequestExample>

```rust Look up names
use seleniumbase_rs_codec::ids::{is_grease, CIPHER_SUITES, EXTENSIONS, GROUPS};

fn main() {
    println!("{:?}", CIPHER_SUITES.name(0x1301));
    println!("{:?}", EXTENSIONS.id("trust_anchors"));
    println!("{:?}", GROUPS.name(0x11EC));
    println!("{}", is_grease(0x0A0A));
}
```

</RequestExample>

<ResponseExample>

```text Look up names
Some("TLS_AES_128_GCM_SHA256")
Some(51764)
Some("X25519MLKEM768")
true
```

</ResponseExample>

## `Registry`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub type Row = (u16, &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registry { /* private fields */ }

impl Registry {
    pub const fn new(rows: &'static [Row]) -> Self;
    pub const fn rows(&self) -> &'static [Row];
    pub fn name(&self, id: u16) -> Option<&'static str>;
    pub fn id(&self, name: &str) -> Option<u16>;
    pub fn contains(&self, id: u16) -> bool;
}
```

A registry of wire ids, each with its IANA name. `new` wraps a static table in
which each id and each name appears once (a test checks it); `rows` is the table
in order; `name` and `id` look up in either direction (`id` matches the name
exactly); `contains` says whether the id is listed.

## Registries

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const CIPHER_SUITES: Registry;          // TLS cipher suites
pub const EXTENSIONS: Registry;             // TLS extensions
pub const GROUPS: Registry;                 // supported groups
pub const SIGNATURE_ALGORITHMS: Registry;   // signature schemes
pub const EC_POINT_FORMATS: Registry;       // EC point formats (RFC 8422)
pub const TLS_VERSIONS: Registry;           // SSL 3.0 to TLS 1.3
pub const HTTP2_SETTINGS: Registry;         // HTTP/2 SETTINGS ids
```

Each holds the rows the module names, sorted by id: for example
`0x1301 TLS_AES_128_GCM_SHA256`, `0xCA34 trust_anchors`, `0x11EC X25519MLKEM768`,
`0x0904 ml_dsa_44`, `0x0304 TLS 1.3`, `0x0004 INITIAL_WINDOW_SIZE`.

## `is_grease`, `is_alpn`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub const fn is_grease(id: u16) -> bool
pub fn is_alpn(name: &str) -> bool
```

Whether `id` is a GREASE value (RFC 8701: both bytes equal, low nibble `0xA`), and
whether `name` is one of `ALPN_PROTOCOLS`.

## Constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const TLS12_LEGACY_VERSION: u16 = 0x0303;
pub const EC_POINT_UNCOMPRESSED: u8 = 0x00;
pub const ALPN_H2: &str = "h2";
pub const ALPN_HTTP_1_1: &str = "http/1.1";
pub const ALPN_PROTOCOLS: &[&str] = &[ALPN_H2, ALPN_HTTP_1_1];
pub const H2_SETTINGS_ACCEPTED: &[u16] = &[
    0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0008, 0x0009,
];
```

The TLS 1.2 `legacy_version` a JA3 version field carries (771); the uncompressed
EC point format, the only one BoringSSL emits; the ALPN ids this module names; and
the HTTP/2 SETTINGS ids a measured spec may carry (any other is refused).

## What BoringSSL can emit

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const BORINGSSL_CIPHERS: &[u16] = &[
    0x1301, 0x1302, 0x1303, 0xC02B, 0xC02F, 0xC02C, 0xC030, 0xCCA9, 0xCCA8, 0xC009, 0xC013, 0xC00A,
    0xC014, 0x009C, 0x009D, 0x002F, 0x0035, 0x000A, 0x003C, 0x003D, 0xC008, 0xC012, 0xC023, 0xC024,
    0xC027, 0xC028,
];
pub const TLS13_CIPHERS: &[u16] = &[0x1301, 0x1302, 0x1303];
pub const BORINGSSL_CLIENT_EXTENSIONS: &[u16] = &[
    0x0000, 0x0005, 0x000A, 0x000B, 0x000D, 0x0010, 0x0012, 0x0015, 0x0017, 0x001B, 0x001C, 0x0022,
    0x0023, 0x002B, 0x002D, 0x0033, 0x4469, 0x44CD, 0xFE0D, 0xFF01,
];
pub const BORINGSSL_ALWAYS_SENT_EXTENSIONS: &[u16] = &[0x000A, 0x000D];
pub const BORINGSSL_LEGACY_ONLY_EXTENSIONS: &[u16] = &[0x000B, 0x0017, 0xFF01];
pub const BORINGSSL_TLS13_ONLY_EXTENSIONS: &[u16] = &[0x002B, 0x002D, 0x0033];
pub const BORINGSSL_GROUPS: &[u16] = &[
    0x001D, 0x0017, 0x0018, 0x0019, 0x0202, 0x0100, 0x0101, 0x11EC, 0x6399, 0xFE32,
];
pub const BORINGSSL_KEY_SHARE_GROUPS: &[u16] = &[
    0x001D, 0x0017, 0x0018, 0x0019, 0x0202, 0x11EC, 0x6399, 0xFE32,
];
pub const BORINGSSL_SIGALGS: &[u16] = &[
    0x0201, 0x0203, 0x0401, 0x0403, 0x0420, 0x0501, 0x0503, 0x0601, 0x0603, 0x0804, 0x0805, 0x0806,
    0x0807,
];
```

- `BORINGSSL_CIPHERS`: the suites the spec layer lets through, each checked by the
  wire probe of `tests/impersonate_wire.rs`. The TLS 1.2 DHE suites reach the wire
  too but are kept out by policy; PSK, CCM, RC4, NULL, export, anonymous and
  signalling suites do not reach the wire.
- `TLS13_CIPHERS`: a spec may reorder them but never drop one.
- `BORINGSSL_CLIENT_EXTENSIONS`: one per `TlsOptions` knob; padding (21) only
  where BoringSSL's length rule places it.
- `BORINGSSL_ALWAYS_SENT_EXTENSIONS`: `supported_groups` and
  `signature_algorithms` are in every hello; a spec without them describes a hello
  no client can send.
- `BORINGSSL_LEGACY_ONLY_EXTENSIONS`: sent exactly when the lowest offered version
  is below TLS 1.3. `BORINGSSL_TLS13_ONLY_EXTENSIONS`: sent exactly when TLS 1.3 is
  offered.
- `BORINGSSL_GROUPS` and `BORINGSSL_KEY_SHARE_GROUPS`: the FFDHE groups can be
  offered but have no key share.
- `BORINGSSL_SIGALGS`: the internal MD5-SHA1 code `0xFF01` never reaches the wire
  and is not listed.

## What the spec refuses

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const DHE_CIPHERS: &[u16] = &[0x0033, 0x0039, 0x009E, 0x009F];
pub const CCM_CIPHERS: &[u16] = &[0x1304, 0x1305];
pub const SCSV_CIPHERS: &[u16] = &[0x00FF, 0x5600];
pub const REFUSED_EXTENSIONS: &[(u16, &str)] = &[
    (0x0029, "only usable with a resumption ticket"),
    (0x002A, "only usable with a resumption ticket"),
    (0x0031, "post-handshake authentication is not part of the client hello"),
    (0x0032, "BoringSSL does not emit it"),
    (0x0016, "BoringSSL does not emit it"),
    (0x000F, "BoringSSL does not emit it"),
    (0xCA34, "needs btls newer than 0.5.6 and wreq newer than rc.31"),
];
pub const UNSUPPORTED_GROUPS: &[u16] = &[
    0x0016, 0x001A, 0x001B, 0x001C, 0x001E, 0x0102, 0x0103, 0x0104, 0x0200, 0x0201, 0x11EB,
];
pub const ML_DSA_SIGALGS: &[u16] = &[0x0904, 0x0905, 0x0906];
```

The TLS 1.2 DHE suites (a policy, not a limit: the probe shows BoringSSL writes
them), the TLS 1.3 CCM suites, the signalling suites (BoringSSL replaces them with
`0xFF01`), the refused extensions with their reasons, the groups BoringSSL does not
name (x448, brainpool, secp256k1, the larger FFDHE groups, ML-KEM 512 and 768 and
the P-256 hybrid), and the ML-DSA signature schemes.

## Related

<Columns cols={2}>

<Card color="#693acf" title="TlsSpec" icon="lock" href="/stealth/api/codec-tls-spec">

`check_reproducible` reads these lists.

</Card>

<Card color="#1a85d4" title="What is refused" icon="ban" href="/stealth/wire-formats#what-is-refused">

The refusals in the guide.

</Card>

</Columns>
