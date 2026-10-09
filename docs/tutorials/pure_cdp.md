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

`LaunchOptions::builder()` validates when you call `build()`: combining
`incognito` with `guest`, or `use_chromium` with an explicit
`executable_path`, is an error before anything launches. A `Proxy` with
credentials never shows its password in `Debug`, `Display` or the command line.

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

Reading: `text`, `html`, `attribute`, `count`, `info`, `bounding_box`,
`is_visible`, `exists`, `is_checked`, `urls`. Acting: `click`, `click_at`,
`double_click`, `right_click`, `hover`, `drag_to`, `fill`, `type_text`,
`set_value`, `clear`, `press`, `select_option`, `check`, `submit`,
`scroll_into_view`, `screenshot`.

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

Same-origin frames nest to any depth and clicks land at the right pixel. A
cross-origin frame cannot be entered from page script, so it matches nothing.

## Cookies, storage, tabs

`page.cookies()` (`all`, `set`, `clear`, `header`, `save`, `load`),
`page.local_storage()` / `page.session_storage()`, `page.window()` and
`page.emulation()` (timezone, locale, geolocation, user agent, offline).
`browser.new_page`, `pages`, `newest_page` and `page.bring_to_front` manage
tabs. Cookie values are redacted in `Debug`.

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

`stealth::reactor::CdpReactor` is the older, browser-wide alternative: it adds
headers to every request from every tab. Prefer `page.intercept` unless you
want exactly that.

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

## CAPTCHAs

`page.solve_captcha()` attempts a Cloudflare Turnstile, reCAPTCHA, hCaptcha,
Friendly Captcha or DataDome widget with trusted mouse events and returns which
one it tried. The widgets live in cross-origin frames, so success is up to the
site: check the page afterwards.

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

`sb.cdp.click(sel)` is `page.locator(sel).click()`; `sb.cdp.assert_text(...)` is
`locator.expect().to_contain_text(...)`; `sb.cdp.get_text` is `locator.text()`.
The full mapping of every `sb.cdp` method is in the
[API parity table](../parity.md).
