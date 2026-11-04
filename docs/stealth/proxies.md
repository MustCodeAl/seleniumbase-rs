---
title: "Proxies and network"
sidebarTitle: "Proxies and network"
---

What a site sees of the network has to fit the fingerprint: an address in the
country the browser claims, no leak of the machine's own addresses through
WebRTC, and a proxy login that only the proxy ever receives.
[How the network works](/concepts/network) explains proxies, WebRTC and the
TLS fingerprint; this page is the settings.

## A proxy for the browser

<Tabs>

<Tab title="BaseCase and the CLI" icon="flask">

Give a proxy with `--proxy host:port` or `scheme://host:port`, pick at random
from a file with `--proxy-list`, or use a PAC file with `--proxy-pac-url`.

</Tab>

<Tab title="Pure CDP engine" icon="bolt">

The launch options take a proxy string, `SERVER:PORT` or
`USER:PASS@SERVER:PORT`, optionally with a scheme such as `socks5://`.
`Proxy::parse` reads the same text. Each isolated context can have a proxy of
its own:

```rust,ignore src/main.rs
use seleniumbase_rs::sb_cdp::{Browser, ContextOptions, LaunchOptions, Proxy};

// One proxy for the whole browser.
let browser = Browser::launch(
    LaunchOptions::builder()
        .proxy("alice:s3cret@proxy.example.com:8080")
        .build()?,
)
.await?;

// Or a different proxy for each isolated context.
let options = ContextOptions::new()
    .proxy(Proxy::parse("bob:hunter2@other.example.com:1080")?)
    .bypass("*.internal.example.com");
let context = browser.new_context_with(options).await?;
let page = context.new_page_at("https://example.com").await?;
```

</Tab>

<Tab title="A fingerprint" icon="fingerprint">

A fingerprint can name a proxy too. It becomes the `--proxy-server` launch
flag ([Evasions and launch flags](/stealth/evasions#launch-flags)); a launch
setting made before `.fingerprint(..)` wins over it.

</Tab>

</Tabs>

## Proxy passwords

Chrome ignores a user name and password written into a proxy address. The pure
CDP engine answers the proxy's authentication challenge itself, tab by tab, so
different tabs can sit behind different proxies, each with its own
credentials.

- The credentials go to the proxy only: a password prompt from a website (a
  `401`) is never given the proxy account.
- A fingerprint's proxy password is never sent as a `Proxy-Authorization`
  header, which would give it to every site the browser visits.
- `Proxy` hides the password from its `Debug` and `Display` output, and the
  launch options keep it off the command line.
- A WebSocket's `407` never reaches the engine, so a WebSocket passes a
  password-protected proxy only with credentials Chrome already holds from an
  earlier request in the same context.

## WebRTC

A page can open an `RTCPeerConnection` without asking, and the candidates it
gathers describe the machine's network. That is how a proxy or VPN gets
bypassed.

- **The shield** (`LaunchOptionsBuilder::shield_webrtc(true)`, or
  `page.shield_webrtc()` for a tab that is already open) makes every
  `RTCPeerConnection` relay-only and drops its STUN servers. Nothing is
  gathered. Pages that need a WebRTC call to connect will not, unless they
  bring a TURN relay.
- **Chrome's own flag**: `webrtc_policy(WebRtcPolicy::...)` on the launch
  options, or a fingerprint's WebRTC policy, which becomes
  `--force-webrtc-ip-handling-policy` only when the fingerprint masks WebRTC.
  None of its values removes the `.local` candidates, so use the shield when a
  page should learn nothing.
- **Checking it**: `page.webrtc_report()` gathers candidates the way a tracking
  script would, with no STUN server, and `is_clean()` says whether it found
  none.

[Pure CDP](/tutorials/pure_cdp#keeping-webrtc-from-leaking-addresses) has the
example.

## QUIC

The policy's `quic` setting decides whether Chrome may use QUIC (HTTP/3):
`QuicMode::Disabled` and `QuicMode::ForceHttp2` pass `--disable-quic`,
`QuicMode::Enabled` passes `--enable-quic`, and `Natural` and `Auto` leave
Chrome's default.

## An address that fits the claims

A browser that claims Berlin and connects from Sydney is easy to spot. Choose
the proxy first, then give the fingerprint the time zone, locale and location
that go with its address ([Fingerprints](/stealth/fingerprints)). The stealth
repository's scraper ties the two together; see
[Egress and the profile](/scraping/stealthy/browser-scraper#egress-and-the-profile).

## The wire

The TLS and HTTP/2 handshake is the browser's own. Making a plain HTTP client
or a proxied browser look like a given Chrome on the wire is in
[Wire formats](/stealth/advanced/wire-formats) and
[MitmProxy](/stealth/advanced/mitm-proxy).
