//! Parses a captured TLS `ClientHello` incrementally, from the record header onwards, without trusting its lengths.
//!
//! [`ClientHello::needed_len`] tells a capture loop how many bytes make up the first handshake
//! message while it is still arriving, and [`ClientHello::parse`] decodes the whole message, which
//! may span several TLS records. Every length is checked against the bytes that remain, so hostile
//! or truncated input returns a [`ClientHelloError`] and never panics. GREASE values (RFC 8701)
//! are kept as received; the JA3, JA4 and spec builders strip them.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use super::ids::is_grease;

/// Largest `ClientHello` handshake message, header included, that the parser accepts.
pub const MAX_CLIENT_HELLO: usize = 64 * 1024;

/// The most TLS records one `ClientHello` may span. A 64 KiB message fits four full records, so
/// this allows any fragmentation a client stack would use, and no peer can make the readers walk
/// more than this many record headers per call, however slowly it sends.
pub const MAX_HELLO_RECORDS: usize = 1024;

/// Bytes in a TLS record header: content type, legacy version and payload length.
const RECORD_HEADER_LEN: usize = 5;
/// Bytes in a handshake message header: message type and 24-bit length.
const HANDSHAKE_HEADER_LEN: usize = 4;
/// Largest plaintext record payload (RFC 8446, section 5.1).
const MAX_RECORD_PAYLOAD: usize = 1 << 14;
/// Record content type of a handshake record.
const CONTENT_TYPE_HANDSHAKE: u8 = 22;
/// Handshake message type of a `ClientHello`.
const HANDSHAKE_CLIENT_HELLO: u8 = 1;
/// Major byte shared by every SSL 3.0 and TLS record version.
const RECORD_VERSION_MAJOR: u16 = 3;
/// Bytes in the `ClientHello` random.
const RANDOM_LEN: usize = 32;
/// Longest legal session id.
const MAX_SESSION_ID_LEN: usize = 32;
/// Name type of a host name in the `server_name` list (RFC 6066).
const SERVER_NAME_TYPE_HOST: u8 = 0;

/// Extension ids the parser decodes, plus the ones the spec layer reasons about.
pub mod ext_id {
    /// `server_name` (RFC 6066).
    pub const SERVER_NAME: u16 = 0x0000;
    /// `status_request`, the OCSP stapling request.
    pub const STATUS_REQUEST: u16 = 0x0005;
    /// `supported_groups`.
    pub const SUPPORTED_GROUPS: u16 = 0x000A;
    /// `ec_point_formats`.
    pub const EC_POINT_FORMATS: u16 = 0x000B;
    /// `signature_algorithms`.
    pub const SIGNATURE_ALGORITHMS: u16 = 0x000D;
    /// Application-layer protocol negotiation.
    pub const ALPN: u16 = 0x0010;
    /// `signed_certificate_timestamp`.
    pub const SIGNED_CERT_TIMESTAMP: u16 = 0x0012;
    /// `padding`.
    pub const PADDING: u16 = 0x0015;
    /// `extended_master_secret`.
    pub const EXTENDED_MASTER_SECRET: u16 = 0x0017;
    /// `compress_certificate`.
    pub const COMPRESS_CERTIFICATE: u16 = 0x001B;
    /// `record_size_limit`.
    pub const RECORD_SIZE_LIMIT: u16 = 0x001C;
    /// `delegated_credential`.
    pub const DELEGATED_CREDENTIAL: u16 = 0x0022;
    /// `session_ticket`.
    pub const SESSION_TICKET: u16 = 0x0023;
    /// `supported_versions`.
    pub const SUPPORTED_VERSIONS: u16 = 0x002B;
    /// `psk_key_exchange_modes`.
    pub const PSK_KEY_EXCHANGE_MODES: u16 = 0x002D;
    /// `key_share`.
    pub const KEY_SHARE: u16 = 0x0033;
    /// `application_settings` on the old codepoint.
    pub const ALPS_OLD: u16 = 0x4469;
    /// `application_settings` on the new codepoint.
    pub const ALPS: u16 = 0x44CD;
    /// `trust_anchors`, which `BoringSSL` as vendored by btls 0.5.6 cannot emit.
    pub const TRUST_ANCHORS: u16 = 0xCA34;
    /// `encrypted_client_hello`.
    pub const ENCRYPTED_CLIENT_HELLO: u16 = 0xFE0D;
    /// `renegotiation_info`.
    pub const RENEGOTIATION_INFO: u16 = 0xFF01;
}

/// One extension as it arrived: its id and the raw bytes of its body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extension {
    /// The extension type.
    pub id: u16,
    /// The body, without the id and length prefix.
    pub data: Vec<u8>,
}

/// A parsed `ClientHello`, with the typed fields the fingerprints read.
///
/// Lists keep GREASE values and wire order. When an extension occurs twice, the typed field
/// holds the value of its first occurrence; [`ClientHello::extensions`] keeps both.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHello {
    /// Version in the record header of the first record.
    pub record_version: u16,
    /// The `client_version` field of the handshake message (771 for TLS 1.2 and 1.3 clients).
    pub legacy_version: u16,
    /// The 32 random bytes.
    pub random: [u8; RANDOM_LEN],
    /// The legacy session id.
    pub session_id: Vec<u8>,
    /// Cipher suites in wire order.
    pub cipher_suites: Vec<u16>,
    /// Compression methods in wire order.
    pub compression_methods: Vec<u8>,
    /// Every extension in wire order, with its raw body.
    pub extensions: Vec<Extension>,
    /// The first host name of `server_name`.
    pub sni: Option<String>,
    /// ALPN protocol names in wire order.
    pub alpn: Vec<Vec<u8>>,
    /// `supported_versions` as sent.
    pub supported_versions: Vec<u16>,
    /// `supported_groups` as sent.
    pub supported_groups: Vec<u16>,
    /// The group of each `key_share` entry, in wire order.
    pub key_share_groups: Vec<u16>,
    /// `ec_point_formats` as sent.
    pub point_formats: Vec<u8>,
    /// `signature_algorithms` as sent.
    pub signature_algorithms: Vec<u16>,
    /// `psk_key_exchange_modes` as sent.
    pub psk_modes: Vec<u8>,
    /// `compress_certificate` algorithms as sent.
    pub cert_compression: Vec<u16>,
    /// Protocols of `application_settings` (either codepoint).
    pub alps: Vec<Vec<u8>>,
    /// The value of `record_size_limit`.
    pub record_size_limit: Option<u16>,
    /// `delegated_credential` signature schemes as sent.
    pub delegated_credentials: Vec<u16>,
}

/// Why a byte stream is not a usable `ClientHello`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClientHelloError {
    /// The first record is not a TLS handshake record.
    #[error("the bytes do not start with a TLS handshake record")]
    NotHandshake,
    /// The first handshake message has a type other than `ClientHello` (1).
    #[error("handshake message type {0} is not a ClientHello")]
    NotClientHello(u8),
    /// The input ends before the message does.
    #[error("truncated ClientHello: need at least {need} bytes, have {have}")]
    Truncated {
        /// The least input length that could succeed.
        need: usize,
        /// The input length that was given.
        have: usize,
    },
    /// A length field disagrees with the bytes around it.
    #[error("a length field is inconsistent with the bytes around it")]
    Length,
    /// The handshake message is longer than [`MAX_CLIENT_HELLO`].
    #[error("the ClientHello is larger than 64 KiB")]
    TooLarge,
    /// The message is spread over more than [`MAX_HELLO_RECORDS`] records.
    #[error("the ClientHello spans more than {} records", MAX_HELLO_RECORDS)]
    TooManyRecords,
    /// The body of one extension is malformed.
    #[error("malformed body of extension {id:#06x}")]
    Extension {
        /// The extension type.
        id: u16,
    },
}

/// Returns `values` without its GREASE entries, in the same order.
#[must_use]
pub fn without_grease(values: &[u16]) -> Vec<u16> {
    values
        .iter()
        .copied()
        .filter(|&value| !is_grease(value))
        .collect()
}

impl ClientHello {
    /// How many bytes the first `ClientHello` occupies, as far as `prefix` shows.
    ///
    /// Returns the byte length of all records that carry the message once the headers seen so
    /// far make it known (it may exceed `prefix.len()`), and `None` while more bytes are needed.
    /// A capture loop reads until the answer is at most the bytes it holds.
    ///
    /// # Errors
    ///
    /// Fails as soon as the bytes cannot be a `ClientHello`: [`ClientHelloError::NotHandshake`],
    /// [`ClientHelloError::NotClientHello`], [`ClientHelloError::Length`] for a bad record
    /// length, [`ClientHelloError::TooLarge`] and [`ClientHelloError::TooManyRecords`]. Each call
    /// costs at most [`MAX_HELLO_RECORDS`] record headers, however the prefix arrived.
    pub fn needed_len(prefix: &[u8]) -> Result<Option<usize>, ClientHelloError> {
        Ok(scan(prefix)?.end)
    }

    /// Parses the first `ClientHello` in `records`, reassembled across handshake records.
    ///
    /// Bytes after the message (further records) are ignored.
    ///
    /// # Errors
    ///
    /// Returns [`ClientHelloError::Truncated`] when the message is incomplete and another
    /// variant when it is malformed; nothing in the input can make this function panic.
    pub fn parse(records: &[u8]) -> Result<Self, ClientHelloError> {
        let scan = scan(records)?;
        let truncated = |need: usize| ClientHelloError::Truncated {
            need,
            have: records.len(),
        };
        let end = scan.end.ok_or_else(|| truncated(records.len() + 1))?;
        let Some(whole_records) = records.get(..end) else {
            return Err(truncated(end));
        };
        let payload = gather(whole_records)?;
        let body = payload
            .get(HANDSHAKE_HEADER_LEN..scan.message_len)
            .ok_or(ClientHelloError::Length)?;
        parse_message(body, scan.record_version)
    }

    /// The first extension with this id, if the hello carries one.
    #[must_use]
    pub fn extension(&self, id: u16) -> Option<&Extension> {
        self.extensions.iter().find(|extension| extension.id == id)
    }

    /// Whether the hello carries an extension with this id.
    #[must_use]
    pub fn has_extension(&self, id: u16) -> bool {
        self.extension(id).is_some()
    }

    /// The extension ids in wire order, GREASE included.
    #[must_use]
    pub fn extension_ids(&self) -> Vec<u16> {
        self.extensions
            .iter()
            .map(|extension| extension.id)
            .collect()
    }

    /// Whether a GREASE value appears in the ciphers, extensions, versions, groups or key shares.
    #[must_use]
    pub fn has_grease(&self) -> bool {
        self.cipher_suites
            .iter()
            .chain(&self.supported_versions)
            .chain(&self.supported_groups)
            .chain(&self.key_share_groups)
            .copied()
            .any(is_grease)
            || self.extensions.iter().any(|ext| is_grease(ext.id))
    }
}

/// A forward-only cursor over a byte slice; every read is bounds-checked.
#[derive(Debug, Clone, Copy)]
struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    const fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    const fn len(&self) -> usize {
        self.buf.len()
    }

    /// The bytes not read yet.
    const fn rest(self) -> &'a [u8] {
        self.buf
    }

    /// Fails unless every byte was consumed.
    fn finish(&self) -> Result<(), ClientHelloError> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(ClientHelloError::Length)
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ClientHelloError> {
        if count > self.buf.len() {
            return Err(ClientHelloError::Length);
        }
        let (head, tail) = self.buf.split_at(count);
        self.buf = tail;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ClientHelloError> {
        let mut out = [0_u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, ClientHelloError> {
        Ok(u8::from_be_bytes(self.array()?))
    }

    fn u16(&mut self) -> Result<u16, ClientHelloError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u24(&mut self) -> Result<usize, ClientHelloError> {
        let [high, middle, low] = self.array::<3>()?;
        Ok((usize::from(high) << 16) | (usize::from(middle) << 8) | usize::from(low))
    }

    /// A vector with a one-byte length prefix.
    fn vec8(&mut self) -> Result<Reader<'a>, ClientHelloError> {
        let len = usize::from(self.u8()?);
        Ok(Reader::new(self.take(len)?))
    }

    /// A vector with a two-byte length prefix.
    fn vec16(&mut self) -> Result<Reader<'a>, ClientHelloError> {
        let len = usize::from(self.u16()?);
        Ok(Reader::new(self.take(len)?))
    }
}

/// The result of walking the records of a stream prefix.
#[derive(Debug, Default)]
struct Scan {
    /// The handshake message header, as far as the prefix holds it. Nothing after it is copied,
    /// so that a walk costs the record headers only.
    head: Vec<u8>,
    /// Version in the first record header.
    record_version: u16,
    /// Length of the handshake message, header included, once its header is known.
    message_len: usize,
    /// Byte length of the records that carry the whole message, once known.
    end: Option<usize>,
}

/// Validates a record header and returns the record version and payload length.
fn record_header(header: &[u8]) -> Result<(u16, usize), ClientHelloError> {
    let mut reader = Reader::new(header);
    let content_type = reader.u8()?;
    let version = reader.u16()?;
    let len = usize::from(reader.u16()?);
    if content_type != CONTENT_TYPE_HANDSHAKE || (version >> 8) != RECORD_VERSION_MAJOR {
        return Err(ClientHelloError::NotHandshake);
    }
    if len == 0 || len > MAX_RECORD_PAYLOAD {
        return Err(ClientHelloError::Length);
    }
    Ok((version, len))
}

/// Reads the handshake header once four payload bytes exist: the message length, header included.
fn handshake_len(payload: &[u8]) -> Result<Option<usize>, ClientHelloError> {
    let Some(head) = payload.get(..HANDSHAKE_HEADER_LEN) else {
        return Ok(None);
    };
    let mut reader = Reader::new(head);
    let kind = reader.u8()?;
    let len = reader.u24()?;
    if kind != HANDSHAKE_CLIENT_HELLO {
        return Err(ClientHelloError::NotClientHello(kind));
    }
    let total = HANDSHAKE_HEADER_LEN + len;
    if total > MAX_CLIENT_HELLO {
        return Err(ClientHelloError::TooLarge);
    }
    Ok(Some(total))
}

/// Walks the records of `prefix` until the first handshake message is complete or the bytes end.
/// It looks at no more than [`MAX_HELLO_RECORDS`] record headers and copies four bytes, so a peer
/// that sends the hello a byte at a time costs a bounded amount per call.
fn scan(prefix: &[u8]) -> Result<Scan, ClientHelloError> {
    let mut scan = Scan::default();
    let mut declared = 0_usize;
    let mut pos = 0_usize;
    let mut records = 0_usize;
    let mut total = None;
    while let Some(header) = prefix.get(pos..pos + RECORD_HEADER_LEN) {
        records += 1;
        if records > MAX_HELLO_RECORDS {
            return Err(ClientHelloError::TooManyRecords);
        }
        let (version, len) = record_header(header)?;
        if pos == 0 {
            scan.record_version = version;
        }
        let body_start = pos + RECORD_HEADER_LEN;
        pos = body_start + len;
        declared += len;
        let present = prefix
            .get(body_start..pos.min(prefix.len()))
            .unwrap_or_default();
        if total.is_none() {
            let missing = HANDSHAKE_HEADER_LEN - scan.head.len();
            scan.head.extend(present.iter().take(missing));
            total = handshake_len(&scan.head)?;
        }
        if let Some(message_len) = total.filter(|&message_len| declared >= message_len) {
            scan.message_len = message_len;
            scan.end = Some(pos);
            break;
        }
    }
    Ok(scan)
}

/// The handshake bytes of the whole records of `records`, in order.
fn gather(records: &[u8]) -> Result<Vec<u8>, ClientHelloError> {
    let mut payload = Vec::new();
    let mut pos = 0_usize;
    while let Some(header) = records.get(pos..pos + RECORD_HEADER_LEN) {
        let (_version, len) = record_header(header)?;
        let body_start = pos + RECORD_HEADER_LEN;
        pos = body_start + len;
        let body = records
            .get(body_start..pos)
            .ok_or(ClientHelloError::Length)?;
        payload.extend_from_slice(body);
    }
    Ok(payload)
}

/// Maps a bad length inside an extension body to [`ClientHelloError::Extension`].
fn tag_extension(id: u16, error: ClientHelloError) -> ClientHelloError {
    if error == ClientHelloError::Length {
        ClientHelloError::Extension { id }
    } else {
        error
    }
}

/// Decodes `list<u16>` and `list<u8>` items out of an already length-checked vector.
fn u16_items(mut items: Reader<'_>) -> Result<Vec<u16>, ClientHelloError> {
    let mut out = Vec::with_capacity(items.len() / 2);
    while !items.is_empty() {
        out.push(items.u16()?);
    }
    Ok(out)
}

/// Decodes a vector of one-byte-length-prefixed strings (ALPN names, ALPS protocols).
fn byte_strings(mut items: Reader<'_>) -> Result<Vec<Vec<u8>>, ClientHelloError> {
    let mut out = Vec::new();
    while !items.is_empty() {
        out.push(items.vec8()?.rest().to_vec());
    }
    Ok(out)
}

/// The body of an extension that is one vector with a two-byte length prefix and nothing after.
fn list16(data: &[u8]) -> Result<Reader<'_>, ClientHelloError> {
    let mut reader = Reader::new(data);
    let body = reader.vec16()?;
    reader.finish()?;
    Ok(body)
}

/// The body of an extension that is one vector with a one-byte length prefix and nothing after.
fn list8(data: &[u8]) -> Result<Reader<'_>, ClientHelloError> {
    let mut reader = Reader::new(data);
    let body = reader.vec8()?;
    reader.finish()?;
    Ok(body)
}

/// The first host name of a `server_name` body.
fn server_name(data: &[u8]) -> Result<Option<String>, ClientHelloError> {
    let mut names = list16(data)?;
    let mut host = None;
    while !names.is_empty() {
        let kind = names.u8()?;
        let name = names.vec16()?.rest();
        if kind == SERVER_NAME_TYPE_HOST && host.is_none() {
            let text = std::str::from_utf8(name)
                .ok()
                .ok_or(ClientHelloError::Length)?;
            host = Some(text.to_owned());
        }
    }
    Ok(host)
}

/// The group of every `key_share` entry; the key exchange bytes are skipped.
fn key_share_groups(data: &[u8]) -> Result<Vec<u16>, ClientHelloError> {
    let mut entries = list16(data)?;
    let mut groups = Vec::new();
    while !entries.is_empty() {
        groups.push(entries.u16()?);
        let _key_exchange = entries.vec16()?;
    }
    Ok(groups)
}

/// The value of a `record_size_limit` body.
fn record_size_limit(data: &[u8]) -> Result<u16, ClientHelloError> {
    let mut reader = Reader::new(data);
    let limit = reader.u16()?;
    reader.finish()?;
    Ok(limit)
}

/// Decodes the body of one extension into the typed field it feeds.
fn apply_extension(hello: &mut ClientHello, id: u16, data: &[u8]) -> Result<(), ClientHelloError> {
    match id {
        ext_id::SERVER_NAME => hello.sni = server_name(data)?,
        ext_id::SUPPORTED_GROUPS => hello.supported_groups = u16_items(list16(data)?)?,
        ext_id::EC_POINT_FORMATS => hello.point_formats = list8(data)?.rest().to_vec(),
        ext_id::SIGNATURE_ALGORITHMS => hello.signature_algorithms = u16_items(list16(data)?)?,
        ext_id::ALPN => hello.alpn = byte_strings(list16(data)?)?,
        ext_id::SUPPORTED_VERSIONS => hello.supported_versions = u16_items(list8(data)?)?,
        ext_id::KEY_SHARE => hello.key_share_groups = key_share_groups(data)?,
        ext_id::PSK_KEY_EXCHANGE_MODES => hello.psk_modes = list8(data)?.rest().to_vec(),
        ext_id::COMPRESS_CERTIFICATE => hello.cert_compression = u16_items(list8(data)?)?,
        ext_id::ALPS | ext_id::ALPS_OLD => hello.alps = byte_strings(list16(data)?)?,
        ext_id::RECORD_SIZE_LIMIT => hello.record_size_limit = Some(record_size_limit(data)?),
        ext_id::DELEGATED_CREDENTIAL => hello.delegated_credentials = u16_items(list16(data)?)?,
        _ => {}
    }
    Ok(())
}

/// Splits the extension block into raw extensions and decodes each into `hello`.
fn read_extensions(hello: &mut ClientHello, mut block: Reader<'_>) -> Result<(), ClientHelloError> {
    let mut seen = BTreeSet::new();
    while !block.is_empty() {
        let id = block.u16()?;
        let data = block.vec16()?.rest();
        if seen.insert(id) {
            apply_extension(hello, id, data).map_err(|error| tag_extension(id, error))?;
        }
        hello.extensions.push(Extension {
            id,
            data: data.to_vec(),
        });
    }
    Ok(())
}

/// Decodes the handshake message body (after the four header bytes).
fn parse_message(body: &[u8], record_version: u16) -> Result<ClientHello, ClientHelloError> {
    let mut reader = Reader::new(body);
    let mut hello = ClientHello {
        record_version,
        legacy_version: reader.u16()?,
        random: reader.array()?,
        ..ClientHello::default()
    };
    let session_id = reader.vec8()?.rest();
    if session_id.len() > MAX_SESSION_ID_LEN {
        return Err(ClientHelloError::Length);
    }
    hello.session_id = session_id.to_vec();
    let suites = reader.vec16()?;
    if suites.is_empty() || suites.len() % 2 != 0 {
        return Err(ClientHelloError::Length);
    }
    hello.cipher_suites = u16_items(suites)?;
    hello.compression_methods = reader.vec8()?.rest().to_vec();
    if hello.compression_methods.is_empty() {
        return Err(ClientHelloError::Length);
    }
    if !reader.is_empty() {
        let block = reader.vec16()?;
        reader.finish()?;
        read_extensions(&mut hello, block)?;
    }
    Ok(hello)
}

#[cfg(test)]
mod bounds;
#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;
