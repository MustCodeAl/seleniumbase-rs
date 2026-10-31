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

### Accessibility tree

<Badge color="blue">Engine</Badge>

The browser's second view of a page for assistive technology: a filtered tree of nodes built from the DOM, each with a role, an accessible name and states. A screen reader reads it, not the pixels. The pure CDP engine has no query on it; `SelectorBuf::role` approximates one in XPath.

See [The accessibility tree](/concepts/mobile-and-accessibility#the-accessibility-tree).

### Actionability

<Badge color="green">Testing</Badge>

The state an element must be in before a tool acts on it: it exists, is visible, is not covered, is enabled and has stopped moving. A pure CDP `Locator` waits for the element to be visible and scrolls it into view before it clicks, and fails with `ElementNotFound` or `ElementNotInteractable` if it never is.

See [JavaScript, events and waiting](/concepts/javascript-and-events#auto-waiting-and-actionability).

### Adaptive bitrate

<Badge color="red">Network</Badge>

Streaming that cuts a video into short segments in several qualities and lets the player switch between them as the connection speeds up or slows down. The common formats are [HLS](#hls) and DASH; a script feeds the segments to the video element.

See [Adaptive streaming: HLS and DASH](/concepts/media-and-video#adaptive-streaming-hls-and-dash).

### ADR

<Badge color="yellow">Project</Badge>

An Architecture Decision Record: a short page that records one design decision and why it was made. The records here cover the stealth architecture, multiple drivers and Playwright, the stealth scraping layout and the plugin framework.

See [ADR 001: Stealth and Fingerprinting Architecture](/adr/001-stealth-architecture).

### AES-GCM

<Badge color="red">Network</Badge>

A symmetric cipher mode that encrypts a message and adds a tag, so a changed message fails to decrypt instead of giving garbage. `sbrs encrypt` uses AES-256-GCM with a random 12-byte nonce.

See [Symmetric and asymmetric encryption](/concepts/encryption#symmetric-and-asymmetric-encryption).

### Akamai fingerprint

<Badge color="red">Network</Badge>

A string that describes how a client opens an HTTP/2 connection, written `SETTINGS | WINDOW_UPDATE | PRIORITY | pseudo-header order`. A site can use it, with the TLS handshake and the order of the headers, to tell clients apart.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### ALPN

<Badge color="red">Network</Badge>

Application-Layer Protocol Negotiation: the part of the TLS handshake where client and server agree on HTTP/1.1 or HTTP/2. On the browser's side, `MitmProxy` offers `http/1.1` only.

See [TLS Server and MitmProxy](/stealth/mitm-proxy).

### Asymmetric encryption

<Badge color="red">Network</Badge>

Encryption with a key pair: the public key locks (or checks a signature) and only the private key unlocks (or signs). It is slow, so it is mostly used to agree on a symmetric key.

See [Symmetric and asymmetric encryption](/concepts/encryption#symmetric-and-asymmetric-encryption).

### ARIA

<Badge color="blue">Engine</Badge>

Accessible Rich Internet Applications: attributes such as `role`, `aria-label` and `aria-expanded` that tell assistive technology what a custom control is. They change what is announced, never what the element does, so a native element is better whenever one exists.

See [Roles, names and states](/concepts/mobile-and-accessibility#roles-names-and-states).

### Audio noise

<Badge color="purple">Stealth</Badge>

A tiny, repeatable change to the audio data a page can read (`AudioBuffer.getChannelData` and similar), so that the audio fingerprint is not the real machine's. With the usual settings it follows the fingerprint's seed, so reading the same audio twice gives the same result.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#the-policy).

### Automation markers

<Badge color="purple">Stealth</Badge>

Signs that a program is driving the browser, such as `navigator.webdriver`, the `cdc_` variables that chromedriver puts in the page and the `enable-automation` launch switch. UC mode masks the ones in the page and leaves out the `enable-automation` switch, binary patching removes the `cdc_` markers from chromedriver itself, and the pure CDP engine has no chromedriver to leave any.

See [Undetected (UC) Mode Guide](/tutorials/uc_mode#what-is-patched-automatically).

## B

### Backpressure

<Badge color="red">Network</Badge>

The signal that tells a fast producer of data to wait for a slow consumer, so that queues stay small. A stream with a full queue reports it, and TCP does the same with its receive window.

See [Network and proxies](/concepts/network#backpressure).

### Base64

<Badge color="red">Network</Badge>

An encoding that writes bytes as letters, digits and a few symbols. It is not encryption: anyone can decode it, so a Base64 value is never a secret.

See [Encryption and certificates](/concepts/encryption).

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

### BitTorrent

<Badge color="red">Network</Badge>

A peer-to-peer protocol that shares a file as hashed pieces among many peers, who are found through trackers or a distributed hash table. The more people hold a file, the faster it downloads.

See [Modern web technologies](/concepts/modern-web#bittorrent).

### Blockchain

<Badge color="red">Network</Badge>

A ledger of records grouped into blocks, copied across many computers and chained together by hashes, so that changing old records is evident. Web3 applications read and write one.

See [Modern web technologies](/concepts/modern-web#web3-keys-and-shared-ledgers).

### Blue-green deployment

<Badge color="yellow">Project</Badge>

A release method that runs the new version (green) beside the old one (blue) and then switches all traffic at once, so a rollback is a switch back.

See [Containers and deployment](/concepts/containers-and-deployment#environments-and-deployment).

### Brotli

<Badge color="red">Network</Badge>

A compression method for text, sent as `Content-Encoding: br` when the client lists `br` in `Accept-Encoding`. It usually beats gzip on HTML, CSS and JavaScript, and browsers use it on HTTPS only.

See [Compression](/concepts/bundling-and-caching#compression).

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

### Bundler

<Badge color="blue">Engine</Badge>

A build tool that follows the imports of a project's source files and joins them into a few files for the browser, usually also transpiling, minifying and splitting them. webpack, Rollup, esbuild, Vite and Turbopack are bundlers. A test meets their output as hashed file names and generated class names.

See [Bundlers](/concepts/bundling-and-caching#bundlers).

## C

### Canary deployment

<Badge color="yellow">Project</Badge>

A release method that sends a small share of users to the new version first and widens it while monitoring, to limit the damage of a bad release.

See [Containers and deployment](/concepts/containers-and-deployment#environments-and-deployment).

### Cache-Control

<Badge color="red">Network</Badge>

The response header that says how long and by whom a response may be reused: `max-age`, `no-cache` (store it, but check first), `no-store` (never keep it), `public`, `private` and `immutable`. A wrong value is a common cause of stale content in tests.

See [Cache-Control](/concepts/bundling-and-caching#cache-control).

### Canonical URL

<Badge color="red">Network</Badge>

The preferred address for content that can be reached at several, named in a `link rel="canonical"` element. A search engine takes it as a strong hint when it merges duplicate pages.

See [Canonical URLs](/concepts/crawlers-and-seo#canonical-urls).

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

### CDP domain

<Badge color="blue">Engine</Badge>

A group of related commands and events in the DevTools Protocol, such as `Page`, `Network`, `Input` or `Fetch`. A command is named `Domain.command`, for example `Page.navigate`, and most domains must be turned on with an `enable` command before they send events.

See [The DevTools Protocol](/concepts/cdp#domains).

### CDP mode

<Badge color="blue">Engine</Badge>

`DriverMode::Cdp`: a WebDriver session that also turns on CDP domains, so you can send raw CDP commands and use helpers such as `cdp_click`. Do not confuse it with Python SeleniumBase's own CDP Mode (`sb.cdp`), which this crate provides as the [pure CDP](#pure-cdp) engine.

See [CDP Mode Guide](/tutorials/cdp_mode).

### CdpReactor

<Badge color="red">Network</Badge>

A helper that connects to a browser's debug WebSocket, turns on the `Fetch` domain and continues every paused request with the headers you give it. It needs a browser started with `--remote-debugging-port`.

See [CDP Mode Guide](/tutorials/cdp_mode#network-reactor).

### Certificate authority

<Badge color="red">Network</Badge>

An organization that your device trusts to vouch for websites by signing their certificates. A browser follows the signatures from a site's certificate up to a root authority it already holds.

See [Certificates and the chain of trust](/concepts/encryption#certificates-and-the-chain-of-trust).

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

### Chunked transfer encoding

<Badge color="red">Network</Badge>

An HTTP/1.1 way to send a response body in pieces, each with its size in front and a piece of size zero at the end, when the total length is not known in advance.

See [Network and proxies](/concepts/network#where-http-puts-a-message-boundary).

### CI/CD

<Badge color="yellow">Project</Badge>

Continuous integration builds and tests every change automatically. Continuous delivery or deployment then moves the tested result to servers or users. Browser tests are one stage of that chain.

See [Containers and deployment](/concepts/containers-and-deployment#continuous-integration).

### Clearance

<Badge color="purple">Stealth</Badge>

The cookie a site gives once a [challenge](#challenge) is solved. It belongs to the address that earned it, so an [identity](#identity) may use it only through the same [egress](#egress).

See [Cookies, Identity and Challenges](/stealth/identity#identity-session-policy-and-the-identity-file).

### Clickjacking

<Badge color="red">Network</Badge>

Tricking a user into clicking something hidden, by framing the target site in an invisible iframe over a decoy. It is defended with the `frame-ancestors` directive or the `X-Frame-Options` header.

See [Clickjacking](/concepts/web-security#clickjacking).

### Client Hints

<Badge color="purple">Stealth</Badge>

The `Sec-CH-UA*` request headers and the `navigator.userAgentData` object through which Chrome tells a site its brand, version and platform. A [Fingerprint](#fingerprint) can set them or turn them off; the user agent keeps Chrome's frozen `major.0.0.0` while the hints carry a real full version.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#randomised-identities).

### ClientHello

<Badge color="red">Network</Badge>

The first message of a TLS handshake, in which a client says which versions, ciphers and extensions it supports. A site can tell clients apart by it, and [JA3](#ja3) and [JA4](#ja4) are summaries of it.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### Codec

<Badge color="blue">Engine</Badge>

The method that compresses video or sound, such as H.264, VP9, AV1, AAC or Opus. It is not the container (MP4, WebM) that holds the compressed streams. A plain Chromium build may lack the proprietary codecs that Google Chrome has.

See [Containers and codecs](/concepts/media-and-video#containers-and-codecs).

### Commander

<Badge color="gray">CLI</Badge>

A terminal UI for browsing and running tests and examples, behind the Cargo feature `tui`. Without the feature, `sbrs commander` prints a hint and exits with status 2.

See [SeleniumBase Commander](/help/commander).

### Commit

<Badge color="yellow">Project</Badge>

A Git snapshot of all files with a message and a link to its parent commit. Its id is a hash of its content.

See [Containers and deployment](/concepts/containers-and-deployment#git-in-five-ideas).

### Console

<Badge color="blue">Engine</Badge>

The developer tools panel that shows the messages of a page by level (verbose, info, warning, error) and runs JavaScript in the page with helpers such as `$0` and `$$`.

See [DevTools and CDP](/concepts/cdp#the-console-in-depth).

### Container

<Badge color="yellow">Project</Badge>

A normal process that the Linux kernel isolates, with its own files, network and process list, started from an image. Unlike a virtual machine it shares the host's kernel.

See [Containers and deployment](/concepts/containers-and-deployment#containers-and-virtual-machines).

### Container image

<Badge color="yellow">Project</Badge>

A read-only package of layered files plus a start command, built from a Dockerfile and kept in a registry.

See [Containers and deployment](/concepts/containers-and-deployment#images-layers-and-registries).

### Constraint validation

<Badge color="green">Testing</Badge>

The browser's built-in form checks (`required`, `pattern`, `min`, `max` and the input type), which run before a form is sent. The message bubble is drawn by the browser, so a test reads the field's `validationMessage` instead of looking for it in the page.

See [Constraint validation](/concepts/forms#constraint-validation).

### Content script

<Badge color="orange">Plugins</Badge>

JavaScript that a browser extension runs inside a web page. It sees the page's DOM but runs in an isolated world, with its own variables, so it cannot read the page's JavaScript objects and the page cannot read its.

See [How browser extensions work](/concepts/browser-extensions#the-parts).

### Content Security Policy

<Badge color="red">Network</Badge>

A response header that lists where a page may load and run content from. It blocks injected scripts, which is why a helper that adds an inline script can fail on such a page. `BrowserConfig::with_disable_csp` removes it for a session.

See [Content Security Policy](/concepts/web-security#content-security-policy).

### Controlled input

<Badge color="green">Testing</Badge>

A form field whose value is kept in a framework's state (React, Vue) and updated by `input` and `change` events. Setting the field's value without those events changes the screen but not the state.

See [Custom widgets and framework-controlled inputs](/concepts/forms#custom-widgets-and-framework-controlled-inputs).

### Cookie

<Badge color="red">Network</Badge>

A small piece of text that a server asks the browser to keep (the `Set-Cookie` header) and that the browser sends back with every matching request (the `Cookie` header). Attributes such as `Domain`, `Path`, `Expires`, `Secure`, `HttpOnly` and `SameSite` decide when it is sent. A login is usually a cookie holding a session id.

See [Sessions, cookies and storage](/concepts/sessions-and-state#cookies-in-depth).

### Cookie jar

<Badge color="red">Network</Badge>

The cookie store of the stealth crates. It follows RFC 6265 and the rules Chrome ships, checks domains against the Public Suffix List, and converts to and from the cookies a CDP browser reports.

See [Cookies, Identity and Challenges](/stealth/identity#the-cookie-jar).

### Crawl frontier

<Badge color="green">Testing</Badge>

The queue of URLs that a crawler knows about and has not visited yet.

See [Crawlers and SEO](/concepts/crawlers-and-seo#how-a-crawler-works).

### Crawler

<Badge color="green">Testing</Badge>

A program that fetches a page, collects its links and visits those, as search engines and link checkers do. Also called a spider. It differs from scraping, which extracts data, and from testing, which checks a site.

See [Crawlers and SEO](/concepts/crawlers-and-seo#crawling-scraping-and-testing).

### COOP, COEP and CORP

<Badge color="red">Network</Badge>

Three headers that put a page in its own cross-origin isolated group: Cross-Origin-Opener-Policy cuts the link to windows from other origins, Cross-Origin-Embedder-Policy refuses resources that have not opted in, and Cross-Origin-Resource-Policy says who may load a file.

See [Web security](/concepts/web-security#browser-security-policies).

### CORS

<Badge color="red">Network</Badge>

Cross-Origin Resource Sharing: response headers with which a server lets another origin read its answers. Browsers enforce it; command-line HTTP clients do not.

See [Web security](/concepts/web-security#browser-security-policies).

### Core Web Vitals

<Badge color="blue">Engine</Badge>

Three user-centred page metrics: LCP (Largest Contentful Paint, loading, good at 2.5 s or less), INP (Interaction to Next Paint, responsiveness, 200 ms or less) and CLS (Cumulative Layout Shift, visual stability, 0.1 or less). Search engines use them as a ranking signal.

See [Core Web Vitals](/concepts/how-a-page-loads#core-web-vitals).

### crx

<Badge color="orange">Plugins</Badge>

The packed Chrome extension format (`.crx`, as CRX3 or the older CRX2). The `crx` plugin unpacks such files into a temporary folder and loads them into a Chromium browser that the plugin set starts; it does not check the signature in the header.

See [crx Plugin](/plugins/crx).

### CSRF

<Badge color="red">Network</Badge>

Cross-site request forgery: a hostile page makes your browser send a request, with your cookies, to a site where you are logged in. It is defended with tokens, SameSite cookies and Origin checks.

See [Cross-site request forgery](/concepts/web-security#cross-site-request-forgery-csrf).

### CSS selector

<Badge color="blue">Engine</Badge>

The default kind of [selector](#selector): a string with none of the special prefixes is read as CSS, for example `input[name='email']`. An attribute selector such as `[name=q]` is CSS too; there is no `name=` prefix.

See [Selectors Guide](/tutorials/selectors#supported-formats).

### cURL

<Badge color="red">Network</Badge>

A command-line tool for HTTP requests. The browser's Copy as cURL turns a recorded request into a command with its headers and cookies, which are real credentials.

See [DevTools and CDP](/concepts/cdp#network-and-copy-as-curl).

## D

### Dapp

<Badge color="red">Network</Badge>

A web page whose buttons call smart contracts through a wallet.

See [Modern web technologies](/concepts/modern-web#how-a-page-talks-to-a-wallet).

### Datagram

<Badge color="red">Network</Badge>

One independent packet sent by UDP, with no connection, no order and no resend.

See [Network and proxies](/concepts/network#udp-and-quic).

### Deferred assertion

<Badge color="green">Testing</Badge>

An assertion that checks the page at once but does not stop the test when it fails. It records the failure, and `process_deferred_asserts` reports all of them together. It is also called a soft assertion.

See [Waits and Assertions](/tutorials/waits_assertions#soft-assertions).

### Demo mode

<Badge color="green">Testing</Badge>

A run setting (`RunSettings::with_demo_mode`) that outlines each element before an interactive call and pauses after it, so a person can follow the test. Never leave it on in CI, because the run waits on purpose.

See [Run Settings: Demo, Slow and Fast Runs](/tutorials/run_settings).

### Developer tools

<Badge color="blue">Engine</Badge>

The panels built into a browser (Elements, Console, Sources, Network, Application and more). In Chrome they talk to the page over the [CDP](#cdp).

See [DevTools and CDP](/concepts/cdp#the-panels).

### Device pixel ratio

<Badge color="blue">Engine</Badge>

How many physical screen pixels make one CSS pixel (`window.devicePixelRatio`). A phone that is 390 CSS pixels wide at a ratio of 3 has 1170 physical pixels. `DeviceMetrics` carries it as `pixel_ratio`.

See [CSS pixels and the pixel ratio](/concepts/mobile-and-accessibility#css-pixels-and-the-pixel-ratio).

### DevTools endpoint

<Badge color="gray">CLI</Badge>

The address of a running Chrome's remote-debugging port, by default `127.0.0.1:9222`, that other tools attach to. `sbrs devtools` starts a Chrome with one open and prints it; any program on the machine can drive a browser whose endpoint is open.

See [DevTools MCP servers](/tutorials/devtools_mcp).

### Digital signature

<Badge color="red">Network</Badge>

A value made with a private key that anyone with the public key can check, proving who made a message and that it was not changed. It is behind certificates, software updates and signed JWTs.

See [Signatures](/concepts/encryption#signatures).

### DNS

<Badge color="red">Network</Badge>

The Domain Name System: the lookup that turns a host name such as `shop.example.com` into an IP address before a connection is opened. Answers are cached for the record's time to live. A lookup that bypasses a proxy is called a DNS leak.

See [DNS: names to addresses](/concepts/network#dns-names-to-addresses).

### Docker Compose

<Badge color="yellow">Project</Badge>

A tool that describes several containers in a `compose.yaml` file and starts them together on a private network, where each service reaches the others by name.

See [Containers and deployment](/concepts/containers-and-deployment#compose-several-containers-together).

### Dockerfile

<Badge color="yellow">Project</Badge>

The recipe for a container image, with one instruction such as `FROM`, `RUN` or `COPY` for each layer. The one at the root of the repository builds `sbrs` in two stages.

See [Containers and deployment](/concepts/containers-and-deployment#the-dockerfile).

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

### Element reference

<Badge color="blue">Engine</Badge>

In WebDriver, the opaque id (under the key `element-6066-11e4-a52e-4f735466cecf`) that the driver returns for an element it found. It is a handle, not the element itself: when the page replaces the element, the handle goes stale and the next command fails with `stale element reference`.

See [Element references](/concepts/webdriver#element-references).

### Environment

<Badge color="yellow">Project</Badge>

A copy of a system with its own purpose, such as development, staging or production. What differs between them is kept in configuration, not in code.

See [Containers and deployment](/concepts/containers-and-deployment#environments-and-deployment).

### EME

<Badge color="blue">Engine</Badge>

Encrypted Media Extensions: the browser interface for protected video. The page asks a content decryption module, such as [Widevine](#widevine), for keys from a license server, and the clear video never reaches the page, so a screenshot of it is often black.

See [Protected video](/concepts/media-and-video#protected-video-eme-cdms-and-widevine).

### Ephemeral CA

<Badge color="red">Network</Badge>

A certificate authority made in memory and kept only for the life of one process. [MitmProxy](#mitmproxy) uses one to end Chrome's TLS. Chrome trusts the leaf certificates only through a pin of the leaf key, and the CA is never installed in a trust store or written to disk.

See [TLS Server and MitmProxy](/stealth/mitm-proxy#the-ephemeral-ca-tls-server).

### ETag

<Badge color="red">Network</Badge>

A response header holding an identifier of the content. The browser sends it back as `If-None-Match`, and the server answers `304 Not Modified` with no body when nothing changed.

See [Revalidation](/concepts/bundling-and-caching#revalidation).

### Evasion provider

<Badge color="purple">Stealth</Badge>

A small unit that returns a JavaScript snippet for one thing a page can probe, such as navigator, WebGL or canvas noise. An `EvasionRegistry` assembles the providers, in priority order, into one script that runs before any page script.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#provider-architecture).

## F

### Event loop

<Badge color="blue">Engine</Badge>

The loop that runs a page's JavaScript on one thread: it takes one task from a queue (a click, a timer, a network answer), runs it to the end, runs the promise callbacks it queued, and may repaint before the next task. It is why an action's effect on a page arrives after the action has returned.

See [JavaScript, events and waiting](/concepts/javascript-and-events#the-event-loop).

### Feature flag

<Badge color="yellow">Project</Badge>

A switch that ships code turned off, so that it can be released later or to some users.

See [Containers and deployment](/concepts/containers-and-deployment#environments-and-deployment).

### Fingerprint

<Badge color="purple">Stealth</Badge>

What a browser claims to be: user agent and [Client Hints](#client-hints), language, time zone, screen, location and the hardware a page can probe. It holds one [Spoof](#spoof) per setting, is built once from a preset, `randomized(os, seed)` or the builder, and is never changed afterwards; `to_builder()` makes a changed copy.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth).

## G

### Gherkin

<Badge color="green">Testing</Badge>

The plain-language format of `.feature` files, where behaviour is written as scenarios made of steps. The crate's [Behave](#behave) runner reads Gherkin and matches each step to a step definition you register.

See [Behave / Gherkin Support](/tutorials/behave).

### Git

<Badge color="yellow">Project</Badge>

A version control system that keeps the history of a project as commits, branches and tags.

See [Containers and deployment](/concepts/containers-and-deployment#git-in-five-ideas).

### GitHub Actions

<Badge color="yellow">Project</Badge>

The CI system of GitHub. A workflow file in `.github/workflows` names the events, jobs and steps to run.

See [Containers and deployment](/concepts/containers-and-deployment#github-actions-as-the-example).

### GREASE

<Badge color="red">Network</Badge>

Random reserved values (RFC 8701) that a client adds to its TLS handshake so that servers do not depend on a fixed list. [JA3](#ja3) and [JA4](#ja4) leave them out, so only the [peetprint](#peetprint) shows a GREASE signature algorithm.

See [Limits and Troubleshooting](/stealth/limits#limits).

## H

### HAR

<Badge color="red">Network</Badge>

HTTP Archive 1.2: a file format that lists the requests a page made and the answers it got. In the pure CDP engine, `page.start_har()` records a tab and `finish()` returns a `Har` that you can save as a `.har` file, which browser developer tools open.

See [Pure CDP](/tutorials/pure_cdp#recording-a-har).

### Head-of-line blocking

<Badge color="red">Network</Badge>

The delay that follows when one lost TCP packet holds back all the bytes behind it. [QUIC](#quic) avoids it by giving each stream its own order.

See [Network and proxies](/concepts/network#tcp-a-reliable-pipe).

### Hash function

<Badge color="red">Network</Badge>

A one-way function that turns any input into a short fixed-size fingerprint. A tiny change in the input gives a completely different result. Fast hashes check files; passwords need slow ones.

See [Encryption and certificates](/concepts/encryption).

### Headed

<Badge color="blue">Engine</Badge>

A browser that opens a window you can watch. Run headed when someone has to see or use the window, for recording, for clicking with the system pointer, or for sites that block [headless](#headless) browsers.

See [Headless and Headed Browsers](/tutorials/headless).

### Headless

<Badge color="blue">Engine</Badge>

A browser that runs without a window; it still loads pages, runs scripts and takes screenshots and PDFs. The default depends on the entry point: `BrowserConfig::default()` is headless, while `sbrs` and the settings file are headed.

See [Headless and Headed Browsers](/tutorials/headless#defaults).

### HMAC

<Badge color="red">Network</Badge>

A hash mixed with a secret key, so only someone with the key can make or check it. It is used for webhook signatures, HS256 JWTs and TOTP codes.

See [Encryption and certificates](/concepts/encryption).

### HSTS

<Badge color="red">Network</Badge>

HTTP Strict Transport Security: a header that tells the browser to use only https for a host for a set time. It also removes the click-through on certificate errors.

See [Mixed content and HSTS](/concepts/web-security#mixed-content-and-hsts).

### HLS

<Badge color="red">Network</Badge>

HTTP Live Streaming: an [adaptive bitrate](#adaptive-bitrate) format whose manifest is a `.m3u8` file listing short segments in several qualities. Safari plays it natively; other browsers use a script and Media Source Extensions, so the video's `src` is a `blob:` address.

See [Adaptive streaming: HLS and DASH](/concepts/media-and-video#adaptive-streaming-hls-and-dash).

### Hot module replacement

<Badge color="blue">Engine</Badge>

A dev-server feature (HMR) that sends a changed module to the open page over a WebSocket and swaps it in without a reload. A page served by a dev server therefore holds a connection open, so a wait for no network activity never finishes.

See [Dev servers and hot reload](/concepts/bundling-and-caching#dev-servers-and-hot-reload).

### HttpOnly

<Badge color="red">Network</Badge>

A cookie attribute that hides the cookie from JavaScript (`document.cookie` cannot read it) while the browser still sends it with requests. It keeps a login cookie out of reach of an injected script.

See [Cookies, in depth](/concepts/sessions-and-state#cookies-in-depth).

### HttpSession

<Badge color="red">Network</Badge>

A client in the experimental stealth crates that sends plain HTTP requests carrying the cookies, the measured TLS and HTTP/2 start, the header order and the identity of a real Chrome page. It does not run JavaScript or solve challenges.

See [HttpSession](/stealth/http-session).

### Human-paced input

<Badge color="purple">Stealth</Badge>

Clicks, typing and scrolling timed like a person's: the pointer follows a curve with a bell-shaped speed, typing has uneven intervals and optional typos, and a click lands inside the element rather than on its exact centre. `page.human(behavior)` returns a `Human` that does it; it is still [trusted input](#trusted-input), only slower.

See [Pure CDP](/tutorials/pure_cdp#human-paced-input).

## I

### Hydration

<Badge color="blue">Engine</Badge>

The step in which a framework such as React or Vue attaches its event handlers to HTML that the server already drew. Until it finishes, the page looks ready and ignores clicks, which is a common cause of a click that works nine times in ten.

See [Modern web technologies](/concepts/modern-web#single-page-apps-and-frameworks).

### Identity

<Badge color="purple">Stealth</Badge>

What a session presents to a site. In the stealth crates a `StealthIdentity` holds the [Fingerprint](#fingerprint), a reference to the measured browser, the cookies it earned, its [clearances](#clearance) and the [egress](#egress) they were earned through; it never holds a proxy, and its file is private. In the fingerprint pages, an identity is also a whole Fingerprint drawn for one seed.

See [Cookies, Identity and Challenges](/stealth/identity#identity-session-policy-and-the-identity-file).

### Impersonation

<Badge color="red">Network</Badge>

Making a client send the same TLS handshake, HTTP/2 start and headers as a [measured](#measured-profile) browser, so a site sees that browser's [wire fingerprint](#wire-fingerprint). The `mimic` crate does it with a wreq client, and only Chrome 155 on macOS is measured today.

See [Wire Formats and Impersonation](/stealth/wire-formats#an-impersonating-client).

### IndexedDB

<Badge color="blue">Engine</Badge>

A database built into the browser, holding objects with indexes and transactions, for each origin. Larger and more capable than `localStorage`, and never sent to the server. Single-page apps often keep tokens and offline data in it.

See [Web storage](/concepts/sessions-and-state#web-storage).

### Injection

<Badge color="red">Network</Badge>

Input that a program treats as a command, such as SQL or shell text. It is defended by keeping data and commands apart, for example with parameterized queries.

See [Injection](/concepts/web-security#injection).

### Interstitial

<Badge color="purple">Stealth</Badge>

A page shown instead of the content you asked for, such as a check page. A response counts as a [challenge](#challenge) only when it carries an interstitial marker.

See [Cookies, Identity and Challenges](/stealth/identity#challenge-detection-and-mitigation).

### IP address

<Badge color="red">Network</Badge>

The number that names a machine on a network, such as `127.0.0.1` (this machine) or an IPv6 address. With a port it names one end of a connection.

See [Network and proxies](/concepts/network#addresses-and-ports).

### IPFS

<Badge color="red">Network</Badge>

The InterPlanetary File System, which names a file by a hash of its content (a CID) instead of by the place where it is stored.

See [Modern web technologies](/concepts/modern-web#ipfs-addressing-by-content).

## J

### JA3

<Badge color="red">Network</Badge>

A fingerprint of a client's [ClientHello](#clienthello): five comma-separated fields with [GREASE](#grease) removed, usually shown as an MD5 hash. A site can use it to tell TLS clients apart.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### JA4

<Badge color="red">Network</Badge>

A newer fingerprint of a client's [ClientHello](#clienthello), written in three parts as `a_b_c` after FoxIO's definition. `JA4_r` and `JA4_ro` are the unhashed variants, and only JA4 itself is implemented here, not the other JA4+ methods.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### JSON-LD

<Badge color="red">Network</Badge>

A format for structured data: JSON inside a `script type="application/ld+json"` block, using the schema.org vocabulary, that tells a search engine what a page is (a product, an article, a recipe). It can earn rich results.

See [Structured data](/concepts/crawlers-and-seo#structured-data-json-ld).

### JSON-RPC

<Badge color="gray">CLI</Badge>

The small request and reply format, in JSON, that MCP uses for its messages. A request has an `id` and a `method`; the reply repeats the `id` and carries a `result` or an `error`; a notification has no `id`.

See [The Model Context Protocol](/concepts/mcp#how-mcp-messages-travel).

### JSON scenario

<Badge color="gray">CLI</Badge>

A hand-written JSON file of steps that `sbrs run-scenario --file` runs, for CI smoke checks or for authors who do not write Rust. It is not the action file that `sbrs mkrec` writes.

See [Syntax Formats](/help/syntax_formats).

## L

### Layer

<Badge color="yellow">Project</Badge>

One step of a container image build, which stores the files that step changed. Layers are shared between images and re-used by the build cache.

See [Containers and deployment](/concepts/containers-and-deployment#images-layers-and-registries).

### JWT

<Badge color="green">Testing</Badge>

JSON Web Token: a signed note of three Base64URL parts (header, payload and signature). It is signed, not encrypted, so anyone can read its claims.

See [JWTs](/concepts/sessions-and-state#jwts).

### Layout viewport

<Badge color="blue">Engine</Badge>

The area that CSS lays a page out against. With `meta name="viewport"` set to `width=device-width`, a phone gives the real screen width; without it the browser pretends to be about 980 pixels wide. The visual viewport is the part you currently see.

See [The viewport meta tag](/concepts/mobile-and-accessibility#the-viewport-meta-tag).

### Lighthouse

<Badge color="green">Testing</Badge>

Google's audit tool, in Chrome DevTools and on the command line. It loads a page under throttling and scores performance, accessibility, best practices and SEO. It drives Chrome through the DevTools Protocol; seleniumbase-rs has no built-in Lighthouse run.

See [Lighthouse](/concepts/how-a-page-loads#lighthouse).

### Link text

<Badge color="blue">Engine</Badge>

A [selector](#selector) kind that finds a link by the text people see. `link=Log in` matches the whole text and `partial_link=Privacy` part of it (short form `p_link=`); in the pure CDP engine a `text=` prefix also means link text.

See [Selectors Guide](/tutorials/selectors#supported-formats).

### LLM

<Badge color="gray">CLI</Badge>

Large language model: a neural network trained to predict the next token of text, used to write text and code and to choose the tool calls of an AI agent. The same prompt can give different replies, so a test of a site that uses one checks properties, not exact wording. `sbrs-mcp` lets an assistant drive a browser.

See [Large language models](/concepts/mcp#large-language-models).

### localStorage

<Badge color="blue">Engine</Badge>

Key and value storage of about 5 MB that a page's JavaScript can read and write, kept for one origin until it is deleted and shared by all its tabs. It is never sent to the server by the browser. In the pure CDP engine, `Page::local_storage()` reads and writes it.

See [Web storage](/concepts/sessions-and-state#web-storage).

### Locator

<Badge color="blue">Engine</Badge>

The pure CDP engine's handle for finding elements. It remembers how to find them, not which element it found, so every action looks again and waits for the element, and keeps working when the page re-renders. Narrow it with `.locator()`, `.nth()`, `.visible()` and similar calls.

See [Pure CDP](/tutorials/pure_cdp#locators).

### Loopback

<Badge color="red">Network</Badge>

The address `127.0.0.1` (IPv6 `::1`), also named `localhost`, which always means this same machine.

See [Network and proxies](/concepts/network#addresses-and-ports).

## M

### Magnet link

<Badge color="red">Network</Badge>

A link that names a BitTorrent download by the hash of its description, so that peers can be found without a server that holds the file.

See [Modern web technologies](/concepts/modern-web#bittorrent).

### Manifest V3

<Badge color="orange">Plugins</Badge>

The current format of a Chromium browser extension's `manifest.json`. Its background code is a service worker that the browser starts and stops, remote code is not allowed, and network rules are handed to the browser as data.

See [How browser extensions work](/concepts/browser-extensions#manifest-v3).

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

### Merge

<Badge color="yellow">Project</Badge>

Joining two Git branches. A fast-forward merge only moves a branch name ahead when there is nothing to combine.

See [Containers and deployment](/concepts/containers-and-deployment#git-in-five-ideas).

### Media query

<Badge color="blue">Engine</Badge>

A CSS condition on the device, such as `@media (min-width: 768px)`, `(hover: none)` or `(prefers-color-scheme: dark)`. Emulating a phone changes the width and touch values that media queries see.

See [Media queries](/concepts/mobile-and-accessibility#media-queries).

### MFA

<Badge color="green">Testing</Badge>

Multi-factor authentication: a login that asks for a second proof, here usually a [TOTP](#totp) code. `enter_totp_code(css, secret)` types the code into a field and presses Enter.

See [MFA / TOTP Guide](/tutorials/mfa_totp).

### Middleware

<Badge color="blue">Engine</Badge>

A step in the chain a server runs around each request: logging, rate limiting, authentication, parsing, validation. A step can end the request early, so a 401, 429 or 422 may come from a step that runs before the handler.

See [Middleware](/concepts/frontends-backends-and-apis#middleware-a-pipeline-around-every-request).

### Minification

<Badge color="blue">Engine</Badge>

Shrinking JavaScript and CSS by removing whitespace and comments and shortening local names. The output is hard to read, which is why a [source map](#source-map) exists.

See [What happens to your code](/concepts/bundling-and-caching#what-happens-to-your-code).

### MitmProxy

<Badge color="red">Network</Badge>

A loopback forward proxy in the experimental `mimic` crate. It ends a real Chrome's TLS with a certificate from an [ephemeral CA](#ephemeral-ca) and replays each request through the impersonating client, so the site sees the measured handshake instead of Chrome's own.

See [TLS Server and MitmProxy](/stealth/mitm-proxy#mitmproxy).

### Mixed content

<Badge color="red">Network</Badge>

An https page that loads something over plain http. Browsers block active mixed content such as scripts, frames and fetches.

See [Mixed content and HSTS](/concepts/web-security#mixed-content-and-hsts).

### Mock browser

<Badge color="green">Testing</Badge>

A pure CDP browser backed by a script instead of Chrome. `Browser::new_mocked()` (Cargo feature `test-util`) records every protocol command and answers it with the handler you register, so you can test what your code sends.

See [Pure CDP](/tutorials/pure_cdp#testing-without-chrome).

### Multi-stage build

<Badge color="yellow">Project</Badge>

A Dockerfile that builds in a large image full of compilers and copies only the result into a small runtime image.

See [Containers and deployment](/concepts/containers-and-deployment#the-dockerfile).

### MutationObserver

<Badge color="blue">Engine</Badge>

A browser API that calls a function when the DOM changes: children added or removed, attributes or text changed. A script run from a test can use it to wait for a change without polling.

See [JavaScript and Web APIs](/concepts/javascript-and-events#javascript-and-the-browser-are-two-things).

## N

### Multipart form data

<Badge color="red">Network</Badge>

The form body format `multipart/form-data`, in which each field is its own part, separated by a boundary string. It is required for file uploads.

See [How the body is encoded](/concepts/forms#how-the-body-is-encoded).

### Native spoofing

<Badge color="purple">Stealth</Badge>

A [StealthPolicy](#stealthpolicy) option (`native_spoofing`) that delivers a fingerprint's claims through CDP overrides and launch arguments where the browser can, instead of through replaced JavaScript getters. The browser then reports the value itself; things the protocol cannot cover, such as WebGL and canvas noise, stay in JavaScript.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#native-level-cdp-spoofing).

## P

### Nonce

<Badge color="red">Network</Badge>

A number used once. AES-GCM needs a unique one for every message under a key; a Content Security Policy uses a fresh random one per response to mark the page's own inline scripts as allowed.

See [Encryption and certificates](/concepts/encryption) and [Content Security Policy](/concepts/web-security#content-security-policy).

### OAuth 2.0

<Badge color="green">Testing</Badge>

A standard that lets an app get limited access to an account without seeing the password. A "sign in with" button uses the authorization code flow: a redirect to the provider and back with a one-time code.

See [Sessions and logging in](/concepts/sessions-and-state#sessions-and-tokens).

### OpenID Connect

<Badge color="green">Testing</Badge>

A layer on top of OAuth 2.0 that adds an ID token, a JWT that tells the app who signed in.

See [Sessions and logging in](/concepts/sessions-and-state#sessions-and-tokens).

### OWASP Top 10

<Badge color="red">Network</Badge>

A ranked list of the most common web application risks, published by OWASP. The 2025 edition starts with broken access control.

See [The OWASP Top 10](/concepts/web-security#the-owasp-top-10).

### Page

<Badge color="blue">Engine</Badge>

One browser tab in the pure CDP engine. It is a cheap handle (a clone points to the same tab) for navigating, running scripts, taking screenshots and making [Locators](#locator).

See [Pure CDP](/tutorials/pure_cdp).

### Parity table

<Badge color="yellow">Project</Badge>

The page that records, for every API of Python SeleniumBase 4.55.2, how this crate covers it: with an item of the same name (implemented), with other items (composed), not at all because there is nothing to port (not-applicable), or not yet (planned).

See [API parity with Python SeleniumBase](/parity).

### Partitioned cookie

<Badge color="red">Network</Badge>

A third-party cookie marked `Partitioned` (the CHIPS feature), stored under the embedded site together with the top-level site you are on, so the same widget on two sites gets two separate cookies. It must also be `Secure`.

See [Modern web technologies](/concepts/modern-web#the-privacy-sandbox-changes).

### Passkey

<Badge color="green">Testing</Badge>

A login credential for the WebAuthn API: a key pair made on a device, where the private key stays on the device and signs a challenge from the site. A test cannot click the system dialog that confirms it, so it uses a virtual authenticator.

See [Modern web technologies](/concepts/modern-web#passkeys-and-webauthn).

### Peer-to-peer (P2P)

<Badge color="red">Network</Badge>

A design in which computers talk to each other directly and each is client and server for the others. BitTorrent, WebRTC data channels and IPFS are examples.

See [Modern web technologies](/concepts/modern-web#peer-to-peer).

### Password hashing

<Badge color="red">Network</Badge>

Storing a password as a deliberately slow, salted hash (Argon2id, scrypt, bcrypt or PBKDF2), so that a stolen database is hard to crack.

See [Encryption and certificates](/concepts/encryption).

### PBKDF2

<Badge color="red">Network</Badge>

A slow key-derivation function that turns a passphrase into a key by repeating HMAC many times. `sbrs encrypt` uses PBKDF2-HMAC-SHA256 with 600,000 iterations and a random salt.

See [Encryption and certificates](/concepts/encryption).

### peetprint

<Badge color="red">Network</Badge>

A [JA3](#ja3)-like TLS fingerprint that also records the signature algorithms, and shows [GREASE](#grease) as `GREASE`.

See [Wire Formats and Impersonation](/stealth/wire-formats#reading-the-wire-formats).

### Permissions-Policy

<Badge color="red">Network</Badge>

A header that switches browser features such as the camera, geolocation and microphone on or off for a page and its frames. It decides whether a permission prompt may appear at all.

See [Permissions-Policy](/concepts/web-security#permissions-policy).

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

### Port

<Badge color="red">Network</Badge>

A number from 0 to 65535 that says which program on a machine receives a message. HTTPS uses 443, and `sbrs devtools` listens on 9222 by default.

See [Network and proxies](/concepts/network#addresses-and-ports).

### Preflight request

<Badge color="red">Network</Badge>

An `OPTIONS` request the browser sends before a cross-origin request that is not simple (a `PUT`, a JSON body or a custom header), to ask whether the server allows it.

See [Preflighted requests](/concepts/web-security#preflighted-requests).

### Prompt injection

<Badge color="gray">CLI</Badge>

Text in a page or in a tool result that is written to look like an instruction to an AI assistant. A model can be fooled by it, so a tool's output is treated as data and not as orders, and safety decisions (which plugins load, where files go) belong to the operator who starts the server.

See [AI agents that drive a browser](/concepts/mcp#ai-agents-that-drive-a-browser).

### Proxy

<Badge color="red">Network</Badge>

A server that makes requests on your behalf, so the site sees the proxy's IP address instead of yours. HTTP, HTTPS and SOCKS5 proxies differ in what they relay; a proxy may ask for a user name and password (a `407` reply), and a PAC file or a bypass list decides which hosts use it.

See [Proxies](/concepts/network#proxies).

### Pull request

<Badge color="yellow">Project</Badge>

A request to merge a branch into another one, where the change is reviewed and CI runs on it.

See [Containers and deployment](/concepts/containers-and-deployment#git-in-five-ideas).

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

A tool that captures your clicks, typing and navigations in a browser and turns them into a Rust test and a JSON action file. `sbrs mkrec FILE.rs` records and `sbrs replay recordings/NAME_rec.json` runs the JSON file again; from code, `BaseCase` records the actions it performs.

See [Recorder Mode Guide](/tutorials/recorder_mode).

### Registry

<Badge color="yellow">Project</Badge>

A server that stores container images, such as Docker Hub or `ghcr.io`. An image name has the form `registry/owner/name:tag`.

See [Containers and deployment](/concepts/containers-and-deployment#images-layers-and-registries).

### Referrer-Policy

<Badge color="red">Network</Badge>

A header that controls how much of the previous page's address is sent in the `Referer` header when a link is followed.

See [Referrer-Policy](/concepts/web-security#referrer-policy).

### Reflow

<Badge color="blue">Engine</Badge>

The browser redoing layout, the step that works out the size and position of every box, after a change such as an added element or a new width. It is costly and can move content, which is why a late reflow can shift a button just as a test clicks it.

See [Reflow and repaint](/concepts/how-a-page-loads#reflow-and-repaint).

### Renderer process

<Badge color="blue">Engine</Badge>

The browser process that runs one site's pages: it parses the HTML and CSS, builds the DOM and runs the JavaScript, inside a sandbox. Chrome starts a separate one for each site.

See [How a modern browser is built](/concepts/browser-architecture#many-processes-one-browser).

### Request interception

<Badge color="red">Network</Badge>

Pausing every request a page makes and answering it by the first matching rule: block it, serve a made-up response or send it on with changes. In the pure CDP engine this is `page.intercept(rules)`, and other tabs are not affected.

See [Pure CDP](/tutorials/pure_cdp#intercepting-requests).

### Reuse session

<Badge color="green">Testing</Badge>

A run option (`with_reuse_session(true)`, `--reuse-session` or `SB_REUSE_SESSION`) that keeps a browser open from one test to the next. The browser is cleaned before the next test gets it, and only one test holds it at a time.

See [Customizing Test Runs](/help/customizing_test_runs#sharing-a-browser-between-tests).

### robots.txt

<Badge color="green">Testing</Badge>

The file at `/robots.txt` in which a site tells crawlers which paths they may fetch. It is a request, not access control.

See [Crawlers and SEO](/concepts/crawlers-and-seo#robotstxt-and-sitemaps).

### Rollback

<Badge color="yellow">Project</Badge>

Returning to the last good version after a bad release.

See [Containers and deployment](/concepts/containers-and-deployment#environments-and-deployment).

### Rolling deployment

<Badge color="yellow">Project</Badge>

Replacing servers a few at a time so that the service never stops, with the old and new versions running together for a while.

See [Containers and deployment](/concepts/containers-and-deployment#environments-and-deployment).

### robots.txt

<Badge color="red">Network</Badge>

A file at the root of a site that tells crawlers which paths they may fetch. It controls crawling, not indexing: a blocked page can still be listed from links to it, and `noindex` only works on a page the crawler is allowed to fetch.

See [Robots, sitemaps and status codes](/concepts/crawlers-and-seo#robots-sitemaps-and-status-codes).

### RTK

<Badge color="yellow">Project</Badge>

A command proxy that shortens the output of `cargo`, `git` and test runners before it reaches an AI assistant, for example `rtk cargo test`. Exit codes are unchanged, and it is optional.

See [Build from source](/contributing/from-source#working-with-an-ai-assistant).

### Runner

<Badge color="yellow">Project</Badge>

The machine that runs a CI job, such as `ubuntu-latest` in GitHub Actions.

See [Containers and deployment](/concepts/containers-and-deployment#github-actions-as-the-example).

### RunSettings

<Badge color="green">Testing</Badge>

Run-wide settings as plain data, the Rust version of Python SeleniumBase's pytest options such as `--demo`, `--slow` and `--message-duration`. Build one and give it to `BrowserConfig::with_run_settings` or `BaseCase::set_run_settings`; the default changes nothing.

See [Run Settings: Demo, Slow and Fast Runs](/tutorials/run_settings).

### rustwright

<Badge color="blue">Engine</Badge>

The native Rust CDP engine behind [Playwright mode](#playwright-mode). It drives Chromium with a Playwright-compatible API and needs no Node driver.

See [Playwright Mode](/help/playwright_mode).

## S

### Same-origin policy

<Badge color="blue">Engine</Badge>

The browser's main security rule: a page can read data only from its own origin, the combination of scheme, host and port. It is why storage is per origin, and why sites that share data on purpose use CORS headers.

See [Origins and the same-origin policy](/concepts/sessions-and-state#origins-and-the-same-origin-policy).

### SameSite

<Badge color="red">Network</Badge>

A cookie attribute that says whether the cookie is sent on requests started from another site: `Strict` never, `Lax` only on top-level navigations (the default in modern browsers), `None` always (and then `Secure` is required). In the pure CDP engine it is the `SameSite` enum.

See [Cookies, in depth](/concepts/sessions-and-state#cookies-in-depth).

### Sandbox

<Badge color="blue">Engine</Badge>

An operating-system cage around the part of a browser that handles web content, so a bug there cannot read files or start programs. The switch `--no-sandbox` removes it.

See [Browser architecture](/concepts/browser-architecture#site-isolation-and-the-sandbox).

### sbrs

<Badge color="gray">CLI</Badge>

The command-line tool of the crate (package `seleniumbase-rs-cli`): open pages, run smoke tests, send CDP commands, patch binaries, record and replay, convert Python scripts and more. Global options such as `--uc`, `--headless`, `--browser` and `--url` go before or after the subcommand.

See [CLI Usage (sbrs)](/tutorials/cli_usage).

### sbrs doctor

<Badge color="gray">CLI</Badge>

A diagnostic check of the environment and configuration. Among other things it compares the chromedriver version with Chrome's.

See [sbrs doctor](/help/cli_reference#sbrs-doctor).

### sbrs install

<Badge color="gray">CLI</Badge>

Downloads a driver or a browser into `./downloaded_drivers/`; the name is required, as in `sbrs install chromedriver` for the one that matches your Chrome. `sbrs get` is another name for it.

See [sbrs install](/help/cli_reference#sbrs-install).

### sbrs-mcp

<Badge color="gray">CLI</Badge>

The MCP server program. It lets an AI client drive a browser over standard input and output, with one of three toolsets chosen by `--server`: `cdp` (pure CDP), `driver` or `sb` (the default). It opens no network port.

See [MCP servers](/tutorials/mcp_server).

### Seed phrase

<Badge color="red">Network</Badge>

The list of recovery words that encodes a wallet's private key. Never put a real one in a test, a script, a log or a repository.

See [Modern web technologies](/concepts/modern-web#web3-keys-and-shared-ledgers).

### Screen reader

<Badge color="blue">Engine</Badge>

Software that speaks a page, or sends it to a braille display, working from the accessibility tree. NVDA, JAWS, VoiceOver, TalkBack and Orca are examples.

See [Screen readers and the keyboard](/concepts/mobile-and-accessibility#screen-readers-and-the-keyboard).

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

### Semantic versioning

<Badge color="yellow">Project</Badge>

Version numbers of the form MAJOR.MINOR.PATCH, where a major change may break users. While the major number is 0, a minor change may break them too.

See [Containers and deployment](/concepts/containers-and-deployment#git-in-five-ideas).

### Server-Sent Events

<Badge color="red">Network</Badge>

A one-way channel in which a server keeps an HTTP response open (`text/event-stream`) and writes events into it, and the browser's `EventSource` reads them. Unlike a WebSocket, the page cannot send on it.

See [Modern web technologies](/concepts/modern-web#websockets-and-server-sent-events).

### Serverless function

<Badge color="blue">Engine</Badge>

Code that a platform starts when a request arrives and may stop afterwards. The first call after a quiet period is slow (a cold start), and nothing kept in the function's memory is there for the next call, so state lives in a database or travels in a cookie or token.

See [Serverless and edge functions](/concepts/frontends-backends-and-apis#serverless-and-edge-functions).

### Service worker

<Badge color="red">Network</Badge>

A script a page registers with the browser that runs on its own thread, outlives the tab and answers the page's requests, from a cache or from the network. It makes offline pages possible, and it can answer a request that never reaches the network.

See [Modern web technologies](/concepts/modern-web#service-workers-and-pwas).

### sessionStorage

<Badge color="blue">Engine</Badge>

Like `localStorage`, but kept for one tab only and gone when the tab closes. In the pure CDP engine, `Page::session_storage()` reads and writes it.

See [Web storage](/concepts/sessions-and-state#web-storage).

### Settings

<Badge color="blue">Engine</Badge>

The layers that configure a run, where a later one wins: built-in defaults, the `sbase_config.toml` file, `SB_*` environment variables and a [BrowserConfig](#browserconfig) built in code. `sbrs` flags override the file and the variables for the commands it runs.

See [Settings and Global Configuration](/tutorials/settings_and_config).

### Shadow DOM

<Badge color="blue">Engine</Badge>

A tree of elements hidden inside a web component. Every selector of `BaseCase` (and of the pure CDP engine's `page.locator`) can enter shadow roots with the `::shadow` combinator, as in `my-app::shadow button`. `BaseCase` enters closed roots too; the pure CDP engine enters open roots only.

See [Shadow DOM Guide](/tutorials/shadow_dom).

### Single-page application

<Badge color="blue">Engine</Badge>

A web app that loads once and then rewrites its own DOM with JavaScript instead of loading a new page for each screen. After the first load there is no load event, the address changes without a navigation, and elements are replaced often.

See [Modern web technologies](/concepts/modern-web#single-page-apps-and-frameworks).

### Site isolation

<Badge color="blue">Engine</Badge>

A Chrome security feature that puts pages from different sites into different renderer processes. It is why a cross-site iframe is its own target for a tool that attaches to the browser.

See [How a modern browser is built](/concepts/browser-architecture#site-isolation-and-the-sandbox).

### Sitemap

<Badge color="green">Testing</Badge>

An XML file that lists the URLs a site wants found. It is the best seed list for crawling your own site in a test.

See [Crawlers and SEO](/concepts/crawlers-and-seo#robotstxt-and-sitemaps).

### Sitemap

<Badge color="red">Network</Badge>

An XML file, usually `/sitemap.xml`, that lists the URLs a site wants search engines to find, often with their last change date. It helps discovery; it does not guarantee indexing.

See [Robots, sitemaps and status codes](/concepts/crawlers-and-seo#robots-sitemaps-and-status-codes).

### Slow mode

<Badge color="green">Testing</Badge>

A run setting (`RunSettings::with_slow_mode`) that pauses after interactive calls as [demo mode](#demo-mode) does, but without outlining the element.

See [Run Settings: Demo, Slow and Fast Runs](/tutorials/run_settings).

### Smart contract

<Badge color="red">Network</Badge>

A program stored on a blockchain that runs when a transaction calls it.

See [Modern web technologies](/concepts/modern-web#web3-keys-and-shared-ledgers).

### SNI

<Badge color="red">Network</Badge>

Server Name Indication: the host name a client states in its TLS handshake. [MitmProxy](#mitmproxy) refuses a connection whose SNI names a different host from the one the browser asked for.

See [TLS Server and MitmProxy](/stealth/mitm-proxy).

### Socket

<Badge color="red">Network</Badge>

The handle that the operating system gives a program for one end of a network connection.

See [Network and proxies](/concepts/network#sockets).

### Soft assertion

<Badge color="green">Testing</Badge>

Another name for a [deferred assertion](#deferred-assertion): a check that records its failure instead of stopping the test.

See [Waits and Assertions](/tutorials/waits_assertions#soft-assertions).

### Source map

<Badge color="blue">Engine</Badge>

A file (such as `app.js.map`) that maps positions in minified output back to the original source, so developer tools can show real names. Production sites often leave it out, so a stack trace from a test can read `a.js:1:48213`.

See [Source maps](/concepts/bundling-and-caching#source-maps).

### Spoof

<Badge color="purple">Stealth</Badge>

One setting of a [Fingerprint](#fingerprint), which is in one of four states: `Natural` (the browser's own value, the default), `Random` (a coherent value drawn when the fingerprint is built), `Value` (exactly what you give) or `Off` (the API is turned off, only where that makes sense, such as geolocation or WebRTC). The state and the value are one field, so they cannot disagree.

See [Fingerprint & Stealth Profiles](/tutorials/fingerprint_stealth#one-setting-four-states).

### SSRF

<Badge color="red">Network</Badge>

Server-side request forgery: making a server fetch an address the attacker chooses, such as an internal admin page or a cloud metadata address.

See [Server-side request forgery](/concepts/web-security#server-side-request-forgery-ssrf).

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

### Streams API

<Badge color="blue">Engine</Badge>

The browser API with `ReadableStream`, `WritableStream` and `TransformStream`, which handles data in pieces and supports [backpressure](#backpressure).

See [Network and proxies](/concepts/network#the-streams-api).

### STUN and TURN

<Badge color="red">Network</Badge>

Helper servers for WebRTC. STUN tells a browser its public address; TURN relays the data when no direct path exists.

See [Modern web technologies](/concepts/modern-web#webrtc-and-data-channels).

### Structured data

<Badge color="red">Network</Badge>

Facts about a page written in a form that machines read, usually as [JSON-LD](#json-ld) with the schema.org vocabulary, so that a search engine can show rich results for it.

See [Structured data](/concepts/crawlers-and-seo#structured-data-json-ld).

### Structured logging

<Badge color="yellow">Project</Badge>

Writing log lines as records with named fields, usually JSON, so a tool can filter and count them. `init_tracing_json()` and `SB_LOG_FORMAT=json` make the library log this way, to standard error.

See [Structured logging](/concepts/telemetry-and-observability#structured-logging).

## T

### Subresource Integrity

<Badge color="red">Network</Badge>

An `integrity` attribute that holds the hash of a script or stylesheet. The browser refuses the file if its hash does not match.

See [Encryption and certificates](/concepts/encryption).

### Symmetric encryption

<Badge color="red">Network</Badge>

Encryption in which one key locks and unlocks. It is fast, so it carries most traffic; AES is the standard. Both sides must already share the key.

See [Symmetric and asymmetric encryption](/concepts/encryption#symmetric-and-asymmetric-encryption).

### Target

<Badge color="blue">Engine</Badge>

In the DevTools Protocol, one thing you can attach to and command: a tab, a cross-site iframe, a worker, or the browser itself. After `Target.attachToTarget` the target has a session id that goes with every message for it on the shared WebSocket.

See [Targets and sessions](/concepts/cdp#targets-and-sessions).

### Tauri

<Badge color="yellow">Project</Badge>

A framework for desktop apps that draw their window with the system webview. The Tauri plugin runs a small W3C WebDriver endpoint inside your app, so `BaseCase` and `sbrs` can test its real window on macOS, Windows and Linux.

See [Tauri Plugin](/ecosystem/tauri).

### TCP

<Badge color="red">Network</Badge>

The Transmission Control Protocol, which gives two programs a reliable, ordered stream of bytes, opened by a three-way handshake. It has no message boundaries.

See [Network and proxies](/concepts/network#tcp-a-reliable-pipe).

### Telemetry

<Badge color="yellow">Project</Badge>

Data a program sends out about itself: logs, metrics and [traces](#trace). OpenTelemetry is the open standard for producing and shipping them. On a web page, analytics and tracking scripts are telemetry that add requests and consent banners to a test.

See [The three kinds of signal](/concepts/telemetry-and-observability#the-three-kinds-of-signal).

### Test plugin

<Badge color="green">Testing</Badge>

Code that runs around a test: when it starts, when it fails (with the browser still open, so it can take evidence) and when it finishes. The bundled ones save a screenshot and the page source on failure and write reports; they are not [browser plugins](#plugin).

See [Test Plugins](/tutorials/test_plugins).

### Testnet

<Badge color="red">Network</Badge>

A separate blockchain with worthless coins for trying things out. Tests should use one, with throwaway keys.

See [Modern web technologies](/concepts/modern-web#web3-keys-and-shared-ledgers).

### Third-party cookie

<Badge color="red">Network</Badge>

A cookie set by something embedded in the page (an ad, a video player) rather than by the site in the address bar. The same service embedded on many sites can recognise you on all of them, so browsers block, restrict or partition these cookies.

See [First-party and third-party](/concepts/sessions-and-state#first-party-and-third-party).

### thirtyfour

<Badge color="blue">Engine</Badge>

The Rust WebDriver client library that the WebDriver engine is built on. Selectors become `drivers::By` values, and elements are `drivers::WebElement` handles.

See [How it compares](/comparison).

### Three-way handshake

<Badge color="red">Network</Badge>

The SYN, SYN-ACK and ACK messages that open a TCP connection, one round trip before any data.

See [Network and proxies](/concepts/network#tcp-a-reliable-pipe).

### TLS

<Badge color="red">Network</Badge>

Transport Layer Security, the encryption under HTTPS. A handshake picks versions, ciphers and keys, and the server proves its identity with a certificate signed by an authority the browser trusts. The client's opening message, the ClientHello, is what JA3 and JA4 fingerprint.

See [TLS and certificates](/concepts/network#tls-and-certificates).

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

### Trace

<Badge color="yellow">Project</Badge>

The path of one request through every service it touches, as a tree of timed spans that share a trace id. Between services the id travels in the W3C `traceparent` header. It is not the [tracing](#tracing) crate.

See [OpenTelemetry](/concepts/telemetry-and-observability#opentelemetry).

### tracing

<Badge color="yellow">Project</Badge>

The Rust crate that seleniumbase-rs logs with. `tracing_util::init_tracing()` installs a subscriber that writes to standard error and reads `RUST_LOG`; `SB_LOG_LEVEL` and `SB_LOG_FORMAT` set the level and format for `sbrs` and `sbrs-mcp`.

See [The library's own logs](/concepts/telemetry-and-observability#the-librarys-own-logs).

### Tree shaking

<Badge color="blue">Engine</Badge>

A build step that drops exports which nothing imports, so unused parts of a library do not ship. It relies on ES modules, whose imports can be read without running the code.

See [What happens to your code](/concepts/bundling-and-caching#what-happens-to-your-code).

### Trusted input

<Badge color="blue">Engine</Badge>

Input sent through the DevTools Protocol, which the page sees as trusted events, as it sees a person's input. In the pure CDP engine, clicks and key presses are trusted input.

See [Pure CDP: Chrome without a WebDriver](/tutorials/pure_cdp).

## U

### UC mode

<Badge color="purple">Stealth</Badge>

Undetected mode (`DriverMode::Uc`, or `sbrs --uc`): it starts Chromium with undetected-Chrome launch arguments, leaves out the `enable-automation` switch and injects a stealth script into every new document. It needs Chrome, Chromium or Edge, and it does not patch chromedriver for you.

See [Undetected (UC) Mode Guide](/tutorials/uc_mode).

### UDP

<Badge color="red">Network</Badge>

The User Datagram Protocol: independent datagrams with no handshake, order or resend. It carries DNS, calls, games and, under [QUIC](#quic), the web.

See [Network and proxies](/concepts/network#udp-and-quic).

### User agent

<Badge color="purple">Stealth</Badge>

The string a browser sends to say what it is. A headless Chrome reports `HeadlessChrome` instead of `Chrome`, and a [Fingerprint](#fingerprint) can set the user agent together with matching [Client Hints](#client-hints).

See [Headless and Headed Browsers](/tutorials/headless#headless-and-stealth).

### User data directory

<Badge color="blue">Engine</Badge>

The folder where Chrome keeps a browser profile: cookies, `localStorage`, `IndexedDB`, cache, settings and extensions (the `--user-data-dir` flag). Starting Chrome again with the same folder gives the same signed-in user. In the pure CDP engine, `LaunchOptionsBuilder::user_data_dir` sets it.

See [Profiles, and what is in one](/concepts/sessions-and-state#profiles-and-what-is-in-one).

## W

### Wallet

<Badge color="red">Network</Badge>

Software that holds a person's keys and signs transactions. In a browser it is usually an extension that adds `window.ethereum` to pages.

See [Modern web technologies](/concepts/modern-web#how-a-page-talks-to-a-wallet).

### wasm plugin

<Badge color="orange">Plugins</Badge>

A [plugin](#plugin) written in any language that compiles to WebAssembly, loaded with the `seleniumbase-rs-plugin-wasm` crate. The host reads its data once (a name, browser switches, page scripts and URL patterns to block), checks every value and drops the module, which has no imports; it is the only kind of plugin that is sandboxed.

See [wasm Plugin Host](/plugins/wasm).

### Web 1.0 and Web 2.0

<Badge color="yellow">Project</Badge>

Labels for the web of documents (static pages) and the web of applications and platforms (JavaScript, accounts, content written by users).

See [Modern web technologies](/concepts/modern-web#three-eras).

### Web3

<Badge color="red">Network</Badge>

A label for applications that use public blockchains and wallets, where a person proves who they are with a key instead of an account.

See [Modern web technologies](/concepts/modern-web#web3-keys-and-shared-ledgers).

### WCAG

<Badge color="green">Testing</Badge>

The Web Content Accessibility Guidelines, the standard behind most accessibility law. Requirements have levels A, AA and AAA and fall under four principles: perceivable, operable, understandable and robust. AA is the usual target.

See [WCAG in one table](/concepts/mobile-and-accessibility#wcag-in-one-table).

### Web API

<Badge color="blue">Engine</Badge>

A feature the browser offers to the JavaScript of a page, such as the DOM, Fetch, Storage, Geolocation or Clipboard. Standards define them and MDN documents them; they are not part of the JavaScript language. A test uses them by running JavaScript in the page.

See [The APIs you meet most](/concepts/javascript-and-events#the-apis-you-meet-most).

### WebAssembly

<Badge color="blue">Engine</Badge>

A compact binary format that a browser runs at close to native speed in the same sandbox as JavaScript. Code written in Rust or C++ is compiled to it. It changes nothing about how a page is driven, but its logic cannot be read in the page source.

See [Modern web technologies](/concepts/modern-web#webassembly).

### WebAuthn

<Badge color="green">Testing</Badge>

The browser API behind passkeys and security keys (`navigator.credentials.create` and `get`). The DevTools Protocol has a `WebAuthn` domain with virtual authenticators so that a test can register and log in without a dialog.

See [Modern web technologies](/concepts/modern-web#passkeys-and-webauthn).

### WebDriver

<Badge color="blue">Engine</Badge>

The Selenium protocol: the test talks to [chromedriver](#chromedriver) (or another driver), which drives the browser. The WebDriver engine (`BaseCase`) is the one to use for Selenium Grid, Firefox or an existing SeleniumBase-style suite.

See [Performance](/performance#choosing-the-engine).

### WebDriver BiDi

<Badge color="blue">Engine</Badge>

The bidirectional part of the W3C WebDriver standard: a WebSocket on which the browser can push events (console messages, network activity, page loads) as well as answer commands. seleniumbase-rs does not use it; the CDP engine covers the same ground for Chromium.

See [WebDriver BiDi](/concepts/webdriver#webdriver-bidi).

### WebGL

<Badge color="purple">Stealth</Badge>

The browser's graphics API for the graphics card (WebGPU is its successor). A page can read the card's vendor and renderer strings and the pixels of a test scene, which makes it a fingerprint; a machine with no graphics card reports a software renderer.

See [Modern web technologies](/concepts/modern-web#webgl-and-webgpu).

### WebRTC data channel

<Badge color="red">Network</Badge>

The `RTCDataChannel` of WebRTC, which sends any data directly between two browsers once they are connected through signaling, STUN and TURN. See also [WebRTC leak](#webrtc-leak).

See [Modern web technologies](/concepts/modern-web#webrtc-and-data-channels).

### WebRTC leak

<Badge color="red">Network</Badge>

A page can open an `RTCPeerConnection` without asking, and the candidates it gathers describe the machine's network, which is how a proxy or VPN gets bypassed. In the pure CDP engine the WebRTC shield makes every connection relay-only, and the `shield` plugin and a fingerprint's WebRTC settings limit what a page can learn.

See [Pure CDP](/tutorials/pure_cdp#keeping-webrtc-from-leaking-addresses).

### WebSocket

<Badge color="red">Network</Badge>

A long-lived two-way connection that starts as an HTTP request and then carries messages in both directions. The DevTools Protocol runs over one, and so does WebDriver BiDi.

See [Connecting](/concepts/cdp#connecting).

### WebVTT

<Badge color="blue">Engine</Badge>

The plain-text format for captions and subtitles: a `WEBVTT` header and cues with start and end times. A `track` element adds a file to a video, and the video's `textTracks` list gives a script the cues.

See [Captions: WebVTT](/concepts/media-and-video#captions-webvtt).

### Widevine

<Badge color="blue">Engine</Badge>

Google's content decryption module for [EME](#eme), shipped with Chrome. A Chromium build or a profile without it may fail to play protected video with a key-system error.

See [Protected video](/concepts/media-and-video#protected-video-eme-cdms-and-widevine).

### Wire fingerprint

<Badge color="red">Network</Badge>

What a site can tell about a client before any page script runs: the TLS [ClientHello](#clienthello), the HTTP/2 connection start, and the order and spelling of the request headers.

See [Stealth Scraping](/stealth/overview).

### Workflow

<Badge color="yellow">Project</Badge>

In GitHub Actions, a YAML file that names the events, jobs and steps of a CI run.

See [Containers and deployment](/concepts/containers-and-deployment#github-actions-as-the-example).

## X

### XPath

<Badge color="blue">Engine</Badge>

A selector language that walks the document tree. A [selector](#selector) that starts with `/`, `//`, `./`, `../` or `(`, or with an axis such as `parent::`, is read as XPath, and `xpath_to_css` converts only simple expressions to CSS.

See [Selectors Guide](/tutorials/selectors#supported-formats).

### XSS

<Badge color="red">Network</Badge>

Cross-site scripting: an attacker's JavaScript runs inside a page that other people view, with that site's rights. It is defended by output encoding, `HttpOnly` cookies and a Content Security Policy.

See [Cross-site scripting](/concepts/web-security#cross-site-scripting-xss).

### Xvfb

<Badge color="blue">Engine</Badge>

A virtual X display: a headed browser can open its window on it when the machine has no screen. On Linux `BrowserConfig::with_virtual_display` starts one for the WebDriver engine, and the `Xvfb` program must be installed.

See [Headless and Headed Browsers](/tutorials/headless#containers-and-ci).

## Related reading

<Columns cols={2}>

<Card color="#1a85d4" title="Working with seleniumbase-rs" icon="rocket" href="/tutorials/getting_started">

Create a project, add seleniumbase-rs and run your first test.

</Card>

<Card color="#199f43" title="FAQ and common problems" icon="circle-question" href="/help/common_problems">

Short answers about engines, browsers, drivers and stealth.

</Card>

<Card color="#a631be" title="Fingerprint & Stealth Profiles" icon="fingerprint" href="/tutorials/fingerprint_stealth">

What a fingerprint says a browser is, and how it is delivered.

</Card>

<Card color="#d47628" title="Plugins" icon="plug" href="/plugins/overview">

Browser plugins as Rust crates: shield, block-resources, crx and wasm.

</Card>

</Columns>
