---
title: "Design: BaseCase over pure CDP"
sidebarTitle: "BaseCase over CDP"
---

Status: planned and built on `lane/basecase-cdp`, one group per commit; section 7 records what the
code found and what stayed refused. Answers open question 5 of
[the Playwright design note](/design/playwright_basecase).
Code it changes: `src/browser/element.rs`, `src/browser/session/`, `src/api/base_case.rs` and
`src/api/base_case_impls/`. Breaking changes are fine (no users yet); nothing below keeps a shim.

## 1. Goal

On a `BaseCase` whose browser is a direct DevTools attachment (`BrowserConfig::with_cdp_url`,
`with_cdp_discovery`: the stealthy browser, a Tauri or Electron window), the whole `BaseCase` API is
the API, as Python SeleniumBase's CDP Mode gives `sb.cdp` the methods of `sb`:

- a method with a CDP form runs through the pure CDP engine (`sb_cdp::Page`, `Locator`,
  `Browser`);
- a method with no CDP form refuses with an error that names the method and the alternative,
  never a panic and never a silent no-op.

## 2. What happens today

A direct attachment is a `BrowserSession` with `external_page: Option<sb_cdp::Page>` and no
`WebDriver` (`session/connect.rs`, `connect_cdp`). Three things make most of `BaseCase` answer
`SeleniumBaseError::Unsupported`:

1. `BrowserSession::webdriver()` returns `Unsupported` ("this needs the WebDriver engine ...") for
   such a session, and about 130 call sites (the session's own methods, and `BaseCase` methods that
   use the driver directly) go through it. Only `goto`, `current_url`, `current_title`,
   `execute_script`, `execute_cdp*`, `devtools_endpoint` and the `cdp_*` helpers of
   `api/cdp_page.rs` answer.
2. `BrowserSession::find` returns an `Element`, and `Element` holds a thirtyfour `WebElement` and
   `WebDriver`. Everything that finds an element first (click, type, text, attribute, the waits,
   the assertions, the highlight of demo mode) therefore fails.
3. The WebDriver model has state the DevTools Protocol has none of: the current frame, the current
   window, an open dialog.

## 3. The four seams

| Seam | Change |
|---|---|
| `Element` | Becomes two-engined: the WebDriver node it is, or on a CDP session a `Locator` (the page's own lazy handle). The methods keep their names (the type's docs already say they are those of `Locator`). On CDP the element looks again on every use, so it is never stale; a match found by `find_all` is pinned with `Locator::nth`. `as_webdriver_element` and `into_webdriver_element` answer `Option` (`None` on CDP). |
| `BrowserSession` | Every method that asked `webdriver()` first asks `pure_cdp_page()` and answers through the page when there is one: navigation, source, scripts, screenshots, cookies, windows. The guard `self.webdriver()?;` at the head of the waits goes: `find` decides. |
| Session state | A CDP session keeps what WebDriver keeps for it: the frame scope (the iframe selectors entered, as a locator prefix), the tab it is on (a target id), and the last JavaScript dialog (a `Page.javascriptDialogOpening` listener). |
| `BaseCase` direct sites | The methods that call `self.session.webdriver()` themselves (windows, alerts, cookies, offset clicks, `press_key`, `set_value`, link lookups, the element scripts) are routed through a session or `Element` method that has both forms, or refuse by name. |

## 4. Groups: today and planned

"Today" is what the method answers on a CDP session before this lane. "Route" is the CDP form.

| Group (examples) | Today | Route, or refusal |
|---|---|---|
| Navigation: `open`, `get_url`, `get_title`; `go_back`, `go_forward`, `refresh`, `get_page_source` | the first three work; the others `Unsupported` | `Page::back`, `forward`, `reload`, `content`. |
| Find and click: `find_element`, `find_elements`, `click`, `double_click`, `context_click`, `hover`, `click_link*`, `click_if_visible`, `check_if_unchecked`, `submit` | `Unsupported` | `Element` over `Locator` (`click`, `double_click`, `right_click`, `hover`, `submit`, `set_checked`); link text through the `link=` selector. |
| Typing and keys: `type_text`, `add_text`, `send_keys`, `clear`, `press_key`, `set_value`, `select_option_by_*` | `Unsupported` | `Locator::fill`, `type_text`, `send_keys`, `clear`, `press`, `select_option`; `press_key` through `Page::keyboard`; `set_value` through `Locator::set_value`. |
| Waits and asserts: `wait_for_element*`, `wait_for_text*`, `assert_element*`, `assert_text*`, `assert_title`, `assert_attribute` | `Unsupported` (the `webdriver()` guard, then `find`) | Unchanged code on top of `find` and `Element`; the guard goes. |
| Text and attributes: `get_text`, `get_attribute`, `get_property`, `get_text_content`, `is_element_visible`, `is_enabled`, `is_selected`, `is_checked`, `is_attribute_present` | `Unsupported` | `Locator::text`, `attribute`, `info`, `is_visible`, `is_enabled`, `is_selected`, `is_checked`; a property and `getAttribute` as Selenium has it through a page script run with the element bound. |
| Windows and frames: `switch_to_frame`, `switch_to_parent_frame`, `switch_to_default_content`, `get_window_size`, `set_window_size`, `maximize_window`, `get_window_position`, tabs and handles | `Unsupported` | Frames: the session keeps the entered iframe selectors and finds through them (`locator(frame).locator(selector)`, which the engine already searches inside). Window: `Page::window()` (`Browser.getWindowForTarget`, `setWindowBounds`). Tabs: handles are target ids; `Browser::pages`, `new_page`, `bring_to_front`, `Page::close`. |
| Cookies and storage: `get_cookies`, `add_cookie`, `delete_cookie`, `clear_all_cookies`, `save_cookies`, `load_cookies`, `get_cookie_string`; local and session storage | cookies `Unsupported`; storage already works (page scripts) | `Page::cookies()` (`all`, `set`, `clear`, `header`); the same `Cookie` type as the WebDriver form. |
| Screenshots and scripts: `save_screenshot`, `screenshot_as_png`, `save_element_as_image_file`, `execute_script`, `execute_async_script` | `execute_script` works; screenshots and async scripts `Unsupported` | `Page::screenshot`, `Locator::screenshot`; the async script through `Runtime.evaluate` with `awaitPromise`. |
| Alerts: `accept_alert`, `dismiss_alert`, `get_alert_text`, `type_alert_text`, `switch_to_alert`, `is_alert_present`, `wait_for_and_accept_alert` | `Unsupported` | The session listens for `Page.javascriptDialogOpening` and answers with `Page.handleJavaScriptDialog`; a call with no dialog open fails as WebDriver's `no such alert` does. |
| Files and downloads: `assert_downloaded_file`, `get_downloaded_files`, `download_file`, `install_addon` | `Unsupported` | `Page::download_*` and the downloads folder the engine sets; `install_addon` and `uninstall_addon` are Firefox WebDriver commands: they refuse and name `sb_cdp` extension loading (`LaunchOptions`). |
| Mouse: `click_at_offset`, `drag_and_drop_by_offset`, `click_chain`, `cdp_mouse_click`, `cdp_type_text` | `Unsupported` | `Page::mouse()` (`Input.dispatchMouseEvent`) and `Locator::click_with_offset`, `drag_to`. |
| Tours, presentations, charts, messenger, jQuery Confirm, demo-mode highlights | the page-script ones work (`execute_script`); the ones that find an element first fail | Unchanged code: they run page scripts and, through `Element`, find. |
| Recorder Mode, `get_chromedriver_version`, `cdp_events` (the chromedriver performance log), multiple drivers | `Unsupported` | Refused, naming the alternative: Recorder Mode and `cdp_events` need chromedriver (use `Page::events` and the `sb_cdp` recorder of `sbrs`); there is no chromedriver to ask for its version (`get_browser_version`). |

## 5. Names and style

- `BaseCase` keeps Python's `get_*` names (the owner's decision); nothing is renamed.
- `Element` keeps its method names; its two engines are an enum private to the module.
- An error for a method with no CDP form is `SeleniumBaseError::Unsupported`, and the text begins with
  the method, says it needs the WebDriver engine, and names the alternative.

## 6. Tests

- Offline: a mock session (`Browser::new_mocked`, `MockCtrl`) behind `BaseCase::with_session`; each
  group asserts the protocol commands sent and how the answers become results and errors.
- Chrome: headless, one binary at a time (`tests/chrome/basecase_cdp_chrome.rs`), a local fixture
  page (`tests/fixtures`): a `Browser::launch` browser, then `BaseCase::new` with `with_cdp_url`
  on its endpoint.
- The UC road of the Playwright sidecar: a Chrome test that attaches the sidecar to a chromedriver
  session and compares the window handle with the CDP target id. It needs a chromedriver that matches
  the installed Chrome (`sbrs install chromedriver`).

## 7. What the code found

Filled in with each group (see the commit messages) and at the end of the lane.
