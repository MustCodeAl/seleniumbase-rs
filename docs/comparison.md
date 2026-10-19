---
title: "How it compares"
---

Where `seleniumbase-rs` fits among the tools people use to automate a browser,
and where each of them is the better choice. This page is written from general
knowledge of these projects and from this crate's own state; check a competitor's
current documentation before you decide, because every one of them keeps moving.

<Note>

Nothing here is a benchmark: there is no measured speed comparison with any other
tool (see [Benchmarks](/benchmarks) for what this crate itself costs).

</Note>

<Visibility for="agents">

Summary for AI exports: choose seleniumbase-rs for a Rust test framework with the SeleniumBase API, or for driverless Chrome automation in Rust with locators that wait. Choose Playwright for Chromium, Firefox and WebKit from one API with a trace viewer, Selenium (WebDriver) for the most mature and widest-supported standard, Python SeleniumBase for a Python suite, `chromiumoxide` for a raw Rust DevTools Protocol library and `thirtyfour` for a low-level Rust WebDriver client. This crate is young (`0.1.0`, not on crates.io) and the page contains no benchmark comparison.

</Visibility>

## The short version

| If you need... | Reach for |
|---|---|
| A test framework in Rust with the SeleniumBase API | `seleniumbase-rs` |
| Chrome automation in Rust without a driver, with locators that wait | `seleniumbase-rs` (`sb_cdp`) |
| An MCP server so an AI agent can drive and debug a browser | `seleniumbase-rs`, or the Python `seleniumbase-mcp` |
| Chromium, Firefox and WebKit from one API, with a trace viewer | Playwright |
| The most mature, widest-supported automation standard | Selenium (WebDriver) |
| A Python suite that already uses SeleniumBase | Python SeleniumBase |
| A raw Rust DevTools Protocol library | `chromiumoxide` or a similar crate |
| A low-level Rust WebDriver client | `thirtyfour` (this crate builds on it) |

## Against the main alternatives

### Python SeleniumBase

This crate is its port, so the comparison is about language and age. Python has
the largest example library, the most users, a mature plugin ecosystem and the
fastest path to a first test. The Rust port is new (`0.1.0`, unreleased), has
every upstream API recorded in [the parity table](/parity) but with the
`composed` and `not-applicable` entries not yet independently reviewed, and
cannot run Python plugins. In exchange it has compile-time checks, a single native
binary, a driver-free engine, a browser pool, request interception and agent
tooling. Choose Python if your team writes Python; choose the port if your
harness belongs in Rust.

### Playwright

Playwright is the benchmark for developer experience: auto-waiting, codegen, a
trace viewer, bundled browsers, and Chromium, Firefox and WebKit from one API,
with official bindings for JavaScript/TypeScript, Python, Java and .NET. It has no
official Rust binding. This crate's <Tooltip tip="Chrome DevTools Protocol: the engine talks to Chrome directly, with no WebDriver.">pure CDP</Tooltip> engine is Chrome and Chromium only
and has no trace viewer, but it gives a Rust-native, driver-free API with retrying
locators and expectations, and a `playwright` feature that offers a
Playwright-compatible mode through a community Rust implementation. If you need
WebKit or Firefox coverage or the trace viewer, use Playwright.

### Selenium and thirtyfour

Selenium is the long-standing standard: every major browser, Selenium Grid, huge
documentation and community. `thirtyfour` is a good Rust WebDriver client. This
crate's WebDriver engine is built on it and adds the SeleniumBase layer on top
(waits before actions, `BaseCase`, recorder, reports, stealth). Use plain
`thirtyfour` when you want a thin client and full control; use this crate when you
want the batteries.

### Puppeteer, `chromiumoxide` and other CDP libraries

These talk to Chrome over the DevTools Protocol, as the `sb_cdp` engine does.
Puppeteer is the Node reference; `chromiumoxide` is a Rust crate with generated
protocol bindings and a broad surface. `sb_cdp` is a smaller, hand-written client
focused on testing: locators that re-find and wait, retrying expectations,
trusted input, a pool of isolated contexts and a mock browser for tests without
Chrome. If you need the whole protocol surface, a generated binding is the better
base.

### Cypress, WebdriverIO and other JavaScript tools

They suit frontend teams that live in JavaScript and want tight integration with
their build tooling. This crate is not a replacement for that workflow; it is
for suites that belong in Rust or that ship as a native binary.

### Anti-detection tools (undetected-chromedriver, nodriver, Botasaurus and similar)

These Python and Node tools focus on not looking automated. This crate has the
same family of features from SeleniumBase (UC mode, fingerprints, masking
providers, human-paced input) and a driver-free engine that leaves no webdriver
markers. TLS (JA3/JA4) and HTTP/2 impersonation, built from measured data for
one browser and system (Chrome 155 on macOS), with an HTTP session that continues
a browser's session (`HttpSession`), a loopback proxy for a real Chrome
(`MitmProxy`) and a session that moves between the two (`StealthSession`), is a
separate, experimental project on top of this crate:
[seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
(see [Stealth Scraping](/tutorials/stealth_scraping)). No tool, this one
included, can promise to pass a given detection service.

### HTTP impersonation clients (curl-impersonate, wreq and similar)

They make a plain HTTP client look like a browser at the TLS level, without a
browser. This crate does not; the companion crate `seleniumbase-rs-mimic`
(in seleniumbase-rs-stealth) wraps wreq on BoringSSL with a measured Chrome 155
profile for macOS, custom JA3/JA4/Akamai inputs, and a way to solve a challenge
in Chrome and continue over HTTP. It is young and measured on macOS with Chrome
155 only. If TLS fidelity for another browser or system is what you need today,
use one of those libraries directly.

## At a glance

| | `seleniumbase-rs` | Python SeleniumBase | Playwright | Selenium / `thirtyfour` | `chromiumoxide` |
|---|---|---|---|---|---|
| Language | Rust | Python | JS/TS, Python, Java, .NET | many / Rust | Rust |
| Browsers | Chrome/Chromium (CDP); Chrome, Firefox, Safari via WebDriver | WebDriver browsers | Chromium, Firefox, WebKit | all WebDriver browsers | Chromium |
| Driver needed | no (pure CDP) or yes (WebDriver) | yes | no | yes | no |
| Test framework included | yes (`BaseCase`, recorder, reports) | yes | yes | no | no |
| Anti-detection | UC mode, fingerprints, humanised input | yes | limited by design | no | no |
| AI-agent (MCP) tooling | yes, with debug and test-export tools | yes | via separate servers | via separate servers | no |
| Distribution | one native binary | Python package | Node package | varies | Rust crate |
| Maturity and community | new, small | large | large | very large | moderate |

## Where this crate is behind

Be honest with yourself about these before choosing it:

<Warning>

- It is young: `0.1.0`, not on crates.io, with most behaviour covered by unit
  tests and a mock browser rather than long use against real sites.
- The parity table is a claim that has not been independently reviewed.
- Chrome-centred: no WebKit, and no bundled browser downloads beyond the
  chromedriver.
- No trace viewer, no cross-language ecosystem, a small community.
- TLS and HTTP/2 impersonation is not in this crate: it is the experimental
  seleniumbase-rs-stealth project, measured for one browser and system (Chrome
  155 on macOS) and tested against local servers and a local Chrome, not against
  live services.

</Warning>

## Next steps

<Columns cols={2}>

<Card color="#1a85d4" title="Why Rust?" icon="gear" href="/why-rust">

When the crate is the right choice, and when another stack fits better.

</Card>

<Card color="#199f43" title="Working with seleniumbase-rs" icon="rocket" href="/tutorials/getting_started">

Write a first test with `BaseCase`.

</Card>

<Card color="#d47628" title="Pure CDP" icon="bolt" href="/tutorials/pure_cdp">

Chrome without a WebDriver.

</Card>

<Card color="#a631be" title="Parity table" icon="table" href="/parity">

The recorded status of every upstream API.

</Card>

<Card color="#1ca1c7" title="Benchmarks" icon="gauge-high" href="/benchmarks">

What this crate itself costs.

</Card>

<Card color="#d32a61" title="Stealth scraping" icon="user-secret" href="/tutorials/stealth_scraping">

The separate, experimental seleniumbase-rs-stealth project.

</Card>

</Columns>

