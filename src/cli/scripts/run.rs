use std::process::{Command, Output};

pub fn run_tests() -> std::io::Result<Output> {
    Command::new("cargo").arg("test").output()
}
