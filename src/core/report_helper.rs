use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

/// Result of a single test.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub duration_secs: f64,
    pub message: String,
}

/// Write an HTML report aggregating test results.
pub fn write_html_report<P: AsRef<Path>>(path: P, results: &[TestResult]) -> std::io::Result<()> {
    let mut rows = String::new();
    for r in results {
        let status = if r.passed { "PASS" } else { "FAIL" };
        let color = if r.passed { "green" } else { "red" };
        rows.push_str(&format!(
            "<tr><td>{}</td><td style='color:{}'>{}</td><td>{:.3}s</td><td>{}</td></tr>\n",
            html_escape(&r.name),
            color,
            status,
            r.duration_secs,
            html_escape(&r.message)
        ));
    }
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset='utf-8'><title>Test Report</title></head>\
         <body><h1>SeleniumBase Rust Test Report</h1>\
         <table border='1'><tr><th>Test</th><th>Status</th><th>Duration</th><th>Message</th></tr>\
         {}</table></body></html>",
        rows
    );
    let mut file = fs::File::create(path)?;
    file.write_all(html.as_bytes())?;
    Ok(())
}

/// Write a JSON report aggregating test results.
pub fn write_json_report<P: AsRef<Path>>(path: P, results: &[TestResult]) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    fs::write(path, json)?;
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_report() {
        let tmp = std::env::temp_dir().join("sb_report_test.html");
        let results = vec![TestResult {
            name: "test_a".into(),
            passed: true,
            duration_secs: 0.5,
            message: "ok".into(),
        }];
        write_html_report(&tmp, &results).unwrap();
        let data = fs::read_to_string(&tmp).unwrap();
        assert!(data.contains("PASS"));
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn test_json_report() {
        let tmp = std::env::temp_dir().join("sb_report_test.json");
        let results = vec![TestResult {
            name: "test_b".into(),
            passed: false,
            duration_secs: 1.2,
            message: "bad".into(),
        }];
        write_json_report(&tmp, &results).unwrap();
        let data = fs::read_to_string(&tmp).unwrap();
        assert!(data.contains("test_b"));
        let _ = fs::remove_file(&tmp);
    }
}
