//! The HTTP/2 and header capture against the real, headed Chrome on this machine.
//!
//! The test starts the local Chrome and opens a window, so it is ignored by default. It works on a
//! copy of the committed measured file in a temporary directory and never changes the repository.
//! It needs the committed TLS measurement to be of the installed Chrome: if Chrome has updated
//! since, it stops with the version mismatch and says to run `capture_tls` and `capture_hints`
//! again. Run it alone:
//!
//! ```text
//! cargo test --features impersonate --test impersonate_capture_h2_chrome -- --ignored --test-threads=1
//! ```
#![cfg(feature = "impersonate")]

use std::fs;
use std::path::Path;

use seleniumbase_rs::impersonate::capture_h2::{
    derive, measure_and_write, MeasuredDoc, Passes, PROBES,
};
use seleniumbase_rs::scrape::runtime::capture::args::ToolArgs;
use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::akamai::{Akamai, Pseudo};
use seleniumbase_rs::wire::emulation::{self, Emulation};
use seleniumbase_rs::wire::BrowserKind;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[tokio::test]
#[ignore = "starts the local Chrome"]
async fn chrome_http2_and_headers_have_the_chrome_shape_and_the_json_round_trips() {
    let committed = emulation::get(BrowserKind::Chrome, OsType::Macos).expect("measured");
    let dir = tempfile::tempdir().expect("a temporary directory");
    let name = format!("{}.json", committed.file_stem());
    fs::copy(
        root().join("src/wire/measured").join(&name),
        dir.path().join(&name),
    )
    .expect("copies the committed file");
    let mut args = ToolArgs::parse(Vec::<String>::new(), root()).expect("default arguments");
    args.out_dir = dir.path().to_path_buf();
    args.fixtures_dir = dir.path().join("fixtures");
    args.chrome.no_sandbox = std::env::var_os("CI").is_some();

    let report = measure_and_write(&args, Passes::Both, "capture_h2 test", chrono::Utc::now())
        .await
        .expect("the capture works");

    let derived = report.derived.as_ref().expect("both passes ran");
    let akamai = derived.http2.akamai();
    assert_eq!(report.h2.as_ref().map(Vec::len), Some(PROBES.len()));
    assert_eq!(report.h1.as_ref().map(Vec::len), Some(PROBES.len()));
    assert_eq!(
        akamai.pseudo_order,
        [
            Pseudo::Method,
            Pseudo::Authority,
            Pseudo::Scheme,
            Pseudo::Path
        ],
        "Chrome orders its pseudo-headers m,a,s,p: {akamai}"
    );
    assert!(
        derived.http2.setting(1).is_some(),
        "SETTINGS_HEADER_TABLE_SIZE: {akamai}"
    );
    assert!(
        derived.http2.setting(4).is_some(),
        "SETTINGS_INITIAL_WINDOW_SIZE: {akamai}"
    );
    assert!(
        derived.http2.window_update > 0,
        "a connection WINDOW_UPDATE follows: {akamai}"
    );
    assert_eq!(akamai.to_string().split('|').count(), 4, "{akamai}");
    assert_eq!(
        Akamai::parse(&akamai.to_string()).expect("the string parses"),
        akamai
    );
    assert_eq!(
        Some(&derived.http2),
        committed.http2.as_ref(),
        "the committed HTTP/2 spec is what this Chrome sends"
    );
    assert_eq!(
        derived.headers, committed.headers,
        "the committed header spec is what this Chrome sends"
    );

    let doc = MeasuredDoc::open(dir.path(), report.id, &report.version).expect("reopens");
    let (h2, h1) = (
        doc.h2_evidence().expect("reads").expect("h2 evidence"),
        doc.h1_evidence().expect("reads").expect("h1 evidence"),
    );
    assert_eq!(derive(&h2, &h1).expect("derives again"), *derived);
    let text = fs::read_to_string(&report.path).expect("the written file");
    let entry = Emulation::from_json(&name, &text).expect("the written file parses");
    let again = Emulation::from_json(&name, &entry.to_json().expect("serializes"))
        .expect("the serialization parses");
    assert_eq!(again, entry, "the measured JSON round-trips");
    assert_eq!(entry.http2.as_ref(), Some(&derived.http2));
}
