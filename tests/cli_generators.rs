//! The `sbase` commands that write starter files (`mkchart`, `mkpres`, `mkrec`,
//! `objectify`, `caseplans --starter`, `behave-gui --new`) and `sbase print`,
//! run as the real binary in a scratch folder.
//!
//! `caseplans` and `behave-gui` on their own are terminal apps (feature `tui`);
//! `tests/cli_terminal_apps.rs` covers them.
//!
//! None of them starts a browser or touches the network.
//!
//! A command that fails exits with the status of `SeleniumBaseError::exit_code`
//! for its kind of failure (6 for a local file), never 0.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

/// The exit status for a failed local file operation (`SeleniumBaseError::Io`).
const IO_FAILURE: i32 = 6;

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(SBASE);
    command
        .args(args)
        .current_dir(dir)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error");
    command
}

fn run(dir: &Path, args: &[&str]) -> Output {
    command(dir, args)
        .stdin(Stdio::null())
        .output()
        .expect("sbase runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Waits for `child`, which must finish within `limit`; kills it otherwise.
fn finish_within(mut child: std::process::Child, limit: Duration) -> Option<Output> {
    let start = Instant::now();
    loop {
        if child.try_wait().expect("poll sbase").is_some() {
            return Some(child.wait_with_output().expect("collect output"));
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_starter_file_commands_do_not_replace_a_file_that_is_there() {
    // (arguments, the file they write, what the success line says)
    let generators: [(&[&str], &str, &str); 5] = [
        (
            &["mkchart", "--file", "chart.html"],
            "chart.html",
            "Created chart",
        ),
        (
            &["mkpres", "--file", "deck.html"],
            "deck.html",
            "Created presentation",
        ),
        (
            &["mkrec", "--file", "rec.rs"],
            "rec.rs",
            "Created recorder file",
        ),
        (&["objectify"], "page_object.rs", "Generated"),
        (&["caseplans", "--starter"], "CASE_PLANS.md", "Generated"),
    ];
    for (args, file, success) in generators {
        let dir = tempfile::tempdir().expect("temp dir");

        let first = run(dir.path(), args);
        assert!(first.status.success(), "{args:?}: {}", stderr(&first));
        assert!(
            stdout(&first).contains(success),
            "{args:?}: {}",
            stdout(&first)
        );
        assert!(dir.path().join(file).is_file(), "{args:?} wrote no {file}");

        std::fs::write(dir.path().join(file), "my own work").expect("edit the file");
        let second = run(dir.path(), args);

        assert_eq!(
            second.status.code(),
            Some(IO_FAILURE),
            "{args:?} must fail, not exit 0: {}",
            stderr(&second)
        );
        assert!(
            stderr(&second).contains("already exists"),
            "{args:?}: {}",
            stderr(&second)
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(file)).unwrap(),
            "my own work",
            "{args:?} replaced the file"
        );
    }
}

#[test]
fn the_page_commands_create_the_folders_of_the_path_they_are_given() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = run(
        dir.path(),
        &["mkchart", "--file", "reports/2026/chart.html"],
    );

    assert!(
        stdout(&output).contains("Created chart"),
        "{}",
        stderr(&output)
    );
    assert!(dir.path().join("reports/2026/chart.html").is_file());
}

#[test]
fn behave_gui_writes_the_feature_it_is_told_about() {
    let dir = tempfile::tempdir().expect("temp dir");
    let answers = "login\nLog in\nuser\nto log in\nI can work\n1\nvalid login\n\
                   Given I am on the login page\nThen I see my account\ndone\n";
    let mut child = command(dir.path(), &["behave-gui", "--new"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sbase starts");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(answers.as_bytes())
        .expect("write the answers");

    let output = finish_within(child, Duration::from_secs(30)).expect("behave-gui finishes");

    assert!(
        stdout(&output).contains("login.feature"),
        "{}",
        stderr(&output)
    );
    let feature = std::fs::read_to_string(dir.path().join("features/login.feature")).unwrap();
    assert!(feature.starts_with("Feature: Log in\n"), "{feature}");
    assert!(
        feature.contains("    Given I am on the login page\n"),
        "{feature}"
    );
    assert!(dir.path().join("features/README.md").is_file());
}

#[test]
fn behave_gui_gives_up_when_its_input_ends_instead_of_asking_for_ever() {
    let dir = tempfile::tempdir().expect("temp dir");
    // The answers stop in the middle of a scenario's steps.
    let answers = "login\nT\na\nb\nc\n1\ns\nGiven a step\n";
    let mut child = command(dir.path(), &["behave-gui", "--new"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sbase starts");
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin
            .write_all(answers.as_bytes())
            .expect("write the answers");
        // Dropped here: the input is over.
    }

    let output = finish_within(child, Duration::from_secs(30))
        .expect("behave-gui must end when its input ends, not wait or loop");

    assert_eq!(
        output.status.code(),
        Some(IO_FAILURE),
        "a half-answered feature is a failure: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("input ended"),
        "{}",
        stderr(&output)
    );
    assert!(
        !dir.path().join("features/login.feature").exists(),
        "a half-answered feature is not written"
    );
}

#[test]
fn behave_gui_fails_when_the_feature_name_is_a_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut child = command(dir.path(), &["behave-gui", "--new"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sbase starts");
    {
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(b"../escape\n").expect("write the answers");
    }

    let output = finish_within(child, Duration::from_secs(30)).expect("behave-gui finishes");

    assert_eq!(
        output.status.code(),
        Some(IO_FAILURE),
        "a refused name is a failure: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("not a plain file name"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn print_fails_for_a_file_that_is_not_there() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = run(dir.path(), &["print", "--file", "missing.txt"]);

    assert_eq!(
        output.status.code(),
        Some(IO_FAILURE),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("missing.txt") || stderr(&output).contains("No such file"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn print_shows_a_file_that_is_not_valid_utf8() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("latin1.txt"), b"caf\xe9 au lait\n").unwrap();

    let output = run(dir.path(), &["print", "--file", "latin1.txt"]);

    assert!(
        stdout(&output).contains("caf\u{fffd} au lait"),
        "{}",
        stderr(&output)
    );
}
