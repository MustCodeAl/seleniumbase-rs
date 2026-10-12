//! `sbase open`, `sbase smoke` and the other commands that drive a browser read
//! the `SB_*` variables, as the test runner does, with an explicit option
//! stronger than the environment and the environment stronger than the config
//! file.
//!
//! The real binary runs against a fake WebDriver server (no browser), and the
//! tests read what it asked that server for: the capabilities of the new
//! session and the scripts it ran.

#[path = "support/fake_webdriver.rs"]
mod fake_webdriver;

use std::path::Path;
use std::process::{Command, Output};

use fake_webdriver::FakeWebDriver;

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

/// Every `SB_*` variable `sbase` reads, cleared so the host's own cannot leak
/// into a test.
const SB_VARS: [&str; 24] = [
    "SB_BROWSER",
    "SB_HEADLESS",
    "SB_TIMEOUT",
    "SB_REMOTE_WEBDRIVER_TIMEOUT",
    "SB_SCREENSHOT_DIR",
    "SB_PROXY",
    "SB_PROXY_PAC_URL",
    "SB_WINDOW_WIDTH",
    "SB_WINDOW_HEIGHT",
    "SB_USER_DATA_DIR",
    "SB_EXTENSION_DIR",
    "SB_LOCALE",
    "SB_USER_AGENT",
    "SB_MODE",
    "SB_AD_BLOCK",
    "SB_REUSE_SESSION",
    "SB_MOBILE",
    "SB_THREADS",
    "SB_ESC_END",
    "SB_REC_SB_CDP",
    "SB_WEBDRIVER_URL",
    "SB_CHROME_BIN",
    "SB_LOG_FORMAT",
    "SB_CHROMEDRIVER_PORT",
];

/// Runs `sbase` in `dir` with `env` set and `args` after the executable's name.
fn run(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(SBASE);
    command
        .current_dir(dir)
        .args(args)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error");
    for var in SB_VARS {
        command.env_remove(var);
    }
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("sbase runs")
}

/// What `sbase` did against a fake server.
struct Run {
    driver: FakeWebDriver,
    output: Output,
}

impl Run {
    fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).into_owned()
    }

    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).into_owned()
    }

    /// The scripts the page was asked to run.
    fn scripts(&self) -> Vec<String> {
        self.driver
            .requests()
            .iter()
            .filter(|request| request.path == "/execute/sync")
            .filter_map(|request| request.body["script"].as_str().map(str::to_owned))
            .collect()
    }
}

/// Runs `sbase <flags> --webdriver <fake> <command...>` with `env` set.
fn against_a_fake_server(env: &[(&str, &str)], flags: &[&str], command: &[&str]) -> Run {
    let dir = tempfile::tempdir().expect("temp dir");
    against_a_fake_server_in(dir.path(), env, flags, command)
}

fn against_a_fake_server_in(
    dir: &Path,
    env: &[(&str, &str)],
    flags: &[&str],
    command: &[&str],
) -> Run {
    let driver = FakeWebDriver::start();
    let mut args = vec!["--webdriver", driver.url()];
    args.extend(flags);
    args.extend(command);
    let output = run(dir, env, &args);
    Run { driver, output }
}

const OPEN: [&str; 2] = ["open", "https://example.test/"];

#[test]
fn open_runs_headed_unless_something_asks_for_headless() {
    let run = against_a_fake_server(&[], &[], &OPEN);

    assert!(run.output.status.success(), "{}", run.stderr());
    assert!(
        run.stdout().contains("Title: Fake page"),
        "{}",
        run.stdout()
    );
    assert!(
        !run.driver.asked_for_headless(),
        "{:?}",
        run.driver.chrome_args()
    );
}

#[test]
fn open_honours_sb_headless() {
    let run = against_a_fake_server(&[("SB_HEADLESS", "true")], &[], &OPEN);

    assert!(run.output.status.success(), "{}", run.stderr());
    assert!(
        run.driver.asked_for_headless(),
        "{:?}",
        run.driver.chrome_args()
    );
}

#[test]
fn smoke_honours_sb_headless_too() {
    let run = against_a_fake_server(
        &[("SB_HEADLESS", "1")],
        &[],
        &["smoke", "https://example.test/", "--title-contains", "Fake"],
    );

    assert!(run.output.status.success(), "{}", run.stderr());
    assert!(
        run.stdout().contains("Assertion passed"),
        "{}",
        run.stdout()
    );
    assert!(
        run.driver.asked_for_headless(),
        "{:?}",
        run.driver.chrome_args()
    );
}

#[test]
fn an_explicit_flag_is_stronger_than_the_environment() {
    let headed = against_a_fake_server(&[("SB_HEADLESS", "true")], &["--headed"], &OPEN);
    assert!(headed.output.status.success(), "{}", headed.stderr());
    assert!(
        !headed.driver.asked_for_headless(),
        "--headed beats SB_HEADLESS=true"
    );

    let headless = against_a_fake_server(&[("SB_HEADLESS", "false")], &["--headless"], &OPEN);
    assert!(headless.output.status.success(), "{}", headless.stderr());
    assert!(
        headless.driver.asked_for_headless(),
        "--headless beats SB_HEADLESS=false"
    );
}

#[test]
fn the_environment_is_stronger_than_the_config_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("sbase_config.toml"), "headless = true\n").unwrap();

    let from_file = against_a_fake_server_in(dir.path(), &[], &[], &OPEN);
    assert!(from_file.output.status.success(), "{}", from_file.stderr());
    assert!(
        from_file.driver.asked_for_headless(),
        "the file alone is read"
    );

    let overridden = against_a_fake_server_in(dir.path(), &[("SB_HEADLESS", "false")], &[], &OPEN);
    assert!(
        overridden.output.status.success(),
        "{}",
        overridden.stderr()
    );
    assert!(
        !overridden.driver.asked_for_headless(),
        "SB_HEADLESS=false beats headless = true in the file"
    );
}

#[test]
fn the_other_settings_variables_are_read_and_a_flag_beats_them() {
    let run = against_a_fake_server(
        &[("SB_USER_AGENT", "FromEnv/1.0"), ("SB_LOCALE", "fr-FR")],
        &[],
        &OPEN,
    );
    assert!(run.output.status.success(), "{}", run.stderr());
    let args = run.driver.chrome_args().expect("a chrome session");
    assert!(
        args.iter().any(|a| a == "--user-agent=FromEnv/1.0"),
        "{args:?}"
    );
    assert!(args.iter().any(|a| a == "--lang=fr-FR"), "{args:?}");

    let flagged = against_a_fake_server(
        &[("SB_USER_AGENT", "FromEnv/1.0")],
        &["--user-agent", "FromFlag/2.0"],
        &OPEN,
    );
    assert!(flagged.output.status.success(), "{}", flagged.stderr());
    let args = flagged.driver.chrome_args().expect("a chrome session");
    assert!(
        args.iter().any(|a| a == "--user-agent=FromFlag/2.0"),
        "{args:?}"
    );
    assert!(!args.iter().any(|a| a.contains("FromEnv")), "{args:?}");
}

#[test]
fn sb_esc_end_puts_the_listener_in_the_page() {
    let run = against_a_fake_server(&[("SB_ESC_END", "true")], &[], &OPEN);
    assert!(run.output.status.success(), "{}", run.stderr());
    assert!(
        run.scripts()
            .iter()
            .any(|script| script.contains("sb_esc_armed")),
        "the ESC listener was put in the page: {:?}",
        run.scripts()
    );

    let without = against_a_fake_server(&[], &[], &OPEN);
    assert!(
        !without
            .scripts()
            .iter()
            .any(|script| script.contains("sb_esc_")),
        "{:?}",
        without.scripts()
    );
}

#[test]
fn sb_webdriver_url_picks_the_server_and_the_flag_beats_it() {
    let driver = FakeWebDriver::start();
    let dir = tempfile::tempdir().expect("temp dir");

    // No --webdriver: the variable names the server.
    let output = run(dir.path(), &[("SB_WEBDRIVER_URL", driver.url())], &OPEN);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        driver
            .requests()
            .iter()
            .any(|r| r.method == "POST" && r.path == "/session"),
        "the server in SB_WEBDRIVER_URL got the new-session request"
    );

    // A server that nobody listens on, and --webdriver pointing at a live one.
    let other = FakeWebDriver::start();
    let output = run(
        dir.path(),
        &[("SB_WEBDRIVER_URL", "http://127.0.0.1:9")],
        &["--webdriver", other.url(), "open", "https://example.test/"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !other.requests().is_empty(),
        "--webdriver beats SB_WEBDRIVER_URL"
    );
}

#[test]
fn sb_chrome_bin_names_the_browser_to_start() {
    let run = against_a_fake_server(&[("SB_CHROME_BIN", "/opt/chrome-test/chrome")], &[], &OPEN);

    assert!(run.output.status.success(), "{}", run.stderr());
    let session = run
        .driver
        .requests()
        .into_iter()
        .find(|r| r.path == "/session")
        .expect("a new-session request");
    assert_eq!(
        session.body["capabilities"]["alwaysMatch"]["goog:chromeOptions"]["binary"],
        "/opt/chrome-test/chrome"
    );
}

#[test]
fn a_bad_value_is_reported_and_nothing_is_started() {
    let run = against_a_fake_server(&[("SB_HEADLESS", "maybe")], &[], &OPEN);

    assert_eq!(run.output.status.code(), Some(2), "{}", run.stderr());
    assert!(
        run.stderr().contains("cannot parse bool: maybe"),
        "{}",
        run.stderr()
    );
    assert!(run.driver.requests().is_empty(), "no session was requested");
}

#[test]
fn sb_rec_sb_cdp_makes_run_scenario_save_a_pure_cdp_program() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("scenario.json"),
        r#"{ "name": "n", "steps": [ { "action": "open", "url": "https://example.test/" } ] }"#,
    )
    .unwrap();

    let run = against_a_fake_server_in(
        dir.path(),
        &[("SB_REC_SB_CDP", "true")],
        &[],
        &["run-scenario", "--file", "scenario.json"],
    );

    assert!(run.output.status.success(), "{}", run.stderr());
    let stdout = run.stdout();
    let saved = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Recording script: "))
        .unwrap_or_else(|| panic!("no recording script was reported:\n{stdout}"));
    let script = std::fs::read_to_string(dir.path().join(saved)).unwrap();
    assert!(
        script.contains("page.goto(\"https://example.test/\").await?;"),
        "{script}"
    );
}
