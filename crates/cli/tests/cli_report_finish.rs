//! `sbrs report finish`, run as the real binary on a folder with a run that was left going, as
//! a plain `cargo test` leaves it. The run is made with the library, the way the `sb` fixture
//! makes it. No browser or network is involved.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use seleniumbase_rs::core::log_helper::TestId;
use seleniumbase_rs::core::test_run::{ReportSettings, RunKey, TestRun};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::Value;

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// A folder with a run of three tests: one passed, one failed, one started and never ended.
/// The recorder of the last one is returned so that it stays alive (a dropped one would end the
/// test itself).
struct LeftRun {
    dir: tempfile::TempDir,
    _hung: seleniumbase_rs::core::test_run::TestRecorder,
}

impl LeftRun {
    fn new(settings: ReportSettings) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = std::fs::canonicalize(dir.path()).expect("the folder exists");
        let run = TestRun::open(root, settings, RunKey::new("fixture")).expect("the run opens");
        run.start(TestId::named("shop::good"))
            .expect("start")
            .finish(&Ok(()))
            .expect("finish");
        run.start(TestId::named("shop::bad"))
            .expect("start")
            .finish(&Err(SeleniumBaseError::invalid_config("boom")))
            .expect("finish");
        let hung = run.start(TestId::named("shop::hung")).expect("start");
        Self { dir, _hung: hung }
    }

    fn root(&self) -> PathBuf {
        std::fs::canonicalize(self.dir.path()).expect("the folder exists")
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }
}

fn sbrs(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(SBASE);
    command
        .arg("report")
        .arg("finish")
        .args(args)
        .current_dir(dir)
        .env("SB_LOG_LEVEL", "error");
    for (name, _) in std::env::vars() {
        if name.starts_with("SB_") && name != "SB_LOG_LEVEL" {
            command.env_remove(name);
        }
    }
    command.output().expect("sbrs starts")
}

fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn live() -> ReportSettings {
    ReportSettings {
        dashboard: true,
        ..ReportSettings::default()
    }
}

#[test]
fn finish_ends_the_run_and_writes_the_reports_it_is_asked_for() {
    let run = LeftRun::new(live());
    assert!(
        read(&run.path("dashboard.html")).contains("http-equiv=\"refresh\""),
        "the dashboard is live before the run is finished"
    );

    let output = sbrs(
        &run.root(),
        &["--html", "report.html", "--junit-xml", "report.xml"],
    );

    assert!(output.status.success(), "{}", err(&output));
    let text = out(&output);
    for line in [
        "----- Dashboard = file://",
        "----- Generated html report: file://",
        "----- generated xml file: ",
        "----- Latest Logs dir: ",
    ] {
        assert!(text.contains(line), "{line} in {text}");
    }
    let dashboard = read(&run.path("dashboard.html"));
    assert!(
        !dashboard.contains("http-equiv=\"refresh\""),
        "the finished dashboard stays: {dashboard}"
    );
    assert!(dashboard.contains("UNREPORTED"), "{dashboard}");
    let html = read(&run.path("report.html"));
    assert!(
        html.contains("shop::good") && html.contains("shop::bad"),
        "{html}"
    );
    let xml = read(&run.path("report.xml"));
    assert_eq!(xml.matches("<testcase").count(), 3, "{xml}");
    assert!(xml.contains("<failure") || xml.contains("<error"), "{xml}");
    let state = read(&run.path(".sbrs_run/run.json"));
    assert!(
        !state.contains("\"finalized_at\":null") && !state.contains("\"finalized_at\": null"),
        "the run is finalized: {state}"
    );
}

#[test]
fn finish_can_run_again_and_gives_the_same_reports() {
    let run = LeftRun::new(live());

    let first = sbrs(&run.root(), &["--html", "report.html"]);
    let second = sbrs(&run.root(), &["--html", "report.html"]);

    assert!(first.status.success() && second.status.success());
    assert!(read(&run.path("report.html")).contains("shop::bad"));
}

#[test]
fn finish_in_a_folder_with_no_run_says_so_and_exits_with_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = std::fs::canonicalize(dir.path()).expect("the folder exists");

    let output = sbrs(&root, &[]);

    assert_eq!(output.status.code(), Some(1), "{}", err(&output));
    let expected = format!(
        "there is no test run in {root} (looked for {root}/.sbrs_run/run.json); \
         run tests that use the sb fixture first",
        root = root.display()
    );
    assert!(err(&output).contains(&expected), "{}", err(&output));
    assert!(out(&output).is_empty(), "{}", out(&output));
}

#[test]
fn finish_finds_the_run_through_sb_report_root() {
    let run = LeftRun::new(live());
    let elsewhere = tempfile::tempdir().expect("temp dir");

    let output = Command::new(SBASE)
        .args(["report", "finish", "--html", "report.html"])
        .current_dir(elsewhere.path())
        .env("SB_REPORT_ROOT", run.root())
        .env("SB_LOG_LEVEL", "error")
        .output()
        .expect("sbrs starts");

    assert!(output.status.success(), "{}", err(&output));
    assert!(
        run.path("report.html").is_file(),
        "a relative report is the run's, not the working folder's"
    );
}

#[test]
fn finish_json_says_what_was_done() {
    let run = LeftRun::new(live());

    let output = sbrs(&run.root(), &["--json", "--junit-xml", "x.xml"]);

    assert!(output.status.success(), "{}", err(&output));
    let done: Value = serde_json::from_str(&out(&output)).expect("JSON on standard output");
    assert_eq!(done["summary"]["total"], 3);
    assert_eq!(done["summary"]["passed"], 1);
    assert_eq!(done["summary"]["failed"], 1);
    assert_eq!(done["summary"]["unreported"], 1);
    assert!(done["dashboard"]
        .as_str()
        .unwrap()
        .ends_with("dashboard.html"));
    assert!(done["junit_xml"].as_str().unwrap().ends_with("x.xml"));
    assert!(done["html"].is_null());
    assert!(done["warnings"].as_array().unwrap().is_empty(), "{done}");
}

#[test]
fn finish_show_report_needs_something_to_show() {
    let run = LeftRun::new(ReportSettings::default());

    let output = sbrs(&run.root(), &["--show-report"]);

    assert_eq!(output.status.code(), Some(2), "{}", err(&output));
    assert!(
        err(&output).contains("--show-report needs --html or --dashboard"),
        "{}",
        err(&output)
    );
}

#[test]
fn finish_refuses_a_flag_given_twice_and_an_unknown_one() {
    let run = LeftRun::new(live());

    assert!(
        !sbrs(&run.root(), &["--html", "a.html", "--html", "b.html"])
            .status
            .success()
    );
    assert!(!sbrs(&run.root(), &["--nosuch"]).status.success());
}
