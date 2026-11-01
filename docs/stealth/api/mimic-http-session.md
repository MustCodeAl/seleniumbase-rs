---
title: "mimic::http_session"
sidebarTitle: "HttpSession"
description: "HttpSession: plain HTTP that continues a browser session with the measured TLS and HTTP/2."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: mimic</Badge>

`HttpSession` sends plain HTTP that looks like the measured Chrome on the wire and carries a browser's cookies. `from_page` is async and copies a `Page`. `from_identity` is synchronous. `get` and `request` send; `fetch` also judges the reply. At the crate root, `Attempt`, `Fetched`, `Rules`, `Verdict` and `Headers` are `HttpAttempt`, `HttpFetched`, `HttpRules`, `HttpVerdict` and `ReplyHeaders`.

The guide is [HttpSession](/stealth/http-session). `HttpOptions::new` uses a 30 second timeout, a 64 MiB body, and 10 redirects. `validate` refuses a timeout above one hour, a body limit above 1 GiB, or more than 30 redirects.

<RequestExample>

```rust Continue a page as plain HTTP
use seleniumbase_rs_stealth::json::Value;
use seleniumbase_rs_stealth::mimic::{HttpError, HttpOptions, HttpSession};
use seleniumbase_rs_stealth::sb_cdp::Page;

async fn orders(page: &Page) -> Result<(), HttpError> {
    let session = HttpSession::from_page(page, None, HttpOptions::new()).await?;
    let reply = session.get("https://example.com/api/orders").await?;
    let orders: Value = reply.json()?;
    println!("{orders}");
    session.sync_cookies_back(page).await?;
    Ok(())
}
```

</RequestExample>

## Constants and type aliases

```rust
// http_session/options.rs
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_TIMEOUT: Duration = Duration::from_secs(3600);
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_MAX_BODY_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_BODY_LIMIT: usize = 1024 * 1024 * 1024;
pub const DEFAULT_MAX_REQUEST_BYTES: usize = 64 * 1024 * 1024;
pub const DEFAULT_MAX_REDIRECTS: usize = 10;
pub const MAX_REDIRECT_LIMIT: usize = 30;
pub const MAX_PACED_SITES: usize = 4096;
```


- `Clock`: A source of the time in seconds since the Unix epoch.
- `DEFAULT_TIMEOUT`: How long one request may take in all, redirects included, unless told otherwise.
- `MAX_TIMEOUT`: The longest total time per request [`HttpOptions::validate`] accepts.
- `DEFAULT_CONNECT_TIMEOUT`: How long a connection may take to open, unless told otherwise.
- `DEFAULT_MAX_BODY_BYTES`: The largest response body read into memory unless told otherwise, after decompression.
- `MAX_BODY_LIMIT`: The largest response body [`HttpOptions::validate`] accepts as a limit.
- `DEFAULT_MAX_REQUEST_BYTES`: The largest request body sent unless told otherwise.
- `DEFAULT_MAX_REDIRECTS`: How many redirects are followed, as in a browser.
- `MAX_REDIRECT_LIMIT`: The most redirects [`HttpOptions::validate`] accepts as a limit.
- `MAX_PACED_SITES`: The most paced sites kept at once; past it the table starts over.

## `http_session`

### `HttpSession`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct HttpSession
```

Plain HTTP requests that continue a browser's session; see the [module](self) documentation.

### `HttpSession` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn from_identity(
        identity: &StealthIdentity,
        egress: Option<EgressRef>,
        options: HttpOptions,
    ) -> Result<Self, HttpError>
pub fn request(&self, method: Method, url: &str) -> HttpRequest<'_>
pub async fn get(&self, url: &str) -> Result<HttpReply, HttpError>
pub fn profile(&self) -> &ImpersonationProfile
pub fn egress(&self) -> Option<&EgressKey>
pub fn cookie_count(&self) -> usize
pub fn assess(
        &self,
        reply: &HttpReply,
        attempt: &Attempt,
        rules: &Rules,
        identity: &StealthIdentity,
    ) -> Verdict
pub async fn fetch(
        &self,
        url: &str,
        attempt: &Attempt,
        rules: &Rules,
        identity: &StealthIdentity,
    ) -> Result<Fetched, HttpError>
pub fn preflight(
        &self,
        url: &str,
        identity: &StealthIdentity,
        state: Option<&DomainState>,
        rules: &Rules,
    ) -> Result<Transition, HttpError>
pub fn export_identity(&self, base: &StealthIdentity) -> StealthIdentity
pub async fn from_page(
        page: &Page,
        egress: Option<EgressRef>,
        mut options: HttpOptions,
    ) -> Result<Self, HttpError>
pub async fn sync_cookies_back(&self, page: &Page) -> Result<usize, HttpError>
```


- `from_identity`: A session for an identity: its fingerprint becomes the user agent, client hints and languages of the requests, and its cookies fill the jar.
- `request`: Starts a request with an explicit method.
- `get`: Sends a `GET` request.
- `profile`: The profile the requests are made with.
- `egress`: The exit the requests leave through; `None` for a direct connection. It carries no credentials.
- `cookie_count`: How many cookies the jar holds.
- `assess`: Judges a reply: detects a challenge, asks the mitigation policy what to do, and asks the session policy whether that changes the mode. Emits `ChallengeDetected` and `ActionChosen` events.
- `fetch`: Fetches `url` with a `GET` and judges the reply; see [`assess`](Self::assess).
- `preflight`: Asks, before a request to `url`, whether the session should go to the browser first because the identity's clearance for the site is about to end, is missing and required, or the site's history says plain requests do not work.
- `export_identity`: The identity `base` with the cookies of this session in it, and the clearances found in them. Emits `IdentityExported`.
- `from_page`: Continues the session of `page` as plain HTTP.
- `sync_cookies_back`: Writes into `page`'s browser the cookies that responses set or removed since the last sync, so the session carries on there. Returns how many cookies were written or removed.

## `http_session::error`

### `HttpError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum HttpError {
    Impersonate(ImpersonateError),
    VersionMismatch {
            /// The major version the browser or identity claims.
            browser: u32,
            /// The major version that was measured.
            measured: u32,
        },
    Identity(#[from] IdentityError),
    BrowserProxy {
            /// The proxy of the browser, as its command line names it (no credentials).
            proxy: String,
        },
    ReservedHeader(String),
    InvalidHeader(String),
    ForbiddenMethod(String),
    Url {
            /// What is wrong with it.
            reason: &'static str,
        },
    TooManyRedirects(usize),
    RedirectRefused {
            /// The target, redacted.
            url: RedactedUrl,
            /// Why it was refused.
            reason: &'static str,
        },
    BodyTooLarge(usize),
    RequestTooLarge(usize),
    Request {
            /// The URL requested, redacted.
            url: RedactedUrl,
            /// What kind of failure, in a word or two.
            kind: &'static str,
        },
    Client {
            /// Which part was refused.
            kind: &'static str,
        },
    Options(&'static str),
    NotUtf8,
    Json {
            /// Whether the text is not JSON, ends too early, or has another layout.
            kind: &'static str,
        },
    Browser(#[from] SeleniumBaseError),
}
```

Why an [`HttpSession`](super::HttpSession) could not be made or a request failed.

## `http_session::options`

### `HttpOptions`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct HttpOptions
```

The limits and plumbing of an HTTP session.

### `HttpOptions` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new() -> Self
pub fn timeout(mut self, timeout: Duration) -> Self
pub fn connect_timeout(mut self, timeout: Duration) -> Self
pub fn max_body_bytes(mut self, limit: usize) -> Self
pub fn max_request_bytes(mut self, limit: usize) -> Self
pub fn max_redirects(mut self, limit: usize) -> Self
pub fn follow_redirects(mut self, follow: bool) -> Self
pub fn pacing(mut self, calls: usize, window: Duration) -> Self
pub fn event_sink(mut self, sink: Arc<dyn EventSink>) -> Self
pub fn trust(mut self, store: CertStore) -> Self
pub fn resolve(mut self, host: impl Into<String>, addr: SocketAddr) -> Self
pub fn bypass(mut self, host: impl Into<String>) -> Self
pub fn browser_proxy_exits_directly(mut self, direct: bool) -> Self
pub fn installed_chrome_major(mut self, major: u32) -> Self
pub fn clock(mut self, clock: Clock) -> Self
pub fn validate(&self) -> Result<(), HttpError>
```


- `new`: The defaults: [`DEFAULT_TIMEOUT`], [`DEFAULT_MAX_BODY_BYTES`], [`DEFAULT_MAX_REDIRECTS`] redirects followed, no pacing, no events, the system trust roots.
- `timeout`: How long one request may take in all, redirects included.
- `connect_timeout`: How long a connection may take to open. A value above the timeout of the request is cut to it.
- `max_body_bytes`: The largest response body read into memory, counted after decompression. A longer one fails the request with [`HttpError::BodyTooLarge`].
- `max_request_bytes`: The largest request body a request may carry.
- `max_redirects`: How many redirects one request follows before it fails with [`HttpError::TooManyRedirects`].
- `follow_redirects`: Whether redirects are followed. Off, a `3xx` answer is the reply (its `Set-Cookie` is still stored).
- `pacing`: At most `calls` requests to one site (a registrable domain) in `window`, every redirect hop counted. The limit is per site, not global.
- `event_sink`: Where the events of the session go.
- `trust`: The certificate roots the server must chain to, in place of the system roots, as a `wreq::tls::trust::CertStore`. Verification stays on: this is how a private root is trusted.
- `resolve`: Resolves `host` to `addr` without asking DNS, as `curl --resolve` does. The TLS server name stays `host`.
- `bypass`: A host that skips the proxy, in the notation of a `no_proxy` list (`example.com`, `.example.com`). Only the egress of a session uses it.
- `browser_proxy_exits_directly`: Says that the browser-wide proxy of the page's browser (a MITM proxy without an egress, say) leaves directly, so that [`HttpSession::from_page`](super::HttpSession::from_page) may build a session without an egress for it. Off by default: a page does not show a browser-wide proxy, and a session that silently went direct would send the page's cookies from this machine's own address, so `from_page` refuses with [`HttpError::BrowserProxy`] unless it is given the egress the proxy really leaves through.
- `installed_chrome_major`: The major version of the Chrome installed on this machine. A session refuses to start when it differs from the measured one: the measurement is stale.
- `clock`: The source of the time. The default reads the system clock.
- `validate`: Checks every limit against its documented range.

## `http_session::outcome`

### `Attempt`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Attempt {
    pub number: u32,
    pub previous_challenges: u32,
    pub can_rotate_egress: bool,
    pub can_escalate: bool,
}
```

Where a fetch stands, as the mitigation policy needs to know it.

### `Rules`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Rules {
    pub mitigation: MitigationPolicy,
    pub session: SessionPolicy,
}
```

The two policies that judge a reply.

### `Verdict`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Verdict {
    pub detection: Detection,
    pub action: Action,
    pub transition: Transition,
}
```

What a reply means for the session.

### `Verdict` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn is_clear(&self) -> bool
```


- `is_clear`: Whether the reply is a real answer: it is not a challenge, a block or a rate limit.

### `Fetched`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Fetched {
    pub reply: HttpReply,
    pub verdict: Verdict,
}
```

A reply and the verdict on it.

## `http_session::reply`

### `Headers`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Headers(Vec<(String, String)>);
```

The headers of a response, in the order received, with lower-case names.

### `Headers` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn get(&self, name: &str) -> Option<&str>
pub fn get_all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str>
pub fn iter(&self) -> impl Iterator<Item = (&str, &str)>
pub fn as_pairs(&self) -> &[(String, String)]
pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
```


- `get`: The first value of `name`, matched without regard to case.
- `get_all`: Every value of `name`, matched without regard to case.
- `iter`: Every header, in the order received.
- `as_pairs`: The pairs, for code that reads them as a slice.
- `len`: How many headers there are.
- `is_empty`: Whether there are no headers.

### `ReplyProtocol`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum ReplyProtocol {
    Http2,
    Http1,
}
```

The protocol a reply travelled on.

### `HttpReply`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct HttpReply
```

One answer, with its whole body.

### `HttpReply` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn status(&self) -> u16
pub fn is_success(&self) -> bool
pub fn redacted_url(&self) -> RedactedUrl
pub fn url(&self) -> &Url
pub fn headers(&self) -> &Headers
pub fn bytes(&self) -> &[u8]
pub fn text(&self) -> Result<&str, HttpError>
pub fn json<T: DeserializeOwned>(&self) -> Result<T, HttpError>
pub fn protocol(&self) -> ReplyProtocol
pub fn redirects(&self) -> &[RedactedUrl]
pub fn elapsed(&self) -> Duration
pub fn detect(&self) -> Detection
```


- `status`: The status code.
- `is_success`: Whether the status is 2xx.
- `redacted_url`: The final URL, after any redirects, with credentials and secrets in the query removed.
- `url`: The final URL, after any redirects, as it was requested.
- `headers`: The response headers.
- `bytes`: The body, decompressed.
- `text`: The body as text.
- `json`: The body parsed as JSON.
- `protocol`: The protocol the reply travelled on.
- `redirects`: The URLs the request was redirected through before this reply, redacted, in order. Empty when it was answered at once.
- `elapsed`: How long the request took, redirects included.
- `detect`: Whether the reply is a bot-protection challenge, with the evidence.

## `http_session::request`

### `HttpRequest`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct HttpRequest<'a>
```

A request being built; send it with [`send`](Self::send).

### `HttpRequest` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn shape(mut self, shape: RequestShape) -> Self
pub fn header(mut self, name: &str, value: &str) -> Self
pub fn body(mut self, body: impl Into<Bytes>) -> Self
pub fn json<T: Serialize + ?Sized>(mut self, value: &T) -> Self
pub fn form(mut self, fields: &[(&str, &str)]) -> Self
pub fn timeout(mut self, timeout: std::time::Duration) -> Self
pub async fn send(self) -> Result<HttpReply, HttpError>
```


- `shape`: Chooses the headers of a navigation ([`RequestShape::Document`], the default for `GET` and `HEAD`) or of a request a script makes ([`RequestShape::Fetch`], the default for the other methods).
- `header`: Sets a header, replacing the profile's value when it has one.
- `body`: Sets the body as given. No `Content-Type` is set.
- `json`: Sets a JSON body and `Content-Type: application/json`.
- `form`: Sets a form body and `Content-Type: application/x-www-form-urlencoded`.
- `timeout`: Limits this request, redirects included, to `timeout`, in place of the session's.
- `send`: Sends the request, following redirects by hand, and reads the whole reply.

## Related

The guide is [HttpSession](/stealth/http-session). The overview is [mimic](/stealth/api/mimic).
