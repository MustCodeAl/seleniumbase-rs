# Remaining BaseCase Helpers

This guide covers additional `BaseCase` helpers that close the parity gap with
Python SeleniumBase. These helpers handle page artifacts, window introspection,
on-page messaging, console logs, traffic generation, charts, and deferred
assertions.

## What you will learn

- How to save screenshots and page source.
- How to inspect and manipulate the browser window.
- How to post messages and read console logs.
- How to use deferred assertions for soft failures.

## Page artifacts

```rust
let screenshot = sb.save_screenshot("home.png").await?;
let source = sb.save_page_source("home.html").await?;
```

Both methods return a `PathBuf` pointing to the saved file in the logs directory
(`latest_logs/`). The name must be a plain file name: `"../home.png"` or
`"/tmp/home.png"` is refused with an `InvalidConfig` error instead of writing
outside the logs directory. A file of the same name is replaced. For an exact
path of your own, use `save_screenshot_to_path`. Files the framework names
itself (`save_screenshot_to_logs`) never replace an earlier one.

## Window introspection

```rust
let (x, y, width, height) = sb.get_window_rect().await?;
sb.maximize().await?;
sb.minimize().await?;
```

## On-page messaging

```rust
sb.post_success_message("Operation completed").await?;
sb.post_error_message("Something went wrong").await?;
sb.post_message_for("Transient message", 3).await?;
```

These helpers inject a visible banner into the page, useful for demos or
MasterQA reports.

## Console logs

```rust
sb.start_recording_console_logs().await?;
// Or use the alias: sb.console_log_script().await?;
sb.console_log_string("Custom log entry").await?;
let logs = sb.get_recorded_console_logs().await?;
```

## Referral / traffic helpers

```rust
let url = sb.generate_referral("https://example.com", "docs")?;
sb.generate_traffic("https://example.com", "docs").await?;
```

## Multi-series charts

```rust
sb.create_bar_chart("Sales").await?;
sb.add_data_point("Jan", 100).await?;
sb.add_data_point("Feb", 150).await?;
sb.add_series_to_chart("Last Year", &[("Jan".into(), 80), ("Feb".into(), 120)]).await?;
let path = sb.display_chart().await?;
```

## Deferred assertions

Deferred assertions collect failures and only fail the test when
`process_deferred_asserts` is called:

```rust
sb.deferred_assert_element_present("#result").await?;
sb.deferred_assert_exact_text("#result", "Done").await?;
sb.process_deferred_asserts().await?;
```

Use them when you want to validate many independent conditions and report all
failures at once.

## Running the example

```bash
cargo run --example remaining_helpers
```

The example demonstrates many of these helpers in sequence.
