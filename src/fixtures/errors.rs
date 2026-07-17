// Wraps common errors for fixtures
pub fn raise_timeout_error(msg: &str) {
    eprintln!("Timeout Error: {}", msg);
}
