//! The measurement tools against a real, headed Chrome.
//!
//! Each test starts the local Chrome and opens a window, so they are ignored by default. Run them
//! one at a time:
//!
//! ```text
//! cargo test --features scrape --test scrape_capture_chrome -- --ignored --test-threads=1
//! ```
#![cfg(feature = "scrape")]

use seleniumbase_rs::scrape::runtime::capture::{
    measure_hints, measure_tls, BrowserVersion, ChromeOptions, EntryId, MeasuredFile,
};
use seleniumbase_rs::stealth::fingerprint::{ClientHints, OsType};
use seleniumbase_rs::wire::client_hello::ClientHello;
use seleniumbase_rs::wire::tls_spec::TlsSpec;
use seleniumbase_rs::wire::BrowserKind;

fn options() -> ChromeOptions {
    ChromeOptions {
        no_sandbox: std::env::var_os("CI").is_some(),
        ..ChromeOptions::default()
    }
}

fn entry(version: &BrowserVersion) -> EntryId {
    EntryId {
        kind: BrowserKind::Chrome,
        major: version.major,
        os: version.os().expect("a known operating system"),
    }
}

#[tokio::test]
#[ignore = "starts the local Chrome"]
async fn chrome_hellos_parse_and_the_saved_json_round_trips() {
    let (version, tls) = measure_tls(&options())
        .await
        .expect("the TLS capture works");

    assert!(
        version.product.starts_with("Chrome/"),
        "{}",
        version.product
    );
    assert!(tls.samples.len() >= 2, "several hellos show the shuffle");
    for sample in &tls.samples {
        let hello: ClientHello = sample.hello().expect("the raw bytes parse");
        assert!(hello.has_grease(), "Chrome sends GREASE");
        assert!(sample.ja4.starts_with("t13d"), "{}", sample.ja4);
        assert!(sample
            .sni
            .as_deref()
            .is_some_and(|name| name.ends_with(".capture.test")));
    }
    assert!(tls.spec.grease);
    assert!(
        tls.spec.permute_extensions,
        "Chrome shuffles its extensions on every connection"
    );
    assert!(
        tls.websocket_spec.is_some(),
        "a WebSocket hello was captured"
    );

    let dir = tempfile::tempdir().expect("a temporary directory");
    let id = entry(&version);
    let mut file = MeasuredFile::open(dir.path(), id).expect("opens");
    file.set_identity(id, &version);
    file.set_tls(&tls).expect("tls");
    file.stamp("capture_tls test", &version, chrono::Utc::now());
    file.save().expect("saves");
    let reread = MeasuredFile::open(dir.path(), id).expect("reopens");
    let spec: TlsSpec =
        serde_json::from_value(reread.get("tls").expect("tls").clone()).expect("tls is a TlsSpec");
    assert_eq!(spec, tls.spec);
    assert_eq!(
        reread.to_pretty_json().expect("json"),
        file.to_pretty_json().expect("json")
    );
}

#[tokio::test]
#[ignore = "starts the local Chrome"]
async fn chrome_reports_hints_that_agree_with_its_request_headers() {
    let (version, hints) = measure_hints(&options())
        .await
        .expect("the hints capture works");

    assert_eq!(hints.user_agent, version.user_agent);
    assert!(
        hints.accept_language.starts_with("en-US"),
        "{}",
        hints.accept_language
    );
    assert!(hints.languages.iter().any(|language| language == "en-US"));
    let client_hints: &ClientHints = &hints.client_hints;
    assert!(client_hints.brands.iter().any(|b| b.brand == "Chromium"));
    assert!(client_hints
        .full_version_list
        .iter()
        .any(|b| b.version == version.full_version));
    assert_eq!(client_hints.ua_full_version, version.full_version);
    assert!(!client_hints.mobile);
    if version.os().expect("a known operating system") == OsType::Macos {
        assert_eq!(client_hints.platform, "macOS");
    }
    assert!(hints
        .request_headers
        .iter()
        .any(|(name, _)| name == "sec-ch-ua-platform-version"));
    assert!(hints
        .first_visit_headers
        .iter()
        .all(|(name, _)| name != "sec-ch-ua-platform-version"));
}
