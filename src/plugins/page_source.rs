use crate::plugins::base_plugin::SeleniumBasePlugin;
use std::fs;
use std::path::Path;

/// Plugin that saves page source on failure.
pub struct PageSourceOnFailurePlugin {
    pub output_dir: String,
}

impl PageSourceOnFailurePlugin {
    pub fn new(output_dir: impl Into<String>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }
}

impl SeleniumBasePlugin for PageSourceOnFailurePlugin {
    fn on_failure(&mut self, command: &str, target: &str, _value: &str, error: &str) {
        let dir = Path::new(&self.output_dir);
        fs::create_dir_all(dir).ok();
        let safe = target.replace(['/', '\\', ':', ' '], "_");
        let name = format!(
            "{}_{}_{}.html",
            safe,
            command.replace(' ', "_"),
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );
        let path = dir.join(name);
        fs::write(
            &path,
            format!(
                "<html><body><h1>Failure context</h1>\
                 <p>Command: {}</p><p>Target: {}</p><p>Error: {}</p></body></html>",
                command, target, error
            ),
        )
        .ok();
    }
}
