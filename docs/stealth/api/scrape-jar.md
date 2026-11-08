---
title: "scrape::jar"
sidebarTitle: "Cookie jar"
description: "The RFC 6265 cookie jar and the Set-Cookie parser."
---

<Badge color="orange">Experimental</Badge> <Badge color="blue">Crate: scrape</Badge>

`jar` stores cookies the way a browser does. `set_cookie` parses one `Set-Cookie` header. `CookieJar` is what a browser session and an `HttpSession` share.

The decision table is on [Identity and cookies](/scraping/advanced-stealthy/identity).

<RequestExample>

```rust An empty jar
use seleniumbase_rs_stealth::scrape::CookieJar;

fn main() {
    let jar = CookieJar::new();
    assert!(jar.is_empty());
}
```

</RequestExample>

## Constants and type aliases

```rust
// jar/set_cookie.rs
pub const MAX_NAME_VALUE_BYTES: usize = 4096;
pub const MAX_ATTRIBUTE_VALUE_BYTES: usize = 1024;
pub const MAX_LIFETIME_SECS: u64 = MAX_LIFETIME_DAYS * SECONDS_PER_DAY;
pub const EARLIEST_EXPIRY: u64 = 0;
pub const SECURE_PREFIX: &str = "__Secure-";
pub const HOST_PREFIX: &str = "__Host-";
// jar/store.rs
pub const MAX_COOKIES_PER_SITE: usize = 180;
pub const MAX_COOKIES: usize = 3000;
pub const MAX_BYTES_PER_SITE: usize = 256 * 1024;
pub const MAX_PATH_BYTES: usize = MAX_ATTRIBUTE_VALUE_BYTES;
pub const MAX_DOMAIN_BYTES: usize = 253;
pub const MAX_TIMESTAMP_SECS: u64 = 253_402_300_799;
```


- `MAX_NAME_VALUE_BYTES`: The most bytes the name and the value of one cookie may hold together (RFC 6265bis).
- `MAX_ATTRIBUTE_VALUE_BYTES`: The most bytes one attribute value may hold; a longer attribute is ignored (RFC 6265bis).
- `MAX_LIFETIME_SECS`: The longest lifetime a cookie may have, in seconds after it is set.
- `EARLIEST_EXPIRY`: The expiry of a cookie whose `Max-Age` is zero or negative, or whose `Expires` lies before the Unix epoch: the earliest time there is, so the cookie is expired on arrival.
- `SECURE_PREFIX`: The name prefix that obliges a cookie to be `Secure`. Matched without regard to ASCII case.
- `HOST_PREFIX`: The name prefix that obliges a cookie to be `Secure`, host-only and set with `Path=/`. Matched without regard to ASCII case.
- `MAX_COOKIES_PER_SITE`: The most cookies one site may hold. A new cookie beyond it evicts another of the same site.
- `MAX_COOKIES`: The most cookies the jar holds in all. A new cookie beyond it evicts another.
- `MAX_BYTES_PER_SITE`: The most bytes the cookies of one site may hold in all.
- `MAX_PATH_BYTES`: The most bytes a cookie path may hold, whether from the `Path` attribute or from the default path of the request URL. It is the bound of an attribute value (RFC 6265bis).
- `MAX_DOMAIN_BYTES`: The most bytes a cookie domain may hold: the longest DNS name, without a trailing dot.
- `MAX_TIMESTAMP_SECS`: The last second a four-digit year holds, 23:59:59 on 31 December 9999 UTC. A stored time past it is not a time a clock reads, so a jar file that carries one is refused.

## `jar::set_cookie`

### `CookiePrefix`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum CookiePrefix {
    Secure,
    Host,
}
```

A cookie-name prefix that carries rules (RFC 6265bis section 4.1.3).

### `CookiePrefix` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn of(text: &str) -> Option<Self>
```


- `of`: The prefix `text` starts with, compared without regard to ASCII case as Chrome compares it.

### `SetCookie`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct SetCookie {
    pub name: String,
    pub value: String,
    pub expires: Option<i64>,
    pub max_age: Option<i64>,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<SameSite>,
}
```

One `Set-Cookie` header, parsed but not yet tied to a request.

### `SetCookie` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn parse(header: &str) -> Result<Self, SetCookieError>
pub fn expiry(&self, now: u64) -> Option<u64>
pub fn prefix(&self) -> Option<CookiePrefix>
```


- `parse`: Parses one `Set-Cookie` header value (RFC 6265 section 5.2, as amended by RFC 6265bis).
- `expiry`: When the cookie expires, in seconds since the Unix epoch, or `None` for a session cookie.
- `prefix`: The prefix the cookie name carries, if any.

### `SetCookieError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum SetCookieError {
    ControlCharacter,
    Empty,
    TooLarge {
            /// The bytes the name and value take together.
            len: usize,
        },
    NamelessPrefix(CookiePrefix),
}
```

Why a `Set-Cookie` header was refused as a whole.

## `jar::store`

### `StoredCookie`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub host_only: bool,
    pub path: String,
    pub expires: Option<u64>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<SameSite>,
    pub created: u64,
    pub last_access: u64,
}
```

A cookie as the jar holds it (RFC 6265 section 5.3).

### `StoredCookie` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn is_expired(&self, now: u64) -> bool
pub fn size_bytes(&self) -> usize
pub fn same_identity(&self, other: &Self) -> bool
```


- `is_expired`: Whether the cookie has expired at `now`: it has an expiry and that time has come.
- `size_bytes`: The bytes the cookie counts for against [`MAX_BYTES_PER_SITE`]: name, value, domain, path.
- `same_identity`: Whether `other` has the same name, domain, host-only flag and path, so one replaces the other.

### `SiteContext`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum SiteContext {
    SameSite {
            /// The request is a top-level navigation. It does not change which cookies are sent.
            top_level: bool,
        },
    CrossSite {
            /// The request is a top-level navigation by a safe method, such as `GET`.
            top_level_get: bool,
        },
}
```

Where a request comes from relative to its target, which decides the `SameSite` rules (RFC 6265bis section 5.2).

### `SetCookieOutcome`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum SetCookieOutcome {
    Stored,
    Replaced,
    Deleted,
    Ignored(IgnoreReason),
}
```

What [`CookieJar::receive`] did with a `Set-Cookie` header. The enum is `non_exhaustive`.

### `IgnoreReason`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum IgnoreReason {
    Malformed,
    DomainMismatch,
    PublicSuffix,
    IpLiteralDomain,
    SecureFromInsecure,
    HostPrefix,
    SecurePrefix,
    TooLarge,
    SameSiteNoneInsecure,
    UnsupportedUrl,
}
```

Why a cookie was refused.

### `CookieJar`

<Badge color="blue" shape="pill">struct</Badge>

```rust
pub struct CookieJar
```

An RFC 6265 cookie store.

### `CookieJar` methods

<Badge color="yellow" shape="pill">method</Badge>

```rust
pub fn new() -> Self
pub fn len(&self) -> usize
pub fn is_empty(&self) -> bool
pub fn cookies(&self) -> &[StoredCookie]
pub fn receive(&mut self, request_url: &Url, set_cookie: &str, now: u64) -> SetCookieOutcome
pub fn header_for(
        &mut self,
        request_url: &Url,
        context: SiteContext,
        now: u64,
    ) -> Option<String>
pub fn from_cdp(cookies: &[Cookie], now: u64) -> Self
pub fn check_cdp(cookies: &[Cookie]) -> Result<(), JarError>
pub fn to_cdp(&self, now: u64) -> Vec<Cookie>
pub fn expiry_of(&self, name: &str, site: &str) -> Option<u64>
pub fn remove_expired(&mut self, now: u64) -> usize
```


- `new`: An empty jar.
- `len`: How many cookies the jar holds, expired ones not yet removed included.
- `is_empty`: Whether the jar holds no cookie.
- `cookies`: The cookies, in the order they were first stored.
- `receive`: Stores the cookie of one `Set-Cookie` header received for `request_url` (RFC 6265 section 5.3 with RFC 6265bis section 5.7). An overwrite keeps the old creation time; an expired cookie removes the stored one. Expired cookies are purged, and when a bound is passed the cookie evicted is, among the others, an insecure one before a `Secure` one, then the least recently accessed, then the oldest.
- `header_for`: The `Cookie` header value for a request to `request_url` made in `context`, or `None` when no cookie applies or the URL cannot carry cookies (RFC 6265 section 5.4). Cookies with longer paths come first, then those created earlier. Expired cookies are purged, and every cookie sent has its last access raised to `now`; a clock that steps back does not lower it, so a jar the library wrote always loads again.
- `from_cdp`: A jar holding `cookies` as CDP reports them (`Network.getAllCookies`). A leading dot in the domain marks a domain cookie, no dot a host-only one, and `expires: None` a session cookie. A cookie the jar would refuse (see [`IgnoreReason`]), one without a domain, and an expired one are left out; a later cookie with the same name, domain and path replaces an earlier one.
- `check_cdp`: Holds `cookies`, as core keeps them (the cookies of a `Session`), to the jar's rules, as a jar file is held when it is read back: the first one the jar would refuse, a repeat, or a bound passed is a [`JarError`]. The identity file calls it for the cookies of a session it reads.
- `to_cdp`: The unexpired cookies as CDP cookies, in the form `Network.getAllCookies` reports: a domain cookie's domain gets a leading dot, a host-only cookie's has none, and a session cookie has no expiry. Whether `Network.setCookies` keeps a host-only cookie host-only depends on how the caller sends it; a bare domain alone may be read as a `Domain` attribute.
- `expiry_of`: The latest expiry among the persistent cookies called `name` on `site`, a registrable domain or any host on it. `None` when there is no such cookie or only session cookies.
- `remove_expired`: Removes every cookie expired at `now` and returns how many there were.

### `JarError`

<Badge color="blue" shape="pill">enum</Badge>

```rust
pub enum JarError {
    Invalid {
            /// The position of the cookie in the file.
            index: usize,
            /// The rule it breaks.
            reason: IgnoreReason,
        },
    Duplicate {
            /// The position of the repeated cookie in the file.
            index: usize,
        },
    TooMany {
            /// How many cookies the file holds.
            count: usize,
        },
    SiteFull {
            /// The position of the first cookie beyond the bound.
            index: usize,
        },
    SiteBytes {
            /// The position of the first cookie beyond the bound.
            index: usize,
        },
    Lifetime {
            /// The position of the cookie in the file.
            index: usize,
        },
    Clock {
            /// The position of the cookie in the file.
            index: usize,
        },
}
```

Why a serialized jar was refused.

## Functions

### `has_forbidden_control`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn has_forbidden_control(text: &str) -> bool
```

Whether `text` holds a control character a cookie may not carry: `%x00-08`, `%x0A-1F` or `%x7F`. A horizontal tab is allowed.

### `parse_cookie_date`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn parse_cookie_date(text: &str) -> Option<i64>
```

Parses a cookie date (RFC 6265 section 5.1.1) into seconds since the Unix epoch.

### `site_of`

<Badge color="green" shape="pill">function</Badge>

```rust
pub fn site_of(host: &str) -> Option<String>
```

The registrable domain (`eTLD+1`) of `host` from the public suffix list.

## Related

The guide is [Identity and cookies](/scraping/advanced-stealthy/identity). The overview is [scrape](/stealth/api/scrape).
