//! Swapping the egress on a running proxy, shutdown and drop, and the static checks.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use seleniumbase_rs::impersonate::mitm::MitmOptions;
use seleniumbase_rs::impersonate::tls_server::ALPN_HTTP1;
use seleniumbase_rs::scrape::egress::route::EgressRef;
use seleniumbase_rs::scrape::ledger::events::ScraperEvent;
use tokio::net::TcpStream;

use crate::support::{
    browser_client, der, eventually, new_ca, start_mitm, EgressProxy, RecordingOrigin, ALPN_H2,
    WITHIN,
};

/// The host the sites are reached by.
const ORIGIN: &str = "origin.test";

#[tokio::test]
async fn swapping_the_egress_changes_the_route_between_requests_on_one_listener() {
    let mut origin = RecordingOrigin::start(&[ALPN_H2, ALPN_HTTP1]).await;
    let first = EgressProxy::start(origin.addr()).await;
    let second = EgressProxy::start(origin.addr()).await;
    let ca = new_ca();
    let options = MitmOptions::default()
        .ca(Arc::clone(&ca))
        .trust_upstream_root(origin.root());
    let egress = EgressRef::parse(&first.spec(Some(("alice", "s3cret"))), None).expect("an egress");
    let (mitm, sink) = start_mitm(Some(egress), options).await;
    let port = mitm.port();
    let client = browser_client(&mitm, &der(&ca));
    let url = format!("https://{ORIGIN}:{}/", origin.addr().port());

    let answer = client.get(&url).send().await.expect("the first request");
    assert_eq!(answer.status(), 200);
    origin.next(WITHIN).await.expect("the origin saw it");
    assert_eq!((first.tunnels(), second.tunnels()), (1, 0));
    let expected = format!("Basic {}", STANDARD.encode("alice:s3cret"));
    assert_eq!(
        first.auth(),
        [Some(expected)],
        "the egress gets its credentials"
    );

    let next = EgressRef::parse(&second.spec(None), None).expect("an egress");
    let (from, to) = (mitm.egress(), Some(next.key().clone()));
    mitm.swap_upstream(Some(next)).expect("the swap works");

    let answer = client.get(&url).send().await.expect("the second request");
    assert_eq!(answer.status(), 200);
    origin.next(WITHIN).await.expect("the origin saw it");
    assert_eq!((first.tunnels(), second.tunnels()), (1, 1));
    assert_eq!(mitm.port(), port, "the listener stayed");
    assert_eq!(mitm.egress(), to);
    let rotated = sink
        .events()
        .into_iter()
        .filter(|event| matches!(event, ScraperEvent::EgressRotated { .. }))
        .collect::<Vec<_>>();
    assert_eq!(rotated, [ScraperEvent::EgressRotated { from, to }]);
    let shown = format!("{mitm:?} {:?}", sink.events());
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
}

#[tokio::test]
async fn shutdown_stops_the_listener_and_leaves_no_task_running() {
    let mut origin = RecordingOrigin::start(&[ALPN_H2, ALPN_HTTP1]).await;
    let metrics = tokio::runtime::Handle::current().metrics();
    let baseline = metrics.num_alive_tasks();
    let ca = new_ca();
    let options = MitmOptions::default()
        .ca(Arc::clone(&ca))
        .trust_upstream_root(origin.root())
        .resolve(ORIGIN, origin.addr())
        .allow_private_destinations(true);
    let (mitm, _sink) = start_mitm(None, options).await;
    let client = browser_client(&mitm, &der(&ca));
    let url = format!("https://{ORIGIN}:{}/", origin.addr().port());
    let answer = client.get(url).send().await.expect("a request");
    assert_eq!(answer.status(), 200);
    origin.next(WITHIN).await.expect("the origin saw it");
    let addr = mitm.local_addr();

    drop(answer);
    drop(client);
    mitm.shutdown(Duration::from_secs(5)).await;

    assert!(
        TcpStream::connect(addr).await.is_err(),
        "the listener still accepts"
    );
    assert!(
        eventually(|| metrics.num_alive_tasks() <= baseline).await,
        "{} tasks alive, {baseline} before the proxy started",
        metrics.num_alive_tasks()
    );
}

#[tokio::test]
async fn dropping_the_last_clone_stops_the_listener() {
    let (mitm, _sink) = start_mitm(None, MitmOptions::default()).await;
    let addr = mitm.local_addr();
    let clone = mitm.clone();

    drop(mitm);
    let alive = TcpStream::connect(addr).await;
    assert!(alive.is_ok(), "a clone keeps the proxy serving");
    drop(alive);
    drop(clone);

    let mut closed = false;
    for _ in 0..250 {
        if TcpStream::connect(addr).await.is_err() {
            closed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(closed, "the listener outlived every clone");
}

/// The source files of the proxy, without its unit tests.
fn proxy_sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/impersonate/mitm");
    let mut files: Vec<(String, String)> = fs::read_dir(&dir)
        .expect("the proxy module exists")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .filter(|path| path.file_name().is_some_and(|name| name != "tests.rs"))
        .map(|path| {
            let text = fs::read_to_string(&path).expect("readable");
            (path.display().to_string(), text)
        })
        .collect();
    files.sort();
    files
}

#[test]
fn the_proxy_never_touches_files_or_exports_certificates_and_keys() {
    let forbidden = [
        "std::fs",
        "tokio::fs",
        "File::",
        "OpenOptions",
        "to_pem",
        "to_der",
        "private_key",
        "certificate()",
        "println!",
        "eprintln!",
        "unsafe ",
    ];
    let sources = proxy_sources();
    assert_eq!(
        sources.len(),
        8,
        "mod, body, connect, destination, forward, sniff, upstream and websocket"
    );
    for (path, text) in sources {
        for word in forbidden {
            assert!(!text.contains(word), "{path} uses `{word}`");
        }
    }
}
