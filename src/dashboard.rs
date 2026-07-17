use serde::Serialize;
use std::path::Path;

use crate::error::SeleniumBaseError;

#[derive(Clone, Debug, Serialize)]
pub struct RunSummary {
    pub scenario_name: String,
    pub total_steps: usize,
    pub passed_steps: usize,
    pub failed_steps: usize,
    pub errors: Vec<String>,
}

pub fn write_dashboard_html(
    summary: &RunSummary,
    path: impl AsRef<Path>,
) -> Result<(), SeleniumBaseError> {
    let safe_title = html_escape(&summary.scenario_name);
    let errors_html = if summary.errors.is_empty() {
        "<li>None</li>".to_owned()
    } else {
        summary
            .errors
            .iter()
            .map(|e| format!("<li>{}</li>", html_escape(e)))
            .collect::<Vec<_>>()
            .join("")
    };
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title>\
         <style>body{{font-family:Arial,sans-serif;margin:24px}}table{{border-collapse:collapse}}td,th{{border:1px solid #ccc;padding:8px}}</style>\
         </head><body><h1>{}</h1><table><tr><th>Total</th><th>Passed</th><th>Failed</th></tr>\
         <tr><td>{}</td><td>{}</td><td>{}</td></tr></table><h2>Errors</h2><ul>{}</ul></body></html>",
        safe_title,
        safe_title,
        summary.total_steps,
        summary.passed_steps,
        summary.failed_steps,
        errors_html
    );
    std::fs::write(path, html).map_err(|e| {
        SeleniumBaseError::InvalidConfig(format!("failed to write dashboard html: {e}"))
    })?;
    Ok(())
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
