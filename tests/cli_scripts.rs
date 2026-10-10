//! The `sbase` banner, error reporting and `sbase test`, run as the real binary.
//!
//! `sbase test` starts Cargo, so these tests put a stand-in `cargo` first on
//! `PATH` that reports how it was called. No browser, network or real Cargo
//! run is involved.

use std::process::{Command, Output};

use seleniumbase_rs::cli::scripts::logo_helper::LOGO;

const SBASE: &str = env!("CARGO_BIN_EXE_sbase");

/// Every `SB_*` variable `sbase test` may set, cleared so the host's own
/// settings cannot leak into a test.
const SETTINGS_VARS: [&str; 14] = [
    "SB_BROWSER",
    "SB_HEADLESS",
    "SB_MODE",
    "SB_USER_AGENT",
    "SB_LOCALE",
    "SB_AD_BLOCK",
    "SB_PROXY",
    "SB_PROXY_PAC_URL",
    "SB_USER_DATA_DIR",
    "SB_EXTENSION_DIR",
    "SB_REUSE_SESSION",
    "SB_MOBILE",
    "SB_THREADS",
    "SB_WEBDRIVER_URL",
];

fn sbase(args: &[&str]) -> Command {
    let mut command = Command::new(SBASE);
    command
        .args(args)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .env("SB_LOG_LEVEL", "error");
    for var in SETTINGS_VARS {
        command.env_remove(var);
    }
    command
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn the_long_help_starts_with_the_banner() {
    let output = sbase(&["--help"]).output().expect("sbase runs");

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).starts_with(LOGO),
        "the banner comes first:\n{}",
        stdout(&output)
    );
}

#[test]
fn a_failure_is_reported_with_an_error_tag_and_a_failing_exit_code() {
    let output = sbase(&["--cdp", "--uc", "open", "https://example.test"])
        .output()
        .expect("sbase runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("[ERROR] Choose either --cdp or --uc, not both."),
        "{}",
        stderr(&output)
    );
    assert!(
        !stderr(&output).contains('\x1b'),
        "no colour when stderr is not a terminal"
    );
    assert!(stdout(&output).is_empty());
}

fn doctor_in(dir: &std::path::Path, chromedriver_path: Option<&str>) -> String {
    let mut command = sbase(&["doctor"]);
    // An empty PATH, so a chromedriver installed on this machine cannot be found.
    command
        .current_dir(dir)
        .env("PATH", dir)
        .env_remove("CHROMEDRIVER_PATH");
    if let Some(path) = chromedriver_path {
        command.env("CHROMEDRIVER_PATH", path);
    }
    let output = command.output().expect("sbase runs");
    assert!(output.status.success(), "{}", stderr(&output));
    stdout(&output)
}

#[test]
fn doctor_finds_the_driver_that_sbase_install_downloads() {
    let dir = tempfile::tempdir().expect("temp dir");
    // The process's working directory is reported canonicalised (on macOS
    // /var is /private/var), so compare against the canonical path.
    let root = dir.path().canonicalize().expect("canonical path");
    let drivers = root.join("downloaded_drivers");
    std::fs::create_dir(&drivers).expect("drivers dir");
    let name = if cfg!(windows) {
        "chromedriver.exe"
    } else {
        "chromedriver"
    };
    std::fs::write(drivers.join(name), b"not a real driver").expect("write driver");

    let report = doctor_in(&root, None);

    assert!(
        report.contains(&format!("Found: {}", drivers.join(name).display())),
        "{report}"
    );
}

#[test]
fn doctor_says_where_it_looked_when_there_is_no_driver() {
    let dir = tempfile::tempdir().expect("temp dir");

    let report = doctor_in(dir.path(), None);

    assert!(report.contains("downloaded_drivers/"), "{report}");
    assert!(report.contains("sbase install"), "{report}");
}

#[test]
fn doctor_reports_a_chromedriver_path_that_names_no_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("nope");

    let report = doctor_in(dir.path(), Some(missing.to_str().expect("utf-8 path")));

    assert!(
        report.contains("which is not a file") && report.contains("CHROMEDRIVER_PATH"),
        "{report}"
    );
}

#[cfg(unix)]
mod test_command {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A directory holding a `cargo` that prints how it was called and exits
    /// with `exit_code`.
    fn fake_cargo(exit_code: i32) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        let script = dir.path().join("cargo");
        let body = format!(
            "#!/bin/sh\n\
             echo \"args=$*\"\n\
             for name in SB_BROWSER SB_HEADLESS SB_MODE SB_PROXY SB_MOBILE SB_THREADS SB_WEBDRIVER_URL; do\n\
               eval \"value=\\${{$name-UNSET}}\"\n\
               echo \"$name=$value\"\n\
             done\n\
             exit {exit_code}\n"
        );
        std::fs::write(&script, body).expect("write script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");
        dir
    }

    fn run_with_fake_cargo(dir: &tempfile::TempDir, args: &[&str]) -> Output {
        let path = format!("{}:/usr/bin:/bin", dir.path().display());
        sbase(args).env("PATH", path).output().expect("sbase runs")
    }

    #[test]
    fn global_options_reach_the_tests_as_environment_variables() {
        let cargo = fake_cargo(0);

        let output = run_with_fake_cargo(
            &cargo,
            &[
                "--headless",
                "--browser",
                "firefox",
                "--uc",
                "-n",
                "2",
                "test",
                "login",
                "--",
                "--nocapture",
            ],
        );

        assert!(output.status.success(), "{}", stderr(&output));
        let out = stdout(&output);
        assert!(
            out.contains("args=test login -- --test-threads 2 --nocapture"),
            "{out}"
        );
        assert!(out.contains("SB_HEADLESS=true"), "{out}");
        assert!(out.contains("SB_BROWSER=firefox"), "{out}");
        assert!(out.contains("SB_MODE=uc"), "{out}");
        assert!(out.contains("SB_THREADS=2"), "{out}");
    }

    #[test]
    fn options_that_were_not_given_are_not_exported() {
        let cargo = fake_cargo(0);

        let output = run_with_fake_cargo(&cargo, &["test"]);

        let out = stdout(&output);
        assert!(out.contains("args=test\n"), "{out}");
        for name in [
            "SB_BROWSER",
            "SB_HEADLESS",
            "SB_MODE",
            "SB_PROXY",
            "SB_MOBILE",
            "SB_THREADS",
            "SB_WEBDRIVER_URL",
        ] {
            assert!(out.contains(&format!("{name}=UNSET")), "{name} in {out}");
        }
    }

    #[test]
    fn a_non_default_webdriver_url_is_exported() {
        let cargo = fake_cargo(0);

        let output = run_with_fake_cargo(
            &cargo,
            &["--webdriver", "http://grid.test:4444", "test", "--release"],
        );

        let out = stdout(&output);
        assert!(out.contains("args=test --release"), "{out}");
        assert!(
            out.contains("SB_WEBDRIVER_URL=http://grid.test:4444"),
            "{out}"
        );
    }

    #[test]
    fn the_exit_code_of_cargo_is_the_exit_code_of_sbase() {
        let cargo = fake_cargo(7);

        let output = run_with_fake_cargo(&cargo, &["test"]);

        assert_eq!(output.status.code(), Some(7));
    }

    #[test]
    fn an_example_is_run_with_cargo_run() {
        let cargo = fake_cargo(0);

        let output = run_with_fake_cargo(&cargo, &["test", "--example", "basic_test"]);

        assert!(
            stdout(&output).contains("args=run --example basic_test"),
            "{}",
            stdout(&output)
        );
    }

    fn proxy_file(text: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join("proxies.txt"), text).expect("write list");
        dir
    }

    #[test]
    fn a_name_from_the_proxy_list_becomes_the_proxy() {
        let cargo = fake_cargo(0);
        let list = proxy_file("proxy1 = 10.0.0.5:3128\nproxy2 = 10.0.0.6:3128\n");
        let file = list.path().join("proxies.txt");

        let output = run_with_fake_cargo(
            &cargo,
            &[
                "--proxy-list",
                file.to_str().unwrap(),
                "--proxy",
                "proxy2",
                "test",
            ],
        );

        assert!(output.status.success(), "{}", stderr(&output));
        assert!(
            stdout(&output).contains("SB_PROXY=10.0.0.6:3128"),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn without_a_name_a_member_of_the_list_is_used_and_announced_without_its_password() {
        let cargo = fake_cargo(0);
        let list = proxy_file("only = alice:topsecret@10.0.0.5:3128\n");
        let file = list.path().join("proxies.txt");

        let output = run_with_fake_cargo(&cargo, &["--proxy-list", file.to_str().unwrap(), "test"]);

        assert!(
            stdout(&output).contains("SB_PROXY=alice:topsecret@10.0.0.5:3128"),
            "the tests need the login"
        );
        assert!(stderr(&output).contains("[INFO] Using proxy only (10.0.0.5:3128)"));
        assert!(
            !stderr(&output).contains("topsecret"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn an_unknown_proxy_name_is_an_error_naming_the_choices() {
        let cargo = fake_cargo(0);
        let list = proxy_file("proxy1 = 10.0.0.5:3128\n");
        let file = list.path().join("proxies.txt");

        let output = run_with_fake_cargo(
            &cargo,
            &[
                "--proxy-list",
                file.to_str().unwrap(),
                "--proxy",
                "nope",
                "test",
            ],
        );

        assert!(!output.status.success());
        assert!(stdout(&output).is_empty(), "cargo must not have run");
        assert!(
            stderr(&output).contains("known names: proxy1"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn a_bad_line_in_the_proxy_list_is_reported_with_its_number() {
        let cargo = fake_cargo(0);
        let list = proxy_file("proxy1 = 10.0.0.5:3128\nbroken\n");
        let file = list.path().join("proxies.txt");

        let output = run_with_fake_cargo(&cargo, &["--proxy-list", file.to_str().unwrap(), "test"]);

        assert!(!output.status.success());
        assert!(stderr(&output).contains("line 2"), "{}", stderr(&output));
    }

    #[test]
    fn a_filter_and_an_example_cannot_be_combined() {
        let cargo = fake_cargo(0);

        let output = run_with_fake_cargo(&cargo, &["test", "login", "--example", "basic_test"]);

        assert!(!output.status.success());
        assert!(stdout(&output).is_empty(), "cargo must not have run");
    }
}
