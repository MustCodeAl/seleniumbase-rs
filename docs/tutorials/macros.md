---
title: "Macros"
---

# Macros

`seleniumbase-rs` exports a set of convenience macros from the crate root. They
reduce boilerplate for selectors, test setup, common interactions, and
assertions. Because the action macros expand `.await` internally, use them
inside an `async` function or closure.

This page catalogs every public macro and shows when to prefer macros over the
explicit method API.

```rust
use seleniumbase_rs::{
    cdp_config, fingerprint, sb_assert_not_visible, sb_assert_text, sb_assert_title,
    sb_assert_title_contains, sb_assert_url, sb_assert_url_contains, sb_assert_visible,
    sb_click, sb_config, sb_fill_form, sb_hover, sb_js, sb_open, sb_quit, sb_scroll_to,
    sb_select, sb_screenshot, sb_test, sb_type, sb_wait_and_click, sb_wait_for,
    sb_with_timeout, selector, uc_config,
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

```rust,no_run
use seleniumbase_rs::sb_test;

sb_test!(visit_example, seleniumbase_rs::BrowserConfig::default(), |sb| {
    sb.open("https://example.com").await?;
    sb.assert_title("Example Domain").await?;
    Ok(())
});
```

The `|sb| { ... }` part only looks like a closure. The macro creates the
`BaseCase`, binds it to the name you give (a `mut` binding of type `BaseCase`,
not a reference), and runs your block inside an `async` block whose result type
is `Result<(), SeleniumBaseError>`; use `?` on crate methods and finish with
`Ok(())`. It panics if the `BaseCase` cannot be created, calls `quit` whether or
not the block failed, and then panics with the error if the block returned one.

<Warning>

The macro quits for you, so do not call `sb_quit!` inside it.

</Warning>

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
let fp = fingerprint!(safari); // same preset as ios
let fp = fingerprint!(edge); // same preset as windows
```

## Action macros

These macros call the corresponding [`BaseCase`](crate::BaseCase) method and
await it. Without a trailing message the macro evaluates to the method's
`Result`, so follow it with `?` (a bare call compiles but ignores the failure and
draws an `unused_must_use` warning).

<Warning>

With a trailing message the macro unwraps the `Result` with `.expect(message)`,
which panics on failure.

</Warning>

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
| `sb_assert_text!(sb, expected, selector)` | `sb.assert_text(expected, selector).await` |
| `sb_assert_title!(sb, expected)` | `sb.assert_title(expected).await` (the whole title) |
| `sb_assert_title_contains!(sb, expected)` | `sb.assert_title_contains(expected).await` |
| `sb_assert_url!(sb, expected)` | `sb.assert_url(expected).await` (the whole URL) |
| `sb_assert_url_contains!(sb, expected)` | `sb.assert_url_contains(expected).await` |
| `sb_assert_not_visible!(sb, selector)` | fails if the element is visible |
| `sb_screenshot!(sb, path)` | `sb.save_screenshot_to_path(path).await` |
| `sb_js!(sb, script)` | `sb.execute_script(script).await` |
| `sb_quit!(sb)` | `sb.quit().await` |
| `sb_assert_visible!(sb, selector)` | `sb.assert_element_visible(selector).await` |
| `assert_visible!(sb, selector)` | the old, deprecated name of `sb_assert_visible!` |

## Form and timeout macros

`sb_fill_form!` types into multiple fields in sequence and returns a
[`Result`](crate::Result), so it works with the `?` operator inside tests:

```rust,no_run
use seleniumbase_rs::sb_fill_form;

# async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> seleniumbase_rs::Result<()> {
sb_fill_form!(sb, "#username" => "alice", "#password" => "secret")?;
# Ok(())
# }
```

`sb_with_timeout!` temporarily sets the default wait timeout (the one the
actions and `sb_wait_for!` use) for the enclosed block and restores it
afterwards. The block must produce a `Result`, and the macro returns that
`Result`, so finish with `?`:

```rust,no_run
use seleniumbase_rs::{sb_click, sb_wait_for, sb_with_timeout};

# async fn demo(sb: &mut seleniumbase_rs::BaseCase) -> seleniumbase_rs::Result<()> {
sb_with_timeout!(sb, 30, {
    sb_wait_for!(sb, "#slow-element")?;
    sb_click!(sb, "#slow-element")
})?;
# Ok(())
# }
```

A complete test with the macros (each action macro ends in `?`):

```rust,no_run
use seleniumbase_rs::{
    sb_assert_visible, sb_click, sb_open, sb_screenshot, sb_test, sb_type,
};

sb_test!(login_with_macros, seleniumbase_rs::BrowserConfig::default(), |sb| {
    sb_open!(sb, "https://example.com/login")?;
    sb_type!(sb, "#username", "alice")?;
    sb_type!(sb, "#password", "secret")?;
    sb_click!(sb, "#submit")?;
    sb_assert_visible!(sb, "#dashboard")?;
    sb_screenshot!(sb, "dashboard.png")?;
    Ok(())
});
```

## When to use macros

Use macros for:

* Quick, linear test scripts where the shorter syntax improves readability.
* Compile-time selectors that never change.
* One-liner actions that would otherwise be dominated by `.await?` noise.

<Tip>

Prefer the explicit method API when you need fine-grained error handling,
custom timeouts, or non-trivial control flow.

</Tip>

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Macro expansion error | Used outside an `async` function | Wrap the call in an async block or `sb_test!`. |
| `sb_test!` not found | Macro not imported | Import `seleniumbase_rs::sb_test`. |
| Action macro panics | A trailing message was given, so a failure is unwrapped with `expect` (for example because the selector did not match) | Drop the message and use `?`, or add an explicit `sb_wait_for!` first or raise the timeout. |
| Warning that a `Result` must be used | An action macro without a message was called without `?` | Add `?`: the failure is otherwise ignored. |
| `selector!` type mismatch | Wrong variant name | Use `css`, `xpath`, `id`, `link`, or `partial_link`. |
