---
title: "How the mobile web works"
sidebarTitle: "Mobile web"
description: "How phones change the web: mobile browsers and engines, responsive design with the viewport meta tag and media queries, CSS pixels and device pixel ratio, touch, device emulation (metrics, touch, mobile User-Agent), real devices versus emulation, and the mobile mode of seleniumbase-rs (--mobile, DeviceMetrics 390 by 715 at 3, BrowserConfig builders)."
---

<Info>

**In short:** a phone is a small, touch-driven browser on a slow network. Sites
adapt to it with a viewport tag and CSS media queries, and sometimes with a
different page for phones. A desktop Chrome can **pretend** to be a phone by
overriding its screen size, touch support and User-Agent. seleniumbase-rs does
that with its mobile mode, which defaults to a 390 by 715 screen at pixel ratio 3.

</Info>

## What is different on a phone

- **A small screen**, held upright or sideways, that changes shape when the
  address bar slides away or the on-screen keyboard opens.
- **Touch, not a mouse.** There is no pointer to hover with, fingers are
  imprecise, and gestures (swipe, pinch) exist.
- **Weaker hardware and networks.** A phone CPU is several times slower than a
  laptop, and the connection comes and goes.
- **Different browsers.** Chrome and Samsung Internet on Android use the Blink
  engine, Firefox on Android uses Gecko, and Safari on iPhone uses WebKit. On an
  iPhone, other browsers have traditionally been WebKit with a different
  skin. See [browser engines](/concepts/browser-engines).

The practical result: a page that works perfectly on a desktop Chrome can be
unusable on a phone, and a test that passes on a desktop does not show it.

## Making a page fit: responsive design

### The viewport meta tag

Early phones were handed pages made for desktops, so they pretended to have a
screen about 980 pixels wide and shrank the result to fit. A modern page tells
the browser to stop doing that:

```html
<meta name="viewport" content="width=device-width, initial-scale=1">
```

`width=device-width` means "lay the page out as wide as the real screen".
Without the tag, a phone draws the desktop layout tiny, and the CSS rules meant
for small screens never apply. Two viewports are involved: the **layout
viewport**, which CSS sizes things against, and the **visual viewport**, the part
you currently see, which changes when you pinch-zoom or when the keyboard opens.

### CSS pixels and the pixel ratio

A phone screen that is 1170 physical pixels wide is called 390 wide by the
browser. The unit in CSS (and in `window.innerWidth`) is the **CSS pixel**, and
the **device pixel ratio** (DPR, `window.devicePixelRatio`) says how many
physical pixels make one CSS pixel; here 3. A sharp image for that screen needs
three times the pixels in each direction, which is what `srcset` is for (see
[compression and caching](/concepts/compression-and-caching)).

### Media queries

CSS changes itself according to the device:

```css
/* Small screens first, then add rules as the space grows. */
.menu { display: none; }
.menu-button { display: block; }

@media (min-width: 768px) {
  .menu { display: flex; }
  .menu-button { display: none; }
}

/* Only where there is no hover, such as a touch screen. */
@media (hover: none) and (pointer: coarse) {
  .button { min-height: 44px; }
}
```

The ones to know: `width` (with `min-` and `max-`), `orientation` (`portrait` or
`landscape`), `resolution`, `hover`, `pointer`, `prefers-color-scheme`,
`prefers-reduced-motion`. Layout tools such as flexbox, grid and container
queries make a component adapt to the space it is given. Sites that serve
**different HTML** to phones (a separate mobile site, or the same URL with other
content) decide with the `User-Agent` and client hints; see
[detection](/concepts/detection).

### Touch

- Touch generates `touchstart` and `touchend`, and, for compatibility, mouse
  events after them. Modern code uses **pointer events** (`pointerdown`,
  `pointerup`), which cover mouse, pen and finger.
- Hover menus do not work, because there is nothing to hover with. A menu that
  opens on hover needs a tap alternative.
- Controls must be big enough to hit. WCAG 2.2 asks for at least 24 by 24 CSS
  pixels as a minimum and recommends 44 by 44 (see
  [accessibility](/concepts/accessibility)).
- A `click` still fires on a tap, so most pages work with a click in a test.

## Device emulation

Chrome's DevTools has a **device toolbar** that turns a desktop tab into a
phone. It is built from a few overrides, each a command of the
[DevTools Protocol](/concepts/cdp):

| What is changed | Command | Effect on the page |
|---|---|---|
| Screen size, pixel ratio and the `mobile` flag | `Emulation.setDeviceMetricsOverride` | `innerWidth`, `devicePixelRatio` and media queries change; the meta viewport tag is honoured; scrollbars become overlays |
| Touch input | `Emulation.setTouchEmulationEnabled` | `ontouchstart` exists, `maxTouchPoints` is above zero, `pointer: coarse` matches |
| User-Agent and client hints | `Emulation.setUserAgentOverride` | The `User-Agent` header and `navigator.userAgent` say "Android" and "Mobile"; `Sec-CH-UA-Mobile` becomes `?1` |

A typical Chrome user agent on an Android phone reads like this. The `Mobile`
word is the signal most servers look for:

```text
Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko)
Chrome/156.0.0.0 Mobile Safari/537.36
```

Other settings usually go with it: network and CPU throttling, a geolocation, an
orientation.

### Emulation or a real device?

| | Emulation in desktop Chrome | A real phone (or a device cloud) |
|---|---|---|
| Speed and cost | Instant, free, runs in CI | Slow to set up, costs money or hardware |
| Control from a test | Full, through the protocol | Through WebDriver with Appium, or Chrome remote debugging over USB for Android |
| Engine | Always Chromium | The real engine: WebKit on iPhone, Gecko on Firefox |
| Layout and CSS | Good for breakpoints and responsive rules | Exact, including safe areas and notches |
| Keyboard, address bar, gestures | Not reproduced | Real, and a classic source of bugs |
| CPU, memory, GPU | The desktop's, unless throttled | The real thing |

Use emulation to catch the large share of problems cheaply (the layout breaks at
390 pixels, a hover menu, a missing viewport tag) and a few real devices for what
only they can show. Do not assume that a pass in emulated Chrome means the page
works in iPhone Safari.

## How seleniumbase-rs uses it

seleniumbase-rs has a **mobile mode** that follows Python SeleniumBase's
`--mobile` option.

- `BrowserConfig::with_mobile(true)` turns Chrome's mobile emulation on with the
  default screen, `DeviceMetrics::default()`: **390 by 715 CSS pixels at a pixel
  ratio of 3** (Python's `constants.Mobile`), and an Android Chrome user agent
  (`MOBILE_USER_AGENT`).
- `BrowserConfig::with_device_metrics(metrics)` chooses another screen, and turns
  mobile on as well. It is the counterpart of Python's `--metrics`, also written
  `--device-metrics`, and of its `d_width`, `d_height` and `d_p_r` settings. Here
  setting metrics is enough, while Python applies them only together with
  `--mobile`.
- `DeviceMetrics::new(width, height, pixel_ratio)` builds the screen, and a text
  `"412,732,3"` parses into one (`FromStr`), in the `WIDTH,HEIGHT,RATIO` form
  Python's option takes. The three parts must be above zero, and the ratio may be
  fractional (`2.625`).
- On the command line, the `sbrs` tool has a global `--mobile` switch ("Use a
  mobile device emulation profile").

<Tabs>

<Tab title="Default phone" icon="mobile-screen">

```rust,ignore src/main.rs
use seleniumbase_rs::{run_browser_test, BrowserConfig, Result};

run_browser_test(BrowserConfig::default().with_mobile(true), async |sb| {
    sb.open("https://example.com").await?;
    let width = sb.execute_script("return window.innerWidth;").await?;
    assert_eq!(width, 390);
    Ok(())
})
.await
```

</Tab>

<Tab title="Another screen" icon="ruler-combined">

```rust,ignore src/main.rs
use seleniumbase_rs::browser::options::DeviceMetrics;
use seleniumbase_rs::{run_browser_test, BrowserConfig};

let metrics: DeviceMetrics = "412,732,3".parse()?;
let config = BrowserConfig::default().with_device_metrics(metrics);

run_browser_test(config, async |sb| {
    sb.open("https://example.com").await?;
    sb.assert_title("Example Domain").await
})
.await?;
```

</Tab>

<Tab title="Command line" icon="terminal">

```bash
sbrs --mobile
```

The switch is global: put it with any command that starts a browser. See
`sbrs --help` for the commands.

</Tab>

</Tabs>

### What happens underneath

The way the emulation reaches Chrome depends on the session.

- A normal WebDriver session passes Chrome's `mobileEmulation` experimental
  option (in `goog:chromeOptions`) with the device metrics and, when there is
  one, the user agent.
- In **undetected** mode that capability would give the session away, so the
  session sends the DevTools commands of the table above instead:
  `Emulation.setDeviceMetricsOverride` with `mobile: true`,
  `Emulation.setTouchEmulationEnabled` with five touch points, and
  `Emulation.setUserAgentOverride` carrying client hints that say "Android" and
  "mobile". See [detection](/concepts/detection).
- These overrides belong to a **tab**. The session sends them again for each
  window it opens or switches to, and `apply_mobile_emulation` on the browser
  session repeats them for a window you switched to some other way.

<Note>

The same three commands work on any DevTools session. In the pure CDP engine,
send them with `Page::execute`, for example
`Emulation.setDeviceMetricsOverride` with `width`, `height`,
`deviceScaleFactor` and `mobile: true`.

</Note>

<Warning>

Mobile mode changes what the page does. A site may redirect a phone User-Agent
to another host, hide its desktop menu, or serve a smaller bundle. Hover-only
controls cannot be reached. Write mobile tests against the mobile flow on
purpose, and keep desktop tests separate.

</Warning>

## Where to go next

<Columns cols={2}>

<Card title="Browser engines" icon="microchip" color="#d52c36" href="/concepts/browser-engines">
  Blink, WebKit and Gecko, and why the engine matters on a phone.
</Card>

<Card title="Detection and spoofing" icon="user-secret" color="#a631be" href="/concepts/detection">
  How sites read the User-Agent, client hints and touch support.
</Card>

<Card title="Rendering and performance" icon="gauge-high" color="#199f43" href="/concepts/rendering-and-performance">
  Throttling the CPU and network to test a slow device.
</Card>

<Card title="Accessibility" icon="universal-access" color="#1a85d4" href="/concepts/accessibility">
  Target sizes, zoom and screen readers on touch devices.
</Card>

</Columns>
