---
title: "codec::ja3"
sidebarTitle: "JA3 and peetprint"
description: "JA3 and the peetprint in seleniumbase-rs-codec: Ja3, Ja3Field, Ja3Error, Peetprint, Slot, PeetField, PeetprintError, and the hex and MD5 helpers."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_stealth::codec::ja3` parses and renders JA3 strings with GREASE removed,
so that a custom JA3 can be checked and replayed. JA3 is the Salesforce TLS client
fingerprint: five comma-separated fields (handshake version, cipher suites,
extensions, supported groups, point formats), each a dash-separated list of
decimal ids, hashed with MD5. The MD5 here is a small pure function because JA3 is
defined with it; nothing in the crate uses it for security. The submodule
`ja3::peetprint` holds the sibling format that also records signature algorithms.

<Tip>

Chrome shuffles its TLS extensions on every connection, so its JA3 changes from
one connection to the next by design. Compare JA4 when you compare a browser with
a client ([JA4](/stealth/api/codec-ja4)).

</Tip>

<RequestExample>

```rust JA3 of a measured hello
use seleniumbase_rs_stealth::codec::ja3::Ja3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The JA3 of one hello Chrome 155 on macOS sent to the capture tool.
    let ja3 = Ja3::parse(
        "771,4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,\
         13-10-35-18-65281-27-45-65037-5-0-23-17613-51-51764-43-11-16,4588-29-23-24,0",
    )?;
    println!("{}", ja3.hash());
    Ok(())
}
```

```rust Peetprint of the same hello
use seleniumbase_rs_stealth::codec::ja3::Peetprint;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let peet = Peetprint::parse(
        "GREASE-772-771|2-1.1|GREASE-4588-29-23-24|\
         GREASE-2308-2309-2310-1027-2052-1025-1283-2053-1281-2054-1537|1|2|\
         GREASE-4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53|\
         0-10-11-13-16-17613-18-23-27-35-43-45-5-51-51764-65037-65281-GREASE-GREASE",
    )?;
    println!("{}", peet.hash());
    Ok(())
}
```

```rust From a parsed hello
use seleniumbase_rs_stealth::codec::client_hello::ClientHello;
use seleniumbase_rs_stealth::codec::ja3::{Ja3, Peetprint};

fn fingerprints(hello: &ClientHello) -> (String, String) {
    (Ja3::of(hello).to_string(), Peetprint::of(hello).to_string())
}
```

</RequestExample>

<ResponseExample>

```text JA3 of a measured hello
1e179c4f9bd5fc86564617794238e4bb
```

```text Peetprint of the same hello
fc97c1cdfb1409c9a9326c1b726d1dee
```

</ResponseExample>

The two strings and their hashes are those the capture tool recorded for the first
hello of `chrome-155-macos.json` (under `capture.tls.samples`).

## `Ja3`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ja3 {
    pub version: u16,
    pub ciphers: Vec<u16>,
    pub extensions: Vec<u16>,
    pub groups: Vec<u16>,
    pub point_formats: Vec<u8>,
}
```

A JA3 fingerprint: the five fields, GREASE removed, in wire order. It implements
`Display` (the JA3 string) and `FromStr` (as `Ja3::parse`).

<ResponseField name="version" type="u16">

The handshake `client_version` (771 for TLS 1.2 and 1.3 clients).

</ResponseField>

<ResponseField name="ciphers" type="Vec<u16>">

Cipher suite ids.

</ResponseField>

<ResponseField name="extensions" type="Vec<u16>">

Extension ids, in the order the client sent them.

</ResponseField>

<ResponseField name="groups" type="Vec<u16>">

Supported group ids.

</ResponseField>

<ResponseField name="point_formats" type="Vec<u8>">

EC point format ids.

</ResponseField>

### `Ja3::of`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn of(hello: &ClientHello) -> Self
```

The JA3 of a parsed hello: GREASE removed everywhere, extensions in wire order.

### `Ja3::parse`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn parse(text: &str) -> Result<Self, Ja3Error>
```

Parses a JA3 string such as `771,4865-4866,0-23,29-23,0`. Surrounding whitespace
is trimmed. GREASE ids in the input are dropped, as the fingerprint is defined
without them. The version and the ciphers must be present; the other lists may be
empty. Numbers are plain ASCII digits: signs and spaces are refused.

<ParamField body="text" type="&str" required>

At most `MAX_JA3_LEN` (16 KiB).

</ParamField>

<ResponseField name="returns" type="Result<Ja3, Ja3Error>">

A `Ja3Error` naming the field and the token when the text is not a JA3 string.

</ResponseField>

### `Ja3::hash`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn hash(&self) -> String
```

The lowercase hex MD5 of the JA3 string, the form JA3 databases list.

## `Ja3Field`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ja3Field {
    Version,
    Ciphers,
    Extensions,
    Groups,
    PointFormats,
}
```

One of the five fields of a JA3 string, named in errors. Its `Display` is
`version`, `ciphers`, `extensions`, `groups` and `point formats`.

## `Ja3Error`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Ja3Error {
    FieldCount(usize),
    Number { field: Ja3Field, token: String },
    Empty(Ja3Field),
    TooLong,
}
```

<ResponseField name="Ja3Error" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="FieldCount(usize)">

The string does not have exactly five comma-separated fields.

</ResponseField>

<ResponseField name="Number { field, token }">

A value is not a decimal number that fits its field.

</ResponseField>

<ResponseField name="Empty(Ja3Field)">

A field that must hold at least one value is empty (after GREASE removal): the
version or the ciphers.

</ResponseField>

<ResponseField name="TooLong">

The string is longer than `MAX_JA3_LEN`.

</ResponseField>

</Expandable>

</ResponseField>

## `hex_lower`, `md5_hex`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn hex_lower(bytes: &[u8]) -> String
pub fn md5_hex(data: &[u8]) -> String
```

Lowercase hex encoding of `bytes`, and the lowercase hex MD5 of `data`. MD5 is
broken as a hash: it is here only because JA3 and the peetprint hash are defined
with it. Do not use it for anything that needs collision resistance.

## `Peetprint`

<Badge color="blue" shape="pill">struct</Badge>

```rust
// seleniumbase_rs_stealth::codec::ja3::peetprint, re-exported as ja3::Peetprint and at the root
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Peetprint {
    pub versions: Vec<Slot>,
    pub alpn: Vec<String>,
    pub groups: Vec<Slot>,
    pub signature_algorithms: Vec<Slot>,
    pub psk_modes: Vec<u8>,
    pub cert_compression: Vec<u16>,
    pub ciphers: Vec<Slot>,
    pub extensions: Vec<Slot>,
}
```

A JA3-like TLS fingerprint that also records signature algorithms, in the format
of a public TLS inspection service, read from its description: eight fields joined
by `|`, each a `-` separated list. They are the supported versions, the ALPN
protocols (written `1.0`, `1.1`, `2`, or `x` and the hex of any other name), the
supported groups, the signature algorithms, the PSK key exchange modes, the
certificate compression algorithms, the cipher suites and the extensions. GREASE
ids are written as the word `GREASE`. It implements `Display` and `FromStr`.

<ResponseField name="versions, groups, signature_algorithms, ciphers" type="Vec<Slot>">

As sent, GREASE kept as `Slot::Grease`.

</ResponseField>

<ResponseField name="alpn" type="Vec<String>">

ALPN protocol names (`h2`, `http/1.1`, ...) as sent.

</ResponseField>

<ResponseField name="psk_modes" type="Vec<u8>">

PSK key exchange modes as sent.

</ResponseField>

<ResponseField name="cert_compression" type="Vec<u16>">

Certificate compression algorithms as sent.

</ResponseField>

<ResponseField name="extensions" type="Vec<Slot>">

The extensions. `Peetprint::of` sorts them; parsing keeps the given order.

</ResponseField>

### `Peetprint` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn of(hello: &ClientHello) -> Self
pub fn normalized(mut self) -> Self
pub fn extension_ids(&self) -> Vec<u16>
pub fn hash(&self) -> String
pub fn parse(text: &str) -> Result<Self, PeetprintError>
```

- `of`: the peetprint of a parsed hello, normalized.
- `normalized`: the same peetprint with the extension list sorted by the text of
  each token. The public description says only that the list is sorted; parsing
  accepts any order, and the spec layer compares extension lists as sets, so the
  sort rule never decides whether a custom peetprint is accepted.
- `extension_ids`: the extension ids without GREASE, in the stored order.
- `hash`: the lowercase hex MD5 of the string, the peetprint hash.
- `parse`: reads a string of at most `MAX_PEETPRINT_LEN` (16 KiB), trimmed; a
  `PeetprintError` names the field and the token that failed.

## `Slot`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Slot {
    Grease,
    Id(u16),
}

impl Slot {
    pub fn from_id(id: u16) -> Self;
}
```

A list entry of a peetprint: any GREASE value (RFC 8701), or a concrete id.
`Slot::from_id` collapses a GREASE id into `Slot::Grease`.

## `slots`, `ids`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn slots(values: &[u16]) -> Vec<Slot>
pub fn ids(values: &[Slot]) -> Vec<u16>
```

In `ja3::peetprint`: the GREASE-collapsed slots of a list of wire ids, and the
concrete ids of a slot list with GREASE dropped.

## `PeetField`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PeetField {
    Versions,
    Alpn,
    Groups,
    SignatureAlgorithms,
    PskModes,
    CertCompression,
    Ciphers,
    Extensions,
}
```

One of the eight fields of a peetprint, named in errors. Its `Display` is
`versions`, `alpn`, `groups`, `signature algorithms`, `psk modes`,
`certificate compression`, `ciphers` and `extensions`.

## `PeetprintError`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PeetprintError {
    FieldCount(usize),
    Token { field: PeetField, token: String },
    TooLong,
}
```

<ResponseField name="PeetprintError" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="FieldCount(usize)">

The string does not have exactly eight `|` separated fields.

</ResponseField>

<ResponseField name="Token { field, token }">

A token is neither a number that fits the field, `GREASE`, nor a known ALPN form.

</ResponseField>

<ResponseField name="TooLong">

The string is longer than `MAX_PEETPRINT_LEN`.

</ResponseField>

</Expandable>

</ResponseField>

## Constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const MAX_JA3_LEN: usize = 16 * 1024;           // ja3
pub const MAX_PEETPRINT_LEN: usize = 16 * 1024;     // ja3::peetprint
```

The longest JA3 string `Ja3::parse` reads, and the longest peetprint
`Peetprint::parse` reads.

## Related

<Columns cols={2}>

<Card color="#a631be" title="JA4" icon="fingerprint" href="/stealth/api/codec-ja4">

The order-stable TLS fingerprint.

</Card>

<Card color="#693acf" title="TlsSpec overlays" icon="lock" href="/stealth/api/codec-tls-spec">

Lay a custom JA3 or peetprint over a measured spec.

</Card>

</Columns>
