//! Tests of `sbrs-mcp repair` against temporary folders. Every path comes from an [`Env`] rooted
//! in a temporary folder, so no config of the machine is read or written; the programs the
//! entries name are files this test makes.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::{Outcome, Sandbox, read};
use crate::mcp_install::apply::Scope;
use crate::mcp_install::client::Client;
use crate::mcp_install::entry::LaunchSpec;
use crate::mcp_install::fsx::backup_path;
use crate::mcp_install::repair::{Finding, Issue, RepairRequest, examine, repair};

/// Makes a file that stands for a program; `executable` sets its mode bits on Unix.
pub(super) fn program(path: &Path, executable: bool) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if executable { 0o755 } else { 0o644 };
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = executable;
}

/// The program `repair` is told is this one.
pub(super) fn current(sb: &Sandbox) -> PathBuf {
    let exe = sb.root.join("current").join("sbrs-mcp");
    program(&exe, true);
    exe
}

pub(super) fn launch(command: &str, args: &[&str]) -> LaunchSpec {
    LaunchSpec {
        command: command.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        env: [("SB_MCP_OUTPUT_DIR".to_owned(), "/data/out".to_owned())].into(),
    }
}

/// The config text of a JSON client holding `entries`, in that order.
pub(super) fn config(client: Client, entries: &[(&str, LaunchSpec)]) -> String {
    let layout = client.layout().unwrap();
    let servers: Map<String, Value> = entries
        .iter()
        .map(|(name, spec)| ((*name).to_owned(), layout.entry(spec)))
        .collect();
    let mut root = Map::new();
    root.insert(layout.key.to_owned(), Value::Object(servers));
    let mut text = serde_json::to_string_pretty(&Value::Object(root)).unwrap();
    text.push('\n');
    text
}

fn request(exe: &Path) -> RepairRequest {
    RepairRequest {
        clients: Vec::new(),
        exe: exe.to_path_buf(),
        name: "seleniumbase".to_owned(),
        dry_run: false,
    }
}

fn run(sb: &Sandbox, request: &RepairRequest) -> Outcome {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = repair(&sb.env, request, &mut out, &mut err);
    Outcome {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn issues(sb: &Sandbox, request: &RepairRequest, client: Client, scope: Scope) -> Vec<Issue> {
    examine(&sb.env, request, client, scope)
        .expect("the config is there")
        .issues
}

fn issue(entry: &str, finding: Finding) -> Issue {
    Issue {
        entry: Some(entry.to_owned()),
        finding,
    }
}

/// The command of the entry `name` in a Cursor config.
fn command_of(sb: &Sandbox, name: &str) -> String {
    let entries = sb.entries(Client::Cursor, Scope::User);
    entries[name].as_ref().unwrap().command.clone()
}

// -- what is found and fixed ----------------------------------------------------------------

#[test]
fn a_program_that_is_not_there_is_found_and_the_entry_points_at_this_program() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let stale = launch("/gone/sbrs-mcp", &["--server", "cdp", "--plugin", "shield"]);
    let before = config(
        Client::Cursor,
        &[
            ("other", launch("/usr/bin/other", &[])),
            ("seleniumbase", stale.clone()),
        ],
    );
    sb.put(&file, &before);
    let request = request(&exe);

    assert_eq!(
        issues(&sb, &request, Client::Cursor, Scope::User),
        [issue(
            "seleniumbase",
            Finding::BinaryMissing {
                path: "/gone/sbrs-mcp".to_owned()
            }
        )]
    );
    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("/gone/sbrs-mcp"), "{}", outcome.out);
    assert!(outcome.out.contains("fixed"), "{}", outcome.out);
    let entries = sb.entries(Client::Cursor, Scope::User);
    let fixed = entries["seleniumbase"].as_ref().unwrap();
    assert_eq!(fixed.command, exe.to_str().unwrap());
    assert_eq!(fixed.args, stale.args, "the arguments stay");
    assert_eq!(fixed.env, stale.env, "the environment stays");
    assert_eq!(
        entries["other"],
        Some(launch("/usr/bin/other", &[])),
        "another server is not touched"
    );
    assert_eq!(read(&backup_path(&file)), before, "the old file is kept");
    assert!(
        issues(&sb, &request, Client::Cursor, Scope::User).is_empty(),
        "nothing is left to find"
    );
}

#[test]
fn a_dry_run_reports_what_would_be_fixed_and_writes_nothing() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let before = config(
        Client::Cursor,
        &[(
            "seleniumbase",
            launch("/gone/sbrs-mcp", &["--server", "cdp"]),
        )],
    );
    sb.put(&file, &before);
    let mut request = request(&exe);
    request.dry_run = true;

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 1, "something is left broken: {}", outcome.out);
    assert!(outcome.out.contains("would be fixed"), "{}", outcome.out);
    assert!(outcome.out.contains("/gone/sbrs-mcp"), "{}", outcome.out);
    assert_eq!(read(&file), before, "the file is as it was");
    assert!(!backup_path(&file).exists(), "no backup for no write");
}

#[test]
fn a_config_with_nothing_wrong_is_left_alone_and_exits_zero() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let healthy = launch(exe.to_str().unwrap(), &["--server", "cdp"]);
    let before = config(Client::Cursor, &[("seleniumbase", healthy)]);
    sb.put(&file, &before);

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("nothing to repair"), "{}", outcome.out);
    assert_eq!(read(&file), before);
    assert!(!backup_path(&file).exists());
}

#[test]
fn nothing_to_check_is_not_an_error() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let outcome = run(&sb, &request(&exe));
    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("nothing to repair"), "{}", outcome.out);
}

#[test]
fn a_file_that_is_not_there_for_a_named_client_is_said_and_is_not_an_error() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let mut request = request(&exe);
    request.clients = vec!["zed".to_owned()];
    let outcome = run(&sb, &request);
    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("zed"), "{}", outcome.out);
}

#[test]
fn another_sbrs_mcp_is_pointed_at_this_one() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let older = sb.root.join("older").join("sbrs-mcp");
    program(&older, true);
    let file = sb.user(Client::Cursor);
    sb.put(
        &file,
        &config(
            Client::Cursor,
            &[(
                "seleniumbase",
                launch(older.to_str().unwrap(), &["--server", "cdp"]),
            )],
        ),
    );
    let request = request(&exe);

    assert_eq!(
        issues(&sb, &request, Client::Cursor, Scope::User),
        [issue(
            "seleniumbase",
            Finding::OtherBinary {
                path: older.to_str().unwrap().to_owned()
            }
        )]
    );
    assert_eq!(run(&sb, &request).code, 0);
    assert_eq!(command_of(&sb, "seleniumbase"), exe.to_str().unwrap());
}

#[cfg(unix)]
#[test]
fn a_program_that_cannot_be_run_is_found() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let flat = sb.root.join("flat").join("sbrs-mcp");
    program(&flat, false);
    let file = sb.user(Client::Cursor);
    sb.put(
        &file,
        &config(
            Client::Cursor,
            &[(
                "seleniumbase",
                launch(flat.to_str().unwrap(), &["--server", "cdp"]),
            )],
        ),
    );
    let request = request(&exe);

    assert_eq!(
        issues(&sb, &request, Client::Cursor, Scope::User),
        [issue(
            "seleniumbase",
            Finding::BinaryNotExecutable {
                path: flat.to_str().unwrap().to_owned()
            }
        )]
    );
    assert_eq!(run(&sb, &request).code, 0);
    assert_eq!(command_of(&sb, "seleniumbase"), exe.to_str().unwrap());
}

#[cfg(unix)]
#[test]
fn a_program_named_without_a_path_is_found_and_made_absolute() {
    let mut sb = Sandbox::mac();
    let exe = current(&sb);
    let bin = exe.parent().unwrap().to_path_buf();
    program(&bin.join("sbrs-mcp"), true);
    sb.env.path = Some(bin.into_os_string());
    let file = sb.user(Client::Cursor);
    sb.put(
        &file,
        &config(
            Client::Cursor,
            &[("seleniumbase", launch("sbrs-mcp", &["--server", "cdp"]))],
        ),
    );
    let request = request(&exe);

    assert_eq!(
        issues(&sb, &request, Client::Cursor, Scope::User),
        [issue(
            "seleniumbase",
            Finding::RelativeCommand {
                command: "sbrs-mcp".to_owned()
            }
        )]
    );
    assert_eq!(run(&sb, &request).code, 0);
    assert_eq!(command_of(&sb, "seleniumbase"), exe.to_str().unwrap());
}

#[test]
fn an_entry_that_starts_another_program_is_not_ours_and_is_left_alone() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let python = sb.root.join("usr").join("python3");
    program(&python, true);
    let file = sb.user(Client::Cursor);
    let before = config(
        Client::Cursor,
        &[
            (
                "seleniumbase",
                launch(python.to_str().unwrap(), &["-m", "seleniumbase", "mcp"]),
            ),
            ("tool", launch("/gone/tool", &[])),
        ],
    );
    sb.put(&file, &before);
    let request = request(&exe);

    assert!(issues(&sb, &request, Client::Cursor, Scope::User).is_empty());
    let outcome = run(&sb, &request);
    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert_eq!(read(&file), before);
}

#[test]
fn an_entry_whose_program_is_gone_is_ours_by_its_name() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    sb.put(
        &file,
        &config(
            Client::Cursor,
            &[(
                "seleniumbase-dev",
                launch("/gone/renamed", &["--server", "dev"]),
            )],
        ),
    );
    let request = request(&exe);

    assert_eq!(
        issues(&sb, &request, Client::Cursor, Scope::User),
        [issue(
            "seleniumbase-dev",
            Finding::BinaryMissing {
                path: "/gone/renamed".to_owned()
            }
        )]
    );
    assert_eq!(run(&sb, &request).code, 0);
    assert_eq!(command_of(&sb, "seleniumbase-dev"), exe.to_str().unwrap());
}

// -- duplicates and names -------------------------------------------------------------------

#[test]
fn duplicate_entries_collapse_to_the_one_with_the_name_install_uses() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let same = launch(exe.to_str().unwrap(), &["--server", "cdp"]);
    let file = sb.user(Client::Cursor);
    sb.put(
        &file,
        &config(
            Client::Cursor,
            &[
                ("a-copy", same.clone()),
                ("seleniumbase", same.clone()),
                ("other", launch("/usr/bin/other", &[])),
            ],
        ),
    );
    let request = request(&exe);

    assert_eq!(
        issues(&sb, &request, Client::Cursor, Scope::User),
        [issue(
            "a-copy",
            Finding::Duplicate {
                of: "seleniumbase".to_owned()
            }
        )]
    );
    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    let entries = sb.entries(Client::Cursor, Scope::User);
    assert!(!entries.contains_key("a-copy"));
    assert_eq!(entries["seleniumbase"].as_ref(), Some(&same));
    assert!(entries.contains_key("other"));
}

#[test]
fn a_duplicate_whose_program_is_gone_is_fixed_by_keeping_one_entry() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let stale = launch("/gone/sbrs-mcp", &["--server", "cdp"]);
    let file = sb.user(Client::Cursor);
    sb.put(
        &file,
        &config(
            Client::Cursor,
            &[("seleniumbase", stale.clone()), ("sb", stale)],
        ),
    );

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    let entries = sb.entries(Client::Cursor, Scope::User);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries["seleniumbase"].as_ref().unwrap().command,
        exe.to_str().unwrap()
    );
}

#[test]
fn a_name_that_is_not_ours_but_starts_this_program_is_reported_and_kept() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let before = config(
        Client::Cursor,
        &[(
            "browser",
            launch(exe.to_str().unwrap(), &["--server", "sb"]),
        )],
    );
    sb.put(&file, &before);
    let request = request(&exe);

    assert_eq!(
        issues(&sb, &request, Client::Cursor, Scope::User),
        [issue("browser", Finding::OtherName)]
    );
    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 0, "a note is not a fault: {}", outcome.err);
    assert!(outcome.out.contains("browser"), "{}", outcome.out);
    assert_eq!(read(&file), before, "nothing is written for a note");
    assert!(!backup_path(&file).exists());
}

#[test]
fn a_name_that_is_not_ours_with_a_missing_program_is_fixed_where_it_stands() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    sb.put(
        &file,
        &config(
            Client::Cursor,
            &[("browser", launch("/gone/sbrs-mcp", &["--server", "sb"]))],
        ),
    );
    let request = request(&exe);

    assert_eq!(
        issues(&sb, &request, Client::Cursor, Scope::User),
        [
            issue(
                "browser",
                Finding::BinaryMissing {
                    path: "/gone/sbrs-mcp".to_owned()
                }
            ),
            issue("browser", Finding::OtherName),
        ]
    );
    assert_eq!(run(&sb, &request).code, 0);
    assert_eq!(command_of(&sb, "browser"), exe.to_str().unwrap());
    assert_eq!(
        sb.entries(Client::Cursor, Scope::User).len(),
        1,
        "it keeps its name and is not copied"
    );
}

// -- configs that cannot be read ------------------------------------------------------------

#[test]
fn json_that_does_not_parse_is_reported_and_never_rewritten() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let broken = "{ \"mcpServers\": { \"seleniumbase\": { \"command\": \"/gone\", }\n";
    sb.put(&file, broken);
    let request = request(&exe);

    let found = issues(&sb, &request, Client::Cursor, Scope::User);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(matches!(found[0].finding, Finding::ConfigUnreadable { .. }));
    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 2, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("cannot be read"), "{}", outcome.out);
    assert!(outcome.out.contains("left untouched"), "{}", outcome.out);
    assert_eq!(read(&file), broken, "the file is exactly as it was");
    assert!(!backup_path(&file).exists());
}

#[test]
fn json_with_comments_is_reported_and_never_rewritten() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let commented = "{\n  // my servers\n  \"mcpServers\": {\n    \"seleniumbase\": { \"command\": \"/gone/sbrs-mcp\" }\n  }\n}\n";
    sb.put(&file, commented);

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 2, "{}{}", outcome.out, outcome.err);
    assert!(outcome.out.contains("comments"), "{}", outcome.out);
    assert_eq!(read(&file), commented);
    assert!(!backup_path(&file).exists());
}

#[test]
fn toml_that_does_not_parse_is_reported_and_never_rewritten() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Grok);
    let broken = "[mcp_servers.seleniumbase\ncommand = \"/gone/sbrs-mcp\"\n";
    sb.put(&file, broken);
    let request = request(&exe);

    let found = issues(&sb, &request, Client::Grok, Scope::User);
    assert!(matches!(
        found.as_slice(),
        [Issue {
            entry: None,
            finding: Finding::ConfigUnreadable { .. }
        }]
    ));
    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 2, "{}{}", outcome.out, outcome.err);
    assert_eq!(read(&file), broken);
    assert!(!backup_path(&file).exists());
}

#[test]
fn an_unreadable_config_does_not_stop_the_others_from_being_repaired() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    sb.put(&sb.user(Client::Cursor), "{ not json");
    let zed = sb.user(Client::Zed);
    sb.put(
        &zed,
        &config(
            Client::Zed,
            &[(
                "seleniumbase",
                launch("/gone/sbrs-mcp", &["--server", "cdp"]),
            )],
        ),
    );

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 2, "the unreadable one is left broken");
    let entries =
        crate::mcp_install::jsonfile::read_existing(&zed, &Client::Zed.layout().unwrap()).unwrap();
    assert_eq!(
        entries["seleniumbase"].as_ref().unwrap().command,
        exe.to_str().unwrap()
    );
}

// -- clients, scopes, formats ---------------------------------------------------------------

#[test]
fn the_client_flag_limits_the_check_to_the_clients_named() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let stale = launch("/gone/sbrs-mcp", &["--server", "cdp"]);
    let cursor = sb.user(Client::Cursor);
    let zed = sb.user(Client::Zed);
    sb.put(
        &cursor,
        &config(Client::Cursor, &[("seleniumbase", stale.clone())]),
    );
    let zed_before = config(Client::Zed, &[("seleniumbase", stale)]);
    sb.put(&zed, &zed_before);
    let mut request = request(&exe);
    request.clients = vec!["cursor".to_owned()];

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    assert_eq!(command_of(&sb, "seleniumbase"), exe.to_str().unwrap());
    assert_eq!(read(&zed), zed_before, "zed was not named");
}

#[test]
fn an_unknown_client_is_refused_with_the_known_ones_listed() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let mut request = request(&exe);
    request.clients = vec!["nosuch".to_owned()];

    let outcome = run(&sb, &request);

    assert_eq!(outcome.code, 2);
    assert!(outcome.err.contains("`nosuch`"), "{}", outcome.err);
    assert!(outcome.err.contains("cursor"), "{}", outcome.err);
}

#[test]
fn a_name_no_client_would_accept_is_refused() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let mut request = request(&exe);
    request.name = "not a name".to_owned();
    assert_eq!(run(&sb, &request).code, 2);
}

#[test]
fn the_config_in_the_working_folder_is_checked_too() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let project = sb.file(Client::Cursor, Scope::Project);
    sb.put(
        &project,
        &config(
            Client::Cursor,
            &[(
                "seleniumbase",
                launch("/gone/sbrs-mcp", &["--server", "cdp"]),
            )],
        ),
    );
    let request = request(&exe);

    assert_eq!(
        issues(&sb, &request, Client::Cursor, Scope::Project).len(),
        1
    );
    assert_eq!(run(&sb, &request).code, 0);
    let entries = sb.entries(Client::Cursor, Scope::Project);
    assert_eq!(
        entries["seleniumbase"].as_ref().unwrap().command,
        exe.to_str().unwrap()
    );
}

#[test]
fn a_toml_client_is_fixed_and_keeps_its_comments_and_tables() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Grok);
    sb.put(
        &file,
        "# my grok\nmodel = \"x\"\n\n[mcp_servers.seleniumbase]\ncommand = \"/gone/sbrs-mcp\"\nargs = [\"--server\", \"cdp\"]\n\n[other]\nkept = true\n",
    );

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    let text = read(&file);
    assert!(text.starts_with("# my grok\nmodel = \"x\"\n"), "{text}");
    assert!(text.contains(exe.to_str().unwrap()), "{text}");
    assert!(!text.contains("/gone/sbrs-mcp"), "{text}");
    assert!(text.contains("[other]\nkept = true\n"), "{text}");
    assert!(backup_path(&file).exists());
}

#[test]
fn everything_outside_the_fixed_entry_keeps_its_bytes() {
    let sb = Sandbox::mac();
    let exe = current(&sb);
    let file = sb.user(Client::Cursor);
    let head = "{\n    \"theme\": \"dark\",\n    \"mcpServers\": {\n        \"other\": { \"command\": \"/usr/bin/other\" },\n        ";
    let entry =
        "\"seleniumbase\": { \"command\": \"/gone/sbrs-mcp\", \"args\": [\"--server\", \"cdp\"] }";
    let tail = "\n    },\n    \"last\": 1\n}";
    sb.put(&file, &format!("{head}{entry}{tail}"));

    let outcome = run(&sb, &request(&exe));

    assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
    let after = read(&file);
    assert!(after.starts_with(head), "{after}");
    assert!(after.ends_with(tail), "{after}");
    assert!(after.contains(exe.to_str().unwrap()), "{after}");
}

#[cfg(unix)]
mod through_claude {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// Puts a `claude` on the `PATH` of the machine that logs its arguments, one run per line.
    fn claude_on_path(sb: &mut Sandbox) -> PathBuf {
        let bin = sb.root.join("bin");
        let log = sb.root.join("claude.log");
        fs::create_dir_all(&bin).unwrap();
        let script = bin.join("claude");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 0\n",
                log.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        sb.env.path = Some(bin.into_os_string());
        log
    }

    #[test]
    fn claude_code_is_repaired_with_its_own_commands_and_its_file_is_not_written() {
        let mut sb = Sandbox::mac();
        let exe = current(&sb);
        let log = claude_on_path(&mut sb);
        let file = sb.user(Client::ClaudeCode);
        let before = config(
            Client::ClaudeCode,
            &[(
                "seleniumbase",
                launch("/gone/sbrs-mcp", &["--server", "cdp"]),
            )],
        );
        sb.put(&file, &before);

        let outcome = run(&sb, &request(&exe));

        assert_eq!(outcome.code, 0, "{}{}", outcome.out, outcome.err);
        let calls = read(&log);
        let lines: Vec<&str> = calls.lines().collect();
        assert_eq!(lines.len(), 2, "{calls}");
        assert_eq!(lines[0], "mcp remove --scope user seleniumbase");
        assert!(
            lines[1].starts_with("mcp add-json --scope user seleniumbase "),
            "{calls}"
        );
        assert!(lines[1].contains(exe.to_str().unwrap()), "{calls}");
        assert_eq!(read(&file), before, "~/.claude.json is the CLI's to write");
    }

    #[test]
    fn a_dry_run_runs_no_claude_command() {
        let mut sb = Sandbox::mac();
        let exe = current(&sb);
        let log = claude_on_path(&mut sb);
        sb.put(
            &sb.user(Client::ClaudeCode),
            &config(
                Client::ClaudeCode,
                &[(
                    "seleniumbase",
                    launch("/gone/sbrs-mcp", &["--server", "cdp"]),
                )],
            ),
        );
        let mut request = request(&exe);
        request.dry_run = true;

        let outcome = run(&sb, &request);

        assert_eq!(outcome.code, 1, "{}{}", outcome.out, outcome.err);
        assert!(
            outcome.out.contains("would run: claude mcp"),
            "{}",
            outcome.out
        );
        assert!(!log.exists(), "nothing was run");
    }

    #[test]
    fn a_client_whose_program_is_missing_is_left_broken_and_the_commands_are_printed() {
        let sb = Sandbox::mac();
        let exe = current(&sb);
        sb.put(
            &sb.user(Client::ClaudeCode),
            &config(
                Client::ClaudeCode,
                &[(
                    "seleniumbase",
                    launch("/gone/sbrs-mcp", &["--server", "cdp"]),
                )],
            ),
        );

        let outcome = run(&sb, &request(&exe));

        assert_eq!(outcome.code, 2, "{}{}", outcome.out, outcome.err);
        assert!(
            outcome.out.contains("claude mcp add-json"),
            "{}",
            outcome.out
        );
        assert!(outcome.err.contains("not on PATH"), "{}", outcome.err);
    }
}
