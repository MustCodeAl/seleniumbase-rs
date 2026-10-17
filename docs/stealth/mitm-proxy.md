---
title: "TLS Server and MitmProxy"
---

# TLS Server and MitmProxy

## The ephemeral-CA TLS server

`mimic::tls_server` is the building block under `capture_h2` and under
[`MitmProxy`](#mitmproxy). `EphemeralCa::generate()` creates a P-256
certificate authority in memory for the life of one process. `leaf_for(host,
now)` issues a leaf for one DNS name or IP address (never a wildcard), cached in
a count-bounded LRU. `acceptor(&[ALPN_HTTP1])` builds the BoringSSL server
context (TLS 1.2 and 1.3, no session resumption), and `accept` runs one
handshake and refuses the connection when the client's SNI names another host.

```rust
use std::time::SystemTime;
use seleniumbase_rs_mimic::tls_server::{
    accept, acceptor, EphemeralCa, ALPN_HTTP1, DEFAULT_HANDSHAKE_TIMEOUT,
};

# async fn demo(tcp: tokio::net::TcpStream) -> Result<(), Box<dyn std::error::Error>> {
let ca = EphemeralCa::generate()?;
let chrome_flag = ca.chrome_spki_arg(); // pass to Chrome together with --user-data-dir
let server = acceptor(&[ALPN_HTTP1])?;
let host = url::Host::parse("example.com")?;
let leaf = ca.leaf_for(&host, SystemTime::now())?;
let tls = accept(&server, &leaf, tcp, DEFAULT_HANDSHAKE_TIMEOUT).await?;
# drop((chrome_flag, tls));
# Ok(())
# }
```

Things to know before you use it:

- It is for a local browser talking to something on loopback. Nothing in the
  module binds a socket; the caller binds `127.0.0.1` only. The CA is never
  installed in a trust store, never written to disk, and `--ignore-certificate-errors`
  is never used.
- Chrome trusts the leaves through `--ignore-certificate-errors-spki-list` with
  the pin of the shared leaf key. **That switch trusts every certificate chain that
  contains the pinned public key, from any sender, and runs no other check.** A
  public key is not secret, so the pin is safe only while every connection of that
  Chrome profile ends at your server. The module keeps the trusted set as small
  as the switch allows (only the leaf key is pinned, and the server presents the
  leaf alone). That a chain carrying the pinned key is accepted is **measured on
  Chrome 155** (headless, macOS). In `tests/impersonate_mitm_chrome.rs`,
  `without_the_proxy_the_leaf_pin_alone_trusts_a_chain_carrying_the_leaf_key`
  starts Chrome with the pin and no proxy, and Chrome loads a page from a
  loopback server whose chain is its own self-signed certificate followed by the
  proxy's leaf. Behind the proxy the same server is never served to the page
  (`a_loopback_server_carrying_the_pinned_key_is_never_served_to_the_page`).
  Both tests are `#[ignore]`d because they start Chrome. They do not separate the
  individual checks the switch skips (signature, host name, validity,
  revocation); those stay as read in the Chromium source.
- The CA lives seven days by default (at most 90), a leaf one day. Private keys
  stay in BoringSSL memory and never appear in `Debug`, logs or errors. They are
  freed, not wiped, when the CA drops, because BoringSSL offers no zeroize hook.

## MitmProxy

`HttpSession` covers the requests you make yourself. `mimic::mitm::MitmProxy`
covers the ones a real Chrome makes. It is a loopback forward proxy that ends
the browser's TLS with a leaf of the [ephemeral CA](#the-ephemeral-ca-tls-server)
and replays each request through the impersonating client, so the origin sees the
`ClientHello`, HTTP/2 start and header spelling of the profile (the measured
ones, minus the [known gaps](limits.md#limits)) instead of whatever the installed browser
would have sent. Both legs then look alike on the wire, and a clearance earned in
the browser is valid over `HttpSession`.

```rust
use std::sync::Arc;
use std::time::Duration;

use seleniumbase_rs_mimic::{ImpersonationProfile, MitmOptions, MitmProxy};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_scrape::NoopEventSink;
use seleniumbase_rs_codec::BrowserKind;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let profile = ImpersonationProfile::measured(BrowserKind::Chrome)?;
let mitm =
    MitmProxy::start(profile, None, MitmOptions::default(), Arc::new(NoopEventSink)).await?;
let options = mitm.configure_launch(LaunchOptions::builder()).build()?;
let browser = Browser::launch(options).await?;
let page = browser.new_page(Some("https://example.com/")).await?;
# drop(page);
browser.close().await?;
mitm.shutdown(Duration::from_secs(5)).await;
# Ok(())
# }
```

`MitmProxy::start(profile, egress, options, sink)` binds `127.0.0.1` on a port the
system picks. `egress` is the proxy the replayed requests leave through (or
`None`); `sink` receives redacted events. `configure_launch` points a Chrome
launch at the proxy and adds the switches it needs.

### What the proxy does with each connection

1. It requires `Proxy-Authorization: Basic` with a random 128-bit token (user
   name `sb`), compared in constant time, and answers `407` otherwise. Chrome
   gets the token through `ProxyAuth` over `Fetch.authRequired`, so it is never
   on Chrome's command line.
2. It refuses a target it cannot intercept with an empty `502` and closes the
   connection. Nothing is ever tunnelled blind: a tunnel whose first bytes are
   neither a TLS handshake for the `CONNECT` host nor a plain-text WebSocket
   handshake (`ws://`, see [WebSocket](#websocket)) is closed.
3. It answers `200` and ends the browser's TLS with a leaf for exactly the
   `CONNECT` host. The browser leg offers ALPN `http/1.1` only, and the SNI must
   name the same host.
4. It reads each request, drops the hop-by-hop and proxy headers, keeps Chrome's
   header order, adds the HTTP/2-only `priority` header from the measured table,
   and sends the request upstream with certificate verification on. The reply
   streams back without being buffered whole. An upstream failure, an untrusted
   certificate included, is an empty `502`.

An absolute-form `http://` request is checked the same way and forwarded without
`CONNECT`. A WebSocket handshake inside a tunnel is relayed (see
[WebSocket](#websocket) below). The browser only ever sees an empty status (`407`,
`400`, `403`, `404`, `413`, `431` or `502`; the `404` only on the proxy's
[priming host](#the-priming-page)); the reason is a `MitmError` variant in the log
(`Unauthorized`, `BadTarget`, `ForbiddenDestination`, `NotIntercepted`,
`HostMismatch`, `MalformedUpgrade`, `BodyTooLarge`, `VersionMismatch`,
`UpstreamRequest`). `start` itself can fail with `Bind`, `Tls`, `InvalidOptions`,
`Random`, `Upstream` or `Egress`.

### What Chrome must be told

`mitm.configure_launch(builder)` sets the proxy, makes the proxy's priming page
the start page (see [the priming page](#the-priming-page)) and adds
`mitm.chrome_args()`: the `--ignore-certificate-errors-spki-list` switch with the
leaf pin only (never
the CA pin), `--proxy-bypass-list=<-loopback>` so loopback hosts go through the
proxy too, and `--disable-quic`. Because that switch trusts any chain that
carries the pinned key (see [the CA section](#the-ephemeral-ca-tls-server)), the
pin is safe only while every connection of the profile ends at the proxy.
Without the last two switches a loopback server or a QUIC connection would reach
Chrome directly. Chrome honours the pin only together with `--user-data-dir`, so
do not reuse that profile directory for a Chrome that runs without the proxy. The
proxy refuses a request whose `User-Agent` claims a Chrome major other than the
profile's.

### Bounds and options

`MitmOptions` sets the bounds; `MitmProxy::start` refuses a value outside them
with `MitmError::InvalidOptions`.

| Setting | Default | Range |
|---|---|---|
| `max_connections`, served at once (further clients wait in the listen queue) | 256 | 1 to 4096 |
| `max_header_bytes`, the request head (a larger one is answered `431`) | 64 KiB | 8 KiB to 1 MiB |
| `max_headers` | 128 | 1 to 1024 |
| `max_body_bytes`, each body in either direction (`413` when announced larger; cut when it grows past it) | 512 MiB | at least 1 |
| `idle_timeout`, a connection waiting for its next request | 60 s | not zero |
| `handshake_timeout`, the browser's TLS handshake | 10 s | not zero |
| `connect_timeout`, the upstream connection | 30 s | not zero |
| `read_timeout`, one upstream read | 60 s | not zero |
| `websocket_idle_timeout`, a relayed WebSocket carrying no byte either way | 300 s | not zero |
| `max_websocket_bytes`, each direction of a relayed WebSocket | 512 MiB | at least 1 |

`trust_upstream_root(der)` adds a root for upstream verification; once one is
added, only the added roots are trusted, not the built-in web roots, and
verification is never turned off. `resolve(host, addr)` pins a host name for the
upstream client, and `ca(Arc<EphemeralCa>)` shares a CA.

Bodies stream both ways; nothing is read whole into memory. `mitm.stats()` gives
counts (`connections`, `tunnels`, `unauthorized`, `refused`, `forwarded`,
`upstream_errors`, `websockets`, `primed`).

### WebSocket

A page's `wss://` WebSocket reaches the proxy as a `CONNECT` whose TLS the proxy
ends, followed by the opening handshake (`GET` with `Upgrade: websocket` and
`Connection: Upgrade`, RFC 6455). The proxy:

1. checks the handshake like any request (the token, the destination, `Host`,
   the Chrome major), and that it is a whole handshake: a bodiless HTTP/1.1 `GET`
   with both `Upgrade: websocket` and `Connection: upgrade`. Half of one
   (`Connection: upgrade` without `Upgrade`, or the other way round), another
   method, HTTP/1.0 or a body is an empty `400`; an upgrade to anything else
   (`h2c`, `TLS/1.2`, a list) is an empty `502`;
2. sends it upstream over HTTP/1.1 with the measured WebSocket `ClientHello`
   (`tls_websocket` in the measured file, minus the known gaps). Chrome offers
   only `http/1.1` on a WebSocket connection, so this hello's JA4 is not the
   ordinary one. The egress, the destination filter, the trusted roots and the
   timeouts are those of ordinary requests;
3. keeps the browser's header order without the hop-by-hop and proxy fields, and
   writes the `Connection: Upgrade` and `Upgrade: websocket` pair itself. Header
   names take the measured spelling, and the handshake fields the measured lists
   do not hold take Chrome's (`Sec-WebSocket-Key`, `Sec-WebSocket-Version`,
   `Sec-WebSocket-Extensions`, `Origin`, ...);
4. relays the upstream's `101 Switching Protocols` with its own headers.
   `Sec-WebSocket-Accept` is checked by the browser, as it would be without a
   proxy. A `101` that switches to another protocol is an empty `502`, and any
   other answer (a `403`, say) reaches the page as an ordinary answer;
5. then copies bytes both ways without reading frames, until a side closes, no
   byte has passed for `websocket_idle_timeout`, one direction has carried more
   than `max_websocket_bytes` (the read that crosses the limit is not passed on,
   so one frame larger than the limit ends the connection), or the proxy stops.
   Both sides are then shut down.

Chrome sends no keep-alive of its own on a WebSocket, so a site that pings less
often than `websocket_idle_timeout` loses its idle connections. A custom
profile's WebSocket connections send the measured WebSocket hello of its base
browser.

`ws://` takes the same road without TLS. Measured on Chrome 155: a page's
`ws://` WebSocket reaches an HTTP proxy as a `CONNECT` (to port 80 when the URL
names none) whose tunnel then carries the opening handshake in plain text, with
the fields of a `wss://` one in the same order. The proxy reads the tunnel's
first bytes. A TLS handshake is ended as above. `GET ` is read as HTTP/1.1, and
a WebSocket handshake in it takes steps 1 and 3 to 5 like a `wss://` one, with
`Host` checked against the `CONNECT` target; it goes upstream as `http://` over
a plain connection, so there is no `ClientHello` to impersonate. Any other
plain-text request gets an empty `502` (`400` for half an upgrade) and the
tunnel is closed, and bytes that start neither way close it unanswered: plain
text is relayed only after an upgrade the site accepted, as for `wss://`. A
handshake outside a tunnel (an absolute-form `http://` request, which Chrome does
not send for `ws://`) is an empty `502`. Through an egress, the plain handshake
leaves the way any `http://` request does, so an HTTP egress has to relay the
upgrade itself; that is not tested.

#### The priming page

Chrome gets the proxy's token from `ProxyAuth`, which answers `Fetch.authRequired`
for a tab's requests. A WebSocket's own `CONNECT` never gets there. Measured on
Chrome 155 (headless): a `wss://` WebSocket opened before any other request had
gone through the proxy got the `407` on its `CONNECT`, Chrome did not retry it
with the token, and the page saw an error; a `Fetch.enable` sent to the browser
target saw nothing of it either. Credentials Chrome was given are kept
in a cache that every tab of the browser context shares, and every later
`CONNECT` carries them, a WebSocket's included: one tab's navigation, then a
WebSocket as another tab's first request, goes through.

So the proxy owns one page, `mitm.prime_url()`: `https://sb-mitm.invalid/`
(`mimic::mitm::PRIME_HOST`, under the reserved `.invalid` name) and a path
of 128 random bits. After the token check, a `GET` of exactly that is answered by
the proxy itself with an empty page that is never cached (`mitm.stats().primed`
counts it). Any other request for that host, Chrome's `favicon.ico` included,
gets an empty `404`. Nothing for that host goes upstream and the event sink hears
nothing of it. The path holds no token; it is random so that a page that was not
given it only ever sees the `404`. `configure_launch` makes the priming page the
start page, so `Browser::launch` loads it in the first tab before it returns, and
a WebSocket may then be a new tab's first request. A start page you set before
`configure_launch` is kept (an `http(s)://` one takes the credentials just as
well; an `about:` or `data:` one does not), and one set after it replaces the
priming page.

A browser context keeps a cache of its own, and one created after the launch
does not share the default context's (measured). Before a WebSocket in a context
behind the proxy, load the priming page in its first tab:

```rust
use seleniumbase_rs_mimic::MitmProxy;
use seleniumbase_rs::sb_cdp::{Browser, ContextOptions};

# async fn demo(browser: Browser, mitm: MitmProxy) -> Result<(), Box<dyn std::error::Error>> {
let context = browser
    .new_context_with(ContextOptions::new().proxy(mitm.browser_proxy()))
    .await?;
let page = context.new_page(Some(mitm.prime_url())).await?;
// WebSockets of this context now carry the proxy's credentials.
# drop(page);
# Ok(())
# }
```

### Changing the upstream, and stopping

`swap_upstream(egress)` and `swap_profile(profile)` build the new upstream
client first and then replace the old one. Requests under way finish on the old
one, and the listener, its port and the browser's open connections stay.
`shutdown(grace)` stops accepting, lets requests finish for up to `grace`, then
aborts what is left; dropping the last clone stops it too.

### What is not intercepted

- **Upgrades other than WebSocket are refused**, and so is plain text in a
  tunnel that is not a WebSocket handshake (see [WebSocket](#websocket)).
- QUIC is off in Chrome, so there is no HTTP/3.
- **The first request to an HTTP/1.1-only origin carries an HTTP/2 `priority`
  header.** The upstream protocol is settled by ALPN after the request was
  prepared, and the first request to an origin is prepared for HTTP/2. The proxy
  then remembers the origin and later requests do not carry it. On HTTP/1.1 the
  header names take the measured spelling; a name outside the measured lists
  keeps the lower case the browser leg delivers.

### What was run

`tests/impersonate_mitm.rs` runs the proxy against loopback sites, an egress
proxy and raw clients, with no browser: the token and the `407`, the priming
page (answered behind the token, a `404` for anything else on its host, nothing
upstream), targets it refuses, the SNI check, the bounds, the measured JA4 and Akamai string and
`priority` at the origin, header order, the refusals above, a 64 MiB body
streaming through, swapping the egress, and shutdown leaving no task. Its
`websocket` tests run a WebSocket echo site written in the test (raw tokio, no
WebSocket library on the site's side): the echo, a handshake completed by
`tokio-tungstenite` as an independent client, the measured WebSocket JA4 at the
site, the handshake's header order, a WebSocket through an egress proxy with its
credentials, and the hostile inputs (half upgrades, other
protocols, a body, an oversize frame, an idle peer, a name resolving to loopback
without `allow_private_destinations`, a handshake outside a tunnel, and a proxy
that stops under an open WebSocket). For `ws://` they run a plain-text echo site:
the handshake in a tunnel without TLS is relayed in the browser's order and
echoes, and a plain `GET`, a `POST`, an `h2c` upgrade, half an upgrade, a `Host`
other than the target and a loopback name without the switch are refused without
reaching the site.

`tests/impersonate_mitm_chrome.rs` drives the real Chrome 155 on macOS, headless,
and is `#[ignore]`d. A page loads through the proxy, and the loopback origin
sees the measured JA4 and Akamai string with the measured `priority` and a
Chrome user agent; the token reaches Chrome only through the `407`. A page
opens a `wss://` WebSocket through the proxy to a loopback echo site and gets its
echo back; the site sees the measured WebSocket JA4 and Chrome's handshake fields
in Chrome's order. A `ws://` WebSocket reaches a plain loopback echo site through
the proxy and gets its echo, without a TLS tunnel. A WebSocket that is a new
tab's first request gets its echo, because the launch loaded the
priming page; a context created after the launch fails its first WebSocket on the
`407` until one of its tabs has loaded the priming page, and then gets the echo.
Two more
tests are about the pin: a loopback server carrying the pinned key is never
served to the page behind the proxy, and the same server is served to a Chrome
that has the pin and no proxy. The origin in these tests is a server of the test,
not a live site. Only the first test compares the browser with the measured
persona, so only it skips on a system that is not measured. The others are about
the proxy's own rules and run on any Chrome or Chromium; one whose major is not
the measured one is started with the profile's user agent, because the proxy
refuses a request that claims another Chrome major. Run that way on linux/arm64
with Debian's Chromium 154 (`docker/Dockerfile.verify`, headless, no sandbox),
the seven ran and passed and the persona test skipped; before the split every
test of the file skipped there.
