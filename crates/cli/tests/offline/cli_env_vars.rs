//! The `SB_*` variables of `sbrs`: the list that `sbrs options --env` prints, a value that
//! cannot be used (status 2, the variable named), and the variables that stand for a flag
//! the option table does not make.
//!
//! The variables of the option table are read by `Settings::load`, which the rows of the
//! table and `sbrs open` below share, so one wrong value for each kind of row proves they are
//! read. The spec-walking tests (every flag that has a variable names it in its help) are
//! unit tests of the binary: `cargo test -p seleniumbase-rs-cli --bin sbrs browser_flags`.

use super::fake_webdriver;
use super::sbrs;

use std::collections::BTreeSet;
use std::path::Path;

use fake_webdriver::FakeWebDriver;
use seleniumbase_rs::config::env_vars::{variables, EXTRA_VARIABLES};
use seleniumbase_rs::config::test_options::{OptionKind, OptionSpec, TEST_OPTIONS};

const OPEN: [&str; 2] = ["open", "https://example.test/"];

/// A value of the variable of `row` that no `Settings` accepts, or `None` for a row whose
/// kind takes any text.
fn bad_value(row: &OptionSpec) -> Option<&'static str> {
    match row.kind {
        OptionKind::Switch => Some("maybe"),
        OptionKind::Value(kind) | OptionKind::Env(kind) => match kind {
            "Seconds" | "Count" | "Port" | "Pixels" | "Float" => Some("not-a-number"),
            "Browser" => Some("chrom"),
            "Mode" => Some("stealth"),
            _ => None,
        },
        _ => None,
    }
}

#[test]
fn a_value_that_cannot_be_used_ends_with_status_2_and_names_the_variable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut checked = 0;
    for row in TEST_OPTIONS {
        let Some(bad) = bad_value(row) else {
            continue;
        };
        let output = sbrs::run(dir.path(), &[(row.env, bad)], &OPEN);
        let stderr = sbrs::stderr(&output);

        assert_eq!(output.status.code(), Some(2), "{}={bad}: {stderr}", row.env);
        assert!(stderr.contains(row.env), "{}={bad}: {stderr}", row.env);
        checked += 1;
    }
    assert!(checked > 60, "only {checked} variables were tried");
}

#[test]
fn the_listing_has_a_line_for_every_variable() {
    let dir = tempfile::tempdir().expect("temp dir");
    let output = sbrs::run(dir.path(), &[], &["options", "--env"]);
    assert!(output.status.success(), "{}", sbrs::stderr(&output));
    let text = sbrs::stdout(&output);

    let listed: BTreeSet<&str> = text
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    for variable in variables() {
        assert!(
            listed.contains(variable.name),
            "{} is not listed:\n{text}",
            variable.name
        );
    }
    // The header and a few lines that show where each column comes from.
    assert!(text.starts_with("VARIABLE"), "{text}");
    let line = |name: &str| {
        text.lines()
            .find(|line| line.split_whitespace().next() == Some(name))
            .unwrap_or_else(|| panic!("no line for {name}"))
    };
    assert!(line("SB_BROWSER").contains("--browser"), "{text}");
    assert!(line("SB_BROWSER").contains("chrome"), "{text}");
    assert!(line("SB_PROXY_LIST").contains("--proxy-list"), "{text}");
    assert!(
        line("SB_MCP_LISTEN").contains("sbrs-mcp --listen"),
        "{text}"
    );
    assert!(line("SB_WEBDRIVER_URL").contains("--webdriver"), "{text}");

    let markdown = sbrs::run(dir.path(), &[], &["options", "--env", "--markdown"]);
    assert!(sbrs::stdout(&markdown).starts_with("| Variable |"));
}

/// The variables the library reads outside the option table (the recorder, S3 logging, the OCR
/// models and the folder of the Grid) are in the listing of `sbrs options --env`.
#[test]
fn the_variables_of_the_recorder_s3_ocr_and_the_grid_are_listed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let output = sbrs::run(dir.path(), &[], &["options", "--env"]);
    let text = sbrs::stdout(&output);
    for name in [
        "SB_REC_FILE",
        "SB_REC_KEEP_SECRETS",
        "SB_S3_BUCKET",
        "SB_S3_REGION",
        "SB_S3_PREFIX",
        "SB_OCR_MODELS",
        "SB_GRID_DIR",
    ] {
        assert!(
            text.lines()
                .any(|line| line.split_whitespace().next() == Some(name)),
            "{name} is not listed:\n{text}"
        );
    }
}

/// Every variable that is not a row of the option table is the subject of a test, here or in
/// another file: a new one without a test fails this.
#[test]
fn every_extra_variable_is_named_by_a_test() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut sources = String::new();
    // Suites live in `offline/` and `chrome/`. A flat listing would only see `main.rs`.
    let mut dirs = vec![tests];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("the tests folder") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                let name = path.file_name().and_then(|name| name.to_str());
                if name == Some("offline") || name == Some("chrome") {
                    dirs.push(path);
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                sources.push_str(&std::fs::read_to_string(&path).expect("a test file"));
            }
        }
    }
    // The runtime variables are tested next to the code that reads them.
    sources.push_str(include_str!("../../../../src/config/runtime.rs"));
    for extra in EXTRA_VARIABLES {
        assert!(sources.contains(extra.name), "no test names {}", extra.name);
    }
}

fn write_list(dir: &Path, name: &str, text: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write the list");
    path.display().to_string()
}

/// What `sbrs` asked the fake server for, as text.
fn session_request(driver: &FakeWebDriver) -> String {
    driver
        .requests()
        .into_iter()
        .find(|request| request.path == "/session")
        .map(|request| request.body.to_string())
        .expect("a new-session request")
}

#[test]
fn sb_proxy_list_is_the_proxy_list_flag() {
    let dir = tempfile::tempdir().expect("temp dir");
    let first = write_list(dir.path(), "first.txt", "only = 10.0.0.5:3128\n");
    let second = write_list(
        dir.path(),
        "second.txt",
        "one = 10.0.0.6:3128\ntwo = 10.0.0.7:3128\n",
    );
    let open = |env: &[(&str, &str)], flags: &[&str]| {
        let driver = FakeWebDriver::start();
        let mut args = vec!["--webdriver", driver.url()];
        args.extend(flags);
        args.extend(OPEN);
        let output = sbrs::run(dir.path(), env, &args);
        assert!(output.status.success(), "{}", sbrs::stderr(&output));
        session_request(&driver)
    };

    // The variable names the list, and a member of it is the proxy.
    let from_variable = open(&[("SB_PROXY_LIST", &first)], &[]);
    assert!(from_variable.contains("10.0.0.5:3128"), "{from_variable}");

    // SB_PROXY names the member to use.
    let named = open(&[("SB_PROXY_LIST", &second), ("SB_PROXY", "two")], &[]);
    assert!(named.contains("10.0.0.7:3128"), "{named}");

    // The flag wins over the variable.
    let flagged = open(
        &[("SB_PROXY_LIST", &first)],
        &["--proxy-list", &second, "--proxy", "one"],
    );
    assert!(flagged.contains("10.0.0.6:3128"), "{flagged}");
    assert!(!flagged.contains("10.0.0.5:3128"), "{flagged}");
}

#[test]
fn sb_save_screenshot_is_the_save_screenshot_flag() {
    let dir = tempfile::tempdir().expect("temp dir");
    let asked = |env: &[(&str, &str)]| {
        let driver = FakeWebDriver::start();
        let mut args = vec!["--webdriver", driver.url()];
        args.extend(OPEN);
        // The fake server has no picture to give, so the command may end with an error; what
        // matters is whether the browser was asked for one.
        let _ = sbrs::run(dir.path(), env, &args);
        driver
            .requests()
            .iter()
            .any(|request| request.path == "/screenshot")
    };

    assert!(asked(&[("SB_SAVE_SCREENSHOT", "true")]));
    assert!(!asked(&[]));
    assert!(!asked(&[("SB_SAVE_SCREENSHOT", "false")]));
}
