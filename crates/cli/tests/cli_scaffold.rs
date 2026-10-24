//! `sbrs mkdir` and `sbrs mkfile`, run as the real binary in a scratch folder.
//!
//! The generated files are compared with the golden files in the library's
//! `tests/golden/cli/`, which the library compiles into the templates. After an
//! intended template change, regenerate them with
//! `UPDATE_GOLDEN=1 cargo test -p seleniumbase-rs-cli --test cli_scaffold` and review the diff. The Rust
//! goldens are also compiled against the current API by the `cfg(doctest)` items
//! in `src/cli/scripts/sb_mkfile.rs` and `sb_mkdir.rs`
//! (`cargo test -p seleniumbase-rs --doc --features "testing bdd"`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// Runs `sbrs` with `dir` as the current directory.
fn sbrs(dir: &Path, args: &[&str]) -> Output {
    Command::new(SBASE)
        .args(args)
        .current_dir(dir)
        .output()
        .expect("sbrs starts")
}

/// `path` with the platform's separator, the way `sbrs` prints it
/// (`ui\helpers.rs` on Windows).
fn native(path: &str) -> String {
    path.replace('/', std::path::MAIN_SEPARATOR_STR)
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn golden_path(name: &str) -> PathBuf {
    // The golden files stay with the library, at the repository root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
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
    let output = sbrs(dir, args);
    assert!(
        output.status.success(),
        "sbrs {args:?} failed: {}",
        stderr(&output)
    );
    stdout(&output)
}

/// Runs `sbrs mkfile stem ARGS` in a new folder and returns what it wrote to `stem.rs`.
fn mkfile(stem: &str, extra: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let mut args = vec!["mkfile", stem];
    args.extend_from_slice(extra);
    let printed = succeeds(dir.path(), &args);
    assert_eq!(printed.trim(), format!("Created {stem}.rs"), "{args:?}");
    read(dir.path(), &format!("{stem}.rs"))
}

/// Every golden test file: the name used for the file and the test, the flags, and the
/// golden. The names are the modules of the compile check in `sb_mkfile.rs`.
const MKFILE_GOLDENS: &[(&str, &[&str])] = &[
    ("pf", &[]),
    ("pf_basic", &["--basic"]),
    ("pf_url", &["--url", "https://example.org/app"]),
    ("pf_rec", &["--rec"]),
    ("pf_zh", &["--zh"]),
    ("bc", &["--bc"]),
    ("bc_rec", &["--bc", "-r"]),
    ("cm", &["--cm"]),
    ("cm_uc", &["--uc"]),
    ("dc", &["--dc"]),
    ("dm", &["--dm"]),
    ("dm_zh", &["--driver-manager", "--chinese"]),
];

#[test]
fn mkfile_writes_the_golden_files() {
    for (stem, flags) in MKFILE_GOLDENS {
        let source = mkfile(stem, flags);
        assert_golden(&source, &format!("mkfile/{stem}.rs.golden"));
        assert_parses(&source, stem);
    }
}

#[test]
fn the_default_test_is_the_rstest_sb_fixture() {
    let source = mkfile("example", &[]);
    assert_eq!(source, mkfile("example", &["--pf"]));
    assert!(source.contains("#[rstest]"), "{source}");
    assert!(source.contains("async fn example(sb: Sb)"), "{source}");
    assert!(source.contains("sb.run(async |sb| {"), "{source}");
}

#[test]
fn each_long_name_writes_what_the_short_one_does() {
    for (short, long) in [
        ("--pf", "--pytest-fixture"),
        ("--bc", "--basecase"),
        ("--cm", "--context-manager"),
        ("--dc", "--driver-context"),
        ("--dm", "--driver-manager"),
        ("--zh", "--chinese"),
        ("--zh", "--Chinese"),
        ("--rec", "--record"),
        ("--rec", "--recorder"),
        ("-r", "--rec"),
        ("-b", "--basic"),
    ] {
        assert_eq!(
            mkfile("same", &[short]),
            mkfile("same", &[long]),
            "{short} and {long}"
        );
    }
}

#[test]
fn every_language_flag_localises_the_strings() {
    for (code, name, hello, goodbye) in [
        ("en", "english", "Hello", "Goodbye"),
        ("zh", "chinese", "你好", "再见"),
        ("nl", "dutch", "Hallo", "Dag"),
        ("fr", "french", "Bonjour", "Au revoir"),
        ("it", "italian", "Ciao", "Addio"),
        ("ja", "japanese", "こんにちは", "さようなら"),
        ("ko", "korean", "여보세요", "안녕"),
        ("pt", "portuguese", "Olá", "Tchau"),
        ("ru", "russian", "Привет", "До свидания"),
        ("es", "spanish", "Hola", "Adiós"),
    ] {
        let by_code = mkfile("lang", &[&format!("--{code}")]);
        assert!(
            by_code.contains(&format!("sb.assert_text({hello:?}, \"p\")")),
            "--{code}: {by_code}"
        );
        assert!(
            by_code.contains(&format!("sb.type_text(\"input\", {goodbye:?})")),
            "--{code}: {by_code}"
        );
        assert!(by_code.contains("async fn lang(sb: Sb)"), "{by_code}");
        assert_parses(&by_code, code);
        assert_eq!(by_code, mkfile("lang", &[&format!("--{name}")]), "--{name}");
        let mut title = name.to_owned();
        title[..1].make_ascii_uppercase();
        assert_eq!(
            by_code,
            mkfile("lang", &[&format!("--{title}")]),
            "--{title}"
        );
    }
}

#[test]
fn a_url_takes_a_full_address_or_a_bare_host() {
    let full = mkfile("site", &["--url", "https://example.org/app"]);
    assert!(
        full.contains("sb.open(\"https://example.org/app\")"),
        "{full}"
    );
    assert!(full.contains("// Add your steps here."), "{full}");

    let bare = mkfile("site", &["--url", "wikipedia.org"]);
    assert!(
        bare.contains("sb.open(\"https://wikipedia.org/\")"),
        "{bare}"
    );
}

#[test]
fn mkfile_creates_missing_folders_and_new_is_an_alias() {
    let dir = tempfile::tempdir().unwrap();

    let printed = succeeds(dir.path(), &["new", "tests/deep/Login-Flow.rs"]);

    assert_eq!(
        printed.trim(),
        format!("Created {}", native("tests/deep/Login-Flow.rs"))
    );
    let source = read(dir.path(), "tests/deep/Login-Flow.rs");
    assert!(source.contains("async fn login_flow("), "{source}");
    assert!(source.contains("cargo test --test Login-Flow"), "{source}");
}

#[test]
fn mkfile_rejects_a_bad_url_and_a_wrong_extension() {
    let dir = tempfile::tempdir().unwrap();

    let output = sbrs(dir.path(), &["mkfile", "a", "--url", "javascript:alert(1)"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("scheme"), "{}", stderr(&output));

    let output = sbrs(dir.path(), &["mkfile", "a.py"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains(".rs"), "{}", stderr(&output));

    assert!(
        fs::read_dir(dir.path()).unwrap().next().is_none(),
        "nothing may be created on failure"
    );
}

#[test]
fn class_fixtures_are_refused_and_the_message_names_the_way_out() {
    for flag in ["--cf", "--class-fixture"] {
        let dir = tempfile::tempdir().unwrap();

        let output = sbrs(dir.path(), &["mkfile", "t", flag]);

        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(
            stderr(&output)
                .contains("error: class fixtures are not ported; use --pf (the rstest sb fixture)"),
            "{}",
            stderr(&output)
        );
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    }
}

#[test]
fn flags_that_cannot_go_together_exit_with_status_2_and_write_nothing() {
    for flags in [
        // Two members of one group: the parser refuses them.
        &["--bc", "--cm"][..],
        &["--zh", "--fr"],
        // Accepted by the parser, refused by the generator.
        &["--uc", "--zh"],
        &["--uc", "--dm"],
        &["--cdp", "--bc"],
        &["--uc", "--cdp"],
        &["--uc", "--rec"],
        &["--rec", "--dm"],
        &["--rec", "--driver-context"],
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut args = vec!["mkfile", "t"];
        args.extend_from_slice(flags);

        let output = sbrs(dir.path(), &args);

        assert_eq!(
            output.status.code(),
            Some(2),
            "{flags:?}: {}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("error:"),
            "{flags:?}: {}",
            stderr(&output)
        );
        assert!(
            fs::read_dir(dir.path()).unwrap().next().is_none(),
            "{flags:?} must write nothing"
        );
    }
}

#[test]
fn mkdir_writes_the_suite() {
    let dir = tempfile::tempdir().unwrap();

    let printed = succeeds(dir.path(), &["mkdir", "ui"]);

    for file in [
        "README.md",
        "main.rs",
        "my_first_test.rs",
        "parameterized_test.rs",
        "cdp_test.rs",
    ] {
        assert!(
            printed.contains(&format!("Created {}", native(&format!("ui/{file}")))),
            "{printed}"
        );
        let contents = read(dir.path(), &format!("ui/{file}"));
        assert_golden(&contents, &format!("mkdir/{file}.golden"));
        if file.ends_with(".rs") {
            assert_parses(&contents, file);
        }
    }
    assert!(
        !dir.path().join("ui/helpers.rs").exists(),
        "the tests need no helper: the fixtures read the settings"
    );
    assert!(!dir.path().join(".github").exists());
}

#[test]
fn mkdir_basic_writes_only_the_scaffolding() {
    let dir = tempfile::tempdir().unwrap();

    succeeds(dir.path(), &["mkdir", "ui", "--basic"]);

    for file in ["README.md", "main.rs"] {
        let contents = read(dir.path(), &format!("ui/{file}"));
        assert_golden(&contents, &format!("mkdir_basic/{file}.golden"));
    }
    assert!(!dir.path().join("ui/my_first_test.rs").exists());
}

#[test]
fn mkdir_gha_writes_the_workflow_in_the_current_folder() {
    let dir = tempfile::tempdir().unwrap();

    let printed = succeeds(dir.path(), &["mkdir", "tests/ui", "--gha"]);

    assert!(
        printed.contains(&format!(
            "Created {}",
            native(".github/workflows/sbrs-tests.yml")
        )),
        "{printed}"
    );
    let workflow = read(dir.path(), ".github/workflows/sbrs-tests.yml");
    assert_golden(&workflow, "mkdir_gha/sbrs-tests.yml.golden");
    assert_golden(
        &read(dir.path(), "tests/ui/README.md"),
        "mkdir_gha/README.md.golden",
    );
    assert!(
        !dir.path().join("tests/ui/.github").exists(),
        "the workflow is not inside the suite's folder"
    );
}

#[test]
fn an_existing_workflow_is_left_untouched_and_nothing_else_is_written() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
    fs::write(
        dir.path().join(".github/workflows/sbrs-tests.yml"),
        "# mine\n",
    )
    .unwrap();

    let output = sbrs(dir.path(), &["mkdir", "ui", "--gha"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("--force"), "{}", stderr(&output));
    assert_eq!(
        read(dir.path(), ".github/workflows/sbrs-tests.yml"),
        "# mine\n"
    );
    assert!(!dir.path().join("ui").exists());
}

#[test]
fn mkdir_bdd_writes_a_scenario_on_the_built_in_steps() {
    let dir = tempfile::tempdir().unwrap();
    // The name is where the golden files live in the library, so that the scenario's
    // feature path is a real file there and the compile check can expand `#[scenario]`.
    let name = "tests/golden/cli/mkdir_bdd";

    let printed = succeeds(dir.path(), &["mkdir", name, "--basic", "--bdd"]);

    for file in [
        "README.md",
        "main.rs",
        "bdd_test.rs",
        "features/example.feature",
    ] {
        assert!(
            printed.contains(&format!("Created {}", native(&format!("{name}/{file}")))),
            "{printed}"
        );
        let contents = read(dir.path(), &format!("{name}/{file}"));
        let golden = if file.ends_with(".feature") {
            // Stored as the feature file itself: the scenario test names this path.
            format!("mkdir_bdd/{file}")
        } else {
            format!("mkdir_bdd/{file}.golden")
        };
        assert_golden(&contents, &golden);
        if file.ends_with(".rs") {
            assert_parses(&contents, file);
        }
    }
}

#[test]
fn nothing_is_overwritten_without_force() {
    let dir = tempfile::tempdir().unwrap();
    succeeds(dir.path(), &["mkfile", "keep"]);
    fs::write(dir.path().join("keep.rs"), "// my edits\n").unwrap();

    let output = sbrs(dir.path(), &["mkfile", "keep"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--force"),
        "the error says how to overwrite: {}",
        stderr(&output)
    );
    assert_eq!(read(dir.path(), "keep.rs"), "// my edits\n");

    succeeds(dir.path(), &["mkfile", "keep", "--force"]);
    assert!(read(dir.path(), "keep.rs").contains("async fn keep("));
}

#[test]
fn a_suite_with_one_existing_file_is_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("ui")).unwrap();
    fs::write(dir.path().join("ui/cdp_test.rs"), "// mine\n").unwrap();

    let output = sbrs(dir.path(), &["mkdir", "ui"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains(&native("ui/cdp_test.rs")),
        "{}",
        stderr(&output)
    );
    assert_eq!(read(dir.path(), "ui/cdp_test.rs"), "// mine\n");
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
        let output = sbrs(&work, &["mkfile", name]);
        assert!(!output.status.success(), "mkfile {name:?} must fail");
        assert!(
            stderr(&output).starts_with("sbrs: invalid name"),
            "mkfile {name:?}: {}",
            stderr(&output)
        );
    }
    for name in ["../outside/pwn", "..", absolute_dir.as_str(), "a/../../x"] {
        let output = sbrs(&work, &["mkdir", name]);
        assert!(!output.status.success(), "mkdir {name:?} must fail");
        let output = sbrs(&work, &["mkdir", name, "--gha"]);
        assert!(!output.status.success(), "mkdir {name:?} --gha must fail");
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

    let output = sbrs(&work, &["mkfile", "link/pwn.rs"]);

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("outside the target directory"),
        "{}",
        stderr(&output)
    );
    assert!(fs::read_dir(&outside).unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn a_linked_github_folder_is_not_followed() {
    let scratch = tempfile::tempdir().unwrap();
    let work = scratch.path().join("work");
    let outside = scratch.path().join("outside");
    fs::create_dir(&work).unwrap();
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, work.join(".github")).unwrap();

    let output = sbrs(&work, &["mkdir", "ui", "--gha"]);

    assert!(!output.status.success());
    assert!(fs::read_dir(&outside).unwrap().next().is_none());
    assert!(!work.join("ui").exists(), "nothing else may be written");
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
    for flag in ["--basic", "--force", "--gha", "--bdd", "tests/ui"] {
        assert!(mkdir.contains(flag), "{flag}: {mkdir}");
    }

    let mkfile = succeeds(dir.path(), &["mkfile", "--help"]);
    for flag in [
        "--url", "--rec", "--uc", "--cdp", "--pf", "--bc", "--cm", "--dc", "--dm", "--en", "--zh",
        "--es",
    ] {
        assert!(mkfile.contains(flag), "{flag}: {mkfile}");
    }
    for hidden in ["--Chinese", "--chinese", "--basecase", "--pytest-fixture"] {
        assert!(
            !mkfile.contains(hidden),
            "{hidden} is a hidden spelling: {mkfile}"
        );
    }

    let top = succeeds(dir.path(), &["--help"]);
    let mkfile_line = top
        .lines()
        .find(|line| line.trim_start().starts_with("mkfile"))
        .expect("mkfile is listed");
    assert!(mkfile_line.contains("[aliases: new]"), "{mkfile_line}");
    assert!(top
        .lines()
        .any(|line| line.trim_start().starts_with("mkdir")));
}
