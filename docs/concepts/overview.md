---
title: "How it works"
sidebarTitle: "Overview"
description: "A map of the How it works section, in a learning order: how a page loads and how a browser is built, how WebDriver, CDP and MCP drive it, the DOM and JavaScript, the network, sessions, security, building and shipping web apps, and mobile, accessibility, SEO and media."
---

<Info>

**In short:** these pages explain the web from the bottom up, in plain words, for
people who automate browsers and need to know why a test behaves the way it
does. Each page ends with the part that matters to seleniumbase-rs, so the
explanation and the API sit side by side. Read them in order, or jump to the
group that matches the problem you have.

</Info>

## Where to start

| If a test... | Start with |
|---|---|
| passes sometimes and fails sometimes | [How a page loads and renders](/concepts/how-a-page-loads), then [JavaScript, events and the Web APIs](/concepts/javascript-and-events) |
| cannot find an element, or finds the wrong one | [Selectors, the DOM and frameworks](/concepts/selectors-and-the-dom) |
| is blocked, or meets a CAPTCHA | [Detection and spoofing](/concepts/detection), then [CAPTCHAs](/concepts/captchas) |
| loses its login, or needs to skip the login | [Sessions, cookies, storage and logging in](/concepts/sessions-and-state) |
| fails only on CI or in a container | [Containers, CI/CD and deployment](/concepts/containers-and-deployment), then [Browser architecture](/concepts/browser-architecture) |
| fails on one site after the site was deployed | [Bundling, compression and caching](/concepts/bundling-and-caching) |
| needs a proxy, a fake response or a recording of the traffic | [The network, TCP, proxies and traffic capture](/concepts/network) |
| should be driven by an AI assistant | [AI, machine learning and the Model Context Protocol](/concepts/mcp) |

## Start here

- [How a page loads and renders](/concepts/how-a-page-loads): from a URL to bytes to pixels, what "loaded" really means, and why automation needs waits.
- [How a modern browser is built](/concepts/browser-architecture): rendering and JavaScript engines, processes, the sandbox, targets, headless mode, profiles and launch flags.

## Automating a browser

- [WebDriver and drivers](/concepts/webdriver): the W3C protocol, the driver binary, sessions, element references and why tests flake without waits.
- [The DevTools Protocol and developer tools](/concepts/cdp): the panels you use by hand and the WebSocket protocol behind them, which the pure CDP engine speaks directly.
- [AI, machine learning and MCP](/concepts/mcp): how an assistant drives a browser through the Model Context Protocol, and how to test sites that contain ML.
- [Detection and spoofing](/concepts/detection): the signals a site reads to tell a program from a person, and where the library answers each one.
- [CAPTCHAs](/concepts/captchas): what they are for, how bot-management scoring works, and what the library does and does not do.
- [Browser extensions](/concepts/browser-extensions): Manifest V3, content scripts, and how automation loads an extension.

## Pages and the DOM

- [Selectors, the DOM and frameworks](/concepts/selectors-and-the-dom): the tree you point at, CSS and XPath, shadow DOM, iframes, what React, Vue and Angular leave behind, and stable selectors.
- [JavaScript, events and the Web APIs](/concepts/javascript-and-events): the event loop, trusted input, auto-waiting, observers, and calling browser APIs from a test.

## Network and the wire

- [The network, TCP, proxies and traffic capture](/concepts/network): sockets, HTTP, DNS, TLS, proxies, interception, HAR recording and what a connection reveals.
- [Bundling, compression and caching](/concepts/bundling-and-caching): why the code a browser runs is not the code that was written, and why stale content breaks tests.

## State, identity and forms

- [Sessions, cookies, storage and logging in](/concepts/sessions-and-state): cookies, web storage, profiles, OAuth, JWTs, TOTP, passkeys, and saving a signed-in state.
- [Forms](/concepts/forms): input types, validation, encodings, uploads, CSRF tokens and framework-controlled inputs.

## Security

- [Web security and browser policies](/concepts/web-security): the OWASP Top 10, the same-origin policy, CORS, CSP and the other rules a browser enforces, and what a test may check.
- [Encryption and certificates](/concepts/encryption): encoding, hashing and encryption told apart, signatures, the chain of trust and keeping secrets out of test code.

## Building and shipping web apps

- [Frontends, backends, APIs and the shapes inside a server](/concepts/frontends-backends-and-apis): what sits behind a page, from reverse proxies and REST to CRUD, middleware and serverless.
- [Containers, CI/CD and deployment](/concepts/containers-and-deployment): Git, CI, Docker, Compose, deployment strategies and where browser tests run.
- [Modern web technologies, eras and peer-to-peer](/concepts/modern-web): single-page apps, service workers, WebAssembly, WebRTC, HTTP/3, passkeys, Web3 wallets and P2P.
- [Telemetry and observability](/concepts/telemetry-and-observability): logs, metrics, traces, analytics scripts and the library's own logging.

## Reach: mobile, accessibility, SEO and media

- [The mobile web and accessibility](/concepts/mobile-and-accessibility): responsive design, device emulation, the accessibility tree and role selectors.
- [Crawlers, spiders and SEO](/concepts/crawlers-and-seo): crawl frontiers, robots.txt, politeness, and what search engines need from a page.
- [Audio and video](/concepts/media-and-video): codecs, adaptive streaming, autoplay, DRM and checking playback from tests.

## Reference

- [Glossary](/glossary): short definitions of every term used in these pages.
