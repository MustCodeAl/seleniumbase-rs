//! `sbase mkchart`: a starter HTML page with a Chart.js bar chart.

use crate::cli::commands::write_new_file;
use std::path::PathBuf;

/// Writes a Chart.js example page to `filename`.
///
/// Missing folders are created. A file that already exists is not replaced:
/// that is an [`std::io::ErrorKind::AlreadyExists`] error. The page loads
/// Chart.js 4 from jsDelivr, so it needs a network connection to draw.
pub fn make_chart(filename: &str) -> std::io::Result<PathBuf> {
    let path = PathBuf::from(filename);
    let content = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Test Results Chart</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4"></script>
</head>
<body>
    <canvas id="results"></canvas>
    <script>
        new Chart(document.getElementById('results'), {
            type: 'bar',
            data: {
                labels: ['Passed', 'Failed', 'Skipped'],
                datasets: [{
                    label: 'Test Results',
                    data: [10, 1, 2],
                    backgroundColor: ['green', 'red', 'orange']
                }]
            }
        });
    </script>
</body>
</html>"#;
    write_new_file(&path, content)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chart_page_pins_the_major_version_of_chartjs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out").join("chart.html");

        let written = make_chart(path.to_str().unwrap()).unwrap();

        let html = std::fs::read_to_string(written).unwrap();
        assert!(html.contains("npm/chart.js@4\""), "{html}");
        assert!(html.contains("<meta charset=\"utf-8\">"));
    }

    #[test]
    fn an_existing_page_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chart.html");
        std::fs::write(&path, "<p>mine</p>").unwrap();

        let error = make_chart(path.to_str().unwrap()).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "<p>mine</p>");
    }
}
