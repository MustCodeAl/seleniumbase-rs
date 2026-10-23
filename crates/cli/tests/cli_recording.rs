//! `sbrs --rec-sb-cdp`: the recording that `sbrs run-scenario` saves, run as
//! the real binary against a fake WebDriver server (no browser).
//!
//! The scenario's steps run through `BaseCase`, which records them; with the
//! flag the saved `.rs` file is a pure CDP program, without it a `BaseCase` one.

#[path = "support/fake_webdriver.rs"]
mod fake_webdriver;

use std::path::Path;
use std::process::{Command, Output};

use fake_webdriver::FakeWebDriver;

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

const SCENARIO: &str = r##"{
    "name": "search",
    "steps": [
        { "action": "open", "url": "https://example.test/" },
        { "action": "click", "css": "#go" },
        { "action": "type_text", "css": "#q", "text": "rust \"2024\"" }
    ]
}"##;

/// Every `SB_*` variable the settings read, cleared so the host's own cannot
/// leak into a test.
const SETTINGS_VARS: [&str; 18] = [
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
];

/// Runs `sbrs` with `args` in `dir`, which holds the scenario.
fn sbrs(dir: &Path, webdriver: &FakeWebDriver, args: &[&str]) -> Output {
    let mut command = Command::new(SBASE);
    command
        .current_dir(dir)
        .arg("--webdriver")
        .arg(webdriver.url())
        .args(args)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error")
        .env_remove("SB_ESC_END")
        .env_remove("SB_REC_SB_CDP")
        .env_remove("SB_WEBDRIVER_URL");
    for var in SETTINGS_VARS {
        command.env_remove(var);
    }
    command.output().expect("sbrs runs")
}

/// Runs the scenario with the global `flags`, and returns the recording script
/// that `run-scenario` saved.
fn recorded_script(flags: &[&str]) -> String {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("scenario.json"), SCENARIO).expect("write scenario");
    // `--rec-sb-cdp` turns Recorder Mode on, as in Python, and a Recorder session asks the
    // browser for its windows and runs the page script: a fake that answers those.
    let driver = if flags.is_empty() {
        FakeWebDriver::start()
    } else {
        FakeWebDriver::start_with_recording("[]")
    };
    let mut args = flags.to_vec();
    args.extend(["run-scenario", "--file", "scenario.json"]);

    let output = sbrs(dir.path(), &driver, &args);

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let saved = stdout
        .lines()
        .find_map(|line| line.strip_prefix("Recording script: "))
        .unwrap_or_else(|| panic!("no recording script was reported:\n{stdout}"));
    // The path is relative to the folder `sbrs` ran in.
    std::fs::read_to_string(dir.path().join(saved)).unwrap_or_else(|e| panic!("{saved}: {e}"))
}

#[test]
fn without_the_flag_the_recording_calls_basecase() {
    let script = recorded_script(&[]);

    assert!(
        script.contains("sb.open(\"https://example.test/\").await?;"),
        "{script}"
    );
    assert!(script.contains("sb.click(\"#go\").await?;"), "{script}");
    assert!(!script.contains("sb_cdp"), "{script}");
}

#[test]
fn rec_sb_cdp_saves_the_recording_as_a_pure_cdp_program() {
    for flag in ["--rec-sb-cdp", "--rec_sb_cdp"] {
        let script = recorded_script(&[flag]);

        assert!(
            script.starts_with("use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};\n"),
            "{flag}: {script}"
        );
        for line in [
            "    page.goto(\"https://example.test/\").await?;\n",
            "    page.locator(\"#go\").click().await?;\n",
            "    page.locator(\"#q\").type_text(\"rust \\\"2024\\\"\").await?;\n",
            "    browser.close().await?;\n",
        ] {
            assert!(script.contains(line), "{flag}: {line:?} in\n{script}");
        }
        // `Locator::type_text` empties the field first, so no `clear` is written.
        assert!(!script.contains(".clear()"), "{flag}: {script}");
        assert!(!script.contains("sb.open"), "{flag}: {script}");
    }
}
