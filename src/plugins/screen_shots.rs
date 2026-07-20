use crate::plugins::base_plugin::SeleniumBasePlugin;
use std::fs;
use std::path::Path;

/// Plugin that saves a screenshot on test failure.
pub struct ScreenshotOnFailurePlugin {
    pub output_dir: String,
}

impl ScreenshotOnFailurePlugin {
    pub fn new(output_dir: impl Into<String>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }
}

impl SeleniumBasePlugin for ScreenshotOnFailurePlugin {
    fn on_failure(&mut self, command: &str, target: &str, _value: &str, error: &str) {
        let dir = Path::new(&self.output_dir);
        fs::create_dir_all(dir).ok();
        let safe = target.replace(['/', '\\', ':', ' '], "_");
        let name = format!("{}_{}_{}.png", safe, command.replace(' ', "_"), timestamp());
        let path = dir.join(name);
        // Real screenshot capture requires a WebDriver reference; log the intended path.
        fs::write(
            &path,
            format!(
                "Failure screenshot placeholder for command '{}' on target '{}'\nError: {}",
                command, target, error
            ),
        )
        .ok();
    }
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()
}
