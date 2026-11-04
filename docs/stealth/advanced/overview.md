---
title: "Advanced stealth"
sidebarTitle: "Overview"
---

The [Stealth](/stealth/introduction) pages cover what a page can read. These
go further: they change what the browser is built from, or what the network
sees before a page loads. Each one takes more care than a fingerprint, and the
last two come from the separate
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
repository.

| Page | What it does | Where it lives |
|---|---|---|
| [Evasion providers](/stealth/advanced/providers) | Add your own page script to the registry the evasions are assembled from | this crate |
| [Binary patching](/stealth/advanced/binary-patching) | Patch the chromedriver and Chrome binaries; engine-spoofing arguments | this crate |
| [Stealth with Playwright mode](/stealth/advanced/playwright) | Give a rustwright session a fingerprint | `seleniumbase-rs-playwright` |
| [Wire formats](/stealth/advanced/wire-formats) | TLS and HTTP/2 fingerprints measured from Chrome, and a client that presents them | seleniumbase-rs-stealth |
| [MitmProxy](/stealth/advanced/mitm-proxy) | Carry a browser's traffic through a proxy that presents the measured handshake | seleniumbase-rs-stealth |

<Tip>

Most detection is answered by a coherent [fingerprint](/stealth/fingerprints),
the right engine ([CDP Mode](/stealth/cdp-mode) or [UC mode](/tutorials/uc_mode))
and an address that fits ([Proxies and network](/stealth/proxies)). Check
those, and [Troubleshooting](/stealth/troubleshooting), before reaching for
these pages.

</Tip>
