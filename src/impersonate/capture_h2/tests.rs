//! The capture's pure parts on synthetic bytes, and its site on loopback sockets. None of it
//! needs a browser; the real-Chrome run is `tests/impersonate_capture_h2_chrome.rs`.
//!
//! The values below are made up for the tests (a stand-in browser's settings and headers). The
//! measured values live in `src/wire/measured` and are checked by `tests/wire_emulation.rs`.

mod site;

use std::path::Path;

use super::assemble::{derive, h1_evidence, h2_evidence, H1Evidence, H2Evidence};
use super::probes::{Probe, DOCUMENT, FETCH, IMAGE, PROBES, SCRIPT, STYLE};
use super::site::{Observed, Seen};
use super::store::{write_h2_fixtures, MeasuredDoc};
use super::{parse_args, CaptureH2Error, Passes};
use crate::scrape::runtime::capture::record::{BrowserVersion, EntryId};
use crate::scrape::runtime::capture::stream::Http1Request;
use crate::stealth::fingerprint::OsType;
use crate::wire::akamai::Pseudo;
use crate::wire::h2_frames::{encode_frame, frame_type, read_preface, CLIENT_PREFACE};
use crate::wire::BrowserKind;

/// Frame flags used to build a stand-in browser's connection start.
const END_STREAM: u8 = 0x1;
const END_HEADERS: u8 = 0x4;
const HAS_PRIORITY: u8 = 0x20;
/// The SETTINGS the stand-in browser sends: header table, push, initial window.
const SETTINGS: [(u16, u32); 3] = [(1, 4096), (2, 0), (4, 1_048_576)];
/// The connection window increment the stand-in browser sends.
const WINDOW_UPDATE: u32 = 983_041;

/// A literal HPACK field without indexing, name and value spelled out.
pub(super) fn literal(name: &str, value: &str) -> Vec<u8> {
    let mut field = vec![0x00, u8::try_from(name.len()).expect("short name")];
    field.extend_from_slice(name.as_bytes());
    field.push(u8::try_from(value.len()).expect("short value"));
    field.extend_from_slice(value.as_bytes());
    field
}

/// The bytes a client sends to start a connection and make one request: the preface, SETTINGS, a
/// connection `WINDOW_UPDATE`, and HEADERS on stream 1 with the priority fields `priority`.
pub(super) fn h2_start(
    settings: &[(u16, u32)],
    headers: &[(&str, &str)],
    priority: Option<(u32, u8)>,
) -> Vec<u8> {
    let mut bytes = CLIENT_PREFACE.to_vec();
    let mut payload = Vec::new();
    for (id, value) in settings {
        payload.extend_from_slice(&id.to_be_bytes());
        payload.extend_from_slice(&value.to_be_bytes());
    }
    bytes.extend(encode_frame(frame_type::SETTINGS, 0, 0, &payload).expect("settings"));
    bytes.extend(
        encode_frame(
            frame_type::WINDOW_UPDATE,
            0,
            0,
            &WINDOW_UPDATE.to_be_bytes(),
        )
        .expect("window update"),
    );
    let mut block: Vec<u8> = Vec::new();
    let mut flags = END_HEADERS | END_STREAM;
    if let Some((depends_on, wire_weight)) = priority {
        flags |= HAS_PRIORITY;
        block.extend_from_slice(&depends_on.to_be_bytes());
        block.push(wire_weight);
    }
    for (name, value) in headers {
        block.extend(literal(name, value));
    }
    bytes.extend(encode_frame(frame_type::HEADERS, flags, 1, &block).expect("headers"));
    bytes
}

/// The headers a stand-in browser sends for `probe` over HTTP/2.
pub(super) fn h2_headers(probe: Probe, port: u16) -> Vec<(String, String)> {
    let mut headers = vec![
        (":method", "GET".to_owned()),
        (":authority", format!("{}:{port}", probe.host())),
        (":scheme", "https".to_owned()),
        (":path", "/".to_owned()),
        ("sec-ch-ua", "\"Stand-in\";v=\"1\"".to_owned()),
        ("sec-ch-ua-mobile", "?0".to_owned()),
        ("user-agent", "Stand-in/1.0".to_owned()),
        ("accept", "*/*".to_owned()),
        ("sec-fetch-dest", probe.dest.to_owned()),
        ("accept-encoding", "gzip".to_owned()),
        ("accept-language", "en-US".to_owned()),
    ];
    if probe == FETCH {
        headers.insert(8, ("origin", "https://document.capture.test:1".to_owned()));
        headers.insert(
            9,
            ("referer", "https://document.capture.test:1/".to_owned()),
        );
    }
    headers.push(("priority", priority_for(probe).to_owned()));
    headers
        .into_iter()
        .map(|(name, value)| (name.to_owned(), value))
        .collect()
}

/// The `priority` header the stand-in browser sends for `probe`.
pub(super) fn priority_for(probe: Probe) -> &'static str {
    if probe == DOCUMENT {
        "u=0, i"
    } else {
        "u=1"
    }
}

/// The observation of a stand-in browser's HTTP/2 connection to `probe`.
pub(super) fn h2_observed(probe: Probe, headers: &[(String, String)]) -> Observed {
    let pairs: Vec<(&str, &str)> = headers
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let raw = h2_start(&SETTINGS, &pairs, Some((0, 255)));
    let preface = read_preface(&raw)
        .expect("a well-formed start")
        .expect("a complete request");
    Observed {
        probe,
        seen: Seen::H2 { raw, preface },
    }
}

/// The observation of a stand-in browser's HTTP/1 request to `probe`; header names in the case
/// the browser writes them, `sec-ch-*` in lowercase.
pub(super) fn h1_observed(probe: Probe, headers: &[(String, String)]) -> Observed {
    let mut wire = vec![
        ("Host".to_owned(), format!("{}:1", probe.host())),
        ("Connection".to_owned(), "keep-alive".to_owned()),
    ];
    for (name, value) in headers
        .iter()
        .filter(|(name, _)| !name.starts_with(':') && name != "priority")
    {
        let written = match name.as_str() {
            "user-agent" => "User-Agent",
            "accept" => "Accept",
            "accept-encoding" => "Accept-Encoding",
            "accept-language" => "Accept-Language",
            "sec-fetch-dest" => "Sec-Fetch-Dest",
            "origin" => "Origin",
            "referer" => "Referer",
            other => other,
        };
        wire.push((written.to_owned(), value.clone()));
    }
    Observed {
        probe,
        seen: Seen::Http1 {
            request: Http1Request {
                method: "GET".to_owned(),
                target: "/".to_owned(),
                version: "HTTP/1.1".to_owned(),
                headers: wire,
                body: Vec::new(),
            },
        },
    }
}

/// Every probe, observed over HTTP/2.
fn all_h2() -> Vec<Observed> {
    PROBES
        .into_iter()
        .map(|probe| h2_observed(probe, &h2_headers(probe, 4433)))
        .collect()
}

/// Every probe, observed over HTTP/1.
fn all_h1() -> Vec<Observed> {
    PROBES
        .into_iter()
        .map(|probe| h1_observed(probe, &h2_headers(probe, 4433)))
        .collect()
}

fn evidence() -> (Vec<H2Evidence>, Vec<H1Evidence>) {
    (
        h2_evidence(&all_h2()).expect("h2 evidence"),
        h1_evidence(&all_h1()).expect("h1 evidence"),
    )
}

#[test]
fn the_probes_have_distinct_hosts_that_map_back() {
    let hosts: Vec<String> = PROBES.iter().map(Probe::host).collect();

    for (probe, host) in PROBES.iter().zip(&hosts) {
        assert!(host.ends_with(".capture.test"), "{host}");
        assert_eq!(Probe::for_host(host), Some(*probe));
        assert_eq!(
            Probe::for_host(&host.to_ascii_uppercase()),
            Some(*probe),
            "host names are not case sensitive"
        );
    }
    assert_eq!(
        hosts
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        hosts.len()
    );
    assert!(Probe::for_host("evil.example").is_none());
    assert!(Probe::for_host("capture.test").is_none());
    assert_eq!(DOCUMENT.url(4433), "https://document.capture.test:4433/");
}

#[test]
fn the_page_script_requests_every_other_probe_plainly() {
    let script = super::probes::page_script(4433);

    for probe in [FETCH, IMAGE, SCRIPT, STYLE] {
        assert!(
            script.contains(&probe.host()),
            "{} is requested",
            probe.name
        );
    }
    assert!(
        !script.contains(&DOCUMENT.host()),
        "the navigation is not scripted"
    );
    assert!(
        !script.contains("no-store") && !script.contains("headers"),
        "no option may change the headers the browser sends by default"
    );
}

#[test]
fn evidence_is_taken_from_the_first_connection_of_each_probe() {
    let mut observed = all_h2();
    let mut again = h2_observed(DOCUMENT, &h2_headers(DOCUMENT, 9999));
    if let Seen::H2 { preface, .. } = &mut again.seen {
        preface.headers.truncate(4);
    }
    observed.push(again);

    let found = h2_evidence(&observed).expect("evidence");

    assert_eq!(found.len(), PROBES.len());
    assert_eq!(found.first().map(|e| e.probe.as_str()), Some("document"));
    assert!(found.first().expect("document").headers.len() > 4);
    assert!(
        found.iter().all(|e| e.akamai.ends_with("|m,a,s,p")),
        "{found:?}"
    );
    assert_eq!(
        found.first().expect("document").akamai,
        "1:4096;2:0;4:1048576|983041|0|m,a,s,p"
    );
}

#[test]
fn a_probe_the_browser_never_requested_is_named() {
    let observed: Vec<Observed> = all_h2().into_iter().filter(|o| o.probe != STYLE).collect();

    assert!(matches!(
        h2_evidence(&observed),
        Err(CaptureH2Error::MissingProbe { probe: "style" })
    ));
    let none: Vec<Observed> = Vec::new();
    assert!(matches!(
        h1_evidence(&none),
        Err(CaptureH2Error::MissingProbe { probe: "document" })
    ));
    assert!(matches!(
        h2_evidence(&all_h1()),
        Err(CaptureH2Error::MissingProbe { .. })
    ));
}

#[test]
fn a_request_with_another_destination_is_refused() {
    let mut headers = h2_headers(IMAGE, 4433);
    for (name, value) in &mut headers {
        if name == "sec-fetch-dest" {
            "document".clone_into(value);
        }
    }
    let mut observed = all_h2();
    observed.retain(|o| o.probe != IMAGE);
    observed.push(h2_observed(IMAGE, &headers));

    let error = h2_evidence(&observed).expect_err("a wrong destination");

    assert!(
        matches!(&error, CaptureH2Error::WrongDestination { probe: "image", found } if found == "document"),
        "{error}"
    );
}

#[test]
fn a_request_with_a_credential_is_refused_in_both_protocols() {
    for header in ["cookie", "Authorization", "proxy-authorization"] {
        let mut headers = h2_headers(SCRIPT, 4433);
        headers.push((header.to_owned(), "secret-value".to_owned()));
        let mut h2 = all_h2();
        h2.retain(|o| o.probe != SCRIPT);
        h2.push(h2_observed(SCRIPT, &headers));
        let mut h1 = all_h1();
        h1.retain(|o| o.probe != SCRIPT);
        h1.push(h1_observed(SCRIPT, &headers));

        for error in [
            h2_evidence(&h2).expect_err("a credential"),
            h1_evidence(&h1).expect_err("a credential"),
        ] {
            assert!(
                matches!(error, CaptureH2Error::Credential { .. }),
                "{error:?}"
            );
            assert!(
                !format!("{error} {error:?}").contains("secret-value"),
                "the value never reaches an error"
            );
        }
    }
}

#[test]
fn derive_builds_the_http2_spec_from_the_navigation() {
    let (h2, h1) = evidence();

    let derived = derive(&h2, &h1).expect("derives");

    assert_eq!(derived.http2.settings, SETTINGS.to_vec());
    assert_eq!(derived.http2.window_update, WINDOW_UPDATE);
    assert_eq!(
        derived.http2.pseudo_order,
        [
            Pseudo::Method,
            Pseudo::Authority,
            Pseudo::Scheme,
            Pseudo::Path
        ]
    );
    let priority = derived.http2.headers_priority.expect("HEADERS priority");
    assert_eq!((priority.depends_on, priority.weight), (0, 256));
    assert_eq!(derived.http2.initial_stream_id, None);
    assert_eq!(
        derived.http2.akamai().to_string(),
        "1:4096;2:0;4:1048576|983041|0|m,a,s,p"
    );
}

#[test]
fn derive_takes_names_and_their_case_from_http1_and_priority_from_http2() {
    let (h2, h1) = evidence();

    let headers = derive(&h2, &h1).expect("derives").headers;

    let names: Vec<&str> = headers.document.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "sec-ch-ua",
            "sec-ch-ua-mobile",
            "User-Agent",
            "Accept",
            "Sec-Fetch-Dest",
            "Accept-Encoding",
            "Accept-Language"
        ],
        "HTTP/1 case and order; no Host, Connection or :pseudo"
    );
    assert_eq!(headers.priority_for("document"), Some("u=0, i"));
    assert_eq!(headers.priority_for("empty"), Some("u=1"));
    assert_eq!(headers.priority_for("image"), Some("u=1"));
    assert_eq!(headers.priority_for("nothing"), None);
}

#[test]
fn request_specific_headers_stay_out_of_the_lists() {
    let (h2, h1) = evidence();
    let fetch = h1.iter().find(|e| e.probe == "fetch").expect("fetch");
    assert!(fetch.header("origin").is_some() && fetch.header("referer").is_some());

    let headers = derive(&h2, &h1).expect("derives").headers;

    for list in [&headers.document, &headers.fetch] {
        assert!(list
            .iter()
            .all(|(n, _)| !n.eq_ignore_ascii_case("origin") && !n.eq_ignore_ascii_case("referer")));
    }
}

#[test]
fn derive_reports_where_the_protocols_disagree_and_not_otherwise() {
    let (h2, h1) = evidence();
    assert!(
        derive(&h2, &h1).expect("derives").differences.is_empty(),
        "the stand-in browser orders and writes its headers the same way in both protocols"
    );

    let mut reordered = h1.clone();
    if let Some(item) = reordered.iter_mut().find(|e| e.probe == "document") {
        item.headers.swap(2, 3);
    }
    let order = derive(&h2, &reordered).expect("derives").differences;
    assert_eq!(order.len(), 1);
    assert!(
        order[0].starts_with("document: HTTP/1.1 sends"),
        "{order:?}"
    );

    let mut changed = h1.clone();
    if let Some(item) = changed.iter_mut().find(|e| e.probe == "script") {
        for (name, value) in &mut item.headers {
            if name == "Accept" {
                "text/css".clone_into(value);
            }
        }
    }
    let value = derive(&h2, &changed).expect("derives").differences;
    assert_eq!(value.len(), 1);
    assert!(value[0].contains("`accept` differs"), "{value:?}");
}

#[test]
fn connections_that_start_differently_are_refused() {
    let (mut h2, h1) = evidence();
    let other = h2_start(
        &[(1, 4096), (4, 1_048_576)],
        &[
            (":method", "GET"),
            (":authority", "a"),
            (":scheme", "https"),
            (":path", "/"),
            ("sec-fetch-dest", "image"),
        ],
        None,
    );
    let preface = read_preface(&other).expect("reads").expect("complete");
    let item = h2.iter_mut().find(|e| e.probe == "image").expect("image");
    *item = H2Evidence::of(IMAGE, &other, &preface).expect("evidence");

    let error = derive(&h2, &h1).expect_err("different settings");

    assert!(matches!(&error, CaptureH2Error::ProbesDisagree { probe } if probe == "image"));
}

#[test]
fn stored_evidence_that_was_edited_is_caught() {
    let (mut h2, h1) = evidence();
    h2.first_mut()
        .expect("document")
        .headers
        .push(("x".to_owned(), "y".to_owned()));
    assert!(matches!(
        derive(&h2, &h1),
        Err(CaptureH2Error::Evidence { probe, .. }) if probe == "document"
    ));

    let (mut h2, h1) = evidence();
    h2.first_mut().expect("document").akamai = "tampered".to_owned();
    assert!(matches!(
        derive(&h2, &h1),
        Err(CaptureH2Error::Evidence { .. })
    ));

    let (mut h2, h1) = evidence();
    h2.first_mut().expect("document").raw_hex.truncate(80);
    assert!(derive(&h2, &h1).is_err());
    h2.first_mut().expect("document").raw_hex = "zz".to_owned();
    assert!(matches!(derive(&h2, &h1), Err(CaptureH2Error::Capture(_))));
}

#[test]
fn missing_evidence_names_the_probe() {
    let (h2, h1) = evidence();
    let without_fetch: Vec<H1Evidence> = h1.into_iter().filter(|e| e.probe != "fetch").collect();

    assert!(matches!(
        derive(&h2, &without_fetch),
        Err(CaptureH2Error::Evidence { probe, .. }) if probe == "fetch"
    ));
    assert!(matches!(
        derive(&[], &[]),
        Err(CaptureH2Error::MissingProbe { probe: "document" })
    ));
}

#[test]
fn evidence_survives_json() {
    let (h2, h1) = evidence();

    let text = serde_json::to_string(&(&h2, &h1)).expect("serializes");
    let (again2, again1): (Vec<H2Evidence>, Vec<H1Evidence>) =
        serde_json::from_str(&text).expect("parses");

    assert_eq!((again2, again1), (h2, h1));
}

#[test]
fn flags_pick_the_pass_and_the_rest_goes_to_the_shared_options() {
    let parse =
        |args: &[&str]| parse_args(args.iter().map(|a| (*a).to_owned()), Path::new("/repo"));

    assert_eq!(parse(&[]).expect("none").1, Passes::Both);
    assert_eq!(parse(&["--h1"]).expect("h1").1, Passes::H1Only);
    assert_eq!(
        parse(&["--h2", "--lang", "de-DE"]).expect("h2").1,
        Passes::H2Only
    );
    let (args, _) = parse(&["--h1", "--timeout", "9", "--h1"]).expect("repeat");
    assert_eq!(args.chrome.timeout, std::time::Duration::from_secs(9));
    assert!(parse(&["--h1", "--h2"]).is_err());
    assert!(parse(&["--nope"]).is_err());
}

/// A measured file that holds only a hello, as `capture_tls` leaves it.
const BASE_JSON: &str = r#"{
  "kind": "Chrome", "major": 155, "full_version": "155.0.1.2", "os": "Macos",
  "user_agent": "Mozilla/5.0 Chrome/155.0.0.0", "tls": { "legacy_version": 771 },
  "colour": "kept", "capture": { "tls": { "samples": [] } },
  "provenance": { "captured_on": "2026-01-01", "tool": "capture_tls 0.1.0",
                  "browser": "Chrome 155.0.1.2", "known_gaps": ["TrustAnchorsExtension"] }
}"#;

fn version(full: &str) -> BrowserVersion {
    BrowserVersion {
        product: format!("Chrome/{full}"),
        full_version: full.to_owned(),
        major: 155,
        user_agent: "Mozilla/5.0 Chrome/155.0.0.0".to_owned(),
    }
}

fn entry() -> EntryId {
    EntryId {
        kind: BrowserKind::Chrome,
        major: 155,
        os: OsType::Macos,
    }
}

fn write_base(dir: &Path, text: &str) {
    std::fs::write(dir.join(entry().file_name()), text).expect("writes the base file");
}

#[test]
fn the_file_is_extended_and_every_other_key_is_kept() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    write_base(dir.path(), BASE_JSON);
    let (h2, h1) = evidence();
    let derived = derive(&h2, &h1).expect("derives");

    let mut doc = MeasuredDoc::open(dir.path(), entry(), &version("155.0.1.2")).expect("opens");
    doc.set_h2_evidence(&h2);
    doc.set_h1_evidence(&h1);
    doc.set_derived(&derived).expect("sets");
    doc.stamp(
        "capture_h2 9.9.9",
        &version("155.0.1.2"),
        chrono::Utc::now(),
    );
    doc.save().expect("saves");

    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(doc.path()).expect("reads")).expect("json");
    assert_eq!(
        saved["colour"], "kept",
        "a key the tool does not own survives"
    );
    assert_eq!(saved["capture"]["tls"]["samples"], serde_json::json!([]));
    assert_eq!(saved["tls"]["legacy_version"], 771);
    assert_eq!(
        saved["http2"]["settings"],
        serde_json::json!([[1, 4096], [2, 0], [4, 1_048_576]])
    );
    assert!(saved["headers"]["document"].is_array());
    assert_eq!(saved["capture"]["differences"], serde_json::json!([]));
    assert_eq!(
        saved["provenance"]["known_gaps"],
        serde_json::json!(["TrustAnchorsExtension"])
    );
    let tool = saved["provenance"]["tool"].as_str().expect("a string");
    assert_eq!(tool, "capture_h2 9.9.9; capture_tls 0.1.0");

    let again = MeasuredDoc::open(dir.path(), entry(), &version("155.0.1.2")).expect("reopens");
    assert_eq!(again.h2_evidence().expect("reads"), Some(h2));
    assert_eq!(again.h1_evidence().expect("reads"), Some(h1));
}

#[test]
fn a_file_without_a_hello_or_from_another_version_is_not_extended() {
    let dir = tempfile::tempdir().expect("a temporary directory");

    let missing = MeasuredDoc::open(dir.path(), entry(), &version("155.0.1.2"));
    assert!(matches!(missing, Err(CaptureH2Error::NoBase { .. })));

    write_base(dir.path(), r#"{ "full_version": "155.0.1.2" }"#);
    let no_tls = MeasuredDoc::open(dir.path(), entry(), &version("155.0.1.2"));
    assert!(matches!(no_tls, Err(CaptureH2Error::NoBase { .. })));

    write_base(dir.path(), BASE_JSON);
    let other = MeasuredDoc::open(dir.path(), entry(), &version("155.0.9.9"));
    assert!(
        matches!(&other, Err(CaptureH2Error::VersionMismatch { file, browser })
            if file == "155.0.1.2" && browser == "155.0.9.9"),
        "{other:?}"
    );

    write_base(dir.path(), "[1, 2]");
    assert!(matches!(
        MeasuredDoc::open(dir.path(), entry(), &version("155.0.1.2")),
        Err(CaptureH2Error::Store { .. })
    ));
    write_base(dir.path(), "not json");
    assert!(matches!(
        MeasuredDoc::open(dir.path(), entry(), &version("155.0.1.2")),
        Err(CaptureH2Error::Json(_))
    ));
}

#[test]
fn the_raw_bytes_become_one_fixture_per_probe() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let (h2, _) = evidence();

    let written = write_h2_fixtures(&dir.path().join("fixtures"), entry(), &h2).expect("writes");

    assert_eq!(written.len(), PROBES.len());
    let first = written.first().expect("a file");
    assert!(
        first.ends_with("chrome-155-macos.h2.document.bin"),
        "{first:?}"
    );
    for (path, item) in written.iter().zip(&h2) {
        let bytes = std::fs::read(path).expect("reads");
        assert_eq!(
            crate::scrape::runtime::capture::record::to_hex(&bytes),
            item.raw_hex
        );
    }
}

#[test]
fn a_file_with_no_evidence_yet_reports_none() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    write_base(dir.path(), BASE_JSON);

    let doc = MeasuredDoc::open(dir.path(), entry(), &version("155.0.1.2")).expect("opens");

    assert_eq!(doc.h2_evidence().expect("reads"), None);
    assert_eq!(doc.h1_evidence().expect("reads"), None);
}
