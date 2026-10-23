//! The browser options as flags of `sbrs`, `SB_*` variables and keys of the settings file, run
//! as the real binary.
//!
//! The commands that drive a browser run against a fake WebDriver server (no browser), and the
//! tests read what the binary asked that server for. `sbrs test` runs against a stand-in
//! `cargo` that reports the `SB_*` variables it was given.

#[path = "support/fake_webdriver.rs"]
mod fake_webdriver;

use std::path::Path;
use std::process::{Command, Output};

use fake_webdriver::FakeWebDriver;
use seleniumbase_rs::config::browser_options::listed_options;
use seleniumbase_rs::config::test_options::OptionKind;
use serde_json::Value;

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// Runs `sbrs args` in `dir` with `env` set and every other `SB_*` variable cleared.
fn sbrs(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(SBASE);
    command
        .current_dir(dir)
        .args(args)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error");
    for (name, _) in std::env::vars() {
        if name.starts_with("SB_") && name != "SB_LOG_LEVEL" {
            command.env_remove(name);
        }
    }
    for (name, value) in env {
        command.env(name, value);
    }
    command.output().expect("sbrs runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// What `sbrs ... open` did against a fake WebDriver server.
struct Run {
    driver: FakeWebDriver,
    output: Output,
}

impl Run {
    /// The `goog:chromeOptions` of the new-session request.
    fn chrome_options(&self) -> Value {
        let requests = self.driver.requests();
        let session = requests
            .iter()
            .find(|request| request.method == "POST" && request.path == "/session")
            .unwrap_or_else(|| panic!("no new session: {}", stderr(&self.output)));
        session.body["capabilities"]["alwaysMatch"]["goog:chromeOptions"].clone()
    }

    fn args(&self) -> Vec<String> {
        self.chrome_options()["args"]
            .as_array()
            .map(|args| {
                args.iter()
                    .filter_map(|a| a.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn blocks_images(&self) -> bool {
        self.chrome_options()["prefs"]["profile.managed_default_content_settings.images"] == 2
    }
}

/// Runs `sbrs --webdriver <fake> <flags> open <page>` in `dir`.
fn open_in(dir: &Path, env: &[(&str, &str)], flags: &[&str]) -> Run {
    let driver = FakeWebDriver::start();
    let mut args = vec!["--webdriver", driver.url()];
    args.extend(flags);
    args.extend(["open", "https://example.test/"]);
    let output = sbrs(dir, env, &args);
    Run { driver, output }
}

fn open(env: &[(&str, &str)], flags: &[&str]) -> Run {
    let dir = tempfile::tempdir().expect("temp dir");
    open_in(dir.path(), env, flags)
}

#[test]
fn a_flag_of_the_table_reaches_the_new_session() {
    let run = open(&[], &["--block-images"]);

    assert!(run.output.status.success(), "{}", stderr(&run.output));
    assert!(run.blocks_images(), "{:?}", run.chrome_options());
}

#[test]
fn a_switch_can_be_turned_off_with_an_equals_sign() {
    let run = open(&[("SB_BLOCK_IMAGES", "true")], &["--block-images=false"]);

    assert!(run.output.status.success(), "{}", stderr(&run.output));
    assert!(!run.blocks_images(), "{:?}", run.chrome_options());
}

#[test]
fn a_python_spelling_with_underscores_is_the_same_flag() {
    let run = open(&[], &["--block_images"]);

    assert!(run.output.status.success(), "{}", stderr(&run.output));
    assert!(run.blocks_images());
}

#[test]
fn an_alias_of_a_row_is_the_same_flag() {
    let run = open(&[], &["--dark"]);

    assert!(run.output.status.success(), "{}", stderr(&run.output));
    assert!(
        run.args()
            .iter()
            .any(|arg| arg == "--enable-features=WebContentsForceDark"),
        "{:?}",
        run.args()
    );
}

#[test]
fn a_variable_of_the_table_reaches_the_new_session() {
    let run = open(&[("SB_DEVICE_METRICS", "360,640,2")], &[]);

    assert!(run.output.status.success(), "{}", stderr(&run.output));
    let emulation = run.chrome_options()["mobileEmulation"].to_string();
    assert!(
        emulation.contains("360") && emulation.contains("640"),
        "{emulation}"
    );
}

#[test]
fn the_settings_file_may_use_dashes_and_aliases_for_its_keys() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "block-images = true\ndark = true\n",
    )
    .expect("write the file");

    let run = open_in(dir.path(), &[], &[]);

    assert!(run.output.status.success(), "{}", stderr(&run.output));
    assert!(run.blocks_images());
    assert!(run
        .args()
        .iter()
        .any(|arg| arg == "--enable-features=WebContentsForceDark"));
}

#[test]
fn an_unknown_key_of_the_settings_file_is_refused_and_names_the_listing() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(dir.path().join("sbase_config.toml"), "blok_images = true\n")
        .expect("write the file");

    let run = open_in(dir.path(), &[], &[]);

    assert!(!run.output.status.success());
    let message = stderr(&run.output);
    assert!(
        message.contains("unknown setting \"blok_images\""),
        "{message}"
    );
    assert!(
        message.contains("`sbrs options` lists the browser options"),
        "{message}"
    );
    assert!(run.driver.requests().is_empty(), "no browser should start");
}

#[test]
fn the_flag_beats_the_variable_and_the_variable_beats_the_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "block_images = true\n",
    )
    .expect("write the file");

    let file = open_in(dir.path(), &[], &[]);
    assert!(file.blocks_images(), "the file turns it on");

    let variable = open_in(dir.path(), &[("SB_BLOCK_IMAGES", "false")], &[]);
    assert!(!variable.blocks_images(), "the variable turns it off");

    let flag = open_in(
        dir.path(),
        &[("SB_BLOCK_IMAGES", "false")],
        &["--block-images"],
    );
    assert!(flag.blocks_images(), "the flag turns it on again");
}

#[test]
fn a_chromium_arg_array_of_the_file_is_not_split_at_commas() {
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "chromium_arg = [\"--host-resolver-rules=MAP a.test 127.0.0.1,EXCLUDE b.test\"]\n",
    )
    .expect("write the file");

    let run = open_in(dir.path(), &[], &[]);

    assert!(run.output.status.success(), "{}", stderr(&run.output));
    assert!(
        run.args()
            .iter()
            .any(|arg| arg == "--host-resolver-rules=MAP a.test 127.0.0.1,EXCLUDE b.test"),
        "{:?}",
        run.args()
    );
}

#[test]
fn server_and_port_name_the_grid() {
    let driver = FakeWebDriver::start();
    let port = driver
        .url()
        .rsplit(':')
        .next()
        .expect("the address has a port")
        .to_owned();
    let dir = tempfile::tempdir().expect("temp dir");

    let _ = sbrs(
        dir.path(),
        &[],
        &[
            "--server",
            "127.0.0.1",
            "--port",
            &port,
            "open",
            "https://example.test/",
        ],
    );

    let paths: Vec<_> = driver.requests().into_iter().map(|r| r.path).collect();
    assert!(
        paths.iter().any(|path| path == "/wd/hub/session"),
        "the grid address is http://127.0.0.1:PORT/wd/hub: {paths:?}"
    );
}

#[test]
fn server_together_with_webdriver_is_refused_before_a_browser_starts() {
    let driver = FakeWebDriver::start();
    let dir = tempfile::tempdir().expect("temp dir");

    let output = sbrs(
        dir.path(),
        &[],
        &[
            "--webdriver",
            driver.url(),
            "--server",
            "grid.test",
            "open",
            "https://example.test/",
        ],
    );

    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("choose --webdriver or --server, not both"),
        "{}",
        stderr(&output)
    );
    assert!(driver.requests().is_empty());
}

#[test]
fn a_browser_and_a_browser_shortcut_are_refused_together() {
    let dir = tempfile::tempdir().expect("temp dir");

    let output = sbrs(
        dir.path(),
        &[],
        &[
            "--browser",
            "chrome",
            "--firefox",
            "open",
            "https://example.test/",
        ],
    );

    assert!(!output.status.success());
    assert!(stderr(&output).contains("not both"), "{}", stderr(&output));
}

#[test]
fn a_value_that_does_not_parse_is_an_error_and_no_browser_starts() {
    let run = open(&[], &["--metrics", "1,2"]);

    assert!(!run.output.status.success());
    assert!(!stderr(&run.output).is_empty());
    assert!(run.driver.requests().is_empty(), "no browser should start");
}

#[test]
fn options_lists_the_table() {
    let dir = tempfile::tempdir().expect("temp dir");

    let text = sbrs(dir.path(), &[], &["options"]);
    assert!(text.status.success(), "{}", stderr(&text));
    let listing = stdout(&text);
    assert!(listing.starts_with("FLAG"), "{listing}");
    assert!(listing.contains("--dark-mode, --dark"));
    assert!(listing.contains("SB_DARK_MODE"));
    assert!(
        !listing.contains("SB_PDB"),
        "the test options are listed by --all"
    );

    let all = stdout(&sbrs(dir.path(), &[], &["options", "--all"]));
    assert!(all.contains("SB_PDB"));

    let markdown = stdout(&sbrs(dir.path(), &[], &["options", "--markdown"]));
    assert!(markdown.starts_with("| Flag | Variable |"), "{markdown}");
    assert!(markdown.contains("`SB_BLOCK_IMAGES`"));
}

/// A value that every flag with a value accepts at parse time.
fn sample(kind: OptionKind) -> &'static str {
    match kind {
        OptionKind::Value("Strategy") => "eager",
        OptionKind::Value("Mode") => "cdp",
        OptionKind::Value("Browser") => "chrome",
        _ => "1",
    }
}

#[test]
fn every_browser_option_is_a_flag_of_the_root_under_each_spelling() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut checked = 0;
    for row in listed_options(false).filter(|row| row.kind.is_flag()) {
        for flag in row.flags() {
            let argument = match row.kind {
                OptionKind::Value(_) | OptionKind::Custom(_) => {
                    format!("--{flag}={}", sample(row.kind))
                }
                _ => format!("--{flag}"),
            };
            // `--help` answers before any setting is read: only the parser is under test.
            let output = sbrs(dir.path(), &[], &[&argument, "--help"]);
            assert!(
                output.status.success(),
                "{argument} is not accepted by the root: {}",
                stderr(&output)
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "{checked}");
}

#[cfg(unix)]
mod forwarding {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// A `cargo` that prints its `SB_*` variables.
    const FAKE_CARGO: &str = r#"#!/bin/sh
if [ "$1" = "--warm" ]; then exit 0; fi
if [ "$1" = "metadata" ]; then
  p=$(pwd -P)
  printf '{"packages":[{"id":"p 0.1.0","manifest_path":"%s/Cargo.toml","features":{},"targets":[{"name":"p","kind":["lib"]}]}],"workspace_members":["p 0.1.0"],"workspace_default_members":["p 0.1.0"]}\n' "$p"
  exit 0
fi
echo "args=$*"
env | while read -r line; do
  case "$line" in SB_*) echo "$line" ;; esac
done
exit 0
"#;

    fn run_test(args: &[&str]) -> Output {
        let bin = tempfile::tempdir().expect("temp dir");
        let work = tempfile::tempdir().expect("temp dir");
        let script = bin.path().join("cargo");
        std::fs::write(&script, FAKE_CARGO).expect("write the stand-in");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");
        // The first run of a new script can be slow on macOS: take it now.
        assert!(Command::new(&script)
            .arg("--warm")
            .status()
            .expect("warm it")
            .success());
        let mut command = Command::new(SBASE);
        command
            .args(args)
            .current_dir(work.path())
            .env("PATH", format!("{}:/usr/bin:/bin", bin.path().display()))
            .env_remove("NO_COLOR")
            .env_remove("RUST_LOG")
            .env("SB_LOG_LEVEL", "error");
        for (name, _) in std::env::vars() {
            if name.starts_with("SB_") && name != "SB_LOG_LEVEL" {
                command.env_remove(name);
            }
        }
        command.output().expect("sbrs runs")
    }

    #[test]
    fn the_flags_before_test_reach_the_tests_as_variables() {
        let output = run_test(&["--dark", "--headless", "--metrics", "360,640,2", "test"]);

        assert!(
            output.status.success(),
            "{}{}",
            stdout(&output),
            stderr(&output)
        );
        let printed = stdout(&output);
        for line in [
            "SB_DARK_MODE=true",
            "SB_HEADLESS=true",
            "SB_DEVICE_METRICS=360,640,2",
        ] {
            assert!(
                printed.lines().any(|found| found == line),
                "{line} in {printed}"
            );
        }
    }

    #[test]
    fn a_flag_after_test_wins_over_the_same_flag_before_it() {
        let output = run_test(&[
            "--device-metrics",
            "1,1,1",
            "test",
            "--device-metrics",
            "2,2,2",
        ]);

        assert!(
            output.status.success(),
            "{}{}",
            stdout(&output),
            stderr(&output)
        );
        let printed = stdout(&output);
        assert!(
            printed
                .lines()
                .any(|line| line == "SB_DEVICE_METRICS=2,2,2"),
            "{printed}"
        );
    }
}
