//! Tests of the installer against temporary folders. Nothing here reads or writes the real
//! config of any client: every path comes from an [`Env`] rooted in a temporary folder, and
//! the `claude` and `codex` programs are scripts on a `PATH` of the test's own that record the
//! arguments they were given.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::apply::Scope;
use super::client::Client;
use super::entry::{plan, Existing, LaunchSpec, Only, PlanInput};
use super::env::{Env, Os};
use super::fsx::backup_path;
use super::jsonfile;
use super::{clients, install, uninstall, ClientsRequest, InstallRequest, UninstallRequest};

const EXE: &str = "/opt/sbrs/sbrs-mcp";

/// The clients whose config is a JSON file that `install` edits itself.
const JSON_CLIENTS: [Client; 6] = [
    Client::ClaudeDesktop,
    Client::Cursor,
    Client::GeminiCli,
    Client::Opencode,
    Client::Vscode,
    Client::Zed,
];

/// The clients that have a project config file and are edited as JSON.
const PROJECT_CLIENTS: [Client; 4] = [
    Client::Cursor,
    Client::GeminiCli,
    Client::Opencode,
    Client::Vscode,
];

/// What a run printed and returned.
struct Outcome {
    code: i32,
    out: String,
    err: String,
}

struct Sandbox {
    _dir: tempfile::TempDir,
    root: PathBuf,
    env: Env,
}

impl Sandbox {
    fn new(os: Os) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(dir.path()).unwrap();
        let env = Env::rooted(&root, os);
        fs::create_dir_all(&env.cwd).unwrap();
        Self {
            _dir: dir,
            root,
            env,
        }
    }

    fn mac() -> Self {
        Self::new(Os::Mac)
    }

    /// The config file of `client`.
    fn file(&self, client: Client, scope: Scope) -> PathBuf {
        match scope {
            Scope::User => client.user_path(&self.env),
            Scope::Project => client.project_path(&self.env),
        }
        .unwrap()
    }

    fn user(&self, client: Client) -> PathBuf {
        self.file(client, Scope::User)
    }

    fn put(&self, path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn output_dir(&self) -> PathBuf {
        self.root.join("out")
    }

    fn request(&self, clients: &[&str]) -> InstallRequest {
        InstallRequest {
            clients: clients.iter().map(|c| (*c).to_owned()).collect(),
            exe: PathBuf::from(EXE),
            name: "seleniumbase".to_owned(),
            browser: "cdp".to_owned(),
            output_dir: Some(self.output_dir()),
            plugins: Vec::new(),
            plugin_opts: Vec::new(),
            scope: Scope::User,
            only: Only::Both,
            dry_run: false,
            force: false,
        }
    }

    fn install(&self, clients: &[&str], tweak: impl FnOnce(&mut InstallRequest)) -> Outcome {
        let mut request = self.request(clients);
        tweak(&mut request);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = install(&self.env, &request, &mut out, &mut err);
        Outcome {
            code,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    fn uninstall(&self, clients: &[&str], tweak: impl FnOnce(&mut UninstallRequest)) -> Outcome {
        let mut request = UninstallRequest {
            clients: clients.iter().map(|c| (*c).to_owned()).collect(),
            name: "seleniumbase".to_owned(),
            scope: Scope::User,
            only: Only::Both,
            dry_run: false,
        };
        tweak(&mut request);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = uninstall(&self.env, &request, &mut out, &mut err);
        Outcome {
            code,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    /// What the browser entry is registered as.
    fn browser_spec(&self) -> LaunchSpec {
        spec(&["--server", "cdp"], self.output_dir().to_str().unwrap())
    }

    /// What the development entry is registered as.
    fn dev_spec(&self) -> LaunchSpec {
        spec(&["--server", "dev"], self.output_dir().to_str().unwrap())
    }

    /// The entries of a JSON client's config.
    fn entries(&self, client: Client, scope: Scope) -> Existing {
        jsonfile::read_existing(&self.file(client, scope), &client.layout().unwrap()).unwrap()
    }
}

fn spec(args: &[&str], output: &str) -> LaunchSpec {
    LaunchSpec {
        command: EXE.to_owned(),
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        env: [("SB_MCP_OUTPUT_DIR".to_owned(), output.to_owned())].into(),
    }
}

fn different_spec() -> LaunchSpec {
    LaunchSpec {
        command: "/somewhere/else".to_owned(),
        args: vec!["--server".to_owned(), "sb".to_owned()],
        env: Default::default(),
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

// -- the plan -----------------------------------------------------------------------------

fn plan_input<'a>(only: Only, plugins: &'a [String], options: &'a [String]) -> PlanInput<'a> {
    PlanInput {
        exe: EXE,
        name: "seleniumbase",
        browser: "driver",
        output_dir: "/data/out",
        plugins,
        plugin_opts: options,
        only,
    }
}

#[test]
fn the_plan_has_a_browser_entry_and_a_dev_entry_with_the_same_output_folder() {
    let entries = plan(&plan_input(Only::Both, &[], &[]));
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "seleniumbase");
    assert_eq!(entries[0].spec.command, EXE);
    assert_eq!(entries[0].spec.args, ["--server", "driver"]);
    assert_eq!(entries[1].name, "seleniumbase-dev");
    assert_eq!(entries[1].spec.command, EXE);
    assert_eq!(entries[1].spec.args, ["--server", "dev"]);
    assert_eq!(entries[0].spec.env, entries[1].spec.env);
    assert_eq!(entries[0].spec.env["SB_MCP_OUTPUT_DIR"], "/data/out");
}

#[test]
fn only_picks_one_entry() {
    let browser = plan(&plan_input(Only::Browser, &[], &[]));
    assert_eq!(
        browser.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["seleniumbase"]
    );
    let dev = plan(&plan_input(Only::Dev, &[], &[]));
    assert_eq!(
        dev.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["seleniumbase-dev"]
    );
}

#[test]
fn plugin_flags_go_to_the_browser_entry_only() {
    let plugins = vec!["shield".to_owned(), "crx".to_owned()];
    let options = vec![r#"shield.level="strict""#.to_owned()];
    let entries = plan(&plan_input(Only::Both, &plugins, &options));
    assert_eq!(
        entries[0].spec.args,
        [
            "--server",
            "driver",
            "--plugin",
            "shield",
            "--plugin",
            "crx",
            "--plugin-opt",
            r#"shield.level="strict""#
        ]
    );
    assert_eq!(entries[1].spec.args, ["--server", "dev"]);
}

// -- the command line's refusals ------------------------------------------------------------

#[test]
fn an_unknown_client_is_refused_and_the_known_ones_are_listed() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["cursor", "nosuch"], |_| {});
    assert_eq!(outcome.code, 2);
    assert!(outcome.err.contains("`nosuch`"), "{}", outcome.err);
    for client in Client::ALL {
        assert!(outcome.err.contains(client.id()), "{}", outcome.err);
    }
    assert!(
        !sb.user(Client::Cursor).exists(),
        "nothing is written when a name is wrong"
    );
}

#[test]
fn a_name_that_some_client_would_refuse_is_refused() {
    let sb = Sandbox::mac();
    for name in ["", "two words", "a/b", "x.y", "naïve"] {
        let outcome = sb.install(&["cursor"], |r| r.name = name.to_owned());
        assert_eq!(outcome.code, 2, "{name:?}");
        assert!(outcome.err.contains("--name"), "{}", outcome.err);
        let outcome = sb.uninstall(&["cursor"], |r| r.name = name.to_owned());
        assert_eq!(outcome.code, 2, "{name:?}");
    }
    assert!(!sb.user(Client::Cursor).exists());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let request = ClientsRequest {
        name: "a b".to_owned(),
    };
    assert_eq!(clients(&sb.env, &request, &mut out, &mut err), 2);
    assert!(out.is_empty());
}

#[test]
fn a_client_must_be_named() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&[], |_| {});
    assert_eq!(outcome.code, 2);
    assert!(
        outcome.err.contains("at least one client"),
        "{}",
        outcome.err
    );
}

#[test]
fn a_client_named_twice_is_handled_once() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["cursor", "cursor"], |_| {});
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert_eq!(
        outcome.out.matches("cursor  ").count(),
        1,
        "{}",
        outcome.out
    );
}

#[test]
fn plugin_flags_are_refused_with_only_dev() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["cursor"], |r| {
        r.only = Only::Dev;
        r.plugins = vec!["shield".to_owned()];
    });
    assert_eq!(outcome.code, 2);
    assert!(outcome.err.contains("--only dev"), "{}", outcome.err);
    assert!(!sb.user(Client::Cursor).exists());
}

#[test]
fn an_unknown_plugin_is_refused_before_anything_is_written() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["cursor"], |r| {
        r.plugins = vec!["nosuch-plugin".to_owned()]
    });
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    assert!(!sb.user(Client::Cursor).exists());
    assert!(!sb.output_dir().exists());
}

#[cfg(feature = "plugins-builtin")]
#[test]
fn a_known_plugin_is_registered_on_the_browser_entry() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["cursor"], |r| {
        r.plugins = vec!["shield".to_owned()];
        r.plugin_opts = vec![r#"shield.level="strict""#.to_owned()];
    });
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let entries = sb.entries(Client::Cursor, Scope::User);
    let browser = entries["seleniumbase"].as_ref().unwrap();
    assert_eq!(
        browser.args,
        [
            "--server",
            "cdp",
            "--plugin",
            "shield",
            "--plugin-opt",
            r#"shield.level="strict""#
        ]
    );
    let dev = entries["seleniumbase-dev"].as_ref().unwrap();
    assert_eq!(dev.args, ["--server", "dev"]);
}

#[test]
fn the_output_folder_is_created_unless_it_is_a_dry_run() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["cursor"], |r| r.dry_run = true);
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert!(!sb.output_dir().exists());
    let outcome = sb.install(&["cursor"], |_| {});
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert!(sb.output_dir().is_dir());
}

#[test]
fn the_default_output_folder_is_in_the_data_folder() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["cursor"], |r| r.output_dir = None);
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let wanted = sb.env.data_dir.join("sbrs-mcp");
    assert!(wanted.is_dir());
    let entries = sb.entries(Client::Cursor, Scope::User);
    assert_eq!(
        entries["seleniumbase"].as_ref().unwrap().env["SB_MCP_OUTPUT_DIR"],
        wanted.to_str().unwrap()
    );
}

#[test]
fn a_relative_output_folder_is_made_absolute_from_the_working_folder() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["cursor"], |r| {
        r.output_dir = Some(PathBuf::from("out/files"))
    });
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let wanted = sb.env.cwd.join("out").join("files");
    assert!(wanted.is_dir());
    assert_eq!(
        sb.entries(Client::Cursor, Scope::User)["seleniumbase"]
            .as_ref()
            .unwrap()
            .env["SB_MCP_OUTPUT_DIR"],
        wanted.to_str().unwrap()
    );
}

// -- the JSON clients ---------------------------------------------------------------------

#[test]
fn install_into_an_empty_config_registers_both_entries() {
    for client in JSON_CLIENTS {
        let sb = Sandbox::mac();
        let outcome = sb.install(&[client.id()], |_| {});
        assert_eq!(outcome.code, 0, "{}: {}", client.id(), outcome.err);
        let entries = sb.entries(client, Scope::User);
        assert_eq!(entries.len(), 2, "{}", client.id());
        assert_eq!(
            entries["seleniumbase"],
            Some(sb.browser_spec()),
            "{}",
            client.id()
        );
        assert_eq!(
            entries["seleniumbase-dev"],
            Some(sb.dev_spec()),
            "{}",
            client.id()
        );
        assert!(outcome.out.contains("added"), "{}", outcome.out);
        assert!(
            !backup_path(&sb.user(client)).exists(),
            "{}: a file that did not exist has no backup",
            client.id()
        );
    }
}

#[test]
fn each_client_gets_the_format_its_documentation_shows() {
    let sb = Sandbox::mac();
    for client in JSON_CLIENTS {
        let outcome = sb.install(&[client.id()], |r| r.only = Only::Browser);
        assert_eq!(outcome.code, 0, "{}: {}", client.id(), outcome.err);
    }
    let output = sb.output_dir().to_str().unwrap().to_owned();
    let entry = |client: Client, key: &str| -> Value {
        let mut root: Value = serde_json::from_str(&read(&sb.user(client))).unwrap();
        root[key]["seleniumbase"].take()
    };
    let env = serde_json::json!({ "SB_MCP_OUTPUT_DIR": output });
    let plain = serde_json::json!({ "command": EXE, "args": ["--server", "cdp"], "env": env });
    let typed = serde_json::json!({
        "type": "stdio", "command": EXE, "args": ["--server", "cdp"], "env": env
    });
    assert_eq!(entry(Client::ClaudeDesktop, "mcpServers"), plain);
    assert_eq!(entry(Client::GeminiCli, "mcpServers"), plain);
    assert_eq!(entry(Client::Zed, "context_servers"), plain);
    assert_eq!(entry(Client::Cursor, "mcpServers"), typed);
    assert_eq!(entry(Client::Vscode, "servers"), typed);
    assert_eq!(
        entry(Client::Opencode, "mcp"),
        serde_json::json!({
            "type": "local",
            "command": [EXE, "--server", "cdp"],
            "environment": env,
            "enabled": true
        })
    );
}

fn config_with_other_servers(key: &str) -> String {
    format!(
        "{{\n    \"theme\": \"dark\",\n    \"{key}\": {{\n        \"other\": {{\n            \
         \"command\": \"keep-me\",\n            \"args\": [\"a\", \"b\"]\n        }}\n    }},\n    \
         \"tail\": [1, 2.5, null, true]\n}}\n"
    )
}

#[test]
fn other_servers_and_other_keys_survive_in_their_order() {
    for client in JSON_CLIENTS {
        let sb = Sandbox::mac();
        let key = client.layout().unwrap().key;
        let path = sb.user(client);
        let original = config_with_other_servers(key);
        sb.put(&path, &original);
        let outcome = sb.install(&[client.id()], |_| {});
        assert_eq!(outcome.code, 0, "{}: {}", client.id(), outcome.err);

        let before: Value = serde_json::from_str(&original).unwrap();
        let text = read(&path);
        let after: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(after["theme"], before["theme"], "{}", client.id());
        assert_eq!(after["tail"], before["tail"], "{}", client.id());
        assert_eq!(after[key]["other"], before[key]["other"], "{}", client.id());
        let top: Vec<&String> = after.as_object().unwrap().keys().collect();
        assert_eq!(top, ["theme", key, "tail"], "{}", client.id());
        let servers: Vec<&String> = after[key].as_object().unwrap().keys().collect();
        assert_eq!(
            servers,
            ["other", "seleniumbase", "seleniumbase-dev"],
            "{}",
            client.id()
        );
        assert!(text.starts_with("{\n    \"theme\": \"dark\","), "{text}");
        assert!(text.ends_with("}\n"), "{text}");
        assert_eq!(read(&backup_path(&path)), original, "{}", client.id());
    }
}

#[test]
fn the_indent_and_the_missing_final_newline_of_a_file_are_kept() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Cursor);
    sb.put(&path, "{\n\t\"theme\": \"dark\"\n}");
    let outcome = sb.install(&["cursor"], |_| {});
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let text = read(&path);
    assert!(
        text.starts_with("{\n\t\"theme\": \"dark\",\n\t\"mcpServers\": {\n\t\t\"seleniumbase\""),
        "{text}"
    );
    assert!(text.ends_with('}'), "{text:?}");
}

#[test]
fn an_empty_file_is_filled_and_a_bom_stays() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::GeminiCli);
    sb.put(&path, "\u{feff}  \n");
    let outcome = sb.install(&["gemini-cli"], |_| {});
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let text = read(&path);
    assert!(text.starts_with("\u{feff}{\n  \"mcpServers\""), "{text}");
    assert_eq!(sb.entries(Client::GeminiCli, Scope::User).len(), 2);
}

#[test]
fn installing_the_same_entries_again_changes_nothing() {
    for client in JSON_CLIENTS {
        let sb = Sandbox::mac();
        let path = sb.user(client);
        assert_eq!(sb.install(&[client.id()], |_| {}).code, 0);
        let first = read(&path);
        let outcome = sb.install(&[client.id()], |_| {});
        assert_eq!(outcome.code, 0, "{}: {}", client.id(), outcome.err);
        assert_eq!(read(&path), first, "{}", client.id());
        assert!(
            !backup_path(&path).exists(),
            "{}: nothing changed, so no backup",
            client.id()
        );
        assert!(outcome.out.contains("unchanged"), "{}", outcome.out);
        assert!(!outcome.out.contains("added"), "{}", outcome.out);
    }
}

#[test]
fn a_different_entry_is_left_alone_without_force() {
    for client in JSON_CLIENTS {
        let sb = Sandbox::mac();
        let layout = client.layout().unwrap();
        let path = sb.user(client);
        let existing = layout.entry(&different_spec());
        let original = format!(
            "{{\n  \"{}\": {{\n    \"seleniumbase\": {existing},\n    \"seleniumbase-dev\": {existing}\n  }}\n}}\n",
            layout.key
        );
        sb.put(&path, &original);
        let outcome = sb.install(&[client.id()], |_| {});
        assert_eq!(outcome.code, 2, "{}: {}", client.id(), outcome.err);
        assert_eq!(
            read(&path),
            original,
            "{}: the file is unchanged",
            client.id()
        );
        assert!(!backup_path(&path).exists(), "{}", client.id());
        assert_eq!(
            outcome.out.matches("conflict").count(),
            2,
            "{}",
            outcome.out
        );
        assert!(outcome.out.contains("--force"), "{}", outcome.out);
    }
}

#[test]
fn each_entry_is_decided_on_its_own() {
    let sb = Sandbox::mac();
    let layout = Client::Cursor.layout().unwrap();
    let path = sb.user(Client::Cursor);
    let existing = layout.entry(&different_spec());
    sb.put(
        &path,
        &format!("{{\"mcpServers\": {{\"seleniumbase\": {existing}}}}}"),
    );
    let outcome = sb.install(&["cursor"], |_| {});
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    let entries = sb.entries(Client::Cursor, Scope::User);
    assert_eq!(
        entries["seleniumbase"],
        Some(different_spec()),
        "left alone"
    );
    assert_eq!(
        entries["seleniumbase-dev"],
        Some(sb.dev_spec()),
        "added all the same"
    );
    assert!(outcome.out.contains("conflict"), "{}", outcome.out);
    assert!(outcome.out.contains("added"), "{}", outcome.out);
}

#[test]
fn force_replaces_a_different_entry_where_it_stood() {
    for client in JSON_CLIENTS {
        let sb = Sandbox::mac();
        let layout = client.layout().unwrap();
        let path = sb.user(client);
        let existing = layout.entry(&different_spec());
        sb.put(
            &path,
            &format!(
                "{{\n  \"{}\": {{\n    \"first\": {{}},\n    \"seleniumbase\": {existing},\n    \"last\": {{}}\n  }}\n}}\n",
                layout.key
            ),
        );
        let outcome = sb.install(&[client.id()], |r| r.force = true);
        assert_eq!(outcome.code, 0, "{}: {}", client.id(), outcome.err);
        let entries = sb.entries(client, Scope::User);
        assert_eq!(
            entries["seleniumbase"],
            Some(sb.browser_spec()),
            "{}",
            client.id()
        );
        let root: Value = serde_json::from_str(&read(&path)).unwrap();
        let order: Vec<&String> = root[layout.key].as_object().unwrap().keys().collect();
        assert_eq!(
            order,
            ["first", "seleniumbase", "last", "seleniumbase-dev"],
            "{}",
            client.id()
        );
        assert!(outcome.out.contains("replaced"), "{}", outcome.out);
        assert!(backup_path(&path).exists());
    }
}

#[test]
fn an_entry_that_is_not_a_stdio_server_counts_as_different() {
    let sb = Sandbox::mac();
    sb.put(
        &sb.user(Client::Cursor),
        r#"{"mcpServers": {"seleniumbase": {"url": "https://example.test/mcp"}}}"#,
    );
    let outcome = sb.install(&["cursor"], |r| r.only = Only::Browser);
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    let outcome = sb.install(&["cursor"], |r| {
        r.only = Only::Browser;
        r.force = true;
    });
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert_eq!(
        sb.entries(Client::Cursor, Scope::User)["seleniumbase"],
        Some(sb.browser_spec())
    );
}

#[test]
fn an_entry_with_extra_keys_that_starts_the_same_process_is_unchanged() {
    let sb = Sandbox::mac();
    let output = Value::String(sb.output_dir().to_str().unwrap().to_owned());
    let text = format!(
        r#"{{"mcpServers": {{"seleniumbase": {{"command": "{EXE}", "args": ["--server", "cdp"],
            "env": {{"SB_MCP_OUTPUT_DIR": {output}}}, "timeout": 5000, "trust": true}}}}}}"#
    );
    sb.put(&sb.user(Client::GeminiCli), &text);
    let outcome = sb.install(&["gemini-cli"], |r| r.only = Only::Browser);
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert_eq!(read(&sb.user(Client::GeminiCli)), text);
}

#[test]
fn an_opencode_entry_that_is_switched_off_is_different() {
    let sb = Sandbox::mac();
    let layout = Client::Opencode.layout().unwrap();
    let mut entry = layout.entry(&sb.browser_spec());
    entry["enabled"] = Value::Bool(false);
    sb.put(
        &sb.user(Client::Opencode),
        &format!("{{\"mcp\": {{\"seleniumbase\": {entry}}}}}"),
    );
    let outcome = sb.install(&["opencode"], |r| r.only = Only::Browser);
    assert_eq!(outcome.code, 2, "{}", outcome.err);
}

#[test]
fn uninstall_removes_both_entries_and_keeps_everything_else() {
    for client in JSON_CLIENTS {
        let sb = Sandbox::mac();
        let key = client.layout().unwrap().key;
        let path = sb.user(client);
        let original = config_with_other_servers(key);
        sb.put(&path, &original);
        assert_eq!(sb.install(&[client.id()], |_| {}).code, 0);
        let outcome = sb.uninstall(&[client.id()], |_| {});
        assert_eq!(outcome.code, 0, "{}: {}", client.id(), outcome.err);
        assert_eq!(outcome.out.matches("removed").count(), 2, "{}", outcome.out);
        assert_eq!(
            read(&path),
            original,
            "{}: back to what it was",
            client.id()
        );

        let again = sb.uninstall(&[client.id()], |_| {});
        assert_eq!(again.code, 0, "{}", again.err);
        assert_eq!(
            again.out.matches("not installed").count(),
            2,
            "{}",
            again.out
        );
        assert_eq!(read(&path), original);
    }
}

#[test]
fn only_installs_and_removes_one_entry() {
    let sb = Sandbox::mac();
    assert_eq!(sb.install(&["cursor"], |r| r.only = Only::Dev).code, 0);
    let entries = sb.entries(Client::Cursor, Scope::User);
    assert_eq!(entries.keys().collect::<Vec<_>>(), ["seleniumbase-dev"]);
    assert_eq!(sb.install(&["cursor"], |r| r.only = Only::Browser).code, 0);
    assert_eq!(sb.entries(Client::Cursor, Scope::User).len(), 2);
    assert_eq!(sb.uninstall(&["cursor"], |r| r.only = Only::Dev).code, 0);
    let entries = sb.entries(Client::Cursor, Scope::User);
    assert_eq!(entries.keys().collect::<Vec<_>>(), ["seleniumbase"]);
}

#[test]
fn a_name_other_than_the_default_names_both_entries() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["cursor"], |r| r.name = "browser".to_owned());
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let entries = sb.entries(Client::Cursor, Scope::User);
    assert_eq!(
        entries.keys().collect::<Vec<_>>(),
        ["browser", "browser-dev"]
    );
    let outcome = sb.uninstall(&["cursor"], |r| r.name = "browser".to_owned());
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert!(sb.entries(Client::Cursor, Scope::User).is_empty());
}

#[test]
fn print_shows_the_snippet_and_writes_nothing() {
    for client in JSON_CLIENTS {
        let sb = Sandbox::mac();
        let outcome = sb.install(&[client.id()], |r| r.dry_run = true);
        assert_eq!(outcome.code, 0, "{}: {}", client.id(), outcome.err);
        assert!(!sb.user(client).exists(), "{}", client.id());
        assert!(
            !sb.user(client).parent().unwrap().exists(),
            "{}: no folder either",
            client.id()
        );
        assert!(!sb.output_dir().exists());
        assert!(outcome.out.contains("would be added"), "{}", outcome.out);
        assert!(
            outcome.out.contains(client.layout().unwrap().key),
            "{}",
            outcome.out
        );
        assert!(outcome.out.contains(EXE), "{}", outcome.out);
        assert!(outcome.out.contains("seleniumbase-dev"), "{}", outcome.out);
    }
}

#[test]
fn print_of_an_existing_file_leaves_it_and_its_backup_alone() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Cursor);
    let original = config_with_other_servers("mcpServers");
    sb.put(&path, &original);
    let outcome = sb.install(&["cursor"], |r| r.dry_run = true);
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert_eq!(read(&path), original);
    assert!(!backup_path(&path).exists());
}

#[test]
fn uninstall_print_removes_nothing() {
    let sb = Sandbox::mac();
    assert_eq!(sb.install(&["cursor"], |_| {}).code, 0);
    let before = read(&sb.user(Client::Cursor));
    let outcome = sb.uninstall(&["cursor"], |r| r.dry_run = true);
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert!(outcome.out.contains("would be removed"), "{}", outcome.out);
    assert_eq!(read(&sb.user(Client::Cursor)), before);
}

#[test]
fn a_file_with_comments_is_not_rewritten() {
    for client in JSON_CLIENTS {
        let sb = Sandbox::mac();
        let path = sb.user(client);
        let original = "{\n  // keep my settings\n  \"theme\": \"dark\"\n}\n";
        sb.put(&path, original);
        let outcome = sb.install(&[client.id()], |_| {});
        assert_eq!(outcome.code, 2, "{}: {}", client.id(), outcome.err);
        assert_eq!(read(&path), original, "{}", client.id());
        assert!(!backup_path(&path).exists(), "{}", client.id());
        assert!(outcome.err.contains("comments"), "{}", outcome.err);
        assert!(outcome.out.contains("to do it by hand"), "{}", outcome.out);
        assert!(outcome.out.contains("seleniumbase-dev"), "{}", outcome.out);
        assert!(
            outcome.out.contains(client.layout().unwrap().key),
            "{}",
            outcome.out
        );
        assert_eq!(
            outcome.out.contains("code --add-mcp"),
            client == Client::Vscode,
            "{}: {}",
            client.id(),
            outcome.out
        );
    }
}

#[test]
fn a_file_with_a_trailing_comma_is_not_rewritten_either() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Zed);
    let original = "{\n  \"theme\": \"dark\",\n}\n";
    sb.put(&path, original);
    let outcome = sb.install(&["zed"], |_| {});
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    assert_eq!(read(&path), original);
    assert!(outcome.err.contains("not valid JSON"), "{}", outcome.err);
}

#[test]
fn a_file_that_is_not_an_object_is_not_rewritten() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Cursor);
    sb.put(&path, "[1, 2]");
    let outcome = sb.install(&["cursor"], |_| {});
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    assert_eq!(read(&path), "[1, 2]");
    sb.put(&path, r#"{"mcpServers": []}"#);
    let outcome = sb.install(&["cursor"], |_| {});
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    assert_eq!(read(&path), r#"{"mcpServers": []}"#);
}

#[test]
fn slashes_inside_strings_are_not_comments() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Cursor);
    sb.put(&path, r#"{"homepage": "https://example.test//path"}"#);
    let outcome = sb.install(&["cursor"], |_| {});
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert!(read(&path).contains("https://example.test//path"));
}

#[test]
fn uninstall_from_a_file_with_comments_says_what_to_remove() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Zed);
    let original = "{\n  // mine\n  \"context_servers\": {}\n}\n";
    sb.put(&path, original);
    let outcome = sb.uninstall(&["zed"], |_| {});
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    assert!(
        outcome
            .out
            .contains("remove seleniumbase, seleniumbase-dev"),
        "{}",
        outcome.out
    );
    assert_eq!(read(&path), original);
}

#[test]
fn project_scope_writes_the_project_file_and_not_the_user_file() {
    for client in PROJECT_CLIENTS {
        let sb = Sandbox::mac();
        let outcome = sb.install(&[client.id()], |r| r.scope = Scope::Project);
        assert_eq!(outcome.code, 0, "{}: {}", client.id(), outcome.err);
        assert_eq!(
            sb.entries(client, Scope::Project).len(),
            2,
            "{}",
            client.id()
        );
        assert!(sb.file(client, Scope::Project).starts_with(&sb.env.cwd));
        assert!(!sb.user(client).exists(), "{}", client.id());
        let outcome = sb.uninstall(&[client.id()], |r| r.scope = Scope::Project);
        assert_eq!(outcome.code, 0, "{}: {}", client.id(), outcome.err);
        assert!(sb.entries(client, Scope::Project).is_empty());
    }
}

#[test]
fn a_client_without_a_project_config_refuses_project_scope() {
    for client in [Client::ClaudeDesktop, Client::Zed, Client::Codex] {
        let sb = Sandbox::mac();
        let outcome = sb.install(&[client.id()], |r| r.scope = Scope::Project);
        assert_eq!(outcome.code, 2, "{}: {}", client.id(), outcome.err);
        assert!(outcome.err.contains("--scope user"), "{}", outcome.err);
        assert!(!sb.user(client).exists());
        assert!(!sb.env.cwd.join(".zed").exists());
        let outcome = sb.uninstall(&[client.id()], |r| r.scope = Scope::Project);
        assert_eq!(outcome.code, 2, "{}", outcome.err);
    }
}

#[test]
fn the_project_file_of_opencode_may_be_jsonc() {
    let sb = Sandbox::mac();
    let jsonc = sb.env.cwd.join("opencode.jsonc");
    sb.put(&jsonc, "{\n  // mine\n}\n");
    let outcome = sb.install(&["opencode"], |r| r.scope = Scope::Project);
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    assert!(!sb.env.cwd.join("opencode.json").exists());
    assert_eq!(read(&jsonc), "{\n  // mine\n}\n");
}

#[test]
fn claude_desktop_has_no_config_on_linux() {
    let sb = Sandbox::new(Os::Linux);
    let outcome = sb.install(&["claude-desktop"], |_| {});
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    assert!(outcome.err.contains("macOS and Windows"), "{}", outcome.err);
    assert!(!sb.env.home.join(".config").join("Claude").exists());
}

#[test]
fn claude_desktop_warns_that_the_app_rewrites_its_file() {
    for os in [Os::Mac, Os::Windows] {
        let sb = Sandbox::new(os);
        let outcome = sb.install(&["claude-desktop"], |_| {});
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(outcome.err.contains("rewrites"), "{}", outcome.err);
        assert!(outcome.err.contains("quit Claude"), "{}", outcome.err);
        let outcome = sb.uninstall(&["claude-desktop"], |_| {});
        assert!(outcome.err.contains("quit Claude"), "{}", outcome.err);
        let other = sb.install(&["cursor"], |_| {});
        assert!(!other.err.contains("quit Claude"), "{}", other.err);
    }
}

#[test]
fn several_clients_are_handled_each_on_its_own() {
    let sb = Sandbox::mac();
    sb.put(&sb.user(Client::Zed), "{ // mine\n}\n");
    let outcome = sb.install(&["cursor", "zed", "gemini-cli"], |_| {});
    assert_eq!(outcome.code, 2, "the worst status wins");
    assert_eq!(sb.entries(Client::Cursor, Scope::User).len(), 2);
    assert_eq!(sb.entries(Client::GeminiCli, Scope::User).len(), 2);
    assert_eq!(read(&sb.user(Client::Zed)), "{ // mine\n}\n");
}

#[test]
fn clients_lists_what_is_registered() {
    let sb = Sandbox::mac();
    assert_eq!(sb.install(&["cursor"], |r| r.only = Only::Dev).code, 0);
    let mut out = Vec::new();
    let code = clients(
        &sb.env,
        &ClientsRequest {
            name: "seleniumbase".to_owned(),
        },
        &mut out,
        &mut Vec::new(),
    );
    assert_eq!(code, 0);
    let text = String::from_utf8(out).unwrap();
    for client in Client::ALL {
        assert!(
            text.contains(&format!("{}  {}", client.id(), client.title())),
            "{text}"
        );
        assert!(text.contains(client.docs()), "{text}");
    }
    let cursor = text.split("cursor  Cursor").nth(1).unwrap();
    let cursor = cursor.split("gemini-cli").next().unwrap();
    assert!(cursor.contains("(found)"), "{cursor}");
    assert!(
        cursor.contains("user: seleniumbase-dev; project: none"),
        "{cursor}"
    );
    assert!(text.contains("claude (not found on PATH)"), "{text}");
}

// -- Codex's config.toml ----------------------------------------------------------------------

const CODEX_CONFIG: &str =
    "# my codex config\nmodel = \"o3\"   # the model\n\n[projects.\"/work\"]\n\
trust_level = \"trusted\"\n\n# an unrelated server\n[mcp_servers.other]\ncommand = \"keep-me\"\n\
args = [\"-x\"]\n";

#[test]
fn codex_config_toml_keeps_its_comments_and_other_tables() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Codex);
    sb.put(&path, CODEX_CONFIG);
    let outcome = sb.install(&["codex"], |_| {});
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let text = read(&path);
    assert!(text.starts_with(CODEX_CONFIG), "{text}");
    assert!(
        text.contains(
            "[mcp_servers.seleniumbase]\ncommand = \"/opt/sbrs/sbrs-mcp\"\nargs = [\"--server\", \"cdp\"]\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("[mcp_servers.seleniumbase.env]\nSB_MCP_OUTPUT_DIR = "),
        "{text}"
    );
    assert!(text.contains("[mcp_servers.seleniumbase-dev]"), "{text}");
    assert!(text.contains("args = [\"--server\", \"dev\"]"), "{text}");
    assert_eq!(read(&backup_path(&path)), CODEX_CONFIG);

    let doc: toml_edit::DocumentMut = text.parse().unwrap();
    assert_eq!(doc["model"].as_str(), Some("o3"));
    assert_eq!(
        doc["mcp_servers"]["other"]["command"].as_str(),
        Some("keep-me")
    );
    assert_eq!(
        doc["mcp_servers"]["seleniumbase"]["env"]["SB_MCP_OUTPUT_DIR"].as_str(),
        sb.output_dir().to_str()
    );
}

#[test]
fn codex_install_into_a_new_file_and_again_changes_nothing() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Codex);
    let outcome = sb.install(&["codex"], |_| {});
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let first = read(&path);
    assert!(first.starts_with("[mcp_servers.seleniumbase]\n"), "{first}");
    assert!(!backup_path(&path).exists());
    let outcome = sb.install(&["codex"], |_| {});
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert_eq!(read(&path), first);
    assert!(outcome.out.contains("unchanged"), "{}", outcome.out);
}

#[test]
fn codex_a_different_entry_needs_force() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Codex);
    let original = "[mcp_servers.seleniumbase]\ncommand = \"/somewhere/else\"\nargs = [\"--server\", \"sb\"]\n# tuned\nstartup_timeout_sec = 30\n";
    sb.put(&path, original);
    let outcome = sb.install(&["codex"], |r| r.only = Only::Browser);
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    assert_eq!(read(&path), original);
    let outcome = sb.install(&["codex"], |r| {
        r.only = Only::Browser;
        r.force = true;
    });
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let text = read(&path);
    assert!(text.contains("command = \"/opt/sbrs/sbrs-mcp\""), "{text}");
    assert!(!text.contains("/somewhere/else"), "{text}");
}

#[test]
fn codex_uninstall_removes_the_tables_and_keeps_the_rest() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Codex);
    sb.put(&path, CODEX_CONFIG);
    assert_eq!(sb.install(&["codex"], |_| {}).code, 0);
    let outcome = sb.uninstall(&["codex"], |_| {});
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    let text = read(&path);
    assert!(!text.contains("seleniumbase"), "{text}");
    assert!(text.starts_with(CODEX_CONFIG), "{text}");
    let again = sb.uninstall(&["codex"], |_| {});
    assert_eq!(
        again.out.matches("not installed").count(),
        2,
        "{}",
        again.out
    );
}

#[test]
fn codex_print_shows_the_toml_and_writes_nothing() {
    let sb = Sandbox::mac();
    let outcome = sb.install(&["codex"], |r| r.dry_run = true);
    assert_eq!(outcome.code, 0, "{}", outcome.err);
    assert!(!sb.user(Client::Codex).exists());
    assert!(
        outcome.out.contains("[mcp_servers.seleniumbase]"),
        "{}",
        outcome.out
    );
    assert!(
        outcome.out.contains("[mcp_servers.seleniumbase-dev]"),
        "{}",
        outcome.out
    );
}

#[test]
fn a_config_toml_that_is_not_toml_is_not_rewritten() {
    let sb = Sandbox::mac();
    let path = sb.user(Client::Codex);
    sb.put(&path, "model = = broken\n");
    let outcome = sb.install(&["codex"], |_| {});
    assert_eq!(outcome.code, 2, "{}", outcome.err);
    assert_eq!(read(&path), "model = = broken\n");
}

// -- the clients' own commands, as scripts on a PATH of the test's own ----------------------------

#[cfg(unix)]
mod fake_cli {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    /// One run of a fake program.
    #[derive(Debug, Eq, PartialEq)]
    struct Call {
        cwd: PathBuf,
        args: Vec<String>,
    }

    /// A `PATH` folder with the fake programs, and where they log.
    struct Bin {
        dir: PathBuf,
        log: PathBuf,
    }

    impl Bin {
        fn new(sb: &Sandbox) -> Self {
            let dir = sb.root.join("bin");
            fs::create_dir_all(&dir).unwrap();
            Self {
                dir,
                log: sb.root.join("calls.log"),
            }
        }

        /// A script called `name` that logs its working folder and arguments after `before`
        /// has had its say, says `message` on standard error, and exits with `code`.
        fn script(&self, name: &str, before: &str, code: i32, message: &str) {
            let path = self.dir.join(name);
            let text = format!(
                "#!/bin/sh\n{before}\n{{ printf 'PWD=%s\\n' \"$(pwd)\"; for a in \"$@\"; do \
                 printf 'ARG=%s\\n' \"$a\"; done; printf 'END\\n'; }} >> '{}'\n\
                 [ -n '{message}' ] && echo '{message}' >&2\nexit {code}\n",
                self.log.display()
            );
            fs::write(&path, text).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        fn claude(&self) {
            self.script("claude", "", 0, "");
        }

        /// A `codex` whose `mcp --help` lists `add` and `remove`.
        fn codex(&self) {
            self.script(
                "codex",
                "if [ \"$1\" = mcp ] && [ \"$2\" = --help ]; then printf 'Commands:\\n  list\\n  get\\n  add\\n  remove\\n'; exit 0; fi",
                0,
                "",
            );
        }

        fn calls(&self) -> Vec<Call> {
            let Ok(text) = fs::read_to_string(&self.log) else {
                return Vec::new();
            };
            text.split("END\n")
                .filter(|block| !block.is_empty())
                .map(|block| {
                    let mut lines = block.lines();
                    let cwd = PathBuf::from(lines.next().unwrap().strip_prefix("PWD=").unwrap());
                    let args = lines
                        .map(|line| line.strip_prefix("ARG=").unwrap().to_owned())
                        .collect();
                    Call { cwd, args }
                })
                .collect()
        }
    }

    fn sandbox_with_bin() -> (Sandbox, Bin) {
        let mut sb = Sandbox::mac();
        let bin = Bin::new(&sb);
        sb.env.path = Some(bin.dir.clone().into_os_string());
        (sb, bin)
    }

    fn strs(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| (*a).to_owned()).collect()
    }

    /// A `~/.claude.json` that holds the entry `name`, among other keys.
    fn claude_json(spec: &LaunchSpec, name: &str) -> String {
        let layout = Client::ClaudeCode.layout().unwrap();
        format!(
            "{{\n  \"numStartups\": 3,\n  \"mcpServers\": {{\n    \"{name}\": {}\n  }}\n}}\n",
            layout.entry(spec)
        )
    }

    #[test]
    fn claude_code_is_registered_with_claude_mcp_add_json_and_its_file_is_not_written() {
        let (sb, bin) = sandbox_with_bin();
        bin.claude();
        let outcome = sb.install(&["claude-code"], |_| {});
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let calls = bin.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        let layout = Client::ClaudeCode.layout().unwrap();
        assert_eq!(
            calls[0].args,
            strs(&[
                "mcp",
                "add-json",
                "--scope",
                "user",
                "seleniumbase",
                &layout.entry(&sb.browser_spec()).to_string()
            ])
        );
        assert_eq!(
            calls[1].args,
            strs(&[
                "mcp",
                "add-json",
                "--scope",
                "user",
                "seleniumbase-dev",
                &layout.entry(&sb.dev_spec()).to_string()
            ])
        );
        assert!(
            !sb.user(Client::ClaudeCode).exists(),
            "~/.claude.json is the CLI's to write"
        );
        assert!(outcome.out.contains("added"), "{}", outcome.out);
        let json: Value = serde_json::from_str(calls[0].args.last().unwrap()).unwrap();
        assert_eq!(json["type"], "stdio");
        assert_eq!(json["command"], EXE);
        assert_eq!(
            json["env"]["SB_MCP_OUTPUT_DIR"],
            sb.output_dir().to_str().unwrap()
        );
    }

    #[test]
    fn claude_code_project_scope_runs_in_the_project_folder() {
        let (sb, bin) = sandbox_with_bin();
        bin.claude();
        let outcome = sb.install(&["claude-code"], |r| {
            r.scope = Scope::Project;
            r.only = Only::Browser;
        });
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let calls = bin.calls();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(
            calls[0].args[..4],
            strs(&["mcp", "add-json", "--scope", "project"])[..]
        );
        assert_eq!(calls[0].cwd, sb.env.cwd);
    }

    #[test]
    fn claude_code_leaves_an_identical_entry_alone() {
        let (sb, bin) = sandbox_with_bin();
        bin.claude();
        sb.put(
            &sb.user(Client::ClaudeCode),
            &claude_json(&sb.browser_spec(), "seleniumbase"),
        );
        let outcome = sb.install(&["claude-code"], |r| r.only = Only::Browser);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(bin.calls().is_empty());
        assert!(outcome.out.contains("unchanged"), "{}", outcome.out);
    }

    #[test]
    fn claude_code_reports_a_different_entry_and_force_removes_then_adds() {
        let (sb, bin) = sandbox_with_bin();
        bin.claude();
        sb.put(
            &sb.user(Client::ClaudeCode),
            &claude_json(&different_spec(), "seleniumbase"),
        );
        let outcome = sb.install(&["claude-code"], |r| r.only = Only::Browser);
        assert_eq!(outcome.code, 2, "{}", outcome.err);
        assert!(bin.calls().is_empty(), "nothing is run for a conflict");
        let outcome = sb.install(&["claude-code"], |r| {
            r.only = Only::Browser;
            r.force = true;
        });
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let calls = bin.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(
            calls[0].args,
            strs(&["mcp", "remove", "--scope", "user", "seleniumbase"])
        );
        assert_eq!(
            calls[1].args[..5],
            strs(&["mcp", "add-json", "--scope", "user", "seleniumbase"])[..]
        );
        assert!(outcome.out.contains("replaced"), "{}", outcome.out);
    }

    #[test]
    fn claude_code_uninstall_removes_the_entries_that_are_there() {
        let (sb, bin) = sandbox_with_bin();
        bin.claude();
        sb.put(
            &sb.user(Client::ClaudeCode),
            &claude_json(&sb.dev_spec(), "seleniumbase-dev"),
        );
        let outcome = sb.uninstall(&["claude-code"], |_| {});
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let calls = bin.calls();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(
            calls[0].args,
            strs(&["mcp", "remove", "--scope", "user", "seleniumbase-dev"])
        );
        assert!(outcome.out.contains("not installed"), "{}", outcome.out);
        assert!(outcome.out.contains("removed"), "{}", outcome.out);
    }

    #[test]
    fn claude_code_print_runs_nothing() {
        let (sb, bin) = sandbox_with_bin();
        bin.claude();
        let outcome = sb.install(&["claude-code"], |r| r.dry_run = true);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(bin.calls().is_empty());
        assert!(!bin.log.exists());
        assert!(
            outcome
                .out
                .contains("would run: claude mcp add-json --scope user seleniumbase "),
            "{}",
            outcome.out
        );
        assert!(outcome.out.contains("seleniumbase-dev"), "{}", outcome.out);
    }

    #[test]
    fn without_claude_on_path_the_commands_are_printed_and_the_exit_status_is_2() {
        let sb = Sandbox::mac();
        let outcome = sb.install(&["claude-code"], |_| {});
        assert_eq!(outcome.code, 2, "{}", outcome.err);
        assert!(
            outcome.err.contains("`claude` is not on PATH"),
            "{}",
            outcome.err
        );
        assert!(
            outcome
                .out
                .contains("claude mcp add-json --scope user seleniumbase "),
            "{}",
            outcome.out
        );
        assert!(
            outcome
                .out
                .contains("claude mcp add-json --scope user seleniumbase-dev "),
            "{}",
            outcome.out
        );
        assert!(
            !sb.user(Client::ClaudeCode).exists(),
            "the file is never written directly"
        );
    }

    #[test]
    fn print_without_claude_on_path_stops_where_the_real_run_would() {
        let sb = Sandbox::mac();
        let outcome = sb.install(&["claude-code"], |r| r.dry_run = true);
        assert_eq!(outcome.code, 2, "{}", outcome.err);
        assert!(
            outcome.err.contains("`claude` is not on PATH"),
            "{}",
            outcome.err
        );
        assert!(
            outcome
                .out
                .contains("claude mcp add-json --scope user seleniumbase "),
            "{}",
            outcome.out
        );
    }

    #[test]
    fn a_claude_that_fails_is_a_failure_with_its_message() {
        let (sb, bin) = sandbox_with_bin();
        bin.script(
            "claude",
            "",
            1,
            "MCP server seleniumbase already exists in user config",
        );
        let outcome = sb.install(&["claude-code"], |_| {});
        assert_eq!(outcome.code, 1, "{}", outcome.err);
        assert!(
            outcome.err.contains("already exists in user config"),
            "{}",
            outcome.err
        );
        assert_eq!(bin.calls().len(), 1, "it stops at the first failure");
    }

    fn codex_add_args(sb: &Sandbox, name: &str, args: &[&str]) -> Vec<String> {
        let mut all = strs(&["mcp", "add", name, "--env"]);
        all.push(format!("SB_MCP_OUTPUT_DIR={}", sb.output_dir().display()));
        all.push("--".to_owned());
        all.push(EXE.to_owned());
        all.extend(strs(args));
        all
    }

    #[test]
    fn codex_is_registered_with_codex_mcp_add_when_it_has_the_command() {
        let (sb, bin) = sandbox_with_bin();
        bin.codex();
        let outcome = sb.install(&["codex"], |_| {});
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let calls = bin.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(
            calls[0].args,
            codex_add_args(&sb, "seleniumbase", &["--server", "cdp"])
        );
        assert_eq!(
            calls[1].args,
            codex_add_args(&sb, "seleniumbase-dev", &["--server", "dev"])
        );
        assert!(!sb.user(Client::Codex).exists(), "codex wrote it, not us");
    }

    #[test]
    fn codex_without_mcp_commands_is_edited_through_config_toml() {
        let (sb, bin) = sandbox_with_bin();
        bin.script("codex", "printf 'Commands:\\n  list\\n'; exit 0", 0, "");
        let outcome = sb.install(&["codex"], |_| {});
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(bin.calls().is_empty());
        assert!(read(&sb.user(Client::Codex)).contains("[mcp_servers.seleniumbase]"));
    }

    #[test]
    fn codex_cli_leaves_an_identical_entry_alone_and_force_removes_then_adds() {
        let (sb, bin) = sandbox_with_bin();
        bin.codex();
        let path = sb.user(Client::Codex);
        sb.put(
            &path,
            &format!(
                "[mcp_servers.seleniumbase]\ncommand = \"{EXE}\"\nargs = [\"--server\", \"cdp\"]\n\n\
                 [mcp_servers.seleniumbase.env]\nSB_MCP_OUTPUT_DIR = {}\n",
                Value::String(sb.output_dir().to_str().unwrap().to_owned())
            ),
        );
        let outcome = sb.install(&["codex"], |r| r.only = Only::Browser);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(bin.calls().is_empty());

        let outcome = sb.install(&["codex"], |r| {
            r.only = Only::Browser;
            r.browser = "sb".to_owned();
        });
        assert_eq!(outcome.code, 2, "{}", outcome.err);
        assert!(bin.calls().is_empty());

        let outcome = sb.install(&["codex"], |r| {
            r.only = Only::Browser;
            r.browser = "sb".to_owned();
            r.force = true;
        });
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let calls = bin.calls();
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert_eq!(calls[0].args, strs(&["mcp", "remove", "seleniumbase"]));
        assert_eq!(
            calls[1].args,
            codex_add_args(&sb, "seleniumbase", &["--server", "sb"])
        );
    }

    #[test]
    fn codex_cli_uninstall_and_print() {
        let (sb, bin) = sandbox_with_bin();
        bin.codex();
        sb.put(
            &sb.user(Client::Codex),
            "[mcp_servers.seleniumbase-dev]\ncommand = \"x\"\n",
        );
        let outcome = sb.uninstall(&["codex"], |r| r.dry_run = true);
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        assert!(bin.calls().is_empty());
        assert!(
            outcome
                .out
                .contains("would run: codex mcp remove seleniumbase-dev"),
            "{}",
            outcome.out
        );
        let outcome = sb.uninstall(&["codex"], |_| {});
        assert_eq!(outcome.code, 0, "{}", outcome.err);
        let calls = bin.calls();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(calls[0].args, strs(&["mcp", "remove", "seleniumbase-dev"]));
    }

    #[test]
    fn clients_shows_the_programs_it_finds_and_runs_nothing() {
        let (sb, bin) = sandbox_with_bin();
        bin.claude();
        let mut out = Vec::new();
        clients(
            &sb.env,
            &ClientsRequest {
                name: "seleniumbase".to_owned(),
            },
            &mut out,
            &mut Vec::new(),
        );
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains(&format!(
                "claude (found: {})",
                bin.dir.join("claude").display()
            )),
            "{text}"
        );
        assert!(text.contains("codex (not found on PATH)"), "{text}");
        assert!(bin.calls().is_empty());
    }
}
