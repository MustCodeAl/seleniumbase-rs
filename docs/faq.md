---
title: "Frequently asked questions"
---

# Frequently asked questions

Short answers, with a pointer to the page that has the detail.

## Choosing and getting started

<AccordionGroup>

<Accordion title="Is this the same as Python SeleniumBase?">

It is a Rust port of it (tracking 4.55.2) and of its MCP server. Every upstream
API has a recorded status in [the parity table](/parity). It also adds things
Python does not have; see [Why Rust?](/why-rust).

</Accordion>

<Accordion title="Do I need Rust to use it?">

To write tests in Rust, yes. The `sbrs` CLI and the MCP server are single
binaries you can use without writing Rust; `sbrs` can also convert Python
SeleniumBase scripts.

</Accordion>

<Accordion title="Which engine should I use?">

Start with the pure CDP engine (`sb_cdp`): it drives Chrome directly, so there is
no chromedriver to match. Use the WebDriver engine (`BaseCase`) for Selenium Grid,
Firefox, or an existing SeleniumBase-style suite. See
[Performance](/performance#choosing-the-engine).

</Accordion>

<Accordion title="Is it on crates.io?">

No. Build it from the repository ([Building](/building)) or add it as a git
dependency.

</Accordion>

<Accordion title="Does it run on Windows and Linux?">

CI builds and tests on Linux, macOS and Windows. Development happens on macOS.
See [Operating systems](/platforms).

</Accordion>

<Accordion title="How does it compare with Playwright, Selenium or Puppeteer?">

See [How it compares](/comparison).

</Accordion>

</AccordionGroup>

## Using it

<AccordionGroup>

<Accordion title="Is it faster than Python SeleniumBase?">

Not for end-to-end tests: Chrome, the network and the page dominate. It has low
overhead of its own ([Benchmarks](/benchmarks)), but there is no measured
comparison with Python. See [Performance](/performance).

</Accordion>

<Accordion title="A test fails with a wait timeout. What now?">

Read the hint in the error: it names the selector and a likely mistake. Use
`wait_for_element_visible`/`wait_for_text` instead of sleeps. In an AI-agent
session, `diagnose_selector` and `inspect_page` show what the page really has.
See [Reliability](/reliability).

</Accordion>

<Accordion title="Why does sbrs exit with different statuses?">

The exit status follows the error's category (a bad selector differs from a
missing browser), so a CI script can tell them apart.

</Accordion>

<Accordion title="Can an AI agent write my tests?">

Yes, with the MCP servers and `export_session_as_test`; the result is a draft you
review. See [Vibe coding](/tutorials/vibe_coding).

</Accordion>

</AccordionGroup>

## Browsers and drivers

<AccordionGroup>

<Accordion title='"chromedriver keeps crashing" or "session not created".'>

The chromedriver and Chrome major versions differ. Run `sbrs doctor`, then
`sbrs install`. The pure CDP engine avoids the problem. See
[Common problems](/help/common_problems).

</Accordion>

<Accordion title="Can it drive Firefox or Safari?">

Firefox through the WebDriver engine (there are typed Firefox preferences); Safari
launch options exist for the WebDriver engine. The pure CDP engine is Chrome and
Chromium only. A `playwright` feature gives a Playwright-compatible mode.

</Accordion>

<Accordion title="Does it work headless?">

Yes: `SB_HEADLESS=true`. Always use it in containers and CI.

</Accordion>

</AccordionGroup>

## Stealth and scraping

<AccordionGroup>

<Accordion title="Does it get past bot detection?">

It reduces the obvious signs (no webdriver markers in the pure CDP engine,
coherent fingerprints, human-paced input), but there is no guarantee against any
particular service, and the randomised identities have not been tested against
live detection. TLS (JA3/JA4) and HTTP/2 impersonation for one measured browser
and system (Chrome 155 on macOS), with an HTTP session, a proxy that applies it to
a real Chrome, and a session that moves between the two, is the separate,
experimental [seleniumbase-rs-stealth](https://github.com/MustCodeAl/seleniumbase-rs-stealth)
project (it left this crate in 0.3.0). It was tested against local servers and a
local Chrome, not live services. See
[the stealth tutorial](/tutorials/fingerprint_stealth) and
[Stealth scraping](/stealth/overview).

</Accordion>

<Accordion title="Is scraping with it allowed?">

That depends on the site's terms and the law where you are. The library does not
make that decision for you.

</Accordion>

</AccordionGroup>

## Contributing and project status

<AccordionGroup>

<Accordion title="How complete is the port?">

Every upstream API has a status (888 implemented, 383 composed, 94
not-applicable, none planned), and most behaviour is tested against a mock or
unit tests rather than a real browser. An audit of the entries marked
implemented found 26 that did less than the Python method, and a second audit of
the composed and not-applicable entries found 34 more that said more than the
Rust does; they were fixed or re-marked, and the `planned` ones are listed in
[the parity table](/parity). The project README has a "Project status"
section.

</Accordion>

<Accordion title="How do I contribute?">

Read `CONTRIBUTING.md` and `CLAUDE.md`. Run the checks listed in
[Building](/building) before opening a pull request.

</Accordion>

</AccordionGroup>
