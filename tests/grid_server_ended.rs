//! `GridProcess::ended`, together with `start`, `state` and `stop`, through the
//! public API.
//!
//! No Selenium Server jar or Java is needed. The "Java" is a small shell script
//! that logs its arguments and then waits for a release file, so a test decides
//! when the Grid ends on its own. Every process here is a real child started by
//! `GridProcess::start`; only the program it runs is a stand-in. The unit tests
//! in `utilities::grid_server` cover start-up failures, `stop` and record
//! handling but not `ended`, which is what a caller asks while it waits for the
//! Grid to answer `/status`.

#![cfg(unix)]

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use seleniumbase_rs::utilities::grid_server::{
    GridLaunch, GridProcess, GridRecord, GridRole, RunState, StopOutcome,
};

/// How long `start` watches for the stand-in dying straight away.
const SETTLE: Duration = Duration::from_millis(200);

/// How long a test waits for the stand-in to end after it is released. Generous,
/// because the machine may be busy with a build.
const PATIENCE: Duration = Duration::from_secs(20);

struct Fixture {
    /// Held so the jar, the stand-in and the state live as long as the test.
    _dir: tempfile::TempDir,
    role: GridRole,
    jar: PathBuf,
    java: PathBuf,
    /// Creating this file makes the stand-in end on its own.
    release: PathBuf,
    process: GridProcess,
}

/// A stand-in `java` that logs its arguments, waits for `release` to exist and
/// then logs and exits with status 4. When asked to stop it logs and exits 0.
///
/// The wait also ends if the script itself is deleted, so a test that fails
/// before releasing it cannot leave the process behind.
fn stand_in_java(dir: &Path, release: &Path) -> PathBuf {
    let path = dir.join("fake-java");
    let script = format!(
        "#!/bin/sh\n\
         [ \"$1\" = --warm ] && exit 0\n\
         echo \"fake java: $*\"\n\
         trap 'echo \"fake java: asked to stop\"; exit 0' TERM\n\
         while [ ! -e '{release}' ] && [ -e \"$0\" ]; do sleep 0.1; done\n\
         echo 'fake java: released, ending on its own'\n\
         exit 4\n",
        release = release.display()
    );
    fs::write(&path, script).expect("write the stand-in");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("make it executable");
    // macOS can hold up the first run of a program that was just written for
    // seconds while it checks it, which would outlast the start-up watch in
    // `start`. Run it once here, where waiting costs nothing. Another thread's
    // `fork` can also leave the file busy for a moment, so try a few times.
    let mut attempts = 0;
    let warmed = loop {
        match Command::new(&path).arg("--warm").status() {
            Err(error) if error.kind() == ErrorKind::ExecutableFileBusy && attempts < 20 => {
                attempts += 1;
                thread::sleep(Duration::from_millis(50));
            }
            other => break other.expect("the stand-in Java runs"),
        }
    };
    assert!(
        warmed.success(),
        "the stand-in Java did not warm up: {warmed}"
    );
    path
}

fn fixture(role: GridRole) -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let jar = dir.path().join("selenium-server-test.jar");
    fs::write(&jar, b"not a real jar").expect("write the jar");
    let release = dir.path().join("release");
    let java = stand_in_java(dir.path(), &release);
    let process = GridProcess::new(dir.path().join("state"), role);
    Fixture {
        _dir: dir,
        role,
        jar,
        java,
        release,
        process,
    }
}

fn start(f: &Fixture) -> GridRecord {
    let launch = match f.role {
        GridRole::Hub => GridLaunch::hub(&f.jar),
        GridRole::Node => GridLaunch::node(&f.jar, "127.0.0.1".parse().expect("a hub address")),
    }
    .java(&f.java);
    f.process
        .start(&launch, SETTLE)
        .expect("the stand-in Grid starts")
}

/// Asks `ended` until it answers, or `None` if it never does.
fn wait_for_ended(process: &GridProcess) -> Option<String> {
    let deadline = Instant::now() + PATIENCE;
    loop {
        if let Some(note) = process.ended().expect("the record can be read") {
            return Some(note);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Writes `record` as the record `process` looks for, as an earlier run would
/// have left it.
fn write_record(process: &GridProcess, record: &GridRecord) {
    fs::create_dir_all(process.record_path().parent().expect("a state dir")).expect("state dir");
    fs::write(
        process.record_path(),
        serde_json::to_vec(record).expect("a record serialises"),
    )
    .expect("write the record");
}

/// A record whose process id is a live process that is not a Grid: this test.
fn impostor(f: &Fixture) -> GridRecord {
    GridRecord {
        role: f.role,
        pid: std::process::id(),
        jar: f.jar.clone(),
        port: 4444,
        started_at: 0,
        log: f.process.log_path(),
    }
}

#[test]
fn nothing_has_ended_when_nothing_was_started() {
    let f = fixture(GridRole::Hub);

    assert_eq!(f.process.ended().unwrap(), None);
    assert_eq!(f.process.state().unwrap(), RunState::NotRunning);
}

#[test]
fn a_grid_is_not_ended_while_it_runs_nor_after_it_was_stopped_on_purpose() {
    let f = fixture(GridRole::Hub);

    let record = start(&f);
    assert_eq!(
        f.process.state().unwrap(),
        RunState::Running(record.clone())
    );
    assert_eq!(f.process.ended().unwrap(), None, "it is still running");

    let outcome = f.process.stop(Duration::from_secs(10)).unwrap();

    assert_eq!(outcome, StopOutcome::Stopped(record));
    assert_eq!(f.process.state().unwrap(), RunState::NotRunning);
    assert_eq!(
        f.process.ended().unwrap(),
        None,
        "a Grid that was stopped on purpose, and whose record is gone, has not ended"
    );
}

#[test]
fn a_grid_that_ends_on_its_own_is_reported_with_the_end_of_its_log() {
    for role in [GridRole::Hub, GridRole::Node] {
        let f = fixture(role);
        let record = start(&f);
        assert_eq!(f.process.ended().unwrap(), None, "{role:?} is running");

        fs::write(&f.release, b"").unwrap();
        let note = wait_for_ended(&f.process)
            .unwrap_or_else(|| panic!("the {role:?} ended but `ended` never said so"));

        let name = role.as_str();
        assert!(
            note.contains(&format!(
                "The Grid {name} (process {}) is no longer running.",
                record.pid
            )),
            "{note}"
        );
        assert!(note.contains(&record.log.display().to_string()), "{note}");
        assert!(
            note.contains(&format!("fake java: -jar {}", f.jar.display())),
            "the log's start is in the note: {note}"
        );
        assert!(note.contains(&format!(" {name} --port ")), "{note}");
        assert!(
            note.contains("fake java: released, ending on its own"),
            "the log's end is in the note: {note}"
        );

        // Asking is read-only: the same answer again, and the record stays
        // until somebody stops (that is, cleans up) the Grid.
        assert_eq!(f.process.ended().unwrap().as_deref(), Some(note.as_str()));
        assert!(f.process.record_path().exists());
        assert_eq!(f.process.state().unwrap(), RunState::Stale(record.clone()));

        let outcome = f.process.stop(Duration::from_millis(100)).unwrap();

        assert_eq!(outcome, StopOutcome::StaleRemoved(record));
        assert_eq!(f.process.state().unwrap(), RunState::NotRunning);
        assert_eq!(f.process.ended().unwrap(), None);
    }
}

#[test]
fn a_dead_grid_is_replaced_by_a_new_start_and_is_then_running_not_ended() {
    let f = fixture(GridRole::Hub);
    let first = start(&f);
    fs::write(&f.release, b"").unwrap();
    assert!(wait_for_ended(&f.process).is_some());
    fs::remove_file(&f.release).unwrap();

    let second = start(&f);

    assert_ne!(second.pid, first.pid);
    assert_eq!(f.process.state().unwrap(), RunState::Running(second));
    assert_eq!(f.process.ended().unwrap(), None);
    f.process.stop(Duration::from_secs(10)).unwrap();
}

#[test]
fn a_record_whose_process_id_now_belongs_to_another_program_counts_as_ended() {
    let f = fixture(GridRole::Hub);
    let record = impostor(&f);
    write_record(&f.process, &record);
    // 20 lines, of which `ended` keeps the last 15.
    let log: String = (1..=20).map(|n| format!("log line {n:02}\n")).collect();
    fs::write(f.process.log_path(), log).unwrap();

    let note = f.process.ended().unwrap().expect("the record is stale");

    assert!(
        note.contains(&format!("(process {}) is no longer running", record.pid)),
        "{note}"
    );
    assert!(note.contains("log line 20"), "{note}");
    assert!(note.contains("log line 06"), "{note}");
    assert!(!note.contains("log line 05"), "only 15 lines: {note}");
    // The impostor is this very process, which is of course still here.
}

#[test]
fn the_note_says_when_the_log_is_empty_or_cannot_be_read() {
    let f = fixture(GridRole::Hub);
    write_record(&f.process, &impostor(&f));

    fs::write(f.process.log_path(), "").unwrap();
    let empty = f.process.ended().unwrap().expect("the record is stale");
    assert!(empty.contains("(the log is empty)"), "{empty}");

    fs::remove_file(f.process.log_path()).unwrap();
    let missing = f.process.ended().unwrap().expect("the record is stale");
    assert!(missing.contains("could not be read"), "{missing}");
}

#[test]
fn a_dead_hub_says_nothing_about_the_node() {
    let f = fixture(GridRole::Hub);
    write_record(&f.process, &impostor(&f));
    assert!(f.process.ended().unwrap().is_some());

    let node = GridProcess::new(f.process.record_path().parent().unwrap(), GridRole::Node);

    assert_eq!(node.ended().unwrap(), None);
}

#[test]
fn a_damaged_record_is_an_error_not_an_ended_grid() {
    let f = fixture(GridRole::Hub);
    fs::create_dir_all(f.process.record_path().parent().unwrap()).unwrap();
    fs::write(f.process.record_path(), "{ not json").unwrap();

    let error = f.process.ended().unwrap_err();

    assert!(error.to_string().contains("damaged"), "{error}");
}
