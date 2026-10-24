//! The `sbrs-mcp` binary with plugins: `--plugin`, `--plugin-opt`, the
//! `[plugins]` table of the settings file and `SB_PLUGINS` choose the plugins
//! every browser the server starts gets.
//!
//! The real server runs over stdio, and no browser is started. The `driver`
//! and `sb` servers (WebDriver) talk to a fake WebDriver server on a loopback
//! port, whose new-session request shows the switches the plugins added; the
//! `cdp` server launches a script that records its arguments and exits, so
//! the launch fails after the plugins have had their say. What a plugin does
//! to a real Chrome is tested in the library.
//! Run with `cargo test -p seleniumbase-rs-cli`.

#![cfg(all(unix, feature = "mcp-server", feature = "plugins-builtin"))]

#[path = "support/fake_chrome.rs"]
mod fake_chrome;
#[path = "support/fake_webdriver.rs"]
mod fake_webdriver;
#[path = "support/sbrs.rs"]
mod sbrs;

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

use fake_chrome::FakeChrome;
use fake_webdriver::FakeWebDriver;
use sbrs::{command_in, stderr};

const MCP: &str = env!("CARGO_BIN_EXE_sbrs-mcp");

/// A switch shield adds to every browser it launches.
const SHIELD_SWITCH: &str = "--disable-background-networking";

const STEP: Duration = Duration::from_secs(60);

/// A server over a pipe, and the client side of the conversation.
struct Server {
    /// Held so that the server is killed when the test ends.
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Server {
    /// Starts `sbrs-mcp args...` in `dir` with `env` set (the `SB_*` variables
    /// of the host are removed) and completes the MCP handshake.
    async fn start(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> Self {
        let mut command = Command::from(command_in(MCP, dir, env));
        let mut child = command
            .args(args)
            .env("SB_SHUTDOWN_TIMEOUT_SECS", "5")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("start sbrs-mcp");
        let stdin = child.stdin.take().expect("a pipe to the server");
        let stdout = BufReader::new(child.stdout.take().expect("a pipe from the server"));
        let mut server = Self {
            _child: child,
            stdin,
            stdout,
            next_id: 1,
        };
        let reply = server
            .request(
                "initialize",
                json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "test", "version": "0" },
                }),
            )
            .await;
        assert!(reply.get("result").is_some(), "unexpected reply: {reply}");
        server
            .send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .await;
        server
    }

    async fn send(&mut self, message: &Value) {
        let line = message.to_string();
        self.stdin.write_all(line.as_bytes()).await.unwrap();
        self.stdin.write_all(b"\n").await.unwrap();
        self.stdin.flush().await.unwrap();
    }

    /// Sends a request and returns the reply that has its id.
    async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))
            .await;
        loop {
            let mut line = String::new();
            let read = timeout(STEP, self.stdout.read_line(&mut line))
                .await
                .expect("the server answered in time")
                .unwrap();
            assert!(
                read > 0,
                "the server closed its output before answering {method}"
            );
            let message: Value = serde_json::from_str(&line).expect("a JSON line");
            if message["id"] == json!(id) {
                return message;
            }
        }
    }

    /// Calls a tool and returns its result.
    async fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let reply = self
            .request(
                "tools/call",
                json!({ "name": tool, "arguments": arguments }),
            )
            .await;
        reply["result"].clone()
    }
}

fn text_of(result: &Value) -> String {
    result["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The arguments of the Chrome a WebDriver server `profile` asked the fake
/// server for, when it starts a browser with `start_browser`.
async fn webdriver_chrome_args(
    dir: &Path,
    extra_env: &[(&str, &str)],
    profile: &str,
    flags: &[&str],
) -> Vec<String> {
    let driver = FakeWebDriver::start();
    let mut env = vec![("SB_WEBDRIVER_URL", driver.url())];
    env.extend_from_slice(extra_env);
    let mut args = vec!["--server", profile];
    args.extend_from_slice(flags);
    let mut server = Server::start(dir, &env, &args).await;

    let result = server
        .call(
            "start_browser",
            json!({ "browser": "chrome", "headless": true, "uc": false }),
        )
        .await;

    assert_ne!(result["isError"], json!(true), "{}", text_of(&result));
    driver.chrome_args().expect("a chrome session")
}

#[tokio::test]
async fn the_driver_and_sb_servers_start_their_browsers_with_the_plugins_of_the_flags() {
    let dir = tempfile::tempdir().unwrap();
    for profile in ["driver", "sb"] {
        let with = webdriver_chrome_args(dir.path(), &[], profile, &["--plugin", "shield"]).await;
        assert!(
            with.iter().any(|arg| arg == SHIELD_SWITCH),
            "{profile}: {with:?}"
        );

        let without = webdriver_chrome_args(dir.path(), &[], profile, &[]).await;
        assert!(
            !without.iter().any(|arg| arg == SHIELD_SWITCH),
            "{profile} without the flag: {without:?}"
        );
    }
}

#[tokio::test]
async fn a_plugin_option_flag_reaches_the_plugin_of_the_server() {
    let dir = tempfile::tempdir().unwrap();

    let args = webdriver_chrome_args(
        dir.path(),
        &[],
        "driver",
        &[
            "--plugin",
            "shield",
            "--plugin-opt",
            r#"shield.level="strict""#,
        ],
    )
    .await;

    assert!(
        args.iter()
            .any(|arg| arg.starts_with("--host-resolver-rules=")),
        "{args:?}"
    );
}

#[tokio::test]
async fn the_settings_file_and_sb_plugins_choose_the_plugins_too() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "[plugins]\nenable = [\"shield\"]\n",
    )
    .unwrap();

    let from_file = webdriver_chrome_args(dir.path(), &[], "driver", &[]).await;
    assert!(
        from_file.iter().any(|arg| arg == SHIELD_SWITCH),
        "{from_file:?}"
    );

    let off = webdriver_chrome_args(dir.path(), &[("SB_PLUGINS", "")], "driver", &[]).await;
    assert!(
        !off.iter().any(|arg| arg == SHIELD_SWITCH),
        "SB_PLUGINS= beats the file: {off:?}"
    );

    let empty = tempfile::tempdir().unwrap();
    let from_variable =
        webdriver_chrome_args(empty.path(), &[("SB_PLUGINS", "shield")], "sb", &[]).await;
    assert!(
        from_variable.iter().any(|arg| arg == SHIELD_SWITCH),
        "{from_variable:?}"
    );
}

#[tokio::test]
async fn a_settings_file_that_is_wrong_for_a_browser_does_not_stop_the_server() {
    // `headless` is not a boolean: only the `[plugins]` table of the file is read.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "headless = \"maybe\"\n[plugins]\nenable = [\"shield\"]\n",
    )
    .unwrap();

    let args = webdriver_chrome_args(dir.path(), &[], "driver", &[]).await;

    assert!(args.iter().any(|arg| arg == SHIELD_SWITCH), "{args:?}");
}

#[test]
fn each_active_plugin_is_logged_with_the_place_that_turned_it_on() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("sbase_config.toml"),
        "[plugins]\nenable = [\"shield\"]\n",
    )
    .unwrap();

    // The server ends when its input does; what it logged on the way is what counts.
    let output = command_in(MCP, dir.path(), &[("SB_LOG_LEVEL", "info")])
        .args(["--server", "cdp", "--plugin", "block-resources"])
        .stdin(Stdio::null())
        .output()
        .expect("sbrs-mcp runs");

    let log = stderr(&output);
    assert!(
        log.contains("shield") && log.contains("the settings file in the current folder"),
        "{log}"
    );
    assert!(
        log.contains("block-resources") && log.contains("--plugin"),
        "{log}"
    );
}

#[tokio::test]
async fn the_cdp_server_launches_its_browser_with_the_plugins() {
    let dir = tempfile::tempdir().unwrap();
    for (flags, expected) in [(&["--plugin", "shield"][..], true), (&[][..], false)] {
        let chrome = FakeChrome::new();
        let mut args = vec!["--server", "cdp"];
        args.extend_from_slice(flags);
        let mut server = Server::start(dir.path(), &[], &args).await;

        // The script exits at once, so the launch fails; what counts is how it was asked.
        let result = server
            .call(
                "start_browser",
                json!({
                    "browser_executable_path": chrome.path().to_string_lossy(),
                    "headless": true,
                }),
            )
            .await;

        let started = chrome
            .args()
            .unwrap_or_else(|| panic!("the browser was started: {}", text_of(&result)));
        assert_eq!(
            started.iter().any(|arg| arg == SHIELD_SWITCH),
            expected,
            "{flags:?}: {started:?}"
        );
    }
}

#[tokio::test]
async fn no_tool_argument_adds_a_plugin() {
    let dir = tempfile::tempdir().unwrap();
    let chrome = FakeChrome::new();
    let mut server = Server::start(dir.path(), &[], &["--server", "cdp"]).await;

    server
        .call(
            "start_browser",
            json!({
                "browser_executable_path": chrome.path().to_string_lossy(),
                "headless": true,
                "plugins": ["shield"],
            }),
        )
        .await;

    // Either the server ignores the argument and starts the browser without
    // the plugin, or it refuses the argument and starts none.
    let started = chrome.args();
    assert!(
        started
            .as_ref()
            .is_none_or(|args| !args.iter().any(|arg| arg == SHIELD_SWITCH)),
        "the operator chooses the plugins, not the model: {started:?}"
    );
}

/// The text a tool call that failed says.
fn failure_of(result: &Value) -> String {
    assert_eq!(result["isError"], json!(true), "the call failed: {result}");
    text_of(result)
}

#[tokio::test]
async fn the_cdp_server_offers_the_run_plugin_task_tool() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = Server::start(dir.path(), &[], &["--server", "cdp"]).await;

    let reply = server.request("tools/list", json!({})).await;

    let tools = reply["result"]["tools"].as_array().expect("a tool list");
    let tool = tools
        .iter()
        .find(|tool| tool["name"] == "run_plugin_task")
        .unwrap_or_else(|| panic!("run_plugin_task is listed: {reply}"));
    assert_eq!(tool["inputSchema"]["required"], json!(["name"]));
    for argument in ["name", "input", "plugins", "headless"] {
        assert!(
            tool["inputSchema"]["properties"].get(argument).is_some(),
            "{argument}: {tool}"
        );
    }
}

#[tokio::test]
async fn run_plugin_task_refuses_a_plugin_the_operator_did_not_enable() {
    let dir = tempfile::tempdir().unwrap();
    let mut server = Server::start(dir.path(), &[], &["--server", "cdp"]).await;

    let result = server
        .call("run_plugin_task", json!({ "name": "shield" }))
        .await;

    let text = failure_of(&result);
    assert!(
        text.contains("`shield` is not enabled for this server"),
        "{text}"
    );
    assert!(text.contains("--plugin NAME"), "{text}");
}

#[tokio::test]
async fn run_plugin_task_checks_the_task_the_input_and_the_plugin_list_before_any_browser_starts() {
    let dir = tempfile::tempdir().unwrap();
    let mut server =
        Server::start(dir.path(), &[], &["--server", "cdp", "--plugin", "shield"]).await;

    // Shield is enabled, but it reacts to a browser and has no task.
    let no_task = server
        .call("run_plugin_task", json!({ "name": "shield" }))
        .await;
    assert!(
        failure_of(&no_task).contains("plugin `shield` offers no task"),
        "{no_task}"
    );

    let bad_input = server
        .call(
            "run_plugin_task",
            json!({ "name": "shield", "input": "{not json" }),
        )
        .await;
    let text = failure_of(&bad_input);
    assert!(text.contains("invalid argument 'input'"), "{text}");

    let more = server
        .call(
            "run_plugin_task",
            json!({ "name": "shield", "plugins": "shield,crx" }),
        )
        .await;
    let text = failure_of(&more);
    assert!(text.contains("invalid argument 'plugins'"), "{text}");
    assert!(text.contains("`crx` is not enabled"), "{text}");
}

/// `sbrs-mcp args...` in an empty folder, which has to end at once.
fn refused(env: &[(&str, &str)], args: &[&str]) -> std::process::Output {
    let dir = tempfile::tempdir().unwrap();
    let output = command_in(MCP, dir.path(), env)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("sbrs-mcp runs");
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    output
}

#[test]
fn a_mistake_in_the_plugin_options_stops_the_server_before_it_listens() {
    let unknown = refused(&[], &["--server", "cdp", "--plugin", "sheild"]);
    let message = stderr(&unknown);
    assert!(message.contains("`sheild`"), "{message}");
    assert!(
        message.contains("known: shield, block-resources, crx"),
        "{message}"
    );

    let from_variable = refused(&[("SB_PLUGINS", "sheild")], &["--server", "cdp"]);
    assert!(stderr(&from_variable).contains("`sheild`"));

    let orphan = refused(&[], &["--plugin-opt", r#"shield.level="strict""#]);
    assert!(
        stderr(&orphan).contains("--plugin shield"),
        "{}",
        stderr(&orphan)
    );

    let refused_option = refused(
        &[],
        &[
            "--plugin",
            "shield",
            "--plugin-opt",
            r#"shield.lvel="strict""#,
        ],
    );
    assert!(
        stderr(&refused_option).contains("lvel"),
        "{}",
        stderr(&refused_option)
    );

    let bare = refused(
        &[],
        &["--plugin", "shield", "--plugin-opt", "shield.level=strict"],
    );
    assert!(
        stderr(&bare).contains("not a TOML value"),
        "{}",
        stderr(&bare)
    );
}

#[test]
fn help_describes_the_plugin_options_and_their_precedence() {
    let dir = tempfile::tempdir().unwrap();

    let output = command_in(MCP, dir.path(), &[])
        .arg("--help")
        .output()
        .expect("sbrs-mcp runs");

    assert!(output.status.success(), "{}", stderr(&output));
    let help = sbrs::words(&sbrs::stdout(&output)).join(" ");
    assert!(help.contains("--plugin"), "{help}");
    assert!(help.contains("--plugin-opt"), "{help}");
    assert!(
        help.contains("the command line, then SB_PLUGINS"),
        "the precedence is in the help: {help}"
    );
    assert!(help.contains("no tool argument adds a plugin"), "{help}");
}
