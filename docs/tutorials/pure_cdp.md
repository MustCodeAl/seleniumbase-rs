---
title: "Pure CDP: Chrome without a WebDriver"
---

# Pure CDP: Chrome without a WebDriver

`sb_cdp` drives Chrome directly over the DevTools Protocol. There is no
chromedriver, so there is none of the markers a WebDriver leaves in the page.
It is the engine behind the `cdp` MCP server.

## What you will learn

- Launch Chrome and open a page
- Find elements with a `Locator` that waits for them
- Fill forms, click, and press keys with trusted input
- Wait and assert with retrying expectations
- Work with frames, cookies, storage and tabs
- Add scripts and styles to a page, and a script to every document it loads
- Record a tab's network traffic as a HAR file
- Keep WebRTC from revealing the machine's addresses
- Test code that drives pages without launching Chrome

## Launch and navigate

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
let page = browser.default_page().await?;

page.goto("seleniumbase.io/simple/login").await?; // https:// is added for you
page.locator("#username").fill("demo_user").await?;
page.locator("#password").fill("secret_pass").await?;
page.locator("button").click().await?;
page.locator("h1").expect().to_contain_text("Welcome").await?;

browser.close().await?;
# Ok(())
# }
```

<Note>

`LaunchOptions::builder()` validates when you call `build()`: combining
`incognito` with `guest`, or `use_chromium` with an explicit
`executable_path`, is an error before anything launches. A `Proxy` with
credentials never shows its password in `Debug`, `Display` or the command line.

</Note>

## Locators

A [`Locator`] describes elements; it does not hold them. Every action finds the
element again and waits for it, so it keeps working when the page re-renders.

| Narrow with | Meaning |
| --- | --- |
| `.locator(sel)` | search inside the matches (and inside a frame, see below) |
| `.nth(i)`, `.first()`, `.last()` | pick one match |
| `.visible()` | keep only visible matches |
| `.parent()` | the parent element |
| `.with_timeout(d)` | how long actions wait |

Selectors are classified the way SeleniumBase does it: `"#id"` is CSS, `"//div"`
is XPath, `"link=Home"` is a link. Use `SelectorBuf::css`/`xpath` to be explicit.
A CSS selector may use SeleniumBase's `button:contains("Sign in")`, which runs as
XPath. To find any element by its text, use
`SelectorBuf::containing_text("Sign in", Some("button"))` (with `None` for the
tag, the innermost elements holding the text); a `text=` prefix means link text.

Reading: `text`, `html`, `attribute`, `count`, `info`, `bounding_box` (from the
top of the document), `viewport_rect` (from the window, as Python's
`get_element_rect`), `is_visible`, `exists`, `is_checked`, `urls`. `all` lists
the current matches; `all_waiting` waits for a first one. Acting: `click`,
`click_at`, `click_and_hold`, `double_click`, `right_click`, `hover`, `drag_to`,
`fill`, `type_text`, `set_value`, `clear`, `press`, `select_option`, `check`,
`submit`, `scroll_into_view`, `flash_with`, `screenshot`.

`fill` inserts the text in one step (fast, still real `input` events);
`type_text` sends one trusted key event per character, for pages that listen to
keys. A `\n` in `type_text` presses Enter.

## Waiting and asserting

```rust,no_run
# use seleniumbase_rs::sb_cdp::{Page, State};
# async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
page.locator(".spinner").wait_for(State::Absent).await?;
page.locator("#result").expect().to_have_text("42").await?;
page.locator(".error").expect().not().to_exist().await?;
page.expect().to_contain_url("/done").await?;
# Ok(())
# }
```

Expectations retry until they hold or the timeout passes; the error names the
locator and the state it was waiting for.

## Frames

A locator inside a frame is just a nested locator:

```rust,no_run
# use seleniumbase_rs::sb_cdp::Page;
# async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
page.locator("#payment").locator("button.pay").click().await?;
# Ok(())
# }
```

Same-origin frames nest to any depth and clicks land at the right pixel.

<Warning>

A cross-origin frame cannot be entered from page script, so it matches nothing.

</Warning>

## Cookies, storage, tabs

`page.cookies()` (`all`, `set`, `clear`, `header`, `save`, `load`),
`page.local_storage()` / `page.session_storage()`, `page.window()` and
`page.emulation()` (timezone, locale, geolocation, user agent, offline).
`browser.new_page`, `pages`, `newest_page` and `page.bring_to_front` manage
tabs. Cookie values are redacted in `Debug`.

## Downloads

The browser saves downloads into a folder you choose, and tells the page API
when each one begins and ends, so a download call returns the path of the saved
file or says why there is none:

```rust,no_run
use std::time::Duration;
use seleniumbase_rs::sb_cdp::Page;

# async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
// A longer wait for big files: the wait is bounded by the page's timeout.
let page = page.with_timeout(Duration::from_secs(60));
page.goto("https://example.com/reports").await?;
// Have the page fetch an address and save it (the page's cookies go along).
let csv = page.download_file("https://example.com/export?id=3", "downloaded_files").await?;
// Save it under a name you choose, replacing a file that is there.
page.save_file_as("https://example.com/export?id=3", "out/report.csv").await?;
// Click a link the server answers with `Content-Disposition: attachment`.
let invoice = page.download_by_click("a#invoice", "downloaded_files").await?;
// Wait for a file in the folder the browser saves to.
page.assert_downloaded_file("invoice.pdf", Duration::from_secs(10)).await?;
# let _ = (csv, invoice);
# Ok(())
# }
```

`download_file` names the file after the server's `Content-Disposition` header,
or the end of the address, and replaces a file already of that name (Chrome
does not number downloads that are controlled over DevTools). A bad status, a cancelled download or an address the
page may not fetch (CORS) is `SeleniumBaseError::Download`; a download that
does not begin or end in time is `WaitTimeout`. The download methods and
`browser.set_download_dir` choose the folder; until one is chosen the browser
saves wherever it would by default, and `browser.download_dir()` (what
`assert_downloaded_file` searches) is `downloaded_files`, as in SeleniumBase.

## Human-paced input

`page.human(behavior)` returns a `Human` whose clicks, typing and scrolling are
paced like a person's, for pages that watch how input arrives:

```rust,no_run
use seleniumbase_rs::sb_cdp::Page;
use seleniumbase_rs::stealth::behavior::Behavior;

# async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let person = page.human(Behavior::builder().typing_wpm(65.0).typo_rate(0.02).build()?);
person.type_text(&page.locator("#search"), "rust cdp").await?;
person.click(&page.locator("button[type=submit]")).await?;
person.scroll_by(600.0).await?;
# Ok(())
# }
```

- **The pointer** travels along a curve with a bell-shaped speed (slow start,
  fast middle, gentle arrival), for as long as Fitts's law says a movement of
  that distance to a target that size takes. It carries a faint tremor, and a
  long movement sometimes overshoots and corrects. A click lands somewhere
  inside the element, not on its exact centre, and the button is held for a few
  tens of milliseconds.
- **Typing** has Gaussian intervals around the chosen speed with a slowly
  drifting rhythm, longer pauses after spaces, punctuation and newlines, and the
  occasional hesitation. With a `typo_rate`, some letters are typed wrong,
  noticed, deleted and retyped; the field always ends up with exactly your text.
- **The pointer remembers** where it was, so successive movements start from
  there, and clones of a `Human` share it.

Everything is still trusted DevTools input; only the timing and the path
differ. It is slower than `locator.click()` by design. `Behavior::builder()
.seed(n)` makes the randomness reproducible. The planning is a pure module,
`stealth::behavior`, usable on its own.

## Messages in the page

`page.post_message`, `page.set_messenger_theme` and `page.activate_messenger`
show a short message in a corner of the page, the way Python SeleniumBase's CDP
Mode does with the Messenger library.

<Info>

The library and jQuery load from public CDNs, so the page needs network access;
`post_message` loads them itself when they are missing, and a navigation drops
them with the page.

</Info>

```rust,no_run
use std::time::Duration;
use seleniumbase_rs::sb_cdp::{
    MessageOptions, MessageStyle, MessengerLocation, MessengerSettings, Page,
};

# async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
page.set_messenger_theme(MessengerSettings {
    location: MessengerLocation::TopLeft,
    ..MessengerSettings::default()
})
.await?;
page.post_message(
    "Logged in",
    &MessageOptions {
        style: MessageStyle::Success,
        duration: Duration::from_secs(3),
        ..MessageOptions::default()
    },
)
.await?;
# Ok(())
# }
```

The message is escaped before it is posted, so `<b>` appears as typed rather
than as bold text. With `pause` left on (the default) the call returns after the
message's duration, so the next step does not start until it is gone.

## Scripts and styles in the page

`page.add_script_tag` and `page.add_style_tag` add a `<script>` tag or a
stylesheet to the current document, from a file (`TagSource::url`) or from the
code itself (`TagSource::content`). For a file they wait until the browser has
loaded it, up to the page's timeout, and a file that does not load is an error;
inline script code that throws is an error too. `page.add_init_script` runs a
script in every document the tab loads from now on, in every frame, before the
page's own scripts, and returns an identifier for `page.remove_init_script`.
They are Playwright's `addScriptTag`, `addStyleTag` and `addInitScript`; on
`BaseCase` the counterparts are `add_js_link`, `add_css_link` and `add_js_code`.

```rust,no_run
use seleniumbase_rs::sb_cdp::{Page, TagSource};

# async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
page.add_script_tag(TagSource::url(
    "https://cdn.jsdelivr.net/npm/jquery@3.7.1/dist/jquery.min.js",
))
.await?;
page.add_style_tag(TagSource::content("header { display: none; }"))
    .await?;

let id = page.add_init_script("window.startedAt = Date.now();").await?;
page.goto("https://example.com").await?; // runs before the page's scripts
page.remove_init_script(&id).await?; // the next documents do without it
# Ok(())
# }
```

A tag reaches only the current document, and a navigation drops it.
`add_init_script` leaves the document already loaded alone, as Playwright
does; run the script there with `page.evaluate` if it needs it too. The page's
Content Security Policy still applies: it can refuse a file's host, which is an
error, or forbid inline scripts, which stops `TagSource::content` code without
one. What you pass reaches the page as a string, never spliced into code.

## Intercepting requests

`page.intercept(rules)` pauses every request the page makes and answers it by
the first matching `Rule`: block it, serve a made-up response, or send it on
with changes. Requests no rule matches are untouched, and other tabs are never
affected.

```rust,no_run
use seleniumbase_rs::sb_cdp::{Page, ResourceType, Response, Rule};

# async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let interception = page
    .intercept(vec![
        Rule::block().resource_type(ResourceType::Image).resource_type(ResourceType::Font),
        Rule::fulfill(Response::json(200, &serde_json::json!({ "plan": "pro" }))).url("*/api/account"),
        Rule::modify().set_header("X-Test-Run", "42").url("https://example.com/*"),
    ])
    .await?;

page.goto("https://example.com").await?;
for seen in interception.log() {
    println!("{:?} {}", seen.outcome, seen.request.url);
}
interception.stop().await?; // or just drop it
# Ok(())
# }
```

A rule can match on a URL glob (`*` any run of characters, `?` one character),
an HTTP method, and a resource type; conditions combine with "and". `modify`
rules can set or remove headers, change the URL or method, and replace the body.
`interception.log()` lists the requests seen and how each was answered.

<Note>

`stealth::reactor::CdpReactor` is the older, browser-wide alternative: it adds
headers to every request from every tab. Prefer `page.intercept` unless you
want exactly that.

</Note>

## Recording a HAR

`page.start_har()` records everything the tab and its iframes request from
then on;
`finish()` stops and returns a `Har` (HTTP Archive 1.2), and `save` writes it
as a `.har` file that Chrome's and Firefox's developer tools, HAR viewers and
mitmproxy open.

```rust,no_run
use seleniumbase_rs::sb_cdp::{HarOptions, Page};

# async fn demo(page: Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let recorder = page.start_har().await?;
page.goto("https://example.com").await?;
let har = recorder.finish().await?;

for entry in &har.log.entries {
    println!("{} {} {} ms", entry.response.status, entry.request.url, entry.time);
}
har.save("example.har")?;

// With response bodies, each up to 256 KiB (the default limit is 1 MiB).
let recorder = page
    .start_har_with(HarOptions::new().content(true).max_content_bytes(256 * 1024))
    .await?;
# let _ = recorder;
# Ok(())
# }
```

Start the recorder before the navigation you want to see: only what happens
after `start_har` returns is recorded, and only for that tab. Each request is
an entry with its headers as they were sent (cookies included), query string,
POST body, status, response headers and cookies, server address and timings
(`blocked`, `dns`, `connect`, `ssl`, `send`, `wait`, `receive`, -1 for a phase
that did not happen). A redirect is one entry per hop; a failed request has
status 0 and the browser's error in `_error`, as in Chrome's own export; each
top-level navigation adds a page with its `onContentLoad` and `onLoad` times.
Bodies are off by default; with `content(true)` a body is fetched when it
finishes loading, binary ones as base64, and one over the limit is left out
with a `comment` saying so. The limit goes up to 64 MiB (`MAX_CONTENT_BYTES`),
because each body comes back whole in one DevTools message. `Har` and its
parts are serde types, so a saved HAR reads back with
`serde_json::from_str::<Har>`.

A cross-site iframe (an ad, a video player, a captcha) runs in a target of its
own, which reports its requests on its own session. The recorder attaches to
each one as it appears (`Target.setAutoAttach`), holds it until its requests
are reported, and records them with the tab's; iframes inside it too. Requests
of service workers, and of other workers that run as targets of their own, are
not recorded. If the recorder falls so far behind the browser that it misses
events, `log.comment` says so.

Recording enables the tab's `Network` domain and leaves it on, since other
features may rely on it. It attaches to the iframes only while it runs: finishing
or dropping the recorder detaches them, so run one recording per tab at a time.
A HAR records cookies and, with bodies, whatever the pages returned, so treat
the file like the session it came from. `tests/sb_cdp_har_chrome.rs` records a
visit to a local server, and a page with a cross-site iframe, in a real Chrome.

## Isolated contexts

`browser.new_context()` opens a `BrowserContext`: tabs that share no cookies,
storage or cache with anything else, like an incognito window. Open tabs in it
with `context.new_page(url)` and throw everything away with `context.dispose()`.
It is much cheaper than a second Chrome process.

### A proxy per context

Each context can have its own proxy, whatever the rest of the browser uses. A
proxy with a username and password is answered automatically, for that
context's tabs only:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, ContextOptions, Proxy};

# async fn demo(browser: Browser) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let context = browser
    .new_context_with(
        ContextOptions::new()
            .proxy(Proxy::parse("alice:s3cret@proxy.example.com:8080")?)
            .bypass("*.internal.example.com"),
    )
    .await?;
let page = context.new_page(Some("https://example.com")).await?;
# Ok(())
# }
```

The password is never sent to Chrome except in the reply to the proxy's own
prompt, and never appears in `Debug` output. A pool lease can have its own proxy
too: `pool.acquire_with(ContextOptions::new().proxy(...))`.

## Many workers: the browser pool

`BrowserPool` shares a few Chrome processes between many concurrent workers.
Each `acquire()` returns a `Lease`: its own isolated context and a tab.

```rust,no_run
use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let pool = BrowserPool::new(
    LaunchOptions::builder().headless(true).build()?,
    PoolOptions::builder().max_browsers(2).contexts_per_browser(4).build()?,
);

let worker = pool.acquire().await?;       // waits its turn if the pool is full
worker.page().goto("https://example.com/login").await?;
// ... sign in ...
worker.save_session("alice").await?;      // cookies and local storage, in memory
worker.release().await;                   // discards the context

let another = pool.acquire().await?;
another.page().goto("https://example.com").await?;
another.load_session("alice").await?;     // signed in, without logging in again
another.release().await;
pool.close().await;
# Ok(())
# }
```

- **Bounded and fair.** The pool never holds more than `max_browsers *
  contexts_per_browser` leases; waiting workers queue in order, and `acquire`
  gives up with a `WaitTimeout` after `acquire_timeout` (60 s by default).
- **Lazy, reused, replaced.** Chrome starts on the first `acquire`, is reused,
  and is replaced after `retire_after` leases (100 by default) so a long-running
  pool does not accumulate leaked memory. A Chrome that has died is replaced
  too.
- **Isolated.** Workers cannot see each other's cookies.
- **Sessions in memory.** `pool.sessions()` is a `SessionStore` shared by every
  lease. Nothing is written to disk and its `Debug` output never shows a value.
  Expired cookies are skipped on load, and local storage is restored only if the
  page is on the origin it was captured from.
- **Release it.** `lease.release().await` frees the slot at once. Dropping a
  lease also frees it, but in the background.

Use `BrowserPool::with_launcher` to supply browsers yourself, for example to
connect to existing ones or, with `test-util`, to hand out mocked browsers.

## Keeping WebRTC from leaking addresses

A page can open an `RTCPeerConnection` without asking, and the candidates it
gathers describe the machine's network. That is how a proxy or VPN gets
bypassed. `page.webrtc_report()` gathers candidates the way a tracking script
would (with no STUN server, so nothing leaves the machine) and tells you what a
page can learn:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let browser = Browser::launch(
    LaunchOptions::builder().shield_webrtc(true).build()?,
)
.await?;
let page = browser.default_page().await?;
assert!(page.webrtc_report().await?.is_clean());
# Ok(())
# }
```

- **By default** Chrome is left alone. On a typical machine the report lists
  `.local` host candidates, which stand in for the machine's private addresses
  but still tell a script that the machine is gathering candidates.
- **The shield** (`LaunchOptionsBuilder::shield_webrtc(true)`, or
  `page.shield_webrtc()` for a tab that is already open) makes every
  `RTCPeerConnection` relay-only and drops its STUN servers. Nothing is
  gathered, now or on any page the tab loads later. Pages that need a WebRTC
  call to connect will not, unless they bring a TURN relay.
- **Chrome's own flag** is available too: `webrtc_policy(WebRtcPolicy::...)`
  takes the same `WebRtcPolicy` a `Fingerprint` carries. On Chrome 155 none of
  its three values removes the `.local` candidates, so use the shield when the
  goal is for a page to learn nothing, and the flag when you only want to limit
  which addresses are offered.

## Looking like another browser

A `Fingerprint` says what a browser claims to be: user agent and Client Hints,
language, time zone, screen, location and the hardware a page can probe.
`Fingerprint::randomized(os, seed)` makes a coherent one. Hand it to the launch
options and every tab, as it opens and before its first navigation, is made to
match:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::{Fingerprint, OsType};

# async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
let fingerprint = Fingerprint::randomized(OsType::Windows, 42);
let browser = Browser::launch(
    LaunchOptions::builder().fingerprint(&fingerprint).build()?,
)
.await?;
let page = browser.default_page().await?;
page.goto("https://example.com").await?;
// A page now reads a Windows user agent, a Sydney time zone, and so on.
# Ok(())
# }
```

`page.apply_fingerprint(&fingerprint)` does the same for one tab, for the pages
it loads from then on (the page that is open keeps its old identity until it
navigates). It works on a tab inside an isolated `BrowserContext` too.

- A launch setting made before `.fingerprint(..)` wins over the fingerprint's
  (user agent, language, window size, WebRTC flag, proxy); one made after it
  replaces it.
- A fingerprint's proxy password is never sent to the browser as a header, which
  would give it to every site. The proxy is answered through its own challenge.
- The fingerprint's `cmd_params` are not passed to Chrome, because a flag such
  as `--renderer-cmd-prefix` runs a command and fingerprints often come from a
  profile file. Add flags you trust with `.arg(..)`.
- Checked on real Chrome 155: the page reads the fingerprint's user agent,
  platform, language, time zone, core count, screen width and WebGL renderer,
  and does not announce `navigator.webdriver`. The page can still tell what the
  fingerprint does not cover; the browser's real version, for one.

## Script errors

`page.js_errors()` lists the uncaught errors and unhandled promise rejections
the current document has raised, and `page.assert_no_js_errors()` fails on the
first. They are collected from the moment the document starts and cleared when
the tab navigates. A broken image is not a script error and is not listed.

## One helper for either engine

`Page` implements the same capability traits as `BaseCase` (`BrowserApi`,
`ElementApi`, `AssertionApi`, `ScreenshotApi`), so a helper written over them
runs on either engine:

```rust,no_run
use seleniumbase_rs::{AssertionApi, BrowserApi, ElementApi};

async fn sign_in<E>(sb: &mut E) -> Result<(), seleniumbase_rs::SeleniumBaseError>
where
    E: BrowserApi + ElementApi + AssertionApi,
{
    sb.open("https://example.com/login").await?;
    sb.type_text("#user", "alice").await?;
    sb.click("#submit").await?;
    sb.assert_title("Dashboard").await
}
```

On a `Page`, `quit` closes the tab, not the browser; call `Browser::close` for
that. `ScreenshotApi::save_screenshot` takes a bare file name and refuses one
that could leave the logs directory.

## CAPTCHAs

`page.solve_captcha()` attempts a Cloudflare Turnstile, reCAPTCHA, hCaptcha,
Friendly Captcha or DataDome widget with trusted mouse events and returns which
one it tried.

<Note>

The widgets live in cross-origin frames, so success is up to the site: check the
page afterwards.

</Note>

To test your own pages with the vendors' test keys, see
[Testing Pages with a CAPTCHA](captcha_testing.md).

Some challenge frames refuse events that did not come from the operating
system. With the `gui` feature, `page.gui_click_captcha()` is the same attempt
with the machine's own pointer, as Python's `gui_click_captcha` does with
PyAutoGUI:

```rust,no_run
# #[cfg(feature = "gui")]
# async fn demo(page: seleniumbase_rs::sb_cdp::Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
// Where the pointer would go, without moving it (and without a click):
if let Some(plan) = page.gui_captcha_plan().await? {
    println!("{} at {:?}", plan.captcha, plan.press);
}
// Move the real pointer there along a human path and click.
page.gui_click_captcha().await?;
# Ok(())
# }
```

It brings the window to the front, works out the checkbox's position on the
screen from the window (`Browser.getWindowForTarget`), the toolbar above the
page, the widget's frame and where each vendor draws its box, and takes over
the mouse for about a second. The click goes to whatever window is on top at
that point. A headless browser and a minimised window are refused, as is a
point outside the main display. The position is exact for a window at 100
percent zoom on an unscaled display; a page zoom or a docked browser panel
shifts it.

<Warning>

It is covered by unit tests and a mocked browser, and has not been tried on a
live CAPTCHA.

</Warning>

## Plugins

With the `plugins` feature, `LaunchOptionsBuilder::plugins` takes a set of
browser plugins: crates whose hooks run before the browser starts, once it runs,
for every tab and at close, to block resources, add scripts, set preferences or
load extensions. See [Browser Plugins](plugins.md). A `Page` is also the plugin
API's `PageHandle`, so code written for plugins can be given one of your tabs.

## Testing without Chrome

With the `test-util` feature, `Browser::new_mocked()` returns a browser backed by
a scripted `MockCtrl`. Every protocol command is recorded and answered by the
handler you register, so you can test what your code sends:

```rust,ignore
let (browser, mock) = Browser::new_mocked();
mock.reply("Runtime.evaluate", serde_json::json!({ "result": { "value": "Example" } }));
let page = browser.default_page().await?;
assert_eq!(page.title().await?, "Example");
assert!(!mock.calls_to("Runtime.evaluate").is_empty());
```

## Coming from Python

<Tip>

`sb.cdp.click(sel)` is `page.locator(sel).click()`; `sb.cdp.assert_text(...)` is
`locator.expect().to_contain_text(...)`; `sb.cdp.get_text` is `locator.text()`.
The full mapping of every `sb.cdp` method is in the
[API parity table](../parity.md).

</Tip>
