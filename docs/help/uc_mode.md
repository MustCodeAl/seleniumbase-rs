---
title: "UC Mode (Undetected Chromedriver)"
sidebarTitle: "UC mode options"
---

UC Mode launches Chromium with stealth flags and applies JavaScript evasions that
mask `navigator.webdriver`, plugins, WebGL, and other fingerprints. Removing the
automation markers from the chromedriver binary itself is a separate step that
you run yourself (below).

<Info>

**What you will learn**

- How to enable UC mode.
- How binary patching removes injected markers.
- How engine spoofing args hide additional automation signals.
- What to do if a site still detects the browser.

</Info>

<Visibility for="agents">

Summary for AI exports: turn UC mode on with `BrowserConfig::default().with_mode(DriverMode::Uc)` or `sbrs --uc` before the subcommand. It needs Chrome, Chromium or Edge. UC mode does not patch the driver: run `sbrs patch-chromedriver --path /path/to/chromedriver` and check that `needs_patch()` is `false`. Pass `engine_spoofing_args()` through `BrowserConfig::with_extra_args`. If a site still detects the browser, use a coherent `Fingerprint`, run headful with `with_headless(false)`, align the egress IP with the spoofed geolocation and timezone, or use the pure CDP engine.

</Visibility>

## Quick enable

Set the driver mode to `Uc` in `BrowserConfig`:

<Tabs>

<Tab title="Rust" icon="code">

```rust,no_run
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(
        BrowserConfig::default().with_mode(DriverMode::Uc),
    ).await?;
    sb.open("https://example.com").await?;
    sb.quit().await?;
    Ok(())
}
```

`BrowserConfig::default()` is headless.

</Tab>

<Tab title="Command line" icon="terminal">

On the command line, `sbrs --uc open https://example.com` does the same (put
`--uc` before the subcommand).

```bash
sbrs --uc open https://example.com
```

</Tab>

</Tabs>

<Warning>

UC mode needs Chrome, Chromium or Edge.

</Warning>

## Binary patching

Even in UC mode, a stock `chromedriver` may inject `cdc_` variables into every
page.

<Warning>

UC mode does not patch the driver for you.

</Warning>

Patch the executable before launch to strip those signatures:

<Tabs>

<Tab title="Command line" icon="terminal">

```bash
cargo run --bin sbrs -- patch-chromedriver --path /path/to/chromedriver
```

</Tab>

<Tab title="From Rust" icon="code">

```rust,no_run
use seleniumbase_rs::evasion::patcher::patch_chromedriver;

# fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
patch_chromedriver("/path/to/chromedriver")?;
# Ok(())
# }
```

</Tab>

</Tabs>

See [Binary Patching](/tutorials/binary_patching) for the full API,
backup/restore, and safety notes.

## Engine spoofing arguments

The `engine_spoofing_args()` helper returns Chromium flags that disable
automation features such as `--disable-blink-features=AutomationControlled`.
Pass them through `BrowserConfig` or `StealthOptions`:

```rust
use seleniumbase_rs::{engine_spoofing_args, BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_extra_args(engine_spoofing_args());
```

## Cloudflare challenges

Cloudflare stands in front of some sites with a "Just a moment..." interstitial
that runs checks in the browser. UC mode reaches such a page like any other, but
the challenge can appear mid-navigation, so open it with a fresh session:

```rust,no_run
# use seleniumbase_rs::BaseCase;
# async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
sb.uc_open_with_reconnect("https://example.com/protected").await?;
# Ok(())
# }
```

`uc_open_with_reconnect(url)` reconnects the session and then opens the URL;
`uc_open_with_disconnect(url)` quits the session and connects a new one with the
same config first.

<Warning>

UC mode lowers the signals that trigger a challenge; it does not answer one. A
challenge that asks for a real interactive proof (a Turnstile checkbox, a managed
challenge with a puzzle, or an interactive "Verify you are human" step) is not
solved by UC mode alone. If the page shows a Turnstile checkbox, `solve_captcha()`
finds it (also in a shadow root), waits for it to stand still, presses it along a
human pointer path, checks that the page shows it cleared (a token, the
`cf_clearance` cookie, a page with no widget) and presses again if not, then
returns `Some(Captcha::Turnstile)`. It is not a solver of puzzles, and
Cloudflare may follow with a harder challenge. To see what it did, call
`solve_captcha_with(&SolveOptions::new().success_marker("..."))`. From the
command line, `sbrs --uc --url https://example.com/protected solve-captcha
--expect-cookie cf_clearance` does it and checks the cookie. See
[How CAPTCHAs work](/concepts/captchas) for the steps and the opt-in live check
against https://nowsecure.nl, and
[Testing pages with a CAPTCHA](/tutorials/captcha_testing) for its limits.

</Warning>

## Still detected?

If a site still flags the browser:

<Steps>

<Step title="Patch chromedriver" icon="wrench">

Patch `chromedriver` and verify `needs_patch()` returns `false`.

</Step>

<Step title="Add the spoofing arguments" icon="flag">

Add `engine_spoofing_args()` to the launch flags.

</Step>

<Step title="Use a coherent fingerprint" icon="fingerprint">

Use a [`Fingerprint`](/tutorials/fingerprint_stealth) with consistent
user agent, platform, screen size, WebGL vendor, and geolocation.

</Step>

<Step title="Run headful" icon="window-maximize">

Run headful (`with_headless(false)`). A headless session already starts
Chrome with `--headless=new`, so there is no newer headless mode to switch to.

</Step>

<Step title="Align the egress IP" icon="globe">

Align egress IP with the spoofed geolocation and timezone.

</Step>

<Step title="Try the pure CDP engine" icon="bolt">

Try the [pure CDP engine](/tutorials/pure_cdp), which needs no
chromedriver.

</Step>

</Steps>

## Troubleshooting

<AccordionGroup>

<Accordion title="navigator.webdriver is still true" icon="wrench">

**Likely cause:** chromedriver not patched.

**Fix:** Run `patch-chromedriver` and verify `needs_patch()`.

</Accordion>

<Accordion title="Detection after patching" icon="flag">

**Likely cause:** Missing engine args.

**Fix:** Add `engine_spoofing_args()` to launch flags.

</Accordion>

<Accordion title="Detection only in headless" icon="window-maximize">

**Likely cause:** Headless artifacts.

**Fix:** Run headful with `with_headless(false)` (or `sbrs --headed`).

</Accordion>

<Accordion title="Inconsistent signals" icon="fingerprint">

**Likely cause:** Fingerprint values mismatch.

**Fix:** Validate with `Fingerprint::validate()`.

</Accordion>

</AccordionGroup>

## Next steps

<Columns cols={3}>

<Card color="#1a85d4" title="Binary Patching" icon="wrench" href="/tutorials/binary_patching">

The full API, backup/restore, and safety notes.

</Card>

<Card color="#199f43" title="Fingerprint & Stealth Profiles" icon="fingerprint" href="/tutorials/fingerprint_stealth">

A consistent user agent, platform, screen size, WebGL vendor and geolocation.

</Card>

<Card color="#d47628" title="Pure CDP engine" icon="bolt" href="/tutorials/pure_cdp">

Drives Chrome with no chromedriver.

</Card>

</Columns>
