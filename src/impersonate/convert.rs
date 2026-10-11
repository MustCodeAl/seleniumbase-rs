//! Maps the pure wire specs onto the option types of wreq and `BoringSSL`.
//!
//! This module and [`client`](super::client) are the only ones that name wreq types, so a change
//! in the wreq release candidate stays inside two files. Everything here is a plain function from
//! a spec to an option value:
//!
//! - [`tls_options`]: a [`TlsSpec`] to [`TlsOptions`] (cipher list, curves, signature algorithms,
//!   extension switches and order, key shares, ALPN and ALPS, certificate compression);
//! - [`http2_options`]: an [`H2Spec`] to [`Http2Options`] (SETTINGS and their order, connection
//!   window, PRIORITY frames, HEADERS priority, pseudo-header order);
//! - [`wreq_headers`]: a rendered header list to a `HeaderMap` and the `OrigHeaderMap` that keeps
//!   the order and capitalisation.
//!
//! [`check_emittable`] holds the rules that `TlsSpec::check_reproducible` cannot know because
//! they belong to this client stack: which extensions `BoringSSL` sends whatever it is told, which
//! it sends only for some protocol versions, and which ALPN names wreq can announce. They were
//! read from the vendored `BoringSSL` and are proven on the wire by `tests/impersonate_wire.rs`.
//!
//! Nothing here reads a clock, a file or the network.

use std::borrow::Cow;
use std::io::{self, Write};

use thiserror::Error;
use wreq::header::{HeaderMap, HeaderName, HeaderValue, OrigHeaderMap};
use wreq::http2::{
    Http2Options, Http2OptionsBuilder, Priorities, Priority, PseudoId, PseudoOrder, SettingId,
    SettingsOrder, StreamDependency, StreamId,
};
use wreq::tls::compress::{CertificateCompressionAlgorithm, CertificateCompressor, Codec};
use wreq::tls::{
    AlpnProtocol, AlpsProtocol, ExtensionType, KeyShare, TlsOptions, TlsOptionsBuilder, TlsVersion,
};

use crate::wire::akamai::{PriorityFrame, Pseudo, MAX_STREAM_ID};
use crate::wire::client_hello::ext_id;
use crate::wire::h2_spec::{setting_id, H2Spec, INITIAL_CONNECTION_WINDOW};
use crate::wire::ids::{
    ALPN_H2, ALPN_HTTP_1_1, BORINGSSL_ALWAYS_SENT_EXTENSIONS, BORINGSSL_LEGACY_ONLY_EXTENSIONS,
    BORINGSSL_TLS13_ONLY_EXTENSIONS, CIPHER_SUITES, SIGNATURE_ALGORITHMS,
};
use crate::wire::tls_spec::TlsSpec;

/// TLS 1.0 on the wire.
const TLS_1_0: u16 = 0x0301;
/// TLS 1.1 on the wire.
const TLS_1_1: u16 = 0x0302;
/// TLS 1.2 on the wire.
const TLS_1_2: u16 = 0x0303;
/// TLS 1.3 on the wire.
const TLS_1_3: u16 = 0x0304;
/// RFC 8879 id of zlib certificate compression.
const CERT_COMPRESSION_ZLIB: u16 = 1;
/// RFC 8879 id of brotli certificate compression.
const CERT_COMPRESSION_BROTLI: u16 = 2;
/// The most bytes a decompressed certificate message may take (the 24-bit handshake length).
const MAX_DECOMPRESSED_CERTIFICATE: u64 = 0x00ff_ffff;
/// Brotli quality used when this client compresses a certificate; it never does so in practice.
const BROTLI_QUALITY: i32 = 5;
/// Brotli window used when this client compresses a certificate.
const BROTLI_WINDOW_BITS: i32 = 22;
/// Zlib compression level used when this client compresses a certificate.
const ZLIB_LEVEL: u32 = 6;
/// The separator of the lists `BoringSSL` parses (ciphers, curves, signature algorithms).
const LIST_SEPARATOR: &str = ":";
/// Header names whose values are marked sensitive, so `Debug` never prints them.
const SENSITIVE_HEADERS: [&str; 3] = ["cookie", "authorization", "proxy-authorization"];

/// One group `BoringSSL` can offer: its wire id, its spelling in `SSL_CTX_set1_curves_list` and
/// the key share that sends it.
struct GroupEntry {
    /// The wire id.
    id: u16,
    /// The spelling `BoringSSL` parses.
    name: &'static str,
    /// The key share wreq sends for it.
    share: KeyShare,
}

/// Every group the vendored `BoringSSL` of btls 0.5.6 names, with the post-quantum and FFDHE
/// additions of its patches.
const GROUPS: [GroupEntry; 10] = [
    GroupEntry {
        id: 0x001D,
        name: "X25519",
        share: KeyShare::X25519,
    },
    GroupEntry {
        id: 0x0017,
        name: "P-256",
        share: KeyShare::P256,
    },
    GroupEntry {
        id: 0x0018,
        name: "P-384",
        share: KeyShare::P384,
    },
    GroupEntry {
        id: 0x0019,
        name: "P-521",
        share: KeyShare::P521,
    },
    GroupEntry {
        id: 0x11EC,
        name: "X25519MLKEM768",
        share: KeyShare::X25519_MLKEM768,
    },
    GroupEntry {
        id: 0x6399,
        name: "X25519Kyber768Draft00",
        share: KeyShare::X25519_KYBER768_DRAFT00,
    },
    GroupEntry {
        id: 0x0202,
        name: "MLKEM1024",
        share: KeyShare::MLKEM1024,
    },
    GroupEntry {
        id: 0xFE32,
        name: "P256Kyber768Draft00",
        share: KeyShare::P256_KYBER768_DRAFT00,
    },
    GroupEntry {
        id: 0x0100,
        name: "ffdhe2048",
        share: KeyShare::FFDHE2048,
    },
    GroupEntry {
        id: 0x0101,
        name: "ffdhe3072",
        share: KeyShare::FFDHE3072,
    },
];

/// The groups `BoringSSL` drops from a hello that offers no TLS 1.3, because they only exist in
/// TLS 1.3 key shares.
const POST_QUANTUM_GROUPS: [u16; 4] = [0x11EC, 0x6399, 0x0202, 0xFE32];

/// Why a spec could not be turned into wreq options.
///
/// The messages name ids and lists, never header values: a value can be a cookie.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ConvertError {
    /// wreq announces only `h2` and `http/1.1` over ALPN.
    #[error("ALPN protocol `{0}` cannot be announced; wreq offers `h2` and `http/1.1`")]
    UnsupportedAlpn(String),
    /// wreq announces only `h2` and `http/1.1` over ALPS.
    #[error("ALPS protocol `{0}` cannot be announced; wreq offers `h2` and `http/1.1`")]
    UnsupportedAlps(String),
    /// `BoringSSL` has no name for this cipher suite.
    #[error("cipher suite {0:#06x} has no name BoringSSL can parse")]
    CipherName(u16),
    /// `BoringSSL` has no name for this signature algorithm.
    #[error("signature algorithm {0:#06x} has no name BoringSSL can parse")]
    SigalgName(u16),
    /// `BoringSSL` has no name for this supported group.
    #[error("supported group {0:#06x} has no name BoringSSL can parse")]
    GroupName(u16),
    /// A key share cannot be sent for this group.
    #[error("a key share cannot be sent for group {0:#06x}")]
    KeyShare(u16),
    /// The offered protocol versions are not one unbroken range.
    #[error("the offered protocol versions {0:?} are not one unbroken range")]
    VersionGap(Vec<u16>),
    /// `BoringSSL` sends this extension whatever the client asks, so a spec without it is a lie.
    #[error("extension {0:#06x} is always sent by BoringSSL and cannot be left out")]
    ExtensionAlwaysSent(u16),
    /// `BoringSSL` would not send this extension with the protocol versions the spec offers.
    #[error("extension {id:#06x} cannot be sent: {reason}")]
    ExtensionVersion {
        /// The extension.
        id: u16,
        /// The version rule that forbids it.
        reason: &'static str,
    },
    /// `BoringSSL` drops this group from a hello that offers no TLS 1.3.
    #[error("group {0:#06x} needs TLS 1.3 and the spec offers none")]
    GroupNeedsTls13(u16),
    /// No certificate compressor is available for this RFC 8879 algorithm.
    #[error("certificate compression algorithm {0} is not available")]
    CertCompression(u16),
    /// A stream id is above 2^31 - 1.
    #[error("stream id {0} does not fit 31 bits")]
    StreamId(u32),
    /// A priority weight is outside 1 to 256.
    #[error("priority weight {0} is outside 1 to 256")]
    Weight(u16),
    /// A header cannot be sent.
    #[error("header `{name}` cannot be sent: {reason}")]
    Header {
        /// The header name.
        name: String,
        /// Why it was refused.
        reason: &'static str,
    },
}

// ===== TLS =====

/// The oldest and newest protocol version the spec offers.
///
/// A spec without `supported_versions` offers TLS 1.2 only.
fn version_range(spec: &TlsSpec) -> Result<(u16, u16), ConvertError> {
    let mut versions = spec.supported_versions.clone();
    versions.sort_unstable_by(|left, right| right.cmp(left));
    let unbroken = versions
        .windows(2)
        .all(|pair| matches!(pair, [high, low] if high - low == 1));
    if !unbroken {
        return Err(ConvertError::VersionGap(spec.supported_versions.clone()));
    }
    match (versions.first(), versions.last()) {
        (Some(&max), Some(&min)) => Ok((min, max)),
        _ => Ok((TLS_1_2, TLS_1_2)),
    }
}

/// The wreq value of a wire protocol version.
fn tls_version(version: u16) -> Option<TlsVersion> {
    match version {
        TLS_1_0 => Some(TlsVersion::TLS_1_0),
        TLS_1_1 => Some(TlsVersion::TLS_1_1),
        TLS_1_2 => Some(TlsVersion::TLS_1_2),
        TLS_1_3 => Some(TlsVersion::TLS_1_3),
        _ => None,
    }
}

/// The wreq ALPN value of a protocol name.
fn alpn_protocol(name: &str) -> Result<AlpnProtocol, ConvertError> {
    match name {
        ALPN_H2 => Ok(AlpnProtocol::HTTP2),
        ALPN_HTTP_1_1 => Ok(AlpnProtocol::HTTP1),
        other => Err(ConvertError::UnsupportedAlpn(other.to_owned())),
    }
}

/// The wreq ALPS value of a protocol name.
fn alps_protocol(name: &str) -> Result<AlpsProtocol, ConvertError> {
    match name {
        ALPN_H2 => Ok(AlpsProtocol::HTTP2),
        ALPN_HTTP_1_1 => Ok(AlpsProtocol::HTTP1),
        other => Err(ConvertError::UnsupportedAlps(other.to_owned())),
    }
}

/// Checks the rules of this client stack that the spec's own check cannot know.
///
/// # Errors
///
/// Returns the first [`ConvertError`] found: an ALPN or ALPS name wreq cannot announce, versions
/// with a gap, an extension `BoringSSL` always sends but the spec leaves out, an extension that
/// needs a protocol version the spec does not offer, a post-quantum group without TLS 1.3, or a
/// certificate compression algorithm with no compressor.
pub fn check_emittable(spec: &TlsSpec) -> Result<(), ConvertError> {
    for name in &spec.alpn {
        alpn_protocol(name)?;
    }
    if let Some(alps) = &spec.alps {
        for name in &alps.protocols {
            alps_protocol(name)?;
        }
    }
    let (min, max) = version_range(spec)?;
    check_extension_versions(spec, min, max)?;
    if max < TLS_1_3 {
        if let Some(&group) = spec
            .supported_groups
            .iter()
            .find(|group| POST_QUANTUM_GROUPS.contains(group))
        {
            return Err(ConvertError::GroupNeedsTls13(group));
        }
    }
    spec.cert_compression
        .iter()
        .try_for_each(|&algorithm| compressor(algorithm).map(drop))
}

/// Checks the extensions that depend on the protocol versions offered.
fn check_extension_versions(spec: &TlsSpec, min: u16, max: u16) -> Result<(), ConvertError> {
    for &id in BORINGSSL_ALWAYS_SENT_EXTENSIONS {
        if !spec.has_extension(id) {
            return Err(ConvertError::ExtensionAlwaysSent(id));
        }
    }
    let rules = [
        (
            BORINGSSL_LEGACY_ONLY_EXTENSIONS,
            min < TLS_1_3,
            "BoringSSL sends it only when TLS 1.2 or older is offered",
        ),
        (
            BORINGSSL_TLS13_ONLY_EXTENSIONS,
            max >= TLS_1_3,
            "BoringSSL sends it only when TLS 1.3 is offered",
        ),
    ];
    for (ids, sent, reason) in rules {
        for &id in ids {
            match (sent, spec.has_extension(id)) {
                (true, false) => return Err(ConvertError::ExtensionAlwaysSent(id)),
                (false, true) => return Err(ConvertError::ExtensionVersion { id, reason }),
                _ => {}
            }
        }
    }
    if min >= TLS_1_3 && spec.session_ticket() {
        return Err(ConvertError::ExtensionVersion {
            id: ext_id::SESSION_TICKET,
            reason: "the TLS 1.2 session ticket extension is not sent in a TLS 1.3-only hello",
        });
    }
    if spec.alps.is_some() && (max < TLS_1_3 || spec.alpn.is_empty()) {
        return Err(ConvertError::ExtensionVersion {
            id: ext_id::ALPS,
            reason: "ALPS needs TLS 1.3 and an ALPN list",
        });
    }
    Ok(())
}

/// The cipher list string: every cipher by its standard name, in the spec's order.
fn cipher_list(spec: &TlsSpec) -> Result<String, ConvertError> {
    let names = spec
        .cipher_suites
        .iter()
        .map(|&id| CIPHER_SUITES.name(id).ok_or(ConvertError::CipherName(id)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names.join(LIST_SEPARATOR))
}

/// The curves list string, in the spec's order.
fn curves_list(spec: &TlsSpec) -> Result<String, ConvertError> {
    let names = spec
        .supported_groups
        .iter()
        .map(|&id| group_entry(id).map(|entry| entry.name))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names.join(LIST_SEPARATOR))
}

/// The group table row of an id.
fn group_entry(id: u16) -> Result<&'static GroupEntry, ConvertError> {
    GROUPS
        .iter()
        .find(|entry| entry.id == id)
        .ok_or(ConvertError::GroupName(id))
}

/// A signature algorithm list string, in the given order.
fn sigalgs_string(ids: &[u16]) -> Result<String, ConvertError> {
    let names = ids
        .iter()
        .map(|&id| {
            SIGNATURE_ALGORITHMS
                .name(id)
                .ok_or(ConvertError::SigalgName(id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names.join(LIST_SEPARATOR))
}

/// The key shares to send, in the spec's order.
fn key_shares(spec: &TlsSpec) -> Result<Vec<KeyShare>, ConvertError> {
    spec.key_share_groups
        .iter()
        .map(|&id| {
            group_entry(id)
                .map(|entry| entry.share)
                .map_err(|_unknown| ConvertError::KeyShare(id))
        })
        .collect()
}

/// A `BoringSSL` certificate compressor for one RFC 8879 algorithm.
#[derive(Debug)]
struct Compressor {
    /// The algorithm.
    algorithm: u16,
}

/// The brotli compressor.
static BROTLI: Compressor = Compressor {
    algorithm: CERT_COMPRESSION_BROTLI,
};
/// The zlib compressor.
static ZLIB: Compressor = Compressor {
    algorithm: CERT_COMPRESSION_ZLIB,
};

impl CertificateCompressor for Compressor {
    fn compress(&self) -> Codec {
        if self.algorithm == CERT_COMPRESSION_BROTLI {
            Codec::Pointer(brotli_compress)
        } else {
            Codec::Pointer(zlib_compress)
        }
    }

    fn decompress(&self) -> Codec {
        if self.algorithm == CERT_COMPRESSION_BROTLI {
            Codec::Pointer(brotli_decompress)
        } else {
            Codec::Pointer(zlib_decompress)
        }
    }

    fn algorithm(&self) -> CertificateCompressionAlgorithm {
        if self.algorithm == CERT_COMPRESSION_BROTLI {
            CertificateCompressionAlgorithm::BROTLI
        } else {
            CertificateCompressionAlgorithm::ZLIB
        }
    }
}

/// A writer that fails once more than `left` bytes would be written, so a hostile peer cannot
/// make a certificate decompress without bound.
struct Limited<'a> {
    /// Where the bytes go.
    inner: &'a mut dyn Write,
    /// The bytes still allowed.
    left: u64,
}

impl Write for Limited<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        if len > self.left {
            return Err(io::Error::other(
                "the decompressed certificate is larger than a handshake message can be",
            ));
        }
        self.left -= len;
        self.inner.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Compresses with brotli.
fn brotli_compress(input: &[u8], mut output: &mut dyn Write) -> io::Result<()> {
    let params = brotli::enc::BrotliEncoderParams {
        quality: BROTLI_QUALITY,
        lgwin: BROTLI_WINDOW_BITS,
        ..Default::default()
    };
    let mut reader = input;
    brotli::BrotliCompress(&mut reader, &mut output, &params).map(drop)
}

/// Decompresses brotli, within the size a certificate message can have.
fn brotli_decompress(input: &[u8], output: &mut dyn Write) -> io::Result<()> {
    let mut limited = Limited {
        inner: output,
        left: MAX_DECOMPRESSED_CERTIFICATE,
    };
    let mut reader = input;
    brotli::BrotliDecompress(&mut reader, &mut limited)
}

/// Compresses with zlib.
fn zlib_compress(input: &[u8], output: &mut dyn Write) -> io::Result<()> {
    let level = flate2::Compression::new(ZLIB_LEVEL);
    let mut encoder = flate2::write::ZlibEncoder::new(output, level);
    encoder.write_all(input)?;
    encoder.finish().map(drop)
}

/// Decompresses zlib, within the size a certificate message can have.
fn zlib_decompress(input: &[u8], output: &mut dyn Write) -> io::Result<()> {
    let mut limited = Limited {
        inner: output,
        left: MAX_DECOMPRESSED_CERTIFICATE,
    };
    let mut decoder = flate2::read::ZlibDecoder::new(input);
    io::copy(&mut decoder, &mut limited).map(drop)
}

/// The compressor for an RFC 8879 algorithm id.
fn compressor(algorithm: u16) -> Result<&'static dyn CertificateCompressor, ConvertError> {
    match algorithm {
        CERT_COMPRESSION_BROTLI => Ok(&BROTLI),
        CERT_COMPRESSION_ZLIB => Ok(&ZLIB),
        other => Err(ConvertError::CertCompression(other)),
    }
}

/// Turns a TLS spec into the options of a wreq client.
///
/// The spec is first checked with [`check_emittable`]. The cipher list names the TLS 1.3 suites
/// too and asks `BoringSSL` to keep their order. When the spec does not permute its extensions
/// the order of `spec.extensions` is pinned; when it does, `BoringSSL` shuffles them on every
/// connection, as Chrome does.
///
/// # Errors
///
/// Returns a [`ConvertError`] when the spec cannot be emitted by this client stack.
pub fn tls_options(spec: &TlsSpec) -> Result<TlsOptions, ConvertError> {
    check_emittable(spec)?;
    let (min, max) = version_range(spec)?;
    let alpn = spec
        .alpn
        .iter()
        .map(|name| alpn_protocol(name))
        .collect::<Result<Vec<_>, _>>()?;
    let mut builder = TlsOptions::builder()
        .preserve_tls13_cipher_list(true)
        .cipher_list(cipher_list(spec)?)
        .curves_list(curves_list(spec)?)
        .sigalgs_list(sigalgs_string(&spec.signature_algorithms)?)
        .key_shares(key_shares(spec)?)
        .alpn_protocols(alpn)
        .min_tls_version(tls_version(min))
        .max_tls_version(tls_version(max))
        .grease_enabled(spec.grease)
        .permute_extensions(spec.permute_extensions)
        .session_ticket(spec.session_ticket())
        .enable_ocsp_stapling(spec.ocsp_stapling())
        .enable_signed_cert_timestamps(spec.sct())
        .enable_ech_grease(spec.ech_grease())
        .record_size_limit(spec.record_size_limit);
    builder = with_alps(builder, spec)?;
    if !spec.delegated_credentials.is_empty() {
        builder = builder.delegated_credentials(sigalgs_string(&spec.delegated_credentials)?);
    }
    if !spec.cert_compression.is_empty() {
        let compressors = spec
            .cert_compression
            .iter()
            .map(|&algorithm| compressor(algorithm))
            .collect::<Result<Vec<_>, _>>()?;
        builder = builder.certificate_compressors(Cow::Owned(compressors));
    }
    if !spec.permute_extensions {
        let order: Vec<ExtensionType> = spec.extensions.iter().map(|&id| id.into()).collect();
        builder = builder.extension_permutation(order);
    }
    Ok(builder.build())
}

/// Sets the ALPS protocols and codepoint, when the spec sends the extension.
fn with_alps(
    builder: TlsOptionsBuilder,
    spec: &TlsSpec,
) -> Result<TlsOptionsBuilder, ConvertError> {
    let Some(alps) = &spec.alps else {
        return Ok(builder);
    };
    let protocols = alps
        .protocols
        .iter()
        .map(|name| alps_protocol(name))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(builder
        .alps_protocols(protocols)
        .alps_use_new_codepoint(alps.new_codepoint))
}

// ===== HTTP/2 =====

/// The protocol weight of a stream as the byte the frame carries.
fn weight_byte(weight: u16) -> Result<u8, ConvertError> {
    weight
        .checked_sub(1)
        .and_then(|byte| u8::try_from(byte).ok())
        .ok_or(ConvertError::Weight(weight))
}

/// A checked stream id.
fn stream_id(id: u32) -> Result<StreamId, ConvertError> {
    if id > MAX_STREAM_ID {
        return Err(ConvertError::StreamId(id));
    }
    Ok(StreamId::from(id))
}

/// A stream dependency from the protocol notation (weight 1 to 256).
fn dependency(
    depends_on: u32,
    weight: u16,
    exclusive: bool,
) -> Result<StreamDependency, ConvertError> {
    Ok(StreamDependency::new(
        stream_id(depends_on)?,
        weight_byte(weight)?,
        exclusive,
    ))
}

/// One PRIORITY frame as wreq sends it.
fn priority(frame: &PriorityFrame) -> Result<Priority, ConvertError> {
    Ok(Priority::new(
        stream_id(frame.stream_id)?,
        dependency(frame.depends_on, frame.weight, frame.exclusive)?,
    ))
}

/// The wreq pseudo-header id of a pseudo-header.
fn pseudo_id(pseudo: Pseudo) -> PseudoId {
    match pseudo {
        Pseudo::Method => PseudoId::Method,
        Pseudo::Authority => PseudoId::Authority,
        Pseudo::Scheme => PseudoId::Scheme,
        Pseudo::Path => PseudoId::Path,
    }
}

/// Applies one SETTINGS entry to the options.
fn apply_setting(builder: Http2OptionsBuilder, id: u16, value: u32) -> Http2OptionsBuilder {
    match id {
        setting_id::HEADER_TABLE_SIZE => builder.header_table_size(value),
        setting_id::ENABLE_PUSH => builder.enable_push(value != 0),
        setting_id::MAX_CONCURRENT_STREAMS => builder.max_concurrent_streams(value),
        setting_id::INITIAL_WINDOW_SIZE => builder.initial_window_size(value),
        setting_id::MAX_FRAME_SIZE => builder.max_frame_size(value),
        setting_id::MAX_HEADER_LIST_SIZE => builder.max_header_list_size(value),
        setting_id::ENABLE_CONNECT_PROTOCOL => builder.enable_connect_protocol(value != 0),
        setting_id::NO_RFC7540_PRIORITIES => builder.no_rfc7540_priorities(value != 0),
        _ => builder,
    }
}

/// Turns an HTTP/2 spec into the options of a wreq client.
///
/// The connection window is the protocol's 65 535 plus the measured `WINDOW_UPDATE`; the weights
/// are the protocol's 1 to 256 and become the wire byte 0 to 255. The settings the spec does not
/// list are not sent, except `SETTINGS_INITIAL_WINDOW_SIZE`, which wreq always sends (the spec's
/// own check refuses a spec without it).
///
/// # Errors
///
/// Returns a [`ConvertError`] for a stream id above 2^31 - 1 or a weight outside 1 to 256.
pub fn http2_options(spec: &H2Spec) -> Result<Http2Options, ConvertError> {
    let window = INITIAL_CONNECTION_WINDOW.saturating_add(spec.window_update);
    let mut builder = Http2Options::builder().initial_connection_window_size(window);
    for &(id, value) in &spec.settings {
        builder = apply_setting(builder, id, value);
    }
    let order = SettingsOrder::builder()
        .extend(spec.settings.iter().map(|&(id, _)| SettingId::from(id)))
        .build();
    let pseudo = PseudoOrder::builder()
        .extend(spec.pseudo_order.iter().copied().map(pseudo_id))
        .build();
    builder = builder.settings_order(order).headers_pseudo_order(pseudo);
    if let Some(headers) = spec.headers_priority {
        builder = builder.headers_stream_dependency(dependency(
            headers.depends_on,
            headers.weight,
            headers.exclusive,
        )?);
    }
    if !spec.priority_frames.is_empty() {
        let mut priorities = Priorities::builder();
        for frame in &spec.priority_frames {
            priorities = priorities.push(priority(frame)?);
        }
        builder = builder.priorities(priorities.build());
    }
    if let Some(id) = spec.initial_stream_id {
        builder = builder.initial_stream_id(id);
    }
    Ok(builder.build())
}

// ===== Headers =====

/// Turns a rendered header list into the default headers and the order and capitalisation wreq
/// writes them in.
///
/// The names keep the spelling they have in the list (HTTP/1 shows it; HTTP/2 lowercases it).
/// The values of `Cookie` and the credential headers are marked sensitive.
///
/// # Errors
///
/// Returns [`ConvertError::Header`] for a name that is not an HTTP token or a value that is not
/// a valid header value.
pub fn wreq_headers(list: &[(String, String)]) -> Result<(HeaderMap, OrigHeaderMap), ConvertError> {
    let mut map = HeaderMap::with_capacity(list.len());
    let mut order = OrigHeaderMap::with_capacity(list.len());
    for (name, value) in list {
        let refuse = |reason| ConvertError::Header {
            name: name.clone(),
            reason,
        };
        let header_name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|_bad| refuse("not a valid name"))?;
        let mut header_value =
            HeaderValue::from_str(value).map_err(|_bad| refuse("not a valid value"))?;
        if SENSITIVE_HEADERS.contains(&header_name.as_str()) {
            header_value.set_sensitive(true);
        }
        map.append(header_name, header_value);
        order.insert(name.clone());
    }
    Ok((map, order))
}

#[cfg(test)]
mod tests;
