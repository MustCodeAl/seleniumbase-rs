# Macros

`seleniumbase-rs` exports a set of convenience macros from the crate root. They
reduce boilerplate for selectors, test setup, common interactions, and
assertions. Because the action macros expand `.await` internally, use them
inside an `async` function or closure.

This page catalogs every public macro and shows when to prefer macros over the
explicit method API.

```rust
use seleniumbase_rs::{
    assert_visible, cdp_config, fingerprint, sb_assert_not_visible, sb_assert_text,
    sb_assert_title, sb_assert_url, sb_click, sb_config, sb_fill_form, sb_hover,
    sb_js, sb_open, sb_quit, sb_scroll_to, sb_select, sb_screenshot, sb_test,
    sb_type, sb_wait_and_click, sb_wait_for, sb_with_timeout, selector, uc_config,
};
```

## What you will learn

- How to build selectors with `selector!`.
- How to declare a test with `sb_test!`.
- How to use config and fingerprint macros.
- How action macros map to `BaseCase` methods.

## `selector!`

Builds a [`Selector`](crate::Selector) variant at compile time.

```rust
use seleniumbase_rs::{selector, Selector};

let s = selector!(css, "#submit");
assert_eq!(s, Selector::Css("#submit"));

let s = selector!(xpath, "//button[text()='Go']");
let s = selector!(id, "username");
let s = selector!(link, "Home");
let s = selector!(partial_link, "Terms");
```

## `sb_test!`

Generates a `#[tokio::test]` async function that creates a [`BaseCase`](crate::BaseCase),
runs the supplied closure, and always calls `quit()` before returning. The test
fails if the closure returns an `Err`.

```rust,ignore
use seleniumbase_rs::sb_test;

sb_test!(visit_example, seleniumbase_rs::BrowserConfig::default(), |sb| {
    sb.open("https://example.com").await?;
    sb.assert_title("Example Domain").await?;
    Ok(())
});
```

The closure receives `&mut BaseCase` and must return `Result<(), E>` where `E`
converts into `SeleniumBaseError` (for example, via `?` on crate methods).

## `uc_config!`

Returns a [`BrowserConfig`](crate::BrowserConfig) with UC mode enabled.

```rust
use seleniumbase_rs::uc_config;

let config = uc_config!();
```

## `cdp_config!`

Returns a [`BrowserConfig`](crate::BrowserConfig) with CDP mode enabled.

```rust
use seleniumbase_rs::cdp_config;

let config = cdp_config!();
```

## `sb_config!`

Builds a [`BrowserConfig`](crate::BrowserConfig) using common field
permutations. Fields can be chained in any order supported by the macro.

```rust
use seleniumbase_rs::{sb_config, Browser, DriverMode};

let config = sb_config! {
    mode: DriverMode::Uc,
    headless: true,
    browser: Browser::Chrome,
};
```

## `fingerprint!`

Returns a built-in [`Fingerprint`](crate::Fingerprint) preset.

```rust
use seleniumbase_rs::fingerprint;

let fp = fingerprint!(windows);
let fp = fingerprint!(macos);
let fp = fingerprint!(linux);
let fp = fingerprint!(android);
let fp = fingerprint!(ios);
let fp = fingerprint!(edge);
```

## Action macros

These macros call the corresponding [`BaseCase`](crate::BaseCase) method and
await it. An optional trailing message is passed to `.expect(...)`.

| Macro | Calls |
|---|---|
| `sb_open!(sb, url)` | `sb.open(url).await` |
| `sb_click!(sb, selector)` | `sb.click(selector).await` |
| `sb_type!(sb, selector, text)` | `sb.type_text(selector, text).await` |
| `sb_hover!(sb, selector)` | `sb.hover(selector).await` |
| `sb_scroll_to!(sb, selector)` | `sb.scroll_to(selector).await` |
| `sb_wait_for!(sb, selector)` | `sb.wait_for_element_visible_default(selector).await` |
| `sb_wait_and_click!(sb, selector)` | wait then `sb.click(selector).await` |
| `sb_select!(sb, selector, text)` | `sb.select_option_by_text(selector, text).await` |
| `sb_assert_text!(sb, selector, expected)` | `sb.assert_text(selector, expected).await` |
| `sb_assert_title!(sb, expected)` | `sb.assert_title_contains(expected).await` |
| `sb_assert_url!(sb, expected)` | `sb.assert_url_contains(expected).await` |
| `sb_assert_not_visible!(sb, selector)` | fails if the element is visible |
| `sb_screenshot!(sb, path)` | `sb.save_screenshot_to_path(path).await` |
| `sb_js!(sb, script)` | `sb.execute_script(script).await` |
| `sb_quit!(sb)` | `sb.quit().await` |
| `assert_visible!(sb, selector)` | `sb.assert_element_visible(selector).await` |

## Form and timeout macros

`sb_fill_form!` types into multiple fields in sequence and returns a
[`Result`](crate::Result), so it works with the `?` operator inside tests:

```rust,ignore
use seleniumbase_rs::sb_fill_form;

sb_fill_form!(sb, "#username" => "alice", "#password" => "secret")?;
```

`sb_with_timeout!` temporarily raises the default implicit wait timeout for the
enclosed block and restores it afterwards:

```rust,ignore
use seleniumbase_rs::sb_with_timeout;

sb_with_timeout!(sb, 30, {
    sb_wait_for!(sb, "#slow-element");
    sb_click!(sb, "#slow-element");
});
```

```rust,ignore
use seleniumbase_rs::{
    assert_visible, sb_click, sb_open, sb_quit, sb_screenshot, sb_test,
    sb_type, selector,
};

sb_test!(login_with_macros, seleniumbase_rs::BrowserConfig::default(), |sb| {
    sb_open!(sb, "https://example.com/login");
    sb_type!(sb, "#username", "alice");
    sb_type!(sb, "#password", "secret");
    sb_click!(sb, "#submit");
    assert_visible!(sb, "#dashboard");
    sb_screenshot!(sb, "dashboard.png");
    sb_quit!(sb);
    Ok(())
});
```

## When to use macros

Use macros for:

* Quick, linear test scripts where the shorter syntax improves readability.
* Compile-time selectors that never change.
* One-liner actions that would otherwise be dominated by `.await?` noise.

Prefer the explicit method API when you need fine-grained error handling,
custom timeouts, or non-trivial control flow.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Macro expansion error | Used outside an `async` function | Wrap the call in an async block or `sb_test!`. |
| `sb_test!` not found | Macro not imported | Import `seleniumbase_rs::sb_test`. |
| Action macro panics | Selector did not match | Add an explicit `sb_wait_for!` first or increase the timeout. |
| `selector!` type mismatch | Wrong variant name | Use `css`, `xpath`, `id`, `link`, or `partial_link`. |
