//! The browser extension (seleniumbase-rs-companion) writes the page report of its Page tab as
//! a fingerprint file, and reads one back. This is the core's side of that contract: the file
//! the extension writes for a page, as it writes it (`toFingerprint` of its `src/shared/fingerprint.ts`,
//! pinned by its `tests/fingerprint.test.ts`), loads here, is coherent, and survives a trip
//! through `save` and `load`. A key this crate does not know is refused, as the extension's
//! tests expect.

use seleniumbase_rs::Fingerprint;

const FROM_THE_EXTENSION: &str = include_str!("fixtures/fingerprint/companion_page_tab.json");

#[test]
fn the_page_report_of_the_extension_is_a_fingerprint() {
    let fingerprint =
        Fingerprint::from_json(FROM_THE_EXTENSION).expect("the extension's file loads");

    let report = fingerprint.validate();
    assert!(report.is_coherent(), "{:?}", report.errors);
    // What the report gave is claimed; what it left out stays the browser's own.
    let claimed = fingerprint.claimed_settings();
    for setting in [
        "user_agent",
        "platform",
        "timezone",
        "screen",
        "webgl",
        "media_devices",
        "client_hints",
    ] {
        assert!(
            claimed.contains(&setting),
            "{setting} is not in {claimed:?}"
        );
    }
}

#[test]
fn it_survives_a_save_and_a_load_unchanged() {
    let fingerprint = Fingerprint::from_json(FROM_THE_EXTENSION).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("page-tab.json");

    fingerprint.save(&file).unwrap();
    let loaded = Fingerprint::load(&file).unwrap();

    assert_eq!(loaded, fingerprint);
    assert!(fingerprint.diff(&loaded).is_empty());
}

#[test]
fn a_key_this_crate_does_not_know_is_refused_not_ignored() {
    let error = Fingerprint::from_json(r#"{ "screen_width": 1512 }"#).unwrap_err();

    assert!(error.to_string().contains("screen_width"), "{error}");
}
