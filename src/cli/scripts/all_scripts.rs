//! `sbrs test --all-scripts`: after the tests, run every example.
//!
//! Python SeleniumBase's `--all-scripts` runs the scripts of its examples
//! folder as tests. Here an example is a Cargo example target, so the run is
//! `cargo run --example NAME` in the folder of each selected package, with the
//! environment of the tests. An example whose `required-features` are not all
//! enabled is skipped (it would not build). Each example gets a time limit, so
//! one that waits for input cannot hold the run forever.
//!
//! The result of an example is one line, `example NAME ... ok` or
//! `example NAME ... FAILED (status N)`, and a run with a failed example
//! fails.
//!
//! # Examples
//!
//! ```
//! use seleniumbase_rs::cli::scripts::all_scripts::{report_line, Ended};
//!
//! assert_eq!(report_line("login", &Ended::Ok), "example login ... ok");
//! assert_eq!(
//!     report_line("login", &Ended::Failed(Some(101))),
//!     "example login ... FAILED (status 101)"
//! );
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long an example may run when no time limit is given.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// One example target of a package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Example {
    /// The example's name (`cargo run --example NAME`).
    pub name: String,
    /// The folder of the package that has it, where it is run.
    pub dir: PathBuf,
}

/// How one example ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ended {
    /// It exited with status 0.
    Ok,
    /// It exited with a failure status (`None` when a signal ended it).
    Failed(Option<i32>),
    /// It ran past its time limit and was stopped.
    TimedOut(Duration),
    /// It could not be started (for example `cargo` is missing).
    NotStarted(String),
}

impl Ended {
    /// Whether the example passed.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }
}

/// The line `sbrs test --all-scripts` prints for an example.
#[must_use]
pub fn report_line(name: &str, ended: &Ended) -> String {
    match ended {
        Ended::Ok => format!("example {name} ... ok"),
        Ended::Failed(Some(status)) => format!("example {name} ... FAILED (status {status})"),
        Ended::Failed(None) => format!("example {name} ... FAILED (ended by a signal)"),
        Ended::TimedOut(limit) => format!(
            "example {name} ... FAILED (stopped after {} seconds)",
            limit.as_secs()
        ),
        Ended::NotStarted(why) => format!("example {name} ... FAILED (not started: {why})"),
    }
}

/// The features that are on for a package: those asked for, the package's
/// default features and what they switch on in turn.
fn enabled_features(package: &serde_json::Value, asked: &[String]) -> BTreeSet<String> {
    let features = package.get("features").and_then(|f| f.as_object());
    let mut on: BTreeSet<String> = asked.iter().cloned().collect();
    on.insert("default".into());
    let mut queue: Vec<String> = on.iter().cloned().collect();
    while let Some(name) = queue.pop() {
        let members = features
            .and_then(|map| map.get(&name))
            .and_then(|list| list.as_array());
        for member in members.into_iter().flatten().filter_map(|m| m.as_str()) {
            // `dep:x` and `x/feature` entries name dependencies, not features of this package.
            if member.contains(':') || member.contains('/') {
                continue;
            }
            if on.insert(member.to_owned()) {
                queue.push(member.to_owned());
            }
        }
    }
    on
}

/// The examples of the packages in `dirs`, from the output of
/// `cargo metadata --no-deps --format-version 1`, without those whose
/// `required-features` are not enabled by `asked` and the defaults. `None`
/// when the text is not cargo's metadata.
#[must_use]
pub fn examples_from_metadata(
    metadata: &str,
    dirs: &[PathBuf],
    asked: &[String],
) -> Option<Vec<Example>> {
    let value: serde_json::Value = serde_json::from_str(metadata).ok()?;
    let mut found = Vec::new();
    for package in value.get("packages")?.as_array()? {
        let manifest = package.get("manifest_path")?.as_str()?;
        let Some(dir) = Path::new(manifest).parent() else {
            continue;
        };
        if !dirs.iter().any(|selected| selected == dir) {
            continue;
        }
        let on = enabled_features(package, asked);
        for target in package.get("targets")?.as_array()? {
            let is_example = target
                .get("kind")
                .and_then(|kind| kind.as_array())
                .is_some_and(|kinds| kinds.iter().any(|k| k == "example"));
            let needs: Vec<&str> = target
                .get("required-features")
                .and_then(|list| list.as_array())
                .map(|list| list.iter().filter_map(|f| f.as_str()).collect())
                .unwrap_or_default();
            if is_example && needs.iter().all(|feature| on.contains(*feature)) {
                if let Some(name) = target.get("name").and_then(|n| n.as_str()) {
                    found.push(Example {
                        name: name.to_owned(),
                        dir: dir.to_path_buf(),
                    });
                }
            }
        }
    }
    found.sort_by(|a, b| (&a.dir, &a.name).cmp(&(&b.dir, &b.name)));
    Some(found)
}

/// The examples of the packages in `dirs`, found with `cargo metadata`.
///
/// # Errors
///
/// Returns a message when `cargo metadata` cannot be run or its output is not
/// understood.
pub fn find_examples(dirs: &[PathBuf], asked: &[String]) -> Result<Vec<Example>, String> {
    let first = dirs.first().ok_or("no package to look for examples in")?;
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(first)
        .stderr(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err("cargo metadata failed".into());
    }
    let text = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    examples_from_metadata(&text, dirs, asked)
        .ok_or_else(|| "cannot read the output of cargo metadata".to_owned())
}

/// Runs `command` with this process's input and output and stops it after
/// `limit`.
#[must_use]
pub fn run_with_limit(mut command: Command, limit: Duration) -> Ended {
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return Ended::NotStarted(error.to_string()),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ended::Ok,
            Ok(Some(status)) => return Ended::Failed(status.code()),
            Ok(None) if start.elapsed() >= limit => {
                // Both calls can only fail when the child is already gone.
                let _ = child.kill();
                let _ = child.wait();
                return Ended::TimedOut(limit);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(error) => return Ended::NotStarted(error.to_string()),
        }
    }
}

/// The `cargo run --example` command for `example`.
#[must_use]
pub fn example_command(
    example: &Example,
    release: bool,
    features: &[String],
    env: &[(String, String)],
) -> Command {
    let mut command = Command::new("cargo");
    command.arg("run");
    if release {
        command.arg("--release");
    }
    if !features.is_empty() {
        command.args(["--features", &features.join(",")]);
    }
    command.args(["--example", &example.name]);
    command.current_dir(&example.dir);
    for (name, value) in env {
        command.env(name, value);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &str = r#"{
        "packages": [
            {
                "id": "a 0.1.0",
                "manifest_path": "/w/a/Cargo.toml",
                "features": {"default": ["tui"], "tui": [], "gui": ["dep:eframe"], "all": ["gui", "tui"]},
                "targets": [
                    {"name": "a", "kind": ["lib"], "required-features": []},
                    {"name": "basic", "kind": ["example"]},
                    {"name": "tui_demo", "kind": ["example"], "required-features": ["tui"]},
                    {"name": "gui_demo", "kind": ["example"], "required-features": ["gui"]},
                    {"name": "t", "kind": ["test"]}
                ]
            },
            {
                "id": "b 0.1.0",
                "manifest_path": "/w/b/Cargo.toml",
                "features": {},
                "targets": [{"name": "other", "kind": ["example"]}]
            }
        ]
    }"#;

    fn names(examples: &[Example]) -> Vec<&str> {
        examples.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn only_examples_of_the_selected_packages_with_their_features_on_are_found() {
        let dirs = [PathBuf::from("/w/a")];
        let found = examples_from_metadata(METADATA, &dirs, &[]).unwrap();
        // `tui` is a default feature; `gui` is off.
        assert_eq!(names(&found), ["basic", "tui_demo"]);
        assert!(found.iter().all(|e| e.dir == Path::new("/w/a")));
    }

    #[test]
    fn a_feature_asked_for_switches_on_the_examples_that_need_it() {
        let dirs = [PathBuf::from("/w/a")];
        let found = examples_from_metadata(METADATA, &dirs, &["gui".to_owned()]).unwrap();
        assert_eq!(names(&found), ["basic", "gui_demo", "tui_demo"]);
        // A feature that switches others on counts for them.
        let found = examples_from_metadata(METADATA, &dirs, &["all".to_owned()]).unwrap();
        assert_eq!(names(&found), ["basic", "gui_demo", "tui_demo"]);
    }

    #[test]
    fn several_packages_are_listed_in_folder_order() {
        let dirs = [PathBuf::from("/w/b"), PathBuf::from("/w/a")];
        let found = examples_from_metadata(METADATA, &dirs, &[]).unwrap();
        assert_eq!(names(&found), ["basic", "tui_demo", "other"]);
        assert_eq!(examples_from_metadata("nonsense", &dirs, &[]), None);
    }

    #[test]
    fn a_report_line_says_how_the_example_ended() {
        assert_eq!(report_line("x", &Ended::Ok), "example x ... ok");
        assert_eq!(
            report_line("x", &Ended::Failed(Some(2))),
            "example x ... FAILED (status 2)"
        );
        assert_eq!(
            report_line("x", &Ended::TimedOut(Duration::from_secs(300))),
            "example x ... FAILED (stopped after 300 seconds)"
        );
        assert!(Ended::Ok.is_ok() && !Ended::Failed(None).is_ok());
    }

    #[test]
    fn the_example_command_runs_in_its_package_with_the_environment() {
        let example = Example {
            name: "basic".into(),
            dir: PathBuf::from("/w/a"),
        };
        let command = example_command(
            &example,
            true,
            &["tui".to_owned()],
            &[("SB_HEADLESS".to_owned(), "true".to_owned())],
        );
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "run",
                "--release",
                "--features",
                "tui",
                "--example",
                "basic"
            ]
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("/w/a")));
        assert!(command
            .get_envs()
            .any(|(k, v)| k == "SB_HEADLESS" && v.is_some_and(|v| v == "true")));
    }

    #[cfg(unix)]
    #[test]
    fn a_command_is_judged_by_its_status_and_stopped_at_the_limit() {
        let sh = |script: &str| {
            let mut command = Command::new("/bin/sh");
            command.args(["-c", script]);
            command
        };
        let limit = Duration::from_secs(5);
        assert_eq!(run_with_limit(sh("exit 0"), limit), Ended::Ok);
        assert_eq!(run_with_limit(sh("exit 3"), limit), Ended::Failed(Some(3)));
        let started = Instant::now();
        let short = Duration::from_millis(200);
        assert_eq!(
            run_with_limit(sh("sleep 30"), short),
            Ended::TimedOut(short)
        );
        assert!(started.elapsed() < Duration::from_secs(10));
        let missing = Command::new("/nonexistent/program");
        assert!(matches!(
            run_with_limit(missing, limit),
            Ended::NotStarted(_)
        ));
    }
}
