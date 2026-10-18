---
title: "BrowserScraper"
---

`scrape::runtime::browser_scraper::BrowserScraper` drives one real Chrome through
the pure CDP engine.

<Note>

It does no TLS spoofing: Chrome's own handshake leaves the machine, so the page
sees a genuine browser. What it adds is the loop a bot-protected site needs
around it.

</Note>

```rust
use seleniumbase_rs_scrape::egress::geo::CountryCode;
use seleniumbase_rs_scrape::runtime::browser_scraper::BrowserScraper;

# async fn demo() -> Result<(), seleniumbase_rs_scrape::error::ScrapeError> {
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
  it took, the challenge solved on the way, and the egress);
- a challenge is solved in the browser (`solve_challenge`), then the page is
  loaded again with the clearance it earned;
- repeated challenges change the egress (`rotate_egress`) and then the browser
  profile (`rotate_profile`). With the default of three attempts, a fetch gives
  up before the profile ladder starts unless the site's stored streak was already
  high; raise `with_max_challenge_attempts` to let one fetch climb it;
- a rate limit or a block with no egress to change to starts a cooldown for the
  site, which the next fetch respects (`ScrapeError::CoolingDown`);
- when the attempts run out the fetch fails: `ScrapeError::Unsolved` if a
  challenge was still there, `ScrapeError::GaveUp` for another reason.

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
