---
title: "How network identity works"
sidebarTitle: "Network identity"
description: "What a site learns from the connection itself: IP address, proxies, WebRTC leaks, and TLS and HTTP/2 fingerprints."
---

<Info>

**In short:** before any page script runs, the connection already tells a site
where you are and what software is talking. A proxy changes the IP address; the
WebRTC policy keeps the real one from leaking; and a real Chrome sends Chrome's
TLS and HTTP/2 fingerprint, which a plain HTTP client does not.

</Info>

## What the connection reveals

| Signal | What the site learns | How to change it |
|---|---|---|
| IP address | Country, network owner, whether it is a datacenter | A proxy |
| WebRTC | Possibly the real IP, even behind a proxy | The WebRTC policy |
| TLS ClientHello | Which TLS library made the connection (<Tooltip tip="Hashes of the TLS handshake: versions, ciphers, extensions and their order">JA3, JA4</Tooltip>) | Use a real browser, or the stealth crates |
| HTTP/2 settings and frame order | Which HTTP/2 stack is talking (the Akamai fingerprint) | The same |
| Header order and values | Browser family and version | The same |

## Proxies

Give a proxy with `--proxy host:port` or `scheme://host:port`, pick at random
from a file with `--proxy-list`, or use a PAC file with `--proxy-pac-url`.

<Note>

Chrome ignores a user name and password written into a proxy address. The pure
CDP engine answers the proxy's authentication challenge itself, tab by tab, so
different tabs can sit behind different proxies, each with its own credentials.

</Note>

## WebRTC leaks

WebRTC can open direct UDP connections that bypass the proxy and expose the real
IP address. The `webrtc_policy` setting of a `StealthPolicy` picks what WebRTC
may use:

| `WebRtcPolicy` | Effect |
|---|---|
| `DisableNonProxiedUdp` (default) | No UDP outside the proxy: the real IP stays hidden. |
| `PublicInterfaceOnly` | Only the public interface is used; local addresses stay hidden. |
| `PublicAndPrivateInterfaces` | Public and private interfaces are both used. |

## TLS and HTTP/2 fingerprints

Every client opens a TLS connection with a ClientHello whose versions, ciphers,
extensions and their order identify the library. HTTP/2 adds its settings and
the order of its first frames. A real Chrome sends Chrome's values, so a browser
driven by seleniumbase-rs passes this check on its own.

A plain HTTP client does not. When requests are made outside the browser (to
continue a browser session faster), the experimental
[stealth crates](/stealth/overview) reproduce Chrome's fingerprint:

<Columns cols={2}>

<Card title="JA3 and JA4" icon="fingerprint" color="#1a85d4" href="/stealth/api/codec-ja4">
  How the TLS fingerprints are computed.
</Card>

<Card title="Akamai HTTP/2" icon="network-wired" color="#199f43" href="/stealth/api/codec-akamai">
  The HTTP/2 fingerprint string.
</Card>

<Card title="Wire formats" icon="code" color="#d47628" href="/stealth/wire-formats">
  Measured fingerprints and impersonating clients.
</Card>

<Card title="Limits" icon="triangle-exclamation" color="#d52c36" href="/stealth/limits">
  What is measured, and what is not.
</Card>

</Columns>

<Warning>

A clean fingerprint does not fix a bad IP: a datacenter address is still a
datacenter address. Keep the proxy's country, the time zone and the locale in
agreement. See [Detection and spoofing](/concepts/detection).

</Warning>
