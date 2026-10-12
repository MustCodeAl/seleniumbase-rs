//! A scratch crate that depends on this one by path, for tests that must build or run code the
//! program under test wrote: the Rust test an MCP server exports, a file `sbase mkfile` made.
//!
//! The crate shares this build's target directory and lockfile, with the features of the test
//! build, so Cargo reuses the dependencies and builds only the crate itself (and this one once,
//! because a path dependency is keyed by the place it was reached from). Cargo runs offline.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The target directory this test binary was built into.
pub fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            // A test binary sits in <target>/<profile>/deps.
            std::env::current_exe()
                .expect("the path of this test")
                .ancestors()
                .nth(3)
                .expect("the target directory")
                .to_path_buf()
        })
}

/// The features this test was built with, as the list a manifest takes.
fn features() -> String {
    let mut features = Vec::new();
    if cfg!(feature = "mcp-server") {
        features.push("\"mcp-server\"");
    }
    if cfg!(feature = "test-util") {
        features.push("\"test-util\"");
    }
    if cfg!(feature = "scrape") {
        features.push("\"scrape\"");
    }
    if cfg!(feature = "turso") {
        features.push("\"turso\"");
    }
    if cfg!(feature = "tui") {
        features.push("\"tui\"");
    }
    format!("[{}]", features.join(", "))
}

/// Writes the crate `name` into `dir`: a manifest that depends on this package by path, a copy
/// of this package's `Cargo.lock`, and an empty `tests` folder.
pub fn create(dir: &Path, name: &str) {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(dir.join("tests")).expect("create the scratch crate");
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\
             publish = false\n\n[workspace]\n\n[dependencies]\n\
             seleniumbase-rs = {{ path = {:?}, features = {} }}\n\
             tokio = {{ version = \"1\", features = [\"macros\", \"rt-multi-thread\", \"test-util\"] }}\n",
            package.display().to_string(),
            features()
        ),
    )
    .expect("write the manifest");
    // The same versions as this build, so the dependencies are not built again.
    std::fs::copy(package.join("Cargo.lock"), dir.join("Cargo.lock")).expect("copy the lockfile");
}

/// The variables that make a Cargo run share this build's target directory and stay offline.
pub fn apply_env(command: &mut Command) -> &mut Command {
    command
        .env("CARGO_TARGET_DIR", target_dir())
        .env("CARGO_NET_OFFLINE", "true")
}

/// A `cargo` command that runs in the scratch crate in `dir`.
pub fn cargo(dir: &Path) -> Command {
    let mut command = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command.current_dir(dir);
    apply_env(&mut command);
    command
}
