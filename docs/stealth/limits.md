---
title: "Limits and Troubleshooting"
---

# Limits and Troubleshooting

## Limits

<Warning>

State these plainly to yourself before you build on any of this.

</Warning>

- **macOS and Chrome 155 only.** Exactly one browser and system pair is measured:
  Chrome 155.0.8059.40 on macOS. Every other pair is refused with
  `NotMeasured`. When Chrome updates, the profile does not follow until you
  re-measure.
- **Known gaps between Chrome's handshake and ours.** The pinned BoringSSL and
  wreq cannot send three things Chrome 155 sends:
  - `TrustAnchorsExtension`: the `trust_anchors` extension (`0xCA34`). It needs a
    newer `btls` and `wreq` than the exact pins. Our JA4 therefore differs from
    Chrome's in the extension count and the `c` hash.
  - `MlDsaSignatureAlgorithms`: the ML-DSA signature schemes (`0x0904` to
    `0x0906`) are missing from our signature algorithms, which changes the `c`
    hash too.
  - A **GREASE signature algorithm**: Chrome writes one first, and the vendored
    BoringSSL writes none. This one is not a `KnownGap` variant. JA3 and JA4 strip
    GREASE, so only the peetprint shows it. A test pins the difference and fails
    when a newer BoringSSL closes it.

  The first two are recorded in the measured file's `provenance.known_gaps`; the
  tests assert that the difference between Chrome and our client is exactly
  these, and fail when one disappears.

  `HttpSession` and `MitmProxy` send the handshake of this client, so the gaps
  apply to them. Behind the `MitmProxy` the origin sees our handshake, gaps
  included, and not Chrome's own, which `BrowserScraper` leaves with unchanged.
  That is the price of one wire identity in both modes of a `StealthSession`.
- **No HTTP/3.** wreq has none, and Chrome behind the proxy has QUIC switched off.
- **WebSocket passes the `MitmProxy` as `wss://` and `ws://`** (the latter in
  plain text, so with no hello to impersonate); every other upgrade is refused.
  It needs the proxy's credentials in Chrome first: `configure_launch` loads the
  priming page at the launch, but a browser context created later has to load
  `mitm.prime_url()` itself (see [the priming page](mitm-proxy.md#the-priming-page)).
  `HttpSession` opens no WebSocket.
- **The first request to an HTTP/1.1-only origin carries an HTTP/2 `priority`
  header**, in `HttpSession` and in `MitmProxy`, because the protocol is known
  only after the connection opens. Later requests to that origin do not carry it.
- **Headless, one system.** The tests that drive Chrome through `MitmProxy`,
  `HttpSession::from_page` and `StealthSession` launch it headless on macOS with
  Chrome 155; none drives them headed. The measured tables were captured from a
  headed Chrome, and headless and headed Chrome differ.
- **`HttpSession` follows redirects where they point,** a private address
  included. Filter the URLs before the session sees them if they are not yours.
- **Tested against local captures and servers, not against live services.** The
  wire layer is tested against committed captures and, in tests marked
  `#[ignore]`, against a local Chrome. The `BrowserScraper` flows are tested with
  a scripted browser and with ignored tests that drive a local Chrome.
  `HttpSession`, `MitmProxy` and `StealthSession` are tested against loopback TLS
  sites and a challenge page written for the tests, with a scripted browser, and
  in ignored tests with the local Chrome. Nothing here was checked against a live
  bot-protection service. The identity file's permissions check is Unix only.
- **No guarantee against any detection service.** A matching handshake is one
  signal among many: the IP address, the behaviour, the account and the page's
  own checks all count. Nothing here promises to pass a given service, and
  services change.
- **Scrape within the rules.** Follow each site's terms of service and `robots`
  rules, respect rate limits, and obey the law where you and the site are. A
  matching fingerprint does not make automation permitted. The library does not
  decide that for you.

## Troubleshooting

<AccordionGroup>

<Accordion title="`NotMeasured { kind, os }`">

**Likely cause.** The browser and system pair has no capture.

**Fix.** Run the capture tools on that system, or use the measured one.

</Accordion>

<Accordion title="`VersionMismatch { browser, measured }`">

**Likely cause.** The identity's user agent claims another Chrome major than the measured profile.

**Fix.** Build the identity with `Fingerprint::randomized_with_major(os, seed, measured_major)`, or re-measure.

</Accordion>

<Accordion title="`UnsupportedExtension`, `UnsupportedSigalg`, `UnsupportedSetting`">

**Likely cause.** The input asks for something BoringSSL or wreq cannot send.

**Fix.** Remove it. The error names the id.

</Accordion>

<Accordion title="The build fails while compiling BoringSSL">

**Likely cause.** `cmake` or a C/C++ toolchain is missing.

**Fix.** Install them, or drop the `mimic` crate.

</Accordion>

<Accordion title="`IdentityFileError::Permissions`">

**Likely cause.** Group or others can read the identity file.

**Fix.** `chmod 600` it, or delete it and make a new one.

</Accordion>

<Accordion title="`ScrapeError::CoolingDown`">

**Likely cause.** The site answered with a block or rate limit and is being left alone.

**Fix.** Wait until the time in the error (seconds since the Unix epoch), or give the scraper another egress.

</Accordion>

<Accordion title="`ScrapeError::Unsolved`">

**Likely cause.** A challenge was still there after the attempts allowed.

**Fix.** Raise `with_max_challenge_attempts`, add egresses, or accept that this page needs a person.

</Accordion>

<Accordion title="`ScrapeError::NoEgressAvailable`">

**Likely cause.** A rotation was asked for and the pool has nothing else in rotation.

**Fix.** Add proxies with `with_geo_proxies`.

</Accordion>

<Accordion title="`HttpError::VersionMismatch`, `StealthError::Impersonate`">

**Likely cause.** The page, the identity or the installed Chrome is another major than the measured one.

**Fix.** Update or downgrade Chrome to the measured major, or run the capture tools again.

</Accordion>

<Accordion title="`HttpError::Impersonate` for a page">

**Likely cause.** The page announces itself as `HeadlessChrome`, is not Chrome, or runs on a system that was not measured.

**Fix.** Give the browser a fingerprint or run it headed; use the measured system.

</Accordion>

<Accordion title="`HttpError::ReservedHeader`">

**Likely cause.** A request set a header the profile owns (`User-Agent`, `Sec-CH-UA*`, `Accept-Language`, `Accept-Encoding`, `Cookie`, `priority`).

**Fix.** Remove it. Change the identity instead.

</Accordion>

<Accordion title="`HttpError::RedirectRefused`, `TooManyRedirects`">

**Likely cause.** A hop led somewhere the session refuses, or past the limit.

**Fix.** Request the final URL, or raise `max_redirects`.

</Accordion>

<Accordion title="`HttpError::Identity(EgressMismatch)`">

**Likely cause.** The identity or the page belongs to another egress than the one passed.

**Fix.** Pass the egress the cookies were earned through.

</Accordion>

<Accordion title="Chrome behind `MitmProxy` shows a `502` page">

**Likely cause.** The proxy refused the request or the upstream failed (an untrusted certificate, a non-WebSocket upgrade, a version mismatch).

**Fix.** Check `mitm.stats()` and the log; `trust_upstream_root` for a private site.

</Accordion>

</AccordionGroup>

---

Verified against commit `4184e74` and the re-exports added after it, by reading
`crates/codec/src/**`, `crates/scrape/src/**`, `crates/impersonate/src/**`, `examples/capture_*.rs`,
`crates/codec/src/measured/chrome-155-macos.json` and the tests that pin the behaviour
described here. The code samples were checked against the signatures in those
files; they were not compiled or run for this check. The real-Chrome tests named
here are `#[ignore]`d and were not run for this check.
