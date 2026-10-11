//! The capability probe.
//!
//! The seed lists of `wire::ids` (`BORINGSSL_*`, `H2_SETTINGS_ACCEPTED`) say what the client can
//! send. This module checks each of them against the wire: it builds a client for every id the
//! registries name, captures the `ClientHello` or HTTP/2 start it sends, and fails when a list
//! claims an id that does not appear or leaves out one that does. The probe, not the table,
//! decides. `cargo test --features impersonate --test impersonate_wire probe -- --nocapture`
//! prints what each id did.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use seleniumbase_rs::wire::client_hello::{ext_id, without_grease, ClientHello};
use seleniumbase_rs::wire::h2_spec::{ALWAYS_SENT_SETTINGS, EMITTABLE_SETTINGS};
use seleniumbase_rs::wire::ids::{
    BORINGSSL_ALWAYS_SENT_EXTENSIONS, BORINGSSL_CIPHERS, BORINGSSL_CLIENT_EXTENSIONS,
    BORINGSSL_GROUPS, BORINGSSL_KEY_SHARE_GROUPS, BORINGSSL_LEGACY_ONLY_EXTENSIONS,
    BORINGSSL_SIGALGS, BORINGSSL_TLS13_ONLY_EXTENSIONS, CIPHER_SUITES, DHE_CIPHERS, EXTENSIONS,
    GROUPS, H2_SETTINGS_ACCEPTED, HTTP2_SETTINGS, SIGNATURE_ALGORITHMS, TLS13_CIPHERS,
};
use wreq::http2::Http2Options;
use wreq::tls::compress::{CertificateCompressionAlgorithm, CertificateCompressor, Codec};
use wreq::tls::{AlpsProtocol, ExtensionType, KeyShare, TlsOptions, TlsOptionsBuilder, TlsVersion};

use crate::exchange::{h2c_of, hello_of};

/// What a client did when asked to send one id.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    /// The id is in the hello.
    Sent,
    /// The client was built, and the id is not in the hello.
    Dropped,
    /// The client could not be built: `BoringSSL` or wreq refused the request.
    Refused(String),
}

/// Prints what each id did, for `--nocapture`.
fn report(label: &str, outcomes: &BTreeMap<u16, Outcome>) {
    for (id, outcome) in outcomes {
        eprintln!("{label} {id:#06x}: {outcome:?}");
    }
}

/// The ids that appeared on the wire.
fn sent(outcomes: &BTreeMap<u16, Outcome>) -> BTreeSet<u16> {
    outcomes
        .iter()
        .filter(|(_, outcome)| **outcome == Outcome::Sent)
        .map(|(id, _)| *id)
        .collect()
}

/// Fails when `seed` claims an id that did not appear, or leaves out one of the probed ids that
/// did. Ids that were not probed are not judged.
fn assert_seed(label: &str, seed: &[u16], outcomes: &BTreeMap<u16, Outcome>) {
    assert_seed_with_policy(label, seed, &[], outcomes);
}

/// Like [`assert_seed`], but `policy_refused` lists ids that do reach the wire and that the spec
/// layer refuses anyway. Each of them must really reach the wire: a policy exception that the
/// wire contradicts is stale.
fn assert_seed_with_policy(
    label: &str,
    seed: &[u16],
    policy_refused: &[u16],
    outcomes: &BTreeMap<u16, Outcome>,
) {
    report(label, outcomes);
    let probed: BTreeSet<u16> = outcomes.keys().copied().collect();
    let claimed: BTreeSet<u16> = seed
        .iter()
        .copied()
        .filter(|id| probed.contains(id))
        .collect();
    let policy: BTreeSet<u16> = policy_refused.iter().copied().collect();
    let on_the_wire = sent(outcomes);
    let false_claims: Vec<String> = claimed
        .difference(&on_the_wire)
        .map(|id| format!("{id:#06x}"))
        .collect();
    let missing: Vec<String> = on_the_wire
        .difference(&claimed)
        .filter(|id| !policy.contains(id))
        .map(|id| format!("{id:#06x}"))
        .collect();
    let stale: Vec<String> = policy
        .difference(&on_the_wire)
        .map(|id| format!("{id:#06x}"))
        .collect();
    assert!(
        false_claims.is_empty() && missing.is_empty() && stale.is_empty(),
        "{label}: the seed claims {false_claims:?} which the wire does not carry, leaves out \
         {missing:?} which it does, and the policy refuses {stale:?} which the wire does not carry"
    );
}

/// `min` and `max` as the options of a probe client.
fn versions(min: TlsVersion, max: TlsVersion) -> TlsOptionsBuilder {
    TlsOptions::builder()
        .min_tls_version(min)
        .max_tls_version(max)
}

/// Builds a client from the options and captures its hello.
async fn hello_with(options: TlsOptions) -> Result<ClientHello, String> {
    hello_of(|builder| builder.tls_options(options)).await
}

/// Judges one id against the hello a probe produced.
fn judge(
    hello: Result<ClientHello, String>,
    present: impl FnOnce(&ClientHello) -> bool,
) -> Outcome {
    match hello {
        Ok(hello) if present(&hello) => Outcome::Sent,
        Ok(_) => Outcome::Dropped,
        Err(reason) => Outcome::Refused(reason),
    }
}

// ===== Ciphers =====

async fn probe_cipher(id: u16, name: &str) -> Outcome {
    let tls13 = TLS13_CIPHERS.contains(&id) || id == 0x1304 || id == 0x1305;
    let options = if tls13 {
        versions(TlsVersion::TLS_1_2, TlsVersion::TLS_1_3)
            .preserve_tls13_cipher_list(true)
            .cipher_list(format!("{name}:TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"))
            .build()
    } else {
        versions(TlsVersion::TLS_1_2, TlsVersion::TLS_1_2)
            .cipher_list(name.to_owned())
            .build()
    };
    judge(hello_with(options).await, |hello| {
        hello.cipher_suites.contains(&id)
    })
}

#[tokio::test]
async fn the_cipher_seed_is_what_the_wire_carries() {
    let mut outcomes = BTreeMap::new();
    for &(id, name) in CIPHER_SUITES.rows() {
        outcomes.insert(id, probe_cipher(id, name).await);
    }
    assert_seed_with_policy("cipher", BORINGSSL_CIPHERS, DHE_CIPHERS, &outcomes);
}

// ===== Groups =====

/// The spelling `BoringSSL` parses for a group, and the key share wreq can send for it.
fn boringssl_group(id: u16) -> Option<(&'static str, Option<KeyShare>)> {
    Some(match id {
        0x001D => ("X25519", Some(KeyShare::X25519)),
        0x0017 => ("P-256", Some(KeyShare::P256)),
        0x0018 => ("P-384", Some(KeyShare::P384)),
        0x0019 => ("P-521", Some(KeyShare::P521)),
        0x11EC => ("X25519MLKEM768", Some(KeyShare::X25519_MLKEM768)),
        0x6399 => (
            "X25519Kyber768Draft00",
            Some(KeyShare::X25519_KYBER768_DRAFT00),
        ),
        0x0202 => ("MLKEM1024", Some(KeyShare::MLKEM1024)),
        0xFE32 => ("P256Kyber768Draft00", Some(KeyShare::P256_KYBER768_DRAFT00)),
        0x0100 => ("ffdhe2048", Some(KeyShare::FFDHE2048)),
        0x0101 => ("ffdhe3072", Some(KeyShare::FFDHE3072)),
        _ => return None,
    })
}

/// Asks for one group next to x25519, so the hello has a group to send a key share for.
///
/// With `share` the hello must also carry a key share for the group; without it, only the
/// `supported_groups` entry is judged, and the key share is x25519's.
async fn probe_group(id: u16, name: &str, share: Option<KeyShare>) -> Outcome {
    let companion = id != 0x001D;
    let curves = if companion {
        format!("X25519:{name}")
    } else {
        name.to_owned()
    };
    let mut shares = vec![KeyShare::X25519];
    if let (true, Some(share)) = (companion, share) {
        shares.push(share);
    }
    let builder = versions(TlsVersion::TLS_1_2, TlsVersion::TLS_1_3)
        .curves_list(curves)
        .key_shares(shares);
    judge(hello_with(builder.build()).await, |hello| {
        let offered = without_grease(&hello.supported_groups).contains(&id);
        offered && (share.is_none() || without_grease(&hello.key_share_groups).contains(&id))
    })
}

/// Every group the registry names, and the one `BoringSSL` adds that the registry lacks.
fn group_candidates() -> BTreeSet<u16> {
    GROUPS
        .rows()
        .iter()
        .map(|&(id, _)| id)
        .chain([0xFE32])
        .collect()
}

#[tokio::test]
async fn the_group_seed_is_what_the_wire_carries() {
    let mut outcomes = BTreeMap::new();
    for id in group_candidates() {
        let name =
            boringssl_group(id).map_or_else(|| GROUPS.name(id).unwrap_or("unnamed"), |g| g.0);
        outcomes.insert(id, probe_group(id, name, None).await);
    }
    assert_seed("group", BORINGSSL_GROUPS, &outcomes);
}

#[tokio::test]
async fn the_key_share_seed_is_what_the_wire_carries() {
    let mut outcomes = BTreeMap::new();
    for id in group_candidates() {
        let outcome = match boringssl_group(id) {
            Some((name, Some(share))) => probe_group(id, name, Some(share)).await,
            _ => Outcome::Dropped,
        };
        outcomes.insert(id, outcome);
    }
    assert_seed("key share", BORINGSSL_KEY_SHARE_GROUPS, &outcomes);
}

// ===== Signature algorithms =====

async fn probe_sigalg(id: u16, name: &str) -> Outcome {
    let options = versions(TlsVersion::TLS_1_2, TlsVersion::TLS_1_3)
        .sigalgs_list(name.to_owned())
        .build();
    judge(hello_with(options).await, |hello| {
        without_grease(&hello.signature_algorithms).contains(&id)
    })
}

#[tokio::test]
async fn the_signature_algorithm_seed_is_what_the_wire_carries() {
    let mut outcomes = BTreeMap::new();
    for &(id, name) in SIGNATURE_ALGORITHMS.rows() {
        outcomes.insert(id, probe_sigalg(id, name).await);
    }
    assert_seed("sigalg", BORINGSSL_SIGALGS, &outcomes);
}

// ===== Extensions =====

/// A certificate compressor that is never run: the probe reads the hello and stops.
#[derive(Debug)]
struct Idle;

impl CertificateCompressor for Idle {
    fn compress(&self) -> Codec {
        Codec::Pointer(copy)
    }

    fn decompress(&self) -> Codec {
        Codec::Pointer(copy)
    }

    fn algorithm(&self) -> CertificateCompressionAlgorithm {
        CertificateCompressionAlgorithm::BROTLI
    }
}

static IDLE: Idle = Idle;

fn copy(input: &[u8], output: &mut dyn Write) -> std::io::Result<()> {
    output.write_all(input)
}

/// Options that ask for one extension, when an option for it exists.
fn options_asking_for(id: u16) -> Option<TlsOptions> {
    let both = versions(TlsVersion::TLS_1_2, TlsVersion::TLS_1_3);
    let builder = match id {
        ext_id::SERVER_NAME
        | ext_id::SUPPORTED_GROUPS
        | ext_id::EC_POINT_FORMATS
        | ext_id::SIGNATURE_ALGORITHMS
        | ext_id::ALPN
        | ext_id::EXTENDED_MASTER_SECRET
        | ext_id::SESSION_TICKET
        | ext_id::SUPPORTED_VERSIONS
        | ext_id::PSK_KEY_EXCHANGE_MODES
        | ext_id::KEY_SHARE
        | ext_id::RENEGOTIATION_INFO => both,
        ext_id::STATUS_REQUEST => both.enable_ocsp_stapling(true),
        ext_id::SIGNED_CERT_TIMESTAMP => both.enable_signed_cert_timestamps(true),
        ext_id::COMPRESS_CERTIFICATE => {
            let compressors: Vec<&'static dyn CertificateCompressor> = vec![&IDLE];
            both.certificate_compressors(Cow::Owned(compressors))
        }
        ext_id::RECORD_SIZE_LIMIT => both.record_size_limit(16_384),
        ext_id::DELEGATED_CREDENTIAL => both.delegated_credentials("ecdsa_secp256r1_sha256"),
        ext_id::ALPS => both
            .alps_protocols([AlpsProtocol::HTTP2])
            .alps_use_new_codepoint(true),
        ext_id::ALPS_OLD => both
            .alps_protocols([AlpsProtocol::HTTP2])
            .alps_use_new_codepoint(false),
        ext_id::ENCRYPTED_CLIENT_HELLO => both.enable_ech_grease(true),
        ext_id::PADDING => padding_options(),
        _ => return None,
    };
    Some(builder.build())
}

/// Every extension `kExtensions` of the vendored `BoringSSL` handles, from its source.
const HANDLERS: [u16; 30] = [
    0x0000, 0x0005, 0x000A, 0x000B, 0x000D, 0x000E, 0x0010, 0x0012, 0x0017, 0x001B, 0x001C, 0x0022,
    0x0023, 0x0029, 0x002A, 0x002B, 0x002C, 0x002D, 0x002F, 0x0033, 0x0039, 0x3374, 0x4469, 0x44CD,
    0x7550, 0x8A3B, 0xCA34, 0xFE0D, 0xFF01, 0xFFA5,
];

/// A TLS 1.2 hello without GREASE whose last handler writes an empty extension: the case in which
/// `BoringSSL` appends the padding extension. The order names every handler, with
/// `extended_master_secret` (empty body) visited last.
fn padding_options() -> TlsOptionsBuilder {
    let order: Vec<ExtensionType> = HANDLERS
        .into_iter()
        .filter(|id| *id != ext_id::EXTENDED_MASTER_SECRET)
        .chain([ext_id::EXTENDED_MASTER_SECRET])
        .map(ExtensionType::from)
        .collect();
    versions(TlsVersion::TLS_1_2, TlsVersion::TLS_1_2)
        .grease_enabled(false)
        .permute_extensions(false)
        .extension_permutation(order)
}

async fn probe_extension(id: u16) -> Outcome {
    let Some(options) = options_asking_for(id) else {
        // No option names this extension. Listing it in the extension order must not make it
        // appear either.
        let order = vec![ExtensionType::from(id)];
        let options = versions(TlsVersion::TLS_1_2, TlsVersion::TLS_1_3)
            .permute_extensions(false)
            .extension_permutation(order)
            .build();
        return judge(hello_with(options).await, |hello| hello.has_extension(id));
    };
    judge(hello_with(options).await, |hello| hello.has_extension(id))
}

#[tokio::test]
async fn the_extension_seed_is_what_the_wire_carries() {
    let mut outcomes = BTreeMap::new();
    for &(id, _) in EXTENSIONS.rows() {
        outcomes.insert(id, probe_extension(id).await);
    }
    assert_seed("extension", BORINGSSL_CLIENT_EXTENSIONS, &outcomes);
}

#[tokio::test]
async fn padding_is_added_by_boringssls_own_rule_and_not_on_request() {
    let hello = hello_with(padding_options().build())
        .await
        .expect("a hello");
    let order = hello.extension_ids();
    assert_eq!(
        order.last(),
        Some(&ext_id::PADDING),
        "an empty last extension makes BoringSSL append padding: {order:?}"
    );
    let chrome_like = hello_with(versions(TlsVersion::TLS_1_2, TlsVersion::TLS_1_3).build())
        .await
        .expect("a hello");
    assert!(!chrome_like.has_extension(ext_id::PADDING));
}

/// What a hello offers, by extension, for one range of protocol versions.
async fn extensions_for(min: TlsVersion, max: TlsVersion) -> BTreeSet<u16> {
    let options = versions(min, max)
        .alps_protocols([AlpsProtocol::HTTP2])
        .alps_use_new_codepoint(true)
        .enable_ech_grease(true)
        .build();
    let hello = hello_with(options).await.expect("a hello");
    hello.extension_ids().into_iter().collect()
}

#[tokio::test]
async fn the_version_rules_of_the_extensions_hold_on_the_wire() {
    let tls12 = extensions_for(TlsVersion::TLS_1_2, TlsVersion::TLS_1_2).await;
    let both = extensions_for(TlsVersion::TLS_1_2, TlsVersion::TLS_1_3).await;
    let tls13 = extensions_for(TlsVersion::TLS_1_3, TlsVersion::TLS_1_3).await;
    eprintln!("TLS 1.2 only: {tls12:x?}\nTLS 1.2 to 1.3: {both:x?}\nTLS 1.3 only: {tls13:x?}");
    for &id in BORINGSSL_ALWAYS_SENT_EXTENSIONS {
        for (label, set) in [("1.2", &tls12), ("1.2-1.3", &both), ("1.3", &tls13)] {
            assert!(
                set.contains(&id),
                "{id:#06x} is missing from a TLS {label} hello"
            );
        }
    }
    for &id in BORINGSSL_LEGACY_ONLY_EXTENSIONS {
        assert!(tls12.contains(&id) && both.contains(&id), "{id:#06x}");
        assert!(!tls13.contains(&id), "{id:#06x} is in a TLS 1.3-only hello");
    }
    for &id in BORINGSSL_TLS13_ONLY_EXTENSIONS {
        assert!(both.contains(&id) && tls13.contains(&id), "{id:#06x}");
        assert!(!tls12.contains(&id), "{id:#06x} is in a TLS 1.2-only hello");
    }
    assert!(!tls12.contains(&ext_id::ALPS), "ALPS needs TLS 1.3");
    assert!(
        !tls13.contains(&ext_id::SESSION_TICKET),
        "the session ticket extension is TLS 1.2 only"
    );
}

#[tokio::test]
async fn the_tls13_suites_always_come_first() {
    let options = versions(TlsVersion::TLS_1_2, TlsVersion::TLS_1_3)
        .preserve_tls13_cipher_list(true)
        .cipher_list(
            "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:\
             TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384:TLS_AES_128_GCM_SHA256"
                .to_owned(),
        )
        .build();
    let hello = hello_with(options).await.expect("a hello");
    let suites = without_grease(&hello.cipher_suites);
    assert_eq!(
        suites,
        [0x1302, 0x1301, 0xC02F, 0xC030],
        "TLS 1.3 suites first, each group in the order asked for"
    );
}

// ===== HTTP/2 =====

/// The value each SETTINGS id is probed with: distinct from every default.
fn probe_value(id: u16) -> u32 {
    match id {
        1 => 8_192,
        2 | 8 | 9 => 1,
        3 => 77,
        4 => 1_000_000,
        5 => 32_768,
        _ => 65_537,
    }
}

fn http2_with(id: u16, value: u32) -> Http2Options {
    let builder = Http2Options::builder();
    match id {
        1 => builder.header_table_size(value),
        2 => builder.enable_push(value != 0),
        3 => builder.max_concurrent_streams(value),
        4 => builder.initial_window_size(value),
        5 => builder.max_frame_size(value),
        6 => builder.max_header_list_size(value),
        8 => builder.enable_connect_protocol(value != 0),
        _ => builder.no_rfc7540_priorities(value != 0),
    }
    .build()
}

#[tokio::test]
async fn the_settings_wreq_can_send_are_the_ones_the_spec_accepts() {
    let mut outcomes = BTreeMap::new();
    for &(id, _) in HTTP2_SETTINGS.rows() {
        let value = probe_value(id);
        let outcome = match h2c_of(|builder| builder.http2_options(http2_with(id, value))).await {
            Ok(captured) if captured.preface.settings.contains(&(id, value)) => Outcome::Sent,
            Ok(_) => Outcome::Dropped,
            Err(reason) => Outcome::Refused(reason),
        };
        outcomes.insert(id, outcome);
    }
    assert_seed("setting", H2_SETTINGS_ACCEPTED, &outcomes);
    assert_seed("setting", &EMITTABLE_SETTINGS, &outcomes);
}

#[tokio::test]
async fn the_settings_wreq_always_sends_are_the_ones_the_spec_knows() {
    let captured = h2c_of(|builder| builder.http2_options(Http2Options::builder().build()))
        .await
        .expect("a connection start");
    let mut always: Vec<u16> = captured
        .preface
        .settings
        .iter()
        .map(|(id, _)| *id)
        .collect();
    always.sort_unstable();
    assert_eq!(
        always, ALWAYS_SENT_SETTINGS,
        "{:?}",
        captured.preface.settings
    );
}

#[tokio::test]
async fn a_connection_window_of_65535_sends_no_window_update() {
    let options = Http2Options::builder()
        .initial_connection_window_size(65_535)
        .build();
    let captured = h2c_of(|builder| builder.http2_options(options))
        .await
        .expect("a connection start");
    assert_eq!(captured.preface.window_update, 0);
}

#[tokio::test]
async fn bare_wreq_offers_h2_then_http11() {
    let hello = hello_with(TlsOptions::default()).await.expect("a hello");
    assert_eq!(hello.alpn, [b"h2".to_vec(), b"http/1.1".to_vec()]);
}
