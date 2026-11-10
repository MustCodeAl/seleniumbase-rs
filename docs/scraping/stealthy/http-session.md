---
title: "HttpSession"
---

Examples importing `seleniumbase_rs_stealth::wire` require the facade's
`wire` feature. See the [single-dependency setup](/scraping/stealthy/overview#which-crate-to-depend-on);
this feature builds BoringSSL and needs cmake and a C/C++ toolchain.

<Badge color="orange">Experimental</Badge>

`wire::http_session::HttpSession` sends plain HTTP requests that look like
the Chrome a browser page came from. A browser spends a second or more on a page;
a request spends milliseconds. Once a page has earned its cookies (a login, a
passed check), an `HttpSession` carries them on: the measured TLS `ClientHello`
and HTTP/2 start, the header names, order and spelling of that Chrome on that
system, and the user agent, client hints and `Accept-Language` of the page or
identity. A browser or system that was not measured is refused, not
approximated.

<Visibility for="agents">

Summary for AI exports: `HttpSession` (crate `seleniumbase-rs-wire`) sends plain HTTP requests that carry the cookies, the measured TLS and HTTP/2 start, the header names and order, the user agent, client hints and `Accept-Language` of a Chrome. Build one with `HttpSession::from_page` or `HttpSession::from_identity`; a browser or system that was not measured is refused. It does not run JavaScript or solve a challenge.

</Visibility>

```rust
use seleniumbase_rs_stealth::wire::{HttpSessionOptions, HttpSession};
use seleniumbase_rs_stealth::sb_cdp::Page;

async fn demo(page: Page) -> Result<(), Box<dyn std::error::Error>> {
    let session = HttpSession::from_page(&page, None, HttpSessionOptions::new()).await?;
    let reply = session.get("https://example.com/api/orders").await?;
    let orders: serde_json::Value = reply.json()?;
    println!("{orders}");
    // Write the cookies that responses set back into the browser.
    session.sync_cookies_back(&page).await?;
    Ok(())
}
```

## Making one

- `HttpSession::from_page(&page, egress, options)` (async) copies the cookies of
  the page's browser profile into the jar, reads `navigator.userAgent` and
  `navigator.languages` from the page, and follows the route its traffic takes:
  the proxy of its browser context. `egress` is the exit the page leaves
  through; `None` takes the context's own proxy or goes direct. A browser-wide
  proxy, such as a MITM proxy the browser was launched with, is not visible to a
  page, so say so with `egress`.
- `HttpSession::from_identity(&identity, egress, options)` builds one from a
  [`Identity`](/scraping/advanced-stealthy/identity#identity-session-policy-and-the-identity-file): its
  fingerprint becomes the user agent, hints and languages, and its cookies fill
  the jar.

Both refuse, before any request, what would contradict the measured handshake:

<Warning>

- A Chrome of another major version than the measured one:
  `HttpError::VersionMismatch { browser, measured }`. For a page, both the user
  agent and `Browser.getVersion` are checked.
- A browser that is not Chrome (Edge, Opera, Firefox and the like), a Chrome
  whose user agent says `HeadlessChrome`, or a system that was not measured:
  `HttpError::WireFormat(..)`. A headless Chrome that was given no fingerprint
  announces itself, so give it one or run it headed.
- An identity of another egress than the one passed, or a page whose context
  proxy is another than `egress`: `HttpError::Identity(..)`.

</Warning>

## Requests

`session.get(url)` sends a `GET`. `session.request(method, url)` returns an
`HttpRequest`; nothing is sent until `.send().await`, and a mistake in a setter
is reported there.

```rust
use seleniumbase_rs_stealth::wire::{HttpSession, Method};
use seleniumbase_rs_stealth::codec::header_spec::RequestShape;

async fn demo(session: HttpSession) -> Result<(), Box<dyn std::error::Error>> {
    let reply = session
        .request(Method::POST, "https://example.com/api/search")
        .shape(RequestShape::Fetch)
        .header("x-requested-with", "XMLHttpRequest")
        .json(&serde_json::json!({ "q": "shoes" }))
        .send()
        .await?;
    println!("{}: {}", reply.status(), reply.text()?);
    Ok(())
}
```

- `GET` and `HEAD` carry the headers of a navigation (`RequestShape::Document`),
  every other method those of a script's request (`RequestShape::Fetch`);
  `.shape(..)` chooses. `.header`, `.body`, `.json`, `.form` and `.timeout`
  complete the request. `Method` is `wreq::Method`, re-exported.
- The profile owns `User-Agent`, `Sec-CH-UA*`, `Accept-Language`,
  `Accept-Encoding`, `Cookie` and `priority`, and the connection owns `Host`,
  `Content-Length` and the like. Setting one is `HttpError::ReservedHeader`, not
  a silent overwrite. Any other header replaces the profile's value in place or is
  added. `CONNECT`, `TRACE` and `TRACK` are refused, and so is a URL with
  credentials in it or a scheme other than http and https.
- A status other than 2xx is a reply, not an error. `HttpResponse` has `status()`,
  `headers()` (`ResponseHeaders`), `bytes()`, `text()`, `json::<T>()`, `url()` after
  redirects (and `redacted_url()`), `redirects()`, `protocol()`, `elapsed()` and
  `detect()`.

## What the session does

- **Cookies.** The jar is the RFC 6265 [`CookieJar`](/scraping/advanced-stealthy/identity#the-cookie-jar). The
  session writes the `Cookie` header and reads `Set-Cookie` itself; wreq's own
  store is off. A cookie for a public suffix, for another host, or against the
  `Secure`, `__Host-` and `SameSite` rules is refused as a browser refuses it.
  `sync_cookies_back(&page)` writes only what this session changed into the
  browser and returns how many cookies it wrote or removed; if the browser
  refuses, the changes are kept for the next attempt.
- **Redirects** are followed by hand, up to 10 by default: the cookies of each
  hop are stored and sent per URL, `303` becomes a `GET`, and the headers you set
  are dropped when a hop leaves the origin (`Accept` and `Content-Type` stay). A
  redirect is followed where it points, a private address included, so a session
  that fetches URLs for someone else must filter them before it asks.

**Limits** are `HttpSessionOptions`, with defaults that are safe on the open web:

<ParamField body="timeout" default="30 s">

The whole request, redirects included. Largest accepted: 1 hour.

</ParamField>

<ParamField body="connect_timeout" default="10 s" />

<ParamField body="max_body_bytes" default="64 MiB">

The reply after decompression. Largest accepted: 1 GiB.

</ParamField>

<ParamField body="max_request_bytes" default="64 MiB" />

<ParamField body="max_redirects, follow_redirects" default="10, on">

Largest accepted: 30.

</ParamField>

<ParamField body="rate_limit(calls, window)" default="off">

Per site.

</ParamField>

Also `event_sink`, `trust(CertStore)` for a private root (certificate
verification is never turned off), `resolve(host, addr)`, `bypass(host)`,
`installed_chrome_major(major)` and `clock(..)`. `HttpSessionOptions::validate`
refuses a value out of range before a client exists.

- **Judging replies.** `reply.detect()` runs the [challenge
  detection](/scraping/advanced-stealthy/identity#challenge-detection-and-mitigation). `session.assess(&reply,
  &HttpAttempt, &HttpRules, &identity)` adds what the `MitigationPolicy` and the
  `EscalationPolicy` make of it, as an `HttpVerdict { detection, action, escalation }`;
  `session.fetch(url, &attempt, &rules, &identity)` sends a `GET` and judges it
  in one call (`HttpFetch { reply, verdict }`); `session.preflight(url, &identity,
  state, &rules)` asks, before a request, whether the identity's clearance is
  about to end so that the browser should take the URL first. They are the
  pieces of your own loop; [`StealthSession`](/scraping/advanced-stealthy/stealth-session) is that loop.

  ```rust
  use seleniumbase_rs_stealth::wire::http_session::{HttpAttempt, HttpRules};
  use seleniumbase_rs_stealth::wire::HttpSession;
  use seleniumbase_rs_stealth::scrape::Identity;

  async fn demo(session: HttpSession, identity: Identity) -> Result<(), Box<dyn std::error::Error>> {
      let fetched = session
          .fetch("https://example.com/", &HttpAttempt::default(), &HttpRules::default(), &identity)
          .await?;
      if fetched.verdict.is_clear() {
          println!("{}", fetched.reply.text()?);
      } else {
          println!("{:?}", fetched.verdict);
      }
      Ok(())
  }
  ```
- **Identity.** `export_identity(&base)` puts the session's cookies back into a
  `Identity` and refreshes its clearances. `proxy_route()` is the exit without
  credentials, `cookie_count()` and `wire_format()` read the state. Cloning gives
  another handle to the same session.
- **Secrets.** Cookies, credentials and proxy passwords never reach `Debug`, logs,
  events or errors: URLs are redacted, a failure of the HTTP client is reduced to
  a word, and header values are not quoted.

## What to know

<Info>

**The first request to an origin is written for HTTP/2.** The `priority` header
is a Chrome HTTP/2 header. The session cannot know the protocol before it
connects, so when an origin turns out to speak HTTP/1.1 only, that first
request carries an HTTP/2 `priority` header. The session then remembers the
origin and writes the HTTP/1.1 headers from the second request on. The tests
assert the second half (the second request has no `priority` header,
`the_protocol_an_origin_settled_on_is_remembered` in
`tests/impersonate_http/basics.rs`); the first request's header is what the
module documentation states, and no test asserts it.

</Info>

<Warning>

It does not run JavaScript, solve a challenge, open a WebSocket, or pass
canvas, WebGL or behavioural checks, and it does not make automation permitted
by a site's terms. A matching handshake is one signal among many.

</Warning>

<Check>

**What was run against a real Chrome.** `tests/impersonate_http_chrome.rs`
(`#[ignore]`d, headless Chrome 155 on macOS, against a loopback TLS site)
shows that the real `navigator.userAgent`, `navigator.languages`,
`Browser.getVersion` and `Network.getAllCookies` are what `from_page` reads,
that a host-only cookie set by a response reaches Chrome as host-only, that a
removal reaches it too, and that a page announcing itself as headless is
refused. The rest of `HttpSession` is tested against loopback sites
(`tests/impersonate_http.rs`) and a scripted browser, with no Chrome.

</Check>

## Next steps

`HttpSession`, `HttpSessionOptions`, `HttpResponse` and `HttpError` are re-exported at
`seleniumbase_rs_stealth::wire`, which is what the example above imports. The
module path `wire::http_session::HttpSession` is the same type. The types that
judge a reply and read its headers have one name, in the module and at the
root: `HttpAttempt`, `HttpFetch`, `HttpRules`, `HttpVerdict` and
`ResponseHeaders`.

<Columns cols={2}>

<Card color="#1a85d4" title="Identity" icon="id-card" href="/scraping/advanced-stealthy/identity">

Identity, session policy, the cookie jar and challenge detection.

</Card>

<Card color="#199f43" title="StealthSession" icon="shuffle" href="/scraping/advanced-stealthy/stealth-session">

The loop built from `fetch`, `assess` and `preflight`.

</Card>

<Card color="#1ca1c7" title="MitmProxy" icon="shield" href="/stealth/advanced/mitm-proxy">

The loopback proxy that gives a real Chrome this same handshake.

</Card>

<Card color="#d5a910" title="Limits" icon="triangle-exclamation" href="/scraping/advanced-stealthy/limits">

The handshake gaps this session sends, and what each `HttpError` means.

</Card>

</Columns>
