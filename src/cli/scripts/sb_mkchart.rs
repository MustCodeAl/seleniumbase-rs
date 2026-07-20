use std::path::PathBuf;

pub fn make_chart(filename: &str) -> std::io::Result<PathBuf> {
    let path = PathBuf::from(filename);
    let content = r#"<!DOCTYPE html>
<html>
<head>
    <title>Test Results Chart</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
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
    std::fs::write(&path, content)?;
    Ok(path)
}
