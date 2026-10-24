//! The client commands of the real `sbrs-mcp` binary: `tools`, `call`, `batch`, `resources`,
//! `read`, `prompts` and `prompt`, against a server the binary starts itself over stdio (`--server`,
//! `--connect`) and against one it connects to over HTTP (`--url`). No browser is started: the
//! `dev` server has none, and the `cdp` server only starts Chrome when a tool asks it to.
//! Run with `cargo test -p seleniumbase-rs-cli --test mcp_client`.

#![cfg(feature = "mcp-client")]

#[path = "support/mcp_http.rs"]
mod support;

use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use support::Server;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

const TOKEN: &str = "a-client-token-for-the-tests";

/// What a run of `sbrs-mcp` did.
struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Runs `sbrs-mcp` with `args` and `env` (and `stdin` as its standard input) to its end.
async fn sbrs_mcp(args: &[&str], env: &[(&str, &str)], stdin: Option<&str>) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sbrs-mcp"));
    command
        .args(args)
        // The environment of the test run must not choose the server or hand over a token.
        .env_remove("SB_MCP_TOKEN")
        .env_remove("SB_MCP_URL")
        .env("SB_SHUTDOWN_TIMEOUT_SECS", "5")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("start sbrs-mcp");
    let mut input = child.stdin.take().expect("standard input");
    if let Some(text) = stdin {
        input.write_all(text.as_bytes()).await.expect("write stdin");
    }
    drop(input);
    let output = timeout(support::STEP, child.wait_with_output())
        .await
        .expect("the command ended in time")
        .expect("run sbrs-mcp");
    Run {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn assert_ok(run: &Run) {
    assert_eq!(
        run.code,
        Some(0),
        "stdout: {}\nstderr: {}",
        run.stdout,
        run.stderr
    );
}

#[tokio::test]
async fn help_lists_the_client_commands_and_the_connection_flags() {
    let top = sbrs_mcp(&["--help"], &[], None).await;
    let call = sbrs_mcp(&["call", "--help"], &[], None).await;

    assert_ok(&top);
    for command in [
        "tools",
        "call",
        "batch",
        "resources",
        "read",
        "prompts",
        "prompt",
    ] {
        assert!(
            top.stdout.contains(&format!("  {command}")),
            "{command}: {}",
            top.stdout
        );
    }
    assert_ok(&call);
    for flag in [
        "--url",
        "--token-file",
        "--connect",
        "--server",
        "--timeout",
        "--arg",
        "--raw",
    ] {
        assert!(call.stdout.contains(flag), "{flag}: {}", call.stdout);
    }
}

// ----------------------------------------------------------------------
// Over stdio: the program starts the server
// ----------------------------------------------------------------------

#[tokio::test]
async fn tools_with_no_flags_start_the_dev_server_and_list_its_tools() {
    let run = sbrs_mcp(&["tools"], &[], None).await;

    assert_ok(&run);
    assert!(run.stdout.contains("find_api"), "{}", run.stdout);
    assert!(run.stdout.contains("explain_error"), "{}", run.stdout);
    assert!(
        run.stderr.is_empty(),
        "the server it starts logs at warn: {}",
        run.stderr
    );
}

#[tokio::test]
async fn raw_tools_are_json_with_their_schemas() {
    let run = sbrs_mcp(&["tools", "--raw", "--server", "dev"], &[], None).await;

    assert_ok(&run);
    let tools: Value = serde_json::from_str(run.stdout.trim()).expect("one JSON array");
    let find = tools
        .as_array()
        .expect("an array")
        .iter()
        .find(|tool| tool["name"] == "find_api")
        .expect("find_api");
    assert_eq!(find["inputSchema"]["type"], "object");
}

#[tokio::test]
async fn one_tool_is_shown_with_the_schema_of_its_arguments() {
    let run = sbrs_mcp(&["tools", "find_api"], &[], None).await;

    assert_ok(&run);
    assert!(run.stdout.starts_with("find_api\n"), "{}", run.stdout);
    assert!(run.stdout.contains("\"query\""), "{}", run.stdout);
}

#[tokio::test]
async fn a_tool_the_server_lacks_is_a_mistake() {
    let run = sbrs_mcp(&["tools", "no_such_tool"], &[], None).await;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(
        run.stderr.contains("no tool no_such_tool"),
        "{}",
        run.stderr
    );
}

#[tokio::test]
async fn call_takes_its_arguments_as_options_json_or_standard_input() {
    for args in [
        &["call", "find_api", "--arg", "query=click"][..],
        &["call", "find_api", r#"{"query":"click"}"#][..],
        &["call", "find_api", "--json", r#"{"query":"click"}"#][..],
    ] {
        let run = sbrs_mcp(args, &[], None).await;
        assert_ok(&run);
        assert!(run.stdout.contains("click"), "{args:?}: {}", run.stdout);
    }
    let from_stdin = sbrs_mcp(
        &["call", "find_api", "-"],
        &[],
        Some(r#"{"query":"click"}"#),
    )
    .await;
    assert_ok(&from_stdin);
    assert!(from_stdin.stdout.contains("click"), "{}", from_stdin.stdout);
}

#[tokio::test]
async fn raw_prints_the_structured_result_as_one_line() {
    let run = sbrs_mcp(
        &["call", "--raw", "find_api", "--arg", "query=click"],
        &[],
        None,
    )
    .await;

    assert_ok(&run);
    assert_eq!(run.stdout.trim().lines().count(), 1, "{}", run.stdout);
    let value: Value = serde_json::from_str(run.stdout.trim()).expect("JSON");
    assert!(value.is_object() || value.is_array(), "{value}");
}

#[tokio::test]
async fn an_unknown_tool_exits_2_with_the_nearest_name() {
    let run = sbrs_mcp(&["call", "find_ap"], &[], None).await;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(
        run.stderr.contains("did you mean find_api"),
        "{}",
        run.stderr
    );
    assert!(run.stdout.is_empty(), "{}", run.stdout);
}

#[tokio::test]
async fn a_tool_that_reports_a_failure_exits_1_and_prints_its_report() {
    // `find_api` needs a `query`.
    let run = sbrs_mcp(&["call", "find_api"], &[], None).await;

    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert!(run.stdout.contains("find_api failed"), "{}", run.stdout);
}

#[tokio::test]
async fn mistakes_in_the_arguments_exit_2_before_a_server_starts() {
    for args in [
        &["call", "find_api", "--arg", "novalue"][..],
        &["call", "find_api", "[1]"][..],
        &["call", "find_api", "{}", "--arg", "query=x"][..],
        &[
            "call",
            "find_api",
            "--url",
            "ftp://x.test",
            "--arg",
            "query=x",
        ][..],
        &["call", "find_api", "--server", "dev", "--connect", "x"][..],
        &["tools", "--timeout", "0"][..],
    ] {
        let run = sbrs_mcp(args, &[], None).await;
        assert_eq!(run.code, Some(2), "{args:?}: {}", run.stderr);
    }
}

#[tokio::test]
async fn any_program_that_speaks_stdio_can_be_the_server() {
    let exe = env!("CARGO_BIN_EXE_sbrs-mcp");
    let run = sbrs_mcp(
        &[
            "call",
            "find_api",
            "--arg",
            "query=click",
            "--connect",
            exe,
            "--connect-arg=--server",
            "--connect-arg=dev",
        ],
        &[],
        None,
    )
    .await;

    assert_ok(&run);
    assert!(run.stdout.contains("click"), "{}", run.stdout);
}

#[tokio::test]
async fn a_program_that_is_not_there_exits_3() {
    let run = sbrs_mcp(
        &["tools", "--connect", "/no/such/program-for-the-test"],
        &[],
        None,
    )
    .await;

    assert_eq!(run.code, Some(3), "{}", run.stderr);
    assert!(run.stderr.starts_with("sbrs-mcp: "), "{}", run.stderr);
}

// ----------------------------------------------------------------------
// Resources and prompts: the cdp server has both and starts no browser
// ----------------------------------------------------------------------

#[tokio::test]
async fn resources_are_listed_and_read() {
    let listed = sbrs_mcp(&["resources", "--server", "cdp"], &[], None).await;
    let journal = sbrs_mcp(&["read", "sbrs://journal", "--server", "cdp"], &[], None).await;

    assert_ok(&listed);
    assert!(
        listed.stdout.contains("sbrs://journal"),
        "{}",
        listed.stdout
    );
    assert!(listed.stdout.contains("(template)"), "{}", listed.stdout);
    assert_ok(&journal);
    let body: Value = serde_json::from_str(&journal.stdout).expect("the journal is JSON");
    assert!(body["calls"].is_array(), "{body}");
}

#[tokio::test]
async fn a_resource_that_is_refused_exits_2() {
    let run = sbrs_mcp(
        &["read", "sbrs://output/../Cargo.toml", "--server", "cdp"],
        &[],
        None,
    )
    .await;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
}

#[tokio::test]
async fn prompts_are_listed_and_rendered() {
    let listed = sbrs_mcp(&["prompts", "--server", "cdp"], &[], None).await;
    let rendered = sbrs_mcp(
        &[
            "prompt",
            "write_test",
            "--arg",
            "url=https://shop.test",
            "--arg",
            "goal=buy a mug",
            "--server",
            "cdp",
        ],
        &[],
        None,
    )
    .await;
    let missing = sbrs_mcp(&["prompt", "write_test", "--server", "cdp"], &[], None).await;

    assert_ok(&listed);
    assert!(listed.stdout.contains("(url*, goal*)"), "{}", listed.stdout);
    assert_ok(&rendered);
    assert!(
        rendered.stdout.starts_with("[user]\n"),
        "{}",
        rendered.stdout
    );
    assert!(rendered.stdout.contains("buy a mug"), "{}", rendered.stdout);
    assert_eq!(missing.code, Some(2), "{}", missing.stderr);
}

#[tokio::test]
async fn a_server_without_prompts_is_a_mistake_of_the_command() {
    let run = sbrs_mcp(&["prompts", "--server", "dev"], &[], None).await;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.stderr.contains("offers no prompts"), "{}", run.stderr);
}

// ----------------------------------------------------------------------
// batch: several calls in one session
// ----------------------------------------------------------------------

const BATCH: &str = r#"# two calls
{"tool": "find_api", "arguments": {"query": "click"}}

{"tool": "find_api", "arguments": {"query": "hover"}}
"#;

#[tokio::test]
async fn a_batch_runs_its_calls_in_order() {
    let run = sbrs_mcp(&["batch", "--raw", "--server", "dev"], &[], Some(BATCH)).await;

    assert_ok(&run);
    let lines: Vec<Value> = run
        .stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line per call"))
        .collect();
    assert_eq!(lines.len(), 2, "{}", run.stdout);
    assert!(lines
        .iter()
        .all(|line| line["tool"] == "find_api" && line["isError"] == false));
    assert!(lines[0]["text"].as_str().unwrap().contains("click"));
    assert!(lines[1]["text"].as_str().unwrap().contains("hover"));
    assert!(run.stderr.contains("[2/2] find_api"), "{}", run.stderr);
}

#[tokio::test]
async fn a_bad_line_stops_a_batch_before_anything_runs() {
    let text = "{\"tool\": \"find_api\", \"arguments\": {\"query\": \"click\"}}\nnot json\n";

    let run = sbrs_mcp(&["batch", "--server", "dev"], &[], Some(text)).await;

    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(run.stderr.contains("line 2"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "nothing ran: {}", run.stdout);
}

#[tokio::test]
async fn a_batch_stops_at_the_first_failure_unless_told_to_go_on() {
    // The first call fails (no `query`); the second would work.
    let text = "{\"tool\": \"find_api\"}\n{\"tool\": \"find_api\", \"arguments\": {\"query\": \"click\"}}\n";

    let stopped = sbrs_mcp(&["batch", "--raw", "--server", "dev"], &[], Some(text)).await;
    let went_on = sbrs_mcp(
        &["batch", "--raw", "--keep-going", "--server", "dev"],
        &[],
        Some(text),
    )
    .await;

    assert_eq!(stopped.code, Some(1), "{}", stopped.stderr);
    assert_eq!(stopped.stdout.lines().count(), 1, "{}", stopped.stdout);
    assert_eq!(went_on.code, Some(1), "the status of the first failure");
    assert_eq!(went_on.stdout.lines().count(), 2, "{}", went_on.stdout);
}

#[tokio::test]
async fn a_batch_reads_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("calls.jsonl");
    std::fs::write(&file, BATCH).unwrap();

    let run = sbrs_mcp(
        &["batch", file.to_str().unwrap(), "--raw", "--server", "dev"],
        &[],
        None,
    )
    .await;

    assert_ok(&run);
    assert_eq!(run.stdout.lines().count(), 2, "{}", run.stdout);
}

// ----------------------------------------------------------------------
// Over HTTP
// ----------------------------------------------------------------------

#[tokio::test]
async fn the_client_commands_work_over_http() {
    let server = Server::start(&["--server", "dev", "--listen", "127.0.0.1:0"], &[]).await;

    let listed = sbrs_mcp(&["tools", "--url", &server.url], &[], None).await;
    let called = sbrs_mcp(
        &[
            "call",
            "find_api",
            "--arg",
            "query=click",
            "--url",
            &server.url,
        ],
        &[],
        None,
    )
    .await;
    let batched = sbrs_mcp(&["batch", "--raw", "--url", &server.url], &[], Some(BATCH)).await;
    // The server is chosen by the environment when no flag does.
    let by_environment = sbrs_mcp(&["tools"], &[("SB_MCP_URL", &server.url)], None).await;

    assert_ok(&listed);
    assert!(listed.stdout.contains("find_api"), "{}", listed.stdout);
    assert_ok(&called);
    assert!(called.stdout.contains("click"), "{}", called.stdout);
    assert_ok(&batched);
    assert_eq!(batched.stdout.lines().count(), 2, "{}", batched.stdout);
    assert_ok(&by_environment);
    assert!(by_environment.stdout.contains("find_api"));
}

#[tokio::test]
async fn every_command_ends_its_session_so_a_small_server_does_not_fill_up() {
    let server = Server::start(
        &[
            "--server",
            "dev",
            "--listen",
            "127.0.0.1:0",
            "--max-sessions",
            "1",
        ],
        &[],
    )
    .await;

    for round in 0..4 {
        let run = sbrs_mcp(&["tools", "--url", &server.url], &[], None).await;
        assert_eq!(
            run.code,
            Some(0),
            "round {round}: {}\n{}",
            run.stderr,
            server.log()
        );
    }
}

#[tokio::test]
async fn a_server_with_a_token_needs_it_and_a_missing_one_exits_3() {
    let server = Server::start(
        &["--server", "dev", "--listen", "127.0.0.1:0"],
        &[("SB_MCP_TOKEN", TOKEN)],
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("token");
    std::fs::write(&file, format!("{TOKEN}\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let without = sbrs_mcp(&["tools", "--url", &server.url], &[], None).await;
    let wrong = sbrs_mcp(
        &["tools", "--url", &server.url],
        &[("SB_MCP_TOKEN", "not-the-token")],
        None,
    )
    .await;
    let from_environment = sbrs_mcp(
        &["tools", "--url", &server.url],
        &[("SB_MCP_TOKEN", TOKEN)],
        None,
    )
    .await;
    let from_file = sbrs_mcp(
        &[
            "tools",
            "--url",
            &server.url,
            "--token-file",
            file.to_str().unwrap(),
        ],
        &[],
        None,
    )
    .await;

    assert_eq!(without.code, Some(3), "{}", without.stderr);
    assert!(
        without.stderr.starts_with("sbrs-mcp: "),
        "{}",
        without.stderr
    );
    assert!(!without.stderr.contains(TOKEN));
    assert!(
        without.stderr.contains("asks for a bearer token"),
        "{}",
        without.stderr
    );

    assert_eq!(wrong.code, Some(3), "{}", wrong.stderr);
    assert!(
        !wrong.stderr.contains("not-the-token"),
        "the token is never printed"
    );
    assert_ok(&from_environment);
    assert_ok(&from_file);
}

#[tokio::test]
async fn a_url_nobody_listens_on_exits_3() {
    let run = sbrs_mcp(
        &["tools", "--url", "http://127.0.0.1:1/mcp", "--timeout", "5"],
        &[],
        None,
    )
    .await;

    assert_eq!(run.code, Some(3), "{}", run.stderr);
}

#[cfg(unix)]
#[tokio::test]
async fn a_slow_server_is_given_up_on_at_the_timeout() {
    // A program that never answers the handshake.
    let run = timeout(
        Duration::from_secs(30),
        sbrs_mcp(
            &[
                "tools",
                "--connect",
                "/bin/sleep",
                "--connect-arg=30",
                "--timeout",
                "1",
            ],
            &[],
            None,
        ),
    )
    .await
    .expect("the command gave up in time");

    assert_eq!(run.code, Some(3), "{}", run.stderr);
    assert!(run.stderr.contains("handshake"), "{}", run.stderr);
}
