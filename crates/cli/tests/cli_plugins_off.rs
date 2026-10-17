//! A build of `sbrs` and `sbrs-mcp` without plugin support fails closed: a
//! program that asked for privacy hardening and silently runs without it is
//! worse than one that stops.
//!
//! These tests need such a build, so they are compiled only when the feature
//! `plugins` is off:
//! `cargo test -p seleniumbase-rs-cli --no-default-features --features mcp-server --test cli_plugins_off`.
//! No browser is started; the fake WebDriver server only sees whether a
//! session was asked for.

#![cfg(not(feature = "plugins"))]

#[path = "support/fake_webdriver.rs"]
mod fake_webdriver;
#[path = "support/sbrs.rs"]
mod sbrs;

use std::path::Path;
use std::process::Output;

use fake_webdriver::FakeWebDriver;
use sbrs::{run, stderr, stdout};

const REBUILD: &str = "--features plugins-builtin";

/// Runs `sbrs <flags> --webdriver <fake> open ...` in `dir`, and returns the
/// result and the requests the fake server saw.
fn open_in(
    dir: &Path,
    env: &[(&str, &str)],
    flags: &[&str],
) -> (Output, Vec<fake_webdriver::Seen>) {
    let driver = FakeWebDriver::start();
    let mut args = vec!["--webdriver", driver.url()];
    args.extend(flags);
    args.extend(["open", "https://example.test/"]);
    let output = run(dir, env, &args);
    (output, driver.requests())
}

#[test]
fn a_plugin_flag_stops_a_browser_command_and_says_how_to_get_plugins() {
    let dir = tempfile::tempdir().expect("temp dir");

    let (output, requests) = open_in(dir.path(), &[], &["--plugin", "shield"]);

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stderr(&output).contains(REBUILD), "{}", stderr(&output));
    assert!(requests.is_empty(), "no browser was asked for");
}

#[test]
fn sb_plugins_and_the_plugins_table_stop_a_browser_command_too() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (output, requests) = open_in(dir.path(), &[("SB_PLUGINS", "shield")], &[]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stderr(&output).contains(REBUILD), "{}", stderr(&output));
    assert!(requests.is_empty());

    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "[plugins]\nenable = [\"shield\"]\n",
    )
    .unwrap();
    let (output, requests) = open_in(dir.path(), &[], &[]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stderr(&output).contains(REBUILD), "{}", stderr(&output));
    assert!(requests.is_empty());

    // An empty variable is an answer: the file's plugins are off, and the
    // command runs.
    let (output, requests) = open_in(dir.path(), &[("SB_PLUGINS", "")], &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!requests.is_empty());
}

#[test]
fn a_command_that_starts_no_browser_ignores_the_plugin_flags() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = run(dir.path(), &[], &["--plugin", "shield", "version"]);

    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn plugin_list_says_there_are_none_and_plugin_info_fails() {
    let dir = tempfile::tempdir().expect("temp dir");

    let list = run(dir.path(), &[], &["plugin", "list"]);
    assert!(list.status.success(), "{}", stderr(&list));
    assert!(stdout(&list).contains("No plugins"), "{}", stdout(&list));
    assert!(stdout(&list).contains(REBUILD), "{}", stdout(&list));

    let info = run(dir.path(), &[], &["plugin", "info", "shield"]);
    assert_eq!(info.status.code(), Some(2), "{}", stderr(&info));
    assert!(stderr(&info).contains(REBUILD), "{}", stderr(&info));
}

#[cfg(feature = "mcp-server")]
#[test]
fn sbrs_mcp_refuses_plugins_before_it_listens() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = sbrs::command_in(env!("CARGO_BIN_EXE_sbrs-mcp"), dir.path(), &[])
        .args(["--server", "cdp", "--plugin", "shield"])
        .output()
        .expect("sbrs-mcp runs");

    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(stderr(&output).contains(REBUILD), "{}", stderr(&output));
}
