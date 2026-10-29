---
title: "BrowserScraper"
sidebarTitle: "BrowserScraper"
---

<Badge color="blue">Crate: scrape</Badge>

<Note>

`BrowserScraper` is re-exported at `seleniumbase_rs_stealth::scrape`. The path
`scrape::runtime::browser_scraper::BrowserScraper` is the same type. Use either.
`ScrapeError` and `CountryCode` are re-exported beside it. No BoringSSL.

</Note>

`scrape::runtime::browser_scraper::BrowserScraper` drives one real Chrome through
the pure CDP engine.

<Note>

It does no TLS spoofing: Chrome's own handshake leaves the machine, so the page
sees a genuine browser. What it adds is the loop a bot-protected site needs
around it.

</Note>

```rust
use seleniumbase_rs_stealth::scrape::egress::geo::CountryCode;
use seleniumbase_rs_stealth::scrape::runtime::browser_scraper::BrowserScraper;

# async fn demo() -> Result<(), seleniumbase_rs_stealth::scrape::error::ScrapeError> {
let mut scraper = BrowserScraper::builder()
    .with_geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::new("DE")?)])?
    .with_max_challenge_attempts(4)
    .build()
    .await?;
let page = scraper.fetch("https://example.com/").await?;
println!("{} bytes, status {}", page.body.len(), page.status);
scraper.close().await?;
# Ok(())
# }
```

## The fetch loop

`fetch(url)` loads the URL, reads the answer and the rendered page, runs
`detect`, and asks the `MitigationPolicy` what to do:

- a real page ends the loop and comes back as a `BrowserFetch` (URL after
  redirects, status, headers, the rendered body, the `Detection`, how many loads
  it took, the challenge solved on the way, and the <Tooltip tip="The exit address a request leaves through: one of your proxies, or the direct connection.">egress</Tooltip>);
- a challenge is solved in the browser (`solve_challenge`), then the page is
  loaded again with the <Tooltip tip="The cookie a site gives once a challenge is solved.">clearance</Tooltip> it earned;
- repeated challenges change the egress (`rotate_egress`) and then the browser
  profile (`rotate_profile`). With the default of three attempts, a fetch gives
  up before the profile ladder starts unless the site's stored streak was already
  high; raise `with_max_challenge_attempts` to let one fetch climb it;
- a rate limit or a block with no egress to change to starts a cooldown for the
  site, which the next fetch respects (`ScrapeError::CoolingDown`);
- when the attempts run out the fetch fails: `ScrapeError::Unsolved` if a
  challenge was still there, `ScrapeError::GaveUp` for another reason.

In short:

```mermaid
flowchart TD
    A["fetch(url): load the URL"] --> B["detect, then MitigationPolicy"]
    B -->|a real page| C["BrowserFetch"]
    B -->|a challenge| D["solve_challenge"]
    D -->|load again with the clearance| A
    B -->|repeated challenges| E["rotate_egress, then rotate_profile"]
    E --> A
    B -->|a rate limit, or a block with no egress to change to| F["cooldown for the site"]
    B -->|the attempts run out| G["ScrapeError::Unsolved or ScrapeError::GaveUp"]
```

*One fetch, and where each answer leads*

What each site did is kept in a `StateStore` (successes, challenges, blocks, the
streak, a cooldown, the egress it last answered through). The default is in
memory; `JsonStateStore::open(path)` keeps one file, written atomically with
`0600` permissions, bounded to 10,000 sites, never reset when it cannot be read
(an unreadable file is an error and is left alone), and with one writer at a
time. Pass it with `with_state_store(Arc::new(store))`. `per_site_rate(calls,
window)` limits fetches per site.

## Egress and the profile

Proxies are `(spec, country)` pairs. A spec is `scheme://[user:password@]host:port`
with `http`, `https`, `socks5` or `socks5h`.

<Info>

The credentials are taken out of the string at once and never appear in `Debug`,
`Display`, logs, events, errors or serialized state, which carry the `EgressKey`
(scheme, host, port, country) instead.

</Info>

The country decides the language and the time zone the browser presents
while it leaves through that proxy, from a built-in locale table (the
`StaticGeoResolver`; no IP lookup service ships).

`proxy_strategy` picks how the pool chooses: `RoundRobin` (the default),
`Random { seed }`, `LeastFailures` or `StickyPerSite`. An egress with three
failures in a row is out of rotation for ten minutes by default.

- `rotate_egress()` opens a new browser context behind the new proxy and drops
  the old one with its cookies, because a clearance belongs to the address that
  earned it. When the new egress is in another country, the browser is also
  started again with the same profile in that country's language and time zone,
  because the fingerprint is written into every document before its scripts run.
  It returns `ScrapeError::NoEgressAvailable` when the pool has nothing else.
- `rotate_profile()` derives a new seed, draws a new fingerprint for the same
  operating system that claims the version of the Chrome that is running, and
  starts the browser again behind the same egress, with empty cookies. What is
  known of each site stays.
- The first profile claims the installed Chrome's major version. Unless you pass
  a `fingerprint(..)`, `build()` first starts a headless Chrome once to learn
  which version is installed, and closes it again. `seed(..)` makes a run
  repeatable.

## Identities and sessions

`export_identity()` takes the identity out of the scraper: the fingerprint, the
cookies the browser holds, the clearances among them, and the egress. Save it
with the [identity file](/stealth/identity#identity-session-policy-and-the-identity-file).
`import_identity(&id)` puts one back: it refuses an identity for another egress
or another Chrome major, restarts the browser when the fingerprint differs, and
sets the cookies. `save_session(name)` and `load_session(name)` keep cookies and
local storage in a `SessionStore`, the way a browser pool lease does.

## Events and errors

The scraper reports what it does to an `EventSink`. A `ScraperEvent` can hold a
URL only as a `RedactedUrl` (userinfo and sensitive query values removed) and a
proxy only as an `EgressKey`, so a password or token cannot reach a sink by
construction. The default sink, `TracingEventSink`, writes each event to
`tracing` on the target `seleniumbase_rs_scrape`; `CollectingEventSink` keeps
the newest events for tests. Errors are `ScrapeError`, whose URLs are redacted
too, and which map to `SeleniumBaseError::Stealth` at the crate boundary.

## Lifetime

<Warning>

A launched browser lives until `close()`. There is no idle timeout and no
deadline in the scraper: it can sit unused for as long as the process lives and
be used again.

</Warning>

## Moving a fingerprint to a locale

A built `Fingerprint` is read through accessors and never changed: every setting is
one `Spoof` (`Natural`, `Random`, `Value` or `Off`) of the core's fingerprint API,
and a change builds the next fingerprint. The locale helpers follow that.
`LocaleExt::localize` and `EgressLocale::localize` return the fingerprint moved to
the locale (language list, `Accept-Language`, time zone and a position near the
city, a function of the fingerprint's seed) and leave the one they were given as it
was. `EgressLocale::localize` takes the time zone its egress reported. This is the
move `rotate_egress` makes when the new proxy is in another country.

```rust
use seleniumbase_rs_stealth::stealth::fingerprint::{Fingerprint, OsType};
use seleniumbase_rs_stealth::scrape::egress::geo::{Locale, LocaleExt};

# fn demo() -> Result<(), Box<dyn std::error::Error>> {
let fingerprint = Fingerprint::randomized_with_major(OsType::Macos, 7, 155);
let german = Locale::by_tag("de-DE").ok_or("no such locale")?;

let moved = german.localize(&fingerprint)?;

assert_eq!(moved.locale().as_str(), Some("de-DE"));
assert_eq!(moved.timezone().as_str(), Some("Europe/Berlin"));
assert_eq!(moved.user_agent(), fingerprint.user_agent());
# Ok(())
# }
```

Every other setting is kept, and a fingerprint that denies geolocation
(`Spoof::Off`) keeps denying it instead of being handed a position. The move
fails with `ScrapeError::Fingerprint` (`FingerprintError` for `LocaleExt`) when the
core refuses to build the result; a fingerprint that was built before does not
cause that. To change anything else, start from `fingerprint.to_builder()`, change
it and call `build()?`.

## The builder

`BrowserScraper::builder()` returns a `BrowserScraperBuilder`. Every setting has
a default, so `builder().build().await?` is a working scraper: Chrome, no proxy,
state in memory, events on `tracing`, three attempts, a 30 second solve timeout
(`DEFAULT_SOLVE_TIMEOUT`), and a profile drawn from a random seed. Unless
`fingerprint` is set, `build` starts a headless Chrome once to learn the installed
major and closes it before starting the browser it keeps.

| Method | Default | What it sets |
|---|---|---|
| `launch_options` | the platform's | Applied first. A user agent set here wins over the fingerprint's. |
| `with_geo_proxies` | none | `(spec, country)` pairs. A bad spec is `ScrapeError::Egress` and never echoes the password. |
| `proxy_strategy` | `RoundRobin` | Also `Random { seed }`, `LeastFailures`, `StickyPerSite`. |
| `with_max_challenge_attempts` | 3 | Zero is read as one. The default gives up before the profile ladder starts. |
| `mitigation` | `MitigationPolicy::default()` | When to rotate and how long to cool down. The attempts above still win. |
| `with_state_store` | in memory | `JsonStateStore::open(path)` for the file. |
| `with_event_sink` | `TracingEventSink` | `CollectingEventSink` keeps the newest events. |
| `geo_resolver` | `StaticGeoResolver` | The built-in locale table. No IP lookup ships. |
| `per_site_rate` | none | At most `calls` fetches of one site in `window`. |
| `seed` | random | Same seed, same first profile. Each rotation derives the next seed. |
| `fingerprint` | drawn from the seed | Skips the version probe. A rotation replaces it. |
| `session_store` | empty | Where `save_session` writes, to share a login with a `BrowserPool`. |
| `solve_timeout` | 30 s | How long a solved challenge is waited for. |
| `clock` | the system clock | Seconds since the Unix epoch. Tests replace it. |

`egress_switch` and `launcher` replace the defaults (`ContextProxySwitch`,
`ChromeLauncher`). A program does not need to set them. `build` returns
`ScrapeError::Browser` when no browser starts and `ScrapeError::Geo` when an
egress's country has no locale. The methods and the fields of `BrowserFetch`
are on [the API page](/stealth/api/scrape#re-exports-at-the-root).

## Where to go next

<Columns cols={2}>

<Card title="Identity and cookies" icon="cookie" color="#199f43" href="/stealth/identity">
  The jar, the identity file, and the decision table `fetch` follows.
</Card>

<Card title="HttpSession" icon="network-wired" color="#a631be" href="/stealth/http-session">
  Carry the cookies this scraper earned as plain HTTP. Needs `mimic`.
</Card>

<Card title="StealthSession" icon="shuffle" color="#d32a61" href="/stealth/stealth-session">
  The same loop, with plain HTTP as the first move.
</Card>

<Card title="Limits" icon="triangle-exclamation" color="#d5a910" href="/stealth/limits">
  What a fetch refuses, and what each `ScrapeError` means.
</Card>

</Columns>
