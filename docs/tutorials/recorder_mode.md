# Recorder Mode Guide

Recorder Mode captures your browser interactions and generates a Rust test or JSON scenario that replays them.

## Record from the CLI

```bash
cargo run --bin sbase -- recorder --output generated_test.rs
```

The browser opens on a default page. Perform the actions you want to capture, then close the browser. The generated Rust test is written to `generated_test.rs`.

## Record from code

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    sb.open("https://seleniumbase.io/demo_page").await?;

    sb.start_recording().await?;
    sb.click("#myButton").await?;
    sb.type_text("#myInput", "hello").await?;
    sb.stop_recording().await?;

    let rust_test = sb.generate_rust_test("MyRecordedTest").await?;
    std::fs::write("my_test.rs", rust_test)?;

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

## JSON scenarios

You can also export to a JSON scenario for the low-code runner:

```bash
cargo run --bin sbase -- recorder --format json --output scenario.json
```

Then replay it:

```bash
cargo run --bin sbase -- run-scenario --file scenario.json
```

## Tips

- Wait for elements to settle before acting; the recorder captures explicit waits automatically.
- Review the generated code and add assertions where needed.
- Use meaningful selectors in the application under test so generated locators remain stable.
