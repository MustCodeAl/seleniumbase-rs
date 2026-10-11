//! Tests of the parts of the Chrome driver that need no browser.

use std::net::{Ipv4Addr, SocketAddr};

use super::*;
use crate::wire::client_hello::fixtures::{
    server_name_body, HelloBuilder, CHROME_CIPHERS, JA4_EXAMPLE_EXTENSIONS,
};
use crate::wire::client_hello::{ext_id, ClientHello};

fn captured(host: &str) -> CapturedHello {
    let raw = HelloBuilder::new(&CHROME_CIPHERS, &JA4_EXAMPLE_EXTENSIONS)
        .with_body(ext_id::SERVER_NAME, server_name_body(host))
        .with_grease()
        .record();
    CapturedHello {
        peer: SocketAddr::from((Ipv4Addr::LOCALHOST, 50_000)),
        hello: ClientHello::parse(&raw).expect("a hello"),
        raw,
    }
}

#[test]
fn the_resolver_rule_sends_every_probe_host_to_loopback() {
    assert_eq!(
        host_resolver_rule(),
        "--host-resolver-rules=MAP *.capture.test 127.0.0.1"
    );
    assert!(WEBSOCKET_HOST.ends_with(CAPTURE_DOMAIN));
}

#[test]
fn the_probe_script_names_every_host_and_the_websocket() {
    let script = probe_script(4321, 3);

    for host in ["h0", "h1", "h2"] {
        assert!(
            script.contains(&format!("https://{host}.capture.test:4321/")),
            "{host}: {script}"
        );
    }
    assert!(!script.contains("h3.capture.test"));
    assert!(script.contains("wss://ws.capture.test:4321/"));
    assert!(script.contains("no-cors"));
}

#[test]
fn the_default_options_run_a_headed_browser() {
    let options = ChromeOptions::default();

    assert!(!options.headless);
    assert_eq!(options.lang, "en-US");
    assert!(options.executable.is_none());
    assert_eq!(options.page_probes, DEFAULT_PAGE_PROBES);
}

#[test]
fn hellos_are_sorted_by_the_name_they_asked_for() {
    let hellos = vec![
        captured("h0.capture.test"),
        captured("ws.capture.test"),
        captured("h1.capture.test"),
        captured("unrelated.example"),
    ];

    let (page, websocket) = split_probe_hellos(&hellos);

    assert_eq!(page.len(), 2);
    assert_eq!(page.first(), hellos.first().map(|h| &h.raw));
    assert_eq!(websocket.as_ref(), hellos.get(1).map(|h| &h.raw));
}

#[test]
fn a_probe_is_done_with_enough_page_hellos_and_a_websocket_one() {
    let two_pages = vec![captured("h0.capture.test"), captured("h1.capture.test")];
    let mut with_websocket = two_pages.clone();
    with_websocket.push(captured("ws.capture.test"));

    assert!(!probes_done(&two_pages, 2), "no websocket hello yet");
    assert!(!probes_done(&with_websocket, 3), "one page hello short");
    assert!(probes_done(&with_websocket, 2));
    assert!(probes_done(&with_websocket, 1));
    assert!(!probes_done(&[], 1));
}
