//! `sbase mkdir` and `sbase mkfile`, run as the real binary in a scratch folder.
//!
//! The generated files are compared with the golden files in
//! `tests/golden/cli/`. After an intended template change, regenerate them with
//! `UPDATE_GOLDEN=1 cargo test --test cli_scaffold` and review the diff. The Rust
//! goldens are also compiled against the current API by the `cfg(doctest)` items
//! in `src/cli/scripts/sb_mkfile.rs` and `sb_mkdir.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

/// Runs `sbase` with `dir` as the current directory.
fn sbase(dir: &Path, args: &[&str]) -> Output {
    Command::new(SBASE)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("sbase starts")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("cli")
        .join(name)
}

/// Compares `actual` with the golden file `name`, or rewrites the golden file
/// when `UPDATE_GOLDEN` is set.
fn assert_golden(actual: &str, name: &str) {
    let path = golden_path(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().expect("golden folder")).expect("create golden folder");
        fs::write(&path, actual).expect("write golden file");
        return;
    }
    let expected = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; run with UPDATE_GOLDEN=1 to create it",
            path.display()
        )
    });
    assert_eq!(
        actual, expected,
        "{name} differs from its golden file; rerun with UPDATE_GOLDEN=1 if the change is intended"
    );
}

fn read(dir: &Path, relative: &str) -> String {
    fs::read_to_string(dir.join(relative)).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

fn assert_parses(source: &str, what: &str) {
    if let Err(error) = syn::parse_file(source) {
        panic!("{what} is not valid Rust: {error}\n{source}");
    }
}

/// Runs a command that must succeed and returns its standard output.
fn succeeds(dir: &Path, args: &[&str]) -> String {
    let output = sbase(dir, args);
    assert!(
        output.status.success(),
        "sbase {args:?} failed: {}",
        stderr(&output)
    );
    stdout(&output)
}

#[test]
fn mkfile_writes_the_example_test() {
    let dir = tempfile::tempdir().unwrap();

    let printed = succeeds(dir.path(), &["mkfile", "example"]);

    assert_eq!(printed.trim(), "Created example.rs");
    let source = read(dir.path(), "example.rs");
    assert_golden(&source, "mkfile_example.rs.golden");
    assert_parses(&source, "the example test");
}

#[test]
fn mkfile_basic_leaves_the_body_empty() {
    let dir = tempfile::tempdir().unwrap();

    succeeds(dir.path(), &["mkfile", "basic_demo", "--basic"]);

    let source = read(dir.path(), "basic_demo.rs");
    assert_golden(&source, "mkfile_basic.rs.golden");
    assert_parses(&source, "the basic test");
}

#[test]
fn mkfile_url_opens_the_given_page() {
    let dir = tempfile::tempdir().unwrap();

    succeeds(
        dir.path(),
        &["mkfile", "url_demo", "--url", "https://example.org/app"],
    );

    let source = read(dir.path(), "url_demo.rs");
    assert_golden(&source, "mkfile_url.rs.golden");
    assert_parses(&source, "the test with a URL");
}

#[test]
fn mkfile_creates_missing_folders_and_new_is_an_alias() {
    let dir = tempfile::tempdir().unwrap();

    let printed = succeeds(dir.path(), &["new", "tests/deep/Login-Flow.rs"]);

    assert_eq!(printed.trim(), "Created tests/deep/Login-Flow.rs");
    let source = read(dir.path(), "tests/deep/Login-Flow.rs");
    assert!(source.contains("async fn login_flow()"), "{source}");
    assert!(source.contains("cargo test --test Login-Flow"), "{source}");
}

#[test]
fn mkfile_rejects_a_bad_url_and_a_wrong_extension() {
    let dir = tempfile::tempdir().unwrap();

    let output = sbase(dir.path(), &["mkfile", "a", "--url", "javascript:alert(1)"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("scheme"), "{}", stderr(&output));

    let output = sbase(dir.path(), &["mkfile", "a.py"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains(".rs"), "{}", stderr(&output));

    assert!(
        fs::read_dir(dir.path()).unwrap().next().is_none(),
        "nothing may be created on failure"
    );
}

#[test]
fn mkdir_writes_the_suite() {
    let dir = tempfile::tempdir().unwrap();

    let printed = succeeds(dir.path(), &["mkdir", "ui"]);

    for file in [
        "README.md",
        "main.rs",
        "helpers.rs",
        "my_first_test.rs",
        "parameterized_test.rs",
    ] {
        assert!(printed.contains(&format!("Created ui/{file}")), "{printed}");
        let contents = read(dir.path(), &format!("ui/{file}"));
        assert_golden(&contents, &format!("mkdir/{file}.golden"));
        if file.ends_with(".rs") {
            assert_parses(&contents, file);
        }
    }
}

#[test]
fn mkdir_basic_writes_only_the_scaffolding() {
    let dir = tempfile::tempdir().unwrap();

    succeeds(dir.path(), &["mkdir", "ui", "--basic"]);

    for file in ["README.md", "main.rs", "helpers.rs"] {
        let contents = read(dir.path(), &format!("ui/{file}"));
        assert_golden(&contents, &format!("mkdir_basic/{file}.golden"));
        if file.ends_with(".rs") {
            assert_parses(&contents, file);
        }
    }
    assert!(!dir.path().join("ui/my_first_test.rs").exists());
}

#[test]
fn nothing_is_overwritten_without_force() {
    let dir = tempfile::tempdir().unwrap();
    succeeds(dir.path(), &["mkfile", "keep"]);
    fs::write(dir.path().join("keep.rs"), "// my edits\n").unwrap();

    let output = sbase(dir.path(), &["mkfile", "keep"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--force"),
        "the error says how to overwrite: {}",
        stderr(&output)
    );
    assert_eq!(read(dir.path(), "keep.rs"), "// my edits\n");

    succeeds(dir.path(), &["mkfile", "keep", "--force"]);
    assert!(read(dir.path(), "keep.rs").contains("async fn keep()"));
}

#[test]
fn a_suite_with_one_existing_file_is_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("ui")).unwrap();
    fs::write(dir.path().join("ui/helpers.rs"), "// mine\n").unwrap();

    let output = sbase(dir.path(), &["mkdir", "ui"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("ui/helpers.rs"),
        "{}",
        stderr(&output)
    );
    assert_eq!(read(dir.path(), "ui/helpers.rs"), "// mine\n");
    assert!(
        !dir.path().join("ui/main.rs").exists(),
        "nothing else may be written"
    );
}

#[test]
fn names_cannot_leave_the_current_directory() {
    let scratch = tempfile::tempdir().unwrap();
    let work = scratch.path().join("work");
    let outside = scratch.path().join("outside");
    fs::create_dir(&work).unwrap();
    fs::create_dir(&outside).unwrap();

    let absolute = outside.join("abs.rs").display().to_string();
    let absolute_dir = outside.join("absdir").display().to_string();
    for name in [
        "../outside/pwn.rs",
        "a/../../outside/pwn.rs",
        "..",
        absolute.as_str(),
        "~/x.rs",
        "C:\\x.rs",
        "a\\b.rs",
        "tests/../../x.rs",
    ] {
        let output = sbase(&work, &["mkfile", name]);
        assert!(!output.status.success(), "mkfile {name:?} must fail");
        assert!(
            stderr(&output).starts_with("sbase: invalid name"),
            "mkfile {name:?}: {}",
            stderr(&output)
        );
    }
    for name in ["../outside/pwn", "..", absolute_dir.as_str(), "a/../../x"] {
        let output = sbase(&work, &["mkdir", name]);
        assert!(!output.status.success(), "mkdir {name:?} must fail");
    }

    assert!(
        fs::read_dir(&outside).unwrap().next().is_none(),
        "nothing may be created outside the working folder"
    );
    assert!(
        fs::read_dir(&work).unwrap().next().is_none(),
        "nothing may be created inside it either"
    );
}

#[cfg(unix)]
#[test]
fn a_link_to_another_folder_is_not_followed() {
    let scratch = tempfile::tempdir().unwrap();
    let work = scratch.path().join("work");
    let outside = scratch.path().join("outside");
    fs::create_dir(&work).unwrap();
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, work.join("link")).unwrap();

    let output = sbase(&work, &["mkfile", "link/pwn.rs"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("outside the target directory"),
        "{}",
        stderr(&output)
    );
    assert!(fs::read_dir(&outside).unwrap().next().is_none());
}

#[test]
fn the_commands_do_not_need_a_readable_settings_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("sbase_config.toml"), "this is = not [toml").unwrap();

    succeeds(dir.path(), &["mkfile", "fine"]);
    succeeds(dir.path(), &["mkdir", "suite"]);
}

#[test]
fn help_describes_the_new_arguments() {
    let dir = tempfile::tempdir().unwrap();

    let mkdir = succeeds(dir.path(), &["mkdir", "--help"]);
    assert!(mkdir.contains("--basic"), "{mkdir}");
    assert!(mkdir.contains("--force"), "{mkdir}");
    assert!(mkdir.contains("tests/ui"), "{mkdir}");

    let mkfile = succeeds(dir.path(), &["mkfile", "--help"]);
    assert!(mkfile.contains("--url"), "{mkfile}");

    let top = succeeds(dir.path(), &["--help"]);
    let mkfile_line = top
        .lines()
        .find(|line| line.trim_start().starts_with("mkfile"))
        .expect("mkfile is listed");
    assert!(mkfile_line.contains("[alias: new]"), "{mkfile_line}");
    assert!(top
        .lines()
        .any(|line| line.trim_start().starts_with("mkdir")));
}
