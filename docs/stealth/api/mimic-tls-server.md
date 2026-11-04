---
title: "mimic::tls_server"
sidebarTitle: "TLS server"
description: "EphemeralCa: an in-memory P-256 CA, one leaf per host, never written to disk."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: mimic</Badge>

`tls_server` is the certificate authority `MitmProxy` uses. `EphemeralCa` is an in-memory P-256 CA. It issues one leaf per host, pinned by SPKI, and never writes a key to disk.

How Chrome is told to trust the leaf is on [MITM proxy](/stealth/advanced/mitm-proxy).

<RequestExample>

```rust An in-memory CA
use seleniumbase_rs_stealth::mimic::{EphemeralCa, TlsServerError};

fn main() -> Result<(), TlsServerError> {
    let ca = EphemeralCa::generate()?;
    println!("{}", ca.chrome_spki_arg());
    Ok(())
}
```

</RequestExample>

## Constants and type aliases

```rust
// tls_server/mod.rs
pub const ALPN_HTTP1: &[u8] = b"http/1.1";
pub const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_LEAF_CAPACITY: usize = 256;
pub const MAX_LEAF_CAPACITY: usize = 4096;
pub const DEFAULT_CA_VALIDITY: Duration = Duration::from_secs(7 * SECONDS_PER_DAY);
pub const MAX_CA_VALIDITY: Duration = Duration::from_secs(90 * SECONDS_PER_DAY);
pub const DEFAULT_LEAF_VALIDITY: Duration = Duration::from_secs(SECONDS_PER_DAY);
pub const DEFAULT_LEAF_RENEW_MARGIN: Duration = Duration::from_secs(SECONDS_PER_HOUR);
pub const CLOCK_SKEW_ALLOWANCE: Duration = Duration::from_secs(SECONDS_PER_HOUR);
```


- `ALPN_HTTP1`: The ALPN id of HTTP/1.1, the only protocol the browser-to-proxy leg negotiates.
- `DEFAULT_HANDSHAKE_TIMEOUT`: How long [`accept`] waits for a client to finish its handshake, unless told otherwise.
- `DEFAULT_LEAF_CAPACITY`: How many leaves the cache holds by default before it evicts the least recently used one.
- `MAX_LEAF_CAPACITY`: The largest leaf cache [`CaOptions`] accept.
- `DEFAULT_CA_VALIDITY`: How long the CA certificate is valid by default, counted from its creation.
- `MAX_CA_VALIDITY`: The longest CA validity [`CaOptions`] accept: the CA is per process, so it stays short.
- `DEFAULT_LEAF_VALIDITY`: How long a leaf is valid by default, counted from its issue and capped at the CA's expiry.
- `DEFAULT_LEAF_RENEW_MARGIN`: A cached leaf is issued again once it is this close to expiry, by default.
- `CLOCK_SKEW_ALLOWANCE`: Every certificate starts this long before its issue, so a slightly slow client clock accepts it.

## `tls_server::ca`

### `EphemeralCa`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct EphemeralCa
```

A certificate authority that lives in memory for one process, for loopback TLS only.

### `EphemeralCa` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn generate() -> Result<Self, TlsServerError>
pub fn generate_at(options: CaOptions, now: SystemTime) -> Result<Self, TlsServerError>
pub fn spki_pins(&self) -> [String; 2]
pub fn chrome_spki_arg(&self) -> String
pub fn leaf_spki_pin(&self) -> &str
pub fn certificate(&self) -> &X509Ref
pub fn options(&self) -> CaOptions
pub fn not_after_unix(&self) -> i64
pub fn cached_leaves(&self) -> usize
pub fn leaf_for(
        &self,
        host: &Host<String>,
        now: SystemTime,
    ) -> Result<Arc<Leaf>, TlsServerError>
```


- `generate`: Creates a CA with [`CaOptions::default`], valid from now.
- `generate_at`: Creates a CA with `options`, valid from `now` (backdated by [`CLOCK_SKEW_ALLOWANCE`]).
- `spki_pins`: The pins of the two public keys: base64 of the SHA-256 of the `SubjectPublicKeyInfo` of the CA key and of the shared leaf key, in that order.
- `chrome_spki_arg`: The Chrome switch for this CA's leaves: `--ignore-certificate-errors-spki-list=&lt;leaf pin&gt;`. Chrome honours it only together with `--user-data-dir`.
- `leaf_spki_pin`: The pin of the shared leaf key: base64 of the SHA-256 of its `SubjectPublicKeyInfo`.
- `certificate`: The self-signed CA certificate.
- `options`: The lifetimes and cache bound this CA was created with.
- `not_after_unix`: The end of the CA's validity, in seconds since the Unix epoch.
- `cached_leaves`: How many leaves the cache holds now.
- `leaf_for`: The leaf for `host` at `now`: the cached one while it is valid for longer than [`CaOptions::leaf_renew_margin`], otherwise a newly issued one.

### `Leaf`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Leaf
```

One leaf certificate for one CONNECT host, signed by an [`EphemeralCa`].

### `Leaf` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn certificate(&self) -> &X509Ref
pub fn issuer(&self) -> &X509Ref
pub fn host(&self) -> String
pub fn not_before_unix(&self) -> i64
pub fn not_after_unix(&self) -> i64
```


- `certificate`: The leaf certificate.
- `issuer`: The CA certificate that signed it. The server does not send it: the chain it presents holds the leaf alone, so a client must trust the CA by other means.
- `host`: The host the SAN names: a lowercase DNS name or an IP address.
- `not_before_unix`: The start of the leaf's validity, in seconds since the Unix epoch.
- `not_after_unix`: The end of the leaf's validity, in seconds since the Unix epoch.

## `tls_server`

### `CaOptions`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct CaOptions {
    pub ca_validity: Duration,
    pub leaf_validity: Duration,
    pub leaf_renew_margin: Duration,
    pub leaf_capacity: usize,
}
```

Lifetimes and the cache bound of an [`EphemeralCa`].

### `TlsServerError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum TlsServerError {
    Ssl(#[from] ErrorStack),
    Handshake(#[source] btls::ssl::Error),
    HandshakeTimeout {
            /// The time the client had.
            limit: Duration,
            /// The timer that ran out.
            #[source]
            source: tokio::time::error::Elapsed,
        },
    InvalidHost(&'static str),
    InvalidAlpn(&'static str),
    InvalidOptions(&'static str),
    ClockOutOfRange,
    CaExpired,
    SniMismatch {
            /// The CONNECT host the leaf was issued for.
            expected: String,
            /// The SNI the client sent, cut to the longest DNS name.
            sni: Option<String>,
        },
}
```

Everything that can go wrong creating the CA, issuing a leaf or serving a handshake.

## Functions

### `spki_pin_of`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn spki_pin_of(cert: &X509Ref) -> Result<String, TlsServerError>
```

The Chrome SPKI pin of a certificate: base64 of the SHA-256 of its `SubjectPublicKeyInfo`.

### `acceptor`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn acceptor(alpn: &[&[u8]]) -> Result<SslAcceptor, TlsServerError>
```

Builds the server context for intercepted connections.

### `accept`

<Badge color="green" shape="pill">function</Badge>

```rust
pub async fn accept<S>(
    acceptor: &SslAcceptor,
    leaf: &Leaf,
    stream: S,
    handshake_timeout: Duration,
) -> Result<SslStream<S>, TlsServerError>
where
    S: AsyncRead + AsyncWrite + Unpin,
```

Runs the server side of one TLS handshake with `leaf` and checks the client's SNI.

## Related

The guide is [MITM proxy](/stealth/advanced/mitm-proxy). The overview is [mimic](/stealth/api/mimic).
