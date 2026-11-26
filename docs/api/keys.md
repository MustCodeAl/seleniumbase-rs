---
title: "Special Keyboard Keys (Keys)"
sidebarTitle: "Keys"
description: "W3C WebDriver and Python Selenium keyboard key constants and engine mapping (Keys)."
---

# Keyboard Keys (`seleniumbase_rs::Keys`)

The `Keys` struct defines special keyboard key constants matching W3C WebDriver specification and Python Selenium's `selenium.webdriver.common.keys.Keys`.

Every constant represents the canonical Unicode Private Use Area (PUA) code point string (`\u{e000}` – `\u{f8ff}`) recognized by WebDriver drivers and translated natively by the pure CDP engine into native DevTools `Input.dispatchKeyEvent` events.

## Common Keys

| Constant | Code Point | Description |
|---|---|---|
| `Keys::ENTER` | `\u{e007}` | Enter key |
| `Keys::RETURN` | `\u{e006}` | Return key |
| `Keys::TAB` | `\u{e004}` | Tab key |
| `Keys::BACKSPACE` / `BACK_SPACE` | `\u{e003}` | Backspace key |
| `Keys::ESCAPE` | `\u{e00c}` | Escape key |
| `Keys::SPACE` | `\u{e00d}` | Space key |
| `Keys::PAGE_UP` | `\u{e00e}` | Page Up key |
| `Keys::PAGE_DOWN` | `\u{e00f}` | Page Down key |
| `Keys::END` | `\u{e010}` | End key |
| `Keys::HOME` | `\u{e011}` | Home key |
| `Keys::ARROW_LEFT` / `LEFT` | `\u{e012}` | Arrow Left key |
| `Keys::ARROW_UP` / `UP` | `\u{e013}` | Arrow Up key |
| `Keys::ARROW_RIGHT` / `RIGHT` | `\u{e014}` | Arrow Right key |
| `Keys::ARROW_DOWN` / `DOWN` | `\u{e015}` | Arrow Down key |
| `Keys::INSERT` | `\u{e016}` | Insert key |
| `Keys::DELETE` | `\u{e017}` | Delete key |

## Modifier Keys

| Constant | Code Point | Description |
|---|---|---|
| `Keys::SHIFT` / `LEFT_SHIFT` | `\u{e008}` | Shift key |
| `Keys::CONTROL` / `LEFT_CONTROL` | `\u{e009}` | Control key |
| `Keys::ALT` / `LEFT_ALT` | `\u{e00a}` | Alt key |
| `Keys::META` / `COMMAND` | `\u{e03d}` | Meta / Command (macOS) / Windows key |

## Function Keys & Numpad

- Function keys: `Keys::F1` through `Keys::F12` (`\u{e031}` – `\u{e03c}`).
- Numpad keys: `Keys::NUMPAD0` through `Keys::NUMPAD9` (`\u{e01a}` – `\u{e023}`), `Keys::MULTIPLY`, `Keys::ADD`, `Keys::SEPARATOR`, `Keys::SUBTRACT`, `Keys::DECIMAL`, `Keys::DIVIDE`.

## Usage Examples

### WebDriver Mode (`BaseCase`)

```rust
use seleniumbase_rs::testing::{rstest, sb, Sb};
use seleniumbase_rs::{tokio, Keys, Result};

#[rstest]
#[tokio::test]
async fn search_with_enter(sb: Sb) -> Result<()> {
    sb.run(async |sb| {
        sb.open("https://example.com").await?;
        sb.type_text("#search", format!("seleniumbase-rs{}", Keys::ENTER)).await?;
        sb.assert_element("#results").await
    })
    .await
}
```

### Pure CDP Mode (`sb_cdp`)

In pure CDP mode, special key constants are automatically resolved into native keyboard events (`Key::Enter`, `Key::Tab`, etc.):

```rust
use seleniumbase_rs::testing::{rstest, sb_cdp, SbCdp};
use seleniumbase_rs::{tokio, Keys, Result};

#[rstest]
#[tokio::test]
async fn press_special_key(sb_cdp: SbCdp) -> Result<()> {
    sb_cdp.run(async |page| {
        page.goto("https://example.com").await?;
        let search = page.locator("#search");
        search.fill("query").await?;
        search.press(Keys::ENTER).await?;
        page.locator("#results").expect().to_be_visible().await
    })
    .await
}
```

### Code Point Mapping (`Keys::to_key`)

`Keys::to_key(ch: char) -> Option<Key>` converts a single character (including Unicode PUA characters) to the underlying engine's `Key` representation:

```rust
use seleniumbase_rs::sb_cdp::Key;
use seleniumbase_rs::Keys;

assert_eq!(Keys::to_key('\u{e007}'), Some(Key::Enter));
assert_eq!(Keys::to_key('\u{e004}'), Some(Key::Tab));
assert_eq!(Keys::to_key('\u{e003}'), Some(Key::Backspace));
```
