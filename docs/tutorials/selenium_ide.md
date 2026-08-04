# Selenium IDE Migration

If you have legacy Selenium IDE `.html` test cases, SeleniumBase for Rust can
parse them into a list of commands that you can replay with `BaseCase`. This
guide shows how to migrate those tests into Rust code.

## What you will learn

- How to parse legacy `.html` IDE files and strings.
- How to replay parsed commands with `BaseCase`.
- Which commands map directly and which need manual translation.

## Parse an IDE HTML file

```rust
use seleniumbase_rs::utilities::selenium_ide::parse_ide_file;

let commands = parse_ide_file("tests/login.html")?;
for cmd in &commands {
    println!("{} {} {}", cmd.command, cmd.target, cmd.value);
}
```

## Parse an HTML string

```rust
use seleniumbase_rs::utilities::selenium_ide::parse_ide_html;

let html = r#"<table>
    <tr><td>open</td><td>/login</td><td></td></tr>
    <tr><td>type</td><td>id=username</td><td>admin</td></tr>
    <tr><td>click</td><td>id=submit</td><td></td></tr>
</table>"#;

let commands = parse_ide_html(html)?;
```

## Replay commands with BaseCase

```rust
use seleniumbase_rs::{BaseCase, BrowserConfig};
use seleniumbase_rs::utilities::selenium_ide::parse_ide_file;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut sb = BaseCase::new(BrowserConfig::default()).await?;
    let commands = parse_ide_file("tests/login.html")?;

    for cmd in commands {
        match cmd.command.as_str() {
            "open" => sb.open(&cmd.target).await?,
            "type" => sb.type_text(&cmd.target, &cmd.value).await?,
            "click" => sb.click(&cmd.target).await?,
            "assertText" => sb.assert_text_visible(&cmd.value, &cmd.target).await?,
            _ => println!("Unhandled command: {}", cmd.command),
        }
    }
    sb.quit().await?;
    Ok(())
}
```

## Supported command format

The parser expects a simple HTML table with three cells per row:

```html
<tr>
  <td>command</td>
  <td>target</td>
  <td>value</td>
</tr>
```

Rows with fewer than three cells are ignored. The `SeleniumIdeCommand` struct
exposes `command`, `target`, and `value` as `String` fields.

## Common command mappings

| IDE command | `BaseCase` method |
|---|---|
| `open` | `open(url)` |
| `type` | `type_text(selector, text)` |
| `click` | `click(selector)` |
| `assertText` | `assert_text_visible(text, selector)` |
| `waitForElementPresent` | `wait_for_element_present(selector).await?` |
| `pause` | `tokio::time::sleep(...).await` |

## Limitations

- This parser targets the legacy Selenium IDE HTML format, not the modern
  `.side` JSON format.
- Flow-control commands (`if`, `while`, `storeEval`) are not interpreted;
  map them to Rust control flow manually.
- Variables and stored values are not automatically carried over; use Rust
  variables or the `store_*` helpers if available.
