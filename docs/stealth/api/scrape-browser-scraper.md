---
title: "scrape::runtime::browser_scraper"
sidebarTitle: "BrowserScraper"
description: "BrowserScraper, its builder, BrowserFetch and the launcher port."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

`runtime::browser_scraper` drives one real Chrome through `sb_cdp`. It does no TLS spoofing: Chrome's own handshake leaves the machine. `fetch` loads a URL, reads the rendered page, and follows `MitigationPolicy`. `close` takes the scraper by value.

Import the main types from `seleniumbase_rs_stealth::scrape`. The fetch loop is on [BrowserScraper](/stealth/browser-scraper).

<RequestExample>

```rust A scraper behind a German proxy
use seleniumbase_rs_stealth::scrape::{BrowserScraper, CountryCode, ScrapeError};
use seleniumbase_rs_stealth::tokio;

#[tokio::main]
async fn main() -> Result<(), ScrapeError> {
    let mut scraper = BrowserScraper::builder()
        .with_geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::new("DE")?)])?
        .with_max_challenge_attempts(4)
        .build()
        .await?;
    let page = scraper.fetch("https://example.com/").await?;
    println!("{} bytes, status {}", page.body.len(), page.status);
    scraper.close().await
}
```

</RequestExample>

## Constants and type aliases

```rust
// runtime/browser_scraper/builder.rs
pub type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;
pub const DEFAULT_SOLVE_TIMEOUT: Duration = Duration::from_secs(30);
```


- `Clock`: Tells the scraper the time, in seconds since the Unix epoch. Tests replace it.
- `DEFAULT_SOLVE_TIMEOUT`: How long a challenge is given to show as solved, unless the builder says otherwise.

## `runtime::browser_scraper`

### `BrowserFetch`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct BrowserFetch {
    pub url: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub detection: Detection,
    pub attempts: u32,
    pub solved: Option<ChallengeKind>,
    pub egress: Option<EgressKey>,
}
```

A page fetched through a real browser.

### `BrowserScraper`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct BrowserScraper
```

A scraper that drives one real browser. See the [module documentation](self).

### `BrowserScraper` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn builder() -> BrowserScraperBuilder
pub fn page(&self) -> &Page
pub fn browser(&self) -> &Browser
pub fn fingerprint(&self) -> &Fingerprint
pub fn seed(&self) -> u64
pub fn egress(&self) -> Option<&EgressKey>
pub fn sessions(&self) -> &SessionStore
pub async fn close(self) -> Result<(), ScrapeError>
pub async fn fetch(&mut self, url: &str) -> Result<BrowserFetch, ScrapeError>
pub async fn solve_challenge(&mut self) -> Result<Detection, ScrapeError>
pub async fn rotate_egress(&mut self) -> Result<(), ScrapeError>
pub async fn rotate_profile(&mut self) -> Result<(), ScrapeError>
pub async fn export_identity(&self) -> Result<StealthIdentity, ScrapeError>
pub async fn import_identity(&mut self, identity: &StealthIdentity) -> Result<(), ScrapeError>
pub async fn save_session(&self, name: impl Into<String>) -> Result<(), ScrapeError>
pub async fn load_session(&self, name: &str) -> Result<bool, ScrapeError>
```


- `builder`: Starts configuring a scraper.
- `page`: The tab the scraper loads pages in. It changes when the egress or the profile does.
- `browser`: The browser the scraper drives. It changes when the profile does.
- `fingerprint`: The fingerprint the browser presents.
- `seed`: The noise seed of the profile in use.
- `egress`: The egress the browser leaves through; `None` for the direct connection.
- `sessions`: The sessions saved with [`save_session`](Self::save_session).
- `close`: Closes the browser. A scraper that is dropped instead leaves the browser to its own drop.
- `fetch`: Fetches `url` through the browser, solving challenges on the way.
- `solve_challenge`: Reads the page now open, solves the challenge on it if there is one, and waits for the browser to show the clearance. Returns what the page is afterwards.
- `rotate_egress`: Leaves through another egress of the pool.
- `rotate_profile`: Presents a new browser profile.
- `export_identity`: Takes the identity out of the scraper: the fingerprint, the cookies the browser holds, the clearances among them, and the egress they were earned through.
- `import_identity`: Puts an identity into the scraper: the browser is started again with its fingerprint when it differs from the one in use, and its cookies are set.
- `save_session`: Saves the cookies of the browser context and the local storage of the page's origin in the scraper's [`SessionStore`](seleniumbase_rs::sb_cdp::SessionStore) as `name`, the way a pool lease saves its login.
- `load_session`: Loads the session saved as `name` into the browser context. Returns `false`, changing nothing, when there is no such session.

## `runtime::browser_scraper::builder`

### `BrowserScraperBuilder`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct BrowserScraperBuilder
```

Configures a [`BrowserScraper`]; see [`BrowserScraper::builder`].

### `BrowserScraperBuilder` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn launch_options(mut self, launch: LaunchOptionsBuilder) -> Self
pub fn with_geo_proxies<I, S>(mut self, proxies: I) -> Result<Self, ScrapeError>
    where
        I: IntoIterator<Item = (S, CountryCode)>,
        S: AsRef<str>,
pub fn proxy_strategy(mut self, strategy: RotationStrategy) -> Self
pub fn with_max_challenge_attempts(mut self, attempts: u32) -> Self
pub fn mitigation(mut self, policy: MitigationPolicy) -> Self
pub fn with_state_store(mut self, store: Arc<dyn StateStore>) -> Self
pub fn with_event_sink(mut self, sink: Arc<dyn EventSink>) -> Self
pub fn geo_resolver(mut self, geo: Arc<dyn GeoResolver>) -> Self
pub fn per_site_rate(mut self, calls: usize, window: Duration) -> Self
pub fn seed(mut self, seed: u64) -> Self
pub fn fingerprint(mut self, fingerprint: Fingerprint) -> Self
pub fn egress_switch(mut self, switch: Box<dyn EgressSwitch>) -> Self
pub fn launcher(mut self, launcher: Arc<dyn BrowserLauncher>) -> Self
pub fn session_store(mut self, sessions: SessionStore) -> Self
pub fn solve_timeout(mut self, timeout: Duration) -> Self
pub fn clock(mut self, clock: Clock) -> Self
pub async fn build(self) -> Result<BrowserScraper, ScrapeError>
```


- `launch_options`: The launch options of the browser. The fingerprint of the scraper is applied after them, so a setting made here (a user agent, say) wins over the fingerprint's.
- `with_geo_proxies`: Adds proxies, each with the country it exits in. The country decides the language and the time zone the browser presents while it leaves through that proxy.
- `proxy_strategy`: How the pool picks an egress when one is needed: at the start and at each rotation.
- `with_max_challenge_attempts`: How many loads of one URL a fetch may make before it gives up. Zero is read as one.
- `mitigation`: The whole mitigation policy: when to rotate, how long to cool down. The attempts set by [`with_max_challenge_attempts`](Self::with_max_challenge_attempts) still apply on top.
- `with_state_store`: Where what is learned about each site is kept between fetches.
- `with_event_sink`: Where the scraper reports what it does.
- `geo_resolver`: How the country, time zone and language of an egress are found.
- `per_site_rate`: At most `calls` fetches of one site in `window`. The limit is per site, not global.
- `seed`: The seed of the first profile. The same seed gives the same profile, and each rotation derives the next seed from the last, so a run can be repeated.
- `fingerprint`: The first profile itself, instead of one drawn from the seed. A rotation replaces it with a generated profile of the same operating system.
- `egress_switch`: How the egress is changed. The default opens a browser context behind the new proxy.
- `launcher`: How a browser is started. The default starts Chrome.
- `session_store`: The store [`save_session`](BrowserScraper::save_session) writes to, to share logins with other scrapers or a [`BrowserPool`](seleniumbase_rs::sb_cdp::BrowserPool).
- `solve_timeout`: How long a solved challenge is waited for. The default is [`DEFAULT_SOLVE_TIMEOUT`].
- `clock`: The clock the scraper reads. The default is the system clock.
- `build`: Starts the browser and returns the scraper.

## `runtime::browser_scraper::launcher`

### `BrowserLauncher`

<Badge color="blue" shape="pill">trait</Badge>

```rust
pub trait BrowserLauncher
```

Starts a browser for a [`BrowserScraper`](super::BrowserScraper).

### `BrowserLauncher` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
async fn launch(&self, options: LaunchOptions) -> Result<Browser, ScrapeError>
```


- `launch`: Starts a browser with `options`.

### `ChromeLauncher`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct ChromeLauncher;
```

Starts Chrome (or the browser the launch options name) with [`Browser::launch`].

## Related

The guide is [BrowserScraper](/stealth/browser-scraper). The overview is [scrape](/stealth/api/scrape).
