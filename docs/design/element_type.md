---
title: "Design: an element type of our own for the WebDriver engine"
sidebarTitle: "An element type of our own"
---

Status: implemented on `lane/element-type`, with every recommendation accepted. Written
before the code (branch `lane/design-element`, from `90ca478e`), and kept as it was, so the
text below describes the plan; two things turned out differently. The conversion of a
`SelectorBuf` to a `By` is `shadow_find::by_of` (with `checked`, which the `BaseCase` sites call
where they called `Selector::auto(css).to_by()?`), and the suspected bug of
`From<&str> for SelectorBuf` (`:contains` turned into XPath before `::shadow` is looked for) does
not occur: the CSS-to-XPath converter reads `::` as an empty pseudo-class name and refuses, so a
shadow selector stays CSS (a test pins it).
Every line number below is from that commit. What `thirtyfour` does was read on Context7
(`/websites/rs_thirtyfour`, the `WebElement` pages); what those pages did not show is either
taken from code this crate already compiles (cited) or marked "to verify". The call-site counts
are literal-text counts over the indexed sources (a `.method(` search per name), not a
type-resolved caller graph.

A review of the branch changed four more things, which the text below does not show. A search from
an `Element` stays inside it: an XPath that starts at the root of the page (`//td`) is refused,
since the WebDriver would search the whole page from any element, and a `:contains()` selector is
read as `.//p[...]`; an `<iframe>` element is not entered (its frame is a different document).
`drivers::Key`, thirtyfour's, is removed, so `Key` has one meaning, the pure CDP engine's
(open question 5). `BaseCase::get_parent` and `get_element_at_x_y` name their elements
(`#a >> ..`, `element at (10, 20)`) where a temporary marker selector had been the name. Python's
`locator` is mapped to `find_element` by hand in the importer, since its parity row is `composed`.

Three follow-up fixes came after (the `element-followups` lane): `BaseCase::get_text` gives a
field's value (open question 11 below); a union is refused when any branch starts at the root
(`.//a | //b`, `(.//a | //b)[1]`, a branch that calls `id()`); and the XPath of `SelectorBuf::role`,
`text`, `label` and `containing_text` starts with `.//`, so they work from an `Element`.


## Goal

`BaseCase` and `BrowserSession` hand users `thirtyfour::WebElement` (through the re-export
`seleniumbase_rs::drivers::WebElement`, `src/drivers.rs:45-48`) and take `thirtyfour::By`. Users
should use this project's API, docs and examples should import only our crates, and names
should follow this project's patterns. So:

* the WebDriver engine returns an element type of its own, `seleniumbase_rs::Element`, whose
  methods carry the names of the pure CDP engine's `sb_cdp::Locator` where the idea is the same;
* every method that takes a selector takes this crate's selector types (`&str`, `SelectorBuf`,
  `Selector`), never a `By`;
* the raw `thirtyfour` element stays reachable through one explicit escape hatch.

Breaking changes are fine (no users yet, no shims).

## The answer in short

* **One new type, `Element`** (`src/browser/element.rs`, re-exported at the crate root beside
  `BrowserSession`, `src/lib.rs:196`, and `Selector`, `src/lib.rs:224`). It is one element the
  browser found, at the moment it was found: the same idea as Python's `WebElement`, and not the
  idea of `sb_cdp::Locator`, which finds again on every use (`src/sb_cdp/locator.rs:58-67`).
  It follows the pattern of the crate's existing WebDriver handle, `Alert`
  (`src/api/switching.rs:372-389`): it holds the session's `WebDriver`, it is `Clone`, and its
  `Debug` is written by hand.
* **The 17 `BaseCase` methods and the 6 `BrowserSession` methods that return a `WebElement`
  return `Element`** (2 of the 17 go away, see open questions 2 and 3).
* **The 26 `BrowserSession` methods that take a `By` take `selector: impl Into<SelectorBuf>`**,
  as `Page::locator` (`src/sb_cdp/page.rs:133`) and `Locator::locator` (`locator.rs:212`) do.
  `Selector::to_by` and `Selector::auto_by` (`src/utils/selectors.rs:212`, `:226`) leave the
  public API.
* **The escape hatch** is `Element::as_webdriver_element(&self) -> &drivers::WebElement` and
  `Element::into_webdriver_element(self) -> drivers::WebElement`. `drivers::WebElement` and
  `drivers::By` stay re-exported for it alone.
* **No other repository changes.** The stealth workspace, the Tauri plugin and the companion
  extension name none of these items (see [Migration](#migration)).

## What hands out thirtyfour types today

### `BaseCase`: 17 public methods return a `WebElement`

| Method | Where | Returns | Python parity entry | Call sites (text) |
|---|---|---|---|---|
| `get_shadow_root(css)` | `src/api/base_case.rs:2103-2114` | `WebElement` | none | 1: the recorder table `src/api/recorder/calls.rs:280`; docs `docs/tutorials/selectors.md:215`, doctest `src/drivers.rs:20` |
| `find_element(css)` | `base_case.rs:2309-2316` | `WebElement` | `parity/api.toml:626-629` | 6: `base_case.rs:3596`, `:3604`; `base_case_impl_mouse.rs:252`; `base_case_impl_parity.rs:209`; `src/mcp/webdriver.rs:302`; `recorder/calls.rs:278` |
| `find_elements(css)` | `base_case.rs:2319-2326` | `Vec<WebElement>` | `parity/api.toml:631-634` | 6: `base_case_impl_extra.rs:876`; `base_case_impl_shadow.rs:223` (test); `mcp/webdriver.rs:328`, `:410`; `tests/chrome/shadow_dom_chrome.rs:170`; `recorder/calls.rs:279` |
| `get_element(css)` | `base_case.rs:3592-3597` | `WebElement` | `parity/api.toml:769` | 0 |
| `locator(css)` | `base_case.rs:3600-3605` | `WebElement` | `parity/api.toml:1220` | 0 |
| `get_element_at_x_y(x, y)` | `base_case_impls/base_case_impl_dom.rs:443-451` | `Option<WebElement>` | `parity/api.toml:773` | 0 |
| `get_parent(css)` | `base_case_impl_dom.rs:454-461` | `Option<WebElement>` | `parity/api.toml:870` | 1: `base_case_impl_driver_tests.rs:439` |
| `find_link_text(text)` | `base_case_impls/base_case_impl_extra.rs:507-516` | `Option<WebElement>` | `parity/api.toml:641` | 0 |
| `find_partial_link_text(text)` | `base_case_impl_extra.rs:519-528` | `Option<WebElement>` | `parity/api.toml:650` | 1: `base_case_impl_extra.rs:458` |
| `find_exact_text(text, css)` | `base_case_impl_extra.rs:540-547` | `WebElement` | `parity/api.toml:636` | 3: `base_case_impl_text_tests.rs:276`, `:282`; `tests/chrome/parity_honesty_chrome.rs:251` |
| `find_text(text, css)` | `base_case_impl_extra.rs:559-562` | `WebElement` | `parity/api.toml:654` | 5: `base_case_impl_text_tests.rs:275`, `:281`, `:299`; `parity_honesty_chrome.rs:240`, `:246` |
| `find_non_empty_text(css)` | `base_case_impl_extra.rs:573-576` | `WebElement` | `parity/api.toml:645` | 3: `base_case_impl_text_tests.rs:277`, `:283`; `parity_honesty_chrome.rs:255` |
| `find_visible_elements(css)` | `base_case_impl_extra.rs:616-629` | `Vec<WebElement>` | `parity/api.toml:659` | 1: `base_case_impl_mouse.rs:166` |
| `select_all(css)` | `base_case_impl_extra.rs:872-877` | `Vec<WebElement>` | `parity/api.toml:1522` | 3: `parity_honesty_chrome.rs:88`, `:90`; `recorder/calls.rs:207` |
| `wait_for_any_of_elements_present(list, secs)` | `base_case_impl_extra.rs:1625-1648` | `WebElement` | `parity/api.toml:1816` | 3: `base_case_impl_driver_tests.rs:693`, `:701`, `:712` |
| `wait_for_link_text(text, secs)` | `base_case_impl_extra.rs:1656-1670` | `WebElement` | `parity/api.toml:1875` | 2: `base_case_impl_driver_tests.rs:678`, `:685` |
| `wait_for_partial_link_text(text, secs)` | `base_case_impl_extra.rs:1678-1694` | `WebElement` | `parity/api.toml:1895` | 2: `base_case_impl_driver_tests.rs:680`, `:686` |

That is 37 code call sites in this repository, 7 of which call a `thirtyfour` method on the
result and so change with the type: `base_case_impl_mouse.rs:173-175` (`scroll_into_view`,
`click`), `:253` (`tag_name`), `:261-265` (`find(By)`, `click`); `base_case_impl_parity.rs:210`
(`rect`); `mcp/webdriver.rs:303-307` (`outer_html`), `:414-422` (`is_displayed`, `click`);
`base_case_impl_text_tests.rs:279` (`element_id`). The other 30 use `.len()`, `.is_err()`,
`.unwrap()` or `let _ =` and compile unchanged.

`docs/design/bon_builders.md:538-541` counted 13 such signatures in `src/api/`; three of those
are private (`base_case_impl_extra.rs:580-585`, `base_case_impl_highlight.rs:148-157`,
`base_case_impl_shadow.rs:23-30`), and it missed seven public ones (`find_elements`,
`get_element_at_x_y`, `get_parent`, `find_link_text`, `find_partial_link_text`,
`find_visible_elements`, `select_all`): 17 public in all, the same 17 that
`src/mcp/dev/api_index.json` lists (lines 223-230, 264-265, 292, 307, 406, 501, 609, 624, 629).

### `BrowserSession`: 26 public methods take a `By`, 6 of them return a `WebElement`

All in `src/browser/session/`. Their callers are `BaseCase` internals (counted per method):

| Method | Where | Returns a `WebElement` | Call sites (text) |
|---|---|---|---|
| `click(locator)` | `elements.rs:58` | | 3 (`base_case.rs:994`, `:1029`, `:2834`) + `tests/offline/sb_cdp_attach.rs:580` |
| `type_text(locator, text)` | `elements.rs:65` | | 1 (`base_case.rs:1010`) |
| `clear(locator)` | `elements.rs:73` | | 1 (`base_case.rs:1020`) |
| `submit(locator)` | `elements.rs:80` | | 1 (`base_case.rs:1040`) |
| `text(locator)` | `elements.rs:91` | | 1 (`base_case.rs:1049`) |
| `hover(locator)` | `elements.rs:97` | | 1 (`base_case.rs:1058`) |
| `select_option_by_text(locator, text)` | `elements.rs:108` | | 1 (`base_case.rs:1083`) |
| `select_option_by_value(locator, value)` | `elements.rs:120` | | 1 (`base_case.rs:1099`) |
| `drag_and_drop(source, target)` | `elements.rs:132` | | 1 (`base_case.rs:1156`) |
| `find(locator)` | `elements.rs:150` | yes | 31 (`base_case.rs` 20, `base_case_impl_extra.rs` 8, `_media.rs:41`, `_mouse.rs:193`, `_shadow.rs:29`) |
| `find_all(locator)` | `elements.rs:157` | yes (`Vec`) | 5 (`base_case.rs:2414`; `_extra.rs:621`, `:1058`, `:1639`; `_highlight.rs:263`) |
| `element_present(locator)` | `elements.rs:162` | | 1 (`base_case.rs:1164`) |
| `get_attribute(locator, name)` | `elements.rs:168` | | 1 (`base_case.rs:1370`) |
| `get_property(locator, name)` | `elements.rs:220` | | 2 (`base_case.rs:1380`; `_dom.rs:136`) |
| `double_click(locator)` | `elements.rs:230` | | 1 (`base_case.rs:1909`) |
| `context_click(locator)` | `elements.rs:241` | | 1 (`base_case.rs:1921`) |
| `is_enabled(locator)` | `elements.rs:252` | | 2 (`base_case.rs:1929`, `:3212`) |
| `is_selected(locator)` | `elements.rs:261` | | 1 (`base_case.rs:1941`) |
| `is_displayed(locator)` | `elements.rs:270` | | 1 (`base_case.rs:1947`) |
| `find_elements(locator)` | `elements.rs:281` | yes (`Vec`) | 1 (`base_case.rs:2325`) |
| `wait_for_element(locator, secs)` | `waits.rs:20` | yes | 5 (`base_case.rs:2109`, `:2280`, `:2315`, `:3017`; `_parity.rs:149`) + test `waits.rs:223` |
| `wait_for_element_visible(locator, secs)` | `waits.rs:42` | yes | 3 (`base_case.rs:1393`; `_highlight.rs:155`; `_parity.rs:150`) + test `waits.rs:237` |
| `wait_for_element_clickable(locator, secs)` | `waits.rs:69` | yes | 3 (`base_case.rs:2097`, `:2332`; `_parity.rs:151`) + `elements.rs:231`, `:242` + test `waits.rs:239` |
| `wait_for_element_absent(locator, secs)` | `waits.rs:101` | | 1 (`base_case.rs:1418`) + test `waits.rs:258` |
| `wait_for_text(locator, text, secs)` | `waits.rs:130` | | test `waits.rs:272` only |
| `switch_to_frame(locator)` | `navigation.rs:68` | | 1 (`_dom.rs:128`) |

71 call sites in `src/api/` (the counts above), nearly all of the form
`let by = Selector::auto(css).to_by()?;` followed by `self.session.method(by, ..)`.

### `Selector`: 2 public methods return a `By`

`Selector::auto_by(text)` (`src/utils/selectors.rs:212-214`) and `Selector::to_by(self)`
(`:226-253`). 66 calls in `src/api/` (the `let by = ...` lines above) and one in
`src/browser/shadow_find.rs:110`; the unit tests in `selectors.rs:996-1211`; the pages
`docs/tutorials/selectors.md:63-74`, `docs/api/crate-root.md:57` and
`docs/DEVELOPER_GUIDE.md:1361`; the test `crates/own-imports/tests/drivers_and_json_from_our_crate.rs:20-24`.

### Other thirtyfour types in public signatures (not this lane)

| Item | Where | Type |
|---|---|---|
| `BrowserSession::driver`, `webdriver` | `src/browser/session/mod.rs:93`, `:116` | `&WebDriver` (the session-level escape hatch) |
| `BaseCase::get_cookie`, `BrowserSession::get_cookie` | `base_case.rs:2187-2192`, `session/page.rs:87-95` | `Cookie` |
| `BaseCase::set_network_conditions`, `BrowserSession::set_network_conditions`, `CdpClient::set_network_conditions` | `base_case.rs:1309`, `session/cdp.rs:221`, `src/evasion/cdp.rs:223` | `NetworkConditions` (deprecated by thirtyfour) |
| `CdpClient::dev_tools` (field), `CdpClient::from_handle` | `src/evasion/cdp.rs:66`, `:71` | `ChromeDevTools`, `SessionHandle` |
| `CapabilityOverrides::apply_to` | `src/browser/options/capabilities.rs:151` | `&mut Capabilities` |
| `StealthOptions::apply_to`, `apply_undetected_args` | `src/evasion/options.rs:130`, `:426` | `C: ChromiumLikeCapabilities` |
| `SeleniumBaseError::WebDriver` | `src/error.rs:74-76` | `WebDriverError` |

Each deserves its own lane (open question 9). They are listed so the count of thirtyfour
types in the public API is complete: 45 signatures with `WebElement` or `By`, 13 with another
thirtyfour type.

### Private code that passes `WebElement` around

These change with the type but are not API: `poll_until_found`'s `first_found`
(`base_case_impl_common.rs:103`), `element_from_script_result` (`base_case_impl_extra.rs:632-660`),
`visible_element`, `run_on_element`, `animate_highlight` (`base_case_impl_highlight.rs:148-184`),
`shadow_element` (`base_case_impl_shadow.rs:23-30`), `attribute_or_property`, `js_click_element`
(`session/elements.rs:191-217`), `shown_text` (`session/waits.rs:173`), and the whole of
`src/browser/shadow_find.rs`.

## What thirtyfour gives (Context7, and the code that already compiles)

From the Context7 pages for `thirtyfour::WebElement`: it is `Clone` (its `ElementWaitable` and
`ElementQueryable` impls call `self.clone()`), `Display` (writes the element id) and `Serialize`
(as the element id); `ElementId` is `Debug + Clone + Eq + Hash`. Its methods include `text`,
`value`, `click`, `clear`, `send_keys`, `tag_name`, `rect` (an `ElementRect`), `find_all(By)`,
`get_shadow_root`, `parent` and `is_clickable`.

Methods this crate already calls, so they exist in 0.37 as this crate resolves it:
`is_displayed` (`base_case_impl_extra.rs:599`), `is_enabled`, `is_selected`
(`session/elements.rs:255`, `:264`), `prop` (`:226`), `to_json` (`:82`), `outer_html`
(`mcp/webdriver.rs:304`), `scroll_into_view` (`base_case_impl_mouse.rs:173`), `find`
(`:262`), `enter_frame` (`session/navigation.rs:70`), `element_id` and `from_json`
(`shadow_find.rs:74`, `:76`), `components::SelectElement` (`elements.rs:114`), and the action
chain's `move_to_element_center`, `drag_and_drop_element`, `double_click_element`,
`context_click_element` (`elements.rs:101`, `:141`, `:234`, `:245`).

To verify: whether `WebElement` implements `Debug` (the pages did not show it; `Element`
writes its own, so nothing depends on it). `Send + Sync` follows from code that compiles:
`#[async_trait] impl ElementApi for BaseCase` (`src/api/traits.rs:138-159`) needs `Send`
futures, and `get_attribute` reaches `BrowserSession::get_attribute`, which holds a `WebElement`
and a `&WebElement` across an `.await` (`session/elements.rs:173-174`, `:196-199`).

## The proposed type

### `Element`

```rust
// src/browser/element.rs (proposed)
#[derive(Clone)]
pub struct Element {
    inner: thirtyfour::WebElement,
    driver: thirtyfour::WebDriver, // scripts and action chains, as `Alert` holds one
    selector: SelectorBuf,         // how it was found, for errors, Debug and Display
}
```

* **What it wraps.** The `thirtyfour::WebElement`, the session's `WebDriver` (scripts such as
  the `getAttribute` atom, `session/elements.rs:18-54`, and the action chains need the driver;
  `Alert` holds one the same way, `switching.rs:380-383`), and the selector it was found by
  (`SelectorBuf`; for `parent()` and `find()` results, the chain such as `#cart >> .row`, in the
  form `Locator`'s `Display` writes, `locator.rs:100-123`).
* **Built only by the crate**: `pub(crate) fn new(inner, driver, selector)`, called by
  `BrowserSession::find`, `find_all` and the waits.
* **`Clone`**: derived; both thirtyfour parts are `Clone` (Context7 for `WebElement`;
  `#[derive(Clone)] struct Alert` holds a `WebDriver`, `switching.rs:379-383`). A clone is the
  same element, not a new lookup.
* **`Debug`**: written by hand, as `Alert`'s is (`switching.rs:385-389`), with the selector and
  the element id and `finish_non_exhaustive`, so nothing depends on thirtyfour's `Debug`.
* **`Display`**: the selector, as `Locator` displays its chain.
* **`Send + Sync`**: yes, pinned by a compile-time test (a `fn assert_send_sync<T: Send + Sync>()`
  called with `Element`).
* **No `PartialEq`** for now (open question 8).
* **Waiting.** An `Element` does not wait: it is already found. The `BaseCase` methods that
  return one wait first, as today (`find_element` waits 10 seconds, `base_case.rs:2315`).
* **Errors.** Every method returns `Result<_, SeleniumBaseError>`. A stale element reference
  becomes `SeleniumBaseError::StaleElement { selector }` (`src/error.rs:32-34`) with the
  element's selector, which the raw type cannot name (the `WebDriverErrorInner` variant to
  match is to verify; `shadow_find.rs:85-87` matches `NoSuchElement` the same way). Every other
  driver error stays `SeleniumBaseError::WebDriver` (`error.rs:74-76`).

### Its methods

Only what users need. Each maps to the `Locator` method of the same idea (one name, one meaning
across the engines) and to the `BaseCase` method that does the same by selector.

| `Element` method | Does | Same idea on `sb_cdp::Locator` | Same idea on `BaseCase` | Under it |
|---|---|---|---|---|
| `text()` | The text Python's checks compare: the value of an `<input>` or `<textarea>`, the rendered text of anything else | `text` (`locator.rs:676`) | `get_text` | `shown_text` (`session/waits.rs:170-179`) |
| `html()` | Outer HTML | `html` (`:687`) | `get_element_html` (MCP, `mcp/webdriver.rs:299`) | `outer_html` |
| `tag()` | Lower-case tag name | `ElementInfo::tag` (`src/sb_cdp/types.rs:73`) | none | `tag_name` |
| `attribute(name)` | The attribute as the markup has it, `None` if absent | `attribute` (`:698`, `el.getAttribute`) | none (see below) | a script on `arguments[0]` |
| `property(name)` | The DOM property | none | `get_property` (`base_case.rs:1374`) | `prop` |
| `bounding_box()` | Position and size, an `sb_cdp::Rect` | `bounding_box` (`:715`) | `get_gui_element_rect` reads it (`_parity.rs:209-223`) | `rect` |
| `is_visible()` | Displayed | `is_visible` (`:750`) | `is_element_visible`, `is_displayed` (`base_case.rs:1945`) | `is_displayed` |
| `is_enabled()` | Enabled | `is_enabled` (`:791`) | `is_enabled` (`base_case.rs:1927`) | `is_enabled` |
| `is_selected()` | Selected or checked | `is_selected` (`:778`) | `is_selected` (`base_case.rs:1939`) | `is_selected` |
| `click()` | Click | `click` (`:810`) | `click` | `click` |
| `double_click()` | Double click | `double_click` (`:826`) | `double_click` | `double_click_element` |
| `right_click()` | Context click | `right_click` (`:839`) | `right_click` / `context_click` | `context_click_element` |
| `hover()` | Move the pointer over it | `hover` (`:852`) | `hover` | `move_to_element_center` |
| `clear()` | Empty the field | `clear` (`:924`) | `clear` | `clear` |
| `send_keys(text)` | Type after what is there | `send_keys` (`:961`) | `send_keys` | `send_keys` |
| `type_text(text)` | Clear, then type | `type_text` (`:979`) | `type_text` | `clear` + `send_keys`, as `elements.rs:65-69` |
| `press(key)` | Press a named key | `press(Key)` (`:1013`) | `press_keys` | `send_keys` of the key's value |
| `select_option(by)` | Choose an `<option>` | `select_option(SelectBy)` (`:1027`) | `select_option_by_text`, `_by_value`, `_by_index` | `SelectElement` |
| `submit()` | Submit its form | `submit` (`:1093`) | `submit` | the script of `elements.rs:84-86` |
| `scroll_into_view()` | Scroll it into view | `scroll_into_view` (`:1114`) | `scroll_into_view` | `scroll_into_view` |
| `drag_to(&target)` | Drag onto another element | `drag_to(&Locator)` (`:889`) | `drag_and_drop` | `drag_and_drop_element` |
| `find(selector)` | The first match inside it, looked for once | `locator(..)` is the lazy form (`:212`) | `nested_click` does it (`_mouse.rs:247-267`) | `find` |
| `find_all(selector)` | Every match inside it, possibly none | `locator(..).all()` (`:605`) | none | `find_all` |
| `parent()` | Its parent element | `parent` (`:253`, lazy) | `get_parent` | thirtyfour's `parent`, or `return arguments[0].parentElement` read back with `from_json` as `shadow_find.rs:76` does (to verify which in step 3) |
| `as_webdriver_element()` | The raw element, borrowed | none | none | the escape hatch |
| `into_webdriver_element()` | The raw element, owned | none | none | the escape hatch |

`press` and `select_option` take the pure CDP engine's own `Key` (`src/sb_cdp/input.rs:221-251`)
and `SelectBy` (`src/sb_cdp/types.rs:133-143`), so the two engines share one key type and one
option chooser (open question 5). `Element::attribute` means what `Locator::attribute` means,
the markup's attribute; `BaseCase::get_attribute` keeps Selenium's property-first answer
(`session/elements.rs:177-201`) under its own, different, Python name.

Left out on purpose: `nth`, `first`, `last`, `visible`, `strict`, `with_timeout`, `wait_for`,
`count`, `expect` (lazy-locator ideas: an `Element` is one node already found); `fill`,
`set_value`, `focus`, `check`, `uncheck`, `flash`, `screenshot`, `info` (`BaseCase` has each by
selector; add them when a user needs one; an element screenshot is also not on the Context7
pages read).

What it looks like (proposed):

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig};

let mut sb = BaseCase::new(BrowserConfig::default()).await?;
sb.open("https://example.com/cart").await?;
let row = sb.find_element("#cart .row").await?;
println!("{}", row.text().await?);
row.find(".remove").await?.click().await?;

// The escape hatch, for a thirtyfour call this crate has no method for:
let raw: &seleniumbase_rs::drivers::WebElement = row.as_webdriver_element();
```

### The shadow root

`get_shadow_root` (`base_case.rs:2103-2114`) is the one method whose result is not an element: a
W3C shadow root can be searched, not clicked or read. Two choices:

* **A. Remove it** (recommended). It is not a Python API (`parity/api.toml` has entries for the
  other 16 methods and none for it). The `host::shadow inner` selectors already reach the same
  elements through every method, `find_element` and `find_elements` included
  (`docs/tutorials/selectors.md:140-150`, `:174-185`). And it is the one shadow path that calls
  thirtyfour's `get_shadow_root` (`base_case.rs:2110-2113`), which `shadow_find.rs:64-67`
  describes as the script fallback, while the selectors ask the driver's Get Element Shadow
  Root command first (`shadow_find.rs:50-82`); so it may miss a closed root the selectors reach
  (to verify on Chrome).
* **B. A `ShadowRoot` type** with `find` and `find_all` only, built through
  `shadow_find::shadow_root_of`, so it enters closed roots as the selectors do.

### Selectors instead of `By`

* `BaseCase` keeps `css: &str` (the crate's rule for the capability traits and `BaseCase`).
* `BrowserSession`'s 26 methods and `Element::find` / `find_all` take
  `selector: impl Into<SelectorBuf>`, the parameter `Page::locator` and `Locator::locator` take.
  `&str`, `String`, `&String` and `Selector` all convert (`selectors.rs:587-623`), so the kind is
  decided once, by the rules `Selector::auto` documents. The parameter is renamed from `locator`
  to `selector`, since `Locator` means the lazy CDP type.
* One crate-private conversion, `SelectorBuf` to `By`, lives in `src/browser/` and replaces the
  public `to_by`. It keeps `to_by`'s two checks: an empty selector is
  `SeleniumBaseError::InvalidSelector` (`selectors.rs:249-251`), and a selector that crosses a
  shadow root is cut, or refused, before anything waits (`:231-234`).
* **A difference found while reading:** `to_by` checks for a shadow-root selector before it
  turns `:contains(..)` into XPath (`selectors.rs:231-241`), but `From<&str> for SelectorBuf`
  turns `:contains(..)` into XPath without looking for `::shadow` first (`:603-609`). A selector
  such as `app::shadow a:contains("Go")` would become XPath and lose its shadow root. Step 4
  gives `From<&str>` `to_by`'s order, with a test that fails first.
* `Selector::Id` has no `SelectorBuf` variant; `From<Selector>` writes it as `[id="..."]`
  (`selectors.rs:594`), where `to_by` sent `By::Id` (`:244`). Step 4 checks with a mock that the
  driver receives a working CSS lookup.
* `Selector::to_by` and `Selector::auto_by` become `pub(crate)` (their unit tests stay), and
  `drivers::By` is re-exported only for code that uses the escape hatch.

## Migration

### Per repository

| Repository | Sites | How it was checked |
|---|---|---|
| seleniumbase-rs (this one) | below | literal searches over the worktree index |
| seleniumbase-rs-stealth | 0 | `WebElement`, `drivers::`, `find_element`, `By::`: no hit; `BaseCase` only in `README.md:261` (its facade re-exports it) |
| tauri-plugin-sbrs-automation | 0 | it is a WebDriver server; `README.md:315-320` builds a `BaseCase` and opens a page; `thirtyfour` only in comments (`src/webdriver.rs:446`, `:1325`; `src/webdriver/actions.rs:108`); `find_element`: no hit |
| seleniumbase-rs-companion | 0 | its `BaseCase` generator writes `click`, `type_text`, `select_option_by_text`, `check_if_unchecked`, `uncheck_if_checked`, `hover`, `wait_for_element_visible`, `assert_text` (`src/shared/calls.ts:28-61`), none of which returns an element |

### This repository, per kind

* **`BaseCase`'s own callers**: the 37 sites in the first table; 7 change (listed under it).
* **`BrowserSession` callers**: about 64 in `src/api/`. Each `let by = Selector::auto(css).to_by()?;`
  plus `self.session.m(by, ..)` becomes `self.session.m(css, ..)`; `By::LinkText(text)`
  (`base_case_impl_extra.rs:511`, `:523`, `:1665`, `:1688`; `base_case.rs:1026`) becomes
  `SelectorBuf::link_text(text)`, `By::PartialLinkText` likewise, and `By::Css(..)`
  (`_extra.rs:651`, `:1058`) becomes `SelectorBuf::css(..)`. One `tokensave_ast_grep_rewrite`
  per shape. Internal code that needs the raw element (scripts' `to_json`, `SelectElement`, the
  action chains, `enter_frame`) uses `as_webdriver_element()` or a `pub(crate)` raw lookup kept
  in `shadow_find`.
* **MCP tools** (`src/mcp/webdriver.rs`): `get_element_html` (`:299-308`, `outer_html` to
  `html`), `click_visible_elements` (`:399-428`, `is_displayed` to `is_visible`, and the
  `map_err(SeleniumBaseError::WebDriver)` calls go).
* **Recorder**: `calls.rs:278-280` replay through `ran<T, E>` (`calls.rs:687`), which takes any
  result, and the generated line is `let _ = sb.find_element(..).await?;` (`calls.rs:671`), which
  names no type. Only the `get_shadow_root` entry (`:280`) goes, with choice A.
* **Importers and scaffolds** (`src/utilities/python_importer.rs`, `src/utilities/importer/`,
  `src/mcp/dev/`, `src/cli/scaffold.rs`): 0. They write no element-returning call; the
  `find_element` hits in `python_importer.rs:1188-1464` are Python input in tests.
* **Golden files**: 0. The `*.golden`, `*.rs.txt` and `*.mdx` files are not in the index; a
  text search over them found no `WebElement`, `drivers::`, `find_element`, `get_shadow_root`,
  `select_all`, `find_text`, `get_parent`, link-text wait or `thirtyfour`, and its only `By::`
  hits are `SelectBy::Text` in `tests/contract_companion/parity-cdp-*.rs.txt:10` and
  `tests/golden/import_python/raw_demo_site.rs.golden:39`.
* **Contract tests** (`tests/contract_*.rs`): 0. None names `WebElement`, `drivers::` or an
  element-returning method; `mise run contracts` should stay green.
* **Generated**: `src/mcp/dev/api_index.json` (17 entries) is written again with
  `SB_UPDATE_API_INDEX=1 cargo test -p seleniumbase-rs --features mcp-server api_index`;
  `parity/api.toml:629`, `:634` lose "Returns a thirtyfour::WebElement" and
  `docs/parity.md:184-185` is generated again from it.
* **Tests**: `base_case_impl_text_tests.rs:279` (`element_id` through the escape hatch);
  `session/waits.rs:223-272` (`By::Css("#a")` to `"#a"`); `tests/offline/sb_cdp_attach.rs:580`
  (`thirtyfour::By::Css("#go")` to `"#go"`);
  `crates/own-imports/tests/drivers_and_json_from_our_crate.rs:13-24` (the `By` test becomes an
  `Element`-from-our-crate import test); `tests/offline/docs_current.rs:171-172` gains GONE
  entries for `drivers::WebElement`, `drivers::By`, `Selector::to_by`, `Selector::auto_by` and
  `get_shadow_root`, with the escape-hatch section of the getting-started page in `ALLOWED`.
* **Docs pages**: `docs/api/queries.md:26-28`; `docs/api/traits.md:20`;
  `docs/api/crate-root.md:57`; `docs/ABI_API.md:29`, `:94`; `docs/best-practices.md:34`, `:387`,
  `:531-551`, `:839`; `docs/glossary.md:1962`; `docs/rust-test-tooling.md:613-614`, `:633-634`;
  `docs/tutorials/api_reference.md:103-104`; `docs/tutorials/getting_started.md:26`, `:372-380`;
  `docs/tutorials/selectors.md:28`, `:63-74`, `:150`, `:174-185`, `:207-218`;
  `docs/DEVELOPER_GUIDE.md:412`, `:1361`; `AGENTS.md:195-197`; `CLAUDE.md:84`;
  `context7.json:34`; the module docs of `src/drivers.rs:1-30` and `src/api/traits.rs:8-11`; a
  `CHANGELOG.md` entry under `[Unreleased]` (then `docs/changelog.md` is generated again).
  `ROADMAP.md:241-245` and `docs/design/bon_builders.md` are history and stay.

## Implementation steps

One commit each, test first. `MockWebDriver` (`base_case_impl_common.rs:740-845`) is
`#[cfg(test)]` inside the library, so the `Element` tests are unit tests in
`src/browser/element.rs`, reached as the session tests reach it
(`crate::api::base_case::webdriver_support`, `session/lifecycle.rs:204`); its `element(id)`
helper (`:762`) builds the element reference a mock lookup answers with.

1. **`feat(browser): an Element type for the WebDriver engine`.** The struct, `new`,
   `Clone`/`Debug`/`Display`, the escape hatch, the reading methods (`text`, `html`, `tag`,
   `attribute`, `property`, `bounding_box`, `is_visible`, `is_enabled`, `is_selected`), and
   the `Send + Sync` test. Tests: a mock that answers `POST /element` with `element("e1")` and
   the element's `GET` commands; `text` of an `<input>` reads its `value` property; `Debug`
   shows the selector.
2. **`feat(browser): actions on an Element`.** `click`, `double_click`, `right_click`, `hover`,
   `clear`, `send_keys`, `type_text`, `press`, `select_option`, `submit`, `scroll_into_view`,
   `drag_to`; re-export `Key` and `SelectBy` at the crate root if open question 5 says so. Tests
   assert the requests the mock receives (`/element/e1/click`, `/actions`, `/execute/sync`).
3. **`feat(browser): finding from an Element`.** `find`, `find_all`, `parent`; a stale reference
   becomes `StaleElement { selector }`. Tests: a mock answering `stale element reference` gives
   that error with the chain as the selector; `find_all` with no match is empty.
4. **`refactor(browser): BrowserSession takes selectors and returns Element`.** The 26 methods,
   the crate-private `SelectorBuf` to `By` conversion, `From<&str>`'s shadow-before-`:contains`
   order (red test first), `Selector::to_by`/`auto_by` made `pub(crate)`, `waits.rs` and
   `sb_cdp_attach.rs` tests updated. The internal `BaseCase` sites are rewritten in this commit
   so it compiles.
5. **`refactor(api): BaseCase returns Element`.** The 17 methods (less `get_shadow_root` and
   `locator` if open questions 2 and 3 say so, with the recorder entry and the parity entry),
   the 7 call sites that use the result, the MCP tools, `api_index.json` written again, the
   parity divergences. Tests: the existing mock tests (`base_case_impl_text_tests.rs`,
   `base_case_impl_driver_tests.rs`, `base_case_impl_shadow.rs:223`) pass unchanged except
   `:279`; one `#[ignore]`d check in `tests/chrome/shadow_dom_chrome.rs` that an `Element` from
   `find_element("host::shadow li")` reads its text.
6. **`docs: the Element type`.** The pages, `AGENTS.md`, `CLAUDE.md`, `context7.json`, the
   getting-started import table, `docs_current.rs`, the own-imports test, `CHANGELOG.md`.

The full gate of `CLAUDE.md` (fmt, clippy with `-D warnings`, tests, docs with `-D warnings`)
runs once after step 6; each step runs the tests of the files it changed.

## Open questions

1. **The name: `Element`, `WebDriverElement` or `ElementRef`?** `Element` is free (the only
   items of that name are enum variants: `src/cli/commands.rs:113`,
   `src/utilities/python_importer.rs:157`, `src/api/recorder/generate.rs:367`,
   `src/api/deferred.rs:60`), it says what Python's `find_element` returns, and the WebDriver
   engine's other handle is unprefixed (`Alert`). **Recommendation:** `Element`, at the crate root.
2. **`get_shadow_root`: remove, or return a `ShadowRoot`?** **Recommendation:** remove it
   (choice A above): not a Python API, covered by `::shadow` selectors in every method, and the
   one path that may not enter closed roots.
3. **`BaseCase::locator(css)` returns a found element, while `Page::locator` returns a lazy
   `Locator`.** Two meanings for one name. **Recommendation:** remove `BaseCase::locator`; its
   parity entry (`parity/api.toml:1220`) becomes `composed` with `rust = "BaseCase::find_element"`
   and a note. Keep `get_element` (`parity/api.toml:769`), a plain alias with no clash.
4. **The escape hatch's names.** `as_webdriver_element` / `into_webdriver_element` follow the
   `as_`/`into_` conversion convention and say which crate's type comes out; a shorter
   `webdriver()` would echo `BrowserSession::webdriver()` (`session/mod.rs:116`), which returns
   the driver, not an element. **Recommendation:** the `as_`/`into_` pair.
5. **`Key` and `SelectBy` live in `sb_cdp`.** `Element::press` and `select_option` taking them
   makes one key type and one option chooser for both engines, but a WebDriver user then imports
   from `sb_cdp`. **Recommendation:** take them as they are and re-export both at the crate root
   in step 2; moving their definitions is not needed.
6. **A frame given as an `Element`.** `FrameRef` (`switching.rs:49-64`) takes a selector, an id
   or name, or an index; whether Python's `switch_to_frame` also takes an element is to verify
   against the Python source. **Recommendation:** not in this lane; a
   `FrameRef::Element` variant can follow when a user needs it.
7. **Recording actions done through an `Element`.** `Alert` records its calls
   (`switching.rs:396-402`). **Recommendation:** no; the `BaseCase` call that found the element
   is recorded, as today (`base_case.rs:2313`), and a recording replays selectors, not handles.
8. **`PartialEq` on `Element`** (same session, same element id). **Recommendation:** not until a
   test or a user needs it.
9. **The other thirtyfour types** (the second table: `WebDriver`, `Cookie`, `NetworkConditions`,
   the capability types, `ChromeDevTools`, `WebDriverError`). **Recommendation:** one lane per
   type after this one, `Cookie` first (it is returned to users by `get_cookie`), each with its
   own note; `drivers` then shrinks to the escape-hatch types.
10. **A lazy WebDriver locator instead of a found element?** **Recommendation:** no. `Element` is
    Python's `WebElement`; the lazy finder is the pure CDP engine's `Locator`; and the rule "pass
    selector strings, do not hold elements" (`AGENTS.md:195-197`, `docs/best-practices.md:531-551`)
    stays the advice for both.
11. **`BaseCase::get_text` and `Locator::text` differ** (seen while mapping `text`):
    `get_text` (`base_case.rs:1045-1050`) reads the driver's rendered text
    (`session/elements.rs:91-94`), so an `<input>` gives nothing, while `Locator::text` and
    Python read the field's value (`locator.rs:669-676`). `Element::text` follows `Locator` and
    `shown_text`. **Recommendation:** a separate fix lane for `get_text`, with a test first.
    **Done:** `BrowserSession::text`, which only `get_text` called, reads `Element::text`
    now. `get_text` still waits for the element to be present where Python's waits until it is
    visible, and `get_text_content` still reads the rendered text where Python's reads the
    `textContent` property: both are differences left as they were, and the `divergence` of
    their rows in `parity/api.toml` (so `docs/parity.md`) names both.
