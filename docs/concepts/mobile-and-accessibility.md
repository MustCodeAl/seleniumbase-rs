---
title: "The mobile web and accessibility"
sidebarTitle: "Mobile web and accessibility"
description: "How phones change the web, and how browsers expose a page to people who cannot use it the usual way: mobile browsers, responsive design with the viewport meta tag and media queries, CSS pixels and device pixel ratio, touch, device emulation; the accessibility tree, ARIA roles, names and states, screen readers, keyboard navigation, WCAG basics, why role and name selectors are stable, axe-core audits, and the mobile mode, role selectors and audit_page tool of seleniumbase-rs."
---

<Info>

**In short:** two ways of using a page differ from a mouse on a big screen. A
phone is a small, touch-driven browser on a slow network, which sites adapt to
with a viewport tag and CSS media queries; a desktop Chrome can **pretend** to be
one by overriding its screen size, touch support and User-Agent, and
seleniumbase-rs does that with its mobile mode. Assistive technology reads a
second, simpler view of the page that the browser builds next to the picture, the
**accessibility tree**, where each node has a role ("button"), a name ("Sign
in") and states. Pages written with the right HTML give that tree for free, and
tests can use it too: finding an element by role and name is more stable than a
CSS class, and it fails when the page is inaccessible.

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
  skin. See [browser engines](/concepts/browser-architecture#what-an-engine-is).

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
[compression and caching](/concepts/bundling-and-caching#making-the-bytes-smaller)).

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
  [accessibility](#who-the-web-has-to-work-for)).
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

## Who the web has to work for

People use the web with a keyboard only, a screen reader that speaks the page, a
magnifier, voice control, a switch, or a high-contrast screen. Some of them
always do, some for a while (a broken arm), some in a moment (a phone in bright
sun, a noisy train). Accessibility ("a11y": a, eleven letters, y) is making sure
the page works for all of them. In many countries it is also a legal duty for
shops, banks and public services.

Assistive technology does not look at pixels. It asks the browser what is on the
page, and the browser answers from the **accessibility tree**.

## The accessibility tree

```mermaid
flowchart LR
    A["HTML and CSS<br/>what the author wrote"] --> B["DOM<br/>tree of elements"]
    B --> C["Accessibility tree<br/>role, name, state, value"]
    C --> D["Platform accessibility API<br/>UI Automation, AX, AT-SPI"]
    D --> E["Screen reader, magnifier,<br/>voice control, switch"]
```

*From markup to a screen reader. The browser does the middle steps.*

The accessibility tree is a filtered copy of the [DOM](/concepts/selectors-and-the-dom):
it leaves out what has no meaning for a person (a purely decorative `div`, an
element hidden with `display: none`) and adds what the DOM only implies. Take
this markup:

```html
<nav aria-label="Main">
  <a href="/shop">Shop</a>
</nav>
<main>
  <h1>Welcome</h1>
  <label for="email">Email</label>
  <input id="email" type="email" required>
  <button>Sign in</button>
</main>
```

The browser derives:

| Element | Role | Name | State |
|---|---|---|---|
| `nav` | navigation | "Main" | |
| `a` | link | "Shop" | |
| `main` | main | | |
| `h1` | heading, level 1 | "Welcome" | |
| `input` | textbox | "Email" | required |
| `button` | button | "Sign in" | |

You can see the tree in Chrome: DevTools, the Elements panel, the Accessibility tab.

## Roles, names and states

Every node answers three questions.

- **Role: what is it?** A button, a link, a heading, a checkbox, a dialog, a
  landmark such as `navigation` or `main`. HTML elements carry a built-in role
  (an **implicit role**). The `role` attribute can set one explicitly.
- **Name: what is it called?** The label a screen reader speaks. The browser
  works it out from the best source available: an `aria-labelledby` reference,
  then an `aria-label`, then the content or the element's own text, a
  `label` element for a form control, the `alt` of an image, the `title` as the
  last resort.
- **State and properties: what condition is it in?** Checked, expanded,
  disabled, selected, required, invalid, busy.

**ARIA** (Accessible Rich Internet Applications) is a set of attributes
(`role`, `aria-label`, `aria-expanded`, `aria-live` and many more) for cases
where HTML has no element for the job. Its first rule is to use a native element
when one exists.

| | Accessible | Not accessible |
|---|---|---|
| A button | `<button>Save</button>`: role, name, keyboard, focus, all built in | `<div class="btn" onclick="save()">Save</div>`: no role, cannot be focused, Enter does nothing |
| Hiding an icon's meaning | `<button aria-label="Close">x</button>` gives the name "Close" | `<button>x</button>` is announced as "x, button" |
| A toggle | `aria-expanded="true"` on the trigger, updated by script | A class that only changes colour |

<Warning>

ARIA changes only what is announced, never what the element does. Adding
`role="button"` to a `div` does not make it focusable or respond to the keyboard.
Bad ARIA is worse than none, because it promises something the page does not
deliver.

</Warning>

## Screen readers and the keyboard

A **screen reader** speaks the page (or sends it to a braille display) and lets
the user move through it without a mouse: NVDA and JAWS on Windows, VoiceOver on
macOS and iOS, TalkBack on Android, Orca on Linux. People jump from heading to
heading, list the links or the form fields, or move between landmarks. That is
why headings need to be real headings in a sensible order, and why a link that
says only "click here" is useless out of context.

The keyboard is the other half. Everything a mouse can do must work without one:

- **Tab** moves **focus** to the next interactive element, **Shift+Tab** goes back.
  The order follows the DOM, so reordering with CSS can make focus jump around.
- **Enter** follows a link or presses a button; **Space** presses a button or
  toggles a checkbox; **Escape** closes a dialog or menu; the **arrow keys** move
  inside one widget (tabs, menus, radio groups).
- The focus must be **visible**, so the user can see where they are. Removing the
  outline with `outline: none` and giving nothing back is a common failure.
- `tabindex="0"` puts an element in the tab order, `tabindex="-1"` allows focus
  from a script only. Avoid positive values.
- A modal dialog keeps focus **inside** while open and returns it to the trigger
  when closed. A "skip to content" link at the top spares keyboard users from
  tabbing through the whole menu on every page.

## WCAG in one table

The **Web Content Accessibility Guidelines** (WCAG) are the standard behind most
laws. Versions 2.1 and 2.2 are in use. Each requirement has a level: **A** (the
basics), **AA** (the usual target and legal requirement), **AAA** (the highest,
not expected everywhere). They fall under four principles, remembered as POUR.

| Principle | Idea | Examples |
|---|---|---|
| **P**erceivable | Information can be seen or heard by everyone | Text alternatives for images; captions; text contrast of at least 4.5 to 1 (3 to 1 for large text); content readable when zoomed to 200% |
| **O**perable | Everything can be used with different inputs | All functions by keyboard; no keyboard trap; visible focus; enough time; targets big enough to hit |
| **U**nderstandable | The page behaves predictably and helps with mistakes | Page language set; labels on every field; clear error messages |
| **R**obust | It works with current and future assistive technology | Valid markup; correct name, role and value for custom controls; status messages announced (`aria-live`) |

## Why accessible selectors are stable

Compare three ways to find the same button:

| Selector | Breaks when |
|---|---|
| `div.container > div:nth-child(3) > button` | A wrapper is added or the order changes |
| `.btn-primary.css-1x2y3z` | A designer renames a class or a build re-hashes it |
| **role "button" with the name "Sign in"** | The button stops being a button, or its label changes: both are changes a user would notice |

A role-and-name selector asks what a person would ask: "the Sign in button".
Developers can restyle, wrap and reorder freely without breaking the test.
When such a selector **cannot** find an element that is obviously there, the
page has a real defect: no label, a clickable `div`, a missing role. The test
turns into a free accessibility check. Playwright's `getByRole` and Testing
Library made this style popular for exactly that reason (see
[selectors and the DOM](/concepts/selectors-and-the-dom) for the other kinds of selector).

Its weak spots: the name is text, so copy changes and translations change it
(use an exact match, or test with a fixed language), and two buttons with the
same name need a parent to tell them apart.

## Automated audits with axe-core

**axe-core** is an open-source JavaScript engine from Deque that checks a page
against WCAG rules. It runs inside the page and returns a list of **violations**,
each with a rule id, an impact (minor, moderate, serious, critical), an
explanation and the nodes at fault. Lighthouse's accessibility score is built on
it (see [rendering and performance](/concepts/how-a-page-loads#from-bytes-to-pixels)).

An automated tool finds a real share of problems, such as a missing `alt`, a form
field without a label or low contrast, but not all: it cannot tell whether an
`alt` text is meaningful, or whether the focus order makes sense. A real audit
adds a keyboard pass and a screen reader pass.

## How seleniumbase-rs uses it

### Mobile mode

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

#### What happens underneath

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

### Role selectors and audits

seleniumbase-rs does not bundle axe-core. It has its own pieces, which cover
different ground.

#### Role and label selectors

`SelectorBuf::role(role, name, exact)` builds a selector for an ARIA role with an
optional accessible name, the counterpart of Playwright's `getByRole`.
`SelectorBuf::label(text, exact)` finds a form control by its label, as
`getByLabel` does.

<Warning>

These are **approximations**, written as XPath, because the engine does not ask
the browser for the real accessibility tree. An element matches a role through
its `role` attribute or as the native element for it (`button`, `a` with `href`,
`h1` to `h6`, `input type="checkbox"` and so on). The name matches the
`aria-label`, text, `value`, `title` or `alt`, or the text of its `label`. Not
followed: `aria-labelledby`, roles that depend on context, hidden elements, and
states such as `checked`. A test can still pass against markup that the real
tree would name differently.

</Warning>

```rust,ignore src/main.rs
use seleniumbase_rs::utils::selectors::SelectorBuf;

// Pure CDP engine: Page::locator takes anything that converts to a SelectorBuf.
page.locator(SelectorBuf::role("button", Some("Sign in"), false)).click().await?;
page.locator(SelectorBuf::label("Email", false)).fill("me@example.test").await?;
```

#### The audit_page tool

The MCP servers that have the DevTools tools (`cdp` and `sb`) offer
`audit_page`. It takes the page's HTML as it is now, after its scripts ran, and
runs `HtmlInspector` over it. It accepts `severity` (`all` or `error`) and
`limit` (50 by default, 500 at most) and returns the numbers of errors and
warnings, a count per rule, and the issues with the rule, message and a CSS
selector for the element. Its own `note` says what it cannot judge: colour
contrast, focus order and what a screen reader says. See [MCP](/concepts/mcp).

#### HtmlInspector in your own tests

`seleniumbase_rs::api::html_inspector::HtmlInspector::inspect(html)` is a static
checker with stable rule ids, so it can be diffed in CI. It does not run
JavaScript, so give it the DOM as the browser has it now.

| Rule | Severity | Reports |
|---|---|---|
| `document-lang` | error | `html` has no `lang` |
| `image-alt` | error | an image has no `alt` attribute |
| `interactive-name` | error | a link or button has no accessible name |
| `form-control-label` | error | an input, select or textarea has no label |
| `duplicate-id` | error | an `id` is used more than once |
| `invalid-aria-reference` | error | `aria-labelledby` or `-describedby` names a missing id |
| `heading-order` | warning | a heading level is skipped |
| `main-landmark` | warning | there is no `main` landmark |

The table shows the main rules; the module lists them all (for example
`skip-link`, `page-h1`, `table-headers`, `link-text`).

```rust,ignore src/main.rs
use seleniumbase_rs::api::html_inspector::HtmlInspector;

let html = sb
    .execute_script("return document.documentElement.outerHTML;")
    .await?;
let result = HtmlInspector::inspect(html.as_str().unwrap_or_default())?;
assert!(!result.has_errors(), "accessibility errors: {:?}", result.issues);
```

#### Running axe-core yourself

Load the axe-core file into the page and run it. Both engines can evaluate
JavaScript, and `Page::evaluate` on the pure CDP engine awaits the promise that
`axe.run()` returns.

```rust,ignore src/main.rs
let axe = std::fs::read_to_string("node_modules/axe-core/axe.min.js")?;
page.evaluate(axe).await?;
let violations = page
    .evaluate("axe.run().then(r => r.violations.map(v => ({ id: v.id, impact: v.impact, nodes: v.nodes.length })))")
    .await?;
assert_eq!(violations.as_array().map(Vec::len), Some(0), "{violations}");
```

<Tip>

Start with `HtmlInspector` for the cheap, repeatable checks in every run, add
axe-core for contrast and ARIA rules on key pages, and keep manual keyboard and
screen-reader passes for releases.

</Tip>

## Where to go next

<Columns cols={2}>

<Card title="Browser architecture" icon="microchip" color="#d52c36" href="/concepts/browser-architecture">
  Blink, WebKit and Gecko, and why the engine matters on a phone.
</Card>

<Card title="Detection and spoofing" icon="user-secret" color="#a631be" href="/concepts/detection">
  How sites read the User-Agent, client hints and touch support.
</Card>

<Card title="How a page loads" icon="gauge-high" color="#199f43" href="/concepts/how-a-page-loads">
  Throttling the CPU and network to test a slow device.
</Card>

<Card title="Selectors and the DOM" icon="code" color="#199f43" href="/concepts/selectors-and-the-dom">
  The tree the accessibility tree is built from, and the other selector kinds.
</Card>

<Card title="JavaScript and Web APIs" icon="puzzle-piece" color="#1a85d4" href="/concepts/javascript-and-events">
  Evaluating JavaScript in the page, which is how an axe run happens.
</Card>

<Card title="Crawlers and SEO" icon="magnifying-glass" color="#a631be" href="/concepts/crawlers-and-seo">
  Headings, titles and structure also help search engines.
</Card>

</Columns>
