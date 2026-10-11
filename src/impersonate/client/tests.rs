//! Unit tests of the impersonation profile and its custom builder. None of them opens a socket:
//! the proof that a profile reaches the wire is `tests/impersonate_wire.rs`.

use super::identity::{quote, sec_ch_ua, user_agent_major};
use super::{impersonation_client, ImpersonateError, ImpersonationProfile, WireProtocol};
use crate::impersonate::convert::ConvertError;
use crate::scrape::egress::geo::Locale;
use crate::stealth::fingerprint::{BrandVersion, Fingerprint, OsType};
use crate::wire::client_hello::ext_id;
use crate::wire::emulation::{self, Emulation, EmulationError, KnownGap};
use crate::wire::h2_spec::SpecError as H2SpecError;
use crate::wire::header_spec::{HeaderValue, RequestShape};
use crate::wire::tls_spec::SpecError as TlsSpecError;
use crate::wire::BrowserKind;

/// The measured Chrome file, including the evidence the headers were derived from.
const CHROME_FILE: &str = include_str!("../../wire/measured/chrome-155-macos.json");

fn chrome() -> ImpersonationProfile {
    ImpersonationProfile::for_system(BrowserKind::Chrome, OsType::Macos)
        .expect("Chrome on macOS is measured")
}

/// The headers the browser sent in one probe of the HTTP/2 pass, pseudo-headers left out.
fn evidence(probe: &str) -> Vec<(String, String)> {
    let file: serde_json::Value = serde_json::from_str(CHROME_FILE).expect("valid JSON");
    let probes = file["capture"]["h2"]["probes"].as_array().expect("probes");
    let entry = probes
        .iter()
        .find(|entry| entry["probe"] == probe)
        .expect("the probe was captured");
    entry["headers"]
        .as_array()
        .expect("headers")
        .iter()
        .map(|pair| {
            (
                pair[0].as_str().expect("name").to_owned(),
                pair[1].as_str().expect("value").to_owned(),
            )
        })
        .filter(|(name, _)| !name.starts_with(':'))
        .collect()
}

fn lowercase(list: Vec<(String, String)>) -> Vec<(String, String)> {
    list.into_iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value))
        .collect()
}

#[test]
fn the_measured_profile_keeps_its_gaps_and_sends_a_reproducible_spec() {
    let profile = chrome();
    assert_eq!(profile.source(), "chrome-155-macos");
    assert_eq!(profile.major(), 155);
    assert_eq!(
        profile.known_gaps(),
        [
            KnownGap::TrustAnchorsExtension,
            KnownGap::MlDsaSignatureAlgorithms
        ]
    );
    assert!(!profile.tls_spec().has_extension(ext_id::TRUST_ANCHORS));
    assert_eq!(profile.validate(), Ok(()));
    assert_eq!(profile.dropped_settings(), []);
    let measured = emulation::get(BrowserKind::Chrome, OsType::Macos).expect("measured");
    assert_eq!(profile.user_agent(), measured.user_agent);
    assert_eq!(profile.tls_spec().grease, measured.tls.grease);
    assert_eq!(
        profile.http2_spec(),
        measured.http2.as_ref().expect("measured")
    );
}

#[test]
fn an_entry_without_its_http2_spec_or_hints_is_not_a_profile() {
    let mut entry = Emulation::from_json("chrome", CHROME_FILE).expect("the measured file");
    let hints = entry.client_hints.take();
    assert!(hints.is_some());
    assert_eq!(
        ImpersonationProfile::from_measured(&entry),
        Err(ImpersonateError::Incomplete {
            what: "client hints"
        })
    );
    entry.client_hints = hints;
    entry.http2 = None;
    assert_eq!(
        ImpersonationProfile::from_measured(&entry),
        Err(ImpersonateError::Incomplete {
            what: "HTTP/2 spec"
        })
    );
}

#[test]
fn a_measured_profile_names_its_browser_and_system() {
    let profile = chrome();
    assert_eq!(profile.kind(), BrowserKind::Chrome);
    assert_eq!(profile.os(), OsType::Macos);
    assert!(profile.header_spec().document.len() > 5);
    assert!(profile.identity().sec_ch_ua.is_some());
}

#[test]
fn a_fingerprint_without_a_user_agent_or_a_chrome_version_is_refused() {
    let mut fingerprint = Fingerprint::randomized_with_major(OsType::Macos, 3, 155);
    fingerprint.user_agent = None;
    assert!(matches!(
        ImpersonationProfile::from_fingerprint(&fingerprint),
        Err(ImpersonateError::Fingerprint(_))
    ));
    fingerprint.user_agent = Some("Mozilla/5.0 (X11) Gecko/20100101 Firefox/130.0".to_owned());
    assert!(matches!(
        ImpersonationProfile::from_fingerprint(&fingerprint),
        Err(ImpersonateError::Fingerprint(_))
    ));
}

#[test]
fn newest_measured_chrome_is_the_default() {
    let newest = ImpersonationProfile::measured(BrowserKind::Chrome).expect("measured");
    assert_eq!(newest, chrome());
}

#[test]
fn an_unmeasured_system_is_refused_not_guessed() {
    for os in [OsType::Windows, OsType::Linux, OsType::Android, OsType::Ios] {
        let error = ImpersonationProfile::for_system(BrowserKind::Chrome, os)
            .expect_err("only macOS is measured");
        assert_eq!(
            error,
            ImpersonateError::Emulation(EmulationError::NotMeasured {
                kind: BrowserKind::Chrome,
                os
            })
        );
    }
}

#[test]
fn the_document_headers_equal_what_chrome_sent() {
    let rendered = chrome()
        .headers_for(RequestShape::Document, None, WireProtocol::Http2)
        .expect("renders");
    assert_eq!(lowercase(rendered), evidence("document"));
}

#[test]
fn the_fetch_headers_equal_what_chrome_sent_for_the_headers_the_spec_names() {
    let profile = chrome();
    let rendered = profile
        .headers_for(RequestShape::Fetch, None, WireProtocol::Http2)
        .expect("renders");
    let spec_names: Vec<String> = profile
        .header_spec()
        .fetch
        .iter()
        .map(|(name, _)| name.to_ascii_lowercase())
        .chain(["priority".to_owned()])
        .collect();
    let sent: Vec<(String, String)> = evidence("fetch")
        .into_iter()
        .filter(|(name, _)| spec_names.contains(name))
        .collect();
    assert_eq!(lowercase(rendered), sent);
}

#[test]
fn http1_requests_carry_no_priority_header() {
    let profile = chrome();
    let h1 = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http1)
        .expect("renders");
    let h2 = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http2)
        .expect("renders");
    assert!(h1.iter().all(|(name, _)| name != "priority"));
    assert_eq!(h2.len(), h1.len() + 1);
    assert_eq!(h2.last().map(|(name, _)| name.as_str()), Some("priority"));
    assert_eq!(h1.as_slice(), h2.get(..h1.len()).expect("a prefix"));
}

#[test]
fn a_cookie_is_added_per_request_in_its_place_and_never_kept() {
    let mut profile = chrome();
    profile.headers = profile
        .headers
        .with_header(RequestShape::Document, "Cookie", HeaderValue::Cookie)
        .expect("valid");
    let with = profile
        .headers_for(RequestShape::Document, Some("sid=1"), WireProtocol::Http1)
        .expect("renders");
    assert!(with
        .iter()
        .any(|(name, value)| name == "Cookie" && value == "sid=1"));
    let without = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http1)
        .expect("renders");
    assert!(without.iter().all(|(name, _)| name != "Cookie"));
    assert!(profile.identity().cookie.is_none());
    assert!(!format!("{:?}", profile.identity()).contains("sid=1"));
}

#[test]
fn a_cookie_without_a_slot_goes_after_accept_language() {
    let profile = chrome();
    assert!(profile
        .header_spec()
        .document
        .iter()
        .all(|(name, _)| !name.eq_ignore_ascii_case("cookie")));
    let list = profile
        .headers_for(RequestShape::Document, Some("sid=1"), WireProtocol::Http2)
        .expect("renders");
    let names: Vec<&str> = list.iter().map(|(name, _)| name.as_str()).collect();
    let language = names
        .iter()
        .position(|name| name.eq_ignore_ascii_case("accept-language"))
        .expect("Chrome sends a language");
    assert_eq!(names.get(language + 1), Some(&"Cookie"));
    assert_eq!(names.last(), Some(&"priority"), "priority stays last");
    let without = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http2)
        .expect("renders");
    assert_eq!(without.len() + 1, list.len());
}

#[test]
fn the_client_hints_header_has_the_shape_chrome_writes() {
    let brands = [
        BrandVersion::new("Google Chrome", "155"),
        BrandVersion::new("Not(A:Brand", "24"),
    ];
    assert_eq!(
        sec_ch_ua(&brands),
        "\"Google Chrome\";v=\"155\", \"Not(A:Brand\";v=\"24\""
    );
    assert_eq!(quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
}

#[test]
fn a_user_agent_names_its_chrome_major() {
    assert_eq!(
        user_agent_major("Mozilla/5.0 (X11) AppleWebKit/537.36 Chrome/155.0.0.0 Safari/537.36"),
        Some(155)
    );
    assert_eq!(user_agent_major("Mozilla/5.0 Firefox/130.0"), None);
    assert_eq!(user_agent_major("Chrome/"), None);
}

#[test]
fn a_random_profile_rides_the_measured_chrome_and_claims_its_version() {
    let profile = ImpersonationProfile::random(7).expect("builds");
    assert_eq!(profile.major(), 155);
    assert!(profile.user_agent().contains("Chrome/155."));
    assert_eq!(profile.tls_spec(), chrome().tls_spec());
    assert_eq!(profile, ImpersonationProfile::random(7).expect("builds"));
}

#[test]
fn a_fingerprint_that_claims_another_chrome_is_refused() {
    let mut fingerprint = Fingerprint::randomized_with_major(OsType::Macos, 3, 155);
    fingerprint.user_agent = fingerprint
        .user_agent
        .as_deref()
        .map(|agent| agent.replace("Chrome/155", "Chrome/153"));
    assert_eq!(
        ImpersonationProfile::from_fingerprint(&fingerprint),
        Err(ImpersonateError::VersionMismatch {
            browser: 153,
            measured: 155
        })
    );
}

#[test]
fn a_fingerprint_of_an_unmeasured_system_or_another_browser_is_refused() {
    let windows = Fingerprint::randomized_with_major(OsType::Windows, 3, 155);
    assert!(matches!(
        ImpersonationProfile::from_fingerprint(&windows),
        Err(ImpersonateError::Emulation(
            EmulationError::NotMeasured { .. }
        ))
    ));
    let mut firefox = Fingerprint::randomized_with_major(OsType::Macos, 3, 155);
    firefox.browser_type = crate::stealth::fingerprint::BrowserType::Firefox;
    assert!(matches!(
        ImpersonationProfile::from_fingerprint(&firefox),
        Err(ImpersonateError::Fingerprint(_))
    ));
}

#[test]
fn a_fingerprint_gives_its_identity_to_the_headers() {
    let fingerprint = Fingerprint::randomized_with_major(OsType::Macos, 11, 155);
    let profile = ImpersonationProfile::from_fingerprint(&fingerprint).expect("builds");
    assert_eq!(
        Some(profile.user_agent()),
        fingerprint.user_agent.as_deref()
    );
    assert_eq!(
        profile.accept_language(),
        fingerprint.accept_languages.as_deref()
    );
    let hints = fingerprint.client_hints.as_ref().expect("Chrome hints");
    assert_eq!(profile.identity().sec_ch_ua, Some(sec_ch_ua(&hints.brands)));
}

#[test]
fn a_locale_sets_the_accept_language() {
    let german = Locale::by_tag("de-DE").expect("a known locale");
    let profile = chrome().with_locale(german);
    assert_eq!(
        profile.accept_language(),
        Some(german.accept_language().as_str())
    );
}

fn base_ja3() -> String {
    chrome().tls_spec().ja3().to_string()
}

#[test]
fn a_custom_ja3_is_accepted_and_pins_the_extension_order() {
    let ja3 = base_ja3();
    let profile = ImpersonationProfile::custom()
        .ja3(&ja3)
        .expect("accepted")
        .build()
        .expect("builds");
    assert!(!profile.tls_spec().permute_extensions);
    assert_eq!(profile.tls_spec().ja3().to_string(), ja3);
}

#[test]
fn a_ja3_with_an_extension_the_stack_cannot_send_is_refused_by_name() {
    let with_trust_anchors = base_ja3().replace("-43-", "-51764-43-");
    let error = ImpersonationProfile::custom()
        .ja3(&with_trust_anchors)
        .expect_err("0xCA34 cannot be emitted");
    assert!(matches!(
        error,
        ImpersonateError::Tls(TlsSpecError::UnsupportedExtension { id: 0xCA34, .. })
    ));
}

#[test]
fn a_malformed_fingerprint_is_refused_by_name() {
    assert!(matches!(
        ImpersonationProfile::custom().ja3("771,4865"),
        Err(ImpersonateError::Ja3(_))
    ));
    assert!(matches!(
        ImpersonationProfile::custom().ja4_r("t13"),
        Err(ImpersonateError::Ja4(_))
    ));
    assert!(matches!(
        ImpersonationProfile::custom().peetprint("1|2"),
        Err(ImpersonateError::Peetprint(_))
    ));
    assert!(matches!(
        ImpersonationProfile::custom().akamai("1:2"),
        Err(ImpersonateError::Akamai(_))
    ));
}

#[test]
fn a_ja3_without_what_boringssl_always_sends_is_refused_by_name() {
    let without_groups_extension = base_ja3().replacen("-10-", "-", 1);
    let error = ImpersonationProfile::custom()
        .ja3(&without_groups_extension)
        .expect_err("supported_groups is always sent");
    assert!(
        matches!(
            error,
            ImpersonateError::Convert(ConvertError::ExtensionAlwaysSent(0x000A))
                | ImpersonateError::Tls(_)
        ),
        "{error:?}"
    );
}

#[test]
fn a_custom_ja4_r_and_ja4_ro_round_trip_through_the_base() {
    let base = chrome();
    let sorted = base
        .tls_spec()
        .ja4_raw(crate::wire::ja4::Ja4Order::Sorted)
        .to_string();
    let original = base
        .tls_spec()
        .ja4_raw(crate::wire::ja4::Ja4Order::Original)
        .to_string();
    let from_sorted = ImpersonationProfile::custom()
        .ja4_r(&sorted)
        .expect("accepted")
        .build()
        .expect("builds");
    assert_eq!(from_sorted.tls_spec().ja4(), base.tls_spec().ja4());
    let from_original = ImpersonationProfile::custom()
        .ja4_ro(&original)
        .expect("accepted")
        .build()
        .expect("builds");
    assert!(!from_original.tls_spec().permute_extensions);
    assert_eq!(
        from_original.tls_spec().extensions,
        base.tls_spec().extensions
    );
}

#[test]
fn custom_signature_algorithms_are_checked() {
    let profile = ImpersonationProfile::custom()
        .signature_algorithms(&[0x0403, 0x0804])
        .expect("accepted")
        .build()
        .expect("builds");
    assert_eq!(profile.tls_spec().signature_algorithms, [0x0403, 0x0804]);
    let refused = ImpersonationProfile::custom().signature_algorithms(&[0x0904]);
    assert!(matches!(
        refused,
        Err(ImpersonateError::Tls(TlsSpecError::UnsupportedSigalg {
            id: 0x0904,
            ..
        }))
    ));
}

#[test]
fn a_custom_akamai_string_replaces_the_http2_start_and_keeps_the_headers_priority() {
    let akamai = "1:65536;4:131072;5:16384|12517377|3:0:0:201,5:0:0:101|m,p,a,s";
    let base = chrome();
    let profile = ImpersonationProfile::custom()
        .akamai(akamai)
        .expect("accepted")
        .build()
        .expect("builds");
    assert_eq!(profile.http2_spec().akamai().to_string(), akamai);
    assert_eq!(profile.http2_spec().initial_stream_id, Some(7));
    assert_eq!(
        profile.http2_spec().headers_priority,
        base.http2_spec().headers_priority
    );
    assert_eq!(profile.tls_spec(), base.tls_spec());
}

#[test]
fn http2_settings_wreq_cannot_send_are_refused_by_name() {
    let grease = ImpersonationProfile::custom().akamai("2570:1;4:1048576|0|0|m,a,s,p");
    assert!(matches!(
        grease,
        Err(ImpersonateError::Http2(H2SpecError::UnsupportedSetting(
            2570
        )))
    ));
    let no_window = ImpersonationProfile::custom().akamai("1:4096|0|0|m,a,s,p");
    assert!(matches!(
        no_window,
        Err(ImpersonateError::Http2(H2SpecError::SettingAlwaysSent(4)))
    ));
    let big_window = ImpersonationProfile::custom().akamai("4:65535|2147483647|0|m,a,s,p");
    assert!(matches!(
        big_window,
        Err(ImpersonateError::Http2(H2SpecError::WindowUpdate(_)))
    ));
}

#[test]
fn identity_inputs_replace_the_bases() {
    let profile = ImpersonationProfile::custom()
        .user_agent("Custom/1.0")
        .accept_language("fr-FR,fr;q=0.8")
        .build()
        .expect("builds");
    assert_eq!(profile.user_agent(), "Custom/1.0");
    assert_eq!(profile.accept_language(), Some("fr-FR,fr;q=0.8"));
    let rendered = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http1)
        .expect("renders");
    let get = |name: &str| {
        rendered
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(get("user-agent"), Some("Custom/1.0"));
    assert_eq!(get("accept-language"), Some("fr-FR,fr;q=0.8"));
}

#[test]
fn header_inputs_change_names_order_and_values() {
    let profile = ImpersonationProfile::custom()
        .header(
            RequestShape::Document,
            "X-Extra",
            HeaderValue::Literal("1".to_owned()),
        )
        .header(
            RequestShape::Document,
            "accept",
            HeaderValue::Literal("text/plain".to_owned()),
        )
        .without_header(RequestShape::Document, "Sec-Fetch-User")
        .header_order(RequestShape::Document, ["Accept-Language", "User-Agent"])
        .build()
        .expect("builds");
    let rendered = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http1)
        .expect("renders");
    let names: Vec<&str> = rendered.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names.first(), Some(&"Accept-Language"));
    assert_eq!(names.get(1), Some(&"User-Agent"));
    assert_eq!(names.last(), Some(&"X-Extra"));
    assert!(!names.contains(&"Sec-Fetch-User"));
    let accept = rendered
        .iter()
        .find(|(name, _)| name == "Accept")
        .expect("kept");
    assert_eq!(accept.1, "text/plain");
}

#[test]
fn header_inputs_that_cannot_be_sent_are_refused_by_name() {
    let reserved = ImpersonationProfile::custom()
        .header(
            RequestShape::Document,
            "Host",
            HeaderValue::Literal("x".to_owned()),
        )
        .build();
    assert!(matches!(reserved, Err(ImpersonateError::Headers(_))));
    let unknown = ImpersonationProfile::custom()
        .header_order(RequestShape::Fetch, ["X-Missing"])
        .build();
    assert_eq!(
        unknown,
        Err(ImpersonateError::UnknownHeader("X-Missing".to_owned()))
    );
}

#[test]
fn the_base_can_be_named_and_the_header_lists_replaced_as_a_whole() {
    let replacement = crate::wire::header_spec::HeaderSpec {
        document: vec![("X-Only".to_owned(), HeaderValue::Literal("1".to_owned()))],
        ..Default::default()
    };
    let profile = ImpersonationProfile::custom()
        .base(BrowserKind::Chrome)
        .base_on(BrowserKind::Chrome, OsType::Macos)
        .headers(replacement)
        .build()
        .expect("builds");
    let rendered = profile
        .headers_for(RequestShape::Document, None, WireProtocol::Http1)
        .expect("renders");
    assert_eq!(rendered, [("X-Only".to_owned(), "1".to_owned())]);
    assert_eq!(profile.source(), "custom over chrome-155-macos");
}

#[test]
fn a_base_with_no_capture_is_refused_when_the_builder_needs_it() {
    let error = ImpersonationProfile::custom()
        .base_on(BrowserKind::Chrome, OsType::Windows)
        .build()
        .expect_err("Windows is not measured");
    assert!(matches!(
        error,
        ImpersonateError::Emulation(EmulationError::NotMeasured { .. })
    ));
}

#[test]
fn the_client_builder_accepts_every_measured_and_custom_profile() {
    impersonation_client(&chrome())
        .expect("builds")
        .build()
        .expect("a client");
    let custom = ImpersonationProfile::custom()
        .ja3(&base_ja3())
        .expect("accepted")
        .akamai("1:65536;4:131072;5:16384|12517377|3:0:0:201|m,p,a,s")
        .expect("accepted")
        .build()
        .expect("builds");
    impersonation_client(&custom)
        .expect("builds")
        .build()
        .expect("a client");
}
