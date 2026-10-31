//! `scripts/check-consumers.sh`, run on a fixture: a made-up consumer and two versions of its
//! contract file (`tests/fixtures/contracts/`). The script is how a contract file is kept from
//! lagging behind the repository it describes, so the part to test is the part that decides:
//! reading the imports of the consumer, and failing, by name, on the one the contract lacks.
//!
//! The real checkouts are not needed: the script takes the folder of each consumer and of the
//! contract files from the environment.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures() -> PathBuf {
    root().join("tests/fixtures/contracts")
}

/// The script, run in a clean environment with the fixture folders for the consumers it is given.
fn script(contracts: &str, args: &[&str]) -> Output {
    let fixtures = fixtures();
    let mut command = Command::new("bash");
    command
        .arg(root().join("scripts/check-consumers.sh"))
        .args(args)
        .env("SBRS_STEALTH_DIR", fixtures.join("consumer"))
        .env("SBRS_TAP_DIR", fixtures.join("tap"))
        .env("SBRS_CONTRACTS_DIR", fixtures.join(contracts))
        .env("SBRS_CLI_CONTRACTS_DIR", fixtures.join(contracts))
        .env_remove("SBRS_COMPANION_DIR")
        .env_remove("SBRS_PLUGINS_DIR")
        .env_remove("SBRS_TAURI_DIR");
    command.output().expect("bash runs the script")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn a_path_the_contract_lacks_fails_the_check_and_is_named() {
    let output = script("incomplete", &["stealth"]);
    let (out, err) = (text(&output.stdout), text(&output.stderr));

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {out}\nstderr: {err}"
    );
    assert!(
        err.contains("seleniumbase_rs::imaginary::Widget"),
        "the missing path is named: {err}"
    );
    assert!(
        err.contains("stealth") && err.contains("contract_stealth.rs"),
        "the consumer and its contract file are named: {err}"
    );
    assert_eq!(
        err.lines().filter(|line| line.starts_with("FAIL")).count(),
        1,
        "only the missing path fails (the one named in a comment of the contract does not count): {err}"
    );
}

#[test]
fn a_contract_that_lists_every_path_passes() {
    let output = script("complete", &["stealth"]);
    let (out, err) = (text(&output.stdout), text(&output.stderr));

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout: {out}\nstderr: {err}"
    );
    assert!(out.contains("ok   stealth: 12 paths"), "{out}");
    assert!(err.is_empty(), "{err}");
}

#[test]
fn every_shape_of_import_is_read_into_its_paths() {
    let output = script("complete", &["--list", "stealth"]);
    assert!(output.status.success(), "{}", text(&output.stderr));

    // Groups over two lines and nested, `as`, `self`, plain paths, paths in code and in a
    // rustdoc link, and the struct literal that is not a group; the run-time list is skipped.
    assert_eq!(
        text(&output.stdout),
        "seleniumbase_rs::ProxyConfig\n\
         seleniumbase_rs::ProxyConfig::default\n\
         seleniumbase_rs::error::SeleniumBaseError\n\
         seleniumbase_rs::evasion::fingerprint\n\
         seleniumbase_rs::evasion::fingerprint::Fingerprint\n\
         seleniumbase_rs::evasion::fingerprint::OsType\n\
         seleniumbase_rs::imaginary::Widget\n\
         seleniumbase_rs::sb_cdp::Browser\n\
         seleniumbase_rs::sb_cdp::Page\n\
         seleniumbase_rs::sb_cdp::Page::goto\n\
         seleniumbase_rs::sb_cdp::Proxy::has_credentials\n\
         seleniumbase_rs::utils::urls::normalize_page_url\n"
    );
}

#[test]
fn a_string_the_formula_relies_on_and_the_contract_lacks_is_named() {
    let incomplete = script("incomplete", &["homebrew"]);
    let err = text(&incomplete.stderr);
    assert_eq!(incomplete.status.code(), Some(1), "{err}");
    assert!(err.contains("\"nushell\""), "{err}");
    assert_eq!(
        err.lines().filter(|line| line.starts_with("FAIL")).count(),
        1,
        "{err}"
    );

    let complete = script("complete", &["homebrew"]);
    assert_eq!(
        complete.status.code(),
        Some(0),
        "{}",
        text(&complete.stderr)
    );
    assert!(
        text(&complete.stdout).contains("ok   homebrew: 6 strings"),
        "{}",
        text(&complete.stdout)
    );
}

#[test]
fn a_consumer_that_is_not_checked_out_fails_naming_the_variable() {
    let missing = Path::new("/nonexistent/seleniumbase-rs-stealth");
    let output = script("complete", &["stealth"]);
    assert!(output.status.success());

    let mut command = Command::new("bash");
    command
        .arg(root().join("scripts/check-consumers.sh"))
        .arg("stealth")
        .env("SBRS_STEALTH_DIR", missing)
        .env("SBRS_CONTRACTS_DIR", fixtures().join("complete"));
    let output = command.output().expect("bash runs the script");

    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("no checkout at /nonexistent"));
}

#[test]
fn a_wrong_command_line_is_refused_with_status_2() {
    let output = script("complete", &["no-such-consumer"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("unknown argument"));
}
