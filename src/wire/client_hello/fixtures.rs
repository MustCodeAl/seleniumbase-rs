//! Byte builders for the tests of the wire parsers: a small `ClientHello` assembler.
//!
//! The defaults mirror the example hellos in the public JA3 and JA4 documentation, so the
//! published fingerprints can be checked against bytes this module builds.

use super::{ext_id, is_grease};

/// Cipher suites of the published JA4 example, in wire order.
pub(crate) const CHROME_CIPHERS: [u16; 15] = [
    0x1301, 0x1302, 0x1303, 0xC02B, 0xC02F, 0xC02C, 0xC030, 0xCCA9, 0xCCA8, 0xC013, 0xC014, 0x009C,
    0x009D, 0x002F, 0x0035,
];

/// Signature algorithms of the published JA4 example, in wire order.
pub(crate) const CHROME_SIGALGS: [u16; 8] = [
    0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501, 0x0806, 0x0601,
];

/// Extension order of the published JA4 example.
pub(crate) const JA4_EXAMPLE_EXTENSIONS: [u16; 16] = [
    0x001B, 0x0000, 0x0033, 0x0010, 0x4469, 0x0017, 0x002D, 0x000D, 0x0005, 0x0023, 0x0012, 0x002B,
    0xFF01, 0x000B, 0x000A, 0x0015,
];

/// Extension order of a widely published JA3 example (a Chrome 85 style hello).
pub(crate) const JA3_EXAMPLE_EXTENSIONS: [u16; 15] = [
    0x0000, 0x0017, 0xFF01, 0x000A, 0x000B, 0x0023, 0x0010, 0x0005, 0x000D, 0x0012, 0x0033, 0x002D,
    0x002B, 0x001B, 0x0015,
];

/// The host name the default `server_name` body carries.
pub(crate) const SNI_HOST: &str = "example.com";

/// A GREASE cipher suite.
pub(crate) const GREASE_CIPHER: u16 = 0x0A0A;
/// A GREASE extension at the head of the extension list.
pub(crate) const GREASE_EXT_FIRST: u16 = 0x1A1A;
/// A GREASE extension at the tail of the extension list.
pub(crate) const GREASE_EXT_LAST: u16 = 0x3A3A;
/// A GREASE supported group.
pub(crate) const GREASE_GROUP: u16 = 0x2A2A;
/// A GREASE protocol version.
pub(crate) const GREASE_VERSION: u16 = 0x4A4A;

/// The record version the builders write.
pub(crate) const RECORD_VERSION: u16 = 0x0301;

/// A body prefixed with its one-byte length.
pub(crate) fn length8(body: &[u8]) -> Vec<u8> {
    let mut out = vec![u8::try_from(body.len()).expect("fixture body fits one length byte")];
    out.extend_from_slice(body);
    out
}

/// A body prefixed with its two-byte length.
pub(crate) fn length16(body: &[u8]) -> Vec<u8> {
    let len = u16::try_from(body.len()).expect("fixture body fits two length bytes");
    let mut out = len.to_be_bytes().to_vec();
    out.extend_from_slice(body);
    out
}

/// Big-endian encoding of a list of 16-bit values.
pub(crate) fn u16s(items: &[u16]) -> Vec<u8> {
    items.iter().flat_map(|item| item.to_be_bytes()).collect()
}

/// Strings with one-byte length prefixes, back to back.
pub(crate) fn strings8(items: &[&str]) -> Vec<u8> {
    items
        .iter()
        .flat_map(|item| length8(item.as_bytes()))
        .collect()
}

/// A `server_name` body naming one host.
pub(crate) fn server_name_body(host: &str) -> Vec<u8> {
    let mut entry = vec![0_u8];
    entry.extend(length16(host.as_bytes()));
    length16(&entry)
}

/// A `key_share` body with one entry per group; GREASE groups carry a one-byte share.
pub(crate) fn key_share_body(groups: &[u16]) -> Vec<u8> {
    let mut entries = Vec::new();
    for &group in groups {
        entries.extend_from_slice(&group.to_be_bytes());
        entries.extend(length16(&vec![0x5A; if is_grease(group) { 1 } else { 32 }]));
    }
    length16(&entries)
}

/// A browser-like body for the extension `id`; ids without a body get an empty one.
pub(crate) fn default_body(id: u16) -> Vec<u8> {
    match id {
        ext_id::SERVER_NAME => server_name_body(SNI_HOST),
        ext_id::STATUS_REQUEST => vec![1, 0, 0, 0, 0],
        ext_id::SUPPORTED_GROUPS => length16(&u16s(&[0x001D, 0x0017, 0x0018])),
        ext_id::EC_POINT_FORMATS => vec![1, 0],
        ext_id::SIGNATURE_ALGORITHMS => length16(&u16s(&CHROME_SIGALGS)),
        ext_id::ALPN => length16(&strings8(&["h2", "http/1.1"])),
        ext_id::PADDING => vec![0; 8],
        ext_id::COMPRESS_CERTIFICATE => vec![2, 0, 2],
        ext_id::RECORD_SIZE_LIMIT => vec![0x40, 0x01],
        ext_id::DELEGATED_CREDENTIAL => length16(&u16s(&[0x0403])),
        ext_id::SUPPORTED_VERSIONS => length8(&u16s(&[0x0304, 0x0303])),
        ext_id::PSK_KEY_EXCHANGE_MODES => vec![1, 1],
        ext_id::KEY_SHARE => key_share_body(&[0x001D]),
        ext_id::ALPS | ext_id::ALPS_OLD => length16(&strings8(&["h2"])),
        ext_id::ENCRYPTED_CLIENT_HELLO => vec![0, 0, 1, 0, 1, 7, 0, 2, 0xAA, 0xBB, 0, 1, 0xCC],
        ext_id::RENEGOTIATION_INFO => vec![0],
        _ => Vec::new(),
    }
}

/// Wraps a handshake message in TLS records of at most `chunk` payload bytes each.
pub(crate) fn wrap_records(message: &[u8], chunk: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for part in message.chunks(chunk) {
        out.push(22);
        out.extend_from_slice(&RECORD_VERSION.to_be_bytes());
        out.extend(length16(part));
    }
    out
}

/// Wraps a raw `ClientHello` body (no handshake header) in one record.
pub(crate) fn record_of_body(body: &[u8]) -> Vec<u8> {
    let len = body.len();
    let mut message = vec![
        1,
        0,
        u8::try_from(len >> 8).expect("short body"),
        (len & 0xFF) as u8,
    ];
    message.extend_from_slice(body);
    wrap_records(&message, usize::MAX)
}

/// Assembles a `ClientHello` from a cipher list and a list of extensions with raw bodies.
#[derive(Debug, Clone)]
pub(crate) struct HelloBuilder {
    /// The `client_version` field.
    pub(crate) legacy_version: u16,
    /// Cipher suites in wire order.
    pub(crate) ciphers: Vec<u16>,
    /// Extensions in wire order.
    pub(crate) extensions: Vec<(u16, Vec<u8>)>,
    /// The legacy session id.
    pub(crate) session_id: Vec<u8>,
}

impl HelloBuilder {
    /// A hello with the default body for every listed extension id.
    pub(crate) fn new(ciphers: &[u16], extension_ids: &[u16]) -> Self {
        Self {
            legacy_version: 0x0303,
            ciphers: ciphers.to_vec(),
            extensions: extension_ids
                .iter()
                .map(|&id| (id, default_body(id)))
                .collect(),
            session_id: vec![0x11; 32],
        }
    }

    /// The hello of the published JA4 example.
    pub(crate) fn chrome_like() -> Self {
        Self::new(&CHROME_CIPHERS, &JA4_EXAMPLE_EXTENSIONS)
    }

    /// The hello of the published JA3 example.
    pub(crate) fn ja3_example() -> Self {
        Self::new(&CHROME_CIPHERS, &JA3_EXAMPLE_EXTENSIONS)
    }

    /// Replaces the body of the first extension with this id; does nothing when it is absent.
    pub(crate) fn with_body(mut self, id: u16, body: Vec<u8>) -> Self {
        if let Some(slot) = self.extensions.iter_mut().find(|(ext, _)| *ext == id) {
            slot.1 = body;
        }
        self
    }

    /// Adds browser-style GREASE: a cipher, two extensions, a group, a version and a key share.
    pub(crate) fn with_grease(mut self) -> Self {
        self.ciphers.insert(0, GREASE_CIPHER);
        self.extensions.insert(0, (GREASE_EXT_FIRST, Vec::new()));
        self.extensions.push((GREASE_EXT_LAST, vec![0]));
        self.with_body(
            ext_id::SUPPORTED_GROUPS,
            length16(&u16s(&[GREASE_GROUP, 0x001D, 0x0017, 0x0018])),
        )
        .with_body(
            ext_id::SUPPORTED_VERSIONS,
            length8(&u16s(&[GREASE_VERSION, 0x0304, 0x0303])),
        )
        .with_body(ext_id::KEY_SHARE, key_share_body(&[GREASE_CIPHER, 0x001D]))
    }

    /// The handshake message: header and body.
    pub(crate) fn message(&self) -> Vec<u8> {
        let mut body = self.legacy_version.to_be_bytes().to_vec();
        body.extend_from_slice(&[0x42; 32]);
        body.extend(length8(&self.session_id));
        body.extend(length16(&u16s(&self.ciphers)));
        body.extend_from_slice(&[1, 0]);
        let mut block = Vec::new();
        for (id, data) in &self.extensions {
            block.extend_from_slice(&id.to_be_bytes());
            block.extend(length16(data));
        }
        body.extend(length16(&block));
        let len = body.len();
        let mut message = vec![
            1,
            0,
            u8::try_from(len >> 8).expect("short body"),
            (len & 0xFF) as u8,
        ];
        message.extend(body);
        message
    }

    /// The hello in a single record.
    pub(crate) fn record(&self) -> Vec<u8> {
        wrap_records(&self.message(), usize::MAX)
    }

    /// The hello split over records of at most `chunk` payload bytes.
    pub(crate) fn fragmented(&self, chunk: usize) -> Vec<u8> {
        wrap_records(&self.message(), chunk)
    }
}
