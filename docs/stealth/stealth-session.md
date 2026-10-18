---
title: "StealthSession"
---

# StealthSession

`mimic::stealth_session::StealthSession` joins the pieces. It presents one
`StealthIdentity` (the fingerprint, the measured browser it belongs to, the
cookies it earned and the egress they were earned through) in one of two modes
and moves between them by the policies:

- `Mode::Http`: an [`HttpSession`](/stealth/http-session#httpsession) sends the requests. It is fast
  and cheap, with the measured handshake and headers.
- `Mode::Browser`: a real Chrome sends them, behind a
  [`MitmProxy`](/stealth/mitm-proxy#mitmproxy) that replays each request with the same measured
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

<Steps>

<Step title="Before the request">

`SessionPolicy::before_fetch`: does the identity hold
a clearance for the site that is about to end, must it hold one and does not,
or does the site's history say plain requests fail? Then the session goes to
the browser first.

</Step>

<Step title="After the answer">

`MitigationPolicy::decide`: a real page ends the loop; a
challenge met over HTTP sends the session to the browser (an escalation); in
the browser the challenge is solved and the cookies move into the identity; a
block or a rate limit starts a cooldown, rotates the egress or the profile, or
stops the fetch.

</Step>

<Step title="After a solve">

`SessionPolicy::after_fetch`: a browser that earned a
clearance hands the identity back to HTTP (a demotion), which carries it on.

</Step>

</Steps>

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

## What adapts, and what it presents

Nothing in a session guesses. It adapts because each decision reads state that
the earlier fetches wrote:

- **Per site:** the ledger counts successes and challenges for each site. A site
  whose history says plain requests fail is fetched in the browser first
  (`EscalateReason::SiteNeedsBrowser`), and a blocked or rate-limited site gets a
  cooldown, so it is not asked again until the cooldown ends.
- **Per identity:** the identity holds the clearances it earned. A clearance that
  ends within `clearance_margin` (120 seconds by default) sends the next fetch back
  to the browser to renew it (`EscalateReason::ClearanceExpiring`); a valid one
  keeps the session on plain HTTP.
- **Per answer:** the mitigation policy reads each response. A challenge met over
  HTTP escalates; a challenge solved in the browser demotes once the identity
  holds the clearance (`demote_after_solve`, on by default); repeated blocks rotate
  the egress and then the profile.

`SessionPolicy` holds the four switches: `prefer` (the mode a session starts in
and returns to, HTTP by default), `require_clearance_for_http`,
`clearance_margin` and `demote_after_solve`.

The fingerprint a session presents is drawn once from its seed:
`Fingerprint::randomized_with_major(os, seed, major)` gives a coherent persona
for this machine's operating system that claims the measured Chrome major, and
it is then moved to the language and time zone of the egress country. Like every
randomized identity it carries `StealthPolicy::balanced()`: the WebRTC leak
policy `DisableNonProxiedUdp`, seeded canvas and audio noise, the `window.chrome`
stubs and the headless fixes (see the
[fingerprint tutorial](/tutorials/fingerprint_stealth)). The fingerprint
only changes when you rotate the profile or the egress, or pass your own with
`fingerprint(..)`, which must claim the measured Chrome.

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

### When to move by hand

The policies move a session when a site answers with a challenge. Move it
yourself when you know more than the answers show:

- **`escalate()` before** a step that needs a real page: a login form, a page
  that renders its content with JavaScript, a button that must be clicked. The
  cookies HTTP collected go with it.
- **`demote()` after** that step, once the browser holds what you came for (the
  login cookie, a clearance): the following fetches are plain HTTP again, much
  faster, and carry the browser's cookies.
- Leave both alone for ordinary fetching; `fetch` escalates and demotes on its own.

```rust
use seleniumbase_rs_mimic::StealthSession;

# async fn login_then_scrape(password: &str) -> Result<(), Box<dyn std::error::Error>> {
let mut session = StealthSession::builder().build().await?;

// Log in where a real browser is needed.
session.escalate().await?;
let page = session.page().expect("a session in browser mode has a page");
page.goto("https://shop.example/login").await?;
page.locator("#user").type_text("me@example.com").await?;
page.locator("#password").type_text(password).await?;
page.locator("button[type=submit]").click().await?;

// Back to plain HTTP: the browser's cookies, the login among them, move into the
// identity and the browser closes.
session.demote().await?;
for n in 1..=20 {
    let orders = session.fetch(&format!("https://shop.example/orders?page={n}")).await?;
    println!("page {n}: {} bytes over {:?}", orders.body.len(), orders.mode);
}
session.close().await?;
# Ok(())
# }
```

## Errors

`StealthError` has one variant per layer: `Scrape` (the browser leg, the site
state, an identity or the mitigation policy; `Unsolved`, `GaveUp` and
`CoolingDown` come as this variant), `Http`, `Mitm` and `Impersonate`. Each
already keeps URLs redacted and credentials out of its text. At the crate
boundary it maps to `SeleniumBaseError`.

## What to know

<Warning>

- The measured Chrome and the installed Chrome must have the same major version.
  A browser that is another version is refused before it loads a page
  (`StealthError::Impersonate`).
- A launched browser lives until the session leaves the browser mode or is
  closed. There is no idle timeout and no deadline in the session.
- It runs the same fetch loop as `BrowserScraper`, with plain HTTP as the first
  move. A matching handshake is one signal among many, and a session does not
  make automation permitted by a site's terms. See [Limits](/stealth/limits#limits).

</Warning>

## What was run

<Check>

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

</Check>
