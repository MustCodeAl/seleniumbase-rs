---
title: "codec::ja4"
sidebarTitle: "JA4"
description: "JA4 in seleniumbase-rs-codec, after FoxIO's TLS-client definition: Ja4, Ja4Raw (JA4_r and JA4_ro), Ja4Facts, Ja4Order, Ja4Diff, Ja4AField, Ja4Section, Ja4Error and the a-section helpers."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_codec::ja4` computes `JA4` and `JA4_r` from a `ClientHello`,
following FoxIO's TLS-client definition. Only JA4 itself is implemented (the TLS
client method, BSD-3-Clause); JA4H, JA4S, JA4X and the other JA4+ methods carry a
different licence and are not part of the crate.

A JA4 string is `a_b_c`:

- `a` holds the transport (`t`), the highest TLS version (`13`), whether SNI is
  present (`d`, or `i` without `server_name`), the cipher and extension counts
  (two digits each, at most 99), and the first and last character of the first
  ALPN value (`00` without one);
- `b` is the first 12 hex digits of the SHA-256 of the sorted cipher suites;
- `c` is the same hash over the sorted extensions (without SNI and ALPN) followed
  by `_` and the signature algorithms in wire order.

The variants are `JA4_o` (original order, SNI and ALPN kept in the extension list),
`JA4_r` (sorted, unhashed) and `JA4_ro` (original order, unhashed). GREASE values
are ignored everywhere. JA4 sorts the extensions first, so it stays stable while
Chrome shuffles them on every connection.

<RequestExample>

```rust Hash a JA4_r
use seleniumbase_rs_codec::ja4::Ja4Raw;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // FoxIO's published example, unhashed.
    let raw = Ja4Raw::parse(
        "t13d1516h2_002f,0035,009c,009d,1301,1302,1303,c013,c014,c02b,c02c,c02f,c030,cca8,cca9_\
         0005,000a,000b,000d,0012,0015,0017,001b,0023,002b,002d,0033,4469,ff01_\
         0403,0804,0401,0503,0805,0501,0806,0601",
    )?;
    println!("{}", raw.hashed());
    Ok(())
}
```

```rust Compare two fingerprints
use seleniumbase_rs_codec::ja4::{Ja4, Ja4AField};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chrome: Ja4 = "t13d1517h2_8daaf6152771_cb7bf5808d99".parse()?;
    let other: Ja4 = "t13d1516h2_8daaf6152771_e5627efa2ab1".parse()?;
    let diff = chrome.diff(&other);
    assert_eq!(diff.a_fields, vec![Ja4AField::ExtensionCount]);
    println!("{diff:?}");
    Ok(())
}
```

```rust From a parsed hello
use seleniumbase_rs_codec::client_hello::ClientHello;
use seleniumbase_rs_codec::ja4::Ja4;

fn all_four(hello: &ClientHello) -> [String; 4] {
    [
        Ja4::of(hello).to_string(),                 // JA4
        Ja4::of_original_order(hello).to_string(),  // JA4_o
        Ja4::raw(hello),                            // JA4_r
        Ja4::raw_original_order(hello),             // JA4_ro
    ]
}
```

</RequestExample>

<ResponseExample>

```text Hash a JA4_r
t13d1516h2_8daaf6152771_e5627efa2ab1
```

```text Compare two fingerprints
Ja4Diff { a_fields: [ExtensionCount], b: false, c: true }
```

</ResponseExample>

The first pair is the published example vector the crate's tests pin. The second
compares the JA4 recorded for Chrome 155 on macOS with that example.

## `Ja4`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ja4 {
    pub a: String,
    pub b: String,
    pub c: String,
}
```

A hashed JA4 fingerprint, `a_b_c`. It implements `Display` and `FromStr`.

<ResponseField name="a" type="String">

The `a` section, ten characters, such as `t13d1517h2`.

</ResponseField>

<ResponseField name="b" type="String">

Twelve hex digits over the cipher suites (`000000000000` for an empty list).

</ResponseField>

<ResponseField name="c" type="String">

Twelve hex digits over the extensions and signature algorithms.

</ResponseField>

### `Ja4` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn of(hello: &ClientHello) -> Self
pub fn of_original_order(hello: &ClientHello) -> Self
pub fn raw(hello: &ClientHello) -> String
pub fn raw_original_order(hello: &ClientHello) -> String
pub fn parse(text: &str) -> Result<Self, Ja4Error>
pub fn diff(&self, other: &Ja4) -> Ja4Diff
```

- `of`: the `JA4` of a hello, sorted ciphers and extensions, hashed.
- `of_original_order`: the `JA4_o`, wire order, hashed.
- `raw`: the `JA4_r` string, sorted and unhashed.
- `raw_original_order`: the `JA4_ro` string, wire order and unhashed.
- `parse`: reads `a_b_c` (trimmed); `a` must have the documented ten-character
  shape and `b` and `c` must be twelve lowercase hex digits.
- `diff`: which parts differ from `other`.

## `Ja4Raw`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ja4Raw {
    pub a: String,
    pub ciphers: Vec<u16>,
    pub extensions: Vec<u16>,
    pub signature_algorithms: Vec<u16>,
}

impl Ja4Raw {
    pub fn parse(text: &str) -> Result<Self, Ja4Error>;
    pub fn hashed(&self) -> Ja4;
}
```

An unhashed fingerprint (`JA4_r` or `JA4_ro`): the `a` section and the id lists
behind `b` and `c`. Its `Display` writes the ids as four lowercase hex digits
joined by commas, and adds `_` and the signature algorithms only when there are
any.

<ResponseField name="a" type="String">

The `a` section.

</ResponseField>

<ResponseField name="ciphers" type="Vec<u16>">

Cipher suites, GREASE removed.

</ResponseField>

<ResponseField name="extensions" type="Vec<u16>">

Extensions, GREASE removed; sorted and without SNI and ALPN in the `JA4_r` form.

</ResponseField>

<ResponseField name="signature_algorithms" type="Vec<u16>">

Signature algorithms in wire order.

</ResponseField>

`Ja4Raw::parse` reads `a_ciphers_extensions[_signature_algorithms]` with ids as
four hex digits, at most `MAX_JA4_LEN` (16 KiB). The string does not say whether
it is sorted; the caller knows which variant it holds. `hashed` gives the JA4 of
these lists, as JA4 defines `b` and `c`.

## `Ja4Facts`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ja4Facts {
    pub legacy_version: u16,
    pub supported_versions: Vec<u16>,
    pub ciphers: Vec<u16>,
    pub extensions: Vec<u16>,
    pub first_alpn: Option<Vec<u8>>,
    pub signature_algorithms: Vec<u16>,
}

impl Ja4Facts {
    pub fn of(hello: &ClientHello) -> Self;
    pub fn a(&self) -> String;
    pub fn raw(&self, order: Ja4Order) -> Ja4Raw;
    pub fn ja4(&self, order: Ja4Order) -> Ja4;
}
```

The facts of a hello that JA4 reads, independent of how the hello was obtained.
`TlsSpec::ja4_facts` builds one from a spec instead of a hello.

<ResponseField name="legacy_version" type="u16">

The handshake `client_version`, used when no `supported_versions` is present.

</ResponseField>

<ResponseField name="supported_versions" type="Vec<u16>">

The `supported_versions` list; empty when the extension is absent. The highest
non-GREASE value is the version in `a`.

</ResponseField>

<ResponseField name="ciphers, extensions" type="Vec<u16>">

Cipher suites and every extension id, in wire order (SNI and ALPN included).

</ResponseField>

<ResponseField name="first_alpn" type="Option<Vec<u8>>">

The first ALPN protocol name, when the hello offers ALPN.

</ResponseField>

<ResponseField name="signature_algorithms" type="Vec<u16>">

Signature algorithms in wire order; empty when the extension is absent.

</ResponseField>

`a()` is the `a` section; `raw(order)` is `JA4_r` for `Ja4Order::Sorted` and
`JA4_ro` for `Ja4Order::Original`; `ja4(order)` is `JA4` or `JA4_o`.

## `Ja4Order`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ja4Order {
    Sorted,
    Original,
}
```

<ResponseField name="Ja4Order" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Sorted">

Sorted ids, SNI and ALPN left out of the extension list: `JA4` and `JA4_r`.

</ResponseField>

<ResponseField name="Original">

Wire order, SNI and ALPN kept: `JA4_o` and `JA4_ro`.

</ResponseField>

</Expandable>

</ResponseField>

## `Ja4Diff`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ja4Diff {
    pub a_fields: Vec<Ja4AField>,
    pub b: bool,
    pub c: bool,
}

impl Ja4Diff {
    pub fn is_empty(&self) -> bool;
}
```

The difference between two fingerprints, part by part: the fields of `a` that
differ, and whether the cipher hash and the extension hash differ. `is_empty` is
true when the fingerprints are equal.

## `Ja4AField`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ja4AField {
    Protocol,
    Version,
    Sni,
    CipherCount,
    ExtensionCount,
    Alpn,
}
```

A field of the `a` section: the transport letter (character 1), the TLS version
code (2 to 3), the SNI letter (4), the cipher count (5 to 6), the extension count
(7 to 8) and the ALPN characters (9 to 10).

## `Ja4Section`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ja4Section {
    A,
    Ciphers,
    Extensions,
    SignatureAlgorithms,
}
```

A section of a fingerprint, named in errors and mismatches (`TlsSpec` overlays
report `SpecError::Ja4Mismatch(Ja4Section)`). Its `Display` is `a`, `ciphers`,
`extensions` and `signature algorithms`.

## `Ja4Error`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Ja4Error {
    Sections(usize),
    PartA(String),
    Hash(Ja4Section),
    Hex { section: Ja4Section, token: String },
    TooLong,
}
```

<ResponseField name="Ja4Error" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="Sections(usize)">

The string has the wrong number of `_` separated sections: 3, or 4 when raw and
signed.

</ResponseField>

<ResponseField name="PartA(String)">

The `a` section is not ten characters of the documented shape.

</ResponseField>

<ResponseField name="Hash(Ja4Section)">

A hashed section is not twelve lowercase hex digits.

</ResponseField>

<ResponseField name="Hex { section, token }">

A raw id is not four hex digits.

</ResponseField>

<ResponseField name="TooLong">

The string is longer than `MAX_JA4_LEN`.

</ResponseField>

</Expandable>

</ResponseField>

## `version_code`, `version_from_code`, `alpn_code`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn version_code(version: u16) -> &'static str
pub fn version_from_code(code: &str) -> Option<u16>
pub fn alpn_code(first: Option<&[u8]>) -> String
```

- `version_code`: the two-character code of a TLS version (`13` for `0x0304`,
  `12`, `11`, `10`, `s3`, `s2`, and `d1` to `d3` for DTLS), or `00` when unknown.
- `version_from_code`: the version a code stands for, `None` for `00` and unknown
  codes.
- `alpn_code`: the ALPN field of `a` for the first ALPN value: the first and last
  character when both are ASCII alphanumeric, else the first and last hex digit of
  the whole value, and `00` without a value.

## Constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const MAX_JA4_LEN: usize = 16 * 1024;
pub const NO_ALPN: &str = "00";
```

The longest raw fingerprint string `Ja4Raw::parse` reads, and the ALPN field of a
hello with no usable ALPN value.

## Related

<Columns cols={2}>

<Card color="#693acf" title="TlsSpec" icon="lock" href="/stealth/api/codec-tls-spec">

`ja4()`, `ja4_raw(order)` and the `JA4_r` overlay of a spec.

</Card>

<Card color="#1a85d4" title="Limits" icon="triangle-exclamation" href="/stealth/limits">

Why the client's JA4 differs from Chrome's by the known gaps.

</Card>

</Columns>
