//! A file name a caller passes must not be able to leave the folder it is
//! meant for.
//!
//! `BaseCase` helpers such as `save_data_as` or `delete_downloaded_file` take a
//! name from the caller. Joining it straight onto a directory lets `../../x` or
//! an absolute path reach any file. They must go through
//! `artifacts::confined_path`, which refuses such names.

use std::fs;
use std::path::Path;

use seleniumbase_rs::artifacts::confined_path;

fn rust_files(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).expect("source directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

/// Lines that join a bare `filename` straight onto a directory.
fn unconfined_joins(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let line = line.trim_start();
            !line.starts_with("//")
                && (line.contains(".join(filename)") || line.contains(".join(file_name)"))
        })
        .map(|(index, line)| (index + 1, line.trim().to_owned()))
        .collect()
}

#[test]
fn the_check_finds_the_pattern_it_looks_for() {
    let buggy = "let dir = logs();\nlet path = dir.join(filename);\n";
    assert_eq!(unconfined_joins(buggy).len(), 1);
    let fixed = "let path = confined_path(&dir, filename)?;\n// dir.join(filename) is unsafe\n";
    assert!(unconfined_joins(fixed).is_empty());
}

#[test]
fn api_helpers_never_join_a_caller_name_onto_a_directory() {
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/api"),
        &mut files,
    );
    assert!(files.len() > 20, "found only {} files", files.len());

    let offenders: Vec<String> = files
        .iter()
        .flat_map(|path| {
            let text = fs::read_to_string(path).expect("source file is readable");
            unconfined_joins(&text)
                .into_iter()
                .map(|(number, line)| format!("{}:{number}: {line}", path.display()))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "use artifacts::confined_path for names a caller supplies:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn a_name_that_climbs_out_of_the_folder_is_refused() {
    let dir = Path::new("latest_logs");
    for bad in [
        "../../etc/passwd",
        "/etc/passwd",
        "a/b",
        "..",
        "",
        "x/../../y",
    ] {
        assert!(confined_path(dir, bad).is_err(), "accepted {bad:?}");
    }
    assert_eq!(
        confined_path(dir, "cookies.json").unwrap(),
        dir.join("cookies.json")
    );
}
