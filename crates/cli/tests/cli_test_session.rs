//! The session of `sbrs test`: the run it opens before Cargo and ends after it, and the reports
//! the run writes.
//!
//! `sbrs test` starts Cargo, so these tests put a stand-in `cargo` first on `PATH`. The
//! stand-in plays the part of the test processes: it finds the run through `SB_REPORT_ROOT`
//! (as the `sb` fixture does) and appends the events of two tests to it, one passed and one
//! failed, with the logs of the failed one. The stand-in is run once when it is written,
//! because macOS delays the first run of a new script. No browser, network or real Cargo run
//! is involved.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// A `cargo` that reports what it was given (`args=` and the `SB_*` variables) and writes two
/// tests into the run. `cargo metadata` fails, so `sbrs` falls back to the working directory.
/// `FAKE_CARGO_EXIT` sets its status (1 unless given), and `FAKE_CARGO_EVENTS=0` leaves the
/// run alone.
const FAKE_CARGO: &str = r#"#!/bin/sh
if [ "$1" = "--warm" ]; then exit 0; fi
if [ "$1" = "metadata" ]; then exit 1; fi
echo "args=$*"
env | while read -r line; do
  case "$line" in SB_*) echo "$line" ;; esac
done
if [ "${FAKE_CARGO_EVENTS:-1}" = "1" ] && [ -n "$SB_REPORT_ROOT" ]; then
  mkdir -p "$SB_REPORT_ROOT/latest_logs/login.bad"
  echo "boom" > "$SB_REPORT_ROOT/latest_logs/login.bad/basic_test_info.txt"
  cat >> "$SB_REPORT_ROOT/.sbrs_run/results.jsonl" <<'EVENTS'
{"v":1,"type":"start","id":"login.good","display":"login::good","suite":"login","pid":4242,"at":"2026-10-23T10:00:00+00:00"}
{"v":1,"type":"end","id":"login.good","result":{"name":"login::good","status":"passed","duration_secs":0.25,"message":"ok"},"at":"2026-10-23T10:00:01+00:00"}
{"v":1,"type":"start","id":"login.bad","display":"login::bad","suite":"login","pid":4243,"at":"2026-10-23T10:00:02+00:00"}
{"v":1,"type":"end","id":"login.bad","result":{"name":"login::bad","status":"failed","duration_secs":0.5,"message":"boom","log_dir":"latest_logs/login.bad"},"at":"2026-10-23T10:00:03+00:00"}
EVENTS
fi
exit ${FAKE_CARGO_EXIT:-1}
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

    /// The working directory as `sbrs` sees it (a temporary folder can be behind a link).
    fn root(&self) -> PathBuf {
        std::fs::canonicalize(self.work.path()).expect("the folder exists")
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
            .env_remove("COLUMNS")
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

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn has_var(output: &Output, line: &str) -> bool {
    stdout(output).lines().any(|found| found == line)
}

fn var_value(output: &Output, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    stdout(output)
        .lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
}

#[test]
fn a_failing_run_prints_the_dashboard_first_and_writes_the_three_files() {
    let fixture = Fixture::new();

    let output = fixture.run(&[
        "test",
        "--dashboard",
        "--html",
        "r.html",
        "--junit-xml",
        "r.xml",
    ]);

    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let text = stdout(&output);
    let mut lines = text.lines();
    let first = lines.next().expect("a first line");
    assert!(
        first.starts_with("Dashboard: file://") && first.ends_with("/dashboard.html"),
        "{first}"
    );
    let stars = lines.next().expect("a second line");
    assert!(
        !stars.is_empty() && stars.chars().all(|ch| ch == '*') && stars.len() <= first.len(),
        "{stars}"
    );
    assert!(text.contains("args=test"), "{text}");
    for line in [
        "----- Dashboard = file://",
        "----- Generated html report: file://",
        "----- generated xml file: ",
        "----- Latest Logs dir: ",
    ] {
        assert!(text.contains(line), "{line} in {text}");
    }

    let dashboard = read(&fixture.path("dashboard.html"));
    assert!(dashboard.contains("login::bad"), "{dashboard}");
    assert!(
        !dashboard.contains("http-equiv=\"refresh\""),
        "the ended run is a page that stays: {dashboard}"
    );
    let html = read(&fixture.path("r.html"));
    assert!(html.contains("login::good") && html.contains("login::bad"));
    let xml = read(&fixture.path("r.xml"));
    assert_eq!(xml.matches("<testcase").count(), 2, "{xml}");
    assert!(xml.contains("<failure"), "{xml}");
    assert!(
        fixture
            .path("latest_logs/login.bad/basic_test_info.txt")
            .is_file(),
        "the logs of the failed test stay"
    );
}

#[test]
fn the_tests_are_given_the_run_to_join() {
    let fixture = Fixture::new();

    let output = fixture.run(&["test", "--dashboard", "--dash-title", "My_Title"]);

    let run_id = var_value(&output, "SB_RUN_ID").expect("SB_RUN_ID reaches the tests");
    assert!(run_id.starts_with("sbrs-"), "{run_id}");
    assert_eq!(
        var_value(&output, "SB_REPORT_ROOT").map(PathBuf::from),
        Some(fixture.root())
    );
    assert!(has_var(&output, "SB_DASHBOARD=true"));
    assert!(has_var(&output, "SB_DASH_TITLE=My_Title"));
    assert!(
        read(&fixture.path("dashboard.html")).contains("My Title"),
        "an underscore in the title is a space"
    );
    let state = read(&fixture.path(".sbrs_run/run.json"));
    assert!(state.contains(&run_id), "{state}");
}

#[test]
fn a_report_path_reaches_the_tests_as_an_absolute_one() {
    let fixture = Fixture::new();

    let output = fixture.run(&[
        "test",
        "--html=out/r.html",
        "--alluredir",
        "allure",
        "--junit_xml",
        "x.xml",
    ]);

    let root = fixture.root();
    assert_eq!(
        var_value(&output, "SB_HTML").map(PathBuf::from),
        Some(root.join("out/r.html"))
    );
    assert_eq!(
        var_value(&output, "SB_ALLURE_DIR").map(PathBuf::from),
        Some(root.join("allure"))
    );
    assert_eq!(
        var_value(&output, "SB_JUNIT_XML").map(PathBuf::from),
        Some(root.join("x.xml"))
    );
    assert!(
        fixture.path("out/r.html").is_file(),
        "a folder is made for it"
    );
}

#[test]
fn a_run_with_no_report_flag_still_has_its_run_and_prints_no_dashboard() {
    let fixture = Fixture::new();

    let output = fixture.run(&["test"]);

    assert!(
        !stdout(&output).contains("Dashboard:"),
        "{}",
        stdout(&output)
    );
    assert!(fixture.path(".sbrs_run/run.json").is_file());
    assert!(!fixture.path("dashboard.html").exists());
    assert!(!fixture.path("report.html").exists());
    assert!(
        stdout(&output).contains("----- Latest Logs dir: "),
        "a failed test leaves its logs: {}",
        stdout(&output)
    );
}

#[test]
fn the_start_of_a_run_archives_the_old_logs_when_asked() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.path("latest_logs/old")).expect("old logs");
    std::fs::write(fixture.path("latest_logs/old/x.txt"), "old").expect("old file");

    let output = fixture.run(&["test", "--archive-logs"]);

    assert!(output.status.code().is_some(), "{}", stderr(&output));
    let archived: Vec<PathBuf> = std::fs::read_dir(fixture.path("archived_logs"))
        .expect("an archive folder")
        .map(|entry| entry.expect("entry").path())
        .collect();
    assert!(
        archived.iter().any(|dir| dir.join("old/x.txt").is_file()),
        "{archived:?}"
    );
    assert!(
        !fixture.path("latest_logs/old").exists(),
        "the new run starts with clean logs"
    );
}

#[test]
fn without_archiving_the_old_logs_are_deleted() {
    let fixture = Fixture::new();
    std::fs::create_dir_all(fixture.path("latest_logs/old")).expect("old logs");
    std::fs::write(fixture.path("latest_logs/old/x.txt"), "old").expect("old file");

    fixture.run(&["test"]);

    assert!(!fixture.path("latest_logs/old").exists());
    assert!(!fixture.path("archived_logs").exists());
}

#[test]
fn a_report_that_cannot_be_written_fails_a_run_that_passed() {
    let fixture = Fixture::new();
    // The report's folder would be a file.
    std::fs::write(fixture.path("blocked"), "x").expect("a file");

    let output = Command::new(SBASE)
        .args(["test", "--html", "blocked/r.html"])
        .current_dir(fixture.work.path())
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", fixture.bin.path().display()),
        )
        .env("FAKE_CARGO_EXIT", "0")
        .env("FAKE_CARGO_EVENTS", "0")
        .output()
        .expect("sbrs runs");

    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("HTML report"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_passing_run_exits_with_zero_and_says_where_its_reports_are() {
    let fixture = Fixture::new();

    let output = Command::new(SBASE)
        .args(["test", "--html", "r.html"])
        .current_dir(fixture.work.path())
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", fixture.bin.path().display()),
        )
        .env("FAKE_CARGO_EXIT", "0")
        .env("FAKE_CARGO_EVENTS", "0")
        .output()
        .expect("sbrs runs");

    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).contains("----- Generated html report: file://"),
        "{}",
        stdout(&output)
    );
    assert!(
        !stdout(&output).contains("----- Dashboard ="),
        "the dashboard is named again only when tests failed"
    );
    assert!(fixture.path("r.html").is_file());
}

#[test]
fn show_report_needs_something_to_show() {
    let fixture = Fixture::new();

    let output = fixture.run(&["test", "--show-report"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("--show-report needs --html or --dashboard"),
        "{}",
        stderr(&output)
    );
    assert!(
        !stdout(&output).contains("args="),
        "cargo must not have run"
    );
}

#[test]
#[cfg(not(feature = "turso"))]
fn db_reporting_needs_the_turso_feature() {
    let fixture = Fixture::new();

    let output = fixture.run(&["test", "--with-db-reporting"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains(
            "--with-db-reporting needs sbrs built with the turso feature: \
             cargo install --path crates/cli --features turso"
        ),
        "{}",
        stderr(&output)
    );
    assert!(
        !stdout(&output).contains("args="),
        "cargo must not have run"
    );
}

#[test]
fn a_listing_is_no_run() {
    let fixture = Fixture::new();

    let output = fixture.run(&["test", "--co", "--dashboard"]);

    assert!(!fixture.path(".sbrs_run").exists());
    assert!(!fixture.path("dashboard.html").exists());
    assert!(
        !stdout(&output).contains("SB_RUN_ID="),
        "{}",
        stdout(&output)
    );
}
