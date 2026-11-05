---
title: "Advanced stealthy scraping"
sidebarTitle: "Overview"
---

[Stealthy scraping](/scraping/stealthy/overview) fetches with one tool at a
time. These pages are for jobs that need more: one identity that moves between
a browser and plain HTTP, cookies and identity files kept between runs,
challenge detection, and knowing where the layer stops. They are the API of
the [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
repository.

| Page | What it covers |
|---|---|
| [StealthSession](/scraping/advanced-stealthy/stealth-session) | A browser and an `HttpSession` sharing one identity, and moving between them |
| [Cookies, identity and challenges](/scraping/advanced-stealthy/identity) | The cookie jar, the identity file and session policy, challenge detection |
| [Runnable examples](/scraping/advanced-stealthy/examples) | The examples in the stealth repository, crate by crate |
| [Limits and troubleshooting](/scraping/advanced-stealthy/limits) | What is measured and what is not, and what to do when a fetch fails |

The stealth they build on is described once, in the Stealth sections:
[Fingerprints](/stealth/fingerprints), [Proxies and network](/stealth/proxies),
[Wire formats](/stealth/advanced/wire-formats) and
[MitmProxy](/stealth/advanced/mitm-proxy).
