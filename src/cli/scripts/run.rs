//! Running a project's tests and examples through Cargo.
//!
//! Python SeleniumBase tests are started with `pytest` and its options.
//! A Rust test is an ordinary Cargo test, so this module builds the matching
//! `cargo` command: which test file or example, a name filter, how many tests
//! run at once, and the `SB_*` environment that carries `sbrs`'s own options
//! (such as `--headless`) into the tests. `sbrs test` is built on it, and so
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

use super::run_filter::NameFilter;

/// Which program runs the tests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Runner {
    /// `cargo test`, libtest: the default.
    #[default]
    Cargo,
    /// `cargo nextest`, which needs cargo-nextest installed.
    Nextest,
}

impl Runner {
    /// The word that names the runner on the command line, in `SB_TEST_RUNNER`
    /// and in the settings file.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Nextest => "nextest",
        }
    }
}

impl std::str::FromStr for Runner {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.trim().to_ascii_lowercase().as_str() {
            "cargo" => Ok(Self::Cargo),
            "nextest" => Ok(Self::Nextest),
            other => Err(format!(
                "invalid runner '{other}': choose from cargo, nextest"
            )),
        }
    }
}

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
    filters: Vec<String>,
    skips: Vec<String>,
    filterset: Option<String>,
    runner: Runner,
    list: bool,
    nocapture: bool,
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

    /// Runs only the tests whose names contain `text`, in addition to the
    /// filters already added (a test runs when its name contains any of them).
    /// Ignored for examples.
    #[must_use]
    pub fn filter(mut self, text: impl Into<String>) -> Self {
        self.filters.push(text.into());
        self
    }

    /// Which runner runs the tests: `cargo test` (the default) or
    /// `cargo nextest`. Ignored for examples.
    #[must_use]
    pub fn runner(mut self, runner: Runner) -> Self {
        self.runner = runner;
        self
    }

    /// Lists the tests that would run instead of running them (`--list` of
    /// libtest, `cargo nextest list`).
    #[must_use]
    pub fn list(mut self, list: bool) -> Self {
        self.list = list;
        self
    }

    /// Leaves out the tests whose names contain one of `names` (`--skip`).
    #[must_use]
    pub fn skips<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.skips = names.into_iter().map(Into::into).collect();
        self
    }

    /// Lets the tests print as they run (`--nocapture`, nextest's
    /// `--no-capture`).
    #[must_use]
    pub fn nocapture(mut self, nocapture: bool) -> Self {
        self.nocapture = nocapture;
        self
    }

    /// A nextest filterset (`-E`). Only the nextest runner reads it.
    #[must_use]
    pub fn filterset(mut self, filterset: impl Into<String>) -> Self {
        self.filterset = Some(filterset.into());
        self
    }

    /// Takes the filters, skips and filterset of a [`NameFilter`].
    #[must_use]
    pub fn name_filter(mut self, filter: NameFilter) -> Self {
        match filter {
            NameFilter::Libtest { filters, skips } => {
                self.filters.extend(filters);
                self.skips.extend(skips);
            }
            NameFilter::Nextest { filterset } => self.filterset = Some(filterset),
            NameFilter::All => {}
        }
        self
    }

    /// The command to run, not yet started.
    #[must_use]
    pub fn command(&self) -> Command {
        let mut command = Command::new("cargo");
        match (&self.target, self.runner) {
            (TestTarget::Example(name), _) => {
                command.arg("run");
                self.add_build_flags(&mut command);
                command.args(["--example", name]);
            }
            (target, Runner::Cargo) => {
                command.arg("test");
                self.add_build_flags(&mut command);
                if let TestTarget::Test(name) = target {
                    command.args(["--test", name]);
                }
                command.args(&self.filters);
                let mut harness: Vec<OsString> = Vec::new();
                if let Some(threads) = self.threads {
                    harness.push("--test-threads".into());
                    harness.push(threads.to_string().into());
                }
                if self.nocapture {
                    harness.push("--nocapture".into());
                }
                if self.list {
                    harness.push("--list".into());
                }
                self.add_skips(&mut harness);
                harness.extend(self.harness_args.iter().cloned());
                if !harness.is_empty() {
                    command.arg("--");
                    command.args(harness);
                }
            }
            (target, Runner::Nextest) => {
                command.args(["nextest", if self.list { "list" } else { "run" }]);
                self.add_build_flags(&mut command);
                if let TestTarget::Test(name) = target {
                    command.args(["--test", name]);
                }
                if let Some(filterset) = &self.filterset {
                    command.args(["-E", filterset]);
                }
                if self.list {
                    command.args(["--message-format", "oneline"]);
                } else {
                    if let Some(threads) = self.threads {
                        command.args(["--test-threads", &threads.to_string()]);
                    }
                    if self.nocapture {
                        command.arg("--no-capture");
                    }
                }
                command.args(&self.filters);
                let mut harness: Vec<OsString> = Vec::new();
                self.add_skips(&mut harness);
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

    fn add_skips(&self, harness: &mut Vec<OsString>) {
        for skip in &self.skips {
            harness.push("--skip".into());
            harness.push(skip.into());
        }
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
    fn several_filters_are_positional_and_skips_follow_the_double_dash() {
        let run = TestRun::new()
            .filter("login")
            .filter("signup")
            .skips(["slow", "flaky"]);
        assert_eq!(
            args(&run),
            ["test", "login", "signup", "--", "--skip", "slow", "--skip", "flaky"]
        );
    }

    #[test]
    fn the_cargo_harness_arguments_come_in_a_fixed_order() {
        let run = TestRun::new()
            .threads(1)
            .nocapture(true)
            .list(true)
            .skips(["slow"])
            .harness_args(["--exact"]);
        assert_eq!(
            args(&run),
            [
                "test",
                "--",
                "--test-threads",
                "1",
                "--nocapture",
                "--list",
                "--skip",
                "slow",
                "--exact"
            ]
        );
    }

    #[test]
    fn nextest_takes_the_filterset_threads_and_capture_as_options() {
        let run = TestRun::new()
            .runner(Runner::Nextest)
            .target(TestTarget::Test("login".into()))
            .release(true)
            .filterset("test(~a) and test(~b)")
            .threads(2)
            .nocapture(true)
            .filter("valid")
            .skips(["slow"])
            .harness_args(["--exact"]);
        assert_eq!(
            args(&run),
            [
                "nextest",
                "run",
                "--release",
                "--test",
                "login",
                "-E",
                "test(~a) and test(~b)",
                "--test-threads",
                "2",
                "--no-capture",
                "valid",
                "--",
                "--skip",
                "slow",
                "--exact"
            ]
        );
    }

    #[test]
    fn listing_has_a_form_for_each_runner() {
        let cargo = TestRun::new().list(true).filter("login");
        assert_eq!(args(&cargo), ["test", "login", "--", "--list"]);
        let nextest = TestRun::new()
            .runner(Runner::Nextest)
            .list(true)
            .filter("login")
            .threads(4);
        assert_eq!(
            args(&nextest),
            ["nextest", "list", "--message-format", "oneline", "login"]
        );
    }

    #[test]
    fn an_example_ignores_the_runner() {
        let run = TestRun::new()
            .runner(Runner::Nextest)
            .target(TestTarget::Example("basic_test".into()));
        assert_eq!(args(&run), ["run", "--example", "basic_test"]);
    }

    #[test]
    fn a_name_filter_fills_the_filters_skips_and_filterset() {
        let libtest = TestRun::new().name_filter(NameFilter::Libtest {
            filters: vec!["a".into(), "b".into()],
            skips: vec!["c".into()],
        });
        assert_eq!(args(&libtest), ["test", "a", "b", "--", "--skip", "c"]);
        let nextest = TestRun::new()
            .runner(Runner::Nextest)
            .name_filter(NameFilter::Nextest {
                filterset: "test(~a)".into(),
            });
        assert_eq!(args(&nextest), ["nextest", "run", "-E", "test(~a)"]);
        assert_eq!(args(&TestRun::new().name_filter(NameFilter::All)), ["test"]);
    }

    #[test]
    fn a_runner_is_read_by_its_name() {
        assert_eq!("cargo".parse::<Runner>(), Ok(Runner::Cargo));
        assert_eq!(" NextEst ".parse::<Runner>(), Ok(Runner::Nextest));
        assert_eq!(
            "make".parse::<Runner>(),
            Err("invalid runner 'make': choose from cargo, nextest".to_owned())
        );
        assert_eq!(Runner::Nextest.name(), "nextest");
        assert_eq!(Runner::default(), Runner::Cargo);
    }

    #[test]

    fn a_later_value_for_the_same_variable_wins() {
        let run = TestRun::new().env("SB_MODE", "cdp").env("SB_MODE", "uc");
        assert_eq!(envs(&run)["SB_MODE"].as_deref(), Some("uc"));
    }
}
