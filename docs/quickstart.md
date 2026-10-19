---
title: "Quickstart"
sidebarTitle: "Quickstart"
description: "Run your first seleniumbase-rs browser test in five minutes."
---

<Info>

**In short:** add one dependency, paste one program, run `cargo run`. You need
Rust 1.99 or newer and a Chromium-based browser (Chrome, Chromium or Edge).

</Info>

<Steps titleSize="h3">

<Step title="Create a project" icon="folder-plus">

```bash Terminal
cargo new my_first_test
cd my_first_test
```

</Step>

<Step title="Add the dependencies" icon="box">

The crate is installed from git (it is not on crates.io). `tokio` runs the async
code.

```toml Cargo.toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

</Step>

<Step title="Write a test" icon="code">

Pick an engine. The pure CDP engine talks to Chrome directly and needs no
driver; `BaseCase` uses WebDriver and starts `chromedriver` for you.

<Tabs>

<Tab title="Pure CDP (no driver)" icon="bolt">

```rust,no_run src/main.rs
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};

#[tokio::main]
async fn main() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
    let page = browser.default_page().await?;

    page.goto("seleniumbase.io/simple/login").await?; // https:// is added for you
    page.locator("#username").fill("demo_user").await?;
    page.locator("#password").fill("secret_pass").await?;
    page.locator("button").click().await?;
    page.locator("h1").expect().to_contain_text("Welcome").await?;

    browser.close().await?;
    Ok(())
}
```

</Tab>

<Tab title="BaseCase (WebDriver)" icon="flask">

```rust,no_run src/main.rs
use seleniumbase_rs::{BaseCase, BrowserConfig, DriverMode};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BrowserConfig::default()
        .with_mode(DriverMode::Uc)
        .with_headless(false);

    // Starts the browser, and chromedriver if needed.
    let mut sb = BaseCase::new(config).await?;

    sb.open("https://seleniumbase.io/simple/login").await?;
    sb.type_text("#username", "demo_user").await?;
    sb.type_text("#password", "secret_pass").await?;
    sb.click("#log-in").await?;
    sb.assert_text_visible("Welcome!", "body").await?;

    sb.quit().await?;
    Ok(())
}
```

</Tab>

</Tabs>

</Step>

<Step title="Run it" icon="play">

```bash Terminal
cargo run
```

The test opens the login page, signs in and checks the welcome text. A failed
check ends the program with an error that names the selector.

</Step>

</Steps>

<Tip>

No Rust project yet? The `sbrs` command line drives a browser on its own:
`cargo install --git https://github.com/MustCodeAl/seleniumbase-rs seleniumbase-rs-cli`,
then `sbrs --headless open https://example.com`. See the [CLI TL;DR](/help/cli_tldr).

</Tip>

## Next steps

<Columns cols={2}>

<Card title="Working with seleniumbase-rs" icon="rocket" color="#1a85d4" href="/tutorials/getting_started">
  The same steps in depth: configuration, a test with `#[tokio::test]`, startup errors.
</Card>

<Card title="Selectors" icon="crosshairs" color="#199f43" href="/tutorials/selectors">
  CSS, XPath and link text, and how a selector picks its strategy.
</Card>

<Card title="Waits and assertions" icon="hourglass-half" color="#d47628" href="/tutorials/waits_assertions">
  Wait for the page instead of sleeping, and check what it shows.
</Card>

<Card title="Headless and headed" icon="eye-slash" color="#a631be" href="/tutorials/headless">
  Run without a window in CI, and what changes when you do.
</Card>

<Card title="API reference" icon="code" color="#1ca1c7" href="/tutorials/api_reference">
  Every BaseCase method, by topic.
</Card>

<Card title="Common problems" icon="triangle-exclamation" color="#d52c36" href="/help/common_problems">
  Driver versions, sandbox errors, and other first-run issues.
</Card>

</Columns>
