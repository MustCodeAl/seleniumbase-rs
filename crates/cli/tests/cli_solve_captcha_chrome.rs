//! `sbrs solve-captcha`, run as the real binary against the imitation interstitial of
//! `tests/fixtures/captcha/` in a real headless Chrome.
//!
//! The command opens `--url`, solves the widget, then checks what the caller expects of the
//! page: text of the site's own page (`--expect-text`) and a clearance cookie
//! (`--expect-cookie`). It is the same command line that the opt-in live check gives for
//! https://nowsecure.nl; this test only ever talks to the local fixture site.
//!
//! The test starts Chrome through chromedriver, so it is ignored by default:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test cli_solve_captcha_chrome -- --ignored --test-threads=1
//! ```

#[path = "../../../tests/support/captcha_site.rs"]
mod captcha_site;

use std::process::{Command, Output, Stdio};

use captcha_site::Site;

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// Every `SB_*` variable that changes what a browser command does, cleared so the host's own
/// settings cannot leak in.
const SETTINGS_VARS: [&str; 6] = [
    "SB_BROWSER",
    "SB_HEADLESS",
    "SB_MODE",
    "SB_START_PAGE",
    "SB_PROXY",
    "SB_WEBDRIVER_URL",
];

/// Runs `sbrs --headless <args>` to the end.
fn sbrs(args: &[&str]) -> Output {
    let mut command = Command::new(SBASE);
    command
        .arg("--headless")
        .args(args)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null());
    for var in SETTINGS_VARS {
        command.env_remove(var);
    }
    command.output().expect("sbrs runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
#[ignore = "requires a local Chrome/chromedriver"]
fn solve_captcha_clears_the_interstitial_and_checks_the_page_and_the_cookie() {
    let site = Site::start();
    let url = site.url("/interstitial");

    let output = sbrs(&[
        "--url",
        &url,
        "solve-captcha",
        "--expect-text",
        "real page",
        "--expect-cookie",
        "cf_clearance",
        "--wait",
        "20",
    ]);

    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}\n{}", text(&output.stderr));
    assert!(
        stdout.contains("Solved a Cloudflare Turnstile CAPTCHA"),
        "{stdout}"
    );
    assert!(stdout.contains("The page shows 'real page'."), "{stdout}");
    assert!(
        stdout.contains("The browser holds the cookie 'cf_clearance'."),
        "{stdout}"
    );
}

#[test]
#[ignore = "requires a local Chrome/chromedriver"]
fn solve_captcha_fails_when_the_expected_text_never_comes() {
    let site = Site::start();
    let url = site.url("/interstitial");

    let output = sbrs(&[
        "--url",
        &url,
        "solve-captcha",
        "--expect-text",
        "a sentence this fixture never shows",
        "--wait",
        "2",
    ]);

    assert!(!output.status.success(), "{}", text(&output.stdout));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("is not on the page"), "{stderr}");
}

#[test]
#[ignore = "requires a local Chrome/chromedriver"]
fn solve_captcha_on_a_page_without_a_widget_says_so() {
    let site = Site::start();
    let url = site.url("/decoy?widget=none");

    let output = sbrs(&["--url", &url, "solve-captcha"]);

    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}\n{}", text(&output.stderr));
    assert!(
        stdout.contains("No supported CAPTCHA was found on the page."),
        "{stdout}"
    );
}
