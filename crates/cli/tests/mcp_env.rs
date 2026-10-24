//! The `SB_MCP_*` variables of `sbrs-mcp`: each one stands for the flag it is named after, the
//! flag wins, a value that cannot be used ends the program with status 2 naming the variable
//! and its flag, and a variable that is for HTTP never stops the stdio server. No browser is
//! started: the `dev` server has none. Run with
//! `cargo test -p seleniumbase-rs-cli --test mcp_env`.

#![cfg(feature = "mcp-server")]

#[path = "support/mcp_http.rs"]
mod support;

use std::net::SocketAddr;

use reqwest::StatusCode;
use seleniumbase_rs::config::env_vars::EXTRA_VARIABLES;
use serde_json::json;
use support::{run_to_exit, Mcp, Server};

const LISTEN: (&str, &str) = ("SB_MCP_LISTEN", "127.0.0.1:0");

#[tokio::test]
async fn the_server_and_the_address_come_from_the_environment() {
    // No flag at all: an address in the environment serves over HTTP.
    let server = Server::start(&[], &[("SB_MCP_SERVER", "dev"), LISTEN]).await;

    assert_eq!(
        server.ready,
        format!(
            "sbrs-mcp: listening on http://127.0.0.1:{}/mcp (server dev)",
            server.port
        )
    );
}

#[tokio::test]
async fn the_path_comes_from_the_environment() {
    let server = Server::start(
        &["--server", "dev"],
        &[LISTEN, ("SB_MCP_PATH", "/from-the-environment")],
    )
    .await;

    assert!(
        server.url.ends_with("/from-the-environment"),
        "{}",
        server.url
    );
}

#[tokio::test]
async fn a_flag_wins_over_its_variable() {
    let server = Server::start(
        &[
            "--server",
            "dev",
            "--listen",
            "127.0.0.1:0",
            "--path",
            "/flag",
        ],
        &[
            ("SB_MCP_SERVER", "cdp"),
            ("SB_MCP_LISTEN", "127.0.0.1:1"),
            ("SB_MCP_PATH", "/environment"),
        ],
    )
    .await;

    assert!(server.ready.ends_with("(server dev)"), "{}", server.ready);
    assert!(server.url.ends_with("/flag"), "{}", server.url);
    assert_ne!(server.port, 1);
}

#[tokio::test]
async fn the_transport_comes_from_the_environment() {
    // Stdio although an address is set: nothing listens and the server ends at the end of
    // its input, which is closed here.
    let (code, stderr) = run_to_exit(
        &["--server", "dev"],
        &[("SB_MCP_TRANSPORT", "stdio"), LISTEN],
    )
    .await;
    assert_ne!(code, Some(2), "{stderr}");
    assert!(
        stderr.contains("connection closed: initialize request"),
        "{stderr}"
    );
    assert!(!stderr.contains("listening on"), "{stderr}");

    let server = Server::start(
        &["--server", "dev"],
        &[("SB_MCP_TRANSPORT", "http"), LISTEN],
    )
    .await;
    assert!(server.ready.contains("listening on"), "{}", server.ready);
}

#[tokio::test]
async fn a_token_file_comes_from_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("token");
    std::fs::write(&file, "the-token-of-the-variable\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let server = Server::start(
        &["--server", "dev"],
        &[LISTEN, ("SB_MCP_TOKEN_FILE", file.to_str().unwrap())],
    )
    .await;
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });

    let without = Mcp::new(&server.url).post(&ping).await;
    assert_eq!(without.status(), StatusCode::UNAUTHORIZED);
    let mut client = Mcp::new(&server.url).with_token("the-token-of-the-variable");
    client.initialize().await;
}

#[tokio::test]
async fn allowed_origins_come_from_the_environment() {
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    let status = |server: &Server| {
        let client = Mcp::new(&server.url);
        let request = client
            .builder(reqwest::Method::POST)
            .header("origin", "https://app.example.test")
            .json(&ping);
        async move { request.send().await.unwrap().status() }
    };

    let without = Server::start(&["--server", "dev"], &[LISTEN]).await;
    assert_eq!(status(&without).await, StatusCode::FORBIDDEN);

    let with = Server::start(
        &["--server", "dev"],
        &[
            LISTEN,
            (
                "SB_MCP_ALLOW_ORIGIN",
                "https://other.example.test, https://app.example.test",
            ),
        ],
    )
    .await;
    assert_ne!(status(&with).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn allowed_hosts_come_from_the_environment() {
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    let host = "mcp.example.test";
    let status = |port: u16| {
        seleniumbase_rs::install_tls_provider();
        let client = reqwest::Client::builder()
            .no_proxy()
            .resolve(host, SocketAddr::from(([127, 0, 0, 1], port)))
            .build()
            .unwrap();
        let request = client
            .post(format!("http://{host}:{port}/mcp"))
            .header("accept", "application/json, text/event-stream")
            .json(&ping);
        async move { request.send().await.unwrap().status() }
    };

    let without = Server::start(&["--server", "dev"], &[LISTEN]).await;
    assert_eq!(status(without.port).await, StatusCode::FORBIDDEN);

    let with = Server::start(&["--server", "dev"], &[LISTEN, ("SB_MCP_ALLOW_HOST", host)]).await;
    assert_ne!(status(with.port).await, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_variable_reaches_the_checks_the_flag_goes_through() {
    let cases: [(&str, &str, &str); 4] = [
        ("SB_MCP_MAX_SESSIONS", "0", "max_sessions"),
        ("SB_MCP_PATH", "mcp", "path"),
        (
            "SB_MCP_TOKEN_FILE",
            "/no/such/token/file",
            "SB_MCP_TOKEN_FILE",
        ),
        ("SB_MCP_ALLOW_ORIGIN", "evil.test", "origin"),
    ];
    for (name, value, expected) in cases {
        let (code, stderr) = run_to_exit(&["--server", "dev"], &[LISTEN, (name, value)]).await;
        assert_eq!(code, Some(2), "{name}={value}: {stderr}");
        assert!(stderr.contains(expected), "{name}={value}: {stderr}");
    }
}

#[tokio::test]
async fn a_value_that_cannot_be_used_exits_2_naming_the_variable_and_its_flag() {
    let mut checked = 0;
    for variable in EXTRA_VARIABLES
        .iter()
        .filter(|variable| variable.read_by == "sbrs-mcp")
    {
        let Some(bad) = variable.refused else {
            continue;
        };
        let flag = variable.flags[0].rsplit(' ').next().unwrap();
        let (code, stderr) = run_to_exit(&["--server", "dev"], &[(variable.name, bad)]).await;

        assert_eq!(code, Some(2), "{}={bad}: {stderr}", variable.name);
        assert!(
            stderr.contains(variable.name),
            "{}: {stderr}",
            variable.name
        );
        assert!(stderr.contains(flag), "{}: {stderr}", variable.name);
        checked += 1;
    }
    assert!(checked >= 4, "only {checked} variables refuse a value");
}

#[tokio::test]
async fn a_variable_for_http_does_not_stop_the_stdio_server() {
    let (code, stderr) = run_to_exit(
        &["--server", "dev"],
        &[
            ("SB_MCP_ALLOW_HOST", "mcp.example.test"),
            ("SB_MCP_ALLOW_ORIGIN", "https://app.example.test"),
            ("SB_MCP_PATH", "/x"),
            ("SB_MCP_MAX_SESSIONS", "2"),
        ],
    )
    .await;

    assert_ne!(code, Some(2), "{stderr}");
    assert!(
        stderr.contains("connection closed: initialize request"),
        "{stderr}"
    );
}
