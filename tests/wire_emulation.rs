//! The embedded measured tables and the files on disk agree, and the committed HTTP/2 spec is what
//! the raw frames under `tests/fixtures/measured` decode to.
//!
//! The pure `wire` modules cannot read files, so this test does: it checks that every
//! `src/wire/measured/*.json` is embedded by `wire::emulation` (and the other way round), and that
//! each raw HTTP/2 fixture decodes to the spec the JSON holds. The test that derives the header
//! spec again from the evidence stored in the JSON is `tests/impersonate_capture_h2.rs`, because
//! the derivation lives with the capture tool.
#![cfg(feature = "scrape")]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::akamai::{Akamai, Pseudo};
use seleniumbase_rs::wire::emulation::{self, source_names};
use seleniumbase_rs::wire::h2_spec::H2Spec;
use seleniumbase_rs::wire::BrowserKind;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn measured_dir() -> PathBuf {
    root().join("src/wire/measured")
}

fn fixtures_dir() -> PathBuf {
    root().join("tests/fixtures/measured")
}

fn json_files_on_disk() -> BTreeSet<String> {
    fs::read_dir(measured_dir())
        .expect("the measured directory exists")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            Path::new(name)
                .extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect()
}

#[test]
fn every_measured_file_is_embedded_and_every_embedded_file_exists() {
    let embedded: BTreeSet<String> = source_names().into_iter().map(str::to_owned).collect();

    assert_eq!(
        json_files_on_disk(),
        embedded,
        "src/wire/measured/*.json and the SOURCES list in src/wire/emulation.rs must name the same files"
    );
}

#[test]
fn the_file_name_matches_the_entry_it_holds() {
    for entry in emulation::measured() {
        let file = format!("{}.json", entry.file_stem());

        assert!(
            source_names().contains(&file.as_str()),
            "{file} should be the name of the file that holds {:?} {} on {:?}",
            entry.kind,
            entry.major,
            entry.os
        );
    }
}

fn chrome() -> &'static emulation::Emulation {
    emulation::get(BrowserKind::Chrome, OsType::Macos).expect("Chrome on macOS is measured")
}

fn fixture(name: &str) -> Vec<u8> {
    let stem = chrome().file_stem();
    let path = fixtures_dir().join(format!("{stem}.{name}.bin"));
    fs::read(&path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

#[test]
fn the_http2_spec_is_measured_and_has_the_chrome_shape() {
    let spec = chrome().http2.as_ref().expect("the HTTP/2 capture has run");

    assert_eq!(
        spec.pseudo_order,
        [
            Pseudo::Method,
            Pseudo::Authority,
            Pseudo::Scheme,
            Pseudo::Path
        ],
        "Chrome sends :method, :authority, :scheme, :path"
    );
    assert!(spec.setting(1).is_some(), "SETTINGS_HEADER_TABLE_SIZE");
    assert!(spec.setting(4).is_some(), "SETTINGS_INITIAL_WINDOW_SIZE");
    assert!(spec.window_update > 0, "a connection WINDOW_UPDATE follows");
    let akamai = spec.akamai().to_string();
    assert_eq!(
        Akamai::parse(&akamai).expect("the string parses"),
        spec.akamai(),
        "{akamai}"
    );
}

#[test]
fn each_raw_http2_fixture_decodes_to_the_committed_spec() {
    let spec = chrome().http2.as_ref().expect("the HTTP/2 capture has run");

    let from_bytes = H2Spec::from_capture(&fixture("h2.document")).expect("the fixture decodes");

    assert_eq!(&from_bytes, spec, "the document connection is the spec");
    for probe in ["fetch", "image", "script", "style"] {
        let other = H2Spec::from_capture(&fixture(&format!("h2.{probe}"))).expect("decodes");
        assert_eq!(other.akamai(), spec.akamai(), "the {probe} connection");
    }
}

#[test]
fn the_header_spec_has_both_shapes_and_the_destination_priorities() {
    let headers = &chrome().headers;

    assert!(!headers.document.is_empty() && !headers.fetch.is_empty());
    let names = |list: &[(String, _)]| {
        list.iter()
            .map(|(name, _)| name.to_ascii_lowercase())
            .collect::<Vec<_>>()
    };
    for list in [names(&headers.document), names(&headers.fetch)] {
        assert!(list.iter().any(|name| name == "user-agent"));
        assert!(list.iter().any(|name| name == "accept-language"));
        assert!(
            !list
                .iter()
                .any(|name| name == "origin" || name == "referer"),
            "request-specific headers are not part of the spec"
        );
    }
    assert!(headers.priority_for("document").is_some());
    assert!(headers.priority_for("empty").is_some());
}
