//! Custom inputs, proven on the wire.
//!
//! A custom JA3, `JA4_r`, `JA4_ro`, peetprint, Akamai string or header set is layered over the
//! measured Chrome. Each accepted input is sent to the loopback server and read back with the
//! crate's parsers; what comes back must be the input, exactly. Each refused input must name its
//! reason and must never reach a client.

use seleniumbase_rs::impersonate::client::{ImpersonateError, ImpersonationProfile, WireProtocol};
use seleniumbase_rs::impersonate::convert::ConvertError;
use seleniumbase_rs::stealth::fingerprint::{BrandVersion, ClientHints};
use seleniumbase_rs::wire::akamai::Akamai;
use seleniumbase_rs::wire::client_hello::{ext_id, ClientHello};
use seleniumbase_rs::wire::h2_spec::SpecError as H2SpecError;
use seleniumbase_rs::wire::header_spec::{HeaderValue, RequestShape};
use seleniumbase_rs::wire::ja3::peetprint::Slot;
use seleniumbase_rs::wire::ja3::{Ja3, Peetprint};
use seleniumbase_rs::wire::ja4::{Ja4, Ja4Order, Ja4Raw};
use seleniumbase_rs::wire::tls_spec::SpecError as TlsSpecError;
use seleniumbase_rs::wire::BrowserKind;

use crate::exchange::{exchange, Via};
use crate::server::Served;

fn base() -> ImpersonationProfile {
    ImpersonationProfile::measured(BrowserKind::Chrome).expect("the measured profile")
}

async fn send(profile: &ImpersonationProfile) -> Served {
    exchange(
        profile,
        RequestShape::Document,
        WireProtocol::Http2,
        Via::Negotiated,
    )
    .await
}

fn lowercase(list: Vec<(String, String)>) -> Vec<(String, String)> {
    list.into_iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value))
        .collect()
}

// ===== JA3 =====

/// JA3 strings built from the base by changing one thing each, with the reason they are interesting.
fn ja3_variants() -> Vec<(&'static str, Ja3)> {
    let base = base().tls_spec().ja3();
    let mut variants = vec![("the base order", base.clone())];

    let mut reversed = base.clone();
    reversed.extensions.reverse();
    variants.push(("the extensions in reverse", reversed));

    let optional = [5, 18, 35, 27, 65037];
    let mut slim = base.clone();
    slim.extensions.retain(|id| !optional.contains(id));
    variants.push(("without the optional extensions", slim));

    let mut ciphers = base.clone();
    ciphers.ciphers = vec![
        0x1303, 0x1301, 0x1302, 0xC02F, 0xC02B, 0xC030, 0xC02C, 0xCCA8, 0xCCA9, 0x0035, 0x002F,
    ];
    variants.push(("another cipher order and subset", ciphers));

    let mut groups = base.clone();
    groups.groups = vec![29, 23, 24, 25];
    variants.push(("classical groups only", groups));

    let mut tls12 = base.clone();
    tls12.ciphers.retain(|id| !(0x1301..=0x1303).contains(id));
    tls12.extensions = vec![0, 23, 65281, 10, 11, 35, 16, 5, 13, 18, 27];
    tls12.groups = vec![29, 23, 24];
    variants.push(("TLS 1.2 only", tls12));
    variants
}

/// The length in bytes of the handshake message `hello` was parsed from: its 4-byte header, then
/// the fields in the order the wire carries them.
fn message_len(hello: &ClientHello) -> usize {
    let extensions: usize = hello.extensions.iter().map(|ext| 4 + ext.data.len()).sum();
    4 + 2
        + hello.random.len()
        + (1 + hello.session_id.len())
        + (2 + 2 * hello.cipher_suites.len())
        + (1 + hello.compression_methods.len())
        + (2 + extensions)
}

/// `hello` as the profile describes it: BoringSSL's own padding taken off.
///
/// BoringSSL fills a hello of 256 to 511 bytes up to 512 with a padding extension, a workaround for
/// old F5 servers that no input can switch off. How long a hello is depends on the random size of
/// its ECH GREASE payload (144 to 240 bytes), so a short variant such as the classical groups
/// alone is padded on some connections and not on others. The rule is asserted rather than
/// ignored: the extension is removed only from a hello that BoringSSL's rule pads (the padded
/// message is 512 bytes, or a few more when the gap was under five bytes), and an unpadded hello
/// must not be one the rule would have padded.
fn without_boringssl_padding(what: &str, hello: &ClientHello) -> ClientHello {
    let length = message_len(hello);
    let padded = hello
        .extensions
        .last()
        .is_some_and(|ext| ext.id == ext_id::PADDING);
    let mut hello = hello.clone();
    if padded {
        assert!(
            (512..=516).contains(&length),
            "{what}: a padded hello is 512 bytes, not {length}"
        );
        hello.extensions.pop();
    } else {
        assert!(
            !(256..512).contains(&length),
            "{what}: BoringSSL pads a hello of {length} bytes, and this one has no padding"
        );
    }
    hello
}

#[tokio::test]
async fn a_custom_ja3_arrives_exactly_as_given() {
    for (what, ja3) in ja3_variants() {
        let profile = ImpersonationProfile::custom()
            .ja3(&ja3.to_string())
            .unwrap_or_else(|error| panic!("{what}: {error}"))
            .build()
            .unwrap_or_else(|error| panic!("{what}: {error}"));
        let served = send(&profile).await;
        let hello = without_boringssl_padding(what, &served.hello);
        assert_eq!(Ja3::of(&hello), ja3, "{what}");
        assert_eq!(
            Ja4::raw_original_order(&hello),
            profile.tls_spec().ja4_raw(Ja4Order::Original).to_string(),
            "{what}: the JA4_ro of the wire must be the profile's"
        );
    }
}

fn refused_ja3(what: &str, ja3: &Ja3) -> ImpersonateError {
    ImpersonationProfile::custom()
        .ja3(&ja3.to_string())
        .err()
        .unwrap_or_else(|| panic!("{what}: a JA3 that cannot be sent was accepted"))
}

#[test]
fn a_ja3_that_cannot_be_sent_is_refused_by_name_before_a_client_exists() {
    let base = base().tls_spec().ja3();

    let mut with = base.clone();
    with.extensions.insert(1, 0xCA34);
    assert!(matches!(
        refused_ja3("trust_anchors", &with),
        ImpersonateError::Tls(TlsSpecError::UnsupportedExtension { id: 0xCA34, .. })
    ));

    let mut with = base.clone();
    with.extensions.push(41);
    assert!(matches!(
        refused_ja3("pre_shared_key", &with),
        ImpersonateError::Tls(TlsSpecError::UnsupportedExtension { id: 41, .. })
    ));

    let mut with = base.clone();
    with.version = 770;
    assert_eq!(
        refused_ja3("TLS 1.1 handshake version", &with),
        ImpersonateError::Tls(TlsSpecError::LegacyVersion(770))
    );

    let mut with = base.clone();
    with.ciphers.push(0x00FF);
    assert_eq!(
        refused_ja3("the renegotiation SCSV", &with),
        ImpersonateError::Tls(TlsSpecError::UnsupportedCipher(0x00FF))
    );

    let mut with = base.clone();
    with.ciphers.retain(|id| *id != 0x1302);
    assert_eq!(
        refused_ja3("a TLS 1.3 subset", &with),
        ImpersonateError::Tls(TlsSpecError::Tls13Suites)
    );

    let mut with = base.clone();
    with.groups.push(30);
    assert_eq!(
        refused_ja3("x448", &with),
        ImpersonateError::Tls(TlsSpecError::UnsupportedGroup(30))
    );

    let mut with = base.clone();
    with.point_formats = vec![0, 1];
    assert_eq!(
        refused_ja3("a compressed point format", &with),
        ImpersonateError::Tls(TlsSpecError::PointFormats(vec![0, 1]))
    );

    let mut with = base.clone();
    with.extensions.push(5);
    assert_eq!(
        refused_ja3("a repeated extension", &with),
        ImpersonateError::Tls(TlsSpecError::DuplicateExtension(5))
    );
}

/// The TLS 1.2-only JA3 of [`ja3_variants`].
fn tls12_ja3() -> Ja3 {
    ja3_variants()
        .into_iter()
        .find(|(what, _)| *what == "TLS 1.2 only")
        .map(|(_, ja3)| ja3)
        .expect("the variant exists")
}

#[test]
fn a_ja3_without_what_boringssl_always_sends_is_refused_by_name() {
    let base = tls12_ja3();

    let mut with = base.clone();
    with.extensions.retain(|id| *id != 10);
    with.groups.clear();
    assert_eq!(
        refused_ja3("no supported_groups", &with),
        ImpersonateError::Convert(ConvertError::ExtensionAlwaysSent(10))
    );

    let mut with = base.clone();
    with.extensions.retain(|id| *id != 13);
    assert_eq!(
        refused_ja3("no signature_algorithms", &with),
        ImpersonateError::Convert(ConvertError::ExtensionAlwaysSent(13))
    );

    for id in [11, 23, 65281] {
        let mut with = base.clone();
        with.extensions.retain(|known| *known != id);
        if id == 11 {
            with.point_formats.clear();
        }
        assert_eq!(
            refused_ja3(
                "a TLS 1.2 hello without one of its legacy extensions",
                &with
            ),
            ImpersonateError::Convert(ConvertError::ExtensionAlwaysSent(id))
        );
    }
}

#[test]
fn a_ja3_that_lists_a_tls13_suite_after_a_tls12_suite_is_refused_by_name() {
    let mut ja3 = base().tls_spec().ja3();
    ja3.ciphers.retain(|id| !(0x1301..=0x1303).contains(id));
    ja3.ciphers.extend([0x1301, 0x1302, 0x1303]);
    assert_eq!(
        refused_ja3("TLS 1.3 suites last", &ja3),
        ImpersonateError::Convert(ConvertError::Tls13SuitesNotFirst)
    );
}

/// The TLS 1.2 JA3 with its tail changed: `tail` is the last extensions, in order.
fn tls12_ja3_ending_in(tail: &[u16]) -> Ja3 {
    let mut ja3 = tls12_ja3();
    ja3.extensions.retain(|id| !tail.contains(id));
    ja3.extensions.extend_from_slice(tail);
    ja3
}

#[tokio::test]
async fn padding_arrives_when_the_ja3_asks_for_it_after_an_empty_extension() {
    let ja3 = tls12_ja3_ending_in(&[23, 21]);
    let profile = ImpersonationProfile::custom()
        .grease(false)
        .expect("accepted")
        .ja3(&ja3.to_string())
        .expect("padding follows an empty extension when there is no GREASE")
        .build()
        .expect("builds");
    let served = send(&profile).await;
    assert_eq!(Ja3::of(&served.hello), ja3);
    assert!(!served.hello.has_grease());
}

#[test]
fn padding_that_boringssl_would_not_write_or_would_add_is_refused_by_name() {
    let asked_after_a_full_extension = tls12_ja3_ending_in(&[27, 21]);
    let error = ImpersonationProfile::custom()
        .grease(false)
        .expect("accepted")
        .ja3(&asked_after_a_full_extension.to_string())
        .expect_err("no padding follows a non-empty extension");
    assert_eq!(
        error,
        ImpersonateError::Convert(ConvertError::PaddingUnavailable)
    );

    let silent_padding = tls12_ja3_ending_in(&[23]);
    let error = ImpersonationProfile::custom()
        .grease(false)
        .expect("accepted")
        .ja3(&silent_padding.to_string())
        .expect_err("BoringSSL would add a padding extension the JA3 does not list");
    assert_eq!(
        error,
        ImpersonateError::Convert(ConvertError::PaddingAppended)
    );

    let with_grease = ImpersonationProfile::custom()
        .ja3(&tls12_ja3_ending_in(&[23, 21]).to_string())
        .expect_err("GREASE ends the extension list with a non-empty extension");
    assert_eq!(
        with_grease,
        ImpersonateError::Convert(ConvertError::PaddingUnavailable)
    );
}

#[tokio::test]
async fn a_pinned_order_can_be_given_back_to_boringssl() {
    let ja3 = ja3_variants().remove(0).1;
    let profile = ImpersonationProfile::custom()
        .ja3(&ja3.to_string())
        .expect("accepted")
        .permute_extensions(true)
        .expect("accepted")
        .build()
        .expect("builds");
    let first = send(&profile).await;
    let second = send(&profile).await;
    assert_eq!(Ja4::of(&first.hello), Ja4::of(&second.hello));
    assert_eq!(
        first.hello.extension_ids().len(),
        second.hello.extension_ids().len()
    );
}

// ===== JA4 and peetprint =====

#[tokio::test]
async fn a_custom_ja4_r_arrives_as_the_same_ja4() {
    let spec = base().tls_spec().clone();
    let mut raw = spec.ja4_raw(Ja4Order::Sorted);
    raw.ciphers.retain(|id| *id != 0x0035 && *id != 0x009D);
    raw.a = format!("{}{:02}{}", &raw.a[..4], raw.ciphers.len(), &raw.a[6..]);
    raw.signature_algorithms = vec![0x0403, 0x0804, 0x0401];
    let profile = ImpersonationProfile::custom()
        .ja4_r(&raw.to_string())
        .expect("accepted")
        .build()
        .expect("builds");
    let served = send(&profile).await;
    assert_eq!(Ja4::of(&served.hello), raw.hashed());
    assert_eq!(Ja4::raw(&served.hello), raw.to_string());
}

#[tokio::test]
async fn a_custom_ja4_ro_arrives_with_its_order() {
    let spec = base().tls_spec().clone();
    let mut raw = spec.ja4_raw(Ja4Order::Original);
    raw.extensions.reverse();
    let (mut ciphers, tls13): (Vec<u16>, Vec<u16>) = raw
        .ciphers
        .iter()
        .partition(|id| !(0x1301..=0x1303).contains(*id));
    ciphers.reverse();
    raw.ciphers = tls13.into_iter().rev().chain(ciphers).collect();
    let profile = ImpersonationProfile::custom()
        .ja4_ro(&raw.to_string())
        .expect("accepted")
        .build()
        .expect("builds");
    let served = send(&profile).await;
    assert_eq!(Ja4::raw_original_order(&served.hello), raw.to_string());
    assert_eq!(Ja4::of_original_order(&served.hello), raw.hashed());
}

#[test]
fn a_ja4_that_cannot_be_sent_is_refused_by_name() {
    let spec = base().tls_spec().clone();
    let mut raw: Ja4Raw = spec.ja4_raw(Ja4Order::Sorted);
    raw.signature_algorithms.push(0x0904);
    let error = ImpersonationProfile::custom()
        .ja4_r(&raw.to_string())
        .expect_err("ML-DSA cannot be sent");
    assert!(matches!(
        error,
        ImpersonateError::Tls(TlsSpecError::UnsupportedSigalg { id: 0x0904, .. })
    ));
}

#[tokio::test]
async fn a_custom_peetprint_arrives_as_the_same_peetprint() {
    let spec = base().tls_spec().clone();
    let mut peet = spec.peetprint();
    peet.ciphers
        .retain(|slot| *slot != Slot::Id(0x0035) && *slot != Slot::Id(0x009D));
    peet.groups.retain(|slot| *slot != Slot::Id(24));
    peet.signature_algorithms
        .retain(|slot| *slot != Slot::Id(0x0601));
    let profile = ImpersonationProfile::custom()
        .peetprint(&peet.to_string())
        .expect("accepted")
        .build()
        .expect("builds");
    let served = send(&profile).await;
    assert_eq!(
        Peetprint::of(&served.hello),
        peet.clone().normalized(),
        "{peet}"
    );
}

// ===== Akamai =====

/// Akamai strings of three browser families and three edge cases, all in canonical form.
const AKAMAI_STRINGS: [(&str, &str); 6] = [
    (
        "the measured Chrome",
        "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p",
    ),
    (
        "a Firefox-like start with six PRIORITY frames",
        "1:65536;4:131072;5:16384|12517377|3:0:0:201,5:0:0:101,7:0:0:1,9:0:7:1,11:0:3:1,13:0:0:241|m,p,a,s",
    ),
    (
        "a Safari-like start",
        "2:0;3:100;4:2097152;9:1|10420225|0|m,s,p,a",
    ),
    (
        "settings in another order and another pseudo order",
        "4:1048576;1:4096;2:0|983041|0|a,m,s,p",
    ),
    (
        "every setting wreq can send",
        "1:4096;2:1;3:200;4:65535;5:32768;6:16384;8:1;9:1|0|0|m,a,s,p",
    ),
    (
        "an exclusive dependency and no window update",
        "3:100;4:65535|0|3:1:0:256|s,m,a,p",
    ),
];

#[tokio::test]
async fn a_custom_akamai_string_arrives_exactly_as_given() {
    for (what, text) in AKAMAI_STRINGS {
        let profile = ImpersonationProfile::custom()
            .akamai(text)
            .unwrap_or_else(|error| panic!("{what}: {error}"))
            .build()
            .unwrap_or_else(|error| panic!("{what}: {error}"));
        let served = send(&profile).await;
        let preface = served.h2();
        let wire = preface.akamai().expect("an Akamai string");
        let wanted = Akamai::parse(text).expect("canonical");
        assert_eq!(wire, wanted, "{what}: {wire} instead of {text}");
        assert_eq!(wire.to_string(), text, "{what}");
        let streams = wanted.priority_frames.iter().map(|frame| frame.stream_id);
        let first_free = streams.max().map_or(1, |highest| highest + 2);
        assert_eq!(preface.request_stream_id, first_free, "{what}");
        assert_eq!(
            preface.headers_priority,
            base().http2_spec().headers_priority,
            "{what}: the HEADERS priority is not in the string, so the base's stays"
        );
    }
}

#[test]
fn an_akamai_string_wreq_cannot_send_is_refused_by_name() {
    let refused = |text: &str| {
        ImpersonationProfile::custom()
            .akamai(text)
            .err()
            .unwrap_or_else(|| panic!("`{text}` was accepted"))
    };
    assert_eq!(
        refused("2570:1;4:1048576|0|0|m,a,s,p"),
        ImpersonateError::Http2(H2SpecError::UnsupportedSetting(2570))
    );
    assert_eq!(
        refused("1:4096|0|0|m,a,s,p"),
        ImpersonateError::Http2(H2SpecError::SettingAlwaysSent(4))
    );
    assert_eq!(
        refused("2:5;4:65535|0|0|m,a,s,p"),
        ImpersonateError::Http2(H2SpecError::SettingValue { id: 2, value: 5 })
    );
    assert_eq!(
        refused("4:65535|2147483647|0|m,a,s,p"),
        ImpersonateError::Http2(H2SpecError::WindowUpdate(2_147_483_647))
    );
    assert!(matches!(
        refused("4:65535;4:1|0|0|m,a,s,p"),
        ImpersonateError::Akamai(_)
    ));
    assert!(matches!(
        refused("4:65535|0|0|m,a,s"),
        ImpersonateError::Akamai(_)
    ));
    assert!(matches!(
        refused("4:65535|0|3:0:0:0|m,a,s,p"),
        ImpersonateError::Akamai(_)
    ));
    assert!(matches!(
        refused("4:65535|0|3:0:3:10|m,a,s,p"),
        ImpersonateError::Http2(H2SpecError::Priority { stream: 3, .. })
    ));
}

// ===== Headers and identity =====

#[tokio::test]
async fn a_custom_header_set_arrives_in_its_order_with_its_values() {
    let profile = ImpersonationProfile::custom()
        .header(
            RequestShape::Document,
            "X-Probe",
            HeaderValue::Literal("one".to_owned()),
        )
        .header(
            RequestShape::Document,
            "Accept",
            HeaderValue::Literal("text/plain".to_owned()),
        )
        .without_header(RequestShape::Document, "Sec-Fetch-User")
        .header_order(RequestShape::Document, ["User-Agent", "Accept"])
        .priority("document", "u=2")
        .user_agent("Probe/9")
        .accept_language("de-DE,de;q=0.9")
        .build()
        .expect("builds");

    let served = send(&profile).await;
    let sent: Vec<(String, String)> = served
        .h2()
        .regular_headers()
        .map(|field| (field.name.clone(), field.value.clone()))
        .collect();
    let expected = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http2)
        .expect("renders");
    assert_eq!(sent, lowercase(expected));

    let names: Vec<&str> = sent.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names.first(), Some(&"user-agent"));
    assert_eq!(names.get(1), Some(&"accept"));
    assert!(!names.contains(&"sec-fetch-user"));
    assert_eq!(names.iter().rev().nth(1), Some(&"x-probe"));
    assert_eq!(names.last(), Some(&"priority"));
    let value = |name: &str| {
        sent.iter()
            .find(|(known, _)| known == name)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(value("user-agent"), Some("Probe/9"));
    assert_eq!(value("accept"), Some("text/plain"));
    assert_eq!(value("accept-language"), Some("de-DE,de;q=0.9"));
    assert_eq!(value("priority"), Some("u=2"));
    assert_eq!(value("x-probe"), Some("one"));
}

#[tokio::test]
async fn custom_client_hints_arrive_as_the_three_hint_headers() {
    let hints = ClientHints {
        brands: vec![
            BrandVersion::new("Probe Browser", "9"),
            BrandVersion::new("Not;A\"Brand", "99"),
        ],
        platform: "TestOS".to_owned(),
        mobile: true,
        ..ClientHints::default()
    };
    let profile = ImpersonationProfile::custom()
        .client_hints(hints)
        .build()
        .expect("builds");
    let served = send(&profile).await;
    let preface = served.h2();
    assert_eq!(
        preface.header("sec-ch-ua"),
        Some("\"Probe Browser\";v=\"9\", \"Not;A\\\"Brand\";v=\"99\"")
    );
    assert_eq!(preface.header("sec-ch-ua-mobile"), Some("?1"));
    assert_eq!(preface.header("sec-ch-ua-platform"), Some("\"TestOS\""));
}

#[tokio::test]
async fn a_custom_header_set_keeps_its_spelling_over_http1() {
    let profile = ImpersonationProfile::custom()
        .header(
            RequestShape::Document,
            "X-Mixed-Case",
            HeaderValue::Literal("1".to_owned()),
        )
        .build()
        .expect("builds");
    let served = exchange(
        &profile,
        RequestShape::Document,
        WireProtocol::Http1,
        Via::Http1Only,
    )
    .await;
    let request = served.http1();
    let names: Vec<&str> = request
        .headers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(names.contains(&"X-Mixed-Case"), "{names:?}");
    assert!(names.contains(&"User-Agent"), "{names:?}");
}
