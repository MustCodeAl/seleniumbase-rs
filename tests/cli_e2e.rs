//! The whole `sbase` command line as a person meets it, run as the real binary.
//!
//! `cli_scaffold.rs`, `cli_generators.rs`, `cli_crypto.rs` and the other `cli_*` files
//! check one family of commands closely. This file checks across them: every
//! subcommand the program lists answers `--help`, and the commands that need no
//! browser run for real in a scratch folder (shell completions, `import-python`,
//! `patch-chromedriver`, `decrypt` with the wrong passphrase, `mkfile` then `print`).
//! The commands that drive a browser are in `cli_e2e_chrome.rs`.

use std::path::Path;
use std::process::{Command, Output, Stdio};

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

/// A command in `dir`, with the host's settings variables removed so they cannot
/// leak into a run.
fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(SBASE);
    command
        .args(args)
        .current_dir(dir)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env_remove("SB_ENCRYPTION_KEY")
        .env("SB_LOG_LEVEL", "error")
        .stdin(Stdio::null());
    command
}

fn run(dir: &Path, args: &[&str]) -> Output {
    command(dir, args).output().expect("sbase runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The subcommands `sbase --help` lists, in the order it lists them.
fn subcommands(dir: &Path) -> Vec<String> {
    let output = run(dir, &["--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let help = stdout(&output);
    let listed = help
        .split("Commands:")
        .nth(1)
        .and_then(|rest| rest.split("Options:").next())
        .expect("the help has a Commands section");
    listed
        .lines()
        .filter_map(|line| {
            let name = line.strip_prefix("  ")?;
            if name.starts_with(' ') {
                return None;
            }
            name.split_whitespace().next().map(str::to_owned)
        })
        .collect()
}

#[test]
fn every_subcommand_prints_its_help_and_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    let names = subcommands(dir.path());

    // A parser that found nothing would make the loop below pass for nothing.
    for known in [
        "open",
        "smoke",
        "doctor",
        "install",
        "test",
        "mkfile",
        "mkdir",
        "encrypt",
        "decrypt",
        "translate",
        "print-pdf",
        "record",
        "replay",
        "grid-hub",
        "completions",
    ] {
        assert!(
            names.iter().any(|name| name == known),
            "{known} in {names:?}"
        );
    }
    assert!(names.len() > 100, "{} subcommands: {names:?}", names.len());

    let mut failures = Vec::new();
    for name in names.iter().filter(|name| *name != "help") {
        for args in [[name.as_str(), "--help"], ["help", name.as_str()]] {
            let output = run(dir.path(), &args);
            let help = stdout(&output);
            if !output.status.success() || !help.contains("Usage:") || help.trim().is_empty() {
                failures.push(format!(
                    "{args:?}: exit {:?}, stderr {}",
                    output.status.code(),
                    stderr(&output)
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn completions_are_written_for_every_shell_and_an_unknown_shell_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let output = run(dir.path(), &["completions", shell]);
        assert!(output.status.success(), "{shell}: {}", stderr(&output));
        let script = stdout(&output);
        assert!(script.contains("sbase"), "{shell}: {script}");
        assert!(script.contains("mkfile"), "{shell} knows the subcommands");
    }

    let unknown = run(dir.path(), &["completions", "tcsh"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(
        stderr(&unknown).contains("bash") && stderr(&unknown).contains("powershell"),
        "the error names the shells there are: {}",
        stderr(&unknown)
    );
}

const SELENIUMBASE_TEST: &str = r##"from seleniumbase import BaseCase


class LoginTest(BaseCase):
    def test_login(self):
        self.open("https://example.test/login")
        self.type("#user", "demo_user")
        self.click("button")
        self.assert_text("Welcome", "h1")
"##;

#[test]
fn import_python_writes_a_rust_test_that_parses() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("login_test.py"), SELENIUMBASE_TEST).unwrap();

    let output = run(dir.path(), &["import-python", "login_test.py"]);

    assert!(output.status.success(), "{}", stderr(&output));
    let rust = stdout(&output);
    let file = syn::parse_file(&rust).unwrap_or_else(|error| panic!("{error}:\n{rust}"));
    assert_eq!(file.items.len(), 2, "a `use` and the test:\n{rust}");
    assert!(
        rust.contains("#[tokio::test]\nasync fn login_test()"),
        "{rust}"
    );
    assert!(
        rust.contains("sb.open(\"https://example.test/login\")"),
        "{rust}"
    );
    assert!(rust.contains("sb.click(\"button\")"), "{rust}");

    // `-o` writes the file instead of printing, and `--test-name` names the test.
    let written = run(
        dir.path(),
        &[
            "import-python",
            "login_test.py",
            "-o",
            "out.rs",
            "--test-name",
            "renamed",
        ],
    );
    assert!(written.status.success(), "{}", stderr(&written));
    assert!(stdout(&written).is_empty(), "{}", stdout(&written));
    let saved = std::fs::read_to_string(dir.path().join("out.rs")).unwrap();
    assert!(saved.contains("async fn renamed()"), "{saved}");
    syn::parse_file(&saved).unwrap();
}

#[test]
fn import_python_says_what_it_could_not_convert_and_leaves_a_todo_in_its_place() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("odd.py"),
        "from seleniumbase import BaseCase\n\
         class T(BaseCase):\n\
         \x20   def test_x(self):\n\
         \x20       self.open(\"https://example.test\")\n\
         \x20       self.frobnicate_the_widget(\"a\")\n",
    )
    .unwrap();

    let output = run(dir.path(), &["import-python", "odd.py"]);

    assert!(output.status.success(), "a gap is a warning, not a failure");
    assert!(
        stdout(&output).contains("TODO(source line 5)"),
        "the gap is marked in the code: {}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("frobnicate_the_widget"),
        "and named on stderr: {}",
        stderr(&output)
    );
}

#[test]
fn import_python_names_a_file_it_cannot_read() {
    let dir = tempfile::tempdir().unwrap();

    let output = run(dir.path(), &["import-python", "no_such_test.py"]);

    assert_eq!(output.status.code(), Some(6), "a local file failed");
    assert!(
        stderr(&output).contains("no_such_test.py"),
        "the error says which file: {}",
        stderr(&output)
    );
    assert!(stdout(&output).is_empty());
}

#[test]
fn mkfile_then_print_shows_the_file_that_was_written() {
    let dir = tempfile::tempdir().unwrap();

    let made = run(
        dir.path(),
        &["mkfile", "tests/login.rs", "--url", "http://127.0.0.1:1/"],
    );
    assert!(made.status.success(), "{}", stderr(&made));
    assert!(
        stdout(&made).contains("Created tests/login.rs"),
        "{}",
        stdout(&made)
    );

    let printed = run(dir.path(), &["print", "--file", "tests/login.rs"]);
    assert!(printed.status.success(), "{}", stderr(&printed));
    let on_disk = std::fs::read_to_string(dir.path().join("tests/login.rs")).unwrap();
    assert_eq!(stdout(&printed).trim_end(), on_disk.trim_end());
    syn::parse_file(&on_disk).expect("the scaffolded test is Rust");
}

#[test]
fn patch_chromedriver_patches_in_place_and_says_so_for_a_file_that_is_not_there() {
    let dir = tempfile::tempdir().unwrap();
    let driver = dir.path().join("chromedriver");
    let marker = b"window.cdc_abcdef1234567890abcdef_Array = window.Array;";
    std::fs::write(&driver, marker).unwrap();

    let output = run(
        dir.path(),
        &["patch-chromedriver", "--path", driver.to_str().unwrap()],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    let patched = std::fs::read(&driver).unwrap();
    assert_eq!(
        patched.len(),
        marker.len(),
        "a patched binary keeps its size"
    );
    assert!(
        !String::from_utf8_lossy(&patched).contains("cdc_abcdef1234567890abcdef"),
        "the marker is gone"
    );
    assert_eq!(
        std::fs::read(dir.path().join("chromedriver.orig")).unwrap(),
        marker,
        "the original is kept beside it"
    );

    let missing = run(dir.path(), &["patch-chromedriver", "--path", "nope"]);
    assert_eq!(missing.status.code(), Some(4), "the driver failed");
    assert!(stderr(&missing).contains("nope"), "{}", stderr(&missing));
    assert!(stderr(&missing).contains("hint:"), "{}", stderr(&missing));
}

#[test]
fn decrypting_with_the_wrong_passphrase_says_the_passphrase_is_wrong() {
    let dir = tempfile::tempdir().unwrap();
    let encrypted = command(dir.path(), &["encrypt", "my secret"])
        .env("SB_ENCRYPTION_KEY", "right")
        .output()
        .unwrap();
    let token = stdout(&encrypted).trim().to_owned();
    assert!(token.starts_with("sbenc1:"), "{token}");

    let output = command(dir.path(), &["decrypt", &token])
        .env("SB_ENCRYPTION_KEY", "wrong")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let message = stderr(&output);
    assert!(
        message.contains("passphrase") && message.contains("SB_ENCRYPTION_KEY"),
        "the error names the passphrase and where it comes from: {message}"
    );
    assert!(
        !message.contains("ring::error") && !message.contains("current browser, engine"),
        "no library internals and no advice about browsers and engines: {message}"
    );
}

#[test]
fn a_grid_command_without_java_or_a_jar_fails_with_a_clear_message() {
    let dir = tempfile::tempdir().unwrap();

    let output = run(
        dir.path(),
        &["grid-hub", "start", "--java", "/no/such/java"],
    );

    assert!(!output.status.success());
    let message = stderr(&output);
    assert!(
        message.contains("jar") || message.contains("java"),
        "the error says what is missing: {message}"
    );
    assert!(stdout(&output).is_empty());
}
