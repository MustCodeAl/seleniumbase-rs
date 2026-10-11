//! The profile built from the measured Chrome 155 on macOS, proven on the wire.
//!
//! The expected values are never typed here: they come from the measured file through
//! `wire::emulation`, and from the evidence that file keeps.

use seleniumbase_rs::impersonate::client::{ImpersonationProfile, WireProtocol};
use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::client_hello::{ext_id, without_grease, ClientHello};
use seleniumbase_rs::wire::emulation::{self, Emulation, KnownGap};
use seleniumbase_rs::wire::h2_spec::H2Spec;
use seleniumbase_rs::wire::header_spec::RequestShape;
use seleniumbase_rs::wire::ja4::{Ja4, Ja4AField};
use seleniumbase_rs::wire::tls_spec::TlsSpec;
use seleniumbase_rs::wire::BrowserKind;

use crate::exchange::{exchange, exchange_with, Via};

/// The measured file, including the evidence its specs were derived from.
const CHROME_FILE: &str = include_str!("../../src/wire/measured/chrome-155-macos.json");

fn chrome() -> &'static Emulation {
    emulation::get(BrowserKind::Chrome, OsType::Macos).expect("Chrome on macOS is measured")
}

fn profile() -> ImpersonationProfile {
    ImpersonationProfile::measured(BrowserKind::Chrome).expect("the measured profile")
}

/// The spec a hello shows, with the order Chrome shuffles removed and the GREASE signature
/// algorithm (which `BoringSSL` adds first) taken out, so it can be compared with a measured spec.
fn comparable(hello: &ClientHello, permute: bool) -> TlsSpec {
    let mut spec = TlsSpec::from_client_hello(hello);
    spec.extensions.sort_unstable();
    spec.signature_algorithms = without_grease(&spec.signature_algorithms);
    spec.permute_extensions = permute;
    spec
}

/// The measured spec as the client can send it, with its extensions sorted to match [`comparable`].
fn expected_on_the_wire(measured: &Emulation) -> TlsSpec {
    let mut spec = measured
        .tls
        .without_known_gaps(&measured.provenance.known_gaps);
    spec.extensions.sort_unstable();
    spec
}

/// Fails with a message that says which known gap closed, when one did.
fn assert_no_gap_closed(hello: &ClientHello, measured: &Emulation) {
    for gap in &measured.provenance.known_gaps {
        match gap {
            KnownGap::TrustAnchorsExtension => assert!(
                !hello.has_extension(ext_id::TRUST_ANCHORS),
                "BoringSSL now sends trust_anchors: the known gap TrustAnchorsExtension is closed; \
                 remove it from provenance.known_gaps and re-measure"
            ),
            KnownGap::MlDsaSignatureAlgorithms => assert!(
                hello
                    .signature_algorithms
                    .iter()
                    .all(|id| !(0x0904..=0x0906).contains(id)),
                "BoringSSL now sends ML-DSA signature algorithms: the known gap \
                 MlDsaSignatureAlgorithms is closed; remove it from provenance.known_gaps and re-measure"
            ),
        }
    }
}

#[tokio::test]
async fn the_hello_is_the_measured_one_minus_exactly_the_known_gaps() {
    let measured = chrome();
    let served = exchange(
        &profile(),
        RequestShape::Document,
        WireProtocol::Http2,
        Via::Negotiated,
    )
    .await;
    let hello = &served.hello;

    assert_no_gap_closed(hello, measured);
    let wire = comparable(hello, measured.tls.permute_extensions);
    assert_eq!(
        wire,
        expected_on_the_wire(measured),
        "the spec of the wire hello must be the measured spec without its known gaps"
    );

    // The measurement as the browser sent it differs in exactly what the gaps say.
    let wire_ja4 = Ja4::of(hello);
    let measured_ja4 = measured.tls.ja4();
    let diff = wire_ja4.diff(&measured_ja4);
    assert_eq!(diff.a_fields, [Ja4AField::ExtensionCount]);
    assert!(!diff.b, "the cipher hash must equal the measured one");
    assert!(diff.c, "the extension hash differs by exactly the gaps");
    assert_eq!(
        wire_ja4,
        measured
            .tls
            .without_known_gaps(&measured.provenance.known_gaps)
            .ja4()
    );
}

#[tokio::test]
async fn the_measured_ja4_is_the_one_in_the_evidence() {
    let file: serde_json::Value = serde_json::from_str(CHROME_FILE).expect("valid JSON");
    let sample = &file["capture"]["tls"]["samples"][0];
    assert_eq!(
        chrome().tls.ja4().to_string(),
        sample["ja4"].as_str().expect("a JA4 in the evidence")
    );
}

#[tokio::test]
async fn alpn_server_name_groups_and_key_shares_equal_the_measurement() {
    let measured = chrome();
    let served = exchange(
        &profile(),
        RequestShape::Document,
        WireProtocol::Http2,
        Via::Negotiated,
    )
    .await;
    let hello = &served.hello;
    assert_eq!(served.alpn.as_deref(), Some(&b"h2"[..]));
    assert_eq!(
        hello.alpn,
        measured
            .tls
            .alpn
            .iter()
            .map(|name| name.as_bytes().to_vec())
            .collect::<Vec<_>>()
    );
    assert_eq!(hello.sni.as_deref(), Some(crate::server::HOST));
    assert_eq!(
        without_grease(&hello.supported_groups),
        measured.tls.supported_groups
    );
    assert_eq!(
        without_grease(&hello.key_share_groups),
        measured.tls.key_share_groups
    );
    assert_eq!(
        without_grease(&hello.cipher_suites),
        measured.tls.cipher_suites
    );
    assert_eq!(hello.has_grease(), measured.tls.grease);
}

#[tokio::test]
async fn chrome_shuffles_its_extensions_and_so_does_the_client() {
    let mut orders = Vec::new();
    let mut ja4s = Vec::new();
    for _ in 0..3 {
        let served = exchange(
            &profile(),
            RequestShape::Document,
            WireProtocol::Http2,
            Via::Negotiated,
        )
        .await;
        orders.push(without_grease(&served.hello.extension_ids()));
        ja4s.push(Ja4::of(&served.hello));
    }
    assert!(
        ja4s.windows(2).all(|pair| pair[0] == pair[1]),
        "JA4 is stable"
    );
    assert!(
        orders.windows(2).any(|pair| pair[0] != pair[1]),
        "three connections with the same extension order: the client is not permuting"
    );
}

#[tokio::test]
async fn bare_wreq_is_not_chrome() {
    let measured = chrome();
    let served = exchange_with(|builder| builder).await;
    let bare = Ja4::of(&served.hello);
    let target = measured
        .tls
        .without_known_gaps(&measured.provenance.known_gaps)
        .ja4();
    assert_ne!(
        bare, target,
        "an untouched wreq client must not look like the profile"
    );
    let h2 = served.h2();
    assert_ne!(
        h2.akamai().expect("an Akamai string").to_string(),
        measured
            .http2
            .as_ref()
            .expect("measured")
            .akamai()
            .to_string()
    );
}

/// The headers an HTTP/2 request carried, pseudo-headers left out.
fn h2_headers(preface: &seleniumbase_rs::wire::h2_frames::H2Preface) -> Vec<(String, String)> {
    preface
        .regular_headers()
        .map(|field| (field.name.clone(), field.value.clone()))
        .collect()
}

fn lowercase(list: Vec<(String, String)>) -> Vec<(String, String)> {
    list.into_iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value))
        .collect()
}

#[tokio::test]
async fn the_http2_start_is_the_measured_akamai_string_and_headers_priority() {
    let measured = chrome();
    let spec = measured.http2.as_ref().expect("the HTTP/2 capture exists");
    let served = exchange(
        &profile(),
        RequestShape::Document,
        WireProtocol::Http2,
        Via::Negotiated,
    )
    .await;
    let preface = served.h2();
    assert_eq!(
        preface.akamai().expect("an Akamai string"),
        spec.akamai(),
        "SETTINGS order and values, connection window, PRIORITY frames, pseudo-header order"
    );
    assert_eq!(preface.headers_priority, spec.headers_priority);
    assert_eq!(H2Spec::from_preface(preface).expect("a spec"), *spec);
}

#[tokio::test]
async fn the_document_headers_arrive_in_the_measured_order_with_the_measured_values() {
    let profile = profile();
    let served = exchange(
        &profile,
        RequestShape::Document,
        WireProtocol::Http2,
        Via::Negotiated,
    )
    .await;
    let expected = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http2)
        .expect("renders");
    assert_eq!(h2_headers(served.h2()), lowercase(expected));
}

#[tokio::test]
async fn the_fetch_headers_arrive_in_the_measured_order_with_the_measured_values() {
    let profile = profile();
    let served = exchange(
        &profile,
        RequestShape::Fetch,
        WireProtocol::Http2,
        Via::Negotiated,
    )
    .await;
    let expected = profile
        .headers_for(RequestShape::Fetch, None, WireProtocol::Http2)
        .expect("renders");
    assert_eq!(h2_headers(served.h2()), lowercase(expected));
}

#[tokio::test]
async fn over_http1_the_headers_keep_the_measured_order_and_spelling() {
    let profile = profile();
    let served = exchange(
        &profile,
        RequestShape::Document,
        WireProtocol::Http1,
        Via::Http1Only,
    )
    .await;
    let request = served.http1();
    let expected = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http1)
        .expect("renders");
    let (host, rest) = request
        .headers
        .split_first()
        .expect("a request has headers");
    assert_eq!(host.0, "Host", "Chrome writes Host first");
    assert_eq!(rest, expected.as_slice());
}

#[tokio::test]
async fn a_cookie_is_sent_after_the_language_and_before_the_priority() {
    let profile = profile();
    let server = crate::server::WireServer::bind().await;
    let client = seleniumbase_rs::impersonate::client::impersonation_client(&profile)
        .expect("a client builder")
        .tls_cert_store(server.trust())
        .resolve(crate::server::HOST, server.addr())
        .no_proxy()
        .build()
        .expect("a client");
    let request = profile
        .apply(
            client.get(server.url()),
            RequestShape::Document,
            Some("session=abc123"),
            WireProtocol::Http2,
        )
        .expect("headers");
    let (served, response) = tokio::join!(server.serve_one(), request.send());
    response.expect("the request completes");
    let served = served.expect("the server saw a request");
    let names: Vec<String> = served
        .h2()
        .regular_headers()
        .map(|field| field.name.clone())
        .collect();
    let language = names
        .iter()
        .position(|name| name == "accept-language")
        .expect("a language");
    assert_eq!(names.get(language + 1).map(String::as_str), Some("cookie"));
    assert_eq!(names.last().map(String::as_str), Some("priority"));
    assert_eq!(served.h2().header("cookie"), Some("session=abc123"));
}
