# Waits and Assertions

Browser tests fail most often because they interact with elements before the
page is ready. `seleniumbase-rs` provides implicit waiting in many action
methods, plus explicit waits and assertions that retry until a condition is met
or a timeout expires.

This page explains the waiting model, the available assertion helpers, soft
assertions, and common mistakes to avoid.

## Implicit smart waiting

Many `BaseCase` methods automatically wait for the target element to be present
and visible before acting. This includes `click`, `type_text`, `get_text`,
`hover`, and several others. You do not need to add a manual wait before every
action, but explicit waits are still useful when you need precise timing or when
testing dynamic state changes.

## Explicit waits

Use explicit waits when the default implicit wait is not enough or when you want
to wait for a specific condition other than visibility.

```rust
// Wait for element to exist in the DOM
sb.wait_for_element_present("#results", 10).await?;

// Wait for element to be visible
sb.wait_for_element_visible(".spinner", 5).await?;

// Wait for element to become invisible
sb.wait_for_element_not_visible(".loading", 15).await?;

// Wait for element to be clickable
sb.wait_for_element_clickable("#checkout", 10).await?;

// Wait for element to be removed from the DOM
sb.wait_for_element_absent(".error", 5).await?;

// Wait for document.readyState === 'complete'
sb.wait_for_ready_state_complete().await?;
```

Timeouts are in seconds and are passed as `u64` values. Some methods also have
a `_default` variant that uses the configured implicit timeout.

## Text and title waits

Wait for text or page title conditions:

```rust
// Wait until the given text is visible inside the selector
sb.wait_for_text("body", "Welcome", 10).await?;

// Wait until text is no longer visible anywhere on the page
sb.wait_for_text_not_visible("Loading", 10).await?;

// Wait until the page title equals the expected value
sb.assert_title("Dashboard").await?;

// Wait until the page title contains the expected substring
sb.assert_title_contains("Dashboard").await?;
```

## Assertions

Assertions return `Result<(), SeleniumBaseError>` and stop the test on failure.

```rust
// Title assertions
sb.assert_title("My App").await?;
sb.assert_title_contains("My App").await?;

// Text assertions
sb.assert_text("body", "Welcome!").await?;
sb.assert_text_visible("Welcome!", "body").await?;
sb.assert_text_not_visible("Error", "body").await?;
sb.assert_exact_text("#greeting", "Hello, Alice").await?;

// Element assertions
sb.assert_element("#success").await?;
sb.assert_element_present("#success").await?;
sb.assert_element_visible("#success").await?;
sb.assert_element_absent(".error").await?;

// Attribute assertion
sb.assert_attribute("#logo", "src", "/logo.png").await?;

// Link and page health assertions
sb.assert_link_text("Home").await?;
sb.assert_no_404_errors().await?;
```

## Soft assertions

Soft assertions collect failures and report them at the end of the test instead
of stopping immediately. This is useful for validation-heavy tests where you
want to see all failures at once.

```rust
sb.deferred_assert_element("#header").await?;
sb.deferred_assert_text("Terms", "footer").await?;
sb.deferred_assert_exact_text("#status", "Ready").await?;

// Process all deferred assertions; this is where failures are raised.
sb.process_deferred_asserts().await?;
```

If any deferred assertion failed, `process_deferred_asserts` returns an error
containing all of the failures.

## Default timeout

You can change the default implicit wait used by helper macros and some methods:

```rust
let old = sb.set_timeout(30).await?;
```

`set_timeout` returns the previous value so you can restore it later. Some
methods also honor the overall test time limit set with `set_time_limit`.

## Anti-patterns

- **Do not use `tokio::time::sleep` to wait for pages**. It makes tests slow and
  does not eliminate timing races. Use explicit waits instead.
- **Do not assert on exact styling**. Assert on content, attributes, and state.
- **Do not silently swallow assertion failures**. Let errors propagate with `?`
  so the test runner reports them.

## Wait and assertion checklist

- [ ] Action methods use implicit waiting where possible.
- [ ] Explicit waits are used for dynamic state changes.
- [ ] Timeouts are chosen based on realistic page timing, not worst-case guesses.
- [ ] Soft assertions are followed by `process_deferred_asserts`.
- [ ] Tests do not contain fixed sleeps.

## Related reading

- [Selectors](./selectors.md) — choosing stable element locators.
- [API Reference](./api_reference.md) — complete list of wait and assertion methods.
