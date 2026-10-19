---
title: "Glossary"
sidebarTitle: "Glossary"
description: "Short, plain definitions of the terms used across the seleniumbase-rs docs, from WebDriver, CDP and UC mode to fingerprints, plugins and TLS fingerprints, each with a link to the page that explains it."
---

These are the terms you meet in the docs, each with a short definition and a link
to the page that explains it. They are in alphabetical order, and a colored label
shows the area a term belongs to. You can link straight to a term, for example
`/glossary#headless`.

<Visibility for="agents">

Summary for AI exports: an alphabetical glossary of the terms used across the seleniumbase-rs docs. Each entry has an area label (Engine, Stealth, Plugins, CLI, Testing, Network or Project), a one to three sentence definition and a link to the page that explains it. The anchor of a term is its lower-case name with hyphens, for example `/glossary#pure-cdp`.

</Visibility>

| Label | What it covers |
|---|---|
| <Badge color="blue">Engine</Badge> | Browsers, drivers, modes, selectors and the types you call |
| <Badge color="purple">Stealth</Badge> | Fingerprints, evasions and anti-detection |
| <Badge color="orange">Plugins</Badge> | Browser plugins and what they may do |
| <Badge color="gray">CLI</Badge> | The `sbrs` and `sbrs-mcp` programs and MCP |
| <Badge color="green">Testing</Badge> | Assertions, run settings, recording and reports |
| <Badge color="red">Network</Badge> | TLS, HTTP/2, proxies, HAR and what a site sees on the wire |
| <Badge color="yellow">Project</Badge> | The repository, its tools and these docs |

**Jump to a letter**

<Columns cols={4}>

<Column>

[A](#a) · [B](#b) · [C](#c) · [D](#d) · [E](#e)

</Column>

<Column>

[F](#f) · [G](#g) · [H](#h) · [I](#i) · [J](#j)

</Column>

<Column>

[L](#l) · [M](#m) · [N](#n) · [P](#p) · [Q](#q)

</Column>

<Column>

[R](#r) · [S](#s) · [T](#t) · [U](#u) · [W](#w) · [X](#x)

</Column>

</Columns>

## A

### ADR

<Badge color="yellow">Project</Badge>

An Architecture Decision Record: a short page that records one design decision and why it was made. The records here cover the stealth architecture, multiple drivers and Playwright, the stealth scraping layout and the plugin framework.

See [ADR 001: Stealth and Fingerprinting Architecture](/adr/001-stealth-architecture).

### Akamai fingerprint

<Badge color="red">Network</Badge>

A string that describes how a client opens an HTTP/2 connection, written `SETTINGS | WINDOW_UPDATE | PRIORITY | pseudo-header order`. A site can use it, with the TLS handshake and the order of the headers, to tell clients apart.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### ALPN

<Badge color="red">Network</Badge>

Application-Layer Protocol Negotiation: the part of the TLS handshake where client and server agree on HTTP/1.1 or HTTP/2. On the browser's side, `MitmProxy` offers `http/1.1` only.

See [TLS Server and MitmProxy](/stealth/mitm-proxy).

### Audio noise

<Badge color="purple">Stealth</Badge>

A tiny, repeatable change to the audio data a page can read (`AudioBuffer.getChannelData` and similar), so that the audio fingerprint is not the real machine's. With the usual settings it follows the fingerprint's seed, so reading the same audio twice gives the same result.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#the-policy).

### Automation markers

<Badge color="purple">Stealth</Badge>

Signs that a program is driving the browser, such as `navigator.webdriver`, the `cdc_` variables that chromedriver puts in the page and the `enable-automation` launch switch. UC mode masks the ones in the page and leaves out the `enable-automation` switch, binary patching removes the `cdc_` markers from chromedriver itself, and the pure CDP engine has no chromedriver to leave any.

See [Undetected (UC) Mode Guide](/tutorials/uc_mode#what-is-patched-automatically).

## B

### BaseCase

<Badge color="blue">Engine</Badge>

The test case of the WebDriver engine: one object whose methods open pages, find elements by selector, click, type, wait and assert. It is the Rust version of Python SeleniumBase's `BaseCase`.

See [Getting Started with SeleniumBase for Rust](/tutorials/getting_started).

### Behave

<Badge color="green">Testing</Badge>

The crate's Gherkin runner: a small parser and step runner for `.feature` files, like Python's `behave`. You register step definitions, plain or async, and can run only the scenarios with certain tags.

See [Behave / Gherkin Support](/tutorials/behave).

### Binary patching

<Badge color="purple">Stealth</Badge>

Editing a driver or browser executable on disk so that it no longer carries the markers bot checks look for, such as the `cdc_` variables. `ChromedriverPatcher` patches chromedriver and `ChromeBinaryPatcher` patches a cached copy of the browser; UC mode does not patch the driver for you.

See [Binary Patching](/tutorials/binary_patching).

### BrowserConfig

<Badge color="blue">Engine</Badge>

The settings for one browser session: which browser, the driver mode, headless or not, proxy, user agent, fingerprint, plugins and more, set with `with_*` methods. `BrowserConfig::default()` is headless and reads no environment variables.

See [Public items of the crate root](/api/crate-root).

### BrowserContext

<Badge color="blue">Engine</Badge>

A set of tabs in the pure CDP engine that shares no cookies, storage or cache with anything else, like an incognito window. It is much cheaper than a second Chrome process, and each context can have its own proxy.

See [Pure CDP](/tutorials/pure_cdp#isolated-contexts).

### BrowserPool

<Badge color="blue">Engine</Badge>

A pool in the pure CDP engine that shares a few Chrome processes between many concurrent workers. Each `acquire()` gives a lease: its own isolated context and a tab.

See [Pure CDP](/tutorials/pure_cdp#many-workers-the-browser-pool).

### BrowserScraper

<Badge color="purple">Stealth</Badge>

A fetcher in the experimental stealth crates that drives one real Chrome through the pure CDP engine. It loads a URL, detects a [challenge](#challenge), solves it in the browser and rotates the [egress](#egress) or the profile when challenges repeat; it does no TLS spoofing.

See [BrowserScraper](/stealth/browser-scraper).

### Built-in plugin

<Badge color="orange">Plugins</Badge>

One of the official plugins that ship inside seleniumbase-rs, one Cargo feature each: `shield`, `block-resources` and `crx`. The prebuilt `sbrs` and `sbrs-mcp` have them compiled in, and you switch one on by name, for example `sbrs --plugin shield open https://example.com`.

See [Choosing Plugins](/plugins/selecting).

## C

### Canvas noise

<Badge color="purple">Stealth</Badge>

A small, seeded change to the pixels a page reads from a canvas (`toDataURL`, `toBlob`, `getImageData`), so that the canvas fingerprint is not the real machine's. The [StealthPolicy](#stealthpolicy) picks the kind: seeded, a fresh seed for each document, low (one pixel in eight) or none.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#the-policy).

### Capability

<Badge color="orange">Plugins</Badge>

One thing a plugin may do, written as a stable string such as `launch.args`, `cdp:Network` or `page.script:example.com`. A plugin declares the ones it needs and a [PluginSet](#pluginset) grants and checks them; for a native Rust plugin this is a declared contract, not a sandbox. It is not the same as the WebDriver capabilities that `with_capabilities` merges into a session.

See [Capabilities](/plugins/api/capabilities).

### Cargo feature

<Badge color="yellow">Project</Badge>

An opt-in part of the crate that you switch on with `--features`, such as `mcp-server`, `tui`, `gui`, `playwright`, `plugins` or `test-util`. No feature of the library is on by default, so a plain build is small.

See [Building and installing](/building#features).

### CDP

<Badge color="blue">Engine</Badge>

The Chrome DevTools Protocol: the browser's own debugging protocol, spoken over a WebSocket. The [pure CDP](#pure-cdp) engine uses it alone, while [CDP mode](#cdp-mode) and [UC mode](#uc-mode) use it next to WebDriver.

See [Pure CDP: Chrome without a WebDriver](/tutorials/pure_cdp).

### CDP mode

<Badge color="blue">Engine</Badge>

`DriverMode::Cdp`: a WebDriver session that also turns on CDP domains, so you can send raw CDP commands and use helpers such as `cdp_click`. Do not confuse it with Python SeleniumBase's own CDP Mode (`sb.cdp`), which this crate provides as the [pure CDP](#pure-cdp) engine.

See [CDP Mode Guide](/tutorials/cdp_mode).

### CdpReactor

<Badge color="red">Network</Badge>

A helper that connects to a browser's debug WebSocket, turns on the `Fetch` domain and continues every paused request with the headers you give it. It needs a browser started with `--remote-debugging-port`.

See [CDP Mode Guide](/tutorials/cdp_mode#network-reactor).

### Challenge

<Badge color="purple">Stealth</Badge>

A page that a bot-protection service shows instead of the content you asked for: a script that must run, a widget to complete, a block or a rate limit. The stealth crates read it from the response, where a header, a widget or a bare 403 is only evidence and the verdict needs an [interstitial](#interstitial) marker, and then decide what to do next.

See [Cookies, Identity and Challenges](/stealth/identity#challenge-detection-and-mitigation).

### Chrome for Testing

<Badge color="blue">Engine</Badge>

Google's builds of Chrome and chromedriver made for automated testing. The WebDriver engine downloads the chromedriver that matches your Chrome from it, and `--cft` (`Browser::ChromeForTesting`) runs a Chrome for Testing browser, downloaded into `downloaded_drivers/` when missing.

See [Customizing Test Runs](/help/customizing_test_runs#launch-settings) and [Operating systems](/building#operating-systems).

### chrome-headless-shell

<Badge color="blue">Engine</Badge>

Chrome's old headless mode, which now exists only as a separate build. The WebDriver engine runs it as `Browser::ChromeHeadlessShell` (`--browser chs`); it is downloaded into `downloaded_drivers/chs/` when missing and cannot run UC mode.

See [Headless and Headed Browsers](/tutorials/headless#new-headless-mode-and-chrome-headless-shell).

### chromedriver

<Badge color="blue">Engine</Badge>

The program that turns [WebDriver](#webdriver) commands into actions in Chrome. Its major version must match Chrome's: `sbrs install` downloads the right one and `sbrs doctor` compares the two. The [pure CDP](#pure-cdp) engine needs none.

See [Operating systems](/building#operating-systems).

### Clearance

<Badge color="purple">Stealth</Badge>

The cookie a site gives once a [challenge](#challenge) is solved. It belongs to the address that earned it, so an [identity](#identity) may use it only through the same [egress](#egress).

See [Cookies, Identity and Challenges](/stealth/identity#identity-session-policy-and-the-identity-file).

### Client Hints

<Badge color="purple">Stealth</Badge>

The `Sec-CH-UA*` request headers and the `navigator.userAgentData` object through which Chrome tells a site its brand, version and platform. A [Fingerprint](#fingerprint) can set them or turn them off; the user agent keeps Chrome's frozen `major.0.0.0` while the hints carry a real full version.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#randomised-identities).

### ClientHello

<Badge color="red">Network</Badge>

The first message of a TLS handshake, in which a client says which versions, ciphers and extensions it supports. A site can tell clients apart by it, and [JA3](#ja3) and [JA4](#ja4) are summaries of it.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### Commander

<Badge color="gray">CLI</Badge>

A terminal UI for browsing and running tests and examples, behind the Cargo feature `tui`. Without the feature, `sbrs commander` prints a hint and exits with status 2.

See [SeleniumBase Commander](/help/commander).

### Cookie jar

<Badge color="red">Network</Badge>

The cookie store of the stealth crates. It follows RFC 6265 and the rules Chrome ships, checks domains against the Public Suffix List, and converts to and from the cookies a CDP browser reports.

See [Cookies, Identity and Challenges](/stealth/identity#the-cookie-jar).

### crx

<Badge color="orange">Plugins</Badge>

The packed Chrome extension format (`.crx`, as CRX3 or the older CRX2). The `crx` plugin unpacks such files into a temporary folder and loads them into a Chromium browser that the plugin set starts; it does not check the signature in the header.

See [crx Plugin](/plugins/crx).

### CSS selector

<Badge color="blue">Engine</Badge>

The default kind of [selector](#selector): a string with none of the special prefixes is read as CSS, for example `input[name='email']`. An attribute selector such as `[name=q]` is CSS too; there is no `name=` prefix.

See [Selectors Guide](/tutorials/selectors#supported-formats).

## D

### Deferred assertion

<Badge color="green">Testing</Badge>

An assertion that checks the page at once but does not stop the test when it fails. It records the failure, and `process_deferred_asserts` reports all of them together. It is also called a soft assertion.

See [Waits and Assertions](/tutorials/waits_assertions#soft-assertions).

### Demo mode

<Badge color="green">Testing</Badge>

A run setting (`RunSettings::with_demo_mode`) that outlines each element before an interactive call and pauses after it, so a person can follow the test. Never leave it on in CI, because the run waits on purpose.

See [Run Settings: Demo, Slow and Fast Runs](/tutorials/run_settings).

### DevTools endpoint

<Badge color="gray">CLI</Badge>

The address of a running Chrome's remote-debugging port, by default `127.0.0.1:9222`, that other tools attach to. `sbrs devtools` starts a Chrome with one open and prints it; any program on the machine can drive a browser whose endpoint is open.

See [DevTools MCP servers](/tutorials/devtools_mcp).

### Docs7

<Badge color="yellow">Project</Badge>

The documentation host that publishes these pages as a website, part of Context7. Its configuration is `docs/docs.json`, and its navigation follows `docs/SUMMARY.md`.

See [Building and installing](/building#the-docs7-site).

### DriverMode

<Badge color="blue">Engine</Badge>

The setting that chooses how `BaseCase` drives Chrome: `WebDriver` (the default), `Cdp` or `Uc`. The names `webdriver`, `cdp` and `uc` are the ones that `SB_MODE` and `sbrs` use.

See [Public items of the crate root](/api/crate-root).

## E

### Egress

<Badge color="red">Network</Badge>

The exit address a request leaves through: one of your proxies, or the direct connection. Changing it (`rotate_egress`) opens a new browser context behind the new proxy and drops the old one with its cookies, because a [clearance](#clearance) belongs to the address that earned it.

See [BrowserScraper](/stealth/browser-scraper#egress-and-the-profile).

### Ephemeral CA

<Badge color="red">Network</Badge>

A certificate authority made in memory and kept only for the life of one process. [MitmProxy](#mitmproxy) uses one to end Chrome's TLS. Chrome trusts the leaf certificates only through a pin of the leaf key, and the CA is never installed in a trust store or written to disk.

See [TLS Server and MitmProxy](/stealth/mitm-proxy#the-ephemeral-ca-tls-server).

### Evasion provider

<Badge color="purple">Stealth</Badge>

A small unit that returns a JavaScript snippet for one thing a page can probe, such as navigator, WebGL or canvas noise. An `EvasionRegistry` assembles the providers, in priority order, into one script that runs before any page script.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#provider-architecture).

## F

### Fingerprint

<Badge color="purple">Stealth</Badge>

What a browser claims to be: user agent and [Client Hints](#client-hints), language, time zone, screen, location and the hardware a page can probe. It holds one [Spoof](#spoof) per setting, is built once from a preset, `randomized(os, seed)` or the builder, and is never changed afterwards; `to_builder()` makes a changed copy.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth).

## G

### Gherkin

<Badge color="green">Testing</Badge>

The plain-language format of `.feature` files, where behaviour is written as scenarios made of steps. The crate's [Behave](#behave) runner reads Gherkin and matches each step to a step definition you register.

See [Behave / Gherkin Support](/tutorials/behave).

### GREASE

<Badge color="red">Network</Badge>

Random reserved values (RFC 8701) that a client adds to its TLS handshake so that servers do not depend on a fixed list. [JA3](#ja3) and [JA4](#ja4) leave them out, so only the [peetprint](#peetprint) shows a GREASE signature algorithm.

See [Limits and Troubleshooting](/stealth/limits#limits).

## H

### HAR

<Badge color="red">Network</Badge>

HTTP Archive 1.2: a file format that lists the requests a page made and the answers it got. In the pure CDP engine, `page.start_har()` records a tab and `finish()` returns a `Har` that you can save as a `.har` file, which browser developer tools open.

See [Pure CDP](/tutorials/pure_cdp#recording-a-har).

### Headed

<Badge color="blue">Engine</Badge>

A browser that opens a window you can watch. Run headed when someone has to see or use the window, for recording, for clicking with the system pointer, or for sites that block [headless](#headless) browsers.

See [Headless and Headed Browsers](/tutorials/headless).

### Headless

<Badge color="blue">Engine</Badge>

A browser that runs without a window; it still loads pages, runs scripts and takes screenshots and PDFs. The default depends on the entry point: `BrowserConfig::default()` is headless, while `sbrs` and the settings file are headed.

See [Headless and Headed Browsers](/tutorials/headless#defaults).

### HttpSession

<Badge color="red">Network</Badge>

A client in the experimental stealth crates that sends plain HTTP requests carrying the cookies, the measured TLS and HTTP/2 start, the header order and the identity of a real Chrome page. It does not run JavaScript or solve challenges.

See [HttpSession](/stealth/http-session).

### Human-paced input

<Badge color="purple">Stealth</Badge>

Clicks, typing and scrolling timed like a person's: the pointer follows a curve with a bell-shaped speed, typing has uneven intervals and optional typos, and a click lands inside the element rather than on its exact centre. `page.human(behavior)` returns a `Human` that does it; it is still [trusted input](#trusted-input), only slower.

See [Pure CDP](/tutorials/pure_cdp#human-paced-input).

## I

### Identity

<Badge color="purple">Stealth</Badge>

What a session presents to a site. In the stealth crates a `StealthIdentity` holds the [Fingerprint](#fingerprint), a reference to the measured browser, the cookies it earned, its [clearances](#clearance) and the [egress](#egress) they were earned through; it never holds a proxy, and its file is private. In the fingerprint pages, an identity is also a whole Fingerprint drawn for one seed.

See [Cookies, Identity and Challenges](/stealth/identity#identity-session-policy-and-the-identity-file).

### Impersonation

<Badge color="red">Network</Badge>

Making a client send the same TLS handshake, HTTP/2 start and headers as a [measured](#measured-profile) browser, so a site sees that browser's [wire fingerprint](#wire-fingerprint). The `mimic` crate does it with a wreq client, and only Chrome 155 on macOS is measured today.

See [Wire Formats and Impersonation](/stealth/wire-formats#an-impersonating-client).

### Interstitial

<Badge color="purple">Stealth</Badge>

A page shown instead of the content you asked for, such as a check page. A response counts as a [challenge](#challenge) only when it carries an interstitial marker.

See [Cookies, Identity and Challenges](/stealth/identity#challenge-detection-and-mitigation).

## J

### JA3

<Badge color="red">Network</Badge>

A fingerprint of a client's [ClientHello](#clienthello): five comma-separated fields with [GREASE](#grease) removed, usually shown as an MD5 hash. A site can use it to tell TLS clients apart.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### JA4

<Badge color="red">Network</Badge>

A newer fingerprint of a client's [ClientHello](#clienthello), written in three parts as `a_b_c` after FoxIO's definition. `JA4_r` and `JA4_ro` are the unhashed variants, and only JA4 itself is implemented here, not the other JA4+ methods.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### JSON scenario

<Badge color="gray">CLI</Badge>

A hand-written JSON file of steps that `sbrs run-scenario --file` runs, for CI smoke checks or for authors who do not write Rust. It is not the action file that `sbrs record` writes.

See [Syntax Formats](/help/syntax_formats).

## L

### Link text

<Badge color="blue">Engine</Badge>

A [selector](#selector) kind that finds a link by the text people see. `link=Log in` matches the whole text and `partial_link=Privacy` part of it (short form `p_link=`); in the pure CDP engine a `text=` prefix also means link text.

See [Selectors Guide](/tutorials/selectors#supported-formats).

### Locator

<Badge color="blue">Engine</Badge>

The pure CDP engine's handle for finding elements. It remembers how to find them, not which element it found, so every action looks again and waits for the element, and keeps working when the page re-renders. Narrow it with `.locator()`, `.nth()`, `.visible()` and similar calls.

See [Pure CDP](/tutorials/pure_cdp#locators).

## M

### MasterQA

<Badge color="green">Testing</Badge>

A hybrid workflow that mixes automated navigation with manual checks. A test starts a session with `start_qa_session`, runs scripted steps, asks a person to confirm each manual step in the terminal with `manual_verify`, and saves a report that lists only the manual steps.

See [MasterQA Guide](/tutorials/masterqa).

### MCP

<Badge color="gray">CLI</Badge>

The Model Context Protocol: the way an AI agent calls a tool such as a browser. This crate ships the MCP server `sbrs-mcp`.

See [MCP servers](/tutorials/mcp_server).

### Measured profile

<Badge color="red">Network</Badge>

Wire values copied from a real browser by capture tools and stored as JSON: its TLS spec, HTTP/2 start, header order and Client Hints. Today there is one, Chrome 155.0.8059.40 on macOS, and any other browser and system pair is refused with `NotMeasured`.

See [Wire Formats and Impersonation](/stealth/wire-formats#what-is-measured-and-what-is-refused).

### MFA

<Badge color="green">Testing</Badge>

Multi-factor authentication: a login that asks for a second proof, here usually a [TOTP](#totp) code. `enter_totp_code(css, secret)` types the code into a field and presses Enter.

See [MFA / TOTP Guide](/tutorials/mfa_totp).

### MitmProxy

<Badge color="red">Network</Badge>

A loopback forward proxy in the experimental `mimic` crate. It ends a real Chrome's TLS with a certificate from an [ephemeral CA](#ephemeral-ca) and replays each request through the impersonating client, so the site sees the measured handshake instead of Chrome's own.

See [TLS Server and MitmProxy](/stealth/mitm-proxy#mitmproxy).

### Mock browser

<Badge color="green">Testing</Badge>

A pure CDP browser backed by a script instead of Chrome. `Browser::new_mocked()` (Cargo feature `test-util`) records every protocol command and answers it with the handler you register, so you can test what your code sends.

See [Pure CDP](/tutorials/pure_cdp#testing-without-chrome).

## N

### Native spoofing

<Badge color="purple">Stealth</Badge>

A [StealthPolicy](#stealthpolicy) option (`native_spoofing`) that delivers a fingerprint's claims through CDP overrides and launch arguments where the browser can, instead of through replaced JavaScript getters. The browser then reports the value itself; things the protocol cannot cover, such as WebGL and canvas noise, stay in JavaScript.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#native-level-cdp-spoofing).

## P

### Page

<Badge color="blue">Engine</Badge>

One browser tab in the pure CDP engine. It is a cheap handle (a clone points to the same tab) for navigating, running scripts, taking screenshots and making [Locators](#locator).

See [Pure CDP](/tutorials/pure_cdp).

### Parity table

<Badge color="yellow">Project</Badge>

The page that records, for every API of Python SeleniumBase 4.55.2, how this crate covers it: with an item of the same name (implemented), with other items (composed), not at all because there is nothing to port (not-applicable), or not yet (planned).

See [API parity with Python SeleniumBase](/parity).

### peetprint

<Badge color="red">Network</Badge>

A [JA3](#ja3)-like TLS fingerprint that also records the signature algorithms, and shows [GREASE](#grease) as `GREASE`.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### Persona

<Badge color="purple">Stealth</Badge>

The whole machine and place that a [Fingerprint](#fingerprint) pretends to be: operating system, hardware, locale, time zone and position. A randomised identity draws the persona first and derives every field from it, so the signals agree with each other.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#randomised-identities).

### Playwright mode

<Badge color="blue">Engine</Badge>

An optional engine, behind the Cargo feature `playwright`, that uses [rustwright](#rustwright) to drive Chromium with a Playwright-compatible API. `activate_playwright_mode` starts it as a second Chromium next to the WebDriver one.

See [Playwright Mode](/help/playwright_mode).

### Plugin

<Badge color="orange">Plugins</Badge>

A browser plugin is a Rust crate that changes how a browser starts and what every tab gets, through hooks (`before_launch`, `on_browser`, `on_page_created` and `on_close`). You add it to a [PluginSet](#pluginset) and give the set to the launch options; it is not a [test plugin](#test-plugin), which runs around a test.

See [Browser Plugins](/tutorials/plugins).

### PluginSet

<Badge color="orange">Plugins</Badge>

The plugins a program uses, with the [capabilities](#capability) each was granted. It checks them against the browser about to start, orders them and calls their hooks.

See [PluginSet](/plugins/api/plugin-set).

### Pure CDP

<Badge color="blue">Engine</Badge>

The engine in `sb_cdp` that drives Chrome directly over the DevTools Protocol, with no chromedriver and none of the markers a WebDriver leaves in the page. Its main types are `Browser`, [Page](#page) and [Locator](#locator), and it is the engine behind the `cdp` MCP server.

See [Pure CDP: Chrome without a WebDriver](/tutorials/pure_cdp).

## Q

### QUIC

<Badge color="red">Network</Badge>

The UDP-based transport that carries HTTP/3. A [StealthPolicy](#stealthpolicy) can force it off (`--disable-quic`), force it on (`--enable-quic`) or leave it to Chrome.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#the-policy).

## R

### Recorder

<Badge color="green">Testing</Badge>

A tool that captures your clicks, typing and navigations in a browser and turns them into a JSON action file or a Rust test. `sbrs record URL -o actions.json` records and `sbrs replay` runs the file again; from code, `BaseCase` records the actions it performs.

See [Recorder Mode Guide](/tutorials/recorder_mode).

### Request interception

<Badge color="red">Network</Badge>

Pausing every request a page makes and answering it by the first matching rule: block it, serve a made-up response or send it on with changes. In the pure CDP engine this is `page.intercept(rules)`, and other tabs are not affected.

See [Pure CDP](/tutorials/pure_cdp#intercepting-requests).

### Reuse session

<Badge color="green">Testing</Badge>

A run option (`with_reuse_session(true)`, `--reuse-session` or `SB_REUSE_SESSION`) that keeps a browser open from one test to the next. The browser is cleaned before the next test gets it, and only one test holds it at a time.

See [Customizing Test Runs](/help/customizing_test_runs#sharing-a-browser-between-tests).

### RTK

<Badge color="yellow">Project</Badge>

A command proxy that shortens the output of `cargo`, `git` and test runners before it reaches an AI assistant, for example `rtk cargo test`. Exit codes are unchanged, and it is optional.

See [Build from source](/contributing/from-source#working-with-an-ai-assistant).

### RunSettings

<Badge color="green">Testing</Badge>

Run-wide settings as plain data, the Rust version of Python SeleniumBase's pytest options such as `--demo`, `--slow` and `--message-duration`. Build one and give it to `BrowserConfig::with_run_settings` or `BaseCase::set_run_settings`; the default changes nothing.

See [Run Settings: Demo, Slow and Fast Runs](/tutorials/run_settings).

### rustwright

<Badge color="blue">Engine</Badge>

The native Rust CDP engine behind [Playwright mode](#playwright-mode). It drives Chromium with a Playwright-compatible API and needs no Node driver.

See [Playwright Mode](/help/playwright_mode).

## S

### sbrs

<Badge color="gray">CLI</Badge>

The command-line tool of the crate (package `seleniumbase-rs-cli`): open pages, run smoke tests, send CDP commands, patch binaries, record and replay, convert Python scripts and more. Global options such as `--uc`, `--headless` and `--browser` go before the subcommand.

See [CLI Usage (sbrs)](/tutorials/cli_usage).

### sbrs doctor

<Badge color="gray">CLI</Badge>

A diagnostic check of the environment and configuration. Among other things it compares the chromedriver version with Chrome's.

See [sbrs doctor](/help/cli_reference#sbrs-doctor).

### sbrs install

<Badge color="gray">CLI</Badge>

Downloads a driver into `./downloaded_drivers/`, in practice the chromedriver that matches your Chrome. `sbrs get` is another name for it.

See [sbrs install](/help/cli_reference#sbrs-install).

### sbrs-mcp

<Badge color="gray">CLI</Badge>

The MCP server program. It lets an AI client drive a browser over standard input and output, with one of three toolsets chosen by `--server`: `cdp` (pure CDP), `driver` or `sb` (the default). It opens no network port.

See [MCP servers](/tutorials/mcp_server).

### Selector

<Badge color="blue">Engine</Badge>

The text that says which element you mean. `BaseCase` reads its start: `#` is an ID, `.` a class, `/` or `//` begins [XPath](#xpath), `link=` is [link text](#link-text), and anything else is a [CSS selector](#css-selector).

See [Selectors Guide](/tutorials/selectors).

### SelectorBuf

<Badge color="blue">Engine</Badge>

The owned selector type of the pure CDP engine, the twin of the borrowed `Selector` that `BaseCase` uses. `SelectorBuf::css` and `xpath` state the kind outright, and `containing_text` finds elements by their text.

See [Pure CDP](/tutorials/pure_cdp#locators).

### Selenium Grid

<Badge color="blue">Engine</Badge>

A hub that sends tests to browsers on other machines. `sbrs grid` starts a hub or node from a Selenium Server jar that you supply, and `BrowserConfig::webdriver_url` points a test at the hub; only the WebDriver engine can use it.

See [Deployment](/deployment#selenium-grid).

### seleniumbase-rs-stealth

<Badge color="purple">Stealth</Badge>

The separate, experimental repository that holds the TLS and HTTP/2 layer as three crates: `codec` (wire formats), `scrape` ([BrowserScraper](#browserscraper), identity, challenge detection) and `mimic` ([HttpSession](#httpsession), [MitmProxy](#mitmproxy), [StealthSession](#stealthsession)). It left this crate in 0.3.0.

See [Stealth Scraping](/stealth/overview).

### Settings

<Badge color="blue">Engine</Badge>

The layers that configure a run, where a later one wins: built-in defaults, the `sbase_config.toml` file, `SB_*` environment variables and a [BrowserConfig](#browserconfig) built in code. `sbrs` flags override the file and the variables for the commands it runs.

See [Settings and Global Configuration](/tutorials/settings_and_config).

### Shadow DOM

<Badge color="blue">Engine</Badge>

A tree of elements hidden inside a web component. `BaseCase` searches inside open shadow roots with the `::shadow` combinator, as in `my-app ::shadow button`; only open shadow roots can be reached.

See [Shadow DOM Guide](/tutorials/shadow_dom).

### Slow mode

<Badge color="green">Testing</Badge>

A run setting (`RunSettings::with_slow_mode`) that pauses after interactive calls as [demo mode](#demo-mode) does, but without outlining the element.

See [Run Settings: Demo, Slow and Fast Runs](/tutorials/run_settings).

### SNI

<Badge color="red">Network</Badge>

Server Name Indication: the host name a client states in its TLS handshake. [MitmProxy](#mitmproxy) refuses a connection whose SNI names a different host from the one the browser asked for.

See [TLS Server and MitmProxy](/stealth/mitm-proxy).

### Soft assertion

<Badge color="green">Testing</Badge>

Another name for a [deferred assertion](#deferred-assertion): a check that records its failure instead of stopping the test.

See [Waits and Assertions](/tutorials/waits_assertions#soft-assertions).

### Spoof

<Badge color="purple">Stealth</Badge>

One setting of a [Fingerprint](#fingerprint), which is in one of four states: `Natural` (the browser's own value, the default), `Random` (a coherent value drawn when the fingerprint is built), `Value` (exactly what you give) or `Off` (the API is turned off, only where that makes sense, such as geolocation or WebRTC). The state and the value are one field, so they cannot disagree.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#one-setting-four-states).

### Stale element

<Badge color="blue">Engine</Badge>

An element found earlier whose part of the page was later replaced, so the old handle no longer works. Find the element again before each action; a [Locator](#locator) does that for you.

See [Selectors Guide](/tutorials/selectors#selector-troubleshooting).

### StealthPolicy

<Badge color="purple">Stealth</Badge>

How a [Fingerprint](#fingerprint)'s claims are delivered and which protections run beside them, such as the WebRTC leak policy, [canvas noise](#canvas-noise), [audio noise](#audio-noise) and the `window.chrome` stubs; it never changes what a setting claims. The presets are `default()` (only the `toString` mask and open shadow roots), `balanced()`, `native_only()` and `max()`.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#the-policy).

### StealthSession

<Badge color="purple">Stealth</Badge>

A session in the experimental `mimic` crate that presents one identity in two modes: plain HTTP through an [HttpSession](#httpsession), or a real Chrome behind a [MitmProxy](#mitmproxy). When a challenge appears in HTTP mode it moves to the browser, solves it there, and returns to HTTP with the clearance.

See [StealthSession](/stealth/stealth-session).

## T

### Tauri

<Badge color="yellow">Project</Badge>

A framework for desktop apps that draw their window with the system webview. The Tauri plugin runs a small W3C WebDriver endpoint inside your app, so `BaseCase` and `sbrs` can test its real window on macOS, Windows and Linux.

See [Tauri Plugin](/ecosystem/tauri).

### Test plugin

<Badge color="green">Testing</Badge>

Code that runs around a test: when it starts, when it fails (with the browser still open, so it can take evidence) and when it finishes. The bundled ones save a screenshot and the page source on failure and write reports; they are not [browser plugins](#plugin).

See [Test Plugins](/tutorials/test_plugins).

### thirtyfour

<Badge color="blue">Engine</Badge>

The Rust WebDriver client library that the WebDriver engine is built on. Selectors become `thirtyfour::By` values, and elements are `thirtyfour::WebElement` handles.

See [How it compares](/comparison).

### TokenSave

<Badge color="yellow">Project</Badge>

A code graph of the repository that an AI assistant asks for symbols, callers and the impact of a change, instead of searching and reading whole files. It is optional: run `tokensave init` in your checkout.

See [Build from source](/contributing/from-source#working-with-an-ai-assistant).

### TOTP

<Badge color="green">Testing</Badge>

Time-based one-time password: a short code worked out from a shared secret and the current time. `get_totp_code(secret)` returns it from a base32 secret (RFC 6238 with HMAC-SHA1, a 30 second step and 6 digits).

See [MFA / TOTP Guide](/tutorials/mfa_totp).

### Tour

<Badge color="green">Testing</Badge>

A guided walkthrough of a page, drawn as HTML overlays by a JavaScript tour library. You build it in a test with `create_tour_with_theme`, `add_tour_step` and `export_tour`, and the exported file is standalone HTML that you open in a browser.

See [Interactive Tours Guide](/tutorials/tours).

### Trusted input

<Badge color="blue">Engine</Badge>

Input sent through the DevTools Protocol, which the page sees as trusted events, as it sees a person's input. In the pure CDP engine, clicks and key presses are trusted input.

See [Pure CDP: Chrome without a WebDriver](/tutorials/pure_cdp).

## U

### UC mode

<Badge color="purple">Stealth</Badge>

Undetected mode (`DriverMode::Uc`, or `sbrs --uc`): it starts Chromium with undetected-Chrome launch arguments, leaves out the `enable-automation` switch and injects a stealth script into every new document. It needs Chrome, Chromium or Edge, and it does not patch chromedriver for you.

See [Undetected (UC) Mode Guide](/tutorials/uc_mode).

### User agent

<Badge color="purple">Stealth</Badge>

The string a browser sends to say what it is. A headless Chrome reports `HeadlessChrome` instead of `Chrome`, and a [Fingerprint](#fingerprint) can set the user agent together with matching [Client Hints](#client-hints).

See [Headless and Headed Browsers](/tutorials/headless#headless-and-stealth).

## W

### wasm plugin

<Badge color="orange">Plugins</Badge>

A [plugin](#plugin) written in any language that compiles to WebAssembly, loaded with the `seleniumbase-rs-plugin-wasm` crate. The host reads its data once (a name, browser switches, page scripts and URL patterns to block), checks every value and drops the module, which has no imports; it is the only kind of plugin that is sandboxed.

See [wasm Plugin Host](/plugins/wasm).

### WebDriver

<Badge color="blue">Engine</Badge>

The Selenium protocol: the test talks to [chromedriver](#chromedriver) (or another driver), which drives the browser. The WebDriver engine (`BaseCase`) is the one to use for Selenium Grid, Firefox or an existing SeleniumBase-style suite.

See [Performance](/performance#choosing-the-engine).

### WebRTC leak

<Badge color="red">Network</Badge>

A page can open an `RTCPeerConnection` without asking, and the candidates it gathers describe the machine's network, which is how a proxy or VPN gets bypassed. In the pure CDP engine the WebRTC shield makes every connection relay-only, and the `shield` plugin and a fingerprint's WebRTC settings limit what a page can learn.

See [Pure CDP](/tutorials/pure_cdp#keeping-webrtc-from-leaking-addresses).

### Wire fingerprint

<Badge color="red">Network</Badge>

What a site can tell about a client before any page script runs: the TLS [ClientHello](#clienthello), the HTTP/2 connection start, and the order and spelling of the request headers.

See [Stealth Scraping](/stealth/overview).

## X

### XPath

<Badge color="blue">Engine</Badge>

A selector language that walks the document tree. A [selector](#selector) that starts with `/`, `//`, `./`, `../` or `(`, or with an axis such as `parent::`, is read as XPath, and `xpath_to_css` converts only simple expressions to CSS.

See [Selectors Guide](/tutorials/selectors#supported-formats).

### Xvfb

<Badge color="blue">Engine</Badge>

A virtual X display: a headed browser can open its window on it when the machine has no screen. On Linux `BrowserConfig::with_virtual_display` starts one for the WebDriver engine, and the `Xvfb` program must be installed.

See [Headless and Headed Browsers](/tutorials/headless#containers-and-ci).

## Related reading

<Columns cols={2}>

<Card color="#1a85d4" title="Working with seleniumbase-rs" icon="rocket" href="/tutorials/getting_started">

Create a project, add seleniumbase-rs and run your first test.

</Card>

<Card color="#199f43" title="FAQ" icon="circle-question" href="/faq">

Short answers about engines, browsers, drivers and stealth.

</Card>

<Card color="#a631be" title="Fingerprint & Stealth Profiles" icon="fingerprint" href="/tutorials/fingerprint_stealth">

What a fingerprint says a browser is, and how it is delivered.

</Card>

<Card color="#d47628" title="Plugins" icon="plug" href="/plugins/overview">

Browser plugins as Rust crates: shield, block-resources, crx and wasm.

</Card>

</Columns>
