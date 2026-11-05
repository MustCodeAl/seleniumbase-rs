---
title: "CDP Mode: undetected browsing without a driver"
sidebarTitle: "CDP Mode"
---

CDP Mode is the undetected path that needs no driver at all: the pure CDP
engine (`sb_cdp`) starts Chrome and drives it over the DevTools Protocol, so
none of the markers a WebDriver session leaves in the page exist. It is what
Python SeleniumBase calls CDP Mode (`sb.cdp`), and it sits next to
[UC mode](/tutorials/uc_mode), which keeps a chromedriver and hides it.

<Note>

`DriverMode::Cdp` is a different thing: a `BaseCase` WebDriver session with a
DevTools connection beside it, described in [CDP mode](/tutorials/cdp_mode).
This page is about the engine with no driver; [Pure CDP](/tutorials/pure_cdp)
is its full guide.

</Note>

<Visibility for="agents">

Summary for AI exports: CDP Mode is the pure CDP engine (`seleniumbase_rs::sb_cdp`) used as the undetected path, as in Python SeleniumBase's CDP Mode. Launch with `Browser::launch(LaunchOptions::builder()...build()?)`; give a coherent `Fingerprint` with `.fingerprint(&fp)`, a proxy with `.proxy("user:pass@host:port")` and `.shield_webrtc(true)`; drive pages with `page.locator(sel)`, whose input is trusted; `page.solve_captcha()` tries a CAPTCHA widget. There is no chromedriver, so no `cdc_` markers and no `navigator.webdriver`. It is not `DriverMode::Cdp`, which is a WebDriver session with DevTools beside it.

</Visibility>

## UC mode or CDP Mode

| | UC mode | CDP Mode |
|---|---|---|
| Driver | chromedriver, its markers scrubbed | none |
| API | `BaseCase` (`DriverMode::Uc`, `sbrs --uc`) | `sb_cdp::Browser`, `Page` and `Locator` |
| Input | WebDriver input; `uc_click` and `uc_type` with human timing | trusted DevTools input, with [human pacing](/stealth/human-input) |
| Fingerprint | applied through the session | applied to every tab before its first navigation |

Pick CDP Mode when a site detects the driver itself, when one fingerprint must
hold from the first navigation of every tab, or when many workers share a few
browsers ([the browser pool](/tutorials/pure_cdp#many-workers-the-browser-pool)).
Pick UC mode when your tests are written for `BaseCase` and need its WebDriver
API.

## Start an undetected browser

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{Fingerprint, OsType};

async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let fingerprint = Fingerprint::randomized(OsType::Windows, 42);
    let browser = Browser::launch(
        LaunchOptions::builder()
            .fingerprint(&fingerprint)
            .shield_webrtc(true)
            .build()?,
    )
    .await?;
    let page = browser.default_page().await?;
    page.goto("https://example.com").await?;
    page.locator("h1").expect().to_contain_text("Example").await?;
    browser.close().await?;
    Ok(())
}
```

- No chromedriver is started, so the page has no `cdc_` properties and
  `navigator.webdriver` is not announced.
- The fingerprint is applied to every tab as it opens, before its first
  navigation ([Fingerprints](/stealth/fingerprints)). A launch setting made
  before `.fingerprint(..)` wins over the fingerprint's; one made after it
  replaces it.
- The WebRTC shield makes every `RTCPeerConnection` relay-only, so a page
  cannot gather the machine's addresses ([Proxies and network](/stealth/proxies#webrtc)).
- `click`, `fill` and `type_text` arrive as trusted events, as a person's input
  does.

## Behind a proxy

The launch options take a proxy with its user name and password,
`.proxy("alice:s3cret@proxy.example.com:8080")`, and each isolated context can
have its own. The engine answers the proxy's challenge itself, so the password
reaches the proxy and no website. See [Proxies and network](/stealth/proxies).

## CAPTCHAs

`page.solve_captcha()` attempts a Cloudflare Turnstile, reCAPTCHA, hCaptcha,
Friendly Captcha or DataDome widget with trusted mouse events along a human
pointer path; [Pure CDP](/tutorials/pure_cdp#captchas) has the details and
[Handling CAPTCHAs](/tutorials/captcha_testing) shows how to test your own
pages with the vendors' test keys.

## Coming from Python's CDP Mode

In Python, `sb.activate_cdp_mode(url)` starts from UC mode and then drops the
driver. Here you start the pure CDP engine directly; there is no driver to
drop. The methods of `sb.cdp` map onto `Page` and `Locator`; the
[Python parity table](/parity#cdp-mode-methods) lists each one.

## Limits

- A page can still read what the fingerprint does not cover; the browser's
  real version, for one.
- The TLS and HTTP/2 handshake is Chrome's own, which fits a fingerprint that
  claims Chrome. A different browser on the wire is
  [Advanced stealth](/stealth/advanced/overview).
- When a site still tells, see [Troubleshooting](/stealth/troubleshooting).
