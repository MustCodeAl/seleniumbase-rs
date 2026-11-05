---
title: "mimic::mitm"
sidebarTitle: "MITM proxy"
description: "MitmProxy: a loopback proxy that replays Chrome's requests with the measured handshake."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: mimic</Badge>

`MitmProxy` binds on loopback, ends Chrome's TLS with a leaf of an in-memory CA, and replays each request through the impersonating client. It requires a random proxy token, refuses destinations it will not intercept, and never tunnels blind.

The guide is [MITM proxy](/stealth/advanced/mitm-proxy). The CA is [TLS server](/stealth/api/mimic-tls-server).

<RequestExample>

```rust Options for the loopback proxy
use seleniumbase_rs_stealth::mimic::MitmOptions;

fn main() {
    let options = MitmOptions::default();
    let _ = options;
}
```

</RequestExample>

## Constants and type aliases

```rust
// mitm/mod.rs
pub const PROXY_USER: &str = "sb";
pub const PRIME_HOST: &str = "sb-mitm.invalid";
pub const PROXY_EVERYTHING_ARG: &str = "--proxy-bypass-list=<-loopback>";
pub const DISABLE_QUIC_ARG: &str = "--disable-quic";
pub const DEFAULT_MAX_CONNECTIONS: usize = 256;
pub const MAX_CONNECTIONS: usize = 4096;
pub const DEFAULT_MAX_HEADER_BYTES: usize = 64 * 1024;
pub const MIN_HEADER_BYTES: usize = 8 * 1024;
pub const MAX_HEADER_BYTES: usize = 1024 * 1024;
pub const DEFAULT_MAX_HEADERS: usize = 128;
pub const MAX_HEADERS: usize = 1024;
pub const DEFAULT_MAX_BODY_BYTES: usize = 512 * 1024 * 1024;
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
pub const DEFAULT_WEBSOCKET_IDLE_TIMEOUT: Duration = Duration::from_secs(300);
pub const DEFAULT_MAX_WEBSOCKET_BYTES: u64 = 512 * 1024 * 1024;
```


- `PROXY_USER`: The user name the browser gives the proxy; the proxy's token is the password.
- `PRIME_HOST`: The host of the proxy's priming page ([`MitmProxy::prime_url`]). Under `.invalid` (RFC 6761), so it never names a real site.
- `PROXY_EVERYTHING_ARG`: The Chrome switch that removes the implicit bypass of loopback hosts, so `localhost` and `127.0.0.1` go through the proxy as well.
- `DISABLE_QUIC_ARG`: The Chrome switch that turns QUIC off, so every connection is TCP through the proxy.
- `DEFAULT_MAX_CONNECTIONS`: How many browser connections are served at once by default.
- `MAX_CONNECTIONS`: The most browser connections [`MitmOptions::max_connections`] accepts.
- `DEFAULT_MAX_HEADER_BYTES`: The largest request head (request line and headers) by default, in bytes.
- `MIN_HEADER_BYTES`: The smallest request-head limit accepted; the HTTP/1 parser needs this much buffer.
- `MAX_HEADER_BYTES`: The largest request-head limit accepted.
- `DEFAULT_MAX_HEADERS`: The most header fields in one request by default.
- `MAX_HEADERS`: The most header fields [`MitmOptions::max_headers`] accepts.
- `DEFAULT_MAX_BODY_BYTES`: The largest request or response body by default, in bytes. Bodies stream; this only cuts a body that grows past it.
- `DEFAULT_IDLE_TIMEOUT`: How long a browser connection may wait for its next request by default.
- `DEFAULT_CONNECT_TIMEOUT`: How long the upstream connection (TCP, egress tunnel and TLS) may take by default.
- `DEFAULT_READ_TIMEOUT`: How long one upstream read may take by default.
- `DEFAULT_WEBSOCKET_IDLE_TIMEOUT`: How long a relayed WebSocket may carry no byte in either direction by default.
- `DEFAULT_MAX_WEBSOCKET_BYTES`: The most bytes a relayed WebSocket carries in each direction by default.

## `MitmOptions`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct MitmOptions
```

Limits and custom inputs of a [`MitmProxy`].

### `MitmOptions` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn max_connections(mut self, count: usize) -> Self
pub fn max_header_bytes(mut self, bytes: usize) -> Self
pub fn max_headers(mut self, count: usize) -> Self
pub fn max_body_bytes(mut self, bytes: usize) -> Self
pub fn idle_timeout(mut self, timeout: Duration) -> Self
pub fn handshake_timeout(mut self, timeout: Duration) -> Self
pub fn connect_timeout(mut self, timeout: Duration) -> Self
pub fn read_timeout(mut self, timeout: Duration) -> Self
pub fn websocket_idle_timeout(mut self, timeout: Duration) -> Self
pub fn max_websocket_bytes(mut self, bytes: u64) -> Self
pub fn trust_upstream_root(mut self, der: impl Into<Vec<u8>>) -> Self
pub fn resolve(mut self, host: impl Into<String>, addr: SocketAddr) -> Self
pub fn allow_private_destinations(mut self, allow: bool) -> Self
pub fn ca(mut self, ca: Arc<EphemeralCa>) -> Self
```


- `max_connections`: Browser connections served at once, from 1 to [`MAX_CONNECTIONS`]. Clients past it wait in the listen queue until a connection ends.
- `max_header_bytes`: The largest request head in bytes, from [`MIN_HEADER_BYTES`] to [`MAX_HEADER_BYTES`]. A larger head is answered `431`.
- `max_headers`: The most header fields in one request, from 1 to [`MAX_HEADERS`].
- `max_body_bytes`: The largest body in either direction, in bytes, at least 1. A request that announces a larger body is answered `413`; a body that grows past it is cut and its connection closed.
- `idle_timeout`: How long a browser connection may wait for its next request (and for the head of its first one) before it is closed. Not zero.
- `handshake_timeout`: How long the browser's TLS handshake may take. Not zero.
- `connect_timeout`: How long the upstream connection (TCP, egress tunnel, TLS) may take. Not zero.
- `read_timeout`: How long one upstream read may take. Not zero.
- `websocket_idle_timeout`: How long a relayed WebSocket may carry no byte in either direction before both of its sides are closed. Not zero. Chrome sends no keep-alive of its own on a WebSocket, so a site that pings less often than this loses its idle connections.
- `max_websocket_bytes`: The most bytes a relayed WebSocket carries in each direction, at least 1. The read that crosses the limit is not passed on, and both sides are closed; frames are not read, so a single frame larger than the limit ends the connection.
- `trust_upstream_root`: Trusts a DER root for upstream verification. Once one is added, the upstream client trusts the added roots only, not the built-in web roots: use it for a private site. Verification is never turned off.
- `resolve`: Makes the upstream client resolve `host` to `addr`. The port of `addr` is ignored: the request's own port is used, as the browser asked for it.
- `allow_private_destinations`: Lets requests go to loopback, private, link-local, shared (carrier-grade NAT), multicast and reserved addresses, the cloud metadata addresses, and to `localhost`. Off by default.
- `ca`: Issues the leaves with `ca` instead of a CA made for this proxy alone. The CA's key stays in memory; sharing it shares the pin Chrome trusts.

## `MitmError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum MitmError {
    Bind(#[source] io::Error),
    Tls(#[from] TlsServerError),
    InvalidOptions(&'static str),
    Random,
    Upstream(#[from] ImpersonateError),
    Egress(&'static str),
    Unauthorized,
    BadTarget(&'static str),
    ForbiddenDestination(&'static str),
    NotIntercepted(&'static str),
    HostMismatch,
    MalformedUpgrade(&'static str),
    BodyTooLarge {
            /// The limit in force.
            limit: usize,
        },
    VersionMismatch {
            /// The major version the user agent claims.
            browser: u32,
            /// The major version the profile was measured as.
            measured: u32,
        },
    UpstreamRequest {
            /// The URL, redacted.
            url: RedactedUrl,
            /// What failed, in words; never the upstream error's own text.
            kind: &'static str,
        },
}
```

Why the proxy could not start, swap its upstream, or serve one request.

## `MitmStats`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct MitmStats {
    pub connections: u64,
    pub tunnels: u64,
    pub unauthorized: u64,
    pub refused: u64,
    pub forwarded: u64,
    pub upstream_errors: u64,
    pub websockets: u64,
    pub primed: u64,
}
```

Counts of what the proxy did since it started.

## `MitmProxy`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct MitmProxy
```

A loopback MITM proxy for one Chrome profile.

### `MitmProxy` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub async fn start(
        profile: ImpersonationProfile,
        egress: Option<EgressRef>,
        options: MitmOptions,
        sink: Arc<dyn EventSink>,
    ) -> Result<Self, MitmError>
pub fn prime_url(&self) -> String
pub fn port(&self) -> u16
pub fn local_addr(&self) -> SocketAddr
pub fn proxy_spec(&self) -> Secret
pub fn browser_proxy(&self) -> Proxy
pub fn chrome_args(&self) -> Vec<String>
pub fn configure_launch(&self, options: LaunchOptions) -> Result<LaunchOptions, SeleniumBaseError>
pub fn stats(&self) -> MitmStats
pub fn egress(&self) -> Option<EgressKey>
pub fn swap_upstream(&self, egress: Option<EgressRef>) -> Result<(), MitmError>
pub fn swap_profile(&self, profile: ImpersonationProfile) -> Result<(), MitmError>
pub async fn shutdown(self, grace: Duration)
```


- `start`: Binds `127.0.0.1` on a port the system picks and starts serving.
- `prime_url`: The address of the proxy's priming page: `https://` [`PRIME_HOST`] and a path of 128 random bits drawn when the proxy started.
- `port`: The loopback port the proxy listens on.
- `local_addr`: The loopback address the proxy listens on; its IP is always `127.0.0.1`.
- `proxy_spec`: `http://sb:&lt;token&gt;@127.0.0.1:&lt;port&gt;`, for a launch or a client that sets its own proxy. It holds the token: pass it on, never print it.
- `browser_proxy`: The proxy for a browser context (`ContextOptions::proxy`). Its `Debug` hides the token.
- `chrome_args`: The Chrome switches the proxy needs besides `--proxy-server`: the leaf pin (never the CA pin), [`PROXY_EVERYTHING_ARG`] and [`DISABLE_QUIC_ARG`].
- `configure_launch`: Points a Chrome launch at this proxy: the proxy with its token (answered over `Fetch.authRequired`, never put on the command line), [`MitmProxy::chrome_args`], and [`MitmProxy::prime_url`] as the start page, so that the browser holds the proxy's credentials before any WebSocket.
- `stats`: The counts so far.
- `egress`: The egress requests leave through now, `None` for a direct connection.
- `swap_upstream`: Sends the next requests through `egress` (or directly with `None`).
- `swap_profile`: Sends the next requests with `profile`'s fingerprint, through the same egress.
- `shutdown`: Stops accepting, lets requests under way finish for up to `grace`, then aborts what is left. Every clone stops with it.

## Related

The guide is [MITM proxy](/stealth/advanced/mitm-proxy). The overview is [mimic](/stealth/api/mimic).
