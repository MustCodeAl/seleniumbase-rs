//! `sbrs-mcp` names itself: its `--version` line, the usage line of an error and
//! its help all say `sbrs-mcp`, the name of the program, not the library package.
//! No browser is started.
//! Run with `cargo test --features mcp-server`.

#![cfg(feature = "mcp-server")]

use std::process::{Command, Output};

fn sbrs_mcp(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sbrs-mcp"))
        .args(args)
        .output()
        .expect("run sbrs-mcp")
}

#[test]
fn version_names_the_program() {
    let output = sbrs_mcp(&["--version"]);
    assert!(output.status.success(), "exit status {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        format!("sbrs-mcp {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_usage_line_of_an_error_names_the_program() {
    let output = sbrs_mcp(&["--bogus"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "exit status {:?}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Usage: sbrs-mcp "), "{stderr}");
    assert!(!stderr.contains("seleniumbase-rs"), "{stderr}");
}

#[test]
fn help_names_the_program() {
    let output = sbrs_mcp(&["--help"]);
    assert!(output.status.success(), "exit status {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("sbrs-mcp "), "{stdout}");
    assert!(stdout.contains("Usage: sbrs-mcp "), "{stdout}");
}
