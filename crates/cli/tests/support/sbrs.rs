//! Runs the real `sbrs` binary in a folder of its own, with an `SB_*`
//! environment that holds only what a test sets.

// A test that includes this file uses only some of what it offers.
#![allow(dead_code)]

use std::path::Path;
use std::process::{Command, Output};

/// The binary under test.
pub const SBRS: &str = env!("CARGO_BIN_EXE_sbrs");

/// A command that runs `program` in `dir` with every `SB_*` variable of the
/// host removed, so the machine's own cannot leak into a test, then `env` set.
pub fn command_in(program: &str, dir: &Path, env: &[(&str, &str)]) -> Command {
    let mut command = Command::new(program);
    command
        .current_dir(dir)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error");
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("SB_") {
            command.env_remove(name);
        }
    }
    for (name, value) in env {
        command.env(name, value);
    }
    command
}

/// Runs `sbrs args...` in `dir` with `env` set and waits for it.
pub fn run(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    command_in(SBRS, dir, env)
        .args(args)
        .output()
        .expect("sbrs runs")
}

/// What the process wrote to standard output.
pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// What the process wrote to standard error.
pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The words of a line, for checks that do not depend on how wide a column is.
pub fn words(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

/// The line of a plugin table that starts with `name`, as its words.
pub fn row<'a>(table: &'a str, name: &str) -> Vec<&'a str> {
    table
        .lines()
        .map(words)
        .find(|words| words.first() == Some(&name))
        .unwrap_or_else(|| panic!("no row for {name} in:\n{table}"))
}
