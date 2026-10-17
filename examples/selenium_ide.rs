//! Parse a legacy Selenium IDE HTML test case into commands, and map them onto
//! `BaseCase` calls.
//!
//! Selenium IDE's old HTML export is a table with one `command | target |
//! value` row per step. `parse_ide_html` (and `parse_ide_file`, for a path)
//! reads the rows that have three cells, strips tags, and decodes entities such
//! as `&amp;`; a title row with a single cell is skipped. Targets keep Selenium
//! IDE's locator prefixes (`id=`, `name=`, `css=`, `xpath=`, `link=`), which
//! `BaseCase` selectors do not understand as written, so the example also
//! translates each locator and prints the `BaseCase` call a replay would make.
//!
//! Run it with:
//!
//! ```bash
//! cargo run --example selenium_ide
//! ```
//!
//! It needs no browser and no network. The modern `.side` JSON format is not
//! supported.

use seleniumbase_rs::utilities::selenium_ide::{parse_ide_file, parse_ide_html, IdeCommand};

/// A test case as Selenium IDE writes it: a title row, entities in the cells,
/// and a cell that spans two lines.
const TEST_CASE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<html lang="en">
<head><title>Login test</title></head>
<body>
<table cellpadding="1" cellspacing="1" border="1">
<thead>
<tr><td rowspan="1" colspan="3">Login test</td></tr>
</thead>
<tbody>
<tr><td>open</td><td>/login?next=/home&amp;lang=en</td><td></td></tr>
<tr><td>type</td><td>id=username</td><td>admin</td></tr>
<tr><td>type</td><td>name=password</td><td>not-a-real-password</td></tr>
<tr><td>click</td><td>css=button.submit</td><td></td></tr>
<tr><td>assertText</td><td>xpath=//h1
</td><td>Welcome &amp; hello</td></tr>
<tr><td>pause</td><td>500</td><td></td></tr>
</tbody></table>
</body></html>
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let commands = parse_ide_html(TEST_CASE)?;
    println!("{} commands:", commands.len());
    for cmd in &commands {
        println!(
            "command={:<11} target={:<28} value={}",
            cmd.command, cmd.target, cmd.value
        );
    }

    // The title row has one cell, so it is not a command.
    if commands.len() != 6 || commands[0].target != "/login?next=/home&lang=en" {
        return Err(format!("unexpected parse result: {commands:?}").into());
    }

    // Reading from a file gives the same commands.
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("login_test.html");
    std::fs::write(&path, TEST_CASE)?;
    if parse_ide_file(&path)? != commands {
        return Err("parse_ide_file disagrees with parse_ide_html".into());
    }

    println!("\nAs BaseCase calls:");
    for cmd in &commands {
        println!("  {}", base_case_call(cmd));
    }
    Ok(())
}

/// The `BaseCase` selector for a Selenium IDE locator.
fn selector_for(target: &str) -> String {
    match target.split_once('=') {
        Some(("id", id)) => format!("#{id}"),
        Some(("name", name)) => format!("[name='{name}']"),
        Some(("css" | "xpath", locator)) => locator.to_owned(),
        // `link=Text` is already a BaseCase link-text selector.
        _ => target.to_owned(),
    }
}

/// The line of Rust that replays `cmd`, or a comment if it needs hand-writing.
fn base_case_call(cmd: &IdeCommand) -> String {
    let selector = selector_for(&cmd.target);
    match cmd.command.as_str() {
        "open" => format!(
            "sb.open({:?}).await?; // join with your base URL",
            cmd.target
        ),
        "type" => format!("sb.type_text({selector:?}, {:?}).await?;", cmd.value),
        "click" => format!("sb.click({selector:?}).await?;"),
        "assertText" => format!("sb.assert_text({:?}, {selector:?}).await?;", cmd.value),
        other => format!("// `{other}` has no direct BaseCase call; write it by hand"),
    }
}
