//! One request from a client to the loopback server, and the hello-only captures of the probes.

use std::time::Duration;

use seleniumbase_rs::impersonate::client::{
    impersonation_client, ImpersonationProfile, WireProtocol,
};
use seleniumbase_rs::scrape::runtime::capture::stream::{Capture, CapturedH2c};
use seleniumbase_rs::wire::client_hello::ClientHello;
use seleniumbase_rs::wire::header_spec::RequestShape;
use wreq::ClientBuilder;

use crate::server::{Served, WireServer, HOST};

/// How long a probe waits for a hello before it gives up.
const PROBE_WITHIN: Duration = Duration::from_secs(10);
/// The host name a probe connects to; the client resolves it to the loopback address.
const PROBE_HOST: &str = "probe.test";

/// How the client of an [`exchange`] is set up beyond the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// Whatever the profile negotiates: HTTP/2 against this server.
    Negotiated,
    /// HTTP/1.1 only.
    Http1Only,
}

/// Sends one request with `profile` to a fresh loopback server and returns what arrived.
///
/// # Panics
///
/// Panics when the exchange fails, naming which side did.
pub async fn exchange(
    profile: &ImpersonationProfile,
    shape: RequestShape,
    wire: WireProtocol,
    via: Via,
) -> Served {
    let server = WireServer::bind().await;
    let mut builder = impersonation_client(profile)
        .expect("a client builder")
        .tls_cert_store(server.trust())
        .resolve(HOST, server.addr())
        .no_proxy();
    if via == Via::Http1Only {
        builder = builder.http1_only();
    }
    let client = builder.build().expect("a client");
    let request = profile
        .apply(client.get(server.url()), shape, None, wire)
        .expect("the request headers");
    let (served, response) = tokio::join!(server.serve_one(), request.send());
    let served = served.expect("the server saw a request");
    let response = response.expect("the request completes");
    assert!(response.status().is_success(), "{}", response.status());
    served
}

/// Sends one request with an arbitrary client builder, trusting the loopback server, and
/// returns what arrived.
pub async fn exchange_with(configure: impl FnOnce(ClientBuilder) -> ClientBuilder) -> Served {
    let server = WireServer::bind().await;
    let builder = wreq::Client::builder()
        .tls_cert_store(server.trust())
        .resolve(HOST, server.addr())
        .no_proxy();
    let client = configure(builder).build().expect("a client");
    let (served, response) = tokio::join!(server.serve_one(), client.get(server.url()).send());
    response.expect("the request completes");
    served.expect("the server saw a request")
}

/// Builds a client, makes it dial a listener that reads only the `ClientHello`, and returns
/// that hello.
///
/// The handshake never completes, which is all a probe of the hello needs. A client that cannot
/// be built (an option `BoringSSL` refuses) is an `Err` with the reason.
pub async fn hello_of(
    configure: impl FnOnce(ClientBuilder) -> ClientBuilder,
) -> Result<ClientHello, String> {
    let capture = Capture::bind()
        .await
        .map_err(|error| error.to_string())?
        .with_timeout(PROBE_WITHIN);
    let builder = wreq::Client::builder()
        .resolve(PROBE_HOST, capture.addr())
        .no_proxy()
        .connect_timeout(PROBE_WITHIN);
    let client = configure(builder)
        .build()
        .map_err(|error| format!("{error:?}"))?;
    let url = format!("https://{PROBE_HOST}:{}/", capture.port());
    let (captured, _refused) = tokio::join!(capture.accept_client_hello(), client.get(url).send());
    captured
        .map(|captured| captured.hello)
        .map_err(|error| error.to_string())
}

/// Builds a client for prior-knowledge HTTP/2 over plain TCP, makes it dial a listener that
/// reads the connection start, and returns it.
pub async fn h2c_of(
    configure: impl FnOnce(ClientBuilder) -> ClientBuilder,
) -> Result<CapturedH2c, String> {
    let capture = Capture::bind()
        .await
        .map_err(|error| error.to_string())?
        .with_timeout(PROBE_WITHIN);
    let builder = wreq::Client::builder()
        .http2_only()
        .no_proxy()
        .connect_timeout(PROBE_WITHIN);
    let client = configure(builder)
        .build()
        .map_err(|error| format!("{error:?}"))?;
    let url = format!("http://{}/", capture.addr());
    let (captured, _answered) = tokio::join!(capture.accept_h2c(), client.get(url).send());
    captured.map_err(|error| error.to_string())
}
