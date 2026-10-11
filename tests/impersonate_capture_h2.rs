//! The committed HTTP/2 and header measurement against its own evidence. None of it needs a browser.
//!
//! The measured JSON keeps, next to the specs, the bytes and header lists they were derived from.
//! These tests derive the specs again from that evidence and compare them with what is committed,
//! and check that the raw HTTP/2 fixtures are the bytes the evidence holds. They fail if someone
//! edits a spec by hand, or the evidence without running the tool.
#![cfg(feature = "impersonate")]

use std::fs;
use std::path::{Path, PathBuf};

use seleniumbase_rs::impersonate::capture_h2::{derive, H1Evidence, H2Evidence, PROBES};
use seleniumbase_rs::scrape::runtime::capture::record::to_hex;
use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::emulation::{self, Emulation};
use seleniumbase_rs::wire::BrowserKind;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn chrome() -> &'static Emulation {
    emulation::get(BrowserKind::Chrome, OsType::Macos).expect("Chrome on macOS is measured")
}

fn file() -> serde_json::Value {
    let path = root()
        .join("src/wire/measured")
        .join(format!("{}.json", chrome().file_stem()));
    let text = fs::read_to_string(&path).expect("the measured file");
    serde_json::from_str(&text).expect("valid JSON")
}

fn evidence() -> (Vec<H2Evidence>, Vec<H1Evidence>) {
    let value = file();
    let probes = |pass: &str| value["capture"][pass]["probes"].clone();
    (
        serde_json::from_value(probes("h2")).expect("capture.h2.probes"),
        serde_json::from_value(probes("h1")).expect("capture.h1.probes"),
    )
}

fn fixture(probe: &str) -> PathBuf {
    root()
        .join("tests/fixtures/measured")
        .join(format!("{}.h2.{probe}.bin", chrome().file_stem()))
}

#[test]
fn every_probe_has_evidence_in_both_passes() {
    let (h2, h1) = evidence();

    let names: Vec<&str> = PROBES.iter().map(|probe| probe.name).collect();
    assert_eq!(
        h2.iter().map(|e| e.probe.as_str()).collect::<Vec<_>>(),
        names
    );
    assert_eq!(
        h1.iter().map(|e| e.probe.as_str()).collect::<Vec<_>>(),
        names
    );
    for item in &h2 {
        assert!(item.sni.ends_with(".capture.test"), "{}", item.sni);
    }
}

#[test]
fn the_committed_specs_are_what_the_evidence_derives_to() {
    let (h2, h1) = evidence();

    let derived = derive(&h2, &h1).expect("the evidence derives");

    assert_eq!(
        Some(&derived.http2),
        chrome().http2.as_ref(),
        "http2 in the file is not what its evidence says"
    );
    assert_eq!(
        derived.headers,
        chrome().headers,
        "headers in the file are not what their evidence says"
    );
    let recorded: Vec<String> =
        serde_json::from_value(file()["capture"]["differences"].clone()).expect("a list");
    assert_eq!(derived.differences, recorded);
}

#[test]
fn the_raw_fixtures_are_the_bytes_the_evidence_holds() {
    let (h2, _) = evidence();

    for item in &h2 {
        let bytes = fs::read(fixture(&item.probe)).expect("the fixture exists");
        assert_eq!(to_hex(&bytes), item.raw_hex, "{}", item.probe);
    }
}

#[test]
fn the_stored_akamai_string_of_each_probe_is_the_one_its_bytes_give() {
    let (h2, _) = evidence();
    let spec = chrome().http2.as_ref().expect("measured");

    for item in &h2 {
        assert_eq!(item.akamai, spec.akamai().to_string(), "{}", item.probe);
        assert_eq!(item.stream_id, 1, "a browser's first request is stream 1");
    }
}

#[test]
fn http1_shows_the_case_the_browser_writes_and_http2_the_lowercase_names() {
    let (h2, h1) = evidence();

    let document_h1 = h1.iter().find(|e| e.probe == "document").expect("document");
    let document_h2 = h2.iter().find(|e| e.probe == "document").expect("document");

    assert!(document_h1
        .headers
        .iter()
        .any(|(name, _)| name == "User-Agent"));
    assert!(document_h2
        .headers
        .iter()
        .all(|(name, _)| name == &name.to_ascii_lowercase()));
    assert!(document_h1.headers.iter().any(|(name, _)| name == "Host"));
    assert!(!document_h2.headers.iter().any(|(name, _)| name == "host"));
}

#[test]
fn no_evidence_carries_a_credential() {
    let (h2, h1) = evidence();
    let credentials = ["cookie", "authorization", "proxy-authorization"];

    for (name, _) in h2
        .iter()
        .flat_map(|e| &e.headers)
        .chain(h1.iter().flat_map(|e| &e.headers))
    {
        assert!(
            !credentials.contains(&name.to_ascii_lowercase().as_str()),
            "{name}"
        );
    }
}
