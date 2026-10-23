//! The whole `sbrs` command line as a person meets it, run as the real binary.
//!
//! `cli_scaffold.rs`, `cli_generators.rs`, `cli_crypto.rs` and the other `cli_*` files
//! check one family of commands closely. This file checks across them: every
//! subcommand the program lists answers `--help`, and the commands that need no
//! browser run for real in a scratch folder (shell completions, `import-python`,
//! `patch-chromedriver`, `decrypt` with the wrong passphrase, `mkfile` then `print`).
//! The commands that drive a browser are in `cli_e2e_chrome.rs`.

use std::path::Path;
use std::process::{Command, Output, Stdio};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

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
    command(dir, args).output().expect("sbrs runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The subcommands `sbrs --help` lists, in the order it lists them.
fn subcommands(dir: &Path) -> Vec<String> {
    let output = run(dir, &["--help"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let help = stdout(&output);
    let listed = help
        .split("Commands:")
        .nth(1)
        .and_then(|rest| rest.split("Flags:").next())
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
fn get_is_install_and_names_the_firefox_and_edge_drivers() {
    let dir = tempfile::tempdir().unwrap();
    let help = run(dir.path(), &["get", "--help"]);
    assert!(help.status.success(), "{}", stderr(&help));
    let text = stdout(&help);
    for driver in ["chromedriver", "geckodriver", "edgedriver"] {
        assert!(text.contains(driver), "{driver} in:\n{text}");
    }

    // A driver it does not know, and a version that names no geckodriver, fail
    // with a status and a message before anything is downloaded.
    for args in [
        &["install", "operadriver"][..],
        &["get", "geckodriver", "v1"],
    ] {
        let output = run(dir.path(), args);
        assert!(!output.status.success(), "{args:?} succeeded");
        let message = stderr(&output);
        assert!(
            message.contains(args[args.len() - 1]),
            "{args:?}: {message}"
        );
    }
    assert!(!dir.path().join("downloaded_drivers").exists());
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
    for shell in [
        "bash",
        "zsh",
        "fish",
        "powershell",
        "elvish",
        "fig",
        "usage",
    ] {
        let output = run(dir.path(), &["completions", shell]);
        assert!(output.status.success(), "{shell}: {}", stderr(&output));
        let script = stdout(&output);
        assert!(script.contains("sbrs"), "{shell}: {script}");
        // The specs list the subcommands; the scripts of a shell ask `sbrs` itself for
        // the candidates when the key is pressed (see below).
        if matches!(shell, "fig" | "usage") {
            assert!(script.contains("mkfile"), "{shell} knows the subcommands");
        }
    }
    // The Fig spec is what Warp's built-in specs, inshellisense and Amazon Q read.
    let fig = stdout(&run(dir.path(), &["completions", "fig"]));
    assert!(fig.contains(": Fig.Spec = {"), "{fig}");

    // What the installed script of a shell calls when the completion key is pressed.
    let answer = run(
        dir.path(),
        &[
            "__complete_word__",
            "--shell",
            "bash",
            "--line",
            "sbrs mkf",
            "--bash-word",
            "mkf",
        ],
    );
    assert!(answer.status.success(), "{}", stderr(&answer));
    assert_eq!(stdout(&answer).trim(), "mkfile");

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

const PLAYWRIGHT_TEST: &str = r#"import { test, expect } from '@playwright/test';

test('signs in', async ({ page }) => {
  await page.goto('https://example.test/login');
  await page.getByLabel('Email').fill('demo@example.test');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page.locator('h1')).toHaveText('Welcome');
});
"#;

const PLAYWRIGHT_SCRIPT: &str = r#"const { chromium } = require('playwright');
(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  await page.goto('https://example.test/');
  await browser.close();
})();
"#;

#[test]
fn import_playwright_writes_rust_tests_that_parse() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("login.spec.ts"), PLAYWRIGHT_TEST).unwrap();

    let output = run(dir.path(), &["import-playwright", "login.spec.ts"]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stderr(&output).is_empty(),
        "nothing to report: {}",
        stderr(&output)
    );
    let rust = stdout(&output);
    syn::parse_file(&rust).unwrap_or_else(|error| panic!("{error}:\n{rust}"));
    assert!(
        rust.contains("#[tokio::test]\nasync fn signs_in()"),
        "{rust}"
    );
    assert!(
        rust.contains("page.goto(\"https://example.test/login\").await?;"),
        "{rust}"
    );
    assert!(
        rust.contains("page.locator(\"h1\").expect().to_have_text(\"Welcome\").await?;"),
        "{rust}"
    );

    // The alias, `-o`, and `--test-name` for a script without test titles.
    std::fs::write(dir.path().join("script.js"), PLAYWRIGHT_SCRIPT).unwrap();
    let written = run(
        dir.path(),
        &[
            "import-pw",
            "script.js",
            "-o",
            "out.rs",
            "--test-name",
            "home page",
        ],
    );
    assert!(written.status.success(), "{}", stderr(&written));
    assert!(stdout(&written).is_empty(), "{}", stdout(&written));
    let saved = std::fs::read_to_string(dir.path().join("out.rs")).unwrap();
    assert!(saved.contains("async fn home_page()"), "{saved}");
    syn::parse_file(&saved).unwrap();

    // `--language` reads a file whose extension says nothing.
    std::fs::write(dir.path().join("login.txt"), PLAYWRIGHT_TEST).unwrap();
    let typed = run(
        dir.path(),
        &["import-playwright", "login.txt", "--language", "ts"],
    );
    assert!(typed.status.success(), "{}", stderr(&typed));
    assert!(stdout(&typed).contains("async fn signs_in()"));
}

#[test]
fn import_playwright_leaves_a_todo_and_names_it_on_stderr() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("mocked.spec.js"),
        "test('mocks', async ({ page }) => {\n  await page.route('**/api', r => r.abort());\n  await page.goto('https://example.test/');\n});\n",
    )
    .unwrap();

    let output = run(dir.path(), &["import-playwright", "mocked.spec.js"]);

    assert!(output.status.success(), "a gap is a warning, not a failure");
    assert!(
        stdout(&output)
            .contains("// TODO(playwright): await page.route('**/api', r => r.abort());"),
        "the gap is marked in the code: {}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("Warning at line 2:") && stderr(&output).contains("route"),
        "and named on stderr: {}",
        stderr(&output)
    );
}

#[test]
fn import_playwright_exit_statuses() {
    let dir = tempfile::tempdir().unwrap();

    let missing = run(dir.path(), &["import-playwright", "no_such_test.spec.ts"]);
    assert_eq!(missing.status.code(), Some(6), "a local file failed");
    assert!(
        stderr(&missing).contains("no_such_test.spec.ts"),
        "the error says which file: {}",
        stderr(&missing)
    );
    assert!(stdout(&missing).is_empty());

    // A statement the importer cannot read at all is an error, as in import-python.
    std::fs::write(
        dir.path().join("broken.spec.js"),
        "test('t', async ({ page }) => {\n  await page.goto(;\n});\n",
    )
    .unwrap();
    let broken = run(dir.path(), &["import-playwright", "broken.spec.js"]);
    assert_eq!(broken.status.code(), Some(1), "{}", stderr(&broken));
    assert!(
        stderr(&broken).contains("Error at line 2"),
        "{}",
        stderr(&broken)
    );
    assert!(stdout(&broken).contains("TODO(playwright)"));

    // A Recorder export is for import-puppeteer.
    std::fs::write(
        dir.path().join("rec.json"),
        "{\"title\": \"r\", \"steps\": []}",
    )
    .unwrap();
    let wrong = run(dir.path(), &["import-playwright", "rec.json"]);
    assert_eq!(wrong.status.code(), Some(2), "{}", stderr(&wrong));
    assert!(
        stderr(&wrong).contains("import-puppeteer"),
        "{}",
        stderr(&wrong)
    );
}

#[test]
fn import_puppeteer_reads_scripts_and_recorder_exports() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("visit.js"),
        "const puppeteer = require('puppeteer');\n(async () => {\n  const browser = await puppeteer.launch();\n  const page = await browser.newPage();\n  await page.goto('https://example.test/');\n  await page.type('#q', 'rust');\n  await page.click('button[type=submit]');\n  await browser.close();\n})();\n",
    )
    .unwrap();
    let output = run(dir.path(), &["import-puppeteer", "visit.js"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let rust = stdout(&output);
    syn::parse_file(&rust).unwrap_or_else(|error| panic!("{error}:\n{rust}"));
    assert!(rust.contains("async fn visit()"), "{rust}");
    assert!(
        rust.contains("page.locator(\"#q\").type_text(\"rust\").await?;"),
        "{rust}"
    );

    std::fs::write(
        dir.path().join("recording.json"),
        "{\n  \"title\": \"Search\",\n  \"steps\": [\n    {\"type\": \"navigate\", \"url\": \"https://example.test/\"},\n    {\"type\": \"click\", \"selectors\": [[\"#go\"]]},\n    {\"type\": \"customStep\", \"name\": \"x\"}\n  ]\n}\n",
    )
    .unwrap();
    let recorded = run(dir.path(), &["import-pptr", "recording.json"]);
    assert!(recorded.status.success(), "{}", stderr(&recorded));
    assert!(stdout(&recorded).contains("async fn search()"));
    assert!(stdout(&recorded).contains("page.locator(\"#go\").click().await?;"));
    assert!(stdout(&recorded).contains("TODO(puppeteer)"));
    assert!(
        stderr(&recorded).contains("Warning at line 6:"),
        "{}",
        stderr(&recorded)
    );

    let missing = run(dir.path(), &["import-puppeteer", "nowhere.js"]);
    assert_eq!(missing.status.code(), Some(6));
    assert!(
        stderr(&missing).contains("nowhere.js"),
        "{}",
        stderr(&missing)
    );
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
        stdout(&made).contains(&format!(
            "Created {}",
            "tests/login.rs".replace('/', std::path::MAIN_SEPARATOR_STR)
        )),
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

/// Windows gives a program's main thread a 1 MB stack, where Linux and macOS
/// give 8 MB. A debug `sbrs` once overflowed it before printing anything, so
/// every command crashed there. On Unix the main thread's stack follows
/// `ulimit -s`, which lets this check run without Windows.
#[cfg(unix)]
#[test]
fn sbase_starts_with_the_stack_a_windows_main_thread_gets() {
    for args in ["--version", "--help", "doctor --help"] {
        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("ulimit -s 1024 && exec \"$0\" {args}"))
            .arg(SBASE)
            .stdin(Stdio::null())
            .output()
            .expect("run sbrs under a 1 MB stack");
        assert!(
            output.status.success(),
            "sbrs {args} with a 1 MB stack: {:?}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn a_reader_that_goes_away_early_is_not_an_error() {
    // `sbrs completions zsh | head -c 80`: the reader closes the pipe, and the
    // next write fails with a broken pipe. That is the normal end of a
    // pipeline, so sbrs must not print an error or fail.
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_sbrs"))
        .args(["completions", "zsh"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start sbrs");
    // Close the reading end before sbrs writes anything.
    drop(child.stdout.take());
    let output = child.wait_with_output().expect("wait for sbrs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status {:?}, stderr: {stderr}",
        output.status
    );
    assert!(!stderr.contains("error"), "stderr: {stderr}");
}

#[test]
fn zsh_completion_ends_a_complete_word_with_a_space() {
    // usage 6.12's zsh script added every candidate with `compadd -S ''`, so `sbrs pl<Tab>`
    // stopped at `plugin` with no space and the next Tab never reached `info` and `list`.
    // The script runs in a real zsh with a stub `compadd` that prints whether zsh would add
    // its space ("space") or nothing ("none"), and the words it was given.
    if std::process::Command::new("zsh")
        .args(["-fc", "true"])
        .status()
        .is_err()
    {
        eprintln!("skipped: zsh is not installed");
        return;
    }
    let sbrs = std::path::Path::new(env!("CARGO_BIN_EXE_sbrs"));
    let path = format!(
        "{}:{}",
        sbrs.parent().expect("sbrs has a folder").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let script = r#"
compadd() {
    local suffix=space array=
    while (( $# )); do
        case $1 in
            -S) suffix=none; shift 2 ;;
            -a) array=$2; shift 2 ;;
            -d) shift 2 ;;
            *) shift ;;
        esac
    done
    print -r -- "$suffix ${(P)array}"
}
compdef() { :; }
_files() { :; }
_command_names() { :; }
typeset -A compstate
source <(sbrs completions zsh)
for BUFFER in 'sbrs pl' 'sbrs mkfile x.rs --ur'; do
    CURSOR=${#BUFFER}
    _sbrs
done
"#;
    let output = std::process::Command::new("zsh")
        .args(["-fc", script])
        .env("PATH", path)
        .output()
        .expect("run zsh");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.lines().any(|line| line == "space plugin"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stdout.lines().any(|line| line == "space --url"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
fn powershell_completion_after_a_space_offers_the_next_level() {
    // usage 6.12's PowerShell script cut the line at `Extent.Text`, which leaves out the space
    // after the last word, so `sbrs plugin <Tab>` completed `plugin` again.
    if std::process::Command::new("pwsh")
        .args(["-NoProfile", "-Command", "exit 0"])
        .status()
        .is_err()
    {
        eprintln!("skipped: pwsh is not installed");
        return;
    }
    let sbrs = std::path::Path::new(env!("CARGO_BIN_EXE_sbrs"));
    let path = format!(
        "{}:{}",
        sbrs.parent().expect("sbrs has a folder").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let script = "
. ([scriptblock]::Create((& sbrs completions powershell | Out-String)))
$line = 'sbrs plugin '
(TabExpansion2 -inputScript $line -cursorColumn $line.Length).CompletionMatches |
    ForEach-Object { $_.CompletionText }
";
    let output = std::process::Command::new("pwsh")
        .args(["-NoProfile", "-Command", script])
        .env("PATH", path)
        .output()
        .expect("run pwsh");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let words: Vec<&str> = stdout.lines().map(str::trim).collect();
    assert!(
        words.contains(&"info") && words.contains(&"list"),
        "stdout: {stdout}\nstderr: {stderr}"
    );
}
