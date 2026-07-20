# Remaining BaseCase Helpers

This guide covers additional `BaseCase` helpers that close the parity gap with Python SeleniumBase.

## Page Artifacts

```rust
let screenshot = sb.save_screenshot("home.png").await?;
let source = sb.save_page_source("home.html").await?;
```

Both methods return a `PathBuf` pointing to the saved file in the logs directory.

## Window Introspection

```rust
let (x, y, width, height) = sb.get_window_rect().await?;
sb.maximize().await?;
sb.minimize().await?;
```

## On-Page Messaging

```rust
sb.post_success_message("Operation completed").await?;
sb.post_error_message("Something went wrong").await?;
sb.post_message_for("Transient message", 3).await?;
```

## Console Logs

```rust
sb.console_log_script().await?;
sb.console_log_string("Custom log entry").await?;
let logs = sb.get_console_logs().await?;
```

## Referral / Traffic Helpers

```rust
let url = sb.generate_referral("https://example.com", "docs")?;
sb.generate_traffic("https://example.com", "docs").await?;
```

## Multi-Series Charts

```rust
sb.create_bar_chart("Sales").await?;
sb.add_data_point("Jan", 100).await?;
sb.add_data_point("Feb", 150).await?;
sb.add_series_to_chart("Last Year", &[("Jan".into(), 80), ("Feb".into(), 120)]).await?;
let path = sb.display_chart().await?;
```

## Deferred Assertions

```rust
sb.deferred_assert_element_present("#result").await?;
sb.deferred_assert_exact_text("#result", "Done").await?;
sb.process_deferred_asserts().await?;
```

Run `cargo run --example remaining_helpers` to see many of these helpers in action.
