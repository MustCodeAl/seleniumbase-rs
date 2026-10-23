---
title: "How Web APIs work"
sidebarTitle: "Web APIs"
description: "What Web APIs are and who defines them: the DOM, Fetch, Storage, Canvas, Geolocation, Notifications, Clipboard and Permissions; the observers (MutationObserver, IntersectionObserver, ResizeObserver, PerformanceObserver) and how they help a test wait for changes; how to read MDN and its compatibility tables; and how seleniumbase-rs evaluates JavaScript."
---

<Info>

**In short:** a Web API is a feature the browser offers to the JavaScript on a
page: read the document, make a request, store data, ask for the location. They
are not part of the JavaScript language; each browser provides them, and they
are described in public standards and on MDN. A test can call any of them with
one line of JavaScript, and the observer APIs are the best way to wait for a
page to change.

</Info>

## JavaScript and the browser are two things

The JavaScript language (called ECMAScript) gives you variables, functions,
arrays, promises and `Math`. It has no idea what a web page is. Everything that
touches the page or the outside world comes from the browser, as objects and
functions that already exist when your script starts:

- `window`, the global object, and `document`, the page (see
  [selectors and the DOM](/concepts/selectors-and-the-dom)).
- `navigator`, facts about the browser and the device.
- `fetch`, `localStorage`, `setTimeout`, `console`, `location`, `history`.

These are **Web APIs**. Each one is written down in a standard (mostly by the
WHATWG and the W3C) so that every browser implements it the same way. The same
JavaScript runs in a Node.js server without them: `document` does not exist
there. That is also why an automation tool has to run its script *inside* the
page (see [JavaScript and events](/concepts/javascript-and-events)).

## The APIs you meet most

| API | What it does | A taste | Notes |
|---|---|---|---|
| **DOM** | Read and change the page's tree of elements | `document.querySelector('h1').textContent` | Always available |
| **Fetch** | Make HTTP requests from a script | `await fetch('/api/items').then(r => r.json())` | Subject to CORS (see [network](/concepts/network)) |
| **Storage** | Keep data in the browser: `localStorage`, `sessionStorage`, IndexedDB, the Cache API | `localStorage.getItem('theme')` | See [sessions and state](/concepts/sessions-and-state) |
| **Canvas** | Draw pixels with code (2D, and WebGL for 3D) | `canvas.getContext('2d').fillRect(0, 0, 50, 50)` | No elements inside; a test sees a picture, not a tree |
| **Geolocation** | Ask where the device is | `navigator.geolocation.getCurrentPosition(ok, fail)` | Needs permission and a secure context |
| **Notifications** | Show a system notification | `new Notification('Done')` | Needs permission |
| **Clipboard** | Read and write copied text | `await navigator.clipboard.writeText('hi')` | Needs a secure context, a user action and often permission |
| **Permissions** | Ask whether a permission is granted, denied or still to be asked | `await navigator.permissions.query({ name: 'geolocation' })` | The answer is `granted`, `denied` or `prompt` |
| **History and URL** | Change the address bar without a reload; take addresses apart | `history.pushState({}, '', '/page/2')` | Used by single-page apps |
| **Workers** | Run scripts on another thread | `new Worker('job.js')` | Cannot touch the DOM |

Hundreds more exist (sensors, Bluetooth, payment, speech). New ones appear first
in Chromium, which is why a feature you rely on can work in your tests and fail
for a user on another browser.

### Secure contexts and permissions

Powerful APIs are limited in two ways.

- **Secure context.** They work only on `https://` pages (and on `localhost`, so
  developers can try them). On a plain `http://` page the object is missing.
- **Permission.** Many need the user to say yes, through a prompt that belongs to
  the **browser**, not to the page.

That second rule is a trap for automation. A prompt asking for the location is
browser interface, outside the page's DOM, so no selector can click "Allow". The
answer is to settle the permission before the page asks, with the
[DevTools Protocol](/concepts/cdp): `Browser.grantPermissions` grants a
permission to an origin, and `Emulation.setGeolocationOverride` sets the
position the page will receive. In a headless run with no answer, the request
usually just fails with the state `denied` or stays `prompt`.

## Observers: telling you when something changes

A modern page is never "finished". It loads data after the load event, adds
rows while you scroll and swaps parts of itself. Instead of asking the page
again and again, **observers** let you subscribe: the browser calls your
function when something happens.

| Observer | Calls you when | Typical use on a page | Typical use in a test |
|---|---|---|---|
| `MutationObserver` | The DOM changes: children added or removed, attributes or text changed | Update a widget when its container changes | Wait until a list gets its rows or a class appears |
| `IntersectionObserver` | An element enters or leaves the visible area (the viewport), by a fraction you choose | Lazy-load images, infinite scroll, "seen" tracking | Understand why content only appears after scrolling |
| `ResizeObserver` | An element's size changes | Responsive components, charts that redraw | Wait for a layout to settle |
| `PerformanceObserver` | A new performance entry is recorded: paint, largest-contentful-paint, layout-shift, long task, resource | Real-user monitoring | Read timings (see [rendering and performance](/concepts/rendering-and-performance)) |

All four work the same way: create the observer with a callback, call
`observe()` on a target, call `disconnect()` when done. They are asynchronous:
the callback runs after the change, batched, on the main thread.

### Waiting for a change

Two ways to wait for "the results are there":

- **Poll.** Ask every 100 ms or so, for example "does `#results` have children
  yet?", until yes or a timeout. It is simple, works anywhere, and is what the
  waits of most tools do.
- **Observe.** Let the browser tell you. It reacts at once and costs nothing
  between changes, but you write a small script.

A reliable observer script checks the state **first** (the change may already
have happened), then watches, and always has a timeout:

```js
new Promise((resolve, reject) => {
  const target = document.querySelector('#results');
  const done = () => target.children.length > 0;
  if (done()) return resolve(target.children.length);

  const timer = setTimeout(() => { observer.disconnect(); reject(new Error('no rows in 5 s')); }, 5000);
  const observer = new MutationObserver(() => {
    if (done()) { clearTimeout(timer); observer.disconnect(); resolve(target.children.length); }
  });
  observer.observe(target, { childList: true });
})
```

<Tip>

Infinite scroll is an `IntersectionObserver` waiting for a sentinel element to
come into view. If a test never scrolls, or the browser window is so tall that
everything is already visible, the next page loads in a different way than for
a real visitor. Scroll to the sentinel, then wait for new items.

</Tip>

## Reading MDN and its compatibility tables

**MDN Web Docs** is the reference for Web APIs. Learning to read a page pays off
every time you automate something new.

<Steps>

<Step title="Start from the summary and the box at the top" icon="flag">

The first lines say what the API is for. Badges mark the state of the feature:
**Experimental** (may change), **Deprecated** (do not start using it),
**Non-standard** (no standard backs it). A **Secure context** notice means it
only works on HTTPS. **Available in Web Workers** says whether a worker may use
it.

</Step>

<Step title="Read the syntax, parameters and return value" icon="code">

The syntax block shows the call. Pay attention to whether the call returns a
value, a promise (then you need `await`), or nothing and calls you back later.
The exceptions it can throw are listed, for instance `NotAllowedError` for a
refused permission.

</Step>

<Step title="Run the example" icon="play">

Most pages have live examples. Paste one into the browser console, or into your
test as a script, and see the real result before you wrap it.

</Step>

<Step title="Check the browser compatibility table" icon="table">

At the bottom, a table lists the first version of each browser that supports each
feature. Green means supported since that version. A flag or "Partial
implementation" means it is behind a setting or incomplete. A "Baseline" label
(widely available, newly available) says whether all major browsers support it.

</Step>

</Steps>

<Warning>

A feature that works in the Chrome that your tests drive may be missing in
Firefox or Safari, where your users are. Passing a test proves the page works in
that browser version, not that the API is safe to use everywhere. Check the
table before building a feature on a new API, and test with the browsers your
users have (see [browser engines](/concepts/browser-engines)).

</Warning>

## How seleniumbase-rs uses it

seleniumbase-rs has no wrapper for these APIs, and does not need one: you call
them by running JavaScript in the page. That is also how the library itself reads
much of the page.

<Tabs>

<Tab title="WebDriver engine" icon="plug">

`BaseCase::execute_script` runs a script and returns its value as JSON.
`BaseCase::evaluate` is an alias for it.

```rust,ignore src/main.rs
let theme = sb.execute_script("return localStorage.getItem('theme');").await?;
let online = sb.execute_script("return navigator.onLine;").await?;
```

</Tab>

<Tab title="Pure CDP engine" icon="bolt">

`sb_cdp::Page::evaluate` runs an expression and **awaits promises**, so a
Promise built from an observer works as it is. `Page::evaluate_as` also turns the
result into a Rust type with serde. A script that throws comes back as
`SeleniumBaseError::CdpDriver`. A `return` that starts the last line is dropped,
so a Selenium-style script still works; a `return` anywhere else needs the
script wrapped in an arrow function that is called.

```rust,ignore src/main.rs
let rows: u64 = page
    .evaluate_as(
        r#"new Promise(resolve => {
             const target = document.querySelector('#results');
             const done = () => target.children.length > 0;
             if (done()) return resolve(target.children.length);
             new MutationObserver((_, observer) => {
               if (done()) { observer.disconnect(); resolve(target.children.length); }
             }).observe(target, { childList: true });
           })"#,
    )
    .await?;
```

</Tab>

<Tab title="MCP server" icon="robot">

The browser-driving MCP servers have a `run_javascript` tool that takes an
`expression`, so an assistant can read `navigator`, `localStorage` or a
`PerformanceObserver` result from the live page. See [MCP](/concepts/mcp).

</Tab>

</Tabs>

<Note>

The result of a script must be JSON-representable. A DOM element comes back as an
empty object `{}`, so return what you need from it (`el.textContent`,
`el.getBoundingClientRect().toJSON()`), not the element itself.

</Note>

<Accordion title="What about permissions in a test?" icon="shield">

Grant them before the page asks. With either engine you can send the DevTools
command by name, for example `Browser.grantPermissions` with the origin and a
list of permission names. On the WebDriver engine this is
`BaseCase::execute_cdp_with_params`. See [the DevTools Protocol](/concepts/cdp).

</Accordion>

## Where to go next

<Columns cols={2}>

<Card title="JavaScript and events" icon="bolt" color="#1a85d4" href="/concepts/javascript-and-events">
  The event loop, listeners and how scripts run on a page.
</Card>

<Card title="Selectors and the DOM" icon="code" color="#199f43" href="/concepts/selectors-and-the-dom">
  The first and biggest Web API, and how to point at its elements.
</Card>

<Card title="Rendering and performance" icon="gauge-high" color="#d47628" href="/concepts/rendering-and-performance">
  What PerformanceObserver reports, and why pages feel slow.
</Card>

<Card title="Accessibility" icon="universal-access" color="#a631be" href="/concepts/accessibility">
  The accessibility tree the browser builds from the DOM.
</Card>

</Columns>
