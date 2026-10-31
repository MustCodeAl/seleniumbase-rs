//! The `sbrs` commands that write starter files (`mkchart`, `mkpres`,
//! `caseplans --starter`, `behave-gui --new`), `sbrs print` and `sbrs convert`,
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
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

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
        .expect("sbrs runs")
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
        if child.try_wait().expect("poll sbrs").is_some() {
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
    let generators: [(&[&str], &str, &str); 1] =
        [(&["caseplans", "--starter"], "CASE_PLANS.md", "Generated")];
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
fn mkpres_and_mkchart_create_the_folders_of_the_path_they_are_given() {
    let dir = tempfile::tempdir().expect("temp dir");

    let chart = run(dir.path(), &["mkchart", "reports/2026/chart"]);
    let deck = run(dir.path(), &["mkpres", "reports/2026/deck.rs"]);

    for (output, file) in [(chart, "chart.rs"), (deck, "deck.rs")] {
        let created = format!("Created {}", native(&format!("reports/2026/{file}")));
        assert!(stdout(&output).contains(&created), "{}", stderr(&output));
        assert!(dir.path().join("reports/2026").join(file).is_file());
    }
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
        .expect("sbrs starts");
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
        .expect("sbrs starts");
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
        .expect("sbrs starts");
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

    let output = run(dir.path(), &["print", "missing.txt"]);

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

    let output = run(dir.path(), &["print", "latin1.txt"]);

    assert!(
        stdout(&output).contains("caf\u{fffd} au lait"),
        "{}",
        stderr(&output)
    );
}

/// `path` with the platform's separator, the way `sbrs` prints it.
fn native(path: &str) -> String {
    path.replace('/', std::path::MAIN_SEPARATOR_STR)
}

/// Runs a command that must succeed and returns its standard output.
fn succeeds(dir: &Path, args: &[&str]) -> String {
    let output = run(dir, args);
    assert!(
        output.status.success(),
        "sbrs {args:?} failed: {}",
        stderr(&output)
    );
    stdout(&output)
}

fn read(dir: &Path, relative: &str) -> String {
    std::fs::read_to_string(dir.join(relative)).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

fn file_count(dir: &Path) -> usize {
    std::fs::read_dir(dir).expect("list the folder").count()
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

/// Compares `actual` with the golden file `name`, or rewrites the golden file when
/// `UPDATE_GOLDEN` is set. The library compiles the golden files against the current API
/// (`cargo test -p seleniumbase-rs --doc --features testing`).
fn assert_golden(actual: &str, name: &str) {
    let path = golden_path(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().expect("golden folder")).expect("golden folder");
        std::fs::write(&path, actual).expect("write the golden file");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
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

/// A generated test imports from `seleniumbase_rs` and from nothing else (see "Imports and
/// dependencies" in docs/tutorials/getting_started.md).
fn assert_own_imports(source: &str, what: &str) {
    for line in source.lines().filter(|line| line.starts_with("use ")) {
        assert!(
            line.starts_with("use seleniumbase_rs::"),
            "{what}: {line:?} imports another crate:\n{source}"
        );
    }
}

fn assert_parses(source: &str, what: &str) {
    if let Err(error) = syn::parse_file(source) {
        panic!("{what} is not valid Rust: {error}\n{source}");
    }
}

/// The generated presentation tests: the file name, the flags, and the golden file
/// (`tests/golden/cli/mkpres/<name>.rs.golden`, compiled by `sb_mkpres.rs`).
const MKPRES_GOLDENS: &[(&str, &[&str])] = &[
    ("deck", &[]),
    ("deck_zh", &["--zh"]),
    ("deck_ru", &["--Russian"]),
];

/// The same for `mkchart` (`tests/golden/cli/mkchart/<name>.rs.golden`).
const MKCHART_GOLDENS: &[(&str, &[&str])] = &[
    ("chart", &[]),
    ("chart_ja", &["--ja"]),
    ("chart_fr", &["--french"]),
];

#[test]
fn mkpres_and_mkchart_write_the_goldens_the_library_compiles() {
    for (command, goldens) in [("mkpres", MKPRES_GOLDENS), ("mkchart", MKCHART_GOLDENS)] {
        for (stem, flags) in goldens {
            let dir = tempfile::tempdir().expect("temp dir");
            let mut args = vec![command, stem];
            args.extend_from_slice(flags);

            let printed = succeeds(dir.path(), &args);

            assert_eq!(printed.trim(), format!("Created {stem}.rs"), "{args:?}");
            let source = read(dir.path(), &format!("{stem}.rs"));
            assert_parses(&source, &format!("{args:?}"));
            assert_own_imports(&source, &format!("{args:?}"));
            assert_golden(&source, &format!("{command}/{stem}.rs.golden"));
        }
    }
}

#[test]
fn every_spelling_of_a_language_flag_writes_the_same_file() {
    for command in ["mkpres", "mkchart"] {
        let write = |flags: &[&str]| {
            let dir = tempfile::tempdir().expect("temp dir");
            let mut args = vec![command, "t"];
            args.extend_from_slice(flags);
            succeeds(dir.path(), &args);
            read(dir.path(), "t.rs")
        };
        let spanish = write(&["--es"]);
        assert_eq!(write(&["--spanish"]), spanish, "{command}");
        assert_eq!(write(&["--Spanish"]), spanish, "{command}");
        assert_ne!(write(&["--fr"]), spanish, "{command}");
        // English is the default.
        assert_eq!(write(&["--en"]), write(&[]), "{command}");
        assert_ne!(write(&[]), spanish, "{command}");
    }
}

#[test]
fn only_one_language_flag_may_be_given_to_mkpres_and_mkchart() {
    for command in ["mkpres", "mkchart"] {
        let dir = tempfile::tempdir().expect("temp dir");

        let output = run(dir.path(), &[command, "t", "--es", "--fr"]);

        assert_eq!(
            output.status.code(),
            Some(2),
            "{command}: {}",
            stderr(&output)
        );
        assert_eq!(file_count(dir.path()), 0, "{command} wrote a file");
    }
}

#[test]
fn a_name_is_made_into_a_rust_file_and_a_test_function() {
    let dir = tempfile::tempdir().expect("temp dir");

    succeeds(dir.path(), &["mkpres", "my-deck"]);
    succeeds(dir.path(), &["mkchart", "q3-results.rs"]);

    let deck = read(dir.path(), "my-deck.rs");
    assert!(deck.contains("async fn my_deck(sb: Sb)"), "{deck}");
    assert!(deck.contains("saved_presentations/my-deck.html"), "{deck}");
    assert!(deck.contains("filename(\"my-deck.html\")"), "{deck}");
    let chart = read(dir.path(), "q3-results.rs");
    assert!(chart.contains("async fn q3_results(sb: Sb)"), "{chart}");
    assert!(chart.contains("filename(\"q3-results.html\")"), "{chart}");
}

#[test]
fn mkpres_and_mkchart_replace_a_file_only_with_force() {
    for command in ["mkpres", "mkchart"] {
        let dir = tempfile::tempdir().expect("temp dir");
        succeeds(dir.path(), &[command, "t"]);
        std::fs::write(dir.path().join("t.rs"), "my own work").expect("edit the file");

        let again = run(dir.path(), &[command, "t"]);

        assert_eq!(
            again.status.code(),
            Some(1),
            "{command}: {}",
            stderr(&again)
        );
        assert!(
            stderr(&again).contains("already exists"),
            "{command}: {}",
            stderr(&again)
        );
        assert_eq!(read(dir.path(), "t.rs"), "my own work", "{command}");

        let forced = succeeds(dir.path(), &[command, "t", "--force"]);
        assert_eq!(forced.trim(), "Created t.rs", "{command}");
        assert_ne!(read(dir.path(), "t.rs"), "my own work", "{command}");
    }
}

#[test]
fn a_name_outside_the_folder_or_not_a_rust_file_is_refused() {
    for command in ["mkpres", "mkchart"] {
        for name in ["../deck", "deck.py", "deck.html", "a b"] {
            let dir = tempfile::tempdir().expect("temp dir");

            let output = run(dir.path(), &[command, name]);

            assert_eq!(
                output.status.code(),
                Some(1),
                "{command} {name}: {}",
                stderr(&output)
            );
            assert!(
                stderr(&output).contains("sbrs:"),
                "{command} {name}: {}",
                stderr(&output)
            );
            assert_eq!(file_count(dir.path()), 0, "{command} {name} wrote a file");
        }
    }
}

#[test]
fn the_html_writers_are_gone() {
    for command in ["mkpres", "mkchart"] {
        let dir = tempfile::tempdir().expect("temp dir");

        let output = run(dir.path(), &[command, "--file", "deck.html"]);

        assert_eq!(
            output.status.code(),
            Some(2),
            "{command}: {}",
            stderr(&output)
        );
        assert_eq!(file_count(dir.path()), 0, "{command} wrote a file");
    }
}

#[test]
fn the_help_of_the_content_commands_lists_their_options_and_examples() {
    let dir = tempfile::tempdir().expect("temp dir");
    let expected: [(&str, &[&str]); 5] = [
        ("mkpres", &["--zh", "--es", "--force", "Examples"]),
        ("mkchart", &["--ja", "--fr", "--force", "Examples"]),
        ("print", &["--line-numbers", "Examples"]),
        ("convert", &["--force", "Examples", "_SB.rs"]),
        ("translate", &["--zh", "--copy", "Examples"]),
    ];
    for (command, words) in expected {
        let help = succeeds(dir.path(), &[command, "--help"]);
        for word in words {
            assert!(
                help.contains(word),
                "{command} --help lacks {word}:\n{help}"
            );
        }
    }
}

#[test]
fn print_shows_the_file_as_it_is() {
    let dir = tempfile::tempdir().expect("temp dir");
    let source = "fn main() {\n\tprintln!(\"héllo\");\n}\n";
    std::fs::write(dir.path().join("a.rs"), source).unwrap();
    std::fs::write(dir.path().join("bare"), "no newline at the end").unwrap();

    assert_eq!(succeeds(dir.path(), &["print", "a.rs"]), source);
    assert_eq!(
        succeeds(dir.path(), &["print", "bare"]),
        "no newline at the end"
    );
}

#[test]
fn n_puts_the_line_number_in_front_of_every_line() {
    let dir = tempfile::tempdir().expect("temp dir");
    let text: String = (1..=12).map(|n| format!("line {n}\n")).collect();
    std::fs::write(dir.path().join("a.txt"), &text).unwrap();
    // Right-aligned to the width of the last number.
    let expected: String = (1..=12).map(|n| format!("{n:>2} line {n}\n")).collect();

    for args in [
        &["print", "a.txt", "-n"][..],
        &["print", "a.txt", "--line-numbers"][..],
        &["print", "-n", "a.txt"][..],
    ] {
        assert_eq!(succeeds(dir.path(), args), expected, "{args:?}");
    }
    assert_eq!(succeeds(dir.path(), &["print", "a.txt"]), text);
}

#[test]
fn print_takes_the_file_as_an_argument_and_not_as_an_option() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("a.txt"), "x\n").unwrap();

    for args in [&["print", "--file", "a.txt"][..], &["print"][..]] {
        let output = run(dir.path(), args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
        assert!(stdout(&output).is_empty(), "{args:?}: {}", stdout(&output));
    }
}

/// A WebDriver export in the shape Selenium IDE writes.
const EXPORT: &str = "from selenium.webdriver.common.by import By\n\n\
driver.get(\"https://example.test/login\")\n\
username = driver.find_element(By.ID, \"username\")\n\
username.send_keys(\"alice\")\n\
driver.find_element(By.CSS_SELECTOR, \"button\").click()\n\
driver.quit()\n";

#[test]
fn convert_writes_the_rust_test_next_to_the_export_and_keeps_the_export() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("login.py"), EXPORT).unwrap();

    let printed = succeeds(dir.path(), &["convert", "login.py"]);

    assert_eq!(printed.trim(), "Created login_SB.rs");
    let rust = read(dir.path(), "login_SB.rs");
    assert!(rust.contains("https://example.test/login"), "{rust}");
    assert!(rust.contains("sb.click(\"button\")"), "{rust}");
    assert!(rust.contains("async fn login(sb: Sb)"), "{rust}");
    assert!(!rust.contains("TODO"), "{rust}");
    assert_parses(&rust, "login_SB.rs");
    assert_own_imports(&rust, "login_SB.rs");
    assert_eq!(read(dir.path(), "login.py"), EXPORT);
    assert_eq!(file_count(dir.path()), 2);
}

#[test]
fn convert_writes_into_the_folder_of_the_export() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir(dir.path().join("exports")).unwrap();
    std::fs::write(dir.path().join("exports/checkout.py"), EXPORT).unwrap();

    let printed = succeeds(dir.path(), &["convert", "exports/checkout.py"]);

    assert_eq!(
        printed.trim(),
        format!("Created {}", native("exports/checkout_SB.rs"))
    );
    assert!(read(dir.path(), "exports/checkout_SB.rs").contains("async fn checkout(sb: Sb)"));
}

#[test]
fn convert_does_not_replace_an_output_unless_forced() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("login.py"), EXPORT).unwrap();
    std::fs::write(dir.path().join("login_SB.rs"), "my own work").unwrap();

    let refused = run(dir.path(), &["convert", "login.py"]);

    assert_eq!(refused.status.code(), Some(2), "{}", stderr(&refused));
    let message = stderr(&refused);
    assert!(
        message.contains("login_SB.rs") && message.contains("--force"),
        "{message}"
    );
    assert_eq!(read(dir.path(), "login_SB.rs"), "my own work");

    let forced = succeeds(dir.path(), &["convert", "login.py", "--force"]);
    assert_eq!(forced.trim(), "Created login_SB.rs");
    assert!(read(dir.path(), "login_SB.rs").contains("example.test"));
    assert_eq!(read(dir.path(), "login.py"), EXPORT);
}

#[test]
fn convert_refuses_a_file_that_is_not_python_and_one_that_is_not_there() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("notes.txt"), EXPORT).unwrap();

    let text = run(dir.path(), &["convert", "notes.txt"]);
    assert_eq!(text.status.code(), Some(2), "{}", stderr(&text));
    assert!(stderr(&text).contains("notes.txt"), "{}", stderr(&text));

    let missing = run(dir.path(), &["convert", "none.py"]);
    assert_eq!(
        missing.status.code(),
        Some(IO_FAILURE),
        "{}",
        stderr(&missing)
    );
    assert!(stderr(&missing).contains("none.py"), "{}", stderr(&missing));

    assert_eq!(file_count(dir.path()), 1, "an output was written");
}

#[test]
fn convert_leaves_what_it_cannot_convert_as_a_todo_and_says_so() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("odd.py"),
        "driver.get(\"https://a.test\")\ndriver.frobnicate(1)\n",
    )
    .unwrap();

    let output = run(dir.path(), &["convert", "odd.py"]);

    // A statement it knows nothing about is a warning: the file is complete enough to use.
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("line 2"), "{}", stderr(&output));
    assert!(read(dir.path(), "odd_SB.rs").contains("TODO"));
}

#[test]
fn convert_exits_1_when_a_statement_could_not_be_read_but_still_writes_the_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("dynamic.py"), "driver.get(base_url)\n").unwrap();

    let output = run(dir.path(), &["convert", "dynamic.py"]);

    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stdout(&output).contains("Created dynamic_SB.rs"));
    assert!(stderr(&output).contains("line 1"), "{}", stderr(&output));
    assert!(read(dir.path(), "dynamic_SB.rs").contains("TODO"));
}
