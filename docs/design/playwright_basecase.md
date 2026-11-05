---
title: "Design: Stealthy Playwright Mode on the BaseCase API"
sidebarTitle: "Playwright mode on BaseCase"
---

Status: design note, written before the code (branch `lane/design-playwright-basecase`).
Code it changes: `crates/playwright/`, `src/api/base_case.rs`, `src/browser/session/cdp.rs`,
`src/sb_cdp/endpoint.rs`. Breaking changes are fine (no users yet); nothing below keeps a shim.

## 1. Goal

The owner's words:

> With playwright you are expecting the user to do tasks using a playwright-like API, when
> really they should be able to use the same BaseCase API, and the Playwright API should only
> include APIs that are specific to Playwright and aren't covered by the BaseCase API.

In this project "the BaseCase API" already runs on both engines: the capability traits
`BrowserApi`, `ElementApi`, `AssertionApi` and `ScreenshotApi` are implemented by `BaseCase`
(WebDriver) and by `sb_cdp::Page` (pure CDP), "so a helper written once over the traits runs on
either engine" (`src/api/traits.rs:1-6`; the traits at `traits.rs:37-105`, the `BaseCase` impls
at `traits.rs:107-189`, the `Page` impls at `traits.rs:197-290`; re-exported at `src/lib.rs:193`).
Beyond the traits, `BaseCase` and `sb_cdp::Page` both carry the SeleniumBase extras the Python
Stealthy Playwright examples call (`solve_captcha`, `sleep`, waits, assertions).

So the redesign is: the user drives the stealthy browser with our API, as on every other page of
the docs, and asks for the rustwright page only for what our API does not do. The bridge wraps
no Playwright call. Python parity stays: Python SeleniumBase launches a stealthy CDP browser and
hands the user a real Playwright page through `connect_over_cdp(sb.get_endpoint_url())`
(`examples/cdp_mode/playwright/ReadMe.md` in the SeleniumBase repository); a Rust user who knows Playwright
still gets the real rustwright `Page`, but it is not the main road.

## 2. Today's public API (crate `seleniumbase-rs-playwright`)

Crate root, `crates/playwright/src/lib.rs`: `pub use rustwright;` (:68),
`pub use session::PlaywrightSession;` (:69), `pub use stealthy::StealthyPlaywright;` (:70).
`rustwright = "0.3"` (`crates/playwright/Cargo.toml:23`).

`PlaywrightSession` (`crates/playwright/src/session.rs:52`, fields: a `rustwright::Browser` and
one active `rustwright::Page`; a hand-written `Debug` at :58):

| Item | Signature | Line | What it does |
|---|---|---|---|
| `attach` | `pub async fn attach(browser: &sb_cdp::Browser) -> Result<Self, SeleniumBaseError>` | :78 | `connect_over_cdp(&browser.http_url())` |
| `connect_over_cdp` | `pub async fn connect_over_cdp(endpoint: &str) -> Result<Self, SeleniumBaseError>` | :90 | any `http://` or `ws://` DevTools address |
| `connect_with` | `pub async fn connect_with(options: ConnectOptions) -> Result<Self, SeleniumBaseError>` | :100 | rustwright's `ConnectOptions`; takes the first open page, else a new one |
| `browser` | `pub fn browser(&self) -> &Browser` | :124 | the rustwright browser |
| `page` | `pub fn page(&self) -> &Page` | :130 | the active rustwright page |
| `is_connected` | `pub fn is_connected(&self) -> bool` | :136 | |
| `with_page` | `pub async fn with_page<T, F>(&self, work: F) -> Result<T, SeleniumBaseError>` | :149 | runs a rustwright closure on the blocking pool |
| `new_page` | `pub async fn new_page(&mut self) -> Result<(), SeleniumBaseError>` | :166 | opens a page and makes it active |
| `goto` | `pub async fn goto(&self, url: &str) -> Result<(), SeleniumBaseError>` | :182 | wraps rustwright `goto` |
| `click` | `pub async fn click(&self, selector: &str) -> Result<(), SeleniumBaseError>` | :200 | wraps rustwright `click` |
| `type_text` | `pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), SeleniumBaseError>` | :215 | wraps rustwright `fill` |
| `get_text` | `pub async fn get_text(&self, selector: &str) -> Result<String, SeleniumBaseError>` | :232 | wraps rustwright `text_content` |
| `evaluate` | `pub async fn evaluate(&self, expression: &str) -> Result<Value, SeleniumBaseError>` | :248 | wraps rustwright `evaluate` |
| `screenshot` | `pub async fn screenshot(&self, path: &Path) -> Result<(), SeleniumBaseError>` | :264 | wraps rustwright `screenshot` |
| `close` | `pub async fn close(&self) -> Result<(), SeleniumBaseError>` | :281 | detaches; the browser keeps running |

Private: `blocking` (:295, `spawn_blocking` with panics turned into errors) and
`classify_element_error` (:307, rustwright errors to `ElementNotFound`, `ElementNotInteractable`,
`WaitTimeout`, `BrowserDisconnected`, else `Playwright`). Unit tests at :342 (error mapping),
:372 (refused endpoint), :388 (panicking work).

`StealthyPlaywright` (`crates/playwright/src/stealthy.rs:40`, `#[derive(Debug)]` at :39):

| Item | Signature | Line | What it does |
|---|---|---|---|
| `launch` | `pub async fn launch(options: LaunchOptions) -> Result<Self, SeleniumBaseError>` | :57 | `Browser::launch`, then `PlaywrightSession::attach`; closes the browser if the attach fails |
| `cdp` | `pub fn cdp(&self) -> &Browser` | :71 | the `sb_cdp::Browser` |
| `playwright` | `pub fn playwright(&self) -> &PlaywrightSession` | :77 | |
| `playwright_mut` | `pub fn playwright_mut(&mut self) -> &mut PlaywrightSession` | :83 | for `new_page` |
| `endpoint` | `pub fn endpoint(&self) -> DevToolsEndpoint` | :90 | `Browser::devtools_endpoint` |
| `close` | `pub async fn close(self) -> Result<(), SeleniumBaseError>` | :100 | detach, then close the browser |

Tests: `crates/playwright/tests/playwright_chrome.rs`, six `#[ignore]`d real-Chrome tests (:53
fingerprint reaches the client, :84 attach sees the open page and leaves the browser running,
:109 both APIs on one tab, :143 a Playwright page is as stealthy as the first, :176 missing
element is `ElementNotFound`, :204 the wrapped actions). Example:
`crates/playwright/examples/playwright_mode.rs` (`main` :54, `drive` :66), which drives the page
through the wrapped `goto`, `get_text`, `evaluate` and `screenshot`.

The guide (`docs/help/playwright_mode.md`) and the reference (`docs/api/playwright.md`) teach
those wrapped calls first: every Usage example (`playwright_mode.md:69-149`) and the "Page
actions" section (`api/playwright.md:138-205`).

## 3. What a user does in Playwright mode today, and what already covers it

Rows: the bridge's own calls, then what the Python Stealthy Playwright examples do with
`page.*`. "Covered" means the bridge should not offer it.

| Today | `sb_cdp` (pure CDP) | `BaseCase` | Verdict |
|---|---|---|---|
| `PlaywrightSession::goto`, `page.goto` | `Page::goto` (`page.rs:353`); `BrowserApi::open` (`traits.rs:39`, Page impl :199) | `BaseCase::open` (`base_case.rs:944`) | covered |
| `click`, `page.click` | `Locator::click` (`locator.rs:810`); `ElementApi::click` (`traits.rs:64`) | `BaseCase::click` (`base_case.rs:985`) | covered |
| `type_text` (rustwright `fill`), `page.fill` | `Locator::fill` (`locator.rs:945`); `ElementApi::type_text` clears, then types (`traits.rs:241`) | `BaseCase::type_text` (`base_case.rs:1002`) | covered |
| `page.type`, `press_sequentially` | `Locator::type_text`, one key at a time (`locator.rs:979`) | `BaseCase::type_text` | covered |
| `get_text`, `text_content`, `inner_text` | `Locator::text` (`locator.rs:676`); `ElementApi::get_text` (`traits.rs:73`) | `BaseCase::get_text` (`base_case.rs:1045`) | covered |
| `page.get_attribute` | `Locator::attribute` (`locator.rs:698`); `ElementApi::get_attribute` (`traits.rs:76`) | `BaseCase::get_attribute` (`base_case.rs:1344`) | covered |
| `evaluate` | `Page::evaluate` (`page.rs:196`), `Page::evaluate_as` (`page.rs:207`) | `BaseCase::execute_script` (`base_case.rs:1230`) | covered |
| `screenshot(path)` | `ScreenshotApi::save_screenshot` (`traits.rs:101`, Page impl :278); `Page::screenshot` bytes (`page.rs:877`) | `BaseCase::save_screenshot_to_path` (`base_case.rs:1623`) | covered |
| `new_page`, `browser.new_page()` | `Browser::new_page` (`browser.rs:925`), `new_page_at` (:937) | `BaseCase::open_new_tab` (`base_case.rs:803`) | covered |
| `with_page` for `title`, `page.title` | `Page::title` (`page.rs:635`) | `BaseCase::get_title` (`base_case.rs:1202`) | covered |
| `page.url` | `Page::url` (`page.rs:626`) | `BaseCase::get_current_url` (`base_case.rs:1207`) | covered |
| `page.content` | `Page::content` (`page.rs:644`) | `BaseCase::get_page_source` (`base_case.rs:1225`) | covered |
| `reload`, `go_back`, `go_forward` | `Page::reload` (:438), `back` (:463), `forward` (:472) | `BaseCase::refresh` (:958), `go_back` (:966), `go_forward` (:975) | covered |
| `page.wait_for_selector` | `Locator::wait_for` (`locator.rs:546`) | `BaseCase::wait_for_element` (`base_case.rs:1322`) | covered |
| `page.wait_for_function` | `Page::wait_for_function` (`page.rs:556`) | | covered |
| `page.wait_for_timeout(ms)` | `seleniumbase_rs::tokio::time::sleep` | `BaseCase::sleep(seconds)` (`base_case.rs:1814`) | covered |
| `locator(..).nth(i)`, `.count()`, `.first()` | `Locator::nth` (:220), `count` (:511), `first` (:229) | | covered |
| `expect(locator).to_have_text`, `to_be_visible` | `Locator::expect` (`locator.rs:1248`), `LocatorExpect::to_have_text` (`expect.rs:181`), `to_be_visible` (:131) | `BaseCase::assert_text` (:1446), `assert_element` (:1176) | covered |
| `expect(page).to_have_title`, `to_have_url` | `Page::expect` (`expect.rs:428`), `PageExpect::to_have_title` (:356), `to_have_url` (:387) | `BaseCase::assert_title` (:422) | covered |
| `page.route` | `Page::intercept` (`intercept.rs:372`) | | covered |
| HAR recording | `Page::start_har` (`har.rs:176`) | | covered |
| `page.pdf` | `Page::pdf` (`page.rs:889`) | | covered |
| Tracing (`context.tracing`) | `Page::start_trace` (`trace.rs:903`), `TraceRecorder::stop` (:1013): our folder and viewer | | covered (our format) |
| CAPTCHA (`sb.solve_captcha()` in the Python examples) | `Page::solve_captcha` (`captcha.rs:462`) | `BaseCase::solve_captcha` (`base_case_impl_mouse.rs:289`) | ours, not Playwright's |
| `StealthyPlaywright::endpoint`, Python `get_endpoint_url()` | `Browser::devtools_endpoint` (`browser.rs:811`), `Browser::http_url` (:782) | none today: proposed `BaseCase::devtools_endpoint` | covered for `sb_cdp`; add for `BaseCase` |
| `StealthyPlaywright::cdp` | the `Browser` from `Browser::launch` (`browser.rs:320`) | | covered |
| `get_by_role`, `get_by_label`, `get_by_test_id`; selector engines `role=`, `text=`, `:has-text()` | none (`get_by_role` appears in `src/` only in the importer) | none | Playwright-only |
| Playwright's own `trace.zip` for its Trace Viewer, video, `playwright.request`, `FrameLocator` | none found (no screencast in `src/sb_cdp/`) | none | Playwright-only, if rustwright's Rust API has them (see 4.5) |

Every method the bridge wraps today is covered. What stays Playwright's is what neither
`BaseCase` nor `sb_cdp` has, and the bridge reaches it through the raw rustwright page.

## 4. Proposal

### 4.1 What the user writes

Python's `sb_cdp` and `cdp_driver` formats are one format in Rust (our API is async already).
The main road is our page; Playwright joins when wanted:

```rust,no_run
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs::tokio;
use seleniumbase_rs::{AssertionApi, BrowserApi};
use seleniumbase_rs_playwright::PlaywrightSidecar;

#[tokio::main]
async fn main() -> seleniumbase_rs::Result<()> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let mut page = browser.default_page().await?;

    // The BaseCase API on the stealthy browser.
    page.open("https://example.com").await?;
    page.solve_captcha().await?;
    page.assert_text("Example Domain", "h1").await?;

    // Playwright only for what our API does not do, on the same tab.
    let playwright = PlaywrightSidecar::attach(&page).await?;
    let title = playwright.run(|pw| pw.title(Default::default())).await?;
    println!("{title}");

    playwright.close().await?; // detaches; the browser and the tab stay
    browser.close().await
}
```

Python's `SB(uc=True)` nested format is `BaseCase` in UC mode with the same sidecar:

```rust,ignore
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};
use seleniumbase_rs_playwright::PlaywrightSidecar;

let mut sb = BaseCase::new(BrowserConfig::default().with_mode(DriverMode::Uc)).await?;
sb.open("https://gitlab.com/users/sign_in").await?;
sb.solve_captcha().await?;
let playwright = PlaywrightSidecar::attach(&sb).await?; // the window `sb` is on
playwright.run(|pw| pw.title(Default::default())).await?;
sb.assert_element("#user_login").await?;
playwright.close().await?;
sb.quit().await?;
```

The same works inside the test fixtures: `SbCdp::run` hands the test a `&Page`
(`src/testing/fixture.rs:415`) and `Sb::run` a `&mut BaseCase` (`fixture.rs:264`), and either can
be passed to `PlaywrightSidecar::attach`. No new fixture.

The rustwright calls above (`title(Default::default())`) are the ones the crate compiles today
(`playwright_chrome.rs:232`); the docs use only calls the crate's own code or tests use.

### 4.2 The bridge's new surface

```rust,ignore
// crates/playwright/src/lib.rs
pub use rustwright;
pub use sidecar::PlaywrightSidecar;
pub use target::PlaywrightTarget;

/// A rustwright client attached beside one of our handles, on the tab that handle is on.
pub struct PlaywrightSidecar { /* rustwright::Browser, rustwright::Page */ }

impl PlaywrightSidecar {
    /// Attaches to the tab `target` is on (an `sb_cdp::Page` or a `BaseCase`).
    pub async fn attach(target: &impl PlaywrightTarget) -> Result<Self, SeleniumBaseError>;
    /// Attaches to any Chromium's DevTools endpoint; takes its first page.
    pub async fn connect(endpoint: &DevToolsEndpoint) -> Result<Self, SeleniumBaseError>;
    pub fn page(&self) -> &rustwright::Page;
    pub fn browser(&self) -> &rustwright::Browser;
    pub fn is_connected(&self) -> bool;
    /// Runs rustwright calls on the blocking pool (they block the thread they run on).
    pub async fn run<T, F>(&self, work: F) -> Result<T, SeleniumBaseError>
    where
        F: FnOnce(rustwright::Page) -> Result<T, rustwright::Error> + Send + 'static,
        T: Send + 'static;
    /// Detaches this client. The browser and its tabs stay; closing an already
    /// disconnected sidecar is `Ok`.
    pub async fn close(&self) -> Result<(), SeleniumBaseError>;
}

/// Sealed: implemented for `sb_cdp::Page` and `BaseCase` only. Its one method lives on the
/// private supertrait and returns `impl Future<Output = ...> + Send`, so `attach` stays `Send`.
pub trait PlaywrightTarget: sealed::Sealed {}
```

The library gains one method, the `BaseCase` half of Python's `get_endpoint_url()`:

```rust,ignore
impl BaseCase {
    /// The DevTools endpoint of the browser this case drives.
    pub async fn devtools_endpoint(&self) -> Result<DevToolsEndpoint, SeleniumBaseError>;
}
// with the same method on `BrowserSession` (src/browser/session/cdp.rs), which BaseCase forwards to.
```

- A direct CDP attachment (`BrowserConfig::with_cdp_url`, `config.rs:424`) answers its page's
  `browser().devtools_endpoint()` (`session/cdp.rs:61` holds that page).
- A WebDriver Chromium session reads `debuggerAddress` from its capabilities with
  `proxy_auth::debugger_address` (`src/browser/proxy_auth.rs:98`, already used for proxy logins
  at `session/connect.rs:186`) and resolves the WebSocket address with `sb_cdp::endpoint::resolve`
  (`src/sb_cdp/endpoint.rs:14`, made `pub(crate)`), the `/json/version` probe `Browser::connect`
  uses.
- A session without one (Firefox, Safari, a grid that hides it) gets `Unsupported`, like
  `BrowserSession::devtools` (`session/cdp.rs:37`); a quit session gets `SessionNotStarted`
  (`session/mod.rs:116-126`).

### 4.3 How `attach` finds the same tab

Today the bridge takes the browser's first page (`session.rs:105-115`), which is wrong as soon as
a second tab is open. `attach` gets two hints from the target and picks one rustwright page:

| Target | Endpoint | Tab id hint | URL hint | Attach timeout |
|---|---|---|---|---|
| `sb_cdp::Page` | `page.browser().devtools_endpoint()` | `Page::id()`, "the tab's DevTools target id" (`page.rs:95`) | `Page::url()` (`page.rs:626`) | `Page::timeout()` (`page.rs:112`) |
| `BaseCase` | `BaseCase::devtools_endpoint()` (new) | `BaseCase::get_current_window_handle()` (`base_case_impl_extra.rs:965`), when it answers | `BaseCase::get_current_url()` (`base_case.rs:1207`, works on both engines, `session/navigation.rs:54-60`) | `BaseCase::timeout_secs()` (`base_case.rs:324`) |

The timeout goes into rustwright's `ConnectOptions::new(..).timeout(..)`, which the crate already
calls (`session.rs:375`). The pick is a pure function, `pick_page(candidates, hints)`: a page
whose target id equals the id hint wins; else the one page whose URL equals the URL hint; else
`SeleniumBaseError::Playwright` naming the open URLs. It never guesses. Whether rustwright 0.3's
Rust `Page` reports its target id, and whether a chromedriver window handle equals the target id,
are checked in step 2 (rust-analyzer `hover` on `rustwright::Page`) and in the Chrome test; the
URL hint covers both if not.

### 4.4 Names, and the project patterns they follow

| New name | Pattern it follows |
|---|---|
| `PlaywrightSidecar` | `ProxyAuthSidecar`, "a DevTools client attached beside a WebDriver session", with `attach` and `close` (`proxy_auth.rs:120-128`, :139, :163). A session in this project owns a browser (`BrowserSession`, `session/mod.rs:48-49`); the bridge never does. |
| `attach(&target)` | `ProxyAuthSidecar::attach`; `CdpDriver::attach(browser)` (`src/api/cdp_driver.rs:91`). |
| `connect(&DevToolsEndpoint)` | `Browser::connect(host, port)`, "attaches to a browser that is already running" (`browser.rs:432-443`), and the `DevToolsEndpoint` type other clients already get (`endpoint.rs:221`). |
| `run(closure)` | `Sb::run` and `SbCdp::run` run a closure with the handle (`fixture.rs:264`, :415). `with_*` is the builder prefix here (`BrowserConfig::with_mode`, `config.rs:390`; `Page::with_timeout`, `page.rs:120`), so `with_page` breaks "one name, one meaning". |
| `close` (detaches) | `Browser::close` on an attached browser leaves it running and only detaches (`browser.rs:1679-1683`); `ProxyAuthSidecar::close` the same. |
| `PlaywrightTarget` | a sealed trait over our two engines, as `CaptchaSurface` is implemented for `Page` and `BaseCase` (`base_case_impl_mouse.rs:28`, `sb_cdp/mod.rs:112`). "Target" is the CDP word `Page::id` uses. |
| `BaseCase::devtools_endpoint` | `Browser::devtools_endpoint` (`browser.rs:811`): same name, same type, same meaning. |

No name is Playwright's: `goto`, `fill`, `connect_over_cdp`, `with_page` and `StealthyPlaywright`
leave the bridge's API.

### 4.5 What the crate keeps that is Playwright's own

Only the raw handles and the way to call them: `rustwright` re-exported, `page()`, `browser()`
and `run`. The bridge names no further Playwright feature, because none can be verified for the
Rust crate:

- Context7 (`/skyvern-ai/rustwright`, `docs/PARITY.md` and the READMEs) documents tracing
  (`Browser.start_tracing`, `stop_tracing`), `expect`, `Locator`, `FrameLocator`, `Video` and
  `Playwright.request` for rustwright's **Python** API. Its `rust-native/README.md` shows only
  `chromium().launch`, `new_page`, `goto(url, None)` and `title(None)`, and the Node README says
  tracing and locators are not bridged there.
- The crate pins `rustwright = "0.3"` and calls `goto(&url, GotoOptions::default())` and
  `title(Default::default())` (`session.rs:186`, `playwright_chrome.rs:232`), not the
  `None` arguments of that README: the Rust surface moves between versions, which is another
  reason not to wrap it.
- rustwright's README says its own stealth is partial and promises no undetectability. The
  stealth is the browser's; the guide says to attach late and detach early.

So the guide says: anything in section 3 marked Playwright-only is reached with `run` or
`page()`, as rustwright's own docs describe it.

### 4.6 Removed or renamed (no shims)

| Today | Becomes |
|---|---|
| `PlaywrightSession` (`session.rs:52`) | `PlaywrightSidecar` |
| `PlaywrightSession::attach(&sb_cdp::Browser)` (:78) | `PlaywrightSidecar::attach(&page)`; a `Browser` user calls `default_page()` first |
| `connect_over_cdp(&str)` (:90), `connect_with(ConnectOptions)` (:100) | `PlaywrightSidecar::connect(&DevToolsEndpoint)`; timeouts come from the target (4.3) |
| `with_page` (:149) | `run` |
| `goto`, `click`, `type_text`, `get_text`, `evaluate`, `screenshot` (:182-273) | removed: our API (section 3) |
| `new_page` (:166) | removed: `Browser::new_page`, then `attach` to it |
| `classify_element_error` (:307) | replaced by a selector-free mapping for `run`: closed, disconnected, target closed, crashed to `BrowserDisconnected`; timeouts to `WaitTimeout`; the rest `Playwright` |
| `StealthyPlaywright` and `launch`, `cdp`, `playwright`, `playwright_mut`, `endpoint`, `close` (`stealthy.rs:40-105`) | removed: `Browser::launch` plus `attach`; `Browser::devtools_endpoint` for the endpoint |
| `crates/playwright/src/session.rs`, `stealthy.rs` | `sidecar.rs`, `target.rs` |

Kept: `blocking` (`session.rs:295`) and its panic test (:388), the refused-endpoint test (:372,
now through `connect`), `SeleniumBaseError::Playwright` (`src/error.rs:84`), the crate name,
`publish = false`, the example name `playwright_mode` (so the `Dockerfile` skeleton needs no
change).

## 5. Docs plan

The guide leads with our API on the stealthy browser and brings Playwright in second.

- `docs/help/playwright_mode.md`: new order. (1) Intro and Installation (:1-57) stay, with the
  intro saying the stealthy browser is driven with the BaseCase API and Playwright joins on the
  same tab. (2) New "Use the BaseCase API on the stealthy browser": the first code block of 4.1
  and a user-facing copy of the section 3 table (Playwright call to our call). (3) New "Add
  Playwright for what only Playwright does": `PlaywrightSidecar::attach`, `run`, `page()`,
  `close`, attach late. (4) "The three Python formats" (:59-160) rewritten: `sb_cdp.Chrome()` and
  `cdp_driver.start_async()` to `Browser::launch` plus `default_page`; `SB(uc=True)` with
  `activate_cdp_mode()` to `BaseCase::new` in UC mode plus `attach(&sb)`; a browser something
  else started to `connect(&endpoint)`. (5) "Converting a regular Playwright script" (:162-193):
  first `sbrs import-playwright`, which already writes pure CDP Rust (`README.md:597`), then
  `run` for the statements it leaves as `// TODO(playwright)` (`src/utilities/importer/mod.rs:17`).
  (6) "Using all of rustwright" (:195-228) folds into (3). (7) Details, Proxy with authentication
  (the code opens the page with our API, then attaches), Notes and Troubleshooting (:230-339)
  renamed. The Visibility summary (:28-32) rewritten.
- `docs/api/playwright.md`: rewritten from 4.2: the crate (three items), `PlaywrightSidecar`
  (attach, connect, handles, run, close), `PlaywrightTarget`, errors, the re-export, See also;
  front matter `description` and `keywords` (:4-5) without `StealthyPlaywright`, `with_page`.
- `docs/api/cdp-uc.md`: a row for `devtools_endpoint()` next to `activate_cdp_mode()` (:26).
- `docs/stealth/advanced/playwright.md` (:21-33): our page opens the site and the sidecar
  evaluates `navigator.platform`.
- `docs/tutorials/headless.md` (:51, :105): the rows name `Browser::launch` and
  `PlaywrightSidecar::attach`.
- `README.md` (:431-442), `CLAUDE.md` (:46, :86), `context7.json` (:4, :35): the new names.
- `docs/changelog.md` and `CHANGELOG.md`: a "Breaking" entry.
- `parity/api.toml` `[cdp."get_endpoint_url"]` (:2297-2300): note that `BaseCase::devtools_endpoint`
  is the `SB()` format; regenerate `docs/parity.md`.
- `tests/offline/docs_current.rs` `GONE`: add `StealthyPlaywright`, `PlaywrightSession`,
  `.with_page(`, `playwright_mut(`; repoint the replacements at :75-92 that name them.
- `docs/adr/002-multi-driver-and-playwright.md`: one line pointing here (history stays).
- This note: `docs/docs.json` "Design notes" group and `docs/SUMMARY.md`.

## 6. Implementation steps

One commit per step, in this order. No step needs chromedriver except the dogfood run.

1. **`BaseCase::devtools_endpoint`** (library). Files: `src/browser/session/cdp.rs` (new
   `BrowserSession::devtools_endpoint` beside `devtools` at :37), `src/api/base_case.rs`
   (forwarder after `activate_cdp_mode` at :1235), `src/sb_cdp/endpoint.rs` (`resolve` to
   `pub(crate)`), `parity/api.toml`, `docs/api/cdp-uc.md`, `src/mcp/dev/api_index.json`
   (`SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index`).
   Tests, no browser: with `MockWebDriver::start(capabilities, ..)`
   (`base_case_impl_common.rs:791`) and a loopback thread serving `/json/version`: Chrome
   capabilities with `goog:chromeOptions.debuggerAddress` give that `http_url` and the advertised
   `websocket_url`; capabilities without it give `Unsupported`; a `/json/version` without
   `webSocketDebuggerUrl` gives the `CdpDriver` error of `endpoint.rs:55`. A direct CDP attachment
   built from `Browser::new_mocked` (`browser.rs:648`) answers `http://127.0.0.1:0`, as
   `tests/offline/sb_cdp_parity_mock.rs:355` expects of the mock.
2. **The sidecar** (bridge). Files: `crates/playwright/src/{lib.rs, sidecar.rs, target.rs}`;
   delete `session.rs` and `stealthy.rs`; `crates/playwright/Cargo.toml` dev-dependency
   `seleniumbase-rs` with `test-util`. Before writing `pick_page`, check with rust-analyzer
   (`set_workspace` to the lane, `hover` on `rustwright::Page` and `ConnectOptions`) what rustwright
   0.3 exposes for a page's target id. Tests, no browser: `pick_page` (id wins over URL; a unique
   URL wins; zero or two matches is an error naming the URLs); the `run` error mapping (the cases
   of `session.rs:342`, minus the selector); `blocking` panics (kept); a refused endpoint through
   `connect` (kept from :372); `PlaywrightTarget` for a page of `Browser::new_mocked` gives the
   mock's endpoint and `Page::id()`.
3. **One headless Chrome test**, replacing the six in `crates/playwright/tests/playwright_chrome.rs`
   (`#[ignore]`, `#[serial_test::serial(chrome)]`): launch headless with
   `Fingerprint::windows_desktop()`, serve two loopback pages, open the second with
   `Browser::new_page_at`, type into it with `ElementApi::type_text`; `attach(&second)` reads the
   typed value back through `run` (so it took the second tab, not the first), sees
   `navigator.platform == "Win32"` and `navigator.webdriver` false; rustwright navigates and
   `second.url()` follows; a `BaseCase` on the same browser through
   `BrowserConfig::with_cdp_url(browser.http_url())` attaches to its own tab; `close` leaves
   `second.title()` working; `close` again after `browser.close()` is `Ok`.
4. **The example** `crates/playwright/examples/playwright_mode.rs`: our API first (`open`,
   `get_text`, `Page::evaluate` for `navigator.webdriver`, `save_screenshot`), then one rustwright
   call through the sidecar, then `close` and `browser.close()`.
5. **Docs**, as in section 5, in the same branch.
6. **Gate and dogfood**: `cargo clippy -p seleniumbase-rs-playwright --all-targets -- -D warnings`,
   `cargo test -p seleniumbase-rs-playwright`,
   `cargo test -p seleniumbase-rs-playwright --test playwright_chrome -- --ignored --test-threads=1`,
   the library's step-1 tests, `cargo test --test offline docs_current`, then the repository gate
   in `CLAUDE.md`. Dogfood: port two Python examples once each against their sites, headless:
   `raw_nike_sync.py` (the `sb_cdp` road) and `raw_fingerprint_nested.py` (the UC `BaseCase`
   road, with chromedriver), and record what the sites showed.

## 7. Open questions for the owner

1. **The type's name.** `PlaywrightSidecar` (follows `ProxyAuthSidecar`), or keep
   `PlaywrightSession`? Recommendation: `PlaywrightSidecar`; "session" here means a handle that
   owns its browser, and this one never does.
2. **A one-call launcher.** Drop `StealthyPlaywright` entirely, or keep a launcher that returns
   our page with a lazily attached sidecar? Recommendation: drop it. Python has none
   (`sb_cdp.Chrome()` then `connect_over_cdp`), and `Browser::launch` plus `attach` keeps one road.
3. **A tab that cannot be matched.** If rustwright 0.3 does not report target ids and two tabs
   show the same URL, refuse or take the first? Recommendation: refuse with an error that names the
   open URLs; a silent wrong tab is worse than an error.
4. **Typed errors from `run`.** Map only disconnection and timeouts, or nothing at all?
   Recommendation: map those two families (they mean the same as everywhere else in the crate) and
   leave the rest `Playwright(text)`; without a selector the element errors cannot be named.
5. **`BaseCase` over pure CDP for every method.** On a `with_cdp_url` session most `BaseCase`
   methods answer `Unsupported` (`session/mod.rs:116-126`); making all of them work over CDP would
   make `BaseCase` itself, not only the four traits, the API on the stealthy CDP browser.
   Recommendation: a separate lane; this redesign needs only the traits and `sb_cdp::Page`, which
   exist now.
6. **An automated test of the UC road.** It needs chromedriver, which the one headless test
   avoids. Recommendation: no automated test in this lane; the dogfood run of step 6 and
   `tools/linux-check/chrome.sh` cover it, and a test follows if it ever breaks.
