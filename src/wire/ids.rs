//! IANA names and numeric ids of the TLS and HTTP/2 values a browser fingerprint is built from,
//! plus the capability seeds of what `BoringSSL` can emit in a client hello.
//!
//! Each registry is a [`Registry`]: a static table of `(id, IANA name)` rows, looked up in both
//! directions. The `BORINGSSL_*` slices list the ids `BoringSSL` can emit. The `*_CIPHERS`,
//! `REFUSED_EXTENSIONS`, `UNSUPPORTED_GROUPS` and `ML_DSA_SIGALGS` constants list the ids the
//! spec refuses before a client exists. The numbers come from the vendored `BoringSSL` headers of
//! btls-sys 0.5.6 and were cross-checked against rustls 0.23.45 and http2 0.5.20. The WP12 probe
//! decides what the wire really carries; this module only names ids.

/// One row of a registry: a wire id and its IANA name.
pub type Row = (u16, &'static str);

/// A registry of wire ids, each with its IANA name, looked up in both directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registry {
    rows: &'static [Row],
}

impl Registry {
    /// Wraps a static table of rows. Each id and each name appears once; a test checks it.
    #[must_use]
    pub const fn new(rows: &'static [Row]) -> Self {
        Self { rows }
    }

    /// The rows in table order.
    #[must_use]
    pub const fn rows(&self) -> &'static [Row] {
        self.rows
    }

    /// The IANA name of `id`, or `None` when the registry does not list it.
    #[must_use]
    pub fn name(&self, id: u16) -> Option<&'static str> {
        self.rows
            .iter()
            .find(|&&(row_id, _)| row_id == id)
            .map(|&(_, name)| name)
    }

    /// The id of the IANA `name`, matched exactly, or `None` when the registry does not list it.
    #[must_use]
    pub fn id(&self, name: &str) -> Option<u16> {
        self.rows
            .iter()
            .find(|&&(_, row_name)| row_name == name)
            .map(|&(id, _)| id)
    }

    /// Whether the registry lists `id`.
    #[must_use]
    pub fn contains(&self, id: u16) -> bool {
        self.name(id).is_some()
    }
}

/// Whether `id` is a GREASE value (RFC 8701): both bytes are equal and the low nibble is `0xA`.
#[must_use]
pub const fn is_grease(id: u16) -> bool {
    const GREASE_LOW_NIBBLE: u16 = 0x000A;
    let byte = id >> 8;
    byte == (id & 0x00FF) && (byte & 0x000F) == GREASE_LOW_NIBBLE
}

/// The TLS 1.2 `legacy_version` of a `ClientHello`, which the JA3 version field carries (771).
pub const TLS12_LEGACY_VERSION: u16 = 0x0303;

/// The uncompressed EC point format, the only one `BoringSSL` emits (RFC 8422).
pub const EC_POINT_UNCOMPRESSED: u8 = 0x00;

/// ALPN identifier of HTTP/2 (RFC 9113).
pub const ALPN_H2: &str = "h2";

/// ALPN identifier of HTTP/1.1 (RFC 9112).
pub const ALPN_HTTP_1_1: &str = "http/1.1";

/// Every ALPN identifier this module names.
pub const ALPN_PROTOCOLS: &[&str] = &[ALPN_H2, ALPN_HTTP_1_1];

/// Whether `name` is one of the ALPN identifiers in [`ALPN_PROTOCOLS`].
#[must_use]
pub fn is_alpn(name: &str) -> bool {
    ALPN_PROTOCOLS.contains(&name)
}

/// Rows of the TLS cipher suite registry this module names, sorted by id.
const CIPHER_SUITE_ROWS: &[Row] = &[
    (0x0000, "TLS_NULL_WITH_NULL_NULL"),
    (0x0002, "TLS_RSA_WITH_NULL_SHA"),
    (0x0003, "TLS_RSA_EXPORT_WITH_RC4_40_MD5"),
    (0x0004, "TLS_RSA_WITH_RC4_128_MD5"),
    (0x0005, "TLS_RSA_WITH_RC4_128_SHA"),
    (0x0008, "TLS_RSA_EXPORT_WITH_DES40_CBC_SHA"),
    (0x000A, "TLS_RSA_WITH_3DES_EDE_CBC_SHA"),
    (0x002F, "TLS_RSA_WITH_AES_128_CBC_SHA"),
    (0x0033, "TLS_DHE_RSA_WITH_AES_128_CBC_SHA"),
    (0x0034, "TLS_DH_anon_WITH_AES_128_CBC_SHA"),
    (0x0035, "TLS_RSA_WITH_AES_256_CBC_SHA"),
    (0x0039, "TLS_DHE_RSA_WITH_AES_256_CBC_SHA"),
    (0x003C, "TLS_RSA_WITH_AES_128_CBC_SHA256"),
    (0x003D, "TLS_RSA_WITH_AES_256_CBC_SHA256"),
    (0x006C, "TLS_DH_anon_WITH_AES_128_CBC_SHA256"),
    (0x008C, "TLS_PSK_WITH_AES_128_CBC_SHA"),
    (0x008D, "TLS_PSK_WITH_AES_256_CBC_SHA"),
    (0x009C, "TLS_RSA_WITH_AES_128_GCM_SHA256"),
    (0x009D, "TLS_RSA_WITH_AES_256_GCM_SHA384"),
    (0x009E, "TLS_DHE_RSA_WITH_AES_128_GCM_SHA256"),
    (0x009F, "TLS_DHE_RSA_WITH_AES_256_GCM_SHA384"),
    (0x00A8, "TLS_PSK_WITH_AES_128_GCM_SHA256"),
    (0x00FF, "TLS_EMPTY_RENEGOTIATION_INFO_SCSV"),
    (0x1301, "TLS_AES_128_GCM_SHA256"),
    (0x1302, "TLS_AES_256_GCM_SHA384"),
    (0x1303, "TLS_CHACHA20_POLY1305_SHA256"),
    (0x1304, "TLS_AES_128_CCM_SHA256"),
    (0x1305, "TLS_AES_128_CCM_8_SHA256"),
    (0x5600, "TLS_FALLBACK_SCSV"),
    (0xC008, "TLS_ECDHE_ECDSA_WITH_3DES_EDE_CBC_SHA"),
    (0xC009, "TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA"),
    (0xC00A, "TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA"),
    (0xC010, "TLS_ECDHE_RSA_WITH_NULL_SHA"),
    (0xC011, "TLS_ECDHE_RSA_WITH_RC4_128_SHA"),
    (0xC012, "TLS_ECDHE_RSA_WITH_3DES_EDE_CBC_SHA"),
    (0xC013, "TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA"),
    (0xC014, "TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA"),
    (0xC018, "TLS_ECDH_anon_WITH_AES_128_CBC_SHA"),
    (0xC023, "TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA256"),
    (0xC024, "TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA384"),
    (0xC027, "TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA256"),
    (0xC028, "TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA384"),
    (0xC02B, "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256"),
    (0xC02C, "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384"),
    (0xC02F, "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"),
    (0xC030, "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384"),
    (0xC035, "TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA"),
    (0xC036, "TLS_ECDHE_PSK_WITH_AES_256_CBC_SHA"),
    (0xC09C, "TLS_RSA_WITH_AES_128_CCM"),
    (0xC09E, "TLS_DHE_RSA_WITH_AES_128_CCM"),
    (0xC0AC, "TLS_ECDHE_ECDSA_WITH_AES_128_CCM"),
    (0xC0AE, "TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8"),
    (0xCCA8, "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256"),
    (0xCCA9, "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256"),
    (0xCCAA, "TLS_DHE_RSA_WITH_CHACHA20_POLY1305_SHA256"),
    (0xCCAC, "TLS_ECDHE_PSK_WITH_CHACHA20_POLY1305_SHA256"),
];

/// The TLS cipher suite registry, by wire id.
pub const CIPHER_SUITES: Registry = Registry::new(CIPHER_SUITE_ROWS);

/// Rows of the TLS extension registry this module names, sorted by id.
const EXTENSION_ROWS: &[Row] = &[
    (0x0000, "server_name"),
    (0x0001, "max_fragment_length"),
    (0x0005, "status_request"),
    (0x000A, "supported_groups"),
    (0x000B, "ec_point_formats"),
    (0x000D, "signature_algorithms"),
    (0x000E, "use_srtp"),
    (0x000F, "heartbeat"),
    (0x0010, "application_layer_protocol_negotiation"),
    (0x0012, "signed_certificate_timestamp"),
    (0x0013, "client_certificate_type"),
    (0x0014, "server_certificate_type"),
    (0x0015, "padding"),
    (0x0016, "encrypt_then_mac"),
    (0x0017, "extended_master_secret"),
    (0x001B, "compress_certificate"),
    (0x001C, "record_size_limit"),
    (0x0022, "delegated_credential"),
    (0x0023, "session_ticket"),
    (0x0029, "pre_shared_key"),
    (0x002A, "early_data"),
    (0x002B, "supported_versions"),
    (0x002C, "cookie"),
    (0x002D, "psk_key_exchange_modes"),
    (0x002F, "certificate_authorities"),
    (0x0030, "oid_filters"),
    (0x0031, "post_handshake_auth"),
    (0x0032, "signature_algorithms_cert"),
    (0x0033, "key_share"),
    (0x0039, "quic_transport_parameters"),
    (0x3374, "next_protocol_negotiation"),
    (0x4469, "application_settings_old"),
    (0x44CD, "application_settings"),
    (0xCA34, "trust_anchors"),
    (0xFD00, "ech_outer_extensions"),
    (0xFE0D, "encrypted_client_hello"),
    (0xFF01, "renegotiation_info"),
];

/// The TLS extension registry, by wire id.
pub const EXTENSIONS: Registry = Registry::new(EXTENSION_ROWS);

/// Rows of the supported groups registry this module names, sorted by id.
const GROUP_ROWS: &[Row] = &[
    (0x0016, "secp256k1"),
    (0x0017, "secp256r1"),
    (0x0018, "secp384r1"),
    (0x0019, "secp521r1"),
    (0x001A, "brainpoolP256r1"),
    (0x001B, "brainpoolP384r1"),
    (0x001C, "brainpoolP512r1"),
    (0x001D, "x25519"),
    (0x001E, "x448"),
    (0x0100, "ffdhe2048"),
    (0x0101, "ffdhe3072"),
    (0x0102, "ffdhe4096"),
    (0x0103, "ffdhe6144"),
    (0x0104, "ffdhe8192"),
    (0x0200, "MLKEM512"),
    (0x0201, "MLKEM768"),
    (0x0202, "MLKEM1024"),
    (0x11EB, "SecP256r1MLKEM768"),
    (0x11EC, "X25519MLKEM768"),
    (0x6399, "X25519Kyber768Draft00"),
];

/// The supported groups registry, by wire id.
pub const GROUPS: Registry = Registry::new(GROUP_ROWS);

/// Rows of the signature scheme registry this module names, sorted by id.
const SIGNATURE_ALGORITHM_ROWS: &[Row] = &[
    (0x0201, "rsa_pkcs1_sha1"),
    (0x0203, "ecdsa_sha1"),
    (0x0401, "rsa_pkcs1_sha256"),
    (0x0403, "ecdsa_secp256r1_sha256"),
    (0x0420, "rsa_pkcs1_sha256_legacy"),
    (0x0501, "rsa_pkcs1_sha384"),
    (0x0503, "ecdsa_secp384r1_sha384"),
    (0x0601, "rsa_pkcs1_sha512"),
    (0x0603, "ecdsa_secp521r1_sha512"),
    (0x0804, "rsa_pss_rsae_sha256"),
    (0x0805, "rsa_pss_rsae_sha384"),
    (0x0806, "rsa_pss_rsae_sha512"),
    (0x0807, "ed25519"),
    (0x0808, "ed448"),
    (0x0809, "rsa_pss_pss_sha256"),
    (0x080A, "rsa_pss_pss_sha384"),
    (0x080B, "rsa_pss_pss_sha512"),
    (0x0904, "ml_dsa_44"),
    (0x0905, "ml_dsa_65"),
    (0x0906, "ml_dsa_87"),
];

/// The signature scheme registry, by wire id.
pub const SIGNATURE_ALGORITHMS: Registry = Registry::new(SIGNATURE_ALGORITHM_ROWS);

/// Rows of the EC point format registry (RFC 8422), by wire id.
const EC_POINT_FORMAT_ROWS: &[Row] = &[
    (0x0000, "uncompressed"),
    (0x0001, "ansiX962_compressed_prime"),
    (0x0002, "ansiX962_compressed_char2"),
];

/// The EC point format registry, by wire id.
pub const EC_POINT_FORMATS: Registry = Registry::new(EC_POINT_FORMAT_ROWS);

/// Rows of the TLS protocol version registry, by wire id.
const TLS_VERSION_ROWS: &[Row] = &[
    (0x0300, "SSL 3.0"),
    (0x0301, "TLS 1.0"),
    (0x0302, "TLS 1.1"),
    (0x0303, "TLS 1.2"),
    (0x0304, "TLS 1.3"),
];

/// The TLS protocol version registry, by wire id.
pub const TLS_VERSIONS: Registry = Registry::new(TLS_VERSION_ROWS);

/// Rows of the HTTP/2 SETTINGS registry, by setting id.
const HTTP2_SETTING_ROWS: &[Row] = &[
    (0x0001, "HEADER_TABLE_SIZE"),
    (0x0002, "ENABLE_PUSH"),
    (0x0003, "MAX_CONCURRENT_STREAMS"),
    (0x0004, "INITIAL_WINDOW_SIZE"),
    (0x0005, "MAX_FRAME_SIZE"),
    (0x0006, "MAX_HEADER_LIST_SIZE"),
    (0x0008, "ENABLE_CONNECT_PROTOCOL"),
    (0x0009, "NO_RFC7540_PRIORITIES"),
];

/// The HTTP/2 SETTINGS registry, by setting id.
pub const HTTP2_SETTINGS: Registry = Registry::new(HTTP2_SETTING_ROWS);

/// HTTP/2 SETTINGS ids a measured spec may carry (plan 3.8). Any other id is refused.
pub const H2_SETTINGS_ACCEPTED: &[u16] = &[
    0x0001, 0x0002, 0x0003, 0x0004, 0x0005, 0x0006, 0x0008, 0x0009,
];

/// Cipher suites `BoringSSL` can emit in a client hello.
///
/// The TLS 1.3 trio and the ECDHE and RSA AEAD and CBC-SHA suites that Chrome offers. PSK, 3DES
/// and SHA-256/384 CBC suites are left out until the probe confirms them.
pub const BORINGSSL_CIPHERS: &[u16] = &[
    0x1301, 0x1302, 0x1303, 0xC02B, 0xC02F, 0xC02C, 0xC030, 0xCCA9, 0xCCA8, 0xC009, 0xC013, 0xC00A,
    0xC014, 0x009C, 0x009D, 0x002F, 0x0035,
];

/// The TLS 1.3 suites. A spec may reorder them but never drop one (plan 3.8).
pub const TLS13_CIPHERS: &[u16] = &[0x1301, 0x1302, 0x1303];

/// Extensions `BoringSSL` can emit in a client hello, one per `TlsOptions` knob. Padding (21) is
/// emitted only where `BoringSSL`'s length rule places it, which a round trip checks.
pub const BORINGSSL_CLIENT_EXTENSIONS: &[u16] = &[
    0x0000, 0x0005, 0x000A, 0x000B, 0x000D, 0x0010, 0x0012, 0x0015, 0x0017, 0x001B, 0x001C, 0x0022,
    0x0023, 0x002B, 0x002D, 0x0033, 0x4469, 0x44CD, 0xFE0D, 0xFF01,
];

/// Extensions `BoringSSL` puts in every client hello whatever the options say: `supported_groups`
/// and `signature_algorithms`. A spec that leaves one out describes a hello no client can send.
pub const BORINGSSL_ALWAYS_SENT_EXTENSIONS: &[u16] = &[0x000A, 0x000D];

/// Extensions `BoringSSL` sends exactly when the hello offers TLS 1.2 or older (the lowest
/// offered version is below 1.3): `ec_point_formats`, `extended_master_secret` and
/// `renegotiation_info`. A TLS 1.3-only hello has none of them.
pub const BORINGSSL_LEGACY_ONLY_EXTENSIONS: &[u16] = &[0x000B, 0x0017, 0xFF01];

/// Extensions `BoringSSL` sends exactly when the hello offers TLS 1.3 (the highest offered
/// version is 1.3): `supported_versions`, `psk_key_exchange_modes` and `key_share`.
pub const BORINGSSL_TLS13_ONLY_EXTENSIONS: &[u16] = &[0x002B, 0x002D, 0x0033];

/// Supported groups `BoringSSL` can emit: x25519, P-256, P-384, P-521, the ML-KEM and Kyber
/// hybrids, and the FFDHE groups that the probe confirms.
pub const BORINGSSL_GROUPS: &[u16] = &[
    0x001D, 0x0017, 0x0018, 0x0019, 0x0202, 0x0100, 0x0101, 0x11EC, 0x6399,
];

/// Signature schemes `BoringSSL` can emit. The internal MD5-SHA1 code `0xFF01` is not an IANA
/// scheme and never reaches the wire, so it is not listed.
pub const BORINGSSL_SIGALGS: &[u16] = &[
    0x0201, 0x0203, 0x0401, 0x0403, 0x0420, 0x0501, 0x0503, 0x0601, 0x0603, 0x0804, 0x0805, 0x0806,
    0x0807,
];

/// The TLS 1.2 DHE cipher suites, which the spec refuses (plan 3.8).
pub const DHE_CIPHERS: &[u16] = &[0x0033, 0x0039, 0x009E, 0x009F];

/// The TLS 1.3 CCM cipher suites, which the spec refuses (plan 3.8).
pub const CCM_CIPHERS: &[u16] = &[0x1304, 0x1305];

/// Signalling cipher suite values, which the spec refuses. `BoringSSL` replaces them with
/// extension 0xFF01 (plan 3.8).
pub const SCSV_CIPHERS: &[u16] = &[0x00FF, 0x5600];

/// Extensions the spec refuses, each with the reason it is refused (plan 3.8).
pub const REFUSED_EXTENSIONS: &[(u16, &str)] = &[
    (0x0029, "only usable with a resumption ticket"),
    (0x002A, "only usable with a resumption ticket"),
    (
        0x0031,
        "post-handshake authentication is not part of the client hello",
    ),
    (0x0032, "BoringSSL does not emit it"),
    (0x0016, "BoringSSL does not emit it"),
    (0x000F, "BoringSSL does not emit it"),
    (
        0xCA34,
        "needs btls newer than 0.5.6 and wreq newer than rc.31",
    ),
];

/// Supported groups the spec refuses: x448, brainpool, secp256k1 and the larger FFDHE groups.
pub const UNSUPPORTED_GROUPS: &[u16] = &[0x0016, 0x001A, 0x001B, 0x001C, 0x001E, 0x0103, 0x0104];

/// ML-DSA signature schemes, which the spec refuses (plan 3.8).
pub const ML_DSA_SIGALGS: &[u16] = &[0x0904, 0x0905, 0x0906];

#[cfg(test)]
mod tests {
    use super::{
        is_alpn, is_grease, Registry, ALPN_H2, ALPN_HTTP_1_1, BORINGSSL_CIPHERS,
        BORINGSSL_CLIENT_EXTENSIONS, BORINGSSL_GROUPS, BORINGSSL_SIGALGS, CCM_CIPHERS,
        CIPHER_SUITES, DHE_CIPHERS, EC_POINT_FORMATS, EXTENSIONS, GROUPS, H2_SETTINGS_ACCEPTED,
        HTTP2_SETTINGS, ML_DSA_SIGALGS, REFUSED_EXTENSIONS, SCSV_CIPHERS, SIGNATURE_ALGORITHMS,
        TLS13_CIPHERS, TLS_VERSIONS, UNSUPPORTED_GROUPS,
    };
    use std::collections::HashSet;

    /// Every registry with a label for failure messages.
    const REGISTRIES: [(&str, Registry); 7] = [
        ("cipher suites", CIPHER_SUITES),
        ("extensions", EXTENSIONS),
        ("groups", GROUPS),
        ("signature algorithms", SIGNATURE_ALGORITHMS),
        ("EC point formats", EC_POINT_FORMATS),
        ("TLS versions", TLS_VERSIONS),
        ("HTTP/2 settings", HTTP2_SETTINGS),
    ];

    #[test]
    fn every_registry_lists_each_id_and_name_once() {
        for (label, registry) in REGISTRIES {
            let mut ids = HashSet::new();
            let mut names = HashSet::new();
            for &(id, name) in registry.rows() {
                assert!(ids.insert(id), "{label}: duplicate id {id:#06x}");
                assert!(names.insert(name), "{label}: duplicate name {name}");
            }
        }
    }

    #[test]
    fn name_and_id_invert_each_other() {
        for (label, registry) in REGISTRIES {
            for &(id, name) in registry.rows() {
                assert_eq!(registry.name(id), Some(name), "{label}: name of {id:#06x}");
                assert_eq!(registry.id(name), Some(id), "{label}: id of {name}");
            }
        }
    }

    #[test]
    fn unlisted_values_answer_none() {
        assert_eq!(CIPHER_SUITES.name(0x0A0A), None);
        assert_eq!(CIPHER_SUITES.id("TLS_NOT_A_SUITE"), None);
        assert!(!EXTENSIONS.contains(0x1234));
    }

    #[test]
    fn known_pairs_match_the_registries() {
        let pairs: &[(Registry, u16, &str)] = &[
            (CIPHER_SUITES, 0x1301, "TLS_AES_128_GCM_SHA256"),
            (
                CIPHER_SUITES,
                0xC02B,
                "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
            ),
            (CIPHER_SUITES, 0x002F, "TLS_RSA_WITH_AES_128_CBC_SHA"),
            (EXTENSIONS, 0x0000, "server_name"),
            (EXTENSIONS, 0xFE0D, "encrypted_client_hello"),
            (EXTENSIONS, 0xFF01, "renegotiation_info"),
            (GROUPS, 0x001D, "x25519"),
            (GROUPS, 0x11EC, "X25519MLKEM768"),
            (GROUPS, 0x6399, "X25519Kyber768Draft00"),
            (SIGNATURE_ALGORITHMS, 0x0804, "rsa_pss_rsae_sha256"),
            (SIGNATURE_ALGORITHMS, 0x0904, "ml_dsa_44"),
            (EC_POINT_FORMATS, 0x0000, "uncompressed"),
            (TLS_VERSIONS, 0x0304, "TLS 1.3"),
            (HTTP2_SETTINGS, 0x0001, "HEADER_TABLE_SIZE"),
            (HTTP2_SETTINGS, 0x0009, "NO_RFC7540_PRIORITIES"),
        ];
        for &(registry, id, name) in pairs {
            assert_eq!(registry.name(id), Some(name));
            assert_eq!(registry.id(name), Some(id));
        }
    }

    #[test]
    fn grease_is_exactly_the_sixteen_doubled_nibble_values() {
        let count = (0..=u16::MAX).filter(|&id| is_grease(id)).count();
        assert_eq!(count, 16);
        for nibble in 0..16_u16 {
            let byte = (nibble << 4) | 0x0A;
            assert!(is_grease((byte << 8) | byte));
        }
        assert!(!is_grease(0x0A0B));
        assert!(!is_grease(0x1A0A));
        assert!(!is_grease(0x1301));
        assert!(!is_grease(0x0000));
    }

    #[test]
    fn alpn_names_are_the_two_protocols_this_module_names() {
        assert!(is_alpn(ALPN_H2));
        assert!(is_alpn(ALPN_HTTP_1_1));
        assert!(!is_alpn(""));
    }

    #[test]
    fn capability_seeds_are_unique_and_named() {
        let seeds: [(&str, &[u16], Registry); 5] = [
            ("BORINGSSL_CIPHERS", BORINGSSL_CIPHERS, CIPHER_SUITES),
            (
                "BORINGSSL_CLIENT_EXTENSIONS",
                BORINGSSL_CLIENT_EXTENSIONS,
                EXTENSIONS,
            ),
            ("BORINGSSL_GROUPS", BORINGSSL_GROUPS, GROUPS),
            ("BORINGSSL_SIGALGS", BORINGSSL_SIGALGS, SIGNATURE_ALGORITHMS),
            ("H2_SETTINGS_ACCEPTED", H2_SETTINGS_ACCEPTED, HTTP2_SETTINGS),
        ];
        for (label, seed, registry) in seeds {
            let unique: HashSet<u16> = seed.iter().copied().collect();
            assert_eq!(unique.len(), seed.len(), "{label} repeats an id");
            for &id in seed {
                assert!(registry.contains(id), "{label}: {id:#06x} has no IANA name");
            }
        }
        for &id in TLS13_CIPHERS {
            assert!(
                BORINGSSL_CIPHERS.contains(&id),
                "TLS 1.3 suite {id:#06x} not seeded"
            );
        }
    }

    #[test]
    fn capability_seeds_never_include_a_refused_id() {
        for &id in BORINGSSL_CIPHERS {
            assert!(!DHE_CIPHERS.contains(&id) && !CCM_CIPHERS.contains(&id));
            assert!(!SCSV_CIPHERS.contains(&id));
        }
        for &id in BORINGSSL_CLIENT_EXTENSIONS {
            assert!(!REFUSED_EXTENSIONS.iter().any(|&(refused, _)| refused == id));
        }
        for &id in BORINGSSL_GROUPS {
            assert!(!UNSUPPORTED_GROUPS.contains(&id));
        }
        for &id in BORINGSSL_SIGALGS {
            assert!(!ML_DSA_SIGALGS.contains(&id));
        }
    }

    #[test]
    fn every_refused_id_has_an_iana_name() {
        for &id in DHE_CIPHERS.iter().chain(CCM_CIPHERS).chain(SCSV_CIPHERS) {
            assert!(
                CIPHER_SUITES.contains(id),
                "refused cipher {id:#06x} is unnamed"
            );
        }
        for &(id, reason) in REFUSED_EXTENSIONS {
            assert!(
                EXTENSIONS.contains(id),
                "refused extension {id:#06x} is unnamed"
            );
            assert!(
                !reason.is_empty(),
                "refused extension {id:#06x} has no reason"
            );
        }
        for &id in UNSUPPORTED_GROUPS {
            assert!(GROUPS.contains(id), "refused group {id:#06x} is unnamed");
        }
        for &id in ML_DSA_SIGALGS {
            assert!(
                SIGNATURE_ALGORITHMS.contains(id),
                "refused sigalg {id:#06x} is unnamed"
            );
        }
    }
}
