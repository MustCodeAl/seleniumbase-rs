//! A password variable for the tests of the imap plugin (feature `plugin-imap`).
//!
//! The plugin accepts a `password_env` only if its name starts with `IMAP_`, `MAIL_` or `SB_`,
//! and it needs the variable to be set when it is built. No process that runs the tests has such
//! a variable, and a test cannot set one itself: the tests of a binary share one process, and
//! `std::env::set_var` is `unsafe` in edition 2024 (and races with the tests that read the
//! environment in any edition). So a test that needs the variable runs again in a child process
//! of the same test binary that has it, and the parent asserts that the child passed.
//!
//! The value is never used: nothing here connects to a mail server.

use std::process::Command;

/// The variable the tests name as `password_env`.
pub(crate) const NAME: &str = "IMAP_SBRS_CORE_TEST_PASSWORD";

/// Runs `body` with [`NAME`] set.
///
/// `module` is `module_path!()` of the calling test module and `test` the name of the test
/// function. When the variable is not set, that one test is run again in a child process that has
/// it, and the child's success is asserted.
///
/// # Panics
///
/// When the child process cannot be started or its test fails.
pub(crate) fn with_password_set(module: &str, test: &str, body: impl FnOnce()) {
    if std::env::var_os(NAME).is_some() {
        body();
        return;
    }
    // libtest names a test without the name of its crate.
    let path = module.split_once("::").map_or("", |(_, path)| path);
    let name = format!("{path}::{test}");
    let exe = std::env::current_exe().expect("the path of this test binary");
    let output = Command::new(exe)
        .args([name.as_str(), "--exact", "--nocapture", "--test-threads=1"])
        .env(NAME, "not-a-real-password")
        .output()
        .expect("the test binary starts again");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // `--exact` with a name that matches nothing also exits 0, so the test that ran is counted.
    assert!(
        output.status.success() && stdout.contains("test result: ok. 1 passed;"),
        "{name} failed in the child process ({}):\n{stdout}\n{stderr}",
        output.status
    );
}
