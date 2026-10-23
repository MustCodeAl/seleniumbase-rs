//! `sbrs test` with the options of Python SeleniumBase's pytest plugin, run as the real binary.
//!
//! `sbrs test` starts Cargo, so these tests put a stand-in `cargo` first on `PATH` that reports
//! how it was called (its arguments and the `SB_*` variables it was given). The stand-in is
//! run once when it is written, because macOS delays the first run of a new script. The
//! command runs in a folder of its own, so the logs folders it resets are the test's.
//! No browser, network or real Cargo run is involved.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// A `cargo` that prints `args=` and its `SB_*` variables. `cargo metadata` answers with one
/// package that has two examples; `cargo run --example two` fails with status 3; `--list`
/// prints two tests. `FAKE_CARGO_EXIT` sets the status of the rest.
const FAKE_CARGO: &str = r#"#!/bin/sh
if [ "$1" = "--warm" ]; then exit 0; fi
if [ "$1" = "metadata" ]; then
  p=$(pwd -P)
  printf '{"packages":[{"id":"p 0.1.0","manifest_path":"%s/Cargo.toml","features":{},"targets":[{"name":"p","kind":["lib"]},{"name":"one","kind":["example"]},{"name":"two","kind":["example"]}]}],"workspace_members":["p 0.1.0"],"workspace_default_members":["p 0.1.0"]}\n' "$p"
  exit 0
fi
echo "args=$*"
env | while read -r line; do
  case "$line" in SB_*) echo "$line" ;; esac
done
case " $* " in
  *" --list "*) printf 'login::valid: test\nlogin::bad: test\n\n2 tests, 0 benchmarks\n' ;;
esac
case "$*" in "run --example two"*) exit 3 ;; esac
exit ${FAKE_CARGO_EXIT:-0}
"#;

struct Fixture {
    bin: tempfile::TempDir,
    work: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let bin = tempfile::tempdir().expect("temp dir");
        let script = bin.path().join("cargo");
        std::fs::write(&script, FAKE_CARGO).expect("write the stand-in");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");
        // The first run of a new script can be slow on macOS: take it now.
        let warmed = Command::new(&script)
            .arg("--warm")
            .status()
            .expect("warm it");
        assert!(warmed.success());
        Self {
            bin,
            work: tempfile::tempdir().expect("temp dir"),
        }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.work.path().join(relative)
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(SBASE);
        command
            .args(args)
            .current_dir(self.work.path())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.bin.path().display()),
            )
            .env_remove("NO_COLOR")
            .env_remove("RUST_LOG")
            .env("SB_LOG_LEVEL", "error");
        for (name, _) in std::env::vars() {
            if name.starts_with("SB_") && name != "SB_LOG_LEVEL" {
                command.env_remove(name);
            }
        }
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("sbrs runs")
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The `args=` line the stand-in printed, without the prefix.
fn cargo_args(output: &Output) -> String {
    stdout(output)
        .lines()
        .find_map(|line| line.strip_prefix("args="))
        .unwrap_or_else(|| panic!("cargo did not run: {}{}", stdout(output), stderr(output)))
        .to_owned()
}

fn has_var(output: &Output, line: &str) -> bool {
    stdout(output).lines().any(|found| found == line)
}

fn refused(output: &Output) -> String {
    assert_eq!(output.status.code(), Some(2), "{}", stderr(output));
    assert!(
        !stdout(output).contains("args="),
        "cargo must not have run: {}",
        stdout(output)
    );
    stderr(output)
}

#[test]
fn the_test_data_options_reach_the_tests() {
    let fixture = Fixture::new();
    let output = fixture.run(&[
        "test",
        "--data=abc",
        "--env=QA",
        "--var1=x",
        "--variables={'k': 1}",
        "--account",
        "acme",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    for line in [
        "SB_DATA=abc",
        "SB_ENV=qa",
        "SB_VAR1=x",
        "SB_VARIABLES={'k': 1}",
        "SB_ACCOUNT=acme",
    ] {
        assert!(has_var(&output, line), "{line} in {}", stdout(&output));
    }
}

#[test]
fn json_variables_are_accepted_too() {
    let fixture = Fixture::new();
    let output = fixture.run(&["test", r#"--variables={"k":1}"#]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(has_var(&output, r#"SB_VARIABLES={"k":1}"#));
}

#[test]
fn an_unknown_environment_exits_2_and_names_the_choices() {
    let fixture = Fixture::new();
    let message = refused(&fixture.run(&["test", "--env=nosuch"]));
    assert!(
        message.contains("invalid value 'nosuch' for --env (SB_ENV): choose from qa, rc, staging"),
        "{message}"
    );
    assert!(message.contains("uat"), "{message}");
    assert!(
        message.contains("Usage:"),
        "the page of the command comes first: {message}"
    );
}

#[test]
fn a_bad_dictionary_exits_2() {
    let fixture = Fixture::new();
    let message = refused(&fixture.run(&["test", "--variables=[1, 2]"]));
    assert!(message.contains("Expecting a dictionary"), "{message}");
}

#[test]
fn k_selects_by_name_the_way_libtest_can() {
    let fixture = Fixture::new();
    for (expression, expected) in [
        ("login or signup", "test login signup"),
        ("not admin", "test -- --skip admin"),
        ("login and not slow", "test login -- --skip slow"),
    ] {
        let output = fixture.run(&["test", "-k", expression]);
        assert!(output.status.success(), "{expression}: {}", stderr(&output));
        assert_eq!(cargo_args(&output), expected, "{expression}");
    }
}

#[test]
fn an_expression_libtest_cannot_run_asks_for_nextest() {
    let fixture = Fixture::new();
    let message = refused(&fixture.run(&["test", "-k", "a and b"]));
    assert!(
        message.contains(
            "-k \"a and b\" needs the nextest runner (sbrs test --runner nextest): \
             libtest can only OR names and skip names"
        ),
        "{message}"
    );
    let output = fixture.run(&["test", "-k", "a and b", "--runner", "nextest"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(cargo_args(&output), "nextest run -E test(~a) and test(~b)");
}

#[test]
fn a_marker_is_a_module_name() {
    let fixture = Fixture::new();
    let output = fixture.run(&["test", "-m", "smoke"]);
    assert_eq!(cargo_args(&output), "test smoke::");
    let output = fixture.run(&["test", "-m", "smoke", "--runner=nextest"]);
    assert_eq!(
        cargo_args(&output),
        "nextest run -E test(/(^|::)smoke(::|$)/)"
    );
}

#[test]
fn n_and_s_set_the_threads_and_the_capture() {
    let fixture = Fixture::new();
    let output = fixture.run(&["test", "-n", "2", "-s"]);
    assert_eq!(cargo_args(&output), "test -- --test-threads 2 --nocapture");
    assert!(has_var(&output, "SB_THREADS=2"));
    let output = fixture.run(&["test", "-n", "auto"]);
    assert_eq!(cargo_args(&output), "test");
    let message = refused(&fixture.run(&["test", "-n", "lots"]));
    assert!(
        message.contains("-n takes a number of threads or `auto`"),
        "{message}"
    );
}

#[test]
fn the_debug_modes_run_one_test_at_a_time_with_its_output() {
    let fixture = Fixture::new();
    for flag in ["--pdb", "--trace", "--final-debug", "--ftrace", "--fdebug"] {
        let output = fixture.run(&["test", flag, "-n", "4"]);
        assert_eq!(
            cargo_args(&output),
            "test -- --test-threads 1 --nocapture",
            "{flag}"
        );
    }
    let output = fixture.run(&["test", "--pdb"]);
    assert!(has_var(&output, "SB_PDB=true"));
}

#[test]
fn a_debug_mode_refuses_nextest() {
    let fixture = Fixture::new();
    let message = refused(&fixture.run(&["test", "--pdb", "--runner", "nextest"]));
    assert!(
        message.contains("--pdb needs the cargo runner: nextest gives tests no standard input"),
        "{message}"
    );
}

#[test]
fn the_runner_comes_from_the_flag_or_the_variable() {
    let fixture = Fixture::new();
    let output = fixture
        .command(&["test", "login"])
        .env("SB_TEST_RUNNER", "nextest")
        .output()
        .expect("sbrs runs");
    assert_eq!(cargo_args(&output), "nextest run login");
    let output = fixture
        .command(&["test", "login", "--runner", "cargo"])
        .env("SB_TEST_RUNNER", "nextest")
        .output()
        .expect("sbrs runs");
    assert_eq!(cargo_args(&output), "test login");
    let output = fixture.run(&["test", "--runner", "make"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
}

#[test]
fn co_lists_the_tests_and_counts_them() {
    let fixture = Fixture::new();
    for flag in ["--co", "--collect-only"] {
        let output = fixture.run(&["test", flag]);
        assert!(output.status.success(), "{flag}: {}", stderr(&output));
        let out = stdout(&output);
        assert!(
            out.contains("login::valid\nlogin::bad\n\ncollected 2 items\n"),
            "{flag}: {out}"
        );
        assert!(
            !out.contains("args="),
            "the stand-in's own output stays out: {out}"
        );
    }
}

#[test]
fn co_touches_no_folder() {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.path("latest_logs")).unwrap();
    std::fs::write(fixture.path("latest_logs/old.txt"), "old").unwrap();
    let output = fixture.run(&["test", "--co", "--archive-logs"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(fixture.path("latest_logs/old.txt").is_file());
    assert!(!fixture.path("archived_logs").exists());
}

#[test]
fn a_run_empties_the_latest_logs_unless_they_are_archived() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.path("latest_logs/test_one")).unwrap();
    std::fs::write(fixture.path("latest_logs/test_one/screenshot.png"), "png").unwrap();
    let output = fixture.run(&["test"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(fixture.path("latest_logs").is_dir());
    assert!(std::fs::read_dir(fixture.path("latest_logs"))
        .unwrap()
        .next()
        .is_none());
    assert!(!fixture.path("archived_logs").exists());
}

#[test]
fn archive_logs_moves_them_to_a_folder_named_by_the_time() {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.path("latest_logs")).unwrap();
    std::fs::write(fixture.path("latest_logs/old.txt"), "old").unwrap();
    let output = fixture.run(&["test", "--archive_logs"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let archives: Vec<_> = std::fs::read_dir(fixture.path("archived_logs"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(archives.len(), 1, "{archives:?}");
    let name = archives[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(
        name.strip_prefix("logs_")
            .is_some_and(|secs| secs.parse::<u64>().is_ok()),
        "{name}"
    );
    assert!(archives[0].join("old.txt").is_file());
    assert!(std::fs::read_dir(fixture.path("latest_logs"))
        .unwrap()
        .next()
        .is_none());
}

#[test]
fn archive_downloads_does_the_same_for_the_downloads_folder() {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.path("downloaded_files")).unwrap();
    std::fs::write(fixture.path("downloaded_files/report.pdf"), "pdf").unwrap();
    let output = fixture.run(&["test", "--archive-downloads"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let archives: Vec<_> = std::fs::read_dir(fixture.path("archived_files"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(archives.len(), 1);
    assert!(archives[0].join("report.pdf").is_file());
    assert!(!fixture.path("archived_logs").exists());
}

#[test]
fn the_settings_file_reaches_the_tests_as_an_absolute_path() {
    let fixture = Fixture::new();
    std::fs::write(fixture.path("conf.toml"), "").unwrap();
    for args in [
        &["-c", "conf.toml", "test"][..],
        &["--config", "conf.toml", "test"],
        &["test", "--settings-file", "conf.toml"],
        &["test", "--settings=conf.toml"],
        &["test", "--settings_file", "conf.toml"],
    ] {
        let output = fixture.run(args);
        assert!(output.status.success(), "{args:?}: {}", stderr(&output));
        let line = stdout(&output)
            .lines()
            .find_map(|line| line.strip_prefix("SB_SETTINGS_FILE=").map(str::to_owned))
            .unwrap_or_else(|| panic!("{args:?}: {}", stdout(&output)));
        assert!(
            Path::new(&line).is_absolute() && line.ends_with("/conf.toml"),
            "{args:?}: {line}"
        );
    }
}

#[test]
fn crumbs_and_the_python_spellings_reach_the_tests() {
    let fixture = Fixture::new();
    let output = fixture.run(&[
        "test",
        "--crumbs",
        "--no_sandbox",
        "--demo_sleep=2",
        "--dark",
        "--firefox",
        "--headed",
        "--ns",
        "--time-limit",
        "5",
        "--reruns",
        "2",
        "--reruns-delay",
        "0.5",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    for line in [
        "SB_CRUMBS=true",
        "SB_NO_SANDBOX=true",
        "SB_DEMO_SLEEP=2",
        "SB_DARK_MODE=true",
        "SB_BROWSER=firefox",
        "SB_HEADLESS=false",
        "SB_NO_SCREENSHOT=true",
        "SB_TIME_LIMIT=5",
        "SB_RERUNS=2",
        "SB_RERUNS_DELAY=0.5",
    ] {
        assert!(has_var(&output, line), "{line} in {}", stdout(&output));
    }
}

#[test]
fn a_flag_after_the_command_wins_over_one_before_it() {
    let fixture = Fixture::new();
    let output = fixture.run(&["--headless", "test", "--headed"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(has_var(&output, "SB_HEADLESS=false"), "{}", stdout(&output));
    let output = fixture.run(&["--browser", "firefox", "test"]);
    assert!(has_var(&output, "SB_BROWSER=firefox"));
}

#[test]
fn two_browsers_are_refused() {
    let fixture = Fixture::new();
    let message = refused(&fixture.run(&["test", "--firefox", "--chrome"]));
    assert!(message.contains("both choose SB_BROWSER"), "{message}");
}

#[test]
fn options_that_do_nothing_are_accepted() {
    let fixture = Fixture::new();
    let output = fixture.run(&[
        "test",
        "--fast",
        "--with-selenium",
        "--is_pytest",
        "--uc-sub",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(cargo_args(&output), "test");
}

#[test]
fn all_scripts_runs_every_example_after_the_tests() {
    let fixture = Fixture::new();
    let output = fixture.run(&["test", "--all-scripts"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("example one ... ok"), "{out}");
    assert!(out.contains("example two ... FAILED (status 3)"), "{out}");
    assert!(out.contains("args=test\n"), "the tests ran first: {out}");
    assert!(out.contains("args=run --example one"), "{out}");
    let message = refused(&fixture.run(&["test", "--all-scripts", "--example", "x"]));
    assert!(message.contains("--all-scripts"), "{message}");
}

#[test]
fn the_exit_status_of_cargo_is_the_one_of_sbrs() {
    let fixture = Fixture::new();
    let output = fixture
        .command(&["test"])
        .env("FAKE_CARGO_EXIT", "7")
        .output()
        .expect("sbrs runs");
    assert_eq!(output.status.code(), Some(7));
}

#[test]
fn behave_options_prints_the_table() {
    let fixture = Fixture::new();
    let output = fixture.run(&["behave-options"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("-D NAME[=VALUE]"), "{out}");
    assert!(out.contains("sbrs test --co"), "{out}");
    assert!(out.contains("sbrs test --runner nextest"), "{out}");
}

#[test]
fn the_help_lists_the_options_in_groups() {
    let fixture = Fixture::new();
    let output = fixture.run(&["test", "--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let out = stdout(&output);
    for text in [
        "Browser and page options:",
        "Test data:",
        "Test run, debugging and reports:",
        "--pdb",
        "--reruns",
        "--runner",
        "--demo-sleep",
        "-k",
        "--co",
        "Examples:",
    ] {
        assert!(out.contains(text), "{text} in:\n{out}");
    }
}
