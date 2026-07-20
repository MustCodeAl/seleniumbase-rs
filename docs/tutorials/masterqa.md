# MasterQA Guide

MasterQA is a hybrid testing workflow that mixes automated navigation with manual verification steps.

## Start a MasterQA session

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    sb.start_master_qa("Verify Login Page").await?;

    // Automated steps
    sb.type_text("#username", "demo_user").await?;
    sb.type_text("#password", "secret_pass").await?;
    sb.click("#log-in").await?;

    // Manual verification step
    sb.verify("Confirm the dashboard loads with the user avatar visible.").await?;

    sb.stop_master_qa().await?;
    sb.quit().await?;
    Ok(())
}
```

## How it works

- `start_master_qa(title)` initializes a Markdown report.
- `verify(instruction)` pauses execution and prompts the operator with the instruction.
- `stop_master_qa()` writes the final report to `latest_logs/masterqa_report.md`.

## Report format

The generated Markdown report contains:

- Test title and timestamp.
- Each automated step.
- Each manual verification step with pass/fail status.
- Summary of results.

## When to use MasterQA

- Exploratory testing where full automation is too expensive.
- Validating visual layouts that are hard to assert programmatically.
- Compliance checks that require human sign-off.
