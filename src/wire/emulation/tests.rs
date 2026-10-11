//! Parsing, lookup and the refusal of unmeasured browser and system pairs.

use super::{
    for_kind, get, get_version, measured, measured_systems, newest, newest_on, parse_all,
    source_names, try_measured, Emulation, EmulationError, KnownGap, MAX_MEASURED_JSON_BYTES,
    SOURCES,
};
use crate::stealth::fingerprint::OsType;
use crate::wire::tls_spec::KnownGap as TlsKnownGap;
use crate::wire::BrowserKind;

/// The one browser that exists today, with the guard that makes a new variant fail to compile
/// here until it is classified.
fn all_kinds() -> [BrowserKind; 1] {
    fn guard(kind: BrowserKind) -> BrowserKind {
        match kind {
            BrowserKind::Chrome => BrowserKind::Chrome,
        }
    }
    [guard(BrowserKind::Chrome)]
}

fn chrome_macos() -> &'static Emulation {
    get(BrowserKind::Chrome, OsType::Macos).expect("Chrome on macOS is measured")
}

/// The measured Chrome entry with a different major version and matching text.
fn with_major(major: u32) -> Emulation {
    let base = chrome_macos();
    Emulation {
        major,
        full_version: format!("{major}.0.1.2"),
        user_agent: base.user_agent.replace(
            &format!("Chrome/{}", base.major),
            &format!("Chrome/{major}"),
        ),
        ..base.clone()
    }
}

/// The embedded text of the Chrome file, as a JSON value to edit.
fn chrome_json() -> serde_json::Value {
    let (_, text) = SOURCES.first().expect("one embedded file");
    serde_json::from_str(text).expect("embedded JSON is valid")
}

fn parse_value(value: &serde_json::Value) -> Result<Emulation, EmulationError> {
    Emulation::from_json("test.json", &value.to_string())
}

#[test]
fn every_embedded_file_parses_and_none_repeats() {
    let entries = try_measured().expect("the embedded files parse");

    assert_eq!(entries.len(), SOURCES.len());
    assert!(!entries.is_empty(), "an empty table measures nothing");
    assert_eq!(source_names().len(), SOURCES.len());
}

#[test]
fn chrome_on_macos_is_measured() {
    let chrome = chrome_macos();

    assert_eq!(chrome.kind, BrowserKind::Chrome);
    assert_eq!(chrome.os, OsType::Macos);
    assert!(chrome
        .user_agent
        .contains(&format!("Chrome/{}", chrome.major)));
    assert!(chrome
        .full_version
        .starts_with(&format!("{}.", chrome.major)));
    chrome
        .tls
        .without_known_gaps(&chrome.provenance.known_gaps)
        .check_reproducible()
        .expect("the hello is reproducible apart from its known gaps");
    assert!(chrome.client_hints.is_some(), "the hints were measured");
    assert!(chrome.tls_websocket.is_some());
}

#[test]
fn the_known_gaps_are_the_tls_spec_type() {
    let gaps: Vec<TlsKnownGap> = chrome_macos().provenance.known_gaps.clone();
    let same: Vec<KnownGap> = gaps;

    assert!(same.contains(&KnownGap::TrustAnchorsExtension));
}

#[test]
fn unmeasured_systems_are_refused_by_name() {
    for os in [OsType::Windows, OsType::Linux, OsType::Android, OsType::Ios] {
        let error = get(BrowserKind::Chrome, os).expect_err("not measured");

        assert_eq!(
            error,
            EmulationError::NotMeasured {
                kind: BrowserKind::Chrome,
                os
            }
        );
        assert!(error.to_string().contains("has not been measured"));
    }
}

#[test]
fn a_measured_system_with_another_major_is_a_version_error() {
    let chrome = chrome_macos();

    let found = get_version(BrowserKind::Chrome, OsType::Macos, chrome.major);
    assert_eq!(found.expect("the measured major").major, chrome.major);

    let missing = get_version(BrowserKind::Chrome, OsType::Macos, chrome.major + 1);
    assert_eq!(
        missing.expect_err("another major"),
        EmulationError::VersionNotMeasured {
            kind: BrowserKind::Chrome,
            major: chrome.major + 1,
            os: OsType::Macos
        }
    );
    let unmeasured = get_version(BrowserKind::Chrome, OsType::Windows, chrome.major);
    assert!(matches!(
        unmeasured,
        Err(EmulationError::NotMeasured { .. })
    ));
}

#[test]
fn every_browser_kind_has_an_entry() {
    for kind in all_kinds() {
        assert!(
            measured().iter().any(|entry| entry.kind == kind),
            "{kind:?} has no measured entry; measure it or refuse it in a documented way"
        );
        assert_eq!(for_kind(kind).kind, kind);
    }
}

#[test]
fn only_measured_systems_are_listed() {
    assert_eq!(
        measured_systems(BrowserKind::Chrome),
        vec![OsType::Macos],
        "Chrome is measured on macOS and nowhere else"
    );
}

#[test]
fn the_newest_major_wins_and_the_first_of_equals() {
    let entries = vec![
        with_major(150),
        with_major(153),
        with_major(153),
        with_major(151),
    ];
    let best = newest(&entries, BrowserKind::Chrome).expect("an entry");

    assert_eq!(best.major, 153);
    assert!(std::ptr::eq(best, entries.get(1).expect("second entry")));
    assert_eq!(
        newest_on(&entries, BrowserKind::Chrome, OsType::Macos).map(|e| e.major),
        Some(153)
    );
    assert!(newest_on(&entries, BrowserKind::Chrome, OsType::Linux).is_none());
    assert!(newest(&[], BrowserKind::Chrome).is_none());
}

#[test]
fn two_files_for_one_entry_are_refused() {
    let (name, text) = SOURCES.first().expect("one embedded file");
    let twice = [(*name, *text), ("copy.json", *text)];

    let error = parse_all(&twice).expect_err("a duplicate");

    assert_eq!(
        error,
        EmulationError::Duplicate {
            first: (*name).to_owned(),
            second: "copy.json".to_owned()
        }
    );
}

#[test]
fn the_raw_evidence_is_ignored_and_a_stray_key_is_not() {
    let mut value = chrome_json();
    assert!(
        value.get("capture").is_some(),
        "the file keeps its evidence"
    );
    value["capture"] = serde_json::json!({ "anything": [1, 2, 3] });
    assert!(parse_value(&value).is_ok());

    value["colour"] = serde_json::json!("blue");
    let error = parse_value(&value).expect_err("an unknown key");
    assert!(matches!(&error, EmulationError::Parse { reason, .. } if reason.contains("colour")));
}

#[test]
fn http2_and_headers_are_optional() {
    let mut value = chrome_json();
    let object = value.as_object_mut().expect("an object");
    object.remove("http2");
    object.remove("headers");

    let entry = parse_value(&value).expect("parses without them");

    assert!(entry.http2.is_none());
    assert!(entry.headers.document.is_empty() && entry.headers.fetch.is_empty());
}

#[test]
fn a_file_that_contradicts_itself_is_refused() {
    let mut value = chrome_json();
    value["full_version"] = serde_json::json!("1.0.0.0");
    assert!(matches!(
        parse_value(&value),
        Err(EmulationError::Inconsistent { reason, .. }) if reason.contains("full_version")
    ));

    let mut value = chrome_json();
    value["user_agent"] = serde_json::json!("Mozilla/5.0 Gecko/20100101 Firefox/130.0");
    assert!(matches!(
        parse_value(&value),
        Err(EmulationError::Inconsistent { reason, .. }) if reason.contains("user_agent")
    ));
}

#[test]
fn an_unknown_browser_or_a_missing_field_is_a_parse_error() {
    let mut value = chrome_json();
    value["kind"] = serde_json::json!("Safari");
    assert!(matches!(
        parse_value(&value),
        Err(EmulationError::Parse { .. })
    ));

    let mut value = chrome_json();
    value.as_object_mut().expect("an object").remove("tls");
    assert!(matches!(
        parse_value(&value),
        Err(EmulationError::Parse { reason, .. }) if reason.contains("tls")
    ));
    assert!(matches!(
        Emulation::from_json("junk.json", "not json"),
        Err(EmulationError::Parse { .. })
    ));
}

#[test]
fn an_invalid_header_spec_is_refused_with_the_file_name() {
    let mut value = chrome_json();
    value["headers"] = serde_json::json!({
        "document": [["Accept", { "literal": "*/*" }], ["accept", { "literal": "*/*" }]]
    });

    let error = parse_value(&value).expect_err("a repeated header");

    assert!(
        matches!(&error, EmulationError::Headers { name, .. } if name == "test.json"),
        "{error}"
    );
}

#[test]
fn a_huge_file_is_refused_before_it_is_parsed() {
    let huge = " ".repeat(MAX_MEASURED_JSON_BYTES + 1);

    assert_eq!(
        Emulation::from_json("huge.json", &huge),
        Err(EmulationError::TooLarge {
            name: "huge.json".to_owned()
        })
    );
}

#[test]
fn an_entry_survives_a_trip_through_json() {
    let entry = chrome_macos();

    let text = entry.to_json().expect("serializes");
    let again = Emulation::from_json("again.json", &text).expect("parses again");

    assert_eq!(&again, entry);
    assert!(!text.contains("\"capture\""), "the evidence is not written");
    assert_eq!(entry.file_stem(), "chrome-155-macos");
}
