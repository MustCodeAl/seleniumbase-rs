# Recorder Mode Guide

Recorder Mode captures your browser interactions and generates either a Rust
source file or a JSON scenario that can be replayed later. It is a fast way to
bootstrap a UI test: point, click, type, assert, and then refine the generated
output.

## What you will learn

- How to record from the CLI and from code.
- What the generated Rust test looks like.
- How to export and replay JSON scenarios.
- Tips for keeping generated tests maintainable.

## Record from code

`BaseCase` records every action you perform through its API automatically. There
is no separate start/stop toggle for the default recorder; simply run the
interactions you want to capture and then export the recording.

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    sb.click("#myButton").await?;
    sb.type_text("#myInput", "hello").await?;

    // Export the recorded actions as Rust source and JSON.
    let rust_test = sb.export_recording_as_rust()?;
    std::fs::write("my_test.rs", rust_test)?;

    // Or save both formats to the logs directory in one call.
    let (_json, _rust) = sb.save_recording_to_logs()?;

    sb.quit().await?;
    Ok(())
}
```

## Generated output

The recorder produces method calls such as:

```rust
sb.click("#myButton").await?;
sb.type_text("#myInput", "hello").await?;
sb.assert_text_visible("Success!", "body").await?;
```

Generated tests use the `sb_test!` macro shape and include a default timeout and
assertion helpers. Always review the output before committing it to your suite.

## JSON scenarios

Recorded actions can also be serialized as JSON for the low-code scenario
runner. From code:

```rust
sb.save_recorded_actions("scenario.json")?;
```

Then replay it with the CLI:

```bash
cargo run --bin sbase -- run-scenario --file scenario.json
```

JSON scenarios are portable and can be executed by CI jobs without recompiling
a test binary.

## Best practices

- Wait for elements to settle before acting; the recorder captures explicit waits automatically.
- Review the generated code and add assertions where needed.
- Use meaningful selectors in the application under test so generated locators remain stable.
- Run the generated test immediately to confirm it passes before editing it.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Recording is empty | Actions were performed without a `BaseCase` method (e.g. raw JS) | Use `BaseCase` interaction methods so the internal recorder captures them. |
| Generated selector is brittle | Page uses auto-generated IDs | Replace recorded selectors with stable `data-testid` or class selectors. |
| JSON scenario fails on replay | Target application changed | Regenerate or hand-edit the scenario. |
