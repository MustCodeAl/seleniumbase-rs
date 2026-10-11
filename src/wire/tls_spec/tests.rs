//! Tests of `TlsSpec`: building it from a hello, the reproducibility check and the three overlays.

use super::{Alps, SpecError, TlsSpec};
use crate::wire::client_hello::fixtures::{
    HelloBuilder, CHROME_CIPHERS, CHROME_SIGALGS, JA4_EXAMPLE_EXTENSIONS,
};
use crate::wire::client_hello::{ext_id, ClientHello};
use crate::wire::ids::REFUSED_EXTENSIONS;
use crate::wire::ja3::{Ja3, Peetprint};
use crate::wire::ja4::{Ja4, Ja4Order};

/// The cipher list of the published Chrome style hello, as JA3 writes it.
pub(super) const CIPHERS: &str =
    "4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53";
/// The widely published JA3 of a Chrome 85 style hello and its MD5.
pub(super) const CHROME_JA3: &str = "771,4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,0-23-65281-10-11-35-16-5-13-18-51-45-43-27-21,29-23-24,0";
pub(super) const CHROME_JA3_HASH: &str = "b32309a26951912be7dba376398abc3b";

pub(super) fn hello(builder: &HelloBuilder) -> ClientHello {
    ClientHello::parse(&builder.record()).expect("the fixture hello parses")
}

/// The spec of the published example hello, GREASE included.
pub(super) fn chrome() -> TlsSpec {
    TlsSpec::from_client_hello(&hello(&HelloBuilder::chrome_like().with_grease()))
}

pub(super) fn ja3(extensions: &str, groups: &str, point_formats: &str) -> Ja3 {
    Ja3::parse(&format!(
        "771,{CIPHERS},{extensions},{groups},{point_formats}"
    ))
    .expect("the test JA3 parses")
}

#[test]
fn builds_a_spec_from_a_hello() {
    let spec = chrome();
    assert_eq!(spec.legacy_version, 0x0303);
    assert_eq!(spec.cipher_suites, CHROME_CIPHERS);
    assert_eq!(spec.extensions, JA4_EXAMPLE_EXTENSIONS);
    assert!(spec.grease && !spec.permute_extensions);
    assert_eq!(spec.supported_groups, [0x001D, 0x0017, 0x0018]);
    assert_eq!(spec.key_share_groups, [0x001D]);
    assert_eq!(spec.point_formats, [0]);
    assert_eq!(spec.signature_algorithms, CHROME_SIGALGS);
    assert_eq!(spec.alpn, ["h2", "http/1.1"]);
    let alps = Alps {
        protocols: vec!["h2".to_owned()],
        new_codepoint: false,
    };
    assert_eq!(spec.alps, Some(alps));
    assert_eq!(spec.supported_versions, [0x0304, 0x0303]);
    assert_eq!(
        (spec.psk_modes.as_slice(), spec.cert_compression.as_slice()),
        (&[1_u8][..], &[2_u16][..])
    );
    assert!(spec.record_size_limit.is_none() && spec.delegated_credentials.is_empty());
    assert!(spec.ocsp_stapling() && spec.sct() && spec.session_ticket());
    assert!(spec.renegotiation() && spec.extended_master_secret() && !spec.ech_grease());
    assert_eq!(spec.check_reproducible(), Ok(()));
}

#[test]
fn a_hello_without_grease_gives_a_spec_without_the_flag() {
    let spec = TlsSpec::from_client_hello(&hello(&HelloBuilder::chrome_like()));
    assert!(!spec.grease);
    assert_eq!(spec.cipher_suites, CHROME_CIPHERS);
}

#[test]
fn the_new_codepoint_and_optional_extensions_are_read() {
    let builder = HelloBuilder::new(
        &CHROME_CIPHERS,
        &[
            ext_id::ALPS,
            ext_id::RECORD_SIZE_LIMIT,
            ext_id::DELEGATED_CREDENTIAL,
            ext_id::ENCRYPTED_CLIENT_HELLO,
        ],
    );
    let spec = TlsSpec::from_client_hello(&hello(&builder));
    assert_eq!(
        spec.alps.as_ref().map(|alps| alps.new_codepoint),
        Some(true)
    );
    assert_eq!(spec.record_size_limit, Some(0x4001));
    assert_eq!(spec.delegated_credentials, [0x0403]);
    assert!(spec.ech_grease());
}

#[test]
fn the_spec_reproduces_the_fingerprints_of_its_hello() {
    for builder in [
        HelloBuilder::chrome_like(),
        HelloBuilder::chrome_like().with_grease(),
    ] {
        let parsed = hello(&builder);
        let spec = TlsSpec::from_client_hello(&parsed);
        assert_eq!(spec.ja3(), Ja3::of(&parsed));
        assert_eq!(spec.ja4(), Ja4::of(&parsed));
        assert_eq!(
            spec.ja4_raw(Ja4Order::Sorted).to_string(),
            Ja4::raw(&parsed)
        );
        assert_eq!(
            spec.ja4_raw(Ja4Order::Original).to_string(),
            Ja4::raw_original_order(&parsed)
        );
        assert_eq!(
            spec.ja4_facts().ja4(Ja4Order::Original),
            Ja4::of_original_order(&parsed)
        );
    }
    let greased = hello(&HelloBuilder::chrome_like().with_grease());
    assert_eq!(chrome().peetprint(), Peetprint::of(&greased));
    let plain = hello(&HelloBuilder::chrome_like());
    let spec = TlsSpec::from_client_hello(&plain);
    assert_eq!(spec.peetprint(), Peetprint::of(&plain));
}

#[test]
fn the_spec_round_trips_through_json_and_defaults_missing_content() {
    let spec = chrome();
    let json = serde_json::to_string(&spec).expect("serializes");
    assert_eq!(
        serde_json::from_str::<TlsSpec>(&json).expect("deserializes"),
        spec
    );
    let minimal: TlsSpec =
        serde_json::from_str(r#"{"legacy_version":771,"cipher_suites":[47],"extensions":[]}"#)
            .expect("optional content defaults");
    assert!(!minimal.grease && minimal.alpn.is_empty() && minimal.alps.is_none());
}

pub(super) fn refused(id: u16) -> SpecError {
    let reason = REFUSED_EXTENSIONS
        .iter()
        .find(|&&(refused, _)| refused == id)
        .map(|&(_, reason)| reason)
        .expect("the id is in the refused table");
    SpecError::UnsupportedExtension { id, reason }
}

pub(super) fn duplicate(list: &'static str, id: u16) -> SpecError {
    SpecError::Duplicate { list, id }
}

pub(super) fn sigalg(id: u16, name: &'static str) -> SpecError {
    SpecError::UnsupportedSigalg { id, name }
}

type Mutation = fn(&mut TlsSpec);

#[test]
fn rejects_what_boringssl_cannot_emit_with_a_named_error() {
    let cases: Vec<(&str, Mutation, SpecError)> = vec![
        (
            "TLS 1.3 as the legacy version",
            |s| s.legacy_version = 0x0304,
            SpecError::LegacyVersion(0x0304),
        ),
        (
            "SSL 3.0 offered",
            |s| s.supported_versions.push(0x0300),
            SpecError::UnsupportedVersion(0x0300),
        ),
        (
            "repeated protocol version",
            |s| s.supported_versions.push(0x0304),
            duplicate("supported_versions", 0x0304),
        ),
        (
            "repeated key share group",
            |s| s.key_share_groups.push(0x001D),
            duplicate("key_share_groups", 0x001D),
        ),
        (
            "no ciphers",
            |s| s.cipher_suites.clear(),
            SpecError::NoCipherSuites,
        ),
        (
            "DHE",
            |s| s.cipher_suites.push(0x0033),
            SpecError::UnsupportedCipher(0x0033),
        ),
        (
            "CCM",
            |s| s.cipher_suites.push(0x1304),
            SpecError::UnsupportedCipher(0x1304),
        ),
        (
            "SCSV",
            |s| s.cipher_suites.push(0x00FF),
            SpecError::UnsupportedCipher(0x00FF),
        ),
        (
            "RC4",
            |s| s.cipher_suites.push(0x0005),
            SpecError::UnsupportedCipher(0x0005),
        ),
        (
            "unknown cipher",
            |s| s.cipher_suites.push(0xBEEF),
            SpecError::UnsupportedCipher(0xBEEF),
        ),
        (
            "repeated cipher",
            |s| s.cipher_suites.push(0x1301),
            duplicate("cipher_suites", 0x1301),
        ),
        (
            "TLS 1.3 subset",
            |s| s.cipher_suites.retain(|&c| c != 0x1302),
            SpecError::Tls13Suites,
        ),
        (
            "TLS 1.3 without suites",
            |s| s.cipher_suites.retain(|&c| c >> 8 != 0x13),
            SpecError::Tls13Suites,
        ),
        (
            "TLS 1.3 suites for a 1.2 client",
            |s| s.supported_versions = vec![0x0303],
            SpecError::Tls13Suites,
        ),
        (
            "pre_shared_key",
            |s| s.extensions.push(0x0029),
            refused(0x0029),
        ),
        (
            "trust_anchors",
            |s| s.extensions.push(0xCA34),
            refused(0xCA34),
        ),
        (
            "repeated extension",
            |s| s.extensions.push(0x0000),
            SpecError::DuplicateExtension(0),
        ),
        (
            "x448",
            |s| s.supported_groups.push(0x001E),
            SpecError::UnsupportedGroup(0x001E),
        ),
        (
            "ffdhe6144",
            |s| s.supported_groups.push(0x0103),
            SpecError::UnsupportedGroup(0x0103),
        ),
        (
            "unknown group",
            |s| s.supported_groups.push(0xFFFF),
            SpecError::UnsupportedGroup(0xFFFF),
        ),
        (
            "repeated group",
            |s| s.supported_groups.push(0x001D),
            duplicate("supported_groups", 0x001D),
        ),
        (
            "key share for a group not offered",
            |s| s.key_share_groups = vec![0x0019],
            SpecError::KeyShareNotOffered(0x0019),
        ),
        (
            "compressed point format",
            |s| s.point_formats = vec![1],
            SpecError::PointFormats(vec![1]),
        ),
        (
            "point formats dropped",
            |s| s.point_formats.clear(),
            SpecError::PointFormats(vec![]),
        ),
        (
            "point formats without the extension",
            |s| s.extensions.retain(|&e| e != 0x000B),
            SpecError::PointFormats(vec![0]),
        ),
        (
            "ML-DSA",
            |s| s.signature_algorithms.push(0x0904),
            sigalg(0x0904, "ml_dsa_44"),
        ),
        (
            "ed448",
            |s| s.signature_algorithms.push(0x0808),
            sigalg(0x0808, "ed448"),
        ),
        (
            "unassigned sigalg",
            |s| s.signature_algorithms.push(0xABCD),
            sigalg(0xABCD, "unassigned"),
        ),
        (
            "ML-DSA delegated credential",
            |s| s.delegated_credentials = vec![0x0905],
            sigalg(0x0905, "ml_dsa_65"),
        ),
        (
            "repeated sigalg",
            |s| s.signature_algorithms.push(0x0403),
            duplicate("signature_algorithms", 0x0403),
        ),
        (
            "empty ALPN name",
            |s| s.alpn[0].clear(),
            SpecError::Alpn(String::new()),
        ),
        (
            "oversized ALPN name",
            |s| s.alpn[0] = "x".repeat(256),
            SpecError::Alpn("x".repeat(256)),
        ),
    ];
    for (label, mutate, want) in cases {
        let mut spec = chrome();
        mutate(&mut spec);
        assert_eq!(spec.check_reproducible(), Err(want), "{label}");
    }
}

#[test]
fn rejects_unknown_and_grease_extension_ids() {
    for id in [0x9999, 0x0A0A, 0x0041] {
        let mut spec = chrome();
        spec.extensions.push(id);
        assert!(
            matches!(spec.check_reproducible(), Err(SpecError::UnsupportedExtension { id: bad, .. }) if bad == id),
            "{id:#06x}"
        );
    }
}

#[test]
fn rejects_an_extension_and_its_content_that_disagree() {
    let cases: Vec<(&str, Mutation, u16)> = vec![
        (
            "groups",
            |s| {
                s.supported_groups.clear();
                s.key_share_groups.clear();
            },
            0x000A,
        ),
        (
            "signature algorithms",
            |s| s.signature_algorithms.clear(),
            0x000D,
        ),
        ("ALPN", |s| s.alpn.clear(), 0x0010),
        ("key share", |s| s.key_share_groups.clear(), 0x0033),
        ("PSK modes", |s| s.psk_modes.clear(), 0x002D),
        (
            "certificate compression",
            |s| s.cert_compression.clear(),
            0x001B,
        ),
        (
            "record size limit without its extension",
            |s| s.record_size_limit = Some(1),
            0x001C,
        ),
        (
            "delegated credentials without their extension",
            |s| s.delegated_credentials = vec![0x0403],
            0x0022,
        ),
        ("ALPS content dropped", |s| s.alps = None, 0x4469),
        (
            "ALPS on the other codepoint",
            |s| {
                if let Some(alps) = s.alps.as_mut() {
                    alps.new_codepoint = true
                }
            },
            0x44CD,
        ),
        (
            "ALPS without protocols",
            |s| {
                if let Some(alps) = s.alps.as_mut() {
                    alps.protocols.clear()
                }
            },
            0x4469,
        ),
    ];
    for (label, mutate, id) in cases {
        let mut spec = chrome();
        mutate(&mut spec);
        assert_eq!(
            spec.check_reproducible(),
            Err(SpecError::ExtensionContent(id)),
            "{label}"
        );
    }
}
