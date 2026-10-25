---
title: "codec::tls_spec"
sidebarTitle: "TlsSpec"
description: "The TLS profile in seleniumbase-rs-codec: TlsSpec, Alps, KnownGap and SpecError, the reproducibility check against what BoringSSL can emit, and the custom JA3, JA4 and peetprint overlays."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_stealth::codec::tls_spec` holds the TLS specification a client should
reproduce. A `TlsSpec` is a whole TLS profile as plain data: the ids a
`ClientHello` carries and the content of the extensions that have any.
`TlsSpec::from_client_hello` records it from a parsed hello. A caller can layer a
custom JA3, JA4 (as `JA4_r` or `JA4_ro`) or peetprint over a base spec; each
overlay replaces what its fingerprint carries, takes the rest from the base, runs
`check_reproducible`, and recomputes its own fingerprint to compare with the
input, so an accepted input reproduces its own fingerprint and a spec BoringSSL
cannot emit is refused before any client exists.

The lists never hold GREASE: the `grease` flag says BoringSSL adds its GREASE
values. The per-extension flags of the design plan (`ocsp_stapling`, `sct`,
`session_ticket`, `renegotiation`, `ech_grease`) are methods that read the
extension list, so they cannot disagree with it. The table of what is refused is
in [Wire formats](/stealth/wire-formats#what-is-refused).

<RequestExample>

```rust Fingerprints of the measured spec
use seleniumbase_rs_stealth::stealth::fingerprint::OsType;
use seleniumbase_rs_stealth::codec::emulation;
use seleniumbase_rs_stealth::codec::BrowserKind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chrome = emulation::get(BrowserKind::Chrome, OsType::Macos)?;
    let measured = &chrome.tls;
    println!("{}", measured.ja4());
    // What a BoringSSL client can send of it.
    let sendable = measured.without_known_gaps(&chrome.provenance.known_gaps);
    sendable.check_reproducible()?;
    assert!(measured.check_reproducible().is_err());
    Ok(())
}
```

```rust Overlay a custom JA3
use seleniumbase_rs_stealth::codec::ja3::Ja3;
use seleniumbase_rs_stealth::codec::tls_spec::{SpecError, TlsSpec};

fn with_custom_ja3(base: &TlsSpec, text: &str) -> Result<TlsSpec, Box<dyn std::error::Error>> {
    let ja3 = Ja3::parse(text)?;
    let spec = base.with_ja3(&ja3)?;
    assert!(!spec.permute_extensions); // a JA3 pins the extension order
    Ok(spec)
}

fn refused(base: &TlsSpec) {
    let result = base.with_signature_algorithms(&[0x0904]);
    assert!(matches!(result, Err(SpecError::UnsupportedSigalg { id: 0x0904, .. })));
}
```

</RequestExample>

<ResponseExample>

```text Fingerprints of the measured spec
t13d1517h2_8daaf6152771_cb7bf5808d99
```

</ResponseExample>

The measured spec carries the `trust_anchors` extension and the ML-DSA signature
schemes, which BoringSSL cannot send, so its own `check_reproducible` fails; the
spec without its known gaps passes.

## `TlsSpec`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsSpec {
    pub legacy_version: u16,
    pub cipher_suites: Vec<u16>,
    pub extensions: Vec<u16>,
    #[serde(default)]
    pub permute_extensions: bool,
    #[serde(default)]
    pub grease: bool,
    #[serde(default)]
    pub supported_groups: Vec<u16>,
    #[serde(default)]
    pub key_share_groups: Vec<u16>,
    #[serde(default)]
    pub point_formats: Vec<u8>,
    #[serde(default)]
    pub signature_algorithms: Vec<u16>,
    #[serde(default)]
    pub alpn: Vec<String>,
    #[serde(default)]
    pub alps: Option<Alps>,
    #[serde(default)]
    pub delegated_credentials: Vec<u16>,
    #[serde(default)]
    pub supported_versions: Vec<u16>,
    #[serde(default)]
    pub psk_modes: Vec<u8>,
    #[serde(default)]
    pub cert_compression: Vec<u16>,
    #[serde(default)]
    pub record_size_limit: Option<u16>,
}
```

A whole TLS profile: what a `ClientHello` must carry for a client to look like
this one.

<ResponseField name="legacy_version" type="u16">

The handshake `client_version` (771).

</ResponseField>

<ResponseField name="cipher_suites" type="Vec<u16>">

Cipher suites in wire order, without GREASE.

</ResponseField>

<ResponseField name="extensions" type="Vec<u16>">

Extension ids in wire order, without GREASE; SNI and ALPN are listed when sent.

</ResponseField>

<ResponseField name="permute_extensions" type="bool" default="false">

Whether the client shuffles the extension order on every connection, as Chrome
does (the measured Chrome 155 spec has `true`).

</ResponseField>

<ResponseField name="grease" type="bool" default="false">

Whether GREASE values are added: a cipher, a group, a version and two extensions.

</ResponseField>

<ResponseField name="supported_groups" type="Vec<u16>">

Supported groups in preference order.

</ResponseField>

<ResponseField name="key_share_groups" type="Vec<u16>">

Groups a key share is sent for, in order.

</ResponseField>

<ResponseField name="point_formats" type="Vec<u8>">

EC point formats: `[0]` when the extension is sent.

</ResponseField>

<ResponseField name="signature_algorithms" type="Vec<u16>">

Signature algorithms in preference order.

</ResponseField>

<ResponseField name="alpn" type="Vec<String>">

ALPN protocol names in preference order.

</ResponseField>

<ResponseField name="alps" type="Option<Alps>">

The `application_settings` extension, when sent.

</ResponseField>

<ResponseField name="delegated_credentials" type="Vec<u16>">

`delegated_credential` signature schemes.

</ResponseField>

<ResponseField name="supported_versions" type="Vec<u16>">

`supported_versions`, highest first, without GREASE.

</ResponseField>

<ResponseField name="psk_modes" type="Vec<u8>">

`psk_key_exchange_modes`.

</ResponseField>

<ResponseField name="cert_compression" type="Vec<u16>">

`compress_certificate` algorithms.

</ResponseField>

<ResponseField name="record_size_limit" type="Option<u16>">

`record_size_limit`.

</ResponseField>

### `TlsSpec::from_client_hello`, `with_observed_order`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn from_client_hello(hello: &ClientHello) -> Self
pub fn with_observed_order(&self, other: &ClientHello) -> Result<Self, SpecError>
```

`from_client_hello` records the profile a parsed hello shows; GREASE becomes the
`grease` flag, and the extension order is taken as observed
(`permute_extensions` is `false`). `with_observed_order` compares a second hello
of the same client: a changed extension order sets `permute_extensions`. It
returns `SpecError::HellosDiffer` when the cipher suites or the set of extensions
differ.

### Extension flags

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn has_extension(&self, id: u16) -> bool
pub fn ocsp_stapling(&self) -> bool
pub fn sct(&self) -> bool
pub fn session_ticket(&self) -> bool
pub fn renegotiation(&self) -> bool
pub fn ech_grease(&self) -> bool
pub fn extended_master_secret(&self) -> bool
```

Whether the spec sends the extension with this id, `status_request` (OCSP
stapling), `signed_certificate_timestamp`, `session_ticket`,
`renegotiation_info`, `encrypted_client_hello` (as GREASE ECH) or
`extended_master_secret`.

### Fingerprints of a spec

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn ja3(&self) -> Ja3
pub fn ja4_facts(&self) -> Ja4Facts
pub fn ja4(&self) -> Ja4
pub fn ja4_raw(&self, order: Ja4Order) -> Ja4Raw
pub fn peetprint(&self) -> Peetprint
```

The JA3 this spec produces (extensions in the listed order); what JA4 reads from
it; its `JA4`; the unhashed JA4 in the given order; and its peetprint, which with
`grease` carries the GREASE tokens BoringSSL adds (one each in versions, groups
and ciphers, two in the extensions). See [JA3](/stealth/api/codec-ja3) and
[JA4](/stealth/api/codec-ja4).

### `TlsSpec::without_known_gaps`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn without_known_gaps(&self, gaps: &[KnownGap]) -> Self
```

Removes the listed known gaps: `TrustAnchorsExtension` drops extension `0xCA34`;
`MlDsaSignatureAlgorithms` drops `0x0904` to `0x0906` from the signature
algorithms and the delegated credentials.

### `TlsSpec::check_reproducible`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn check_reproducible(&self) -> Result<(), SpecError>
```

Checks the spec against what BoringSSL can emit (plan section 3.8) and returns
the first `SpecError` found, checking in this order: the version, the cipher
suites, the extensions, the groups, the point formats, the signature algorithms,
then extension content.

<Expandable title="what is checked">

- `legacy_version` must be 771; `supported_versions` may hold TLS 1.0 to 1.3,
  each once.
- The cipher list is not empty, every suite is in `ids::BORINGSSL_CIPHERS`, each
  once, and the three TLS 1.3 suites are present together exactly when TLS 1.3 is
  offered.
- Every extension is one BoringSSL has an option for and is not refused
  (`ids::REFUSED_EXTENSIONS`, GREASE ids), each once.
- Every group is in `ids::BORINGSSL_GROUPS`, each once; a key share needs its
  group offered.
- `point_formats` is `[0]` with `ec_point_formats` and empty without it.
- Every signature algorithm and delegated credential is in
  `ids::BORINGSSL_SIGALGS` and is not ML-DSA.
- An extension that carries data and the field that holds it agree, and every
  ALPN name is 1 to 255 bytes.

</Expandable>

### Overlays

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn with_ja3(&self, ja3: &Ja3) -> Result<Self, SpecError>
pub fn with_signature_algorithms(&self, ids: &[u16]) -> Result<Self, SpecError>
pub fn with_ja4_raw(&self, raw: &Ja4Raw, order: Ja4Order) -> Result<Self, SpecError>
pub fn with_peetprint(&self, peet: &Peetprint) -> Result<Self, SpecError>
```

<ResponseField name="with_ja3" type="method">

Replaces the version, ciphers, extensions, groups and point formats with a custom
JA3. Everything JA3 does not carry (signature algorithms, ALPN, key shares,
versions and the other extension bodies) comes from `self`; content of extensions
the JA3 omits is dropped. Extension permutation is turned off, so the JA3 on the
wire is exact.

</ResponseField>

<ResponseField name="with_signature_algorithms" type="method">

Replaces the signature algorithms. `UnsupportedSigalg` for a scheme BoringSSL
cannot send, `ExtensionContent` when the list is empty or the extension is not
sent.

</ResponseField>

<ResponseField name="with_ja4_raw" type="method">

Replaces the ciphers, extensions and signature algorithms with a `JA4_r`
(`Ja4Order::Sorted`) or `JA4_ro` (`Ja4Order::Original`). The sorted form carries
no order: the ids keep the order they have in `self`, new ones follow in
ascending order, and `permute_extensions` is kept. The original-order form pins
ciphers and extensions and turns permutation off. The TLS version and the first
ALPN name come from the `a` section; groups and the other bodies come from
`self`. `Ja4Mismatch` when the result would not reproduce `raw`.

</ResponseField>

<ResponseField name="with_peetprint" type="method">

Replaces the ciphers, extensions, groups, versions, ALPN, signature algorithms,
PSK modes and certificate compression with a custom peetprint. The extension field
is sorted, so extension order comes from `self`. GREASE tokens set the `grease`
flag, and a layout BoringSSL would not produce is refused. `PeetprintMismatch`
when the result would not reproduce `peet`.

</ResponseField>

## `Alps`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alps {
    pub protocols: Vec<String>,
    pub new_codepoint: bool,
}
```

The `application_settings` extension: the protocols it announces (such as `h2`)
and whether it uses the new codepoint (`0x44CD`) rather than the old one
(`0x4469`).

## `KnownGap`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KnownGap {
    TrustAnchorsExtension,
    MlDsaSignatureAlgorithms,
}
```

A known difference between a measured browser hello and what BoringSSL can send.
The measured file records them in `provenance.known_gaps`.

<ResponseField name="KnownGap" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="TrustAnchorsExtension">

The `trust_anchors` extension (`0xCA34`), which needs a newer btls and wreq.

</ResponseField>

<ResponseField name="MlDsaSignatureAlgorithms">

The ML-DSA signature schemes (`0x0904` to `0x0906`).

</ResponseField>

</Expandable>

</ResponseField>

## `SpecError`

<Badge color="purple" shape="pill">enum</Badge>

```rust
// Re-exported at the crate root as TlsSpecError.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SpecError {
    LegacyVersion(u16),
    UnsupportedVersion(u16),
    NoCipherSuites,
    UnsupportedCipher(u16),
    Tls13Suites,
    UnsupportedExtension { id: u16, reason: &'static str },
    DuplicateExtension(u16),
    UnsupportedGroup(u16),
    KeyShareNotOffered(u16),
    PointFormats(Vec<u8>),
    UnsupportedSigalg { id: u16, name: &'static str },
    Duplicate { list: &'static str, id: u16 },
    ExtensionContent(u16),
    Alpn(String),
    Ja4Mismatch(Ja4Section),
    Ja4Alpn(String),
    PeetprintMismatch(PeetField),
    HellosDiffer,
}
```

Why a spec cannot be reproduced, or an overlay cannot be applied.

<ResponseField name="SpecError" type="enum">

<Expandable title="variants">

<ResponseField name="LegacyVersion(u16)">

The handshake version is not TLS 1.2 (771), the only value BoringSSL writes.

</ResponseField>

<ResponseField name="UnsupportedVersion(u16)">

`supported_versions` lists a version BoringSSL does not offer.

</ResponseField>

<ResponseField name="NoCipherSuites">

The cipher suite list is empty.

</ResponseField>

<ResponseField name="UnsupportedCipher(u16)">

BoringSSL cannot send this cipher suite (or the spec refuses it by policy, as for
the DHE suites).

</ResponseField>

<ResponseField name="Tls13Suites">

The TLS 1.3 suites are not all present, or do not match `supported_versions`.

</ResponseField>

<ResponseField name="UnsupportedExtension { id, reason }">

BoringSSL cannot send this extension; `reason` says why, such as
`needs btls newer than 0.5.6 and wreq newer than rc.31` for `0xCA34`.

</ResponseField>

<ResponseField name="DuplicateExtension(u16)">

An extension is listed twice.

</ResponseField>

<ResponseField name="UnsupportedGroup(u16)">

BoringSSL cannot offer this supported group.

</ResponseField>

<ResponseField name="KeyShareNotOffered(u16)">

A key share is sent for a group that `supported_groups` does not offer.

</ResponseField>

<ResponseField name="PointFormats(Vec<u8>)">

The point formats are not `[0]` with the extension, or not empty without it.

</ResponseField>

<ResponseField name="UnsupportedSigalg { id, name }">

BoringSSL cannot send this signature scheme; `name` is its IANA name, or `unassigned`.

</ResponseField>

<ResponseField name="Duplicate { list, id }">

An id occurs twice in a list where each may occur once.

</ResponseField>

<ResponseField name="ExtensionContent(u16)">

An extension is listed without its content, or the content has no extension.

</ResponseField>

<ResponseField name="Alpn(String)">

An ALPN protocol name is empty or longer than 255 bytes.

</ResponseField>

<ResponseField name="Ja4Mismatch(Ja4Section)">

The recomputed `JA4_r` differs from the input in this section.

</ResponseField>

<ResponseField name="Ja4Alpn(String)">

No known ALPN name has this two-character `JA4` ALPN code.

</ResponseField>

<ResponseField name="PeetprintMismatch(PeetField)">

The recomputed peetprint differs from the input in this field.

</ResponseField>

<ResponseField name="HellosDiffer">

Two hellos meant to show one client differ in more than extension order.

</ResponseField>

</Expandable>

</ResponseField>

## Related

<Columns cols={2}>

</Columns>
