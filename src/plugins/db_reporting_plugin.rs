use crate::core::report_helper::{write_json_report, TestResult};
use crate::plugins::base_plugin::SeleniumBasePlugin;
use std::fs;
use std::path::Path;

/// Plugin that writes a JSON report for each test result.
pub struct DbReportingPlugin {
    pub output_dir: String,
    results: Vec<TestResult>,
}

impl DbReportingPlugin {
    pub fn new(output_dir: impl Into<String>) -> Self {
        Self {
            output_dir: output_dir.into(),
            results: Vec::new(),
        }
    }
}

impl SeleniumBasePlugin for DbReportingPlugin {
    fn after_command(&mut self, name: &str, _target: &str, _value: &str, passed: bool) {
        self.results.push(TestResult {
            name: name.into(),
            passed,
            duration_secs: 0.0,
            message: if passed { "ok".into() } else { "failed".into() },
        });
    }

    fn on_stop(&mut self) {
        let dir = Path::new(&self.output_dir);
        fs::create_dir_all(dir).ok();
        let path = dir.join("report.json");
        write_json_report(path, &self.results).ok();
    }
}
