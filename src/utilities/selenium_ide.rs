use regex::{Captures, Regex};
use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use thiserror::Error;

/// A command extracted from a Selenium IDE HTML test case.
#[derive(Debug, Clone, PartialEq)]
pub struct IdeCommand {
    /// The command name, such as `open` or `click`.
    pub command: String,
    /// The selector or URL the command acts on.
    pub target: String,
    /// The text or option the command takes. Empty when the command has none.
    pub value: String,
}

/// The HTML file could not be read.
///
/// [`Display`](std::fmt::Display) is the operating-system error's text.
/// [`Error::source`](std::error::Error::source) is that error.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IdeParseError {
    /// Reading the file failed.
    #[error("{source}")]
    Read {
        /// The operating-system error.
        #[from]
        source: std::io::Error,
    },
}

/// Parse a legacy Selenium IDE HTML file and extract commands.
///
/// # Errors
///
/// Returns [`IdeParseError::Read`] when the file cannot be read.
pub fn parse_ide_file<P: AsRef<Path>>(path: P) -> Result<Vec<IdeCommand>, IdeParseError> {
    let html = fs::read_to_string(path)?;
    Ok(parse_ide_html(&html))
}

/// Compiles `pattern` once and reuses it.
fn cached(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("the pattern is a constant and valid"))
}

/// Parse a Selenium IDE HTML string and extract commands.
///
/// A row with at least three cells is a command: the command name, its target
/// and its value. Tags are removed from the cells and HTML entities decoded, so
/// `&amp;` in a URL comes out as `&`. Rows may span lines, as Selenium IDE writes
/// them. HTML that is not a table yields an empty list. Reading a string
/// cannot fail.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utilities::selenium_ide::parse_ide_html;
///
/// let commands = parse_ide_html(
///     "<table><tr><td>open</td><td>/login</td><td></td></tr></table>",
/// );
/// assert_eq!(commands[0].command, "open");
/// assert_eq!(commands[0].target, "/login");
/// assert!(commands[0].value.is_empty());
/// ```
#[must_use]
pub fn parse_ide_html(html: &str) -> Vec<IdeCommand> {
    static ROW: OnceLock<Regex> = OnceLock::new();
    static CELL: OnceLock<Regex> = OnceLock::new();
    // `s` lets `.` match a newline; `i` accepts `<TR>` as well as `<tr>`.
    let row_re = cached(&ROW, r"(?is)<tr\b[^>]*>(.*?)</tr>");
    let cell_re = cached(&CELL, r"(?is)<td\b[^>]*>(.*?)</td>");
    let mut commands = Vec::new();
    for row in row_re.captures_iter(html) {
        let cells: Vec<String> = cell_re
            .captures_iter(&row[1])
            .map(|cell| decode_entities(&strip_tags(&cell[1])))
            .collect();
        if cells.len() >= 3 {
            commands.push(IdeCommand {
                command: cells[0].trim().to_string(),
                target: cells[1].trim().to_string(),
                value: cells[2].trim().to_string(),
            });
        }
    }
    commands
}

fn strip_tags(s: &str) -> String {
    static TAG: OnceLock<Regex> = OnceLock::new();
    cached(&TAG, r"(?s)<[^>]+>").replace_all(s, "").to_string()
}

/// Decodes the entities Selenium IDE writes: the named ones that matter in
/// markup, a non-breaking space, and decimal or hex character references.
///
/// `&amp;` is decoded in the same pass as the others, never first, so
/// `&amp;lt;` becomes the text `&lt;` and not `<`.
fn decode_entities(s: &str) -> String {
    static ENTITY: OnceLock<Regex> = OnceLock::new();
    cached(&ENTITY, r"&(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z]+);")
        .replace_all(s, |cap: &Captures| {
            let name = &cap[1];
            let decoded = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => name.strip_prefix('#').and_then(|digits| {
                    let code = match digits.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => digits.parse().ok()?,
                    };
                    char::from_u32(code)
                }),
            };
            // An entity this does not know stays as written.
            decoded.map_or_else(|| cap[0].to_owned(), String::from)
        })
        .into_owned()
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
        let cmds = parse_ide_html(html);
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].command, "open");
        assert_eq!(cmds[1].target, "id=user");
    }

    #[test]
    fn rows_that_span_lines_are_read_as_selenium_ide_writes_them() {
        let html = "<table>\n<thead><tr><td rowspan=\"1\" colspan=\"3\">My test</td></tr></thead>\n<tbody>\n<TR>\n\t<td>open</td>\n\t<td>/</td>\n\t<td></td>\n</TR>\n<tr>\n\t<td>click</td>\n\t<td>css=a.go\n\t</td>\n\t<td></td>\n</tr>\n</tbody></table>";
        let cmds = parse_ide_html(html);
        assert_eq!(
            cmds,
            vec![
                IdeCommand {
                    command: "open".into(),
                    target: "/".into(),
                    value: String::new()
                },
                IdeCommand {
                    command: "click".into(),
                    target: "css=a.go".into(),
                    value: String::new()
                },
            ],
            "the title row has one cell and is not a command"
        );
    }

    #[test]
    fn entities_in_a_cell_are_decoded() {
        let html = "<tr><td>open</td><td>/search?a=1&amp;b=2</td><td>&lt;b&gt; &quot;hi&quot; &#39;x&#39; &#x41;&nbsp;</td></tr>";
        let cmds = parse_ide_html(html);
        assert_eq!(cmds[0].target, "/search?a=1&b=2");
        assert_eq!(cmds[0].value, "<b> \"hi\" 'x' A");
    }

    #[test]
    fn an_escaped_ampersand_is_decoded_once_only() {
        assert_eq!(decode_entities("&amp;lt;"), "&lt;");
        assert_eq!(decode_entities("&unknown; &#xZZ;"), "&unknown; &#xZZ;");
    }

    #[test]
    fn the_datalist_a_newer_export_adds_does_not_leak_into_the_target() {
        let html = "<tr><td>click</td><td>css=a<datalist><option value=\"css=a\"></option></datalist></td><td></td></tr>";
        assert_eq!(parse_ide_html(html)[0].target, "css=a");
    }

    #[test]
    fn a_missing_file_keeps_the_io_error() {
        let error = parse_ide_file("/no/such/selenium-ide.html").unwrap_err();
        let source = std::error::Error::source(&error).expect("the io error");
        let io = source
            .downcast_ref::<std::io::Error>()
            .expect("an io error");
        assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
        assert_eq!(error.to_string(), io.to_string());
    }
}
