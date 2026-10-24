//! What the client commands print: tables for people, JSON for scripts, files for images.
//!
//! Every writer takes the stream to write to, so a test can capture it.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use seleniumbase_rs::mcp::client::{
    CallOutcome, ClientProgress, PromptOutcome, PromptSummary, ResourceContent, ResourceSummary,
    ToolSummary,
};
use serde_json::{json, Value};

/// The first line of a description, for a table.
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default().trim()
}

/// The text with a line break at its end, if it has none.
fn line(out: &mut dyn Write, text: &str) -> io::Result<()> {
    out.write_all(text.as_bytes())?;
    if !text.ends_with('\n') {
        out.write_all(b"\n")?;
    }
    Ok(())
}

/// The tools in a table: the name and the first line of the description.
pub fn write_tools(tools: &[ToolSummary], raw: bool, out: &mut dyn Write) -> io::Result<()> {
    if raw {
        let listed: Vec<Value> = tools.iter().map(tool_json).collect();
        return line(out, &Value::Array(listed).to_string());
    }
    let width = tools.iter().map(|tool| tool.name.len()).max().unwrap_or(0);
    for tool in tools {
        let description = first_line(tool.description.as_deref().unwrap_or_default());
        writeln!(out, "{:<width$}  {description}", tool.name)?;
    }
    Ok(())
}

fn tool_json(tool: &ToolSummary) -> Value {
    let mut value = json!({
        "name": tool.name,
        "description": tool.description,
        "inputSchema": tool.input_schema,
    });
    if let Some(title) = &tool.title {
        value["title"] = json!(title);
    }
    if let Some(schema) = &tool.output_schema {
        value["outputSchema"] = schema.clone();
    }
    value
}

/// One tool in full: its description and the schema of its arguments.
pub fn write_tool(tool: &ToolSummary, raw: bool, out: &mut dyn Write) -> io::Result<()> {
    if raw {
        return line(out, &tool_json(tool).to_string());
    }
    writeln!(out, "{}", tool.name)?;
    if let Some(description) = &tool.description {
        writeln!(out, "\n{description}")?;
    }
    writeln!(out, "\narguments (JSON Schema):")?;
    line(out, &pretty(&tool.input_schema))?;
    if let Some(schema) = &tool.output_schema {
        writeln!(out, "\nstructured result (JSON Schema):")?;
        line(out, &pretty(schema))?;
    }
    Ok(())
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// What a call returned: its text, or with `raw` its structured result as compact JSON (and
/// the text when it has none).
pub fn write_outcome(outcome: &CallOutcome, raw: bool, out: &mut dyn Write) -> io::Result<()> {
    match (&outcome.structured, raw) {
        (Some(structured), true) => line(out, &structured.to_string()),
        _ => line(out, &outcome.text),
    }
}

/// One call of a batch as a JSON line.
pub fn write_outcome_json(
    tool: &str,
    outcome: &CallOutcome,
    out: &mut dyn Write,
) -> io::Result<()> {
    line(
        out,
        &json!({
            "tool": tool,
            "isError": outcome.is_error,
            "text": outcome.text,
            "structured": outcome.structured,
        })
        .to_string(),
    )
}

/// The file extension for an image of this MIME type.
fn extension(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or_default().trim() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

/// Writes the images of `outcome` to `dir` as `image-N.ext`, numbering from `*counter + 1`,
/// and returns the paths.
pub fn save_images(
    outcome: &CallOutcome,
    dir: &Path,
    counter: &mut usize,
) -> io::Result<Vec<PathBuf>> {
    if outcome.images.is_empty() {
        return Ok(Vec::new());
    }
    fs::create_dir_all(dir)?;
    let mut paths = Vec::new();
    for (mime, bytes) in &outcome.images {
        *counter += 1;
        let path = dir.join(format!("image-{counter}.{}", extension(mime)));
        fs::write(&path, bytes)?;
        paths.push(path);
    }
    Ok(paths)
}

/// A progress report as the line `sbrs-mcp --progress` prints.
pub fn progress_line(report: &ClientProgress) -> String {
    let mut text = match report.total {
        Some(total) => format!("progress: {}/{}", number(report.current), number(total)),
        None => format!("progress: {}", number(report.current)),
    };
    if let Some(message) = &report.message {
        text.push(' ');
        text.push_str(message);
    }
    text
}

/// A whole number without its fraction.
fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

/// The resources in a table: the URI, the name and the first line of the description.
pub fn write_resources(
    resources: &[ResourceSummary],
    raw: bool,
    out: &mut dyn Write,
) -> io::Result<()> {
    if raw {
        let listed: Vec<Value> = resources
            .iter()
            .map(|r| {
                json!({
                    "uri": r.uri, "name": r.name, "title": r.title,
                    "description": r.description, "mimeType": r.mime_type,
                    "template": r.template,
                })
            })
            .collect();
        return line(out, &Value::Array(listed).to_string());
    }
    let width = resources.iter().map(|r| r.uri.len()).max().unwrap_or(0);
    for resource in resources {
        let kind = if resource.template { " (template)" } else { "" };
        let description = first_line(resource.description.as_deref().unwrap_or_default());
        writeln!(
            out,
            "{:<width$}  {}{kind}  {description}",
            resource.uri, resource.name
        )?;
    }
    Ok(())
}

/// What a resource holds: text is printed; a binary piece is saved in `dir` as
/// `resource-N.ext`, or named on standard error when there is no `dir`.
pub fn write_contents(
    contents: &[ResourceContent],
    dir: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> io::Result<()> {
    for (index, piece) in contents.iter().enumerate() {
        if let Some(text) = &piece.text {
            line(out, text)?;
        } else if let Some(bytes) = &piece.blob {
            let mime = piece
                .mime_type
                .as_deref()
                .unwrap_or("application/octet-stream");
            match dir {
                Some(dir) => {
                    fs::create_dir_all(dir)?;
                    let path = dir.join(format!("resource-{}.{}", index + 1, extension(mime)));
                    fs::write(&path, bytes)?;
                    writeln!(
                        err,
                        "sbrs-mcp: saved {} ({mime}, {} bytes)",
                        path.display(),
                        bytes.len()
                    )?;
                }
                None => writeln!(
                    err,
                    "sbrs-mcp: {} is binary ({mime}, {} bytes); --out DIR saves it",
                    piece.uri,
                    bytes.len()
                )?,
            }
        }
    }
    Ok(())
}

/// The prompts in a table: the name, its arguments (`*` marks a required one) and the first
/// line of the description.
pub fn write_prompts(prompts: &[PromptSummary], raw: bool, out: &mut dyn Write) -> io::Result<()> {
    if raw {
        let listed: Vec<Value> = prompts
            .iter()
            .map(|p| {
                let arguments: Vec<Value> = p
                    .arguments
                    .iter()
                    .map(|a| json!({ "name": a.name, "description": a.description, "required": a.required }))
                    .collect();
                json!({ "name": p.name, "title": p.title, "description": p.description, "arguments": arguments })
            })
            .collect();
        return line(out, &Value::Array(listed).to_string());
    }
    let width = prompts.iter().map(|p| p.name.len()).max().unwrap_or(0);
    for prompt in prompts {
        let arguments: Vec<String> = prompt
            .arguments
            .iter()
            .map(|a| format!("{}{}", a.name, if a.required { "*" } else { "" }))
            .collect();
        let description = first_line(prompt.description.as_deref().unwrap_or_default());
        writeln!(
            out,
            "{:<width$}  ({})  {description}",
            prompt.name,
            arguments.join(", ")
        )?;
    }
    Ok(())
}

/// A rendered prompt: each message with its role, or with `raw` as JSON.
pub fn write_prompt(prompt: &PromptOutcome, raw: bool, out: &mut dyn Write) -> io::Result<()> {
    if raw {
        let messages: Vec<Value> = prompt
            .messages
            .iter()
            .map(|(role, text)| json!({ "role": role, "text": text }))
            .collect();
        return line(
            out,
            &json!({ "description": prompt.description, "messages": messages }).to_string(),
        );
    }
    for (role, text) in &prompt.messages {
        writeln!(out, "[{role}]")?;
        line(out, text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
