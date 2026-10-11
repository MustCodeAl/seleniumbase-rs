//! The MITM proxy in front of the real Chrome on this machine.
//!
//! - A page loads through the proxy, and the origin sees the measured JA4 and Akamai string, with
//!   Chrome's own headers and the measured `priority`. The token reaches Chrome only through the
//!   `407` that `ProxyAuth` answers over `Fetch.authRequired`; the time of that navigation is
//!   printed as the measured cost of the `Fetch`-based answer.
//! - A loopback server presenting a chain that carries the proxy's pinned leaf key (public: every
//!   handshake shows it) is never served to the page: Chrome sends the request to the proxy
//!   (`--proxy-bypass-list=<-loopback>`), whose upstream verification refuses the chain.
//! - The control: Chrome with the pin but without the proxy does accept that chain, which is why
//!   the proxy must cover loopback hosts.
//!
//! The tests start Chrome, so they are ignored by default. Only one Chrome-driving test binary may
//! run on the machine at a time:
//!
//! ```text
//! cargo test --features impersonate --test impersonate_mitm_chrome -- --ignored --test-threads=1
//! ```
#![cfg(feature = "impersonate")]

#[path = "impersonate_mitm/support.rs"]
mod support;

use std::net::Ipv4Addr;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use btls::asn1::Asn1Time;
use btls::bn::BigNum;
use btls::ec::{EcGroup, EcKey};
use btls::hash::MessageDigest;
use btls::nid::Nid;
use btls::pkey::{PKey, Private};
use btls::ssl::{Ssl, SslAcceptor, SslMethod};
use btls::x509::extension::SubjectAlternativeName;
use btls::x509::{X509Name, X509};
use seleniumbase_rs::impersonate::mitm::{MitmOptions, MitmProxy};
use seleniumbase_rs::impersonate::tls_server::ALPN_HTTP1;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, LaunchOptionsBuilder, Page};
use seleniumbase_rs::wire::ja4::Ja4;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio_btls::SslStream;

use support::{
    der, new_ca, profile, read_head, start_mitm, tunnel, RecordingOrigin, ALPN_H2, WITHIN,
};

/// The host of the origin.
const ORIGIN: &str = "origin.test";
/// What the attacker's page says; it must never reach the page through the proxy.
const ATTACKER_MARKER: &str = "served by the loopback attacker";
/// How long the attacker's certificate is backdated.
const BACKDATE_SECS: i64 = 3600;
/// How long the attacker's certificate lasts.
const LIFETIME_SECS: i64 = 86_400;

/// Starts Chrome with `builder` and checks it is the measured major.
async fn launch(builder: LaunchOptionsBuilder) -> Browser {
    let options = builder
        .headless(true)
        .build()
        .expect("valid launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let version = browser
        .execute("Browser.getVersion", json!({}))
        .await
        .expect("a version");
    let product = version["product"].as_str().unwrap_or_default().to_owned();
    let major: Option<u32> = product
        .split('/')
        .nth(1)
        .and_then(|full| full.split('.').next())
        .and_then(|major| major.parse().ok());
    assert_eq!(
        major,
        Some(profile().major()),
        "the installed Chrome ({product}) is not the measured one; run the capture tools again"
    );
    browser
}

/// Chrome behind `mitm`.
async fn launch_behind(mitm: &MitmProxy) -> Browser {
    launch(mitm.configure_launch(LaunchOptions::builder())).await
}

/// The text of the page, or an empty string.
async fn page_html(page: &Page) -> String {
    page.evaluate_as("document.documentElement.outerHTML")
        .await
        .unwrap_or_default()
}

/// The HTTP status of the page's navigation, as the page saw it.
async fn page_status(page: &Page) -> Option<u64> {
    page.evaluate_as("performance.getEntriesByType('navigation')[0]?.responseStatus ?? null")
        .await
        .ok()
        .flatten()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn chrome_loads_a_page_through_the_proxy_and_the_origin_sees_the_measured_fingerprint() {
    let mut origin = RecordingOrigin::start(&[ALPN_H2, ALPN_HTTP1]).await;
    let options = MitmOptions::default()
        .trust_upstream_root(origin.root())
        .resolve(ORIGIN, origin.addr());
    let (mitm, _sink) = start_mitm(None, options).await;
    let browser = launch_behind(&mitm).await;
    let page = browser.new_page(None::<&str>).await.expect("a tab");
    let url = format!("https://{ORIGIN}:{}/", origin.addr().port());

    let started = Instant::now();
    page.goto(&url).await.expect("the page loads");
    let elapsed = started.elapsed();

    let marker: String = page
        .evaluate_as("document.getElementById('marker')?.textContent ?? ''")
        .await
        .expect("evaluates");
    assert_eq!(marker, "served by the origin");
    let seen = origin
        .next(WITHIN)
        .await
        .expect("the origin saw the navigation");
    let measured = profile();
    assert_eq!(Ja4::of(&seen.hello), measured.tls_spec().ja4(), "JA4");
    let preface = seen.h2();
    assert_eq!(
        preface.akamai().expect("an Akamai string"),
        measured.http2_spec().akamai(),
        "Akamai string"
    );
    assert_eq!(preface.header("sec-fetch-dest"), Some("document"));
    assert_eq!(
        preface.header("priority"),
        measured.header_spec().priority_for("document")
    );
    let agent = preface.header("user-agent").unwrap_or_default();
    assert!(
        agent.contains(&format!("Chrome/{}.", measured.major())),
        "{agent}"
    );
    let stats = mitm.stats();
    assert!(stats.tunnels >= 1, "{stats:?}");
    assert!(
        stats.unauthorized >= 1,
        "Chrome got the token from a 407: {stats:?}"
    );
    eprintln!("navigation through the proxy, Fetch-based proxy auth included: {elapsed:?}");

    browser.close().await.expect("Chrome closes");
    mitm.shutdown(Duration::from_secs(5)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn a_loopback_server_carrying_the_pinned_key_is_never_served_to_the_page() {
    let ca = new_ca();
    let (mitm, _sink) = start_mitm(None, MitmOptions::default().ca(Arc::clone(&ca))).await;
    let attacker = Attacker::start(pinned_leaf(&mitm, &der(&ca)).await).await;
    let browser = launch_behind(&mitm).await;
    let page = browser.new_page(None::<&str>).await.expect("a tab");

    // A navigation that fails outright is a refusal too; only what loads matters.
    let _loaded = page
        .goto(format!("https://127.0.0.1:{}/", attacker.port))
        .await;

    let html = page_html(&page).await;
    assert!(
        !html.contains(ATTACKER_MARKER),
        "the attacker's page was served"
    );
    assert_eq!(page_status(&page).await, Some(502), "{html}");
    assert_eq!(attacker.requests(), 0, "the attacker answered a request");
    assert!(mitm.stats().upstream_errors >= 1, "{:?}", mitm.stats());

    browser.close().await.expect("Chrome closes");
    mitm.shutdown(Duration::from_secs(5)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn without_the_proxy_the_leaf_pin_alone_trusts_a_chain_carrying_the_leaf_key() {
    let ca = new_ca();
    let (mitm, _sink) = start_mitm(None, MitmOptions::default().ca(Arc::clone(&ca))).await;
    let attacker = Attacker::start(pinned_leaf(&mitm, &der(&ca)).await).await;
    let pin = mitm
        .chrome_args()
        .into_iter()
        .find(|arg| arg.starts_with("--ignore-certificate-errors-spki-list="))
        .expect("the pin switch");
    let browser = launch(LaunchOptions::builder().arg(pin)).await;
    let page = browser.new_page(None::<&str>).await.expect("a tab");

    let _loaded = page
        .goto(format!("https://127.0.0.1:{}/", attacker.port))
        .await;

    let html = page_html(&page).await;
    assert!(
        html.contains(ATTACKER_MARKER),
        "Chrome refused the chain; the documented trust of the pin switch does not hold: {html}"
    );
    browser.close().await.expect("Chrome closes");
    mitm.shutdown(Duration::from_secs(5)).await;
}

/// The proxy's leaf certificate, as any client of the proxy sees it in the handshake.
async fn pinned_leaf(mitm: &MitmProxy, ca_der: &[u8]) -> X509 {
    let tls = tunnel(mitm, "any.test", 443, ca_der).await;
    tls.ssl()
        .peer_certificate()
        .expect("the proxy presents its leaf")
}

/// A loopback HTTPS server whose chain is its own self-signed certificate for `127.0.0.1`
/// followed by the proxy's leaf, which carries the key Chrome pins.
struct Attacker {
    port: u16,
    requests: Arc<AtomicUsize>,
    _tasks: JoinSet<()>,
}

impl Attacker {
    async fn start(pinned: X509) -> Self {
        let (key, cert) = self_signed_loopback();
        let mut builder =
            SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).expect("an acceptor");
        builder.set_private_key(&key).expect("the key");
        builder.set_certificate(&cert).expect("the certificate");
        builder
            .add_extra_chain_cert(pinned)
            .expect("the pinned leaf");
        let acceptor = Arc::new(builder.build());
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("binds loopback");
        let port = listener.local_addr().expect("an address").port();
        let requests = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&requests);
        let mut tasks = JoinSet::new();
        tasks.spawn(async move {
            let mut connections = JoinSet::new();
            while let Ok((tcp, _peer)) = listener.accept().await {
                let (acceptor, counter) = (Arc::clone(&acceptor), Arc::clone(&counter));
                connections.spawn(async move {
                    let Ok(ssl) = Ssl::new(acceptor.context()) else {
                        return;
                    };
                    let Ok(mut tls) = SslStream::new(ssl, tcp) else {
                        return;
                    };
                    if Pin::new(&mut tls).accept().await.is_err() {
                        return;
                    }
                    if read_head(&mut tls).await.is_some() {
                        counter.fetch_add(1, Ordering::SeqCst);
                        let body = format!("<p>{ATTACKER_MARKER}</p>");
                        let reply = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
                             Connection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _written = tls.write_all(reply.as_bytes()).await;
                        let _closed = tls.shutdown().await;
                    }
                });
                while connections.try_join_next().is_some() {}
            }
        });
        Self {
            port,
            requests,
            _tasks: tasks,
        }
    }

    fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

/// A P-256 key and a self-signed certificate for the IP `127.0.0.1`.
fn self_signed_loopback() -> (PKey<Private>, X509) {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).expect("P-256");
    let key = PKey::from_ec_key(EcKey::generate(&group).expect("a key")).expect("a key");
    let mut name = X509Name::builder().expect("a name");
    name.append_entry_by_nid(Nid::COMMONNAME, "127.0.0.1")
        .expect("a common name");
    let name = name.build();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs();
    let now = i64::try_from(now).expect("fits");
    let mut builder = X509::builder().expect("a builder");
    builder.set_version(2).expect("v3");
    let serial = BigNum::from_u32(7)
        .and_then(|number| number.to_asn1_integer())
        .expect("a serial");
    builder.set_serial_number(&serial).expect("the serial");
    builder.set_subject_name(&name).expect("the subject");
    builder.set_issuer_name(&name).expect("the issuer");
    builder.set_pubkey(&key).expect("the key");
    let start = Asn1Time::from_unix(now - BACKDATE_SECS).expect("a start");
    let end = Asn1Time::from_unix(now + LIFETIME_SECS).expect("an end");
    builder.set_not_before(&start).expect("the start");
    builder.set_not_after(&end).expect("the end");
    let mut san = SubjectAlternativeName::new();
    san.ip("127.0.0.1");
    let san = san
        .build(&builder.x509v3_context(None, None))
        .expect("a SAN");
    builder.append_extension(&san).expect("the SAN");
    builder.sign(&key, MessageDigest::sha256()).expect("signed");
    (key, builder.build())
}
