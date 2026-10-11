//! Real TLS handshakes over loopback sockets, leaf issuance and the CA's key hygiene.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use btls::nid::Nid;
use btls::pkey::{PKeyRef, Private};
use btls::ssl::{Ssl, SslAlert, SslConnector, SslMethod, SslVerifyError, SslVerifyMode};
use btls::x509::X509Ref;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_btls::SslStream;
use url::Host;

use super::{
    accept, acceptor, alpn_ids, select_alpn, spki_pin_of, CaOptions, EphemeralCa, Leaf,
    TlsServerError, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT, DEFAULT_LEAF_CAPACITY, MAX_CA_VALIDITY,
};

/// What the client writes once the handshake is done.
const PING: &[u8; 4] = b"ping";
/// What the server answers.
const PONG: &[u8; 4] = b"pong";
/// Chrome's ALPN offer: h2 first, then HTTP/1.1.
const CHROME_ALPN: &[u8] = b"\x02h2\x08http/1.1";
/// An offer the browser-to-proxy leg must refuse.
const H2_ONLY_ALPN: &[u8] = b"\x02h2";
/// A fixed clock for the pure leaf-lifetime tests: 2027-01-15T08:00:00Z.
const FIXED_NOW: u64 = 1_800_000_000;
/// Files larger than this are not scanned for key material (none of ours is written at all).
const MAX_SCANNED_FILE: u64 = 8 * 1024 * 1024;

fn host(name: &str) -> Host<String> {
    Host::parse(name).expect("test host parses")
}

fn fixed_now() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(FIXED_NOW)
}

/// A client that trusts a chain only when one of its certificates has one of `pins`, the way
/// Chrome's `--ignore-certificate-errors-spki-list` does, and trusts nothing else.
fn pinned_connector(pins: Vec<String>, alpn: &[u8]) -> SslConnector {
    let mut builder = SslConnector::builder(SslMethod::tls()).expect("connector builder");
    builder.set_alpn_protos(alpn).expect("client ALPN offer");
    builder.set_custom_verify_callback(SslVerifyMode::PEER, move |ssl| {
        let refuse = SslVerifyError::Invalid(SslAlert::BAD_CERTIFICATE);
        let chain = ssl.peer_cert_chain().ok_or(refuse)?;
        let pinned = chain
            .iter()
            .any(|cert| spki_pin_of(cert).is_ok_and(|pin| pins.contains(&pin)));
        if pinned {
            Ok(())
        } else {
            Err(refuse)
        }
    });
    builder.build()
}

/// A client TLS session for `server_name`; SNI is sent only for DNS names and when `sni` holds.
fn client_ssl(connector: &SslConnector, server_name: &str, sni: bool) -> Ssl {
    connector
        .configure()
        .expect("client configuration")
        .use_server_name_indication(sni)
        .into_ssl(server_name)
        .expect("client session")
}

/// Accepts one connection on a loopback port with `leaf`, answers `ping` with `pong`.
async fn serve_once(leaf: Arc<Leaf>) -> (u16, JoinHandle<Result<(), TlsServerError>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let port = listener.local_addr().expect("local address").port();
    let server = acceptor(&[ALPN_HTTP1]).expect("acceptor");
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.expect("accept tcp");
        let mut tls = accept(&server, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT).await?;
        let mut buf = [0_u8; 4];
        tls.read_exact(&mut buf).await.expect("read ping");
        assert_eq!(&buf, PING);
        tls.write_all(PONG).await.expect("write pong");
        tls.flush().await.expect("flush pong");
        Ok::<(), TlsServerError>(())
    });
    (port, task)
}

/// Connects to `port` and runs the client handshake.
async fn connect(port: u16, ssl: Ssl) -> Result<SslStream<TcpStream>, btls::ssl::Error> {
    let tcp = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("connect tcp");
    let mut tls = SslStream::new(ssl, tcp).expect("client stream");
    Pin::new(&mut tls).connect().await?;
    Ok(tls)
}

/// Sends `ping` and expects `pong`.
async fn ping(tls: &mut SslStream<TcpStream>) {
    tls.write_all(PING).await.expect("write ping");
    let mut buf = [0_u8; 4];
    tls.read_exact(&mut buf).await.expect("read pong");
    assert_eq!(&buf, PONG);
}

fn dns_names(cert: &X509Ref) -> Vec<String> {
    cert.subject_alt_names()
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.dnsname().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn ip_addresses(cert: &X509Ref) -> Vec<Vec<u8>> {
    cert.subject_alt_names()
        .map(|names| {
            names
                .iter()
                .filter_map(|name| name.ipaddress().map(<[u8]>::to_vec))
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn a_client_pinned_to_either_spki_reaches_the_leaf_of_its_host() {
    let ca = EphemeralCa::generate().expect("CA");
    let leaf = ca
        .leaf_for(&host("example.test"), SystemTime::now())
        .expect("leaf");
    for pin in ca.spki_pins() {
        let (port, server) = serve_once(Arc::clone(&leaf)).await;
        let connector = pinned_connector(vec![pin], CHROME_ALPN);
        let mut tls = connect(port, client_ssl(&connector, "example.test", true))
            .await
            .expect("handshake with a pinned key");
        assert_eq!(tls.ssl().selected_alpn_protocol(), Some(ALPN_HTTP1));
        let peer = tls.ssl().peer_certificate().expect("server certificate");
        assert_eq!(dns_names(&peer), vec!["example.test".to_owned()]);
        assert_eq!(ip_addresses(&peer), Vec::<Vec<u8>>::new());
        ping(&mut tls).await;
        server.await.expect("server task").expect("server side");
    }
}

#[tokio::test]
async fn a_client_pinned_to_another_ca_fails_the_handshake() {
    let ca = EphemeralCa::generate().expect("CA");
    let other = EphemeralCa::generate().expect("other CA");
    let leaf = ca
        .leaf_for(&host("example.test"), SystemTime::now())
        .expect("leaf");
    let (port, server) = serve_once(leaf).await;
    let connector = pinned_connector(other.spki_pins().to_vec(), CHROME_ALPN);
    let client = connect(port, client_ssl(&connector, "example.test", true)).await;
    assert!(client.is_err(), "a wrong pin must fail the handshake");
    let outcome = server.await.expect("server task");
    assert!(
        matches!(outcome, Err(TlsServerError::Handshake(_))),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn an_ip_host_is_served_without_sni() {
    let ca = EphemeralCa::generate().expect("CA");
    let leaf = ca
        .leaf_for(&host("127.0.0.1"), SystemTime::now())
        .expect("leaf");
    let (port, server) = serve_once(leaf).await;
    let connector = pinned_connector(ca.spki_pins().to_vec(), CHROME_ALPN);
    let mut tls = connect(port, client_ssl(&connector, "127.0.0.1", true))
        .await
        .expect("handshake");
    let peer = tls.ssl().peer_certificate().expect("server certificate");
    assert_eq!(ip_addresses(&peer), vec![vec![127, 0, 0, 1]]);
    assert_eq!(dns_names(&peer), Vec::<String>::new());
    ping(&mut tls).await;
    server.await.expect("server task").expect("server side");
}

#[tokio::test]
async fn an_sni_naming_another_host_is_refused() {
    for (sni_name, send_sni) in [("other.test", true), ("example.test", false)] {
        let ca = EphemeralCa::generate().expect("CA");
        let leaf = ca
            .leaf_for(&host("example.test"), SystemTime::now())
            .expect("leaf");
        let (port, server) = serve_once(leaf).await;
        let connector = pinned_connector(ca.spki_pins().to_vec(), CHROME_ALPN);
        let client = connect(port, client_ssl(&connector, sni_name, send_sni)).await;
        let outcome = server.await.expect("server task");
        match outcome {
            Err(TlsServerError::SniMismatch { expected, sni }) => {
                assert_eq!(expected, "example.test");
                assert_eq!(sni.as_deref(), send_sni.then_some(sni_name));
            }
            other => panic!("expected an SNI mismatch, got {other:?}"),
        }
        if let Ok(mut tls) = client {
            let mut buf = [0_u8; 1];
            assert!(
                !matches!(tls.read(&mut buf).await, Ok(n) if n > 0),
                "a refused connection carries no data"
            );
        }
    }
}

#[tokio::test]
async fn a_client_without_http1_in_its_alpn_offer_is_refused() {
    let ca = EphemeralCa::generate().expect("CA");
    let leaf = ca
        .leaf_for(&host("example.test"), SystemTime::now())
        .expect("leaf");
    let (port, server) = serve_once(leaf).await;
    let connector = pinned_connector(ca.spki_pins().to_vec(), H2_ONLY_ALPN);
    let client = connect(port, client_ssl(&connector, "example.test", true)).await;
    assert!(client.is_err(), "no common ALPN protocol must fail");
    let outcome = server.await.expect("server task");
    assert!(
        matches!(outcome, Err(TlsServerError::Handshake(_))),
        "{outcome:?}"
    );
}

#[test]
fn alpn_selection_follows_the_server_preference_and_ignores_a_malformed_tail() {
    let preferences = vec![ALPN_HTTP1.to_vec()];
    assert_eq!(select_alpn(&preferences, CHROME_ALPN), Some(ALPN_HTTP1));
    assert_eq!(select_alpn(&preferences, H2_ONLY_ALPN), None);
    assert_eq!(select_alpn(&preferences, b"\x02h2\x09http/1.1"), None);
    assert_eq!(select_alpn(&preferences, b""), None);
    assert_eq!(alpn_ids(b"\x02h2\x00\x08http/1.1").count(), 1);
    assert!(matches!(acceptor(&[]), Err(TlsServerError::InvalidAlpn(_))));
    assert!(matches!(
        acceptor(&[b""]),
        Err(TlsServerError::InvalidAlpn(_))
    ));
    let long = [b'x'; 256];
    assert!(matches!(
        acceptor(&[&long]),
        Err(TlsServerError::InvalidAlpn(_))
    ));
}

#[test]
fn the_ca_is_a_p256_root_and_its_leaves_chain_to_it() {
    let ca = EphemeralCa::generate().expect("CA");
    let ca_key = ca.certificate().public_key().expect("CA public key");
    assert!(ca.certificate().verify(&ca_key).expect("self signature"));
    let curve = ca_key.ec_key().expect("EC key").group().curve_name();
    assert_eq!(curve, Some(Nid::X9_62_PRIME256V1));
    let leaf = ca
        .leaf_for(&host("example.test"), SystemTime::now())
        .expect("leaf");
    assert!(leaf.certificate().verify(&ca_key).expect("leaf signature"));
    let [ca_pin, leaf_pin] = ca.spki_pins();
    assert_eq!(spki_pin_of(ca.certificate()).expect("CA pin"), ca_pin);
    assert_eq!(spki_pin_of(leaf.certificate()).expect("leaf pin"), leaf_pin);
    assert_eq!(spki_pin_of(leaf.issuer()).expect("issuer pin"), ca_pin);
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&ca_pin)
        .expect("pin is base64");
    assert_eq!(decoded.len(), 32, "a pin is a SHA-256 digest");
    assert_eq!(
        ca.chrome_spki_arg(),
        format!("--ignore-certificate-errors-spki-list={ca_pin},{leaf_pin}")
    );
    let other = ca
        .leaf_for(&host("other.test"), SystemTime::now())
        .expect("leaf");
    assert_eq!(spki_pin_of(other.certificate()).expect("pin"), leaf_pin);
}

#[test]
fn the_same_host_gets_the_same_leaf() {
    let ca = EphemeralCa::generate_at(CaOptions::default(), fixed_now()).expect("CA");
    let first = ca
        .leaf_for(&host("example.test"), fixed_now())
        .expect("leaf");
    let again = ca
        .leaf_for(&Host::Domain("Example.TEST".to_owned()), fixed_now())
        .expect("leaf");
    assert!(Arc::ptr_eq(&first, &again));
    let other = ca.leaf_for(&host("other.test"), fixed_now()).expect("leaf");
    assert!(!Arc::ptr_eq(&first, &other));
    assert_eq!(ca.cached_leaves(), 2);
    assert!(matches!(
        ca.leaf_for(&Host::Domain("*.example.test".to_owned()), fixed_now()),
        Err(TlsServerError::InvalidHost(_))
    ));
}

#[test]
fn the_leaf_cache_evicts_the_least_recently_used_leaf_at_its_bound() {
    let ca = EphemeralCa::generate_at(CaOptions::default(), fixed_now()).expect("CA");
    let leaf = |i: usize| {
        ca.leaf_for(&host(&format!("h{i}.test")), fixed_now())
            .expect("leaf")
    };
    let first: Vec<Arc<Leaf>> = (0..DEFAULT_LEAF_CAPACITY).map(leaf).collect();
    assert_eq!(ca.cached_leaves(), DEFAULT_LEAF_CAPACITY);
    assert!(
        Arc::ptr_eq(&leaf(0), &first[0]),
        "touching h0 makes h1 the oldest"
    );
    let newest = leaf(DEFAULT_LEAF_CAPACITY);
    assert_eq!(ca.cached_leaves(), DEFAULT_LEAF_CAPACITY);
    assert!(Arc::ptr_eq(&leaf(DEFAULT_LEAF_CAPACITY), &newest));
    assert!(
        Arc::ptr_eq(&leaf(0), &first[0]),
        "h0 was used recently and stays"
    );
    assert!(
        !Arc::ptr_eq(&leaf(1), &first[1]),
        "h1 was evicted and is issued again"
    );
    assert_eq!(ca.cached_leaves(), DEFAULT_LEAF_CAPACITY);
}

#[test]
fn a_leaf_near_expiry_is_issued_again() {
    let options = CaOptions::default();
    let ca = EphemeralCa::generate_at(options, fixed_now()).expect("CA");
    let name = host("example.test");
    let first = ca.leaf_for(&name, fixed_now()).expect("leaf");
    let renew_at = fixed_now() + options.leaf_validity - options.leaf_renew_margin;
    let before = ca
        .leaf_for(&name, renew_at - Duration::from_secs(1))
        .expect("leaf");
    assert!(
        Arc::ptr_eq(&first, &before),
        "still fresh one second earlier"
    );
    let renewed = ca.leaf_for(&name, renew_at).expect("leaf");
    assert!(
        !Arc::ptr_eq(&first, &renewed),
        "inside the margin it is issued again"
    );
    assert!(renewed.not_after_unix() > first.not_after_unix());
    let serial = |leaf: &Leaf| {
        let serial = leaf.certificate().serial_number().to_bn().expect("serial");
        serial.to_vec()
    };
    assert_ne!(serial(&renewed), serial(&first));
    assert_eq!(ca.cached_leaves(), 1);
    let rewound = ca
        .leaf_for(&name, fixed_now() - Duration::from_hours(2))
        .expect("leaf");
    assert!(
        !Arc::ptr_eq(&renewed, &rewound),
        "a clock before not_before re-issues"
    );
}

#[test]
fn leaves_end_with_the_ca_and_a_ca_about_to_expire_issues_none() {
    let options = CaOptions {
        ca_validity: Duration::from_hours(48),
        ..CaOptions::default()
    };
    let ca = EphemeralCa::generate_at(options, fixed_now()).expect("CA");
    let name = host("example.test");
    let late = fixed_now() + Duration::from_hours(36);
    let leaf = ca.leaf_for(&name, late).expect("leaf");
    assert_eq!(leaf.not_after_unix(), ca.not_after_unix());
    let margin = options.leaf_renew_margin.as_secs();
    let too_late = UNIX_EPOCH
        + Duration::from_secs(u64::try_from(ca.not_after_unix()).expect("positive") - margin);
    assert!(matches!(
        ca.leaf_for(&name, too_late),
        Err(TlsServerError::CaExpired)
    ));
}

#[test]
fn options_outside_their_bounds_are_refused() {
    let base = CaOptions::default();
    let day = Duration::from_hours(24);
    let bad = [
        CaOptions {
            leaf_capacity: 0,
            ..base
        },
        CaOptions {
            leaf_capacity: super::MAX_LEAF_CAPACITY + 1,
            ..base
        },
        CaOptions {
            ca_validity: MAX_CA_VALIDITY + day,
            ..base
        },
        CaOptions {
            leaf_validity: base.ca_validity + day,
            ..base
        },
        CaOptions {
            leaf_renew_margin: base.leaf_validity,
            ..base
        },
    ];
    for options in bad {
        assert!(
            matches!(
                EphemeralCa::generate_at(options, fixed_now()),
                Err(TlsServerError::InvalidOptions(_))
            ),
            "{options:?}"
        );
    }
    assert!(matches!(
        EphemeralCa::generate_at(base, UNIX_EPOCH - day),
        Err(TlsServerError::ClockOutOfRange)
    ));
}

/// Every form the private keys could take in text or bytes.
fn key_material(key: &PKeyRef<Private>) -> Vec<Vec<u8>> {
    let der = key.private_key_to_der().expect("DER");
    let pkcs8 = key.private_key_to_der_pkcs8().expect("PKCS#8");
    let scalar = key
        .ec_key()
        .expect("EC key")
        .private_key()
        .to_owned()
        .expect("scalar");
    let hex = scalar.to_hex_str().expect("hex").to_string();
    let engine = base64::engine::general_purpose::STANDARD;
    vec![
        engine.encode(&der).into_bytes(),
        engine.encode(&pkcs8).into_bytes(),
        hex.to_ascii_uppercase().into_bytes(),
        hex.to_ascii_lowercase().into_bytes(),
        scalar.to_vec(),
        der,
        pkcs8,
    ]
}

fn ca_key_material(ca: &EphemeralCa) -> Vec<Vec<u8>> {
    let mut forms = key_material(&ca.key);
    forms.extend(key_material(&ca.leaf_key));
    forms
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[test]
fn debug_output_never_shows_a_private_key() {
    let ca = EphemeralCa::generate().expect("CA");
    let leaf = ca
        .leaf_for(&host("example.test"), SystemTime::now())
        .expect("leaf");
    let shown = format!("{ca:?}\n{ca:#?}\n{leaf:?}\n{leaf:#?}");
    for secret in ca_key_material(&ca) {
        assert!(
            !contains(shown.as_bytes(), &secret),
            "Debug leaked key material"
        );
    }
    assert!(shown.contains("<redacted>"));
    assert!(
        shown.contains(&ca.spki_pins()[0]),
        "the public pins stay visible"
    );
}

/// Regular files of `dir` with their length and modification time.
fn listing(dir: &Path) -> HashMap<PathBuf, (u64, Option<SystemTime>)> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| {
                    let meta = entry.metadata().ok().filter(std::fs::Metadata::is_file)?;
                    Some((entry.path(), (meta.len(), meta.modified().ok())))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn the_ca_keys_never_reach_the_filesystem() {
    let dirs = [
        std::env::temp_dir(),
        std::env::current_dir().expect("working directory"),
    ];
    let before: Vec<_> = dirs.iter().map(|dir| listing(dir)).collect();
    let ca = EphemeralCa::generate().expect("CA");
    let leaf = ca
        .leaf_for(&host("example.test"), SystemTime::now())
        .expect("leaf");
    let (port, server) = serve_once(leaf).await;
    let connector = pinned_connector(ca.spki_pins().to_vec(), CHROME_ALPN);
    let mut tls = connect(port, client_ssl(&connector, "example.test", true))
        .await
        .expect("handshake");
    ping(&mut tls).await;
    server.await.expect("server task").expect("server side");
    let secrets = ca_key_material(&ca);
    for (dir, old) in dirs.iter().zip(&before) {
        for (path, stamp) in listing(dir) {
            if old.get(&path) == Some(&stamp) || stamp.0 > MAX_SCANNED_FILE {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            for secret in &secrets {
                assert!(
                    !contains(&bytes, secret),
                    "{} holds key material",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn the_tls_server_code_has_no_file_io_and_no_key_export() {
    let sources = [
        ("mod.rs", include_str!("mod.rs")),
        ("ca.rs", include_str!("ca.rs")),
        ("cache.rs", include_str!("cache.rs")),
        ("host.rs", include_str!("host.rs")),
    ];
    let forbidden = [
        "std::fs",
        "fs::",
        "File::",
        "OpenOptions",
        "tempfile",
        "private_key_to",
    ];
    for (file, source) in sources {
        for needle in forbidden {
            assert!(!source.contains(needle), "{file} mentions {needle}");
        }
    }
}
