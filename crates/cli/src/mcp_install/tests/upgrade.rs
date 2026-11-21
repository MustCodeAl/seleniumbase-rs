//! Tests of `sbrs-mcp upgrade` against temporary folders. The programs it would look at or start
//! (`brew`, `git`, the upgrade command) go through a [`Scripted`] runner that answers from a list
//! and records what it was asked to run; nothing here starts a real program.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::repair::{config, current, launch, program};
use super::{read, Outcome, Sandbox};
use crate::mcp_install::apply::Scope;
use crate::mcp_install::client::Client;
use crate::mcp_install::external::Cmd;
use crate::mcp_install::fsx::backup_path;
use crate::mcp_install::runner::Runner;
use crate::mcp_install::upgrade::{detect_method, upgrade, InstallMethod, UpgradeRequest};

const RELEASES: &str = "https://github.com/MustCodeAl/seleniumbase-rs/releases";

/// The refs of the repository as `git ls-remote --tags --refs` prints them: a prerelease and a
/// tag that is no version must not win, and 0.8.10 is later than 0.8.6.
const TAGS: &str = "a1\trefs/tags/v0.7.0\nb2\trefs/tags/v0.8.6\nc3\trefs/tags/v0.9.0-rc1\n\
                    d4\trefs/tags/v0.8.10\ne5\trefs/tags/nightly\n";

const CARGO_COMMAND: &str = "cargo install --git https://github.com/MustCodeAl/seleniumbase-rs \
                             --tag v0.8.10 seleniumbase-rs-cli --locked";

/// A runner that answers `output` from `outputs` (the first whose key the command line
/// contains) and records every command `run` is given.
#[derive(Debug, Default)]
struct Scripted {
    outputs: Vec<(&'static str, String)>,
    run_error: Option<String>,
    ran: Mutex<Vec<String>>,
}

impl Scripted {
    fn ran(&self) -> Vec<String> {
        self.ran.lock().unwrap().clone()
    }
}

impl Runner for Scripted {
    fn run(&self, cmd: &Cmd) -> Result<(), String> {
        self.ran.lock().unwrap().push(cmd.display());
        match &self.run_error {
            Some(message) => Err(message.clone()),
            None => Ok(()),
        }
    }

    fn output(&self, cmd: &Cmd) -> Option<String> {
        let line = cmd.display();
        self.outputs
            .iter()
            .find(|(key, _)| line.contains(key))
            .map(|(_, text)| text.clone())
    }
}

/// Gives the machine `scripted` as its runner and returns it, to look at what it ran.
fn script(sb: &mut Sandbox, scripted: Scripted) -> Arc<Scripted> {
    let runner = Arc::new(scripted);
    sb.env.runner = runner.clone();
    runner
}

fn brew_has_sbrs() -> Scripted {
    Scripted {
        outputs: vec![("brew list sbrs", "/opt/homebrew/Cellar/sbrs\n".to_owned())],
        ..Scripted::default()
    }
}

fn git_has_tags() -> Scripted {
    Scripted {
        outputs: vec![("ls-remote", TAGS.to_owned())],
        ..Scripted::default()
    }
}

fn request(exe: &Path) -> UpgradeRequest {
    UpgradeRequest {
        clients: Vec::new(),
        exe: exe.to_path_buf(),
        name: "seleniumbase".to_owned(),
        dry_run: false,
        run: false,
    }
}

fn run(sb: &Sandbox, request: &UpgradeRequest) -> Outcome {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = upgrade(&sb.env, request, &mut out, &mut err);
    Outcome {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

/// A program inside a Homebrew Cellar of the machine.
fn cellar_program(sb: &Sandbox) -> PathBuf {
    let exe = sb
        .root
        .join("brew")
        .join("Cellar")
        .join("sbrs")
        .join("0.8.5")
        .join("bin")
        .join("sbrs-mcp");
    program(&exe, true);
    exe
}

/// A program in the `bin` folder of Cargo's home.
fn cargo_program(sb: &Sandbox) -> PathBuf {
    let exe = sb.env.cargo_home.join("bin").join("sbrs-mcp");
    program(&exe, true);
    exe
}

fn entry_of(sb: &Sandbox, name: &str) -> crate::mcp_install::entry::LaunchSpec {
    sb.entries(Client::Cursor, Scope::User)[name]
        .clone()
        .expect("a program entry")
}

// -- the entries ----------------------------------------------------------------------------

#[test]
fn a_stale_entry_is_rewritten_to_this_program_with_its_options_and_other_servers_stay() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let browser_args = [
        "--server",
        "driver",
        "--plugin",
        "shield",
        "--plugin-opt",
        "shield.level=\"strict\"",
    ];
    let before = config(
        Client::Cursor,
        &[
            ("other", launch("/usr/bin/other", &["--flag"])),
            ("seleniumbase", launch("/old/sbrs-mcp", &browser_args)),
            (
                "seleniumbase-dev",
                launch("/old/sbrs-mcp", &["--server", "dev"]),
            ),
        ],
    );
    sb.put(&file, &before);

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("replaced"), "{}", outcome.out);
    assert_eq!(
        entry_of(&sb, "seleniumbase"),
        launch(exe.to_str().unwrap(), &browser_args)
    );
    assert_eq!(
        entry_of(&sb, "seleniumbase-dev"),
        launch(exe.to_str().unwrap(), &["--server", "dev"])
    );
    assert_eq!(
        entry_of(&sb, "other"),
        launch("/usr/bin/other", &["--flag"]),
        "another server is not touched"
    );
    assert_eq!(read(&backup_path(&file)), before, "the old file is kept");
}

#[test]
fn an_entry_that_is_current_is_unchanged_and_its_file_is_not_written() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let before = config(
        Client::Cursor,
        &[(
            "seleniumbase",
            launch(exe.to_str().unwrap(), &["--server", "cdp"]),
        )],
    );
    sb.put(&file, &before);

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("unchanged"), "{}", outcome.out);
    assert_eq!(read(&file), before);
    assert!(!backup_path(&file).exists());
}

#[test]
fn an_entry_with_arguments_install_does_not_write_keeps_them_and_gets_the_new_program() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let args = [
        "--server",
        "cdp",
        "--transport",
        "http",
        "--listen",
        "127.0.0.1:0",
    ];
    sb.put(
        &file,
        &config(
            Client::Cursor,
            &[("seleniumbase", launch("/old/sbrs-mcp", &args))],
        ),
    );

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert_eq!(
        entry_of(&sb, "seleniumbase"),
        launch(exe.to_str().unwrap(), &args)
    );
}

#[test]
fn an_entry_that_does_not_start_a_local_program_is_left_alone() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let before = "{\n  \"mcpServers\": {\n    \"seleniumbase\": { \"url\": \"http://127.0.0.1:8931/mcp\" }\n  }\n}\n";
    sb.put(&file, before);

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert_eq!(read(&file), before);
}

#[test]
fn a_dry_run_rewrites_nothing_and_runs_nothing_even_with_run() {
    let mut sb = Sandbox::mac();
    let exe = cellar_program(&sb);
    let runner = script(&mut sb, Scripted::default());
    let file = sb.user(Client::Cursor);
    let before = config(
        Client::Cursor,
        &[(
            "seleniumbase",
            launch("/old/sbrs-mcp", &["--server", "cdp"]),
        )],
    );
    sb.put(&file, &before);
    let mut request = request(&exe);
    request.dry_run = true;
    request.run = true;

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("would be replaced"), "{}", outcome.out);
    assert!(
        outcome.out.contains("would run: brew upgrade sbrs"),
        "{}",
        outcome.out
    );
    assert_eq!(read(&file), before);
    assert!(!backup_path(&file).exists());
    assert!(runner.ran().is_empty(), "{:?}", runner.ran());
}

#[test]
fn the_client_flag_limits_the_rewrite_to_the_clients_named() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let stale = launch("/old/sbrs-mcp", &["--server", "cdp"]);
    sb.put(
        &sb.user(Client::Cursor),
        &config(Client::Cursor, &[("seleniumbase", stale.clone())]),
    );
    let zed = sb.user(Client::Zed);
    let zed_before = config(Client::Zed, &[("seleniumbase", stale)]);
    sb.put(&zed, &zed_before);
    let mut request = request(&exe);
    request.clients = vec!["cursor".to_owned()];

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert_eq!(entry_of(&sb, "seleniumbase").command, exe.to_str().unwrap());
    assert_eq!(read(&zed), zed_before);
}

#[test]
fn a_config_that_cannot_be_read_is_not_touched_and_the_others_are_rewritten() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let cursor = sb.user(Client::Cursor);
    sb.put(&cursor, "{ not json");
    let zed = sb.user(Client::Zed);
    sb.put(
        &zed,
        &config(
            Client::Zed,
            &[(
                "seleniumbase",
                launch("/old/sbrs-mcp", &["--server", "cdp"]),
            )],
        ),
    );

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 2, "{}{}", outcome.out, outcome.err);
    assert_eq!(read(&cursor), "{ not json");
    assert!(!backup_path(&cursor).exists());
    let entries =
        crate::mcp_install::jsonfile::read_existing(&zed, &Client::Zed.layout().unwrap()).unwrap();
    assert_eq!(
        entries["seleniumbase"].as_ref().unwrap().command,
        exe.to_str().unwrap()
    );
}

#[test]
fn an_unknown_client_is_refused_and_nothing_runs() {
    let mut sb = Sandbox::mac();
    let exe = cellar_program(&sb);
    let runner = script(&mut sb, brew_has_sbrs());
    let mut request = request(&exe);
    request.clients = vec!["nosuch".to_owned()];
    request.run = true;

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 2);
    assert!(outcome.err.contains("`nosuch`"), "{}", outcome.err);
    assert!(runner.ran().is_empty());
}

// -- how the program was installed ----------------------------------------------------------

#[test]
fn a_program_in_a_homebrew_cellar_is_a_homebrew_install() {
    let sb = Sandbox::mac();
    assert_eq!(
        detect_method(&sb.env, &cellar_program(&sb)),
        InstallMethod::Homebrew
    );
}

#[test]
fn a_program_elsewhere_is_a_homebrew_install_when_brew_lists_sbrs() {
    let mut sb = Sandbox::mac();
    let exe = current(&sb);
    script(&mut sb, brew_has_sbrs());
    assert_eq!(detect_method(&sb.env, &exe), InstallMethod::Homebrew);
}

#[test]
fn a_program_in_the_cargo_bin_folder_is_a_cargo_install() {
    let sb = Sandbox::mac();
    assert_eq!(
        detect_method(&sb.env, &cargo_program(&sb)),
        InstallMethod::Cargo
    );
}

#[test]
fn a_cargo_path_wins_over_a_brew_list_that_succeeds() {
    let mut sb = Sandbox::mac();
    let exe = cargo_program(&sb);
    script(&mut sb, brew_has_sbrs());
    assert_eq!(
        detect_method(&sb.env, &exe),
        InstallMethod::Cargo,
        "the program that runs is the one cargo installed"
    );
}

#[test]
fn any_other_program_is_a_release_archive() {
    let mut sb = Sandbox::mac();
    let exe = current(&sb);
    let runner = script(&mut sb, Scripted::default());
    assert_eq!(detect_method(&sb.env, &exe), InstallMethod::Archive);
    assert!(runner.ran().is_empty(), "looking at it runs nothing");
}

// -- the command that upgrades the program --------------------------------------------------

#[test]
fn homebrew_prints_brew_upgrade_and_runs_nothing_without_run() {
    let mut sb = Sandbox::mac();
    let exe = cellar_program(&sb);
    let runner = script(&mut sb, Scripted::default());

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("Homebrew"), "{}", outcome.out);
    assert!(outcome.out.contains("brew upgrade sbrs"), "{}", outcome.out);
    assert!(runner.ran().is_empty(), "{:?}", runner.ran());
}

#[test]
fn run_runs_brew_upgrade_and_nothing_else() {
    let mut sb = Sandbox::mac();
    let exe = cellar_program(&sb);
    let runner = script(&mut sb, Scripted::default());
    let mut request = request(&exe);
    request.run = true;

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert_eq!(runner.ran(), ["brew upgrade sbrs"]);
}

#[test]
fn a_failing_upgrade_command_is_exit_1_and_says_why() {
    let mut sb = Sandbox::mac();
    let exe = cellar_program(&sb);
    let runner = script(
        &mut sb,
        Scripted {
            run_error: Some("`brew upgrade sbrs` exited with exit status: 1".to_owned()),
            ..Scripted::default()
        },
    );
    let mut request = request(&exe);
    request.run = true;

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 1, "{}{}", outcome.out, outcome.err);
    assert!(outcome.err.contains("exited with"), "{}", outcome.err);
    assert_eq!(runner.ran().len(), 1);
}

#[test]
fn cargo_prints_the_install_command_with_the_latest_release_tag() {
    let mut sb = Sandbox::mac();
    let exe = cargo_program(&sb);
    let runner = script(&mut sb, git_has_tags());

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("cargo install"), "{}", outcome.out);
    assert!(outcome.out.contains(CARGO_COMMAND), "{}", outcome.out);
    assert!(runner.ran().is_empty(), "{:?}", runner.ran());
}

#[test]
fn run_runs_the_cargo_install_command_it_printed() {
    let mut sb = Sandbox::mac();
    let exe = cargo_program(&sb);
    let runner = script(&mut sb, git_has_tags());
    let mut request = request(&exe);
    request.run = true;

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert_eq!(runner.ran(), [CARGO_COMMAND]);
}

#[test]
fn cargo_without_a_tag_prints_a_placeholder_and_run_refuses_to_run_it() {
    let mut sb = Sandbox::mac();
    let exe = cargo_program(&sb);
    let runner = script(&mut sb, Scripted::default());

    let printed = run(&sb, &request(&exe));
    assert_eq!(printed.code, 0, "{}{}", printed.out, printed.err);
    assert!(
        printed.out.contains("--tag <latest tag>"),
        "{}",
        printed.out
    );
    assert!(printed.out.contains(RELEASES), "{}", printed.out);

    let mut request = request(&exe);
    request.run = true;
    let refused = run(&sb, &request);
    assert_eq!(refused.code, 2, "{}{}", refused.out, refused.err);
    assert!(runner.ran().is_empty(), "{:?}", runner.ran());
}

#[test]
fn a_release_archive_gets_the_release_page_and_run_has_nothing_to_run() {
    let mut sb = Sandbox::mac();
    let exe = current(&sb);
    let runner = script(&mut sb, Scripted::default());

    let printed = run(&sb, &request(&exe));
    assert_eq!(printed.code, 0, "{}{}", printed.out, printed.err);
    assert!(printed.out.contains(RELEASES), "{}", printed.out);
    assert!(
        printed.out.contains(exe.to_str().unwrap()),
        "it names the program to replace: {}",
        printed.out
    );

    let mut request = request(&exe);
    request.run = true;
    let refused = run(&sb, &request);
    assert_eq!(refused.code, 2, "{}{}", refused.out, refused.err);
    assert!(refused.err.contains("--run"), "{}", refused.err);
    assert!(runner.ran().is_empty());
}

#[test]
fn the_entries_are_rewritten_before_the_command_runs() {
    let mut sb = Sandbox::mac();
    let exe = cargo_program(&sb);
    script(&mut sb, git_has_tags());
    sb.put(
        &sb.user(Client::Cursor),
        &config(
            Client::Cursor,
            &[(
                "seleniumbase",
                launch("/old/sbrs-mcp", &["--server", "cdp"]),
            )],
        ),
    );
    let mut request = request(&exe);
    request.run = true;

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    let rewritten = outcome
        .out
        .find("replaced")
        .expect("the rewrite is reported");
    let command = outcome
        .out
        .find("cargo install")
        .expect("the command is reported");
    assert!(rewritten < command, "{}", outcome.out);
    assert_eq!(entry_of(&sb, "seleniumbase").command, exe.to_str().unwrap());
}

#[test]
fn the_program_in_a_cellar_is_registered_through_the_link_that_survives_an_upgrade() {
    let sb = Sandbox::mac();
    let cellar = cellar_program(&sb);
    let link = sb.root.join("brew").join("bin").join("sbrs-mcp");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&cellar, &link).unwrap();
        assert_eq!(crate::mcp_install::env::stable_path(cellar.clone()), link);
    }
    // Without the link, or outside a Cellar, the path stays as it was.
    let lone = sb.root.join("elsewhere").join("sbrs-mcp");
    program(&lone, true);
    assert_eq!(crate::mcp_install::env::stable_path(lone.clone()), lone);
    let _ = fs::remove_file(&link);
    assert_eq!(crate::mcp_install::env::stable_path(cellar.clone()), cellar);
}
