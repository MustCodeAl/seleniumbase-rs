//! `sbrs fingerprint` and the global `--fingerprint`, run as the real binary. None of these
//! starts a browser: the real-Chrome ones are in `cli_fingerprint_chrome.rs`.

#[path = "support/sbrs.rs"]
mod sbrs;

use std::path::{Path, PathBuf};
use std::process::Output;

use sbrs::{run, stderr, stdout};

fn path(file: &Path) -> &str {
    file.to_str().expect("the folder name is UTF-8")
}

fn status(output: &Output) -> i32 {
    output.status.code().expect("sbrs ended by itself")
}

/// Writes the preset `name` into `dir` and returns the file.
fn exported(dir: &Path, name: &str) -> PathBuf {
    let file = dir.join(format!("{name}.json"));
    let output = run(
        dir,
        &[],
        &["fingerprint", "export", "--preset", name, "-o", path(&file)],
    );
    assert_eq!(status(&output), 0, "{}", stderr(&output));
    file
}

#[test]
fn presets_lists_every_preset_with_what_it_claims() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &[], &["fingerprint", "presets"]);

    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let table = stdout(&output);
    for name in ["windows", "edge", "macos", "linux", "android", "ios"] {
        let row = sbrs::row(&table, name);
        assert!(row.len() > 3, "{name}: {row:?}");
    }
}

#[test]
fn a_preset_is_written_to_the_file_and_checks_out() {
    let dir = tempfile::tempdir().unwrap();
    let file = exported(dir.path(), "windows");

    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert!(json.is_object(), "{json}");

    let check = run(dir.path(), &[], &["fingerprint", "check", path(&file)]);
    assert_eq!(status(&check), 0, "{}", stderr(&check));
    assert!(
        stdout(&check).trim_end().ends_with("coherent"),
        "{}",
        stdout(&check)
    );
}

#[cfg(unix)]
#[test]
fn the_file_is_private_to_its_owner_because_a_fingerprint_may_hold_proxy_credentials() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let file = exported(dir.path(), "linux");

    let mode = std::fs::metadata(&file).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn without_a_file_the_fingerprint_goes_to_standard_output() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(
        dir.path(),
        &[],
        &["fingerprint", "export", "--preset", "macos"],
    );

    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert!(json.is_object());
}

#[test]
fn a_random_identity_follows_its_seed_and_its_system() {
    let dir = tempfile::tempdir().unwrap();
    let drawn = |seed: &str, os: &str| {
        let output = run(
            dir.path(),
            &[],
            &[
                "fingerprint",
                "export",
                "--random",
                "--seed",
                seed,
                "--os",
                os,
            ],
        );
        assert_eq!(status(&output), 0, "{}", stderr(&output));
        stdout(&output)
    };

    assert_eq!(drawn("7", "macos"), drawn("7", "macos"), "a seed repeats");
    assert_ne!(
        drawn("7", "macos"),
        drawn("8", "macos"),
        "another seed is another identity"
    );
    assert_ne!(
        drawn("7", "macos"),
        drawn("7", "windows"),
        "another system is another identity"
    );
}

#[test]
fn export_wants_one_source_and_says_so_with_status_2() {
    let dir = tempfile::tempdir().unwrap();
    let cases: [(&[&str], &str); 4] = [
        (&["fingerprint", "export"], "--preset NAME or --random"),
        (
            &["fingerprint", "export", "--preset", "windows", "--random"],
            "not both",
        ),
        (
            &[
                "fingerprint",
                "export",
                "--preset",
                "windows",
                "--seed",
                "3",
            ],
            "--random only",
        ),
        (
            &["fingerprint", "export", "--preset", "beos"],
            "windows, edge, macos",
        ),
    ];
    for (args, expected) in cases {
        let output = run(dir.path(), &[], args);
        assert_eq!(status(&output), 2, "{args:?}");
        assert!(
            stderr(&output).contains(expected),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(
            stdout(&output).is_empty(),
            "{args:?}: nothing is printed on a mistake"
        );
    }
}

#[test]
fn check_refuses_a_file_that_is_missing_or_is_not_a_fingerprint_with_status_2() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope.json");
    let output = run(dir.path(), &[], &["fingerprint", "check", path(&missing)]);
    assert_eq!(status(&output), 2);
    assert!(stderr(&output).contains("nope.json"), "{}", stderr(&output));

    let garbage = dir.path().join("garbage.json");
    std::fs::write(&garbage, "{ \"user_agent\": 7 }").unwrap();
    let output = run(dir.path(), &[], &["fingerprint", "check", path(&garbage)]);
    assert_eq!(status(&output), 2);
    assert!(!stderr(&output).is_empty());
}

#[test]
fn diff_exits_0_for_equal_files_and_1_for_files_that_differ() {
    let dir = tempfile::tempdir().unwrap();
    let windows = exported(dir.path(), "windows");
    let macos = exported(dir.path(), "macos");

    let same = run(
        dir.path(),
        &[],
        &["fingerprint", "diff", path(&windows), path(&windows)],
    );
    assert_eq!(status(&same), 0, "{}", stderr(&same));
    assert!(
        stdout(&same).starts_with("no difference"),
        "{}",
        stdout(&same)
    );

    let other = run(
        dir.path(),
        &[],
        &["fingerprint", "diff", path(&windows), path(&macos)],
    );
    assert_eq!(status(&other), 1, "{}", stderr(&other));
    assert!(
        stdout(&other).contains("setting(s) differ"),
        "{}",
        stdout(&other)
    );

    let missing = dir.path().join("nope.json");
    let broken = run(
        dir.path(),
        &[],
        &["fingerprint", "diff", path(&windows), path(&missing)],
    );
    assert_eq!(status(&broken), 2);
}

#[test]
fn the_help_lists_the_subcommands() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &[], &["fingerprint", "--help"]);

    assert_eq!(status(&output), 0, "{}", stderr(&output));
    let help = stdout(&output);
    for name in ["capture", "export", "check", "diff", "presets"] {
        assert!(help.contains(name), "{name} is not in:\n{help}");
    }
}

#[test]
fn a_fingerprint_file_that_cannot_be_read_stops_the_command_before_a_browser_starts() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope.json");

    // The flag, the variable and the settings file say the same thing: a wrong path is an
    // error where the settings are read, not a browser without the fingerprint.
    let flag = run(
        dir.path(),
        &[],
        &["--fingerprint", path(&missing), "get-title"],
    );
    assert_eq!(status(&flag), 2, "{}", stderr(&flag));
    assert!(stderr(&flag).contains("nope.json"), "{}", stderr(&flag));

    let variable = run(
        dir.path(),
        &[("SB_FINGERPRINT", path(&missing))],
        &["get-title"],
    );
    assert_eq!(status(&variable), 2, "{}", stderr(&variable));
    assert!(
        stderr(&variable).contains("nope.json"),
        "{}",
        stderr(&variable)
    );

    let settings = dir.path().join("sbase_config.toml");
    std::fs::write(
        &settings,
        format!("[fingerprint]\nfile = {:?}\n", path(&missing)),
    )
    .unwrap();
    let table = run(dir.path(), &[], &["get-title"]);
    assert_eq!(status(&table), 2, "{}", stderr(&table));
    assert!(stderr(&table).contains("nope.json"), "{}", stderr(&table));
}

#[test]
fn the_flag_is_global_so_it_may_follow_the_command_too() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope.json");

    let after = run(
        dir.path(),
        &[],
        &["get-title", "--fingerprint", path(&missing)],
    );

    // Read as the flag it is, not refused as an argument the command does not know.
    assert_eq!(status(&after), 2);
    assert!(stderr(&after).contains("nope.json"), "{}", stderr(&after));
    assert!(!stderr(&after).contains("unexpected"), "{}", stderr(&after));
}
