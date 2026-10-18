//! `sbrs-mcp install`, `uninstall` and `clients` through the real binary: the command line, the
//! exit statuses, and that running it with no command still serves.
//!
//! Every run gets a temporary `HOME` (and `PATH`, `CODEX_HOME` and the XDG variables of its
//! own), so no config of the machine is read or written. Where a client's own program is
//! needed, `PATH` holds a script that records its arguments. No browser is started.
//! Run with `cargo test --features mcp-server`.

#![cfg(feature = "mcp-server")]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::Value;

const KNOWN: [&str; 8] = [
    "claude-code",
    "claude-desktop",
    "codex",
    "cursor",
    "gemini-cli",
    "opencode",
    "vscode",
    "zed",
];

/// A machine of its own: a home, a working folder and an (empty) folder for `PATH`.
struct Machine {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        for folder in ["home", "work", "bin"] {
            fs::create_dir_all(root.join(folder)).unwrap();
        }
        Self { _dir: dir, root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn work(&self) -> PathBuf {
        self.root.join("work")
    }

    fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    fn output_dir(&self) -> PathBuf {
        self.root.join("out")
    }

    /// The command: `sbrs-mcp args...` in the working folder, with this machine's variables.
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sbrs-mcp"));
        command
            .args(args)
            .current_dir(self.work())
            .env("HOME", self.home())
            .env("USERPROFILE", self.home())
            .env("PATH", self.bin())
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_DATA_HOME")
            .env_remove("CODEX_HOME")
            .env_remove("RUST_LOG")
            .env_remove("NO_COLOR");
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("SB_") {
                command.env_remove(name);
            }
        }
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("run sbrs-mcp")
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The path `install` registers: the running binary, resolved.
fn registered_exe() -> String {
    fs::canonicalize(env!("CARGO_BIN_EXE_sbrs-mcp"))
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

#[test]
fn clients_lists_every_client_with_its_config_and_whether_it_was_found() {
    let machine = Machine::new();
    let output = machine.run(&["clients"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    for id in KNOWN {
        assert!(
            stdout.contains(&format!("\n{id}  ")) || stdout.starts_with(&format!("{id}  ")),
            "{stdout}"
        );
    }
    assert!(stdout.contains("config "), "{stdout}");
    assert!(stdout.contains("(not found)"), "{stdout}");
    assert!(stdout.contains("claude (not found on PATH)"), "{stdout}");
    assert!(stdout.contains("codex (not found on PATH)"), "{stdout}");
    assert!(stdout.contains("registered  user: none"), "{stdout}");
    assert!(
        stdout.contains("https://code.claude.com/docs/en/mcp"),
        "{stdout}"
    );
    assert!(
        stdout.contains("https://cursor.com/docs/context/mcp"),
        "{stdout}"
    );
}

#[test]
fn install_print_shows_what_would_be_written_and_writes_nothing() {
    let machine = Machine::new();
    let output = machine.run(&["install", "--print", "cursor"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert!(stdout.contains("\"mcpServers\""), "{stdout}");
    assert!(stdout.contains("\"seleniumbase\""), "{stdout}");
    assert!(stdout.contains("\"seleniumbase-dev\""), "{stdout}");
    assert!(stdout.contains("would be added"), "{stdout}");
    assert!(
        stdout.contains(&registered_exe().replace('\\', "\\\\")),
        "{stdout}"
    );
    assert!(stdout.contains("SB_MCP_OUTPUT_DIR"), "{stdout}");
    assert_eq!(
        fs::read_dir(machine.home()).unwrap().count(),
        0,
        "the home folder is untouched"
    );
}

#[test]
fn install_print_with_a_server_a_name_and_an_output_folder() {
    let machine = Machine::new();
    let out = machine.output_dir();
    let output = machine.run(&[
        "install",
        "--print",
        "--server",
        "driver",
        "--name",
        "browser",
        "--output-dir",
        out.to_str().unwrap(),
        "zed",
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert!(stdout.contains("\"context_servers\""), "{stdout}");
    assert!(stdout.contains("\"browser\""), "{stdout}");
    assert!(stdout.contains("\"browser-dev\""), "{stdout}");
    assert!(stdout.contains("[\"--server\", \"driver\"]"), "{stdout}");
    assert!(stdout.contains("[\"--server\", \"dev\"]"), "{stdout}");
    assert!(!out.exists(), "a dry run creates no folder");
}

#[test]
fn a_bad_client_name_is_exit_2_and_the_known_clients_are_listed() {
    let machine = Machine::new();
    for command in ["install", "uninstall"] {
        let output = machine.run(&[command, "nosuch"]);
        assert_eq!(output.status.code(), Some(2), "{command}");
        let stderr = text(&output.stderr);
        assert!(stderr.contains("`nosuch`"), "{stderr}");
        for id in KNOWN {
            assert!(stderr.contains(id), "{stderr}");
        }
    }
    let output = machine.run(&["install"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("at least one client"));
}

#[test]
fn a_bad_flag_value_is_exit_2() {
    let machine = Machine::new();
    for args in [
        ["install", "--server", "nosuch", "cursor"],
        ["install", "--scope", "nosuch", "cursor"],
        ["install", "--only", "nosuch", "cursor"],
    ] {
        let output = machine.run(&args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            text(&output.stderr)
        );
    }
    // Only the browser servers are chosen with --server; the dev server is `--only dev`.
    let output = machine.run(&["install", "--server", "dev", "cursor"]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn flags_of_the_server_do_not_go_with_a_command() {
    let machine = Machine::new();
    let output = machine.run(&["--server", "cdp", "clients"]);
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
}

#[test]
fn help_names_the_commands_and_the_install_flags() {
    let machine = Machine::new();
    let output = machine.run(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    let stdout = text(&output.stdout);
    for word in ["install", "uninstall", "clients"] {
        assert!(stdout.contains(word), "{stdout}");
    }
    let output = machine.run(&["install", "--help"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    for word in [
        "--server",
        "--name",
        "--output-dir",
        "--plugin",
        "--plugin-opt",
        "--scope",
        "--only",
        "--print",
        "--force",
        "NAME-dev",
    ] {
        assert!(stdout.contains(word), "{word} in {stdout}");
    }
}

#[test]
fn with_no_command_the_server_still_serves() {
    let machine = Machine::new();
    let mut child = machine
        .command(&["--server", "cdp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start sbrs-mcp");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = sender.send(line);
    });
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"test","version":"0"}}}}}}"#
    )
    .unwrap();
    stdin.flush().unwrap();
    let reply = receiver.recv_timeout(Duration::from_secs(60));
    drop(stdin);
    let waited = child.wait();
    let reply = reply.expect("the server answered the handshake in time");
    assert!(reply.contains("\"protocolVersion\""), "{reply}");
    assert!(reply.contains("\"id\":1"), "{reply}");
    assert!(
        waited.unwrap().success(),
        "the server ends when its client goes"
    );
}

#[cfg(unix)]
mod on_a_temporary_home {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn cursor_config(machine: &Machine) -> PathBuf {
        machine.home().join(".cursor").join("mcp.json")
    }

    fn read_json(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn install_registers_the_absolute_path_and_uninstall_removes_it() {
        let machine = Machine::new();
        let out = machine.output_dir();
        let out = out.to_str().unwrap();
        let output = machine.run(&["install", "--output-dir", out, "cursor"]);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        let config = read_json(&cursor_config(&machine));
        let servers = &config["mcpServers"];
        assert_eq!(servers["seleniumbase"]["command"], registered_exe());
        assert_eq!(
            servers["seleniumbase"]["args"],
            serde_json::json!(["--server", "cdp"])
        );
        assert_eq!(servers["seleniumbase-dev"]["command"], registered_exe());
        assert_eq!(
            servers["seleniumbase-dev"]["args"],
            serde_json::json!(["--server", "dev"])
        );
        assert_eq!(servers["seleniumbase"]["env"]["SB_MCP_OUTPUT_DIR"], out);
        assert!(
            machine.output_dir().is_dir(),
            "the output folder is created"
        );
        assert!(Path::new(&registered_exe()).is_absolute());

        let again = machine.run(&["install", "--output-dir", out, "cursor"]);
        assert_eq!(again.status.code(), Some(0), "{}", text(&again.stderr));
        assert!(text(&again.stdout).contains("unchanged"));

        let list = machine.run(&["clients", "--name", "seleniumbase"]);
        assert!(
            text(&list.stdout).contains("user: seleniumbase, seleniumbase-dev"),
            "{}",
            text(&list.stdout)
        );

        let removed = machine.run(&["uninstall", "cursor"]);
        assert_eq!(removed.status.code(), Some(0), "{}", text(&removed.stderr));
        assert_eq!(
            read_json(&cursor_config(&machine))["mcpServers"],
            serde_json::json!({})
        );
    }

    #[test]
    fn a_different_entry_is_exit_2_until_force() {
        let machine = Machine::new();
        let out = machine.output_dir();
        let out = out.to_str().unwrap();
        assert_eq!(
            machine
                .run(&["install", "--output-dir", out, "--server", "cdp", "cursor"])
                .status
                .code(),
            Some(0)
        );
        let before = fs::read_to_string(cursor_config(&machine)).unwrap();
        let output = machine.run(&["install", "--output-dir", out, "--server", "sb", "cursor"]);
        assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
        assert_eq!(fs::read_to_string(cursor_config(&machine)).unwrap(), before);
        let output = machine.run(&[
            "install",
            "--output-dir",
            out,
            "--server",
            "sb",
            "--force",
            "cursor",
        ]);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        assert_eq!(
            read_json(&cursor_config(&machine))["mcpServers"]["seleniumbase"]["args"],
            serde_json::json!(["--server", "sb"])
        );
    }

    #[test]
    fn the_project_scope_writes_into_the_working_folder() {
        let machine = Machine::new();
        let out = machine.output_dir();
        let output = machine.run(&[
            "install",
            "--scope",
            "project",
            "--output-dir",
            out.to_str().unwrap(),
            "cursor",
        ]);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        assert!(machine.work().join(".cursor").join("mcp.json").is_file());
        assert!(!cursor_config(&machine).exists());
        let output = machine.run(&["install", "--scope", "project", "claude-desktop"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(
            text(&output.stderr).contains("--scope user"),
            "{}",
            text(&output.stderr)
        );
    }

    #[test]
    fn a_commented_file_is_exit_2_and_untouched() {
        let machine = Machine::new();
        let zed = machine.home().join(".config").join("zed");
        fs::create_dir_all(&zed).unwrap();
        let file = zed.join("settings.json");
        let original = "// my settings\n{\n  \"theme\": \"One Dark\"\n}\n";
        fs::write(&file, original).unwrap();
        let out = machine.output_dir();
        let output = machine.run(&["install", "--output-dir", out.to_str().unwrap(), "zed"]);
        assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
        assert_eq!(fs::read_to_string(&file).unwrap(), original);
        assert!(text(&output.stdout).contains("\"context_servers\""));
    }

    /// A script called `name` in the machine's `bin` folder that logs its arguments.
    fn fake(machine: &Machine, name: &str, before: &str) -> PathBuf {
        let log = machine.root.join(format!("{name}.log"));
        let path = machine.bin().join(name);
        fs::write(
            &path,
            format!(
                "#!/bin/sh\n{before}\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done >> '{}'\nprintf '@@END@@\\n' >> '{}'\n",
                log.display(),
                log.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        log
    }

    fn logged_calls(log: &Path) -> Vec<Vec<String>> {
        let Ok(text) = fs::read_to_string(log) else {
            return Vec::new();
        };
        text.split("@@END@@\n")
            .filter(|call| !call.is_empty())
            .map(|call| call.lines().map(str::to_owned).collect())
            .collect()
    }

    #[test]
    fn claude_code_goes_through_the_claude_program() {
        let machine = Machine::new();
        let log = fake(&machine, "claude", "");
        let out = machine.output_dir();
        let output = machine.run(&[
            "install",
            "--output-dir",
            out.to_str().unwrap(),
            "claude-code",
        ]);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        let calls = logged_calls(&log);
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(
            calls[0][..5],
            ["mcp", "add-json", "--scope", "user", "seleniumbase"]
        );
        assert_eq!(
            calls[1][..5],
            ["mcp", "add-json", "--scope", "user", "seleniumbase-dev"]
        );
        let json: Value = serde_json::from_str(&calls[0][5]).unwrap();
        assert_eq!(json["command"], registered_exe());
        assert!(
            !machine.home().join(".claude.json").exists(),
            "never written directly"
        );

        let dry = machine.run(&["install", "--print", "claude-code"]);
        assert_eq!(dry.status.code(), Some(0));
        assert_eq!(logged_calls(&log).len(), 2, "--print ran nothing");
        assert!(
            text(&dry.stdout).contains("would run: claude mcp add-json --scope user seleniumbase ")
        );
    }

    #[test]
    fn claude_code_without_the_claude_program_prints_the_command_and_exits_2() {
        let machine = Machine::new();
        let output = machine.run(&["install", "claude-code"]);
        assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
        assert!(text(&output.stdout).contains("claude mcp add-json --scope user seleniumbase "));
        assert!(!machine.home().join(".claude.json").exists());
    }

    #[test]
    fn codex_goes_through_codex_mcp_add() {
        let machine = Machine::new();
        let log = fake(
            &machine,
            "codex",
            "if [ \"$1\" = mcp ] && [ \"$2\" = --help ]; then printf 'Commands:\\n  list\\n  add\\n  remove\\n'; exit 0; fi",
        );
        let out = machine.output_dir();
        let output = machine.run(&[
            "install",
            "--output-dir",
            out.to_str().unwrap(),
            "--only",
            "browser",
            "codex",
        ]);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        let calls = logged_calls(&log);
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(calls[0][..3], ["mcp", "add", "seleniumbase"]);
        assert_eq!(calls[0][3], "--env");
        assert_eq!(
            calls[0][4],
            format!("SB_MCP_OUTPUT_DIR={}", out.to_str().unwrap())
        );
        assert_eq!(
            calls[0][5..],
            [
                "--".to_owned(),
                registered_exe(),
                "--server".to_owned(),
                "cdp".to_owned()
            ]
        );
        assert!(!machine.home().join(".codex").exists());
    }

    #[test]
    fn codex_without_a_program_is_edited_through_config_toml() {
        let machine = Machine::new();
        let out = machine.output_dir();
        let output = machine.run(&["install", "--output-dir", out.to_str().unwrap(), "codex"]);
        assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
        let config = fs::read_to_string(machine.home().join(".codex").join("config.toml")).unwrap();
        assert!(config.contains("[mcp_servers.seleniumbase]"), "{config}");
        assert!(
            config.contains("[mcp_servers.seleniumbase-dev]"),
            "{config}"
        );
        assert!(
            config.contains(&format!("command = {:?}", registered_exe())),
            "{config}"
        );
    }
}
