//! `sbrs-mcp --version` prints the name the binary printed before the command
//! line moved to its own package. No browser is started.
//! Run with `cargo test --features mcp-server`.

#![cfg(feature = "mcp-server")]

use std::process::Command;

#[test]
fn version_names_the_library_package_as_it_did_before_the_split() {
    let output = Command::new(env!("CARGO_BIN_EXE_sbrs-mcp"))
        .arg("--version")
        .output()
        .expect("run sbrs-mcp --version");
    assert!(output.status.success(), "exit status {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.trim(),
        format!("seleniumbase-rs {}", env!("CARGO_PKG_VERSION"))
    );
}
