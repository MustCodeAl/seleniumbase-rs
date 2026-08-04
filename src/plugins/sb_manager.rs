use crate::plugins::base_plugin::SeleniumBasePlugin;

/// Manager that holds registered plugins and broadcasts hook calls.
pub struct PluginManager {
    plugins: Vec<Box<dyn SeleniumBasePlugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn register(&mut self, plugin: Box<dyn SeleniumBasePlugin>) {
        self.plugins.push(plugin);
    }

    pub fn on_start(&mut self) {
        for p in &mut self.plugins {
            p.on_start();
        }
    }

    pub fn before_command(&mut self, name: &str, target: &str, value: &str) {
        for p in &mut self.plugins {
            p.before_command(name, target, value);
        }
    }

    pub fn after_command(&mut self, name: &str, target: &str, value: &str, passed: bool) {
        for p in &mut self.plugins {
            p.after_command(name, target, value, passed);
        }
    }

    pub fn on_failure(&mut self, command: &str, target: &str, value: &str, error: &str) {
        for p in &mut self.plugins {
            p.on_failure(command, target, value, error);
        }
    }

    pub fn on_stop(&mut self) {
        for p in &mut self.plugins {
            p.on_stop();
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}
