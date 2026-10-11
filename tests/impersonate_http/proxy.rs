//! The egress as a proxy: where the traffic goes, how the credentials travel, and what the
//! failure says.

use seleniumbase_rs::impersonate::http_session::{HttpError, HttpOptions, HttpSession};
use seleniumbase_rs::scrape::passport::identity::EmulationRef;
use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::BrowserKind;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::site::{egress, fingerprint, identity_of, measured_major};

/// A proxy that reads the head of the first request it gets, refuses it with a 502, and hands the
/// head back.
async fn refusing_proxy() -> (u16, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.expect("binds");
    let port = listener.local_addr().expect("an address").port();
    let task = tokio::spawn(async move {
        let (mut stream, _peer) = listener.accept().await.expect("a client");
        let mut head = Vec::new();
        let mut buf = [0_u8; 1024];
        while !head.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buf).await.expect("reads");
            if read == 0 {
                break;
            }
            head.extend_from_slice(&buf[..read]);
        }
        let refusal = b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\n\r\n";
        stream.write_all(refusal).await.expect("answers");
        String::from_utf8_lossy(&head).into_owned()
    });
    (port, task)
}

fn session_behind(spec: &str) -> HttpSession {
    let proxy = egress(spec);
    let major = measured_major();
    let base = identity_of(
        fingerprint(OsType::Macos, major),
        EmulationRef {
            kind: BrowserKind::Chrome,
            major,
            os: OsType::Macos,
        },
        Some(proxy.key().clone()),
    );
    HttpSession::from_identity(&base, Some(proxy), HttpOptions::new()).expect("a session")
}

#[tokio::test]
async fn an_http_proxy_gets_the_target_and_the_credentials_in_a_header() {
    let (port, task) = refusing_proxy().await;
    let session = session_behind(&format!("http://alice:s3cret@127.0.0.1:{port}"));

    let error = session
        .get("https://shop.test/account?token=hunter2")
        .await
        .expect_err("the proxy refuses");
    let head = task.await.expect("the proxy task");

    assert!(head.starts_with("CONNECT shop.test:443 "), "{head}");
    assert!(
        head.to_ascii_lowercase().contains("proxy-authorization: "),
        "{head}"
    );
    assert!(head.contains("Basic YWxpY2U6czNjcmV0"), "{head}");
    assert!(
        !head.contains("hunter2"),
        "the query never reaches the proxy: {head}"
    );
    assert!(matches!(&error, HttpError::Request { .. }), "{error}");
    let shown = format!("{error} {error:?} {session:?}");
    for secret in ["s3cret", "alice", "hunter2", "YWxpY2U"] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
}

#[tokio::test]
async fn a_proxy_without_credentials_gets_no_authorization() {
    let (port, task) = refusing_proxy().await;
    let session = session_behind(&format!("http://127.0.0.1:{port}"));

    let error = session
        .get("https://shop.test/")
        .await
        .expect_err("the proxy refuses");
    let head = task.await.expect("the proxy task").to_ascii_lowercase();

    assert!(!head.contains("proxy-authorization"), "{head}");
    assert!(matches!(error, HttpError::Request { .. }), "{error}");
}

#[tokio::test]
async fn the_bypass_list_sends_a_host_around_the_proxy() {
    let (port, task) = refusing_proxy().await;
    let proxy = egress(&format!("http://127.0.0.1:{port}"));
    let major = measured_major();
    let base = identity_of(
        fingerprint(OsType::Macos, major),
        EmulationRef {
            kind: BrowserKind::Chrome,
            major,
            os: OsType::Macos,
        },
        Some(proxy.key().clone()),
    );
    let closed = std::net::SocketAddr::from(([127, 0, 0, 1], 1));
    let options = HttpOptions::new()
        .bypass("direct.test")
        .resolve("direct.test", closed);
    let session = HttpSession::from_identity(&base, Some(proxy), options).expect("a session");

    let error = session
        .get("https://direct.test:1/")
        .await
        .expect_err("nothing listens on that port");
    task.abort();

    assert!(matches!(error, HttpError::Request { .. }), "{error}");
    let shown = format!("{error}");
    assert!(
        !shown.contains("proxy"),
        "the failure is the direct connection's, not the proxy's: {shown}"
    );
}
