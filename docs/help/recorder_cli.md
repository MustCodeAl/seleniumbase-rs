# Recorder CLI

The `BaseCase` recorder is primarily code-driven: every action you perform
through the API is stored in an internal [`ActionRecorder`]. This page explains
how to turn that recording into a Rust test or JSON scenario from the CLI, and
how to replay recorded scenarios.

## What you will learn

- How `BaseCase` records actions automatically.
- How to export a recording to Rust source or JSON from code.
- How to replay a JSON scenario with `sbase run-scenario`.

## How recording works

There is no separate "record" toggle. As soon as you call `BaseCase` methods
such as `open`, `click`, or `type_text`, the action is appended to the recorder:

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;

    sb.open("https://seleniumbase.io/demo_page").await?;
    sb.click("#myButton").await?;
    sb.type_text("#myInput", "hello").await?;

    // Save the recorded actions as JSON in the logs directory.
    sb.save_recorded_actions("scenario.json")?;

    // Or emit a standalone Rust test.
    let rust_test = sb.export_recording_as_rust()?;
    std::fs::write("my_test.rs", rust_test)?;

    sb.quit().await?;
    Ok(())
}
```

## Replay a JSON scenario from the CLI

Once you have a JSON scenario file, replay it with:

```bash
cargo run --bin sbase -- run-scenario --file scenario.json
```

You can also generate a dashboard while replaying:

```bash
cargo run --bin sbase -- run-scenario --file scenario.json --dashboard dashboard.html
```

## Scenario JSON format

Each entry in the JSON array maps to a `BaseCase` method. A minimal scenario
looks like:

```json
[
  { "action": "open", "target": "https://seleniumbase.io/demo_page" },
  { "action": "click", "target": "#myButton" },
  { "action": "type_text", "target": "#myInput", "value": "hello" }
]
```

The `action` field is the snake-case method name. `target` and `value` are
optional and correspond to the first and second method arguments.

## Best practices

- Review generated code before committing it.
- Replace brittle auto-generated selectors with stable `data-testid`
  attributes.
- Add explicit assertions after the recording stops.
- Keep scenario files under version control so CI replays the same steps.
