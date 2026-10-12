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
//! - And because the proxy covers loopback, it must be the one to refuse: a direct proxy never
//!   connects a page to a loopback or metadata address.
//! - A page opens a `wss://` WebSocket through the proxy to a loopback echo site and receives its
//!   echo; the site sees the measured WebSocket JA4 and Chrome's own handshake fields. A `ws://`
//!   WebSocket is refused by the proxy and never reaches its server. A WebSocket that is a tab's
//!   first request through the proxy fails on the `407`, which Chrome does not retry with the
//!   token (pinned as a known limit).
//!
//! Only the first test compares the browser with the measured persona, so only it needs the
//! measured system (Chrome 155 on macOS) and skips elsewhere (`support/measured_host.rs`). The
//! others are about the proxy's rules and run on any Chrome or Chromium: one whose major is not the
//! measured one is started with the profile's user agent, because the proxy refuses a request that
//! claims another Chrome major, and that refusal is not what these tests are about.
//!
//! The tests start Chrome, so they are ignored by default. Only one Chrome-driving test binary may
//! run on the machine at a time:
//!
//! ```text
//! cargo test --features impersonate --test impersonate_mitm_chrome -- --ignored --test-threads=1
//! ```
#![cfg(feature = "impersonate")]

#[path = "impersonate_mitm/echo.rs"]
mod echo;
#[path = "support/measured_host.rs"]
mod measured_host;
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
use seleniumbase_rs::stealth::fingerprint::OsType;
use seleniumbase_rs::wire::emulation;
use seleniumbase_rs::wire::ja4::Ja4;
use seleniumbase_rs::wire::BrowserKind;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use tokio_btls::SslStream;

use echo::EchoOrigin;
use support::{
    der, new_ca, profile, read_head, start_mitm, tunnel, RecordingOrigin, Seen, ALPN_H2, WITHIN,
};

/// The host of the origin.
const ORIGIN: &str = "origin.test";
/// The host of the WebSocket echo site.
const WS: &str = "ws.test";
/// The host of a plain TCP server that must never be reached.
const PLAIN: &str = "plain.test";
/// What the page sends over its WebSocket.
const GREETING: &str = "hello from Chrome through the proxy";
/// What the attacker's page says; it must never reach the page through the proxy.
const ATTACKER_MARKER: &str = "served by the loopback attacker";
/// How long the attacker's certificate is backdated.
const BACKDATE_SECS: i64 = 3600;
/// How long the attacker's certificate lasts.
const LIFETIME_SECS: i64 = 86_400;

/// Starts Chrome with `builder` and checks it is the measured major, for the tests that compare
/// the browser with the measured persona. Those skip on a system that is not measured
/// (`measured_host`), and fail on a measured one whose Chrome has moved on.
async fn launch(builder: LaunchOptionsBuilder) -> Browser {
    let browser = start(builder).await;
    let (major, product) = browser_major(&browser).await;
    assert_eq!(
        major,
        Some(profile().major()),
        "the installed Chrome ({product}) is not the measured one; run the capture tools again"
    );
    browser
}

/// Starts the installed Chrome or Chromium with `builder`, whatever its major, for the tests that
/// do not depend on the persona: the pin, the destination filter and the WebSocket relay. They run
/// on any system.
///
/// The proxy refuses a request whose user agent claims another Chrome major than its profile
/// (`MitmError::VersionMismatch`), which none of these tests is about. A browser of another major
/// (Chromium 154 on Linux, say) is therefore started again with the profile's user agent; the
/// measured Chrome keeps its own.
async fn launch_any(builder: LaunchOptionsBuilder) -> Browser {
    let browser = start(builder.clone()).await;
    let (major, product) = browser_major(&browser).await;
    if major == Some(profile().major()) {
        return browser;
    }
    eprintln!(
        "{product} is not the measured Chrome {}: started again with the profile's user agent",
        profile().major()
    );
    browser.close().await.expect("Chrome closes");
    start(builder.user_agent(profile().user_agent())).await
}

/// Starts a headless browser with `builder`.
async fn start(builder: LaunchOptionsBuilder) -> Browser {
    let options = builder
        .headless(true)
        .build()
        .expect("valid launch options");
    Browser::launch(options).await.expect("Chrome starts")
}

/// The major version the browser reports, and the product it names.
async fn browser_major(browser: &Browser) -> (Option<u32>, String) {
    let version = browser
        .execute("Browser.getVersion", json!({}))
        .await
        .expect("a version");
    let product = version["product"].as_str().unwrap_or_default().to_owned();
    let major = product
        .split('/')
        .nth(1)
        .and_then(|full| full.split('.').next())
        .and_then(|major| major.parse().ok());
    (major, product)
}

/// The measured Chrome behind `mitm`.
async fn launch_behind(mitm: &MitmProxy) -> Browser {
    launch(mitm.configure_launch(LaunchOptions::builder())).await
}

/// Any Chrome or Chromium behind `mitm` (see [`launch_any`]).
async fn launch_any_behind(mitm: &MitmProxy) -> Browser {
    launch_any(mitm.configure_launch(LaunchOptions::builder())).await
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
    if !measured_host::measured_or_skip(
        "chrome_loads_a_page_through_the_proxy_and_the_origin_sees_the_measured_fingerprint",
    ) {
        return;
    }
    let mut origin = RecordingOrigin::start(&[ALPN_H2, ALPN_HTTP1]).await;
    let options = MitmOptions::default()
        .trust_upstream_root(origin.root())
        .resolve(ORIGIN, origin.addr())
        .allow_private_destinations(true);
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
    assert_measured_navigation(&seen);
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
    // Loopback is allowed here so that the refusal under test is the certificate check.
    let options = MitmOptions::default()
        .ca(Arc::clone(&ca))
        .allow_private_destinations(true);
    let (mitm, _sink) = start_mitm(None, options).await;
    let attacker = Attacker::start(pinned_leaf(&mitm, &der(&ca)).await).await;
    let browser = launch_any_behind(&mitm).await;
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
    let browser = launch_any(LaunchOptions::builder().arg(pin)).await;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn a_page_cannot_use_the_direct_proxy_to_reach_a_loopback_service() {
    let service = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("binds loopback");
    let port = service.local_addr().expect("an address").port();
    let connections = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&connections);
    let counting = tokio::spawn(async move {
        while service.accept().await.is_ok() {
            counted.fetch_add(1, Ordering::SeqCst);
        }
    });
    let (mitm, _sink) = start_mitm(None, MitmOptions::default()).await;
    let browser = launch_any_behind(&mitm).await;
    let page = browser.new_page(None::<&str>).await.expect("a tab");

    // Chrome sends loopback hosts to the proxy (`--proxy-bypass-list=<-loopback>`), which has
    // to be the one to say no. A navigation that fails outright is a refusal too.
    for url in [
        format!("http://127.0.0.1:{port}/"),
        format!("http://localhost:{port}/"),
        format!("https://127.0.0.1:{port}/"),
        format!("http://[::1]:{port}/"),
        format!("http://169.254.169.254:{port}/latest/meta-data/"),
    ] {
        let _loaded = page.goto(&url).await;
        assert_ne!(page_status(&page).await, Some(200), "{url}");
    }

    assert_eq!(
        connections.load(Ordering::SeqCst),
        0,
        "the proxy connected to a loopback service"
    );
    assert!(mitm.stats().refused >= 1, "{:?}", mitm.stats());
    assert_eq!(mitm.stats().forwarded, 0, "{:?}", mitm.stats());
    counting.abort();
    browser.close().await.expect("Chrome closes");
    mitm.shutdown(Duration::from_secs(5)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn a_page_opens_a_websocket_through_the_proxy_and_receives_its_echo() {
    let mut echo = EchoOrigin::start().await;
    let options = MitmOptions::default()
        .trust_upstream_root(echo.root())
        .resolve(WS, echo.addr())
        .allow_private_destinations(true);
    let (mitm, _sink) = start_mitm(None, options).await;
    let browser = launch_any_behind(&mitm).await;
    let page = browser.new_page(None::<&str>).await.expect("a tab");
    let port = echo.addr().port();
    page.goto(format!("https://{WS}:{port}/"))
        .await
        .expect("the page loads");

    let echoed = open_websocket(&page, &format!("wss://{WS}:{port}/echo")).await;
    assert_eq!(echoed, format!("echo:{GREETING}"), "{:?}", mitm.stats());

    let seen = echo
        .next(WITHIN)
        .await
        .expect("the echo site took the handshake");
    let measured = emulation::get(BrowserKind::Chrome, OsType::Macos).expect("measured");
    let websocket = measured
        .tls_websocket
        .as_ref()
        .expect("a measured WebSocket hello")
        .without_known_gaps(&measured.provenance.known_gaps);
    assert_eq!(Ja4::of(&seen.hello), websocket.ja4(), "WebSocket JA4");
    assert_eq!(seen.alpn.as_deref(), Some(&b"http/1.1"[..]));
    let request = &seen.request;
    assert_eq!(request.header("upgrade"), Some("websocket"));
    assert_eq!(request.header("connection"), Some("Upgrade"));
    assert!(request.header("sec-websocket-key").is_some());
    assert_eq!(request.header("sec-websocket-version"), Some("13"));
    let origin = format!("https://{WS}:{port}");
    assert_eq!(request.header("origin"), Some(origin.as_str()));
    let agent = request.header("user-agent").unwrap_or_default();
    assert!(
        agent.contains(&format!("Chrome/{}.", profile().major())),
        "{agent}"
    );
    assert!(request.header("proxy-authorization").is_none());
    let names: Vec<&str> = request
        .headers
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    eprintln!("the WebSocket handshake Chrome sent through the proxy, in order: {names:?}");
    assert!(mitm.stats().websockets >= 1, "{:?}", mitm.stats());

    browser.close().await.expect("Chrome closes");
    mitm.shutdown(Duration::from_secs(5)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn a_plain_ws_websocket_is_refused_by_the_proxy() {
    let echo = EchoOrigin::start().await;
    let plain = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("binds loopback");
    let plain_addr = plain.local_addr().expect("an address");
    let connections = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&connections);
    let counting = tokio::spawn(async move {
        while plain.accept().await.is_ok() {
            counted.fetch_add(1, Ordering::SeqCst);
        }
    });
    let options = MitmOptions::default()
        .trust_upstream_root(echo.root())
        .resolve(WS, echo.addr())
        .resolve(PLAIN, plain_addr)
        .allow_private_destinations(true);
    let (mitm, _sink) = start_mitm(None, options).await;
    let browser = launch_any_behind(&mitm).await;
    let page = browser.new_page(None::<&str>).await.expect("a tab");
    // A navigation through the proxy first, so that Chrome holds the proxy's credentials; then
    // a blank page, which is not a secure context and so may try `ws://`.
    page.goto(format!("https://{WS}:{}/", echo.addr().port()))
        .await
        .expect("the page loads");
    page.goto("about:blank").await.expect("a blank page");
    let before = mitm.stats();

    let outcome = open_websocket(&page, &format!("ws://{PLAIN}:{}/", plain_addr.port())).await;

    let after = mitm.stats();
    eprintln!("a ws:// WebSocket behind the proxy: {outcome}; before {before:?}, after {after:?}");
    assert_eq!(outcome, "error");
    assert_eq!(
        connections.load(Ordering::SeqCst),
        0,
        "the plain server was reached"
    );
    assert!(
        after.refused > before.refused,
        "the proxy itself refused it: {after:?}"
    );
    assert_eq!(after.websockets, 0, "{after:?}");
    counting.abort();
    browser.close().await.expect("Chrome closes");
    mitm.shutdown(Duration::from_secs(5)).await;
}

/// A known limit, pinned: `ProxyAuth` gives Chrome the token over `Fetch.authRequired`, and a
/// WebSocket's `CONNECT` that gets the `407` is not retried with it. A WebSocket therefore works
/// once an earlier request has taken the proxy's credentials (any navigation through the proxy
/// does), and fails when it is a tab's first request. When this test fails because the echo
/// came back, the limit is gone: update the tutorial's MITM section.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "starts the local Chrome"]
async fn a_websocket_before_any_request_through_the_proxy_fails_on_the_407() {
    let echo = EchoOrigin::start().await;
    let options = MitmOptions::default()
        .trust_upstream_root(echo.root())
        .resolve(WS, echo.addr())
        .allow_private_destinations(true);
    let (mitm, _sink) = start_mitm(None, options).await;
    let browser = launch_any_behind(&mitm).await;
    let page = browser.new_page(None::<&str>).await.expect("a tab");
    let before = mitm.stats();

    let outcome = open_websocket(&page, &format!("wss://{WS}:{}/echo", echo.addr().port())).await;

    let after = mitm.stats();
    eprintln!(
        "a wss:// WebSocket as a tab's first request through the proxy: {outcome}; \
         before {before:?}, after {after:?}; the echo site read {} handshakes",
        echo.handshakes()
    );
    assert_eq!(outcome, "error");
    assert!(
        after.unauthorized > before.unauthorized,
        "the CONNECT got the 407: {after:?}"
    );
    assert_eq!(echo.handshakes(), 0, "the handshake reached the site");
    assert_eq!(after.websockets, 0, "{after:?}");
    browser.close().await.expect("Chrome closes");
    mitm.shutdown(Duration::from_secs(5)).await;
}

/// Opens a WebSocket to `url` in `page` and sends [`GREETING`]: `"echo:<text>"` with what came
/// back, `"error"`, `"closed:<code>"` or `"timeout"`.
async fn open_websocket(page: &Page, url: &str) -> String {
    let script = format!(
        r#"new Promise((resolve) => {{
            const socket = new WebSocket("{url}");
            const timer = setTimeout(() => resolve("timeout"), 8000);
            socket.onopen = () => socket.send("{GREETING}");
            socket.onmessage = (event) => {{
                clearTimeout(timer);
                resolve("echo:" + String(event.data));
                socket.close(1000);
            }};
            socket.onerror = () => {{ clearTimeout(timer); resolve("error"); }};
            socket.onclose = (event) => {{ clearTimeout(timer); resolve("closed:" + event.code); }};
        }})"#
    );
    page.evaluate_as(&script).await.expect("evaluates")
}

/// Checks that a navigation reached the origin with the measured JA4, Akamai string and
/// priority, and with Chrome's own identity.
fn assert_measured_navigation(seen: &Seen) {
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
                connections.spawn(async move { answer_once(&acceptor, tcp, &counter).await });
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

/// Runs the attacker's handshake and, if a request follows, answers it with its page.
async fn answer_once(acceptor: &SslAcceptor, tcp: TcpStream, counter: &AtomicUsize) {
    let Ok(ssl) = Ssl::new(acceptor.context()) else {
        return;
    };
    let Ok(mut tls) = SslStream::new(ssl, tcp) else {
        return;
    };
    if Pin::new(&mut tls).accept().await.is_err() || read_head(&mut tls).await.is_none() {
        return;
    }
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
