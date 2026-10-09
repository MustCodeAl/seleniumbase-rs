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
