//! Where a direct proxy will not send a request: the machine itself, the local network and the
//! cloud metadata addresses, whether a page names them or a DNS answer leads there.

use std::net::Ipv4Addr;
use std::sync::Arc;

use seleniumbase_rs::impersonate::mitm::{MitmOptions, MitmProxy};
use seleniumbase_rs::impersonate::tls_server::ALPN_HTTP1;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

use crate::support::{
    closes, der, exchange, measured_user_agent, navigation, new_ca, proxy_auth, raw_connect,
    read_head, start_mitm, tunnel, RecordingOrigin, ALPN_H2, QUIET, WITHIN,
};

/// A name that the options send to a loopback server.
const REBOUND: &str = "rebind.test";

/// Targets a page must not be able to reach: loopback, private, link-local, carrier-grade NAT,
/// the metadata services, the IPv6 and obfuscated spellings of those, and `localhost` names.
const FORBIDDEN: [&str; 14] = [
    "127.0.0.1:443",
    "169.254.169.254:80",
    "[::1]:443",
    "10.0.0.5:8443",
    "192.168.1.1:443",
    "172.16.0.9:443",
    "100.100.100.200:80",
    "168.63.129.16:80",
    "[::ffff:169.254.169.254]:80",
    "[fd00:ec2::254]:80",
    "2130706433:80",
    "0x7f.0.0.1:80",
    "localhost:443",
    "metadata.localhost:80",
];

/// A proxy whose options send [`REBOUND`] to `addr`, and the DER of its CA.
async fn proxy_sending_rebound_to(
    addr: std::net::SocketAddr,
    options: MitmOptions,
    origin_root: Option<Vec<u8>>,
) -> (MitmProxy, Vec<u8>) {
    let ca = new_ca();
    let mut options = options.ca(Arc::clone(&ca)).resolve(REBOUND, addr);
    if let Some(root) = origin_root {
        options = options.trust_upstream_root(root);
    }
    let (mitm, _sink) = start_mitm(None, options).await;
    (mitm, der(&ca))
}

#[tokio::test]
async fn a_connect_to_an_internal_address_or_localhost_is_refused_before_any_tunnel() {
    let (mitm, _sink) = start_mitm(None, MitmOptions::default()).await;
    let auth = proxy_auth(&mitm);

    for target in FORBIDDEN {
        let (mut tcp, head) = raw_connect(mitm.local_addr(), target, Some(&auth)).await;
        let head = head.expect("an answer");
        assert_eq!(head.status(), 403, "{target}");
        assert_eq!(head.content_length(), Some(0), "{target}: no detail");
        assert!(
            closes(&mut tcp).await,
            "{target}: the connection stays open"
        );
    }

    assert_eq!(mitm.stats().tunnels, 0);
    assert_eq!(mitm.stats().refused, FORBIDDEN.len() as u64);
}

#[tokio::test]
async fn the_same_targets_are_taken_when_private_destinations_are_allowed() {
    let options = MitmOptions::default().allow_private_destinations(true);
    let (mitm, _sink) = start_mitm(None, options).await;
    let auth = proxy_auth(&mitm);

    for target in [
        "127.0.0.1:443",
        "[::1]:443",
        "localhost:443",
        "10.0.0.5:8443",
    ] {
        let (_tcp, head) = raw_connect(mitm.local_addr(), target, Some(&auth)).await;
        assert_eq!(head.expect("an answer").status(), 200, "{target}");
    }
}

#[tokio::test]
async fn an_absolute_form_request_to_an_internal_address_never_connects() {
    let service = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("binds loopback");
    let port = service.local_addr().expect("an address").port();
    let (mitm, _sink) = start_mitm(None, MitmOptions::default()).await;
    let auth = proxy_auth(&mitm);

    for host in [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        "169.254.169.254".to_owned(),
    ] {
        let mut tcp = TcpStream::connect(mitm.local_addr())
            .await
            .expect("reaches the proxy");
        let request = format!(
            "GET http://{host}/latest/meta-data/ HTTP/1.1\r\nHost: {host}\r\n\
             Proxy-Authorization: {auth}\r\n\r\n"
        );
        tcp.write_all(request.as_bytes()).await.expect("writes");

        let head = read_head(&mut tcp).await.expect("an answer");
        assert_eq!(head.status(), 403, "{host}");
        assert_eq!(head.content_length(), Some(0), "{host}: no detail");
    }

    let reached = tokio::time::timeout(QUIET, service.accept()).await;
    assert!(
        reached.is_err(),
        "the proxy connected to a loopback service"
    );
    assert_eq!(mitm.stats().forwarded, 0);
}

#[tokio::test]
async fn a_name_that_resolves_to_loopback_is_refused_and_nothing_connects() {
    let mut origin = RecordingOrigin::start(&[ALPN_H2, ALPN_HTTP1]).await;
    let (mitm, ca) =
        proxy_sending_rebound_to(origin.addr(), MitmOptions::default(), Some(origin.root())).await;
    let port = origin.addr().port();
    let mut tls = tunnel(&mitm, REBOUND, port, &ca).await;

    let request = navigation(REBOUND, port, &measured_user_agent(), &[]);
    let (head, _body) = exchange(&mut tls, &request).await.expect("an answer");

    assert_eq!(head.status(), 502);
    assert!(
        origin.next(QUIET).await.is_none(),
        "the proxy connected to the address the name resolved to"
    );
    assert_eq!(mitm.stats().forwarded, 0);
    assert_eq!(mitm.stats().upstream_errors, 1);
}

#[tokio::test]
async fn the_same_name_is_served_when_private_destinations_are_allowed() {
    let mut origin = RecordingOrigin::start(&[ALPN_H2, ALPN_HTTP1]).await;
    let options = MitmOptions::default().allow_private_destinations(true);
    let (mitm, ca) = proxy_sending_rebound_to(origin.addr(), options, Some(origin.root())).await;
    let port = origin.addr().port();
    let mut tls = tunnel(&mitm, REBOUND, port, &ca).await;

    let request = navigation(REBOUND, port, &measured_user_agent(), &[]);
    let (head, _body) = exchange(&mut tls, &request).await.expect("an answer");

    assert_eq!(head.status(), 200);
    assert!(origin.next(WITHIN).await.is_some());
}

#[tokio::test]
async fn an_absolute_form_request_to_a_name_that_resolves_inside_never_connects() {
    let service = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("binds loopback");
    let addr = service.local_addr().expect("an address");
    let (mitm, _ca) = proxy_sending_rebound_to(addr, MitmOptions::default(), None).await;
    let mut tcp = TcpStream::connect(mitm.local_addr())
        .await
        .expect("reaches the proxy");
    let host = format!("{REBOUND}:{}", addr.port());
    let request = format!(
        "GET http://{host}/ HTTP/1.1\r\nHost: {host}\r\nProxy-Authorization: {}\r\n\r\n",
        proxy_auth(&mitm)
    );
    tcp.write_all(request.as_bytes()).await.expect("writes");

    let head = read_head(&mut tcp).await.expect("an answer");

    assert_eq!(head.status(), 502);
    let reached = tokio::time::timeout(QUIET, service.accept()).await;
    assert!(
        reached.is_err(),
        "the proxy connected to a loopback service"
    );
}
