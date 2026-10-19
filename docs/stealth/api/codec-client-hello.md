---
title: "codec::client_hello"
sidebarTitle: "ClientHello"
description: "The TLS ClientHello parser of seleniumbase-rs-codec: ClientHello, Extension, ClientHelloError, needed_len, the ext_id constants and the GREASE helpers."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: codec</Badge>

`seleniumbase_rs_codec::client_hello` parses a captured TLS `ClientHello`
incrementally, from the record header onwards, without trusting its lengths.
`ClientHello::needed_len` tells a capture loop how many bytes make up the first
handshake message while it is still arriving, and `ClientHello::parse` decodes the
whole message, which may span several TLS records. Every length is checked against
the bytes that remain, so hostile or truncated input returns a
`ClientHelloError` and never panics. GREASE values (RFC 8701) are kept as
received; the JA3, JA4 and spec builders strip them.

<RequestExample>

```rust A capture loop
use seleniumbase_rs_codec::client_hello::{ClientHello, ClientHelloError};

/// `buf` holds the bytes read from the socket so far.
fn try_parse(buf: &[u8]) -> Result<Option<ClientHello>, ClientHelloError> {
    match ClientHello::needed_len(buf)? {
        Some(end) if end <= buf.len() => Ok(Some(ClientHello::parse(&buf[..end])?)),
        _ => Ok(None), // read more
    }
}
```

```rust Read the typed fields
use seleniumbase_rs_codec::client_hello::{ext_id, without_grease, ClientHello};

fn describe(hello: &ClientHello) {
    println!("SNI {:?}", hello.sni);
    println!("ciphers {:?}", without_grease(&hello.cipher_suites));
    println!("ALPS sent: {}", hello.has_extension(ext_id::ALPS));
    println!("GREASE: {}", hello.has_grease());
}
```

</RequestExample>

## `ClientHello`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHello {
    pub record_version: u16,
    pub legacy_version: u16,
    pub random: [u8; 32],
    pub session_id: Vec<u8>,
    pub cipher_suites: Vec<u16>,
    pub compression_methods: Vec<u8>,
    pub extensions: Vec<Extension>,
    pub sni: Option<String>,
    pub alpn: Vec<Vec<u8>>,
    pub supported_versions: Vec<u16>,
    pub supported_groups: Vec<u16>,
    pub key_share_groups: Vec<u16>,
    pub point_formats: Vec<u8>,
    pub signature_algorithms: Vec<u16>,
    pub psk_modes: Vec<u8>,
    pub cert_compression: Vec<u16>,
    pub alps: Vec<Vec<u8>>,
    pub record_size_limit: Option<u16>,
    pub delegated_credentials: Vec<u16>,
}
```

A parsed `ClientHello`, with the typed fields the fingerprints read. Lists keep
GREASE values and wire order. When an extension occurs twice, the typed field
holds the value of its first occurrence; `extensions` keeps both.

<ResponseField name="record_version" type="u16">

Version in the record header of the first record.

</ResponseField>

<ResponseField name="legacy_version" type="u16">

The `client_version` field of the handshake message (771 for TLS 1.2 and 1.3 clients).

</ResponseField>

<ResponseField name="random" type="[u8; 32]">

The 32 random bytes.

</ResponseField>

<ResponseField name="session_id" type="Vec<u8>">

The legacy session id (at most 32 bytes).

</ResponseField>

<ResponseField name="cipher_suites" type="Vec<u16>">

Cipher suites in wire order.

</ResponseField>

<ResponseField name="compression_methods" type="Vec<u8>">

Compression methods in wire order.

</ResponseField>

<ResponseField name="extensions" type="Vec<Extension>">

Every extension in wire order, with its raw body.

</ResponseField>

<ResponseField name="sni" type="Option<String>">

The first host name of `server_name`.

</ResponseField>

<ResponseField name="alpn" type="Vec<Vec<u8>>">

ALPN protocol names in wire order.

</ResponseField>

<ResponseField name="supported_versions, supported_groups" type="Vec<u16>">

The two lists as sent.

</ResponseField>

<ResponseField name="key_share_groups" type="Vec<u16>">

The group of each `key_share` entry, in wire order; the key exchange bytes are skipped.

</ResponseField>

<ResponseField name="point_formats" type="Vec<u8>">

`ec_point_formats` as sent.

</ResponseField>

<ResponseField name="signature_algorithms" type="Vec<u16>">

`signature_algorithms` as sent.

</ResponseField>

<ResponseField name="psk_modes" type="Vec<u8>">

`psk_key_exchange_modes` as sent.

</ResponseField>

<ResponseField name="cert_compression" type="Vec<u16>">

`compress_certificate` algorithms as sent.

</ResponseField>

<ResponseField name="alps" type="Vec<Vec<u8>>">

Protocols of `application_settings`, on either codepoint.

</ResponseField>

<ResponseField name="record_size_limit" type="Option<u16>">

The value of `record_size_limit`.

</ResponseField>

<ResponseField name="delegated_credentials" type="Vec<u16>">

`delegated_credential` signature schemes as sent.

</ResponseField>

### `ClientHello::needed_len`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn needed_len(prefix: &[u8]) -> Result<Option<usize>, ClientHelloError>
```

How many bytes the first `ClientHello` occupies, as far as `prefix` shows.
Returns the byte length of all records that carry the message once the headers
seen so far make it known (it may exceed `prefix.len()`), and `None` while more
bytes are needed. A capture loop reads until the answer is at most the bytes it
holds. Each call costs at most `MAX_HELLO_RECORDS` record headers, however the
prefix arrived.

<ResponseField name="returns" type="Result<Option<usize>, ClientHelloError>">

Fails as soon as the bytes cannot be a `ClientHello`: `NotHandshake`,
`NotClientHello`, `Length` for a bad record length, `TooLarge` and
`TooManyRecords`.

</ResponseField>

### `ClientHello::parse`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn parse(records: &[u8]) -> Result<Self, ClientHelloError>
```

Parses the first `ClientHello` in `records`, reassembled across handshake records.
Bytes after the message (further records) are ignored. Returns
`ClientHelloError::Truncated` when the message is incomplete and another variant
when it is malformed; nothing in the input can make it panic.

### `ClientHello::extension`, `has_extension`, `extension_ids`, `has_grease`

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn extension(&self, id: u16) -> Option<&Extension>
pub fn has_extension(&self, id: u16) -> bool
pub fn extension_ids(&self) -> Vec<u16>
pub fn has_grease(&self) -> bool
```

The first extension with this id; whether the hello carries one; the extension
ids in wire order, GREASE included; and whether a GREASE value appears in the
ciphers, extensions, versions, groups or key shares.

## `Extension`

<Badge color="blue" shape="pill">struct</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extension {
    pub id: u16,
    pub data: Vec<u8>,
}
```

One extension as it arrived.

<ResponseField name="id" type="u16">

The extension type.

</ResponseField>

<ResponseField name="data" type="Vec<u8>">

The body, without the id and length prefix.

</ResponseField>

## `ClientHelloError`

<Badge color="purple" shape="pill">enum</Badge>

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClientHelloError {
    NotHandshake,
    NotClientHello(u8),
    Truncated { need: usize, have: usize },
    Length,
    TooLarge,
    TooManyRecords,
    Extension { id: u16 },
}
```

Why a byte stream is not a usable `ClientHello`.

<ResponseField name="ClientHelloError" type="enum">

<Expandable title="variants" defaultOpen>

<ResponseField name="NotHandshake">

The first record is not a TLS handshake record.

</ResponseField>

<ResponseField name="NotClientHello(u8)">

The first handshake message has a type other than `ClientHello` (1).

</ResponseField>

<ResponseField name="Truncated { need, have }">

The input ends before the message does. `need` is the least input length that
could succeed, `have` the input length that was given.

</ResponseField>

<ResponseField name="Length">

A length field disagrees with the bytes around it.

</ResponseField>

<ResponseField name="TooLarge">

The handshake message is larger than `MAX_CLIENT_HELLO` (64 KiB).

</ResponseField>

<ResponseField name="TooManyRecords">

The message is spread over more than `MAX_HELLO_RECORDS` records.

</ResponseField>

<ResponseField name="Extension { id }">

The body of one extension is malformed.

</ResponseField>

</Expandable>

</ResponseField>

## `without_grease`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub fn without_grease(values: &[u16]) -> Vec<u16>
```

Returns `values` without its GREASE entries, in the same order.

## `is_grease`

<Badge color="yellow" shape="pill">fn</Badge>

```rust
pub const fn is_grease(id: u16) -> bool
```

Whether `id` is a GREASE value (RFC 8701): both bytes are equal and the low
nibble is `0xA`. Defined in `ids` and re-exported here.

## Constants

<Badge color="gray" shape="pill">const</Badge>

```rust
pub const MAX_CLIENT_HELLO: usize = 64 * 1024;
pub const MAX_HELLO_RECORDS: usize = 1024;
```

The largest `ClientHello` handshake message, header included, that the parser
accepts; and the most TLS records one `ClientHello` may span. A 64 KiB message
fits four full records, so the record limit allows any fragmentation a client
stack would use, and no peer can make the readers walk more than that many record
headers per call.

## `ext_id`

<Badge color="gray" shape="pill">mod</Badge>

```rust
pub mod ext_id {
    pub const SERVER_NAME: u16 = 0x0000;
    pub const STATUS_REQUEST: u16 = 0x0005;
    pub const SUPPORTED_GROUPS: u16 = 0x000A;
    pub const EC_POINT_FORMATS: u16 = 0x000B;
    pub const SIGNATURE_ALGORITHMS: u16 = 0x000D;
    pub const ALPN: u16 = 0x0010;
    pub const SIGNED_CERT_TIMESTAMP: u16 = 0x0012;
    pub const PADDING: u16 = 0x0015;
    pub const EXTENDED_MASTER_SECRET: u16 = 0x0017;
    pub const COMPRESS_CERTIFICATE: u16 = 0x001B;
    pub const RECORD_SIZE_LIMIT: u16 = 0x001C;
    pub const DELEGATED_CREDENTIAL: u16 = 0x0022;
    pub const SESSION_TICKET: u16 = 0x0023;
    pub const SUPPORTED_VERSIONS: u16 = 0x002B;
    pub const PSK_KEY_EXCHANGE_MODES: u16 = 0x002D;
    pub const KEY_SHARE: u16 = 0x0033;
    pub const ALPS_OLD: u16 = 0x4469;
    pub const ALPS: u16 = 0x44CD;
    pub const TRUST_ANCHORS: u16 = 0xCA34;
    pub const ENCRYPTED_CLIENT_HELLO: u16 = 0xFE0D;
    pub const RENEGOTIATION_INFO: u16 = 0xFF01;
}
```

The extension ids the parser decodes, plus the ones the spec layer reasons
about. `ALPS_OLD` and `ALPS` are `application_settings` on its old and new
codepoint; `TRUST_ANCHORS` is the extension that BoringSSL as vendored by btls
0.5.6 cannot emit.

## Related

<Columns cols={2}>

<Card color="#d47628" title="JA3 and peetprint" icon="fingerprint" href="/stealth/api/codec-ja3">

`Ja3::of(&hello)` and `Peetprint::of(&hello)`.

</Card>

<Card color="#a631be" title="JA4" icon="fingerprint" href="/stealth/api/codec-ja4">

`Ja4::of(&hello)` and the raw variants.

</Card>

<Card color="#693acf" title="TlsSpec" icon="lock" href="/stealth/api/codec-tls-spec">

`TlsSpec::from_client_hello(&hello)` records the profile.

</Card>

<Card color="#1a85d4" title="Wire formats guide" icon="book" href="/stealth/wire-formats#reading-the-wire-formats">

The formats in context.

</Card>

</Columns>
