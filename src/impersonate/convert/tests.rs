//! Unit tests of the spec-to-wreq conversion. None of them opens a socket: the proof that the
//! options reach the wire is `tests/impersonate_wire.rs`.

use wreq::http2::{
    Priorities, Priority, PseudoId, PseudoOrder, SettingId, SettingsOrder, StreamDependency,
    StreamId,
};
use wreq::tls::compress::Codec;
use wreq::tls::{AlpnProtocol, AlpsProtocol, ExtensionType, KeyShare, TlsVersion};

use super::{
    check_emittable, compressor, http2_options, tls_options, wreq_headers, ConvertError,
    CERT_COMPRESSION_BROTLI, CERT_COMPRESSION_ZLIB, MAX_DECOMPRESSED_CERTIFICATE,
};
use crate::stealth::fingerprint::OsType;
use crate::wire::akamai::Akamai;
use crate::wire::client_hello::ext_id;
use crate::wire::emulation::{self, Emulation};
use crate::wire::h2_spec::H2Spec;
use crate::wire::ids::{CIPHER_SUITES, SIGNATURE_ALGORITHMS};
use crate::wire::tls_spec::TlsSpec;
use crate::wire::BrowserKind;

/// The names `BoringSSL` gives the groups the measured Chrome offers.
const BORINGSSL_GROUP_NAMES: [(u16, &str); 4] = [
    (0x11EC, "X25519MLKEM768"),
    (0x001D, "X25519"),
    (0x0017, "P-256"),
    (0x0018, "P-384"),
];

fn chrome() -> &'static Emulation {
    emulation::get(BrowserKind::Chrome, OsType::Macos).expect("Chrome on macOS is measured")
}

/// The measured TLS spec with the parts `BoringSSL` cannot send taken out.
fn effective_tls() -> TlsSpec {
    let chrome = chrome();
    chrome.tls.without_known_gaps(&chrome.provenance.known_gaps)
}

fn measured_h2() -> H2Spec {
    chrome().http2.clone().expect("the HTTP/2 capture exists")
}

/// A TLS 1.2-only spec derived from the measured one: the extensions that need TLS 1.3 are gone.
fn tls12_only() -> TlsSpec {
    let mut spec = effective_tls();
    let tls13 = [
        ext_id::SUPPORTED_VERSIONS,
        ext_id::PSK_KEY_EXCHANGE_MODES,
        ext_id::KEY_SHARE,
        ext_id::ALPS,
        ext_id::ENCRYPTED_CLIENT_HELLO,
    ];
    spec.extensions.retain(|id| !tls13.contains(id));
    spec.supported_versions.clear();
    spec.psk_modes.clear();
    spec.key_share_groups.clear();
    spec.alps = None;
    spec.cipher_suites
        .retain(|id| !(0x1301..=0x1303).contains(id));
    spec.supported_groups.retain(|id| *id != 0x11EC);
    spec
}

#[test]
fn the_measured_spec_passes_every_check() {
    let spec = effective_tls();
    assert_eq!(spec.check_reproducible(), Ok(()));
    assert_eq!(check_emittable(&spec), Ok(()));
    assert_eq!(measured_h2().check_reproducible(), Ok(()));
}

#[test]
fn the_cipher_list_names_every_suite_in_order_and_round_trips() {
    let spec = effective_tls();
    let options = tls_options(&spec).expect("converts");
    let list = options.cipher_list.expect("a cipher list is set");
    let ids: Vec<u16> = list
        .split(':')
        .map(|name| CIPHER_SUITES.id(name).expect("an IANA name"))
        .collect();
    assert_eq!(ids, spec.cipher_suites);
    assert_eq!(options.preserve_tls13_cipher_list, Some(true));
}

#[test]
fn the_signature_algorithms_and_groups_round_trip() {
    let spec = effective_tls();
    let options = tls_options(&spec).expect("converts");
    let sigalgs: Vec<u16> = options
        .sigalgs_list
        .expect("a list is set")
        .split(':')
        .map(|name| SIGNATURE_ALGORITHMS.id(name).expect("an IANA name"))
        .collect();
    assert_eq!(sigalgs, spec.signature_algorithms);
    let curves = options.curves_list.expect("a list is set");
    let groups: Vec<u16> = curves
        .split(':')
        .map(|name| {
            BORINGSSL_GROUP_NAMES
                .iter()
                .find(|(_, known)| *known == name)
                .map(|(id, _)| *id)
                .expect("a BoringSSL group name")
        })
        .collect();
    assert_eq!(groups, spec.supported_groups);
}

#[test]
fn every_switch_follows_the_spec() {
    let spec = effective_tls();
    let options = tls_options(&spec).expect("converts");
    assert_eq!(options.grease_enabled, Some(spec.grease));
    assert_eq!(options.permute_extensions, Some(spec.permute_extensions));
    assert!(options.enable_ocsp_stapling && spec.ocsp_stapling());
    assert!(options.enable_signed_cert_timestamps && spec.sct());
    assert!(options.enable_ech_grease && spec.ech_grease());
    assert!(options.session_ticket && spec.session_ticket());
    assert_eq!(options.record_size_limit, spec.record_size_limit);
    assert_eq!(
        options.alpn_protocols.as_deref(),
        Some(&[AlpnProtocol::HTTP2, AlpnProtocol::HTTP1][..])
    );
    assert_eq!(
        options.alps_protocols.as_deref(),
        Some(&[AlpsProtocol::HTTP2][..])
    );
    assert!(options.alps_use_new_codepoint);
    assert_eq!(options.min_tls_version, Some(TlsVersion::TLS_1_2));
    assert_eq!(options.max_tls_version, Some(TlsVersion::TLS_1_3));
    assert_eq!(
        options.key_shares.as_deref(),
        Some(&[KeyShare::X25519_MLKEM768, KeyShare::X25519][..])
    );
    assert_eq!(
        options.certificate_compressors.map(|list| list.len()),
        Some(1)
    );
    assert!(options.delegated_credentials.is_none());
}

#[test]
fn a_permuting_spec_leaves_the_order_to_boringssl() {
    let options = tls_options(&effective_tls()).expect("converts");
    assert!(options.extension_permutation.is_none());
}

#[test]
fn a_spec_that_does_not_permute_pins_its_extension_order_after_the_unlisted_handlers() {
    let mut spec = effective_tls();
    spec.permute_extensions = false;
    let options = tls_options(&spec).expect("converts");
    let wanted: Vec<ExtensionType> = spec.extensions.iter().map(|&id| id.into()).collect();
    assert_eq!(options.permute_extensions, Some(false));
    let pinned = options.extension_permutation.expect("an order is pinned");
    assert!(
        pinned.len() > wanted.len(),
        "the unlisted handlers come first"
    );
    assert!(
        pinned.ends_with(&wanted),
        "the spec's own order comes last, unchanged"
    );
}

/// A spec that does not permute, with its extensions ending in `last`.
fn pinned_ending_in(last: u16) -> TlsSpec {
    let mut spec = effective_tls();
    spec.permute_extensions = false;
    spec.extensions.retain(|id| *id != last);
    spec.extensions.push(last);
    spec
}

#[test]
fn padding_follows_an_empty_last_extension_in_a_hello_without_grease() {
    let mut spec = pinned_ending_in(ext_id::EXTENDED_MASTER_SECRET);
    spec.grease = false;
    assert_eq!(check_emittable(&spec), Err(ConvertError::PaddingAppended));
    spec.extensions.push(ext_id::PADDING);
    assert_eq!(
        check_emittable(&spec),
        Ok(()),
        "padding listed after the empty extension"
    );
    spec.grease = true;
    assert_eq!(
        check_emittable(&spec),
        Err(ConvertError::PaddingUnavailable),
        "GREASE ends the hello with a non-empty extension, so no padding follows"
    );
}

#[test]
fn padding_cannot_be_asked_for_after_a_non_empty_extension() {
    let mut spec = pinned_ending_in(ext_id::ALPN);
    spec.grease = false;
    assert_eq!(check_emittable(&spec), Ok(()));
    spec.extensions.push(ext_id::PADDING);
    assert_eq!(
        check_emittable(&spec),
        Err(ConvertError::PaddingUnavailable)
    );
}

#[test]
fn a_permuting_spec_is_not_judged_for_padding() {
    let mut spec = effective_tls();
    spec.grease = false;
    spec.extensions.push(ext_id::PADDING);
    assert!(spec.permute_extensions);
    assert_eq!(check_emittable(&spec), Ok(()));
}

#[test]
fn a_tls12_only_spec_caps_the_version_and_sends_no_tls13_key_shares() {
    let spec = tls12_only();
    assert_eq!(spec.check_reproducible(), Ok(()));
    let options = tls_options(&spec).expect("converts");
    assert_eq!(options.max_tls_version, Some(TlsVersion::TLS_1_2));
    assert_eq!(options.min_tls_version, Some(TlsVersion::TLS_1_2));
    assert_eq!(options.key_shares.as_deref(), Some(&[][..]));
    assert!(options.alps_protocols.is_none());
}

#[test]
fn unsupported_alpn_and_alps_names_are_refused() {
    let mut spec = effective_tls();
    spec.alpn = vec!["h3".to_owned()];
    assert_eq!(
        tls_options(&spec).err(),
        Some(ConvertError::UnsupportedAlpn("h3".to_owned()))
    );
    let mut spec = effective_tls();
    spec.alps.as_mut().expect("Chrome sends ALPS").protocols = vec!["spdy/3".to_owned()];
    assert_eq!(
        tls_options(&spec).err(),
        Some(ConvertError::UnsupportedAlps("spdy/3".to_owned()))
    );
}

#[test]
fn the_tls13_suites_must_come_before_every_other_suite() {
    let mut spec = effective_tls();
    spec.cipher_suites.rotate_left(3);
    assert_eq!(
        check_emittable(&spec),
        Err(ConvertError::Tls13SuitesNotFirst)
    );
    let mut spec = effective_tls();
    spec.cipher_suites[..3].reverse();
    assert_eq!(
        check_emittable(&spec),
        Ok(()),
        "any order among the TLS 1.3 suites"
    );
}

#[test]
fn versions_with_a_hole_are_refused() {
    let mut spec = effective_tls();
    spec.supported_versions = vec![0x0304, 0x0302];
    assert_eq!(
        check_emittable(&spec),
        Err(ConvertError::VersionGap(vec![0x0304, 0x0302]))
    );
}

#[test]
fn an_extension_boringssl_always_sends_cannot_be_left_out() {
    for id in [0x000A, 0x000D] {
        let mut spec = effective_tls();
        spec.extensions.retain(|known| *known != id);
        assert_eq!(
            check_emittable(&spec),
            Err(ConvertError::ExtensionAlwaysSent(id))
        );
    }
}

#[test]
fn the_legacy_extensions_follow_the_lowest_version() {
    for id in [0x000B, 0x0017, 0xFF01] {
        let mut spec = effective_tls();
        spec.extensions.retain(|known| *known != id);
        assert_eq!(
            check_emittable(&spec),
            Err(ConvertError::ExtensionAlwaysSent(id)),
            "a TLS 1.2 hello always carries {id:#06x}"
        );
    }
    let mut spec = effective_tls();
    spec.supported_versions = vec![0x0304];
    assert!(matches!(
        check_emittable(&spec),
        Err(ConvertError::ExtensionVersion { id: 0x000B, .. })
    ));
}

#[test]
fn the_tls13_extensions_follow_the_highest_version() {
    for id in [0x002B, 0x002D, 0x0033] {
        let mut spec = effective_tls();
        spec.extensions.retain(|known| *known != id);
        assert_eq!(
            check_emittable(&spec),
            Err(ConvertError::ExtensionAlwaysSent(id))
        );
    }
    let mut spec = tls12_only();
    spec.extensions.push(ext_id::PSK_KEY_EXCHANGE_MODES);
    assert!(matches!(
        check_emittable(&spec),
        Err(ConvertError::ExtensionVersion { id: 0x002D, .. })
    ));
}

#[test]
fn a_post_quantum_group_needs_tls13() {
    let mut spec = tls12_only();
    spec.supported_groups.insert(0, 0x11EC);
    assert_eq!(
        check_emittable(&spec),
        Err(ConvertError::GroupNeedsTls13(0x11EC))
    );
}

#[test]
fn only_zlib_and_brotli_certificates_can_be_compressed() {
    let mut spec = effective_tls();
    spec.cert_compression = vec![3];
    assert_eq!(
        check_emittable(&spec),
        Err(ConvertError::CertCompression(3))
    );
    spec.cert_compression = vec![CERT_COMPRESSION_ZLIB, CERT_COMPRESSION_BROTLI];
    assert_eq!(check_emittable(&spec), Ok(()));
}

#[test]
fn certificate_compressors_round_trip_and_cap_the_output() {
    let data: Vec<u8> = (0..20_000_u32).map(|n| (n % 251) as u8).collect();
    for algorithm in [CERT_COMPRESSION_BROTLI, CERT_COMPRESSION_ZLIB] {
        let codec = compressor(algorithm).expect("available");
        let (Codec::Pointer(compress), Codec::Pointer(decompress)) =
            (codec.compress(), codec.decompress())
        else {
            panic!("the compressors are plain functions");
        };
        let mut packed = Vec::new();
        compress(&data, &mut packed).expect("compresses");
        let mut unpacked = Vec::new();
        decompress(&packed, &mut unpacked).expect("decompresses");
        assert_eq!(unpacked, data, "algorithm {algorithm}");

        let bomb_len = usize::try_from(MAX_DECOMPRESSED_CERTIFICATE).expect("fits") + 1;
        let mut zeros_packed = Vec::new();
        compress(&vec![0_u8; bomb_len], &mut zeros_packed).expect("compresses");
        let mut sink = Vec::new();
        assert!(
            decompress(&zeros_packed, &mut sink).is_err(),
            "algorithm {algorithm} must refuse to expand past a handshake message"
        );
    }
}

#[test]
fn the_measured_http2_spec_becomes_the_expected_options() {
    let spec = measured_h2();
    let options = http2_options(&spec).expect("converts");
    assert_eq!(
        options.initial_conn_window_size,
        65_535 + spec.window_update
    );
    assert_eq!(options.initial_window_size, spec.setting(4).expect("sent"));
    assert_eq!(options.header_table_size, spec.setting(1));
    assert_eq!(options.enable_push, Some(false));
    assert_eq!(options.max_header_list_size, spec.setting(6));
    assert_eq!(options.max_concurrent_streams, None);
    assert_eq!(options.max_frame_size, None);
    assert!(!options.adaptive_window);
    assert_eq!(options.initial_stream_id, None);
    assert_eq!(options.priorities, None);
    let order = SettingsOrder::builder()
        .extend(spec.settings.iter().map(|&(id, _)| SettingId::from(id)))
        .build();
    assert_eq!(options.settings_order, Some(order));
    let pseudo = PseudoOrder::builder()
        .extend([
            PseudoId::Method,
            PseudoId::Authority,
            PseudoId::Scheme,
            PseudoId::Path,
        ])
        .build();
    assert_eq!(options.headers_pseudo_order, Some(pseudo));
    let priority = spec.headers_priority.expect("Chrome sends priority fields");
    assert_eq!(priority.weight, 256, "protocol weights run from 1 to 256");
    assert_eq!(
        options.headers_stream_dependency,
        Some(StreamDependency::new(StreamId::from(0), 255, true))
    );
}

#[test]
fn an_akamai_string_with_priority_frames_becomes_priorities_and_a_later_first_stream() {
    let akamai =
        Akamai::parse("1:65536;4:131072;5:16384|12517377|3:0:0:201,5:0:0:101,7:1:0:1|m,p,a,s")
            .expect("parses");
    let spec = H2Spec::from_akamai(&akamai).expect("sendable");
    let options = http2_options(&spec).expect("converts");
    let mut expected = Priorities::builder();
    for (stream, exclusive, weight_byte) in [(3_u32, false, 200_u8), (5, false, 100), (7, true, 0)]
    {
        expected = expected.push(Priority::new(
            StreamId::from(stream),
            StreamDependency::new(StreamId::from(0), weight_byte, exclusive),
        ));
    }
    assert_eq!(options.priorities, Some(expected.build()));
    assert_eq!(options.initial_stream_id, Some(9));
    assert_eq!(options.initial_conn_window_size, 65_535 + 12_517_377);
    assert_eq!(options.headers_stream_dependency, None);
}

#[test]
fn a_spec_with_no_window_update_keeps_the_protocol_window() {
    let mut spec = measured_h2();
    spec.window_update = 0;
    let options = http2_options(&spec).expect("converts");
    assert_eq!(options.initial_conn_window_size, 65_535);
}

#[test]
fn out_of_range_stream_ids_and_weights_are_refused() {
    let mut spec = measured_h2();
    spec.headers_priority.as_mut().expect("sent").depends_on = 0x8000_0000;
    assert_eq!(
        http2_options(&spec).err(),
        Some(ConvertError::StreamId(0x8000_0000))
    );
    let mut spec = measured_h2();
    spec.headers_priority.as_mut().expect("sent").weight = 0;
    assert_eq!(http2_options(&spec).err(), Some(ConvertError::Weight(0)));
    spec.headers_priority.as_mut().expect("sent").weight = 257;
    assert_eq!(http2_options(&spec).err(), Some(ConvertError::Weight(257)));
}

fn pair(name: &str, value: &str) -> (String, String) {
    (name.to_owned(), value.to_owned())
}

#[test]
fn headers_keep_their_order_and_spelling() {
    let list = [
        pair("sec-ch-ua", "\"Brand\";v=\"1\""),
        pair("User-Agent", "Agent/1"),
        pair("Accept-Language", "en-US,en;q=0.9"),
    ];
    let (map, order) = wreq_headers(&list, &[]).expect("converts");
    let spelled: Vec<&[u8]> = order.iter().map(|(_, case)| case.as_ref()).collect();
    assert_eq!(
        spelled,
        [
            b"sec-ch-ua".as_slice(),
            b"User-Agent".as_slice(),
            b"Accept-Language".as_slice()
        ]
    );
    let keys: Vec<&str> = order.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(keys, ["sec-ch-ua", "user-agent", "accept-language"]);
    assert_eq!(map.get("user-agent").expect("set"), "Agent/1");
}

#[test]
fn transport_headers_lead_the_order_without_a_value() {
    let list = [pair("Accept", "*/*")];
    let (map, order) = wreq_headers(&list, &["Host"]).expect("converts");
    let names: Vec<&str> = order.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["host", "accept"]);
    assert!(map.get("host").is_none(), "the transport adds the value");
}

#[test]
fn only_groups_with_a_key_share_implementation_get_a_key_share() {
    let mut spec = effective_tls();
    spec.supported_groups.push(0x0100);
    spec.key_share_groups = vec![0x0100];
    assert_eq!(check_emittable(&spec), Err(ConvertError::KeyShare(0x0100)));
    spec.key_share_groups = vec![0x001D];
    assert_eq!(
        check_emittable(&spec),
        Ok(()),
        "offered, but no share is sent"
    );
}

#[test]
fn credential_values_are_marked_sensitive_and_never_printed() {
    let list = [pair("Cookie", "session=abc"), pair("Accept", "*/*")];
    let (map, _) = wreq_headers(&list, &[]).expect("converts");
    assert!(map.get("cookie").expect("set").is_sensitive());
    assert!(!map.get("accept").expect("set").is_sensitive());
    assert!(!format!("{map:?}").contains("session=abc"));
}

#[test]
fn a_header_that_cannot_be_sent_is_refused_without_its_value() {
    let bad_name = [pair("Bad Name", "x")];
    assert!(matches!(
        wreq_headers(&bad_name, &[]),
        Err(ConvertError::Header {
            reason: "not a valid name",
            ..
        })
    ));
    let bad_value = [pair("X-Test", "line\nbreak secret")];
    let error = wreq_headers(&bad_value, &[]).expect_err("refused");
    assert!(matches!(
        &error,
        ConvertError::Header {
            reason: "not a valid value",
            ..
        }
    ));
    assert!(!error.to_string().contains("secret"));
}
