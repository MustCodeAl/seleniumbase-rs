---
title: "StealthSession"
---

# StealthSession

`mimic::stealth_session::StealthSession` joins the pieces. It presents one
`StealthIdentity` (the fingerprint, the measured browser it belongs to, the
cookies it earned and the egress they were earned through) in one of two modes
and moves between them by the policies:

- `Mode::Http`: an [`HttpSession`](http-session.md#httpsession) sends the requests. It is fast
  and cheap, with the measured handshake and headers.
- `Mode::Browser`: a real Chrome sends them, behind a
  [`MitmProxy`](mitm-proxy.md#mitmproxy) that replays each request with the same measured
  handshake. A clearance earned here is valid in the other mode.

```rust
use seleniumbase_rs_mimic::StealthSession;
use seleniumbase_rs_scrape::CountryCode;

# async fn demo() -> Result<(), Box<dyn std::error::Error>> {
let mut session = StealthSession::builder()
    .with_geo_proxies([("http://user:pass@de.proxy.example:8080", CountryCode::new("DE")?)])?
    .build()
    .await?;
let page = session.fetch("https://example.com/").await?;
println!("{} bytes, {:?} mode", page.body.len(), page.mode);
session.close().await?;
# Ok(())
# }
```

## The loop

A session starts in the mode its `SessionPolicy` prefers (HTTP by default).
`fetch(url)` asks the policies three questions:

1. **Before the request**, `SessionPolicy::before_fetch`: does the identity hold
   a clearance for the site that is about to end, must it hold one and does not,
   or does the site's history say plain requests fail? Then the session goes to
   the browser first.
2. **After the answer**, `MitigationPolicy::decide`: a real page ends the loop; a
   challenge met over HTTP sends the session to the browser (an escalation); in
   the browser the challenge is solved and the cookies move into the identity; a
   block or a rate limit starts a cooldown, rotates the egress or the profile, or
   stops the fetch.
3. **After a solve**, `SessionPolicy::after_fetch`: a browser that earned a
   clearance hands the identity back to HTTP (a demotion), which carries it on.

Every change of mode emits `ScraperEvent::ModeChanged` with its reason. The
browser is closed when the session goes back to HTTP and started again, with the
same identity and egress, when it is needed. **The MITM proxy is started the first
time a browser is needed** (at `build()` when the policy prefers the browser) and
lives until `close()`, so its port and its certificate stay the same across
browsers.

A site in cooldown is not requested at all. The state of the site (successes,
challenges, cooldown) is read before the first load and saved after the last,
whether the fetch worked or not. The attempts per fetch default to 4
(`DEFAULT_ATTEMPTS`, one more than a browser-only scraper): one load meets the
challenge over HTTP, one in the browser solves it, and one over HTTP uses the
clearance.

`fetch` returns a `StealthFetch`: `url` after redirects, `status`, `headers`, the
`body` (the response body over HTTP, the rendered page in the browser), the
`detection`, the `mode` it was got in, how many loads it took (`attempts`), the
challenge `solved` on the way, and the `egress`. A page that took a challenge to
reach is usually an HTTP page: the browser earned the clearance and the session
went back to HTTP to use it. A body that is not UTF-8 is read lossily.

In short, the two modes and what moves a session between them:

```mermaid
stateDiagram-v2
    [*] --> Http: preferred mode (default)
    Http --> Browser: before_fetch, or a challenge met over HTTP (escalation)
    Browser --> Http: a clearance earned (demotion, after_fetch)
```

## Building one

`StealthSession::builder()` has a default for everything: Chrome with the
platform's launch options, no proxy, state in memory, events written to `tracing`,
HTTP preferred, and a profile drawn from a random seed.

| Setting | What it does |
|---|---|
| `launch_options(builder)`, `launcher(..)` | How the browser starts. A setting made in the options (a user agent, say) wins over the fingerprint's, and the proxy is always the MITM proxy's. |
| `with_geo_proxies([(spec, country)])`, `proxy_strategy(..)` | The egress pool and how it picks, as for `BrowserScraper`. Both legs leave through the same egress. |
| `with_max_challenge_attempts(n)`, `mitigation(policy)` | Loads per fetch (zero reads as one) and the whole mitigation policy. |
| `policy(SessionPolicy)` | The mode it prefers, whether HTTP needs a clearance, the clearance margin, whether a solve sends it back to HTTP. |
| `with_state_store(..)`, `with_event_sink(..)`, `geo_resolver(..)`, `per_site_rate(calls, window)` | As for `BrowserScraper`. The HTTP leg and the proxy report to the same sink. |
| `seed(..)`, `fingerprint(..)` | The first profile. A fingerprint must claim the measured Chrome. |
| `restore(identity)` | Starts from a saved identity: its fingerprint, cookies and clearances. It must belong to one of the proxies added, or to the direct connection when there is none. |
| `http_options(HttpOptions)`, `mitm_options(MitmOptions)` | The limits of each leg. The session's clock and event sink replace the ones in the HTTP options. |
| `solve_timeout(..)`, `clock(..)` | How long a solve is waited for, and the clock. |

`build()` fails with `StealthError::Impersonate` when this machine's system has no
measured Chrome or the profile claims another version, `StealthError::Scrape` for
an egress whose country has no locale, an identity that belongs to another egress
or no browser to start, `StealthError::Http` and `StealthError::Mitm` for the
legs.

## Moving by hand

| Call | What it does |
|---|---|
| `escalate()` | Goes to the browser now, with the identity as it stands. The cookies of the HTTP session move into the browser. |
| `demote()` | Goes back to HTTP now, with the cookies the browser holds, and closes the browser. A browser that cannot be read (it crashed) does not trap the session: the identity as of the last read is used. |
| `rotate_profile()` | A new seed derived from the old one, a new fingerprint for the same operating system that claims the measured Chrome. Cookies and clearances are dropped; what is known of each site stays. In browser mode a new Chrome starts behind the same proxy, on the same port. |
| `rotate_egress()` | Another egress of the pool, with the same fingerprint in the language and time zone of the new country. The proxy is pointed at it first, so its port stays. Cookies and clearances are dropped, because a clearance belongs to the address that earned it. `NoEgressAvailable` when the pool has nothing else. |
| `export_identity()`, `import_identity(&id)` | Take the identity out of the session, or put one in. It must belong to the egress in use. |
| `mode()`, `identity()`, `egress()`, `seed()`, `http()`, `page()` | Read the state. `http()` is the `HttpSession` in HTTP mode, for requests of your own that carry the earned cookies; `page()` is the tab in browser mode. |
| `close()` | Closes the browser if one runs and stops the proxy. A dropped session leaves both to their own drop. |

On an error from a rotation, an import or a change of mode the session is as it
was.

## Errors

`StealthError` has one variant per layer: `Scrape` (the browser leg, the site
state, an identity or the mitigation policy; `Unsolved`, `GaveUp` and
`CoolingDown` come as this variant), `Http`, `Mitm` and `Impersonate`. Each
already keeps URLs redacted and credentials out of its text. At the crate
boundary it maps to `SeleniumBaseError`.

## What to know

- The measured Chrome and the installed Chrome must have the same major version.
  A browser that is another version is refused before it loads a page
  (`StealthError::Impersonate`).
- A launched browser lives until the session leaves the browser mode or is
  closed. There is no idle timeout and no deadline in the session.
- It runs the same fetch loop as `BrowserScraper`, with plain HTTP as the first
  move. A matching handshake is one signal among many, and a session does not
  make automation permitted by a site's terms. See [Limits](limits.md#limits).

## What was run

`tests/stealth_session.rs` runs the real HTTP leg against a loopback TLS shop
that trusts only its own ephemeral authority, with a scripted browser
(`Browser::new_mocked`, feature `test-util`): every transition of the state
machine and its events, export and restore, rotating the profile and the egress,
and that secrets stay out of output. The MITM proxy is started and the browser
is launched behind it, but the traffic of a browser is not made.

`tests/stealth_session_chrome.rs` runs the story with the real Chrome 155 on
macOS, headless (`#[ignore]`d): the session asks over HTTP and meets a script
challenge on a loopback TLS shop; it starts the proxy and a Chrome behind it with
the same fingerprint and egress; Chrome runs the challenge page's script, which
earns the clearance cookie; the cookies move into the identity, Chrome closes,
and the session asks again over HTTP with the clearance. The shop sees one
`ClientHello` shape and one user agent from both legs. A second test takes a
clearance near its end back to Chrome, and a third shows that a rotated profile
is what the page sees while the proxy stays. The shop is a stand-in written for
the test: no vendor's challenge was run.
