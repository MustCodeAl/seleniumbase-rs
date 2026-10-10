//! Running a project's tests and examples through Cargo.
//!
//! Python SeleniumBase tests are started with `pytest` and its options.
//! A Rust test is an ordinary Cargo test, so this module builds the matching
//! `cargo` command: which test file or example, a name filter, how many tests
//! run at once, and the `SB_*` environment that carries `sbase`'s own options
//! (such as `--headless`) into the tests. `sbase test` is built on it, and so
//! can any other launcher, such as the commander TUI.
//!
//! [`TestRun::command`] only builds the command, so its arguments and
//! environment can be checked without running anything.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::scripts::run::{TestRun, TestTarget};
//!
//! let command = TestRun::new()
//!     .target(TestTarget::Test("login".into()))
//!     .filter("valid_password")
//!     .threads(2)
//!     .command();
//!
//! let args: Vec<_> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
//! assert_eq!(
//!     args,
//!     ["test", "--test", "login", "valid_password", "--", "--test-threads", "2"]
//! );
//! ```

use std::ffi::OsString;
use std::io;
use std::process::{Command, ExitStatus, Output};

/// What to run.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum TestTarget {
    /// Every test of the package (`cargo test`).
    #[default]
    All,
    /// One integration test file under `tests/` (`cargo test --test NAME`).
    Test(String),
    /// One example under `examples/` (`cargo run --example NAME`).
    Example(String),
}

/// A `cargo test` or `cargo run --example` invocation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TestRun {
    target: TestTarget,
    filter: Option<String>,
    threads: Option<usize>,
    release: bool,
    features: Vec<String>,
    env: Vec<(String, String)>,
    harness_args: Vec<OsString>,
}

impl TestRun {
    /// A run of every test with Cargo's defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Selects a test file, an example, or everything.
    #[must_use]
    pub fn target(mut self, target: TestTarget) -> Self {
        self.target = target;
        self
    }

    /// Runs only the tests whose name contains `filter`. Ignored for examples.
    #[must_use]
    pub fn filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// How many tests run at the same time (`--test-threads`). Ignored for
    /// examples. Each browser test starts its own browser, so this is also the
    /// number of browsers open at once.
    #[must_use]
    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = Some(threads);
        self
    }

    /// Builds with the release profile.
    #[must_use]
    pub fn release(mut self, release: bool) -> Self {
        self.release = release;
        self
    }

    /// Enables Cargo features, for example `["mcp-server"]`.
    #[must_use]
    pub fn features<I, S>(mut self, features: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.features = features.into_iter().map(Into::into).collect();
        self
    }

    /// Sets an environment variable for the tests. A later call with the same
    /// name wins.
    #[must_use]
    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((name.into(), value.into()));
        self
    }

    /// Arguments for the test harness, passed after `--` (for example
    /// `--nocapture`). Ignored for examples.
    #[must_use]
    pub fn harness_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.harness_args = args.into_iter().map(Into::into).collect();
        self
    }

    /// The command to run, not yet started.
    #[must_use]
    pub fn command(&self) -> Command {
        let mut command = Command::new("cargo");
        match &self.target {
            TestTarget::Example(name) => {
                command.arg("run");
                self.add_build_flags(&mut command);
                command.args(["--example", name]);
            }
            target => {
                command.arg("test");
                self.add_build_flags(&mut command);
                if let TestTarget::Test(name) = target {
                    command.args(["--test", name]);
                }
                if let Some(filter) = &self.filter {
                    command.arg(filter);
                }
                let mut harness: Vec<OsString> = Vec::new();
                if let Some(threads) = self.threads {
                    harness.push("--test-threads".into());
                    harness.push(threads.to_string().into());
                }
                harness.extend(self.harness_args.iter().cloned());
                if !harness.is_empty() {
                    command.arg("--");
                    command.args(harness);
                }
            }
        }
        for (name, value) in &self.env {
            command.env(name, value);
        }
        command
    }

    fn add_build_flags(&self, command: &mut Command) {
        if self.release {
            command.arg("--release");
        }
        if !self.features.is_empty() {
            command.args(["--features", &self.features.join(",")]);
        }
    }

    /// Runs the command with this process's input and output and returns how
    /// it ended.
    ///
    /// # Errors
    ///
    /// Returns an error if `cargo` cannot be started.
    pub fn status(&self) -> io::Result<ExitStatus> {
        self.command().status()
    }

    /// Runs the command and captures what it prints.
    ///
    /// # Errors
    ///
    /// Returns an error if `cargo` cannot be started.
    pub fn output(&self) -> io::Result<Output> {
        self.command().output()
    }
}

/// Runs `cargo test` in the current directory and captures the output.
///
/// # Errors
///
/// Returns an error if `cargo` cannot be started.
pub fn run_tests() -> io::Result<Output> {
    TestRun::new().output()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(run: &TestRun) -> Vec<String> {
        run.command()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn envs(run: &TestRun) -> std::collections::BTreeMap<String, Option<String>> {
        run.command()
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect()
    }

    #[test]
    fn the_default_run_is_plain_cargo_test() {
        let command = TestRun::new().command();
        assert_eq!(command.get_program(), "cargo");
        assert_eq!(args(&TestRun::new()), ["test"]);
    }

    #[test]
    fn a_test_file_and_a_filter_come_before_the_harness_arguments() {
        let run = TestRun::new()
            .target(TestTarget::Test("login".into()))
            .filter("bad_password")
            .threads(3)
            .harness_args(["--nocapture"]);
        assert_eq!(
            args(&run),
            [
                "test",
                "--test",
                "login",
                "bad_password",
                "--",
                "--test-threads",
                "3",
                "--nocapture"
            ]
        );
    }

    #[test]
    fn an_example_is_run_not_tested_and_takes_no_harness_arguments() {
        let run = TestRun::new()
            .target(TestTarget::Example("basic_test".into()))
            .filter("ignored")
            .threads(4)
            .release(true)
            .features(["gui", "mcp-server"]);
        assert_eq!(
            args(&run),
            [
                "run",
                "--release",
                "--features",
                "gui,mcp-server",
                "--example",
                "basic_test"
            ]
        );
    }

    #[test]
    fn build_flags_apply_to_tests_too() {
        let run = TestRun::new().release(true).features(["turso"]);
        assert_eq!(args(&run), ["test", "--release", "--features", "turso"]);
    }

    #[test]
    fn the_environment_reaches_the_command() {
        let run = TestRun::new()
            .env("SB_HEADLESS", "true")
            .env("SB_BROWSER", "firefox");
        let envs = envs(&run);
        assert_eq!(envs["SB_HEADLESS"].as_deref(), Some("true"));
        assert_eq!(envs["SB_BROWSER"].as_deref(), Some("firefox"));
        assert_eq!(envs.len(), 2);
    }

    #[test]
    fn a_later_value_for_the_same_variable_wins() {
        let run = TestRun::new().env("SB_MODE", "cdp").env("SB_MODE", "uc");
        assert_eq!(envs(&run)["SB_MODE"].as_deref(), Some("uc"));
    }
}
