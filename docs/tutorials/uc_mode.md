---
title: "Undetected (UC) Mode"
sidebarTitle: "UC mode"
---

Undetected (UC) mode reduces the fingerprints that websites use to identify
automated browsers. It combines Chromium launch flags, JavaScript evasions,
and optional binary patching to make the browser look more like a regular user
session.

UC mode drives Chrome through <Tooltip tip="Chrome DevTools Protocol: the protocol DevTools uses to control Chrome.">CDP</Tooltip>.
It needs Chrome, Chromium or Edge. With Firefox, `BaseCase::new` returns an
`InvalidConfig` error. It does not patch the chromedriver binary for you.

<Info>

**What you will learn**

- What UC mode changes, and when to turn it on.
- Every option: the command-line flags, the `BrowserConfig` methods, and the `uc_*` calls.
- How to patch `chromedriver`, add engine spoofing arguments, and open a Cloudflare interstitial.

</Info>

<Visibility for="agents">

Summary for AI exports: set `BrowserConfig::default().with_mode(DriverMode::Uc)` or run `sbrs --uc` before the subcommand (`--undetected` and `--undetectable` are the same flag; `SB_MODE=uc`). It needs Chrome, Chromium or Edge, and it cannot be combined with `--cdp`. UC mode starts Chrome with the undetected-Chrome launch arguments, leaves out the `enable-automation` switch and injects a stealth script into every new document. It does not patch chromedriver: use `ChromedriverPatcher`, `patch_chromedriver`, or `sbrs patch-chromedriver --path`. Log DevTools events in this mode with `--uc-cdp-events` (`--uc-cdp`, `SB_UC_CDP_EVENTS`). Clicks and typing with human timing are `uc_click` / `uc_type` and `sbrs uc-click` / `sbrs uc-type`. A fresh session is `reconnect`, `uc_open_with_reconnect` or `uc_open_with_disconnect`. For a site that still detects you, combine the patched driver, `engine_spoofing_args()`, a coherent `Fingerprint`, a headed run (`with_headless(false)` or `--headed`), `FingerprintBuilder::native_spoofing(true)`, an egress IP that matches the spoofed place, and the pure CDP engine.

</Visibility>

## When to use UC mode

Use UC mode when:

- A site blocks normal WebDriver traffic.
- You need to open a page that stands behind a Cloudflare-like interstitial.
- You want the browser fingerprint closer to a regular session.

<Warning>

Avoid UC mode when you do not need anti-detection. The extra flags and patches
make debugging harder and can conflict with some CI environments.

</Warning>

## Options

`--uc` is a global flag: it goes before the subcommand. `--cdp` and `--uc`
cannot be combined.

| Option | Where | What it does |
| --- | --- | --- |
| `BrowserConfig::with_mode(DriverMode::Uc)` | Rust | Turns UC mode on for that session. |
| `--uc`, `--undetected`, `--undetectable` | command line, before the subcommand | Same mode. The environment variable is `SB_MODE=uc`. |
| `--uc-cdp-events`, `--uc-cdp` | command line | Logs the DevTools events while UC mode is on. `SB_UC_CDP_EVENTS`. `true` or `false`. |
| `with_headless(false)`, `--headed`, `--gui` | Rust, command line | A visible window. `BrowserConfig::default()` is headless. `--headed` sets `SB_HEADLESS=false`. |
| `--headless`, `--headless2` | command line | Headless (`SB_HEADLESS`). A headless session uses `--headless=new`. |
| `with_user_agent`, `--user-agent`, `--agent`, `-a` | Rust, command line | The user-agent string (`SB_USER_AGENT`). |
| `with_locale`, `--locale` | Rust, command line | The browser locale (`SB_LOCALE`). |
| `set_timezone`, `set_geolocation` | `BaseCase` | Timezone and geolocation through CDP. They work in `Cdp` and `Uc` mode. In plain WebDriver mode they return an "Enable mode cdp or uc" error. |
| `uc_click`, `sbrs uc-click --css` | Rust, command line | A click with human-like timing. Alias of `human_click`. |
| `uc_type`, `sbrs uc-type --css --text` | Rust, command line | Typing with human-like timing. Alias of `human_type`. |
| `reconnect` | `BaseCase` | Closes the WebDriver session and connects a new one with the same config. A driver you started yourself keeps running; the auto-started driver is a new chromedriver. |
| `uc_open_with_reconnect(url)` | `BaseCase` | `reconnect`, then opens `url`. |
| `uc_open_with_disconnect(url)` | `BaseCase` | Quits the session, connects a new one with the same config, then opens `url`. |
| `is_uc_mode_active` | `BaseCase` | `true` when the session was started in `DriverMode::Uc`. |
| `sbrs patch-chromedriver --path`, `ChromedriverPatcher`, `patch_chromedriver` | command line, Rust | Strips the markers a stock chromedriver injects. UC mode does not do this itself. The auto-started driver is `downloaded_drivers/chromedriver`. |
| `engine_spoofing_args` then `with_extra_args` | Rust | Extra Chromium flags, including `--disable-blink-features=AutomationControlled`. |
| `FingerprintBuilder::native_spoofing(true)` | Rust | Delivers fingerprint claims through CDP and launch arguments instead of JavaScript, where the browser can. |
| `solve_captcha`, `solve_captcha_with` | `BaseCase` | Presses a Turnstile checkbox along a human pointer path. Not a puzzle solver. |
| `sbrs --uc --url URL solve-captcha` | command line | The same press, then checks text and a cookie you name. |
| `uc_gui_press_key`, `uc_gui_handle_captcha` | `BaseCase`, feature `gui` | Native key and mouse input. The captcha helper waits for `css` and clicks it. It does not check that the challenge passed. |

## Enable UC mode

<Tabs>

<Tab title="Rust" icon="code">

```rust,no_run
use seleniumbase_rs::{tokio, BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(false);

    let mut sb = BaseCase::new(config).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    // Verify that navigator.webdriver is masked.
    let hidden = sb.execute_script("return navigator.webdriver === undefined").await?;
    println!("navigator.webdriver hidden: {hidden}");

    sb.quit().await?;
    Ok(())
}
```

`BrowserConfig::default()` is headless; `with_headless(false)` opens a window.

</Tab>

<Tab title="Command line" icon="terminal">

```bash
sbrs --uc open URL
```

`--uc` goes before the subcommand. `sbrs --uc open https://example.com` opens
that page. `sbrs --uc --headed` shows a window.

</Tab>

</Tabs>

## What is patched automatically

When UC mode is active, the framework starts Chrome with the standard
undetected-Chrome launch arguments (`--disable-blink-features=AutomationControlled`,
`--no-first-run`, `--disable-infobars` and more), leaves out the
`enable-automation` switch, turns off the automation extension, and injects a
stealth script into every new document. The script applies these evasions:

- `cdc_` variables injected by `chromedriver` are removed from the page context.
- `navigator.webdriver` is masked or removed.
- `navigator.plugins`, `navigator.languages`, `hardwareConcurrency`, and
  `deviceMemory` are mocked to realistic values.
- Chrome-only objects such as `chrome.app`, `chrome.runtime`, `chrome.csi`, and
  `chrome.loadTimes` are created.
- WebGL vendor and renderer report a common Intel profile.
- `navigator.permissions.query` returns natural values for notifications.
- `navigator.mediaDevices.enumerateDevices` returns realistic device labels.
- iframe `contentWindow` patches propagate the mask into nested frames.

If you set no user agent and Chrome reports `HeadlessChrome`, the user agent is
overridden to say `Chrome`. If the `BrowserConfig` carries a `Fingerprint`, its
bootstrap script and CDP overrides are applied too.

<Note>

UC mode does **not** patch the chromedriver binary for you; see
[Patch the chromedriver binary](#patch-the-chromedriver-binary).

</Note>

## Extra evasions you can apply

### Timezone and geolocation

<Info>

These use CDP, so they work in `Cdp` and `Uc` mode; in plain WebDriver mode they
return an "Enable mode cdp or uc" error.

</Info>

```rust,no_run
use seleniumbase_rs::BaseCase;
async fn demo(sb: &BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    sb.set_timezone("America/Los_Angeles").await?;
    sb.set_geolocation(34.0522, -118.2437, 100.0).await?;
    Ok(())
}
```

### Custom user agent and locale

```rust
use seleniumbase_rs::{BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 ...")
    .with_locale("en-US");
```

### Human-like input and fresh sessions

`uc_click(css)` and `uc_type(css, text)` are aliases for `human_click` and
`human_type`, which move and type with human-like timing. On the command line
the same calls are `uc-click` and `uc-type`, and the page is the one `--url`
opened:

```bash
sbrs --uc --url https://example.com uc-click --css a
sbrs --uc --url https://example.com uc-type --css '#q' --text hello
```

`uc_open_with_reconnect(url)` reconnects the session and then opens the URL;
`uc_open_with_disconnect(url)` quits the session, connects a new one with the
same config, then opens the URL. There is no command-line equivalent of those
two helpers.

### Reconnect for a fresh session

```rust,no_run
use seleniumbase_rs::BaseCase;
async fn demo(sb: &mut BaseCase) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    sb.reconnect().await?;
    Ok(())
}
```

`reconnect` closes the current WebDriver session and connects a new one with the
same configuration. With a driver you run yourself (`--webdriver URL`) the driver
process keeps running; with the auto-started driver, a new chromedriver is
started for the new session. This is useful when a site invalidates the current
session.

## Patch the chromedriver binary

Even in UC mode, a stock `chromedriver` may inject `cdc_` variables into every
page. Patch the executable to strip those signatures. Check `needs_patch()`
afterwards: it is `false` when the markers are gone.

<Tabs>

<Tab title="Command line" icon="terminal">

```bash
sbrs patch-chromedriver --path /path/to/chromedriver
```

The auto-started driver is `downloaded_drivers/chromedriver` (`sbrs install`
puts it there), so that is the file to patch.

</Tab>

<Tab title="From Rust" icon="code">

```rust,no_run
use seleniumbase_rs::{ChromedriverPatcher, EnginePatch};

fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let patcher = ChromedriverPatcher::new("/path/to/chromedriver");
    if patcher.needs_patch()? {
        patcher.patch(EnginePatch::balanced())?;
    }
    Ok(())
}
```

`patch_chromedriver(path)` is the same patch in one call (`EnginePatch::all()`).

</Tab>

</Tabs>

See [Binary Patching](/stealth/advanced/binary-patching) for backup, restore, and safety notes.

## Engine spoofing arguments

Add Chromium flags that disable automation telemetry:

```rust
use seleniumbase_rs::{engine_spoofing_args, BrowserConfig, DriverMode};

let config = BrowserConfig::default()
    .with_mode(DriverMode::Uc)
    .with_extra_args(engine_spoofing_args());
```

`engine_spoofing_args()` returns flags such as
`--disable-blink-features=AutomationControlled` and disables background
networking and default apps.

`FingerprintBuilder::native_spoofing(true)` is separate: it delivers the
fingerprint's claims through CDP and launch arguments instead of JavaScript,
where the browser can, so a page finds no override to inspect.

## Cloudflare challenges

Cloudflare stands in front of some sites with a "Just a moment..." interstitial
that runs checks in the browser before it lets a request through. In UC mode you
reach such a page the same way you reach any other, but a challenge can appear
mid-navigation, so the helpers that open a URL with a fresh session are meant for
it.

<Warning>

Cloudflare is not a widget you can pass on demand. UC mode lowers the signals
that trigger a challenge; it does not answer one. A challenge that asks for a
real interactive proof (a Turnstile checkbox, a managed challenge with a puzzle,
or an interactive "Verify you are human" step) is not solved by UC mode alone,
and a clearance cookie obtained this way lasts only as long as Cloudflare's
policy allows.

</Warning>

### Open a challenge page with a fresh session

`uc_open_with_reconnect(url)` reconnects the WebDriver session and then opens the
URL; `uc_open_with_disconnect(url)` quits the session, connects a new one with the
same config, then opens the URL. Both give the challenge a clean session, which is
what Cloudflare's checks expect:

<Tabs>

<Tab title="Rust" icon="code">

```rust,no_run
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let mut sb = BaseCase::new(
        BrowserConfig::default()
            .with_mode(DriverMode::Uc)
            .with_headless(false),
    ).await?;

    sb.uc_open_with_reconnect("https://example.com/protected").await?;
    // The interstitial runs its checks in the browser; wait for the page behind it.
    sb.wait_for_element_visible("#content", 30).await?;
    sb.quit().await?;
    Ok(())
}
```

</Tab>

<Tab title="Command line" icon="terminal">

```bash
sbrs --uc --headed open https://example.com/protected
```

There is no command-line equivalent of the reconnect helpers; use the library
call when a challenge needs a fresh session.

</Tab>

</Tabs>

### If the challenge shows a Turnstile checkbox

A Cloudflare Turnstile widget is a case for `solve_captcha()`. It finds the
widget (also in a shadow root, and by the id and title Cloudflare gives its
frame), waits for it to stand still, presses its checkbox along a human pointer
path, checks that the page shows it cleared (a token in the response field, the
`cf_clearance` cookie, a page with no widget) and presses again, up to three
times, if it did not. It returns the kind it cleared.

```rust,no_run
use seleniumbase_rs::sb_cdp::{Captcha, SolveOptions};
use seleniumbase_rs::{BaseCase, SeleniumBaseError};
async fn demo(sb: &mut BaseCase) -> Result<(), SeleniumBaseError> {
    assert_eq!(sb.solve_captcha().await?, Some(Captcha::Turnstile));

    // Or say what counts as the site's own page, and see what was done:
    let options = SolveOptions::new().success_marker("document.querySelector('#welcome')");
    if let Some(solved) = sb.solve_captcha_with(&options).await? {
        println!("{solved}");
    }
    Ok(())
}
```

It fails with `SeleniumBaseError::Captcha`, saying what it saw, when the page
could show a result and never did. It is not a solver of puzzles, and Cloudflare
may follow with a harder challenge. From the command line:

```bash
sbrs --uc --url https://nowsecure.nl solve-captcha --expect-text nowsecure --expect-cookie cf_clearance
```

It exits with status 0 when the text and the cookie are there, and with status 1
and a message when they are not. Add `--headed` to watch a window.

[Handling CAPTCHAs](/tutorials/captcha_testing) covers what it does and
its limits; [How CAPTCHAs work](/concepts/captchas) explains why the page behind
the challenge, not the click, decides the outcome, and how the solver is tested.

#### Checking it on https://nowsecure.nl yourself

nowsecure.nl is a public bot-detection test page behind Cloudflare. Whether the
solver passes it is for you to check, on purpose; the library's own tests only
use local imitation pages. The check passes when the site's own page loads, a
Turnstile on the way is cleared and a `cf_clearance` cookie is held afterwards.

```bash
SB_LIVE_SITES=1 cargo test --test captcha_live_chrome -- --ignored --test-threads=1 --nocapture
```

The test is `#[ignore]`d and does nothing unless `SB_LIVE_SITES=1`. It runs the
pure CDP engine and this UC engine in turn, and prints what the solver reported,
the page title and the cookies it found. `SB_HEADED=1` shows a window,
`SB_LIVE_URL` replaces the address and `SB_LIVE_MARKER` is a script expression
that is true on the site's own page, if that page changes.

If a run fails, keep the printed lines: they are what to turn into a fixture in
`tests/fixtures/captcha/` that reproduces the gap.

## Still detected?

If a site still flags the browser, combine the layers:

<Steps>

<Step title="Patch chromedriver" icon="wrench">

Patch `chromedriver` with `ChromedriverPatcher` and verify `needs_patch()`
returns `false`.

</Step>

<Step title="Add the spoofing arguments" icon="sliders">

Add `engine_spoofing_args()` to the launch flags.

</Step>

<Step title="Use a coherent fingerprint" icon="fingerprint">

Use a coherent [`Fingerprint`](/stealth/introduction) with consistent user
agent, platform, screen size, WebGL vendor, and geolocation.

</Step>

<Step title="Run headful" icon="window-maximize">

Run headful (`with_headless(false)` or `sbrs --headed`). A headless session
already uses `--headless=new`, not the legacy headless implementation.

</Step>

<Step title="Enable native spoofing" icon="shield">

Call `FingerprintBuilder::native_spoofing(true)` so spoofing moves out of
JavaScript and into CDP and launch arguments where the browser can.

</Step>

<Step title="Align the egress IP" icon="globe">

Align the egress IP with the spoofed geolocation and timezone.

</Step>

<Step title="Try the pure CDP engine" icon="plug">

Try the [pure CDP engine](/tutorials/pure_cdp), which launches Chrome without
chromedriver at all.

</Step>

</Steps>

## Troubleshooting

<AccordionGroup>

<Accordion title="navigator.webdriver is still true" icon="wrench">

**Likely cause:** chromedriver not patched.

**Fix:** Run `sbrs patch-chromedriver --path` and verify `needs_patch()`.

</Accordion>

<Accordion title="Detection after patching" icon="flag">

**Likely cause:** Missing engine args.

**Fix:** Add `engine_spoofing_args()` to the launch flags.

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

## Related reading

<Columns cols={2}>

<Card color="#1a85d4" title="Fingerprint & Stealth Profiles" icon="fingerprint" href="/stealth/introduction">

A coherent `Fingerprint`: user agent, platform, screen size, WebGL vendor and geolocation.

</Card>

<Card color="#199f43" title="Binary Patching" icon="wrench" href="/stealth/advanced/binary-patching">

Backup, restore, and safety notes.

</Card>

<Card color="#d47628" title="CDP Mode" icon="bolt" href="/tutorials/cdp_mode" />

<Card color="#a631be" title="Pure CDP" icon="plug" href="/tutorials/pure_cdp">

Launches Chrome without chromedriver at all.

</Card>

</Columns>
