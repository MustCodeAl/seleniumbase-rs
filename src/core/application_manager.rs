pub struct ApplicationManager {
    pub is_running: bool,
}
impl ApplicationManager {
    pub fn new() -> Self {
        Self { is_running: false }
    }
}
