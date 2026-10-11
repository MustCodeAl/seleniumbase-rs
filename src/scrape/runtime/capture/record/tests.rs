//! Tests of the capture record types, the measurement and the JSON file.

use chrono::TimeZone;
use serde_json::json;

use super::*;
use crate::stealth::fingerprint::{BrandVersion, ClientHints};
use crate::wire::client_hello::fixtures::{
    length16, strings8, u16s, HelloBuilder, CHROME_CIPHERS, CHROME_SIGALGS, JA4_EXAMPLE_EXTENSIONS,
};

/// A user agent in the format Chrome uses on macOS, as parser input.
const MAC_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                      (KHTML, like Gecko) Chrome/100.0.0.0 Safari/537.36";

fn hello(order: &[u16]) -> Vec<u8> {
    HelloBuilder::new(&CHROME_CIPHERS, order)
        .with_grease()
        .record()
}

fn reversed() -> Vec<u16> {
    let mut ids = JA4_EXAMPLE_EXTENSIONS.to_vec();
    ids.reverse();
    ids
}

fn rotated() -> Vec<u16> {
    let mut ids = JA4_EXAMPLE_EXTENSIONS.to_vec();
    ids.rotate_left(5);
    ids
}

fn id() -> EntryId {
    EntryId {
        kind: BrowserKind::Chrome,
        major: 155,
        os: OsType::Macos,
    }
}

fn version() -> BrowserVersion {
    BrowserVersion::from_get_version(&json!({
        "product": "Chrome/155.0.8059.40",
        "userAgent": MAC_UA,
    }))
    .expect("a Chrome version")
}

fn permuted_measurement() -> TlsMeasurement {
    let page = vec![
        hello(&JA4_EXAMPLE_EXTENSIONS),
        hello(&reversed()),
        hello(&rotated()),
    ];
    let websocket = HelloBuilder::chrome_like()
        .with_body(ext_id::ALPN, length16(&strings8(&["http/1.1"])))
        .with_grease()
        .record();
    TlsMeasurement::from_hellos(&page, Some(&websocket)).expect("one browser")
}

fn hints_measurement() -> HintsMeasurement {
    HintsMeasurement {
        user_agent: MAC_UA.to_owned(),
        accept_language: "en-US,en;q=0.9".to_owned(),
        languages: vec!["en-US".to_owned(), "en".to_owned()],
        client_hints: ClientHints {
            brands: vec![BrandVersion::new("Chromium", "100")],
            full_version_list: vec![BrandVersion::new("Chromium", "100.0.1.2")],
            platform: "macOS".to_owned(),
            platform_version: "15.0.0".to_owned(),
            architecture: "arm".to_owned(),
            bitness: "64".to_owned(),
            model: String::new(),
            ua_full_version: "100.0.1.2".to_owned(),
            mobile: false,
            wow64: false,
            form_factors: vec!["Desktop".to_owned()],
        },
        first_visit_headers: vec![("sec-ch-ua".to_owned(), "\"Chromium\";v=\"100\"".to_owned())],
        request_headers: vec![("user-agent".to_owned(), MAC_UA.to_owned())],
    }
}

#[test]
fn hex_round_trips_in_either_case() {
    assert_eq!(to_hex(&[0, 255, 16]), "00ff10");
    assert_eq!(from_hex("00ff10").expect("hex"), [0, 255, 16]);
    assert_eq!(from_hex("00FF10").expect("hex"), [0, 255, 16]);
    assert_eq!(from_hex("").expect("empty"), Vec::<u8>::new());
}

#[test]
fn malformed_hex_is_refused_without_panicking() {
    for bad in ["abc", "zz", "0g", "é1", "0x00", " 00"] {
        assert!(matches!(from_hex(bad), Err(CaptureError::Hex)), "{bad:?}");
    }
}

#[test]
fn entry_names_follow_browser_major_and_os() {
    assert_eq!(id().file_name(), "chrome-155-macos.json");
    assert_eq!(id().stem(), "chrome-155-macos");
    let slugs = [
        (OsType::Windows, "chrome-9-windows"),
        (OsType::Linux, "chrome-9-linux"),
        (OsType::Android, "chrome-9-android"),
        (OsType::Ios, "chrome-9-ios"),
    ];
    for (os, stem) in slugs {
        let entry = EntryId {
            major: 9,
            os,
            ..id()
        };
        assert_eq!(entry.stem(), stem);
    }
}

#[test]
fn a_version_is_read_from_the_cdp_result() {
    let version = version();

    assert_eq!(version.product, "Chrome/155.0.8059.40");
    assert_eq!(version.full_version, "155.0.8059.40");
    assert_eq!(version.major, 155);
    assert_eq!(version.os().expect("macOS"), OsType::Macos);
    assert!(!version.is_headless());
}

#[test]
fn a_headless_browser_is_recognised() {
    let version = BrowserVersion::from_get_version(&json!({
        "product": "HeadlessChrome/155.0.8059.40",
        "userAgent": MAC_UA.replace("Chrome/", "HeadlessChrome/"),
    }))
    .expect("headless is readable");

    assert!(version.is_headless());
    assert_eq!(version.major, 155);
}

#[test]
fn products_this_tool_cannot_read_are_refused() {
    for result in [
        json!({"product": "Firefox/130.0", "userAgent": MAC_UA}),
        json!({"product": "Chrome/abc", "userAgent": MAC_UA}),
        json!({"product": "Chrome/", "userAgent": MAC_UA}),
        json!({"userAgent": MAC_UA}),
        json!({"product": "Chrome/155.0.1"}),
    ] {
        let error = BrowserVersion::from_get_version(&result).expect_err("unreadable");
        assert!(
            matches!(error, CaptureError::Product(_)),
            "{result}: {error}"
        );
    }
}

#[test]
fn the_operating_system_comes_from_the_user_agent() {
    let cases = [
        (
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/100.0.0.0",
            Some(OsType::Windows),
        ),
        (
            "Mozilla/5.0 (X11; Linux x86_64) Chrome/100.0.0.0",
            Some(OsType::Linux),
        ),
        (
            "Mozilla/5.0 (Linux; Android 14; Pixel) Chrome/100.0.0.0 Mobile",
            Some(OsType::Android),
        ),
        (
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Chrome/100.0.0.0",
            Some(OsType::Macos),
        ),
        ("Mozilla/5.0 (Unknown) Chrome/100.0.0.0", None),
    ];
    for (user_agent, expected) in cases {
        let version = BrowserVersion {
            product: "Chrome/100.0.0.0".to_owned(),
            full_version: "100.0.0.0".to_owned(),
            major: 100,
            user_agent: user_agent.to_owned(),
        };
        match expected {
            Some(os) => assert_eq!(version.os().expect("known"), os, "{user_agent}"),
            None => assert!(matches!(version.os(), Err(CaptureError::UnknownOs))),
        }
    }
}

#[test]
fn a_record_holds_the_fingerprints_of_its_bytes() {
    let raw = hello(&JA4_EXAMPLE_EXTENSIONS);
    let parsed = ClientHello::parse(&raw).expect("a hello");

    let record = HelloRecord::of(&raw).expect("a record");

    assert_eq!(record.sni.as_deref(), Some("example.com"));
    assert_eq!(record.ja3, Ja3::of(&parsed).to_string());
    assert_eq!(record.ja3_hash, Ja3::of(&parsed).hash());
    assert_eq!(record.ja4, Ja4::of(&parsed).to_string());
    assert_eq!(record.ja4_r, Ja4::raw(&parsed));
    assert_eq!(record.ja4_ro, Ja4::raw_original_order(&parsed));
    assert_eq!(record.peetprint, Peetprint::of(&parsed).to_string());
    assert_eq!(record.peetprint_hash, Peetprint::of(&parsed).hash());
    assert!(record.ja4.starts_with("t13d1516h2_"), "{}", record.ja4);
    assert_eq!(record.raw().expect("hex"), raw);
    assert_eq!(record.hello().expect("parses"), parsed);
    let json = serde_json::to_string(&record).expect("serializes");
    assert_eq!(
        serde_json::from_str::<HelloRecord>(&json).expect("parses"),
        record
    );
}

#[test]
fn bytes_that_are_not_a_hello_make_no_record() {
    let error = HelloRecord::of(b"GET / HTTP/1.1\r\n").expect_err("not TLS");

    assert!(matches!(error, CaptureError::Hello(_)), "{error}");
}

#[test]
fn a_browser_that_shuffles_its_extensions_is_detected() {
    let measurement = permuted_measurement();

    assert!(measurement.spec.permute_extensions);
    assert!(measurement.spec.grease);
    assert_eq!(measurement.spec.extensions, JA4_EXAMPLE_EXTENSIONS);
    assert_eq!(measurement.spec.cipher_suites, CHROME_CIPHERS);
    assert_eq!(measurement.samples.len(), 3);
    let first = measurement.samples.first().expect("three samples");
    for sample in &measurement.samples {
        assert_eq!(sample.ja4, first.ja4, "JA4 ignores the extension order");
    }
    assert_ne!(measurement.samples.get(1).expect("three").ja3, first.ja3);
    assert_eq!(measurement.known_gaps, Vec::<KnownGap>::new());
}

#[test]
fn the_websocket_hello_gets_its_own_spec() {
    let measurement = permuted_measurement();

    let websocket = measurement.websocket_spec.expect("a websocket spec");
    assert_eq!(websocket.alpn, ["http/1.1"]);
    assert_eq!(measurement.spec.alpn, ["h2", "http/1.1"]);
    assert!(measurement.websocket_sample.is_some());
}

#[test]
fn hellos_in_the_same_order_do_not_show_a_shuffle() {
    let same = hello(&JA4_EXAMPLE_EXTENSIONS);

    let measurement =
        TlsMeasurement::from_hellos(&[same.clone(), same.clone(), same], None).expect("stable");

    assert!(!measurement.spec.permute_extensions);
    assert!(measurement.websocket_spec.is_none());
    assert!(measurement.websocket_sample.is_none());
}

#[test]
fn one_hello_is_enough_for_a_spec() {
    let measurement =
        TlsMeasurement::from_hellos(&[hello(&JA4_EXAMPLE_EXTENSIONS)], None).expect("one");

    assert_eq!(measurement.samples.len(), 1);
    assert!(!measurement.spec.permute_extensions);
}

/// Chrome 155 puts a GREASE value at the head of `signature_algorithms`.
const GREASE_SIGALG: u16 = 0x9A9A;

fn hello_with_grease_sigalg(extension_order: &[u16]) -> Vec<u8> {
    let mut sigalgs = vec![GREASE_SIGALG];
    sigalgs.extend_from_slice(&CHROME_SIGALGS);
    HelloBuilder::new(&CHROME_CIPHERS, extension_order)
        .with_body(ext_id::SIGNATURE_ALGORITHMS, length16(&u16s(&sigalgs)))
        .with_grease()
        .record()
}

#[test]
fn a_grease_signature_algorithm_is_kept_in_the_evidence_but_not_in_the_spec() {
    let raw = hello_with_grease_sigalg(&JA4_EXAMPLE_EXTENSIONS);
    let websocket = hello_with_grease_sigalg(&reversed());

    let measurement = TlsMeasurement::from_hellos(std::slice::from_ref(&raw), Some(&websocket))
        .expect("one browser");

    assert_eq!(measurement.spec.signature_algorithms, CHROME_SIGALGS);
    assert_eq!(
        measurement
            .websocket_spec
            .as_ref()
            .map(|spec| spec.signature_algorithms.clone()),
        Some(CHROME_SIGALGS.to_vec())
    );
    assert!(
        measurement.spec.grease,
        "the flag still says the browser GREASEs"
    );
    let sample = measurement.samples.first().expect("one sample");
    assert_eq!(sample.raw().expect("hex"), raw, "the evidence is untouched");
    let hello = sample.hello().expect("parses");
    assert_eq!(hello.signature_algorithms.first(), Some(&GREASE_SIGALG));
    assert_eq!(
        sample.ja4,
        Ja4::of(&hello).to_string(),
        "JA4 already ignores the GREASE value"
    );
}

#[test]
fn no_hello_is_an_error() {
    let error = TlsMeasurement::from_hellos(&[], None).expect_err("nothing");

    assert!(matches!(error, CaptureError::NoHello { .. }), "{error}");
}

#[test]
fn hellos_of_different_clients_are_refused() {
    let other = HelloBuilder::new(&CHROME_CIPHERS[1..], &JA4_EXAMPLE_EXTENSIONS)
        .with_grease()
        .record();

    let error = TlsMeasurement::from_hellos(&[hello(&JA4_EXAMPLE_EXTENSIONS), other], None)
        .expect_err("different ciphers");

    assert!(matches!(error, CaptureError::Spec(_)), "{error}");
}

#[test]
fn hellos_with_different_ja4_are_unstable() {
    let mut sigalgs = CHROME_SIGALGS.to_vec();
    sigalgs.swap(0, 1);
    let other = HelloBuilder::new(&CHROME_CIPHERS, &JA4_EXAMPLE_EXTENSIONS)
        .with_body(ext_id::SIGNATURE_ALGORITHMS, length16(&u16s(&sigalgs)))
        .with_grease()
        .record();

    let error = TlsMeasurement::from_hellos(&[hello(&JA4_EXAMPLE_EXTENSIONS), other], None)
        .expect_err("different signature algorithms");

    assert!(matches!(error, CaptureError::Unstable { .. }), "{error}");
}

#[test]
fn what_boringssl_cannot_send_is_recorded_as_known_gaps() {
    let mut extensions = JA4_EXAMPLE_EXTENSIONS.to_vec();
    extensions.push(ext_id::TRUST_ANCHORS);
    let raw = HelloBuilder::new(&CHROME_CIPHERS, &extensions)
        .with_body(
            ext_id::SIGNATURE_ALGORITHMS,
            length16(&u16s(&[0x0403, 0x0904])),
        )
        .with_grease()
        .record();

    let measurement = TlsMeasurement::from_hellos(&[raw], None).expect("one hello");

    assert_eq!(
        measurement.known_gaps,
        [
            KnownGap::TrustAnchorsExtension,
            KnownGap::MlDsaSignatureAlgorithms
        ]
    );
}

#[test]
fn a_new_measured_file_is_empty() {
    let dir = tempfile::tempdir().expect("a temporary directory");

    let file = MeasuredFile::open(dir.path(), id()).expect("a missing file is an empty document");

    assert!(file.get("tls").is_none());
    assert_eq!(file.path(), dir.path().join("chrome-155-macos.json"));
}

#[test]
fn the_tools_fill_one_file_between_them() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let measurement = permuted_measurement();
    let now = chrono::Utc
        .with_ymd_and_hms(2026, 10, 11, 9, 30, 0)
        .single()
        .expect("a valid time");
    let mut file = MeasuredFile::open(dir.path(), id()).expect("open");

    file.set_identity(id(), &version());
    file.set_tls(&measurement).expect("tls");
    file.stamp("capture_tls 0.1.0", &version(), now);
    file.set_hints(&hints_measurement()).expect("hints");
    file.stamp("capture_hints 0.1.0", &version(), now);
    file.save().expect("saves");

    let reread = MeasuredFile::open(dir.path(), id()).expect("reopens");
    assert_eq!(
        reread.to_pretty_json().expect("json"),
        file.to_pretty_json().expect("json")
    );
    assert!(file.to_pretty_json().expect("json").ends_with("}\n"));
    assert_eq!(reread.get("kind"), Some(&json!("Chrome")));
    assert_eq!(reread.get("major"), Some(&json!(155)));
    assert_eq!(reread.get("full_version"), Some(&json!("155.0.8059.40")));
    assert_eq!(reread.get("os"), Some(&json!("Macos")));
    assert_eq!(reread.get("user_agent"), Some(&json!(MAC_UA)));
    let tls: TlsSpec =
        serde_json::from_value(reread.get("tls").expect("tls").clone()).expect("tls is a TlsSpec");
    assert_eq!(tls, measurement.spec);
    let websocket: TlsSpec =
        serde_json::from_value(reread.get("tls_websocket").expect("tls_websocket").clone())
            .expect("tls_websocket is a TlsSpec");
    assert_eq!(Some(websocket), measurement.websocket_spec);
    let hints: ClientHints =
        serde_json::from_value(reread.get("client_hints").expect("hints").clone())
            .expect("client_hints is ClientHints");
    assert_eq!(hints, hints_measurement().client_hints);
    assert_eq!(
        reread.get("provenance"),
        Some(&json!({
            "captured_on": "2026-10-11",
            "tool": "capture_hints 0.1.0; capture_tls 0.1.0",
            "browser": "Chrome 155.0.8059.40",
            "known_gaps": [],
        }))
    );
    let samples = &reread.get("capture").expect("capture")["tls"]["samples"];
    assert_eq!(samples.as_array().expect("an array").len(), 3);
    assert!(reread.get("capture").expect("capture")["hints"]["request_headers"].is_array());
}

#[test]
fn a_tool_keeps_the_keys_it_does_not_own() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("chrome-155-macos.json");
    std::fs::write(&path, r#"{"http2": {"window_update": 1}, "tls": "stale"}"#).expect("seeded");
    let mut file = MeasuredFile::open(dir.path(), id()).expect("open");

    file.set_tls(&permuted_measurement()).expect("tls");

    assert_eq!(file.get("http2"), Some(&json!({"window_update": 1})));
    assert!(file.get("tls").expect("tls")["cipher_suites"].is_array());
}

#[test]
fn known_gaps_survive_a_later_stamp() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let mut extensions = JA4_EXAMPLE_EXTENSIONS.to_vec();
    extensions.push(ext_id::TRUST_ANCHORS);
    let raw = HelloBuilder::new(&CHROME_CIPHERS, &extensions)
        .with_grease()
        .record();
    let measurement = TlsMeasurement::from_hellos(&[raw], None).expect("one hello");
    let now = chrono::Utc::now();
    let mut file = MeasuredFile::open(dir.path(), id()).expect("open");

    file.set_tls(&measurement).expect("tls");
    file.stamp("capture_tls 0.1.0", &version(), now);
    file.stamp("capture_hints 0.1.0", &version(), now);

    assert_eq!(
        file.get("provenance").expect("provenance")["known_gaps"],
        json!(["TrustAnchorsExtension"])
    );
}

#[test]
fn a_file_that_is_not_a_json_object_is_refused() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("chrome-155-macos.json");

    std::fs::write(&path, "[1, 2]").expect("seeded");
    assert!(matches!(
        MeasuredFile::open(dir.path(), id()),
        Err(CaptureError::NotAnObject { .. })
    ));

    std::fs::write(&path, "{not json").expect("seeded");
    assert!(matches!(
        MeasuredFile::open(dir.path(), id()),
        Err(CaptureError::Json(_))
    ));
}

#[test]
fn saving_creates_the_directory() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let nested = dir.path().join("a").join("b");
    let mut file = MeasuredFile::open(&nested, id()).expect("open");
    file.set_identity(id(), &version());

    file.save().expect("saves into a new directory");

    assert!(nested.join("chrome-155-macos.json").is_file());
}

#[test]
fn tool_names_are_merged_by_name() {
    assert_eq!(merge_tool(None, "capture_tls 0.1.0"), "capture_tls 0.1.0");
    assert_eq!(
        merge_tool(Some("capture_tls 0.1.0"), "capture_hints 0.1.0"),
        "capture_hints 0.1.0; capture_tls 0.1.0"
    );
    assert_eq!(
        merge_tool(
            Some("capture_hints 0.1.0; capture_tls 0.1.0"),
            "capture_tls 0.2.0"
        ),
        "capture_hints 0.1.0; capture_tls 0.2.0"
    );
    assert_eq!(
        merge_tool(Some(""), "capture_tls 0.1.0"),
        "capture_tls 0.1.0",
        "an empty list has no entries"
    );
}

#[test]
fn raw_fixtures_are_the_captured_bytes() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let measurement = permuted_measurement();

    let written = write_fixtures(dir.path(), id(), &measurement).expect("written");

    let names: Vec<String> = written
        .iter()
        .map(|path| {
            path.file_name()
                .expect("a file")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        names,
        [
            "chrome-155-macos.client_hello.1.bin",
            "chrome-155-macos.client_hello.2.bin",
            "chrome-155-macos.client_hello.3.bin",
            "chrome-155-macos.websocket_hello.bin",
        ]
    );
    for (path, sample) in written.iter().zip(&measurement.samples) {
        assert_eq!(
            std::fs::read(path).expect("readable"),
            sample.raw().expect("hex")
        );
    }
}
