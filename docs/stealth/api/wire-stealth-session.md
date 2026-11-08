---
title: "wire::stealth_session"
sidebarTitle: "StealthSession"
description: "StealthSession: one identity in plain HTTP or in a real Chrome."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: wire</Badge>

`StealthSession` keeps one `Identity` and moves it between `HttpSession` and a real Chrome behind `MitmProxy`. A challenge escalates to the browser. A clearance demotes back to plain HTTP.

The guide is [StealthSession](/scraping/advanced-stealthy/stealth-session).

<RequestExample>

```rust Start the builder
use seleniumbase_rs_stealth::wire::StealthSession;

fn main() {
    let _builder = StealthSession::builder();
}
```

</RequestExample>

## Constants and type aliases

```rust
// stealth_session/builder.rs
pub const DEFAULT_ATTEMPTS: u32 = DEFAULT_MAX_CHALLENGE_ATTEMPTS + 1;
pub const MITM_SHUTDOWN_GRACE: Duration = Duration::from_secs(5);
```


- `DEFAULT_ATTEMPTS`: How many loads of one URL a session fetch makes before it gives up, unless told otherwise.
- `MITM_SHUTDOWN_GRACE`: How long the MITM proxy is given to finish its requests when the session is closed.

## `stealth_session`

### `StealthSession`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct StealthSession
```

A scraping session that moves between HTTP and a real browser. See the [module documentation](/stealth/api/wire).

### `StealthSession` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn builder() -> StealthSessionBuilder
pub fn fetch_kind(&self) -> FetchKind
pub fn identity(&self) -> &Identity
pub fn proxy_route(&self) -> Option<&ProxyKey>
pub fn seed(&self) -> u64
pub fn http_session(&self) -> Option<&HttpSession>
pub fn page(&self) -> Option<&Page>
pub async fn close(self) -> Result<(), StealthError>
pub async fn fetch(&mut self, url: &str) -> Result<StealthFetch, StealthError>
pub async fn rotate_fingerprint(&mut self) -> Result<(), StealthError>
pub async fn rotate_proxy(&mut self) -> Result<(), StealthError>
pub async fn export_identity(&self) -> Result<Identity, StealthError>
pub async fn import_identity(
        &mut self,
        identity: &Identity,
    ) -> Result<(), StealthError>
pub async fn escalate(&mut self) -> Result<(), StealthError>
pub async fn demote(&mut self) -> Result<(), StealthError>
```


- `builder`: Starts configuring a session.
- `fetch_kind`: How the requests are sent now: over HTTP or through the browser.
- `identity`: The identity as of the last change of mode, rotation, import or restore.
- `proxy_route`: The proxy both legs leave through; `None` for the direct connection. It carries no credentials.
- `seed`: The noise seed of the profile in use.
- `http_session`: The HTTP session, in HTTP mode: for requests of your own, such as an API call, that carry the cookies the session has earned. `None` in browser mode.
- `page`: The tab of the browser, in browser mode. `None` in HTTP mode.
- `close`: Closes the browser if one is running, and stops the MITM proxy.
- `fetch`: Fetches `url` in whichever mode the session is in, moving between HTTP and the browser as the policies decide. See the [module documentation](/stealth/api/wire) for the loop.
- `rotate_fingerprint`: Presents a new browser profile through the same egress.
- `rotate_proxy`: Leaves through another egress of the pool, with the same fingerprint in the language and time zone of the new country.
- `export_identity`: Takes the identity out of the session: the fingerprint, the cookies the leg in use holds, the clearances among them, and the egress they were earned through.
- `import_identity`: Puts an identity into the session: the leg in use is started again with its fingerprint and cookies.
- `escalate`: Goes to the browser now, with the identity as it stands, whatever the policies would say.
- `demote`: Goes back to plain HTTP now, with the cookies the browser holds, and closes the browser.

## `stealth_session::builder`

### `StealthSessionBuilder`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct StealthSessionBuilder
```

Configures a [`StealthSession`]; see [`StealthSession::builder`].

### `StealthSessionBuilder` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn launch_options(mut self, launch: LaunchOptions) -> Self
pub fn launcher<F, Fut>(mut self, launch: F) -> Self
    where
        F: Fn(LaunchOptions) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Browser, SeleniumBaseError>> + Send + 'static,
pub fn geo_proxies<I, S>(mut self, proxies: I) -> Result<Self, StealthError>
    where
        I: IntoIterator<Item = (S, CountryCode)>,
        S: AsRef<str>,
pub fn proxy_rotation(mut self, rotation: RotationStrategy) -> Self
pub fn max_challenge_attempts(mut self, attempts: u32) -> Self
pub fn mitigation(mut self, policy: MitigationPolicy) -> Self
pub fn escalation_policy(mut self, policy: EscalationPolicy) -> Self
pub fn ledger(mut self, store: Arc<dyn Ledger>) -> Self
pub fn event_sink(mut self, sink: Arc<dyn EventSink>) -> Self
pub fn geo_resolver(mut self, geo: Arc<dyn GeoResolver>) -> Self
pub fn rate_limit(mut self, calls: usize, window: Duration) -> Self
pub fn seed(mut self, seed: u64) -> Self
pub fn fingerprint(mut self, fingerprint: Fingerprint) -> Self
pub fn identity(mut self, identity: Identity) -> Self
pub fn http_session_options(mut self, options: HttpSessionOptions) -> Self
pub fn mitm_options(mut self, options: MitmOptions) -> Self
pub fn clock(mut self, clock: Clock) -> Self
pub async fn build(self) -> Result<StealthSession, StealthError>
```


- `launch_options`: The launch options of the browser, a built `LaunchOptions`. The session adds its fingerprint to a copy of them (`LaunchOptions::with_fingerprint`), so a setting made here (a user agent, say) wins over the fingerprint's. The proxy is the MITM proxy's, whatever is set here.
- `launcher`: How a browser is started: a closure from the `LaunchOptions` to a `Browser`, as core's `BrowserPool::with_launcher` takes (see [`Launcher`](/stealth/api/scrape-browser-scraper)). The default is core's `Browser::launch`.
- `geo_proxies`: Adds proxies, each with the country it exits in. The country decides the language and the time zone the session presents while it leaves through that proxy.
- `proxy_rotation`: How the pool picks an egress when one is needed: at the start and at each rotation. Takes core's `seleniumbase_rs::config::proxy_list::RotationStrategy`.
- `max_challenge_attempts`: How many loads of one URL a fetch may make before it gives up. Zero is read as one.
- `mitigation`: The whole mitigation policy: when to rotate, how long to cool down, and how long a solved challenge is waited for (its `solve` options: `SolveOptions::new().verify_timeout(..)`; the default policy waits `DEFAULT_VERIFY_TIMEOUT`, 30 s). The attempts set by `max_challenge_attempts` still apply on top; without either, the attempts are [`DEFAULT_ATTEMPTS`].
- `escalation_policy`: When the session changes mode: the mode it prefers, whether HTTP needs a clearance, how close to its end a clearance may get, and whether a solve sends the session back to HTTP.
- `ledger`: Where what is learned about each site is kept between fetches.
- `event_sink`: Where the session reports what it does. The HTTP session and the MITM proxy report to it as well.
- `geo_resolver`: How the country, time zone and language of an egress are found.
- `rate_limit`: At most `calls` fetches of one site in `window`, in either mode. The limit is per site, not global.
- `seed`: The seed of the first profile. The same seed gives the same profile, and each rotation derives the next seed from the last, so a run can be repeated.
- `fingerprint`: The first profile itself, instead of one drawn from the seed. It must claim the measured Chrome. A rotation replaces it with a generated profile of the same operating system. The rotations derive from `seed`, not from the seed of this fingerprint.
- `identity`: Starts from a saved identity: its fingerprint, its cookies and its clearances. It takes the place of `fingerprint` and `seed`.
- `http_session_options`: The limits and plumbing of the HTTP leg: timeouts, body size, redirects, the trust store and the resolver entries. The clock and the event sink of the session replace the ones in these options.
- `mitm_options`: The limits and custom inputs of the MITM proxy that carries the browser leg.
- `clock`: The clock the session reads, in seconds since the Unix epoch. The default is the system clock.
- `build`: Builds the session and starts its first leg: an HTTP session, or a browser behind the MITM proxy when the policy prefers the browser.

## `stealth_session::error`

### `StealthError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum StealthError {
    Scrape(#[from] ScrapeError),
    Http(#[from] HttpError),
    Mitm(#[from] MitmError),
    WireFormat(#[from] WireFormatError),
}
```

Why a `StealthSession` could not be made, moved or fetch a page.

## `stealth_session::fetch`

### `StealthFetch`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct StealthFetch {
    pub url: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub detection: Detection,
    pub fetch_kind: FetchKind,
    pub attempts: u32,
    pub solved: Option<ChallengeKind>,
    pub proxy: Option<ProxyKey>,
}
```

A page fetched by a `StealthSession`, by whichever leg got it.

## Related

The guide is [StealthSession](/scraping/advanced-stealthy/stealth-session). The overview is [wire](/stealth/api/wire).
