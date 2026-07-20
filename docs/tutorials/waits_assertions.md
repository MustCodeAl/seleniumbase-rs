# Waits and Assertions

Browser tests fail when they interact with elements before the page is ready. SeleniumBase for Rust provides explicit waits and assertions that retry until a condition is met or a timeout expires.

## Implicit smart-waiting

`BaseCase` methods such as `click`, `type_text`, and `get_text` wait automatically for the target element to be present and visible before acting. You can still call explicit waits when you need fine-grained control.

## Explicit waits

```rust
sb.wait_for_element_present("#results", Some(10.0)).await?;
sb.wait_for_element_visible(".spinner", Some(5.0)).await?;
sb.wait_for_element_not_visible(".loading", Some(15.0)).await?;
sb.wait_for_element_clickable("#checkout", Some(10.0)).await?;
sb.wait_for_element_absent(".error", Some(5.0)).await?;
sb.wait_for_ready_state_complete().await?;
```

Time values are in seconds. Passing `None` uses the global timeout.

## Text and title waits

```rust
sb.wait_for_text_visible("Welcome", "body", Some(10.0)).await?;
sb.wait_for_text_not_visible("Loading", "body", Some(10.0)).await?;
sb.wait_for_title("Dashboard").await?;
sb.wait_for_title_contains("Dashboard").await?;
```

## Assertions

Assertions throw `SeleniumBaseError` on failure.

```rust
sb.assert_title("My App").await?;
sb.assert_title_contains("My App").await?;
sb.assert_text_visible("Welcome!", "body").await?;
sb.assert_text_not_visible("Error", "body").await?;
sb.assert_element_present("#success").await?;
sb.assert_element_visible("#success").await?;
sb.assert_attribute("#logo", "src", "/logo.png").await?;
sb.assert_no_404_errors().await?;
sb.assert_no_js_errors().await?;
```

## Soft assertions

Deferred assertions collect failures and report them at the end of the test instead of stopping immediately.

```rust
sb.deferred_assert_element("#header").await?;
sb.deferred_assert_text("Terms", "footer").await?;
sb.process_deferred_asserts().await?;
```

## Global timeout

Set a default time limit for all waits:

```rust
sb.set_time_limit(30.0);
```

## Anti-patterns

- Do not use `tokio::time::sleep` to wait for pages; prefer explicit waits.
- Do not assert on exact styling; assert on content and state.
