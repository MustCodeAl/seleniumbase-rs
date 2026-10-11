# Frequently asked questions

Short answers, with a pointer to the page that has the detail.

## Choosing and getting started

**Is this the same as Python SeleniumBase?**
It is a Rust port of it (tracking 4.55.2) and of its MCP server. Every upstream
API has a recorded status in [the parity table](parity.md). It also adds things
Python does not have; see [Why Rust?](why-rust.md).

**Do I need Rust to use it?**
To write tests in Rust, yes. The `sbase` CLI and the MCP server are single
binaries you can use without writing Rust; `sbase` can also convert Python
SeleniumBase scripts.

**Which engine should I use?**
Start with the pure CDP engine (`sb_cdp`): it drives Chrome directly, so there is
no chromedriver to match. Use the WebDriver engine (`BaseCase`) for Selenium Grid,
Firefox, or an existing SeleniumBase-style suite. See
[Performance](performance.md#choosing-the-engine).

**Is it on crates.io?**
No. Build it from the repository ([Building](building.md)) or add it as a git
dependency.

**Does it run on Windows and Linux?**
CI builds and tests on Linux, macOS and Windows. Development happens on macOS.
See [Operating systems](platforms.md).

**How does it compare with Playwright, Selenium or Puppeteer?**
See [How it compares](comparison.md).

## Using it

**Is it faster than Python SeleniumBase?**
Not for end-to-end tests: Chrome, the network and the page dominate. It has low
overhead of its own ([Benchmarks](benchmarks.md)), but there is no measured
comparison with Python. See [Performance](performance.md).

**A test fails with a wait timeout. What now?**
Read the hint in the error: it names the selector and a likely mistake. Use
`wait_for_element_visible`/`wait_for_text` instead of sleeps. In an AI-agent
session, `diagnose_selector` and `inspect_page` show what the page really has.
See [Reliability](reliability.md).

**Why does `sbase` exit with different statuses?**
The exit status follows the error's category (a bad selector differs from a
missing browser), so a CI script can tell them apart.

**Can an AI agent write my tests?**
Yes, with the MCP servers and `export_session_as_test`; the result is a draft you
review. See [Vibe coding](tutorials/vibe_coding.md).

## Browsers and drivers

**"chromedriver keeps crashing" or "session not created".**
The chromedriver and Chrome major versions differ. Run `sbase doctor`, then
`sbase install`. The pure CDP engine avoids the problem. See
[Common problems](help/common_problems.md).

**Can it drive Firefox or Safari?**
Firefox through the WebDriver engine (there are typed Firefox preferences); Safari
launch options exist for the WebDriver engine. The pure CDP engine is Chrome and
Chromium only. A `playwright` feature gives a Playwright-compatible mode.

**Does it work headless?**
Yes: `SB_HEADLESS=true`. Always use it in containers and CI.

## Stealth and scraping

**Does it get past bot detection?**
It reduces the obvious signs (no webdriver markers in the pure CDP engine,
coherent fingerprints, human-paced input), but there is no guarantee against any
particular service, and the randomised identities have not been tested against
live detection. A TLS (JA3/JA4) and HTTP/2 impersonation client exists behind the
`impersonate` feature for one measured browser and system (Chrome 155 on macOS),
with an HTTP session, a proxy that applies it to a real Chrome, and a session that
moves between the two. They were tested against local servers and a local Chrome,
not live services. See
[the stealth tutorial](tutorials/fingerprint_stealth.md) and
[Stealth Scraping](tutorials/stealth_scraping.md).

**Is scraping with it allowed?**
That depends on the site's terms and the law where you are. The library does not
make that decision for you.

## Contributing and project status

**How complete is the port?**
Every upstream API has a status (862 implemented, 374 composed, 112
not-applicable, 17 planned), but the composed and not-applicable entries have
not been independently reviewed, and most behaviour is tested against a mock or
unit tests rather than a real browser. An audit of the entries marked
implemented found 26 that did less than the Python method; they were fixed or
re-marked, and the `planned` ones are listed in [the parity table](parity.md). The project README has a "Project
status" section.

**How do I contribute?**
Read `CONTRIBUTING.md` and `CLAUDE.md`. Run the checks listed in
[Building](building.md) before opening a pull request.
