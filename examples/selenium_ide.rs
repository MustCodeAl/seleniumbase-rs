use seleniumbase_rs::utilities::selenium_ide::parse_ide_html;

/// Demonstrates parsing a legacy Selenium IDE HTML test case into commands.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let html = r#"<table>
        <tr><td>open</td><td>/login</td><td></td></tr>
        <tr><td>type</td><td>id=username</td><td>admin</td></tr>
        <tr><td>click</td><td>id=submit</td><td></td></tr>
    </table>"#;

    let commands = parse_ide_html(html)?;
    for cmd in &commands {
        println!(
            "command={:<10} target={:<20} value={}",
            cmd.command, cmd.target, cmd.value
        );
    }
    Ok(())
}
