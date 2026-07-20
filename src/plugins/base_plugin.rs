/// Plugin trait for SeleniumBase test hooks.
///
/// Each hook has a no-op default so plugins only implement the methods they care about.
pub trait SeleniumBasePlugin: Send {
    fn on_start(&mut self) {}
    fn before_command(&mut self, _name: &str, _target: &str, _value: &str) {}
    fn after_command(&mut self, _name: &str, _target: &str, _value: &str, _passed: bool) {}
    fn on_failure(&mut self, _command: &str, _target: &str, _value: &str, _error: &str) {}
    fn on_stop(&mut self) {}
}
