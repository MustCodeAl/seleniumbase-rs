---
title: "scrape::egress"
sidebarTitle: "Egress"
description: "Proxy specs, the rotating pool, countries and locales, and URL redaction."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

`egress` is where a scrape leaves the machine. `route` parses a proxy spec into an `EgressRef` and an `EgressKey` that carries no credentials. `pool` rotates those keys. `geo` maps a country to a language and a time zone. `redact` builds a `RedactedUrl` so a log cannot echo a password.

How a clearance stays with the address that earned it is on [Identity and cookies](/stealth/identity).

<RequestExample>

```rust A country and a redacted URL
use seleniumbase_rs_stealth::scrape::{CountryCode, RedactedUrl, ScrapeError};

fn main() -> Result<(), ScrapeError> {
    let de = CountryCode::new("DE")?;
    let shown = RedactedUrl::new("https://user:secret@example.com/a?token=1");
    println!("{} {shown}", de.as_str());
    Ok(())
}
```

</RequestExample>

## Constants and type aliases

```rust
// egress/pool.rs
pub const DEFAULT_QUARANTINE_AFTER: u32 = 3;
pub const DEFAULT_QUARANTINE_SECS: u64 = 600;
pub const MAX_STICKY_SITES: usize = 4096;
// egress/redact.rs
pub const SENSITIVE_QUERY_KEYS: [&str; 12] = [
```


- `DEFAULT_QUARANTINE_AFTER`: Failures in a row that quarantine an egress, unless the pool says otherwise.
- `DEFAULT_QUARANTINE_SECS`: Seconds a quarantined egress stays out of rotation, unless the pool says otherwise.
- `MAX_STICKY_SITES`: How many sites [`RotationStrategy::StickyPerSite`] remembers.
- `SENSITIVE_QUERY_KEYS`: Query keys whose values are replaced by `***`. Matching ignores ASCII case and sees through percent-encoding, so `to%6Ben` is `token`.

## `egress::geo`

### `CountryCode`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct CountryCode([u8; 2]);
```

An ISO 3166-1 alpha-2 country code, upper case. Only a valid one can be built.

### `CountryCode` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new(code: &str) -> Result<Self, GeoError>
pub fn as_str(&self) -> &str
```


- `new`: Validates a two-letter code and upper-cases it.
- `as_str`: The code as text, such as `DE`.

### `GeoError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum GeoError {
    InvalidCountry(String),
    NoLocale(CountryCode),
    Unknown(EgressKey),
}
```

Why a geo lookup or a country code was refused.

### `LocaleExt`

<Badge color="blue" shape="pill">trait</Badge>

```rust
pub trait LocaleExt
```

What the scrape layer adds to the core's [`Locale`].

### `LocaleExt` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
fn for_country(country: CountryCode) -> Option<&'static Locale>
fn by_tag(tag: &str) -> Option<&'static Locale>
fn localize(&self, fingerprint: &Fingerprint) -> Result<Fingerprint, FingerprintError>
```


- `for_country`: The first locale of `country` in [`LOCALES`]. The US has four time zones under one country code; the first of them is returned.
- `by_tag`: The locale whose tag is `tag`, such as `de-DE`. BCP 47 tags are case-insensitive, so `de-de` finds it too.
- `localize`: The fingerprint `fingerprint` becomes in this locale: the language list, the `Accept-Language` header, the time zone and a position near the city are this locale's, and every other setting is kept. The position is a function of the fingerprint's seed, so the same identity always reports the same spot. A fingerprint that denies geolocation (`Spoof::Off`) keeps denying it.

### `GeoInfo`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct GeoInfo {
    pub country: CountryCode,
    pub timezone: Option<String>,
    pub coordinates: Option<(f64, f64)>,
}
```

What a geo lookup learned about an egress.

### `GeoResolver`

<Badge color="blue" shape="pill">trait</Badge>

```rust
pub trait GeoResolver
```

Resolves the country, time zone and position of an egress. The shipped implementation, [`StaticGeoResolver`], answers from the proxy spec and the locale table, and never looks up an address.

### `GeoResolver` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
async fn resolve(&self, egress: &EgressRef) -> Result<GeoInfo, GeoError>
```


- `resolve`: Looks up the geo information for `egress`.

### `StaticGeoResolver`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct StaticGeoResolver;
```

Answers from the country given with the proxy spec, through the locale table. No network lookup ships.

## `egress::pool`

### `RotationStrategy`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum RotationStrategy {
    RoundRobin,
    Random {
            /// The seed of the draw.
            seed: u64,
        },
    LeastFailures,
    StickyPerSite,
}
```

How the pool chooses among the egresses that are available.

### `EgressOutcome`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum EgressOutcome {
    Success,
    ConnectError,
    Challenged,
    Blocked,
}
```

What a request through an egress came to.

### `ProxyPool`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct ProxyPool
```

A rotating set of egresses, with failure counting and quarantine.

### `ProxyPool` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new(egresses: Vec<EgressRef>, strategy: RotationStrategy) -> Self
pub fn with_geo_proxies<I, S>(items: I, strategy: RotationStrategy) -> Result<Self, EgressError>
    where
        I: IntoIterator<Item = (S, CountryCode)>,
        S: AsRef<str>,
pub fn quarantine(mut self, after_failures: u32, for_secs: u64) -> Self
pub fn pick(&self, site: Option<&str>, now: u64) -> Option<EgressRef>
pub fn pick_in(&self, country: CountryCode, site: Option<&str>, now: u64) -> Option<EgressRef>
pub fn pick_other(
        &self,
        current: &EgressKey,
        site: Option<&str>,
        now: u64,
    ) -> Option<EgressRef>
pub fn report(&self, egress: &EgressKey, outcome: EgressOutcome, now: u64)
```


- `new`: A pool over `egresses`, with the default quarantine.
- `with_geo_proxies`: A pool built from `(spec, country)` pairs, each spec parsed the way [`EgressRef::parse`] parses it.
- `quarantine`: Sets how many failures in a row quarantine an egress, and for how many seconds. A limit of zero is read as one: the first failure quarantines.
- `pick`: The egress for a request to `site` at time `now`, in seconds. `None` when the pool is empty or every egress is quarantined.
- `pick_in`: Like [`ProxyPool::pick`], limited to egresses in `country`.
- `pick_other`: Like [`ProxyPool::pick`], excluding `current`, so that a rotation really moves. With [`RotationStrategy::StickyPerSite`] the site then sticks to the new egress.
- `report`: Records what happened to a request through `egress` at time `now`. A success clears the failure count. A failure adds one, and once the count reaches the pool's limit the egress is quarantined and its count starts again. An egress the pool does not hold is ignored.

## `egress::redact`

### `Secret`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct Secret(String);
```

A secret string. It prints as `&lt;redacted&gt;` and is never serialized. [`Secret::expose`] is the one way out, and it belongs where the value must leave the process.

### `Secret` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new(value: impl Into<String>) -> Self
pub fn expose(&self) -> &str
```


- `new`: Wraps a secret value.
- `expose`: The secret value, for the one consumer that needs it, such as a launch command.

### `RedactedUrl`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct RedactedUrl(String);
```

A URL with its userinfo and fragment removed and its sensitive query values replaced. The only constructor redacts, so every value of this type is safe to log and to serialize.

### `RedactedUrl` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new(raw: &str) -> Self
pub fn placeholder() -> Self
pub fn as_str(&self) -> &str
```


- `new`: Redacts `raw`. A string that does not parse as a URL becomes a fixed placeholder, and a URL without a host (`data:`, `mailto:`) keeps only its scheme.
- `placeholder`: The stand-in for an address that could not be read, for an error that must not echo what it was given.
- `as_str`: The redacted URL as text.

## `egress::route`

### `EgressKey`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct EgressKey {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub country: Option<CountryCode>,
}
```

The part of an egress that may be stored, logged and serialized: where traffic goes, never how to authenticate. Only this type is ever serialized.

### `EgressRef`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct EgressRef
```

A proxy the traffic leaves through. Its credentials live in the [`Proxy`] and in the spec kept for launching; neither is printed or serialized, and this type does not implement `Serialize`.

### `EgressRef` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn parse(spec: &str, country: Option<CountryCode>) -> Result<Self, EgressError>
pub fn key(&self) -> &EgressKey
pub fn proxy(&self) -> &Proxy
```


- `parse`: Parses `spec`, written as `scheme://[user:password@]host:port`, or as `host:port` for http. The credentials are taken out of the string here; nothing downstream sees them unless it asks for the proxy or the launch spec. They are passed on as written, not percent-decoded, as [`Proxy::parse`] takes them. The `socks5` schemes need a port; `http` and `https` default to 80 and 443. A spec with a path, query or fragment is refused.
- `key`: The storable part of this egress.
- `proxy`: The proxy for a browser context, such as `ContextOptions::proxy`.

### `EgressError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum EgressError {
    Scheme(String),
    Address(RedactedUrl),
}
```

Why an egress spec was refused.

## Related

The guide is [Identity and cookies](/stealth/identity). The overview is [scrape](/stealth/api/scrape).
