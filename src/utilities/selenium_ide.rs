use regex::Regex;
use std::fs;
use std::path::Path;

/// A command extracted from a Selenium IDE HTML test case.
#[derive(Debug, Clone, PartialEq)]
pub struct IdeCommand {
    pub command: String,
    pub target: String,
    pub value: String,
}

/// Parse a legacy Selenium IDE HTML file and extract commands.
pub fn parse_ide_file<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<IdeCommand>, Box<dyn std::error::Error>> {
    let html = fs::read_to_string(path)?;
    parse_ide_html(&html)
}

/// Parse a Selenium IDE HTML string and extract commands.
pub fn parse_ide_html(html: &str) -> Result<Vec<IdeCommand>, Box<dyn std::error::Error>> {
    let row_re = Regex::new(r"<tr[^>]*>(.*?)</tr>")?;
    let cell_re = Regex::new(r"<td[^>]*>(.*?)</td>")?;
    let mut commands = Vec::new();
    for row in row_re.captures_iter(html) {
        let mut cells: Vec<String> = Vec::new();
        for cap in cell_re.captures_iter(&row[1]) {
            cells.push(strip_tags(&cap[1]));
        }
        if cells.len() >= 3 {
            commands.push(IdeCommand {
                command: cells[0].trim().to_string(),
                target: cells[1].trim().to_string(),
                value: cells[2].trim().to_string(),
            });
        }
    }
    Ok(commands)
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    re.replace_all(s, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ide_html() {
        let html = r#"<table>
            <tr><td>open</td><td>/login</td><td></td></tr>
            <tr><td>type</td><td>id=user</td><td>admin</td></tr>
        </table>"#;
        let cmds = parse_ide_html(html).unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].command, "open");
        assert_eq!(cmds[1].target, "id=user");
    }
}
