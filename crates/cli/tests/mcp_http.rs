//! The `sbrs-mcp` binary served over streamable HTTP: the address line, the
//! token, the `Origin` check, the exit statuses of what the operator typed
//! wrong, and the end on SIGTERM. No browser is started: the `dev` server has
//! none. Run with `cargo test -p seleniumbase-rs-cli --test mcp_http`.

#![cfg(feature = "mcp-server")]

#[path = "support/mcp_http.rs"]
mod support;

use reqwest::StatusCode;
use serde_json::json;
use support::{run_to_exit, Mcp, Server};

const TOKEN: &str = "an-environment-token-for-the-tests";

#[tokio::test]
async fn the_dev_server_prints_where_it_listens_and_serves_its_tools() {
    let mut server = Server::start(&["--server", "dev", "--listen", "127.0.0.1:0"], &[]).await;
    assert_eq!(
        server.ready,
        format!(
            "sbrs-mcp: listening on http://127.0.0.1:{}/mcp (server dev)",
            server.port
        ),
        "exactly this line, so scripts can parse it"
    );
    assert_ne!(server.port, 0);

    let mut client = Mcp::new(&server.url);
    let initialized = client.initialize().await;
    assert_eq!(initialized["serverInfo"]["name"], "seleniumbase-dev");
    let listed = client.rpc("tools/list", json!({})).await;
    let tools = listed["result"]["tools"].as_array().expect("tools");
    assert!(
        tools.iter().any(|tool| tool["name"] == "find_api"),
        "{tools:?}"
    );
    let found = client.text("find_api", json!({ "query": "click" })).await;
    assert!(found.contains("click"), "{found}");

    #[cfg(unix)]
    {
        server.signal("TERM");
        assert_eq!(server.exit_code().await, Some(0), "{}", server.log());
    }
    #[cfg(not(unix))]
    drop(server.child.kill().await);
}

#[tokio::test]
async fn the_default_path_can_be_changed() {
    let server = Server::start(
        &[
            "--server",
            "dev",
            "--listen",
            "127.0.0.1:0",
            "--path",
            "/tools",
        ],
        &[],
    )
    .await;
    assert!(server.url.ends_with("/tools"), "{}", server.url);
    let mut client = Mcp::new(&server.url);
    client.initialize().await;
}

#[tokio::test]
async fn a_token_from_the_environment_is_required_and_checked() {
    let server = Server::start(
        &["--server", "dev", "--listen", "127.0.0.1:0"],
        &[("SB_MCP_TOKEN", TOKEN)],
    )
    .await;
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });

    let without = Mcp::new(&server.url).post(&ping).await;
    assert_eq!(without.status(), StatusCode::UNAUTHORIZED);
    let wrong = Mcp::new(&server.url).with_token("not-it").post(&ping).await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let mut client = Mcp::new(&server.url).with_token(TOKEN);
    client.initialize().await;
    assert!(
        !server.log().contains(TOKEN),
        "the token is never logged: {}",
        server.log()
    );
}

#[tokio::test]
async fn a_foreign_origin_is_403() {
    let server = Server::start(&["--server", "dev", "--listen", "127.0.0.1:0"], &[]).await;
    let client = Mcp::new(&server.url);
    let response = client
        .builder(reqwest::Method::POST)
        .header("origin", "https://evil.test")
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_token_file_wins_over_the_environment_and_its_newline_is_not_part_of_it() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("token");
    std::fs::write(&file, "the-token-in-the-file\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let server = Server::start(
        &[
            "--server",
            "dev",
            "--listen",
            "127.0.0.1:0",
            "--token-file",
            file.to_str().unwrap(),
        ],
        &[("SB_MCP_TOKEN", TOKEN)],
    )
    .await;
    assert!(
        !server.log().contains("can be read by other users"),
        "{}",
        server.log()
    );

    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    let from_environment = Mcp::new(&server.url).with_token(TOKEN).post(&ping).await;
    assert_eq!(from_environment.status(), StatusCode::UNAUTHORIZED);
    let mut client = Mcp::new(&server.url).with_token("the-token-in-the-file");
    client.initialize().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_token_file_other_users_can_read_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("token");
    std::fs::write(&file, "shared-token").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

    let server = Server::start(
        &[
            "--server",
            "dev",
            "--listen",
            "127.0.0.1:0",
            "--token-file",
            file.to_str().unwrap(),
        ],
        &[],
    )
    .await;
    assert!(
        server
            .before_ready
            .iter()
            .any(|line| line.contains("can be read by other users")),
        "{:?}",
        server.before_ready
    );
}

#[tokio::test]
async fn an_address_others_can_reach_needs_a_token_and_exits_2_without_one() {
    let (code, stderr) = run_to_exit(&["--server", "dev", "--listen", "0.0.0.0:8931"], &[]).await;
    assert_eq!(code, Some(2), "{stderr}");
    assert!(
        stderr.contains("refusing to listen on 0.0.0.0:8931 without a token"),
        "{stderr}"
    );
}

#[tokio::test]
async fn a_loopback_server_without_a_token_makes_one_and_requires_it() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start_as_given(
        &["--server", "dev", "--listen", "127.0.0.1:0"],
        &[("SB_MCP_OUTPUT_DIR", dir.path().to_str().unwrap())],
    )
    .await;
    let line = server
        .before_ready
        .iter()
        .find(|line| line.starts_with("sbrs-mcp: HTTP bearer token: "))
        .unwrap_or_else(|| {
            panic!(
                "the token is printed before the address: {:?}",
                server.before_ready
            )
        });
    let token = line["sbrs-mcp: HTTP bearer token: ".len()..]
        .split_whitespace()
        .next()
        .expect("a token on the line");
    assert!(
        token.len() >= 32 && token.chars().all(|c| c.is_ascii_hexdigit()),
        "a random hex token, not {token:?}"
    );
    let saved = std::fs::read_to_string(dir.path().join("mcp-token")).expect("the token file");
    assert_eq!(saved.trim(), token);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join("mcp-token"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "readable by the operator only");
    }

    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    let without = Mcp::new(&server.url).post(&ping).await;
    assert_eq!(without.status(), StatusCode::UNAUTHORIZED);
    let mut client = Mcp::new(&server.url).with_token(token);
    client.initialize().await;
}

#[tokio::test]
async fn no_token_is_for_loopback_listeners_only() {
    let (code, stderr) = run_to_exit(
        &["--server", "dev", "--listen", "0.0.0.0:0", "--no-token"],
        &[],
    )
    .await;
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.contains("without a token"), "{stderr}");
}

#[tokio::test]
async fn the_no_token_variable_serves_a_loopback_listener_without_one() {
    let server = Server::start_as_given(
        &["--server", "dev", "--listen", "127.0.0.1:0"],
        &[("SB_MCP_NO_TOKEN", "true")],
    )
    .await;
    assert!(
        !server
            .before_ready
            .iter()
            .any(|line| line.contains("HTTP bearer token")),
        "{:?}",
        server.before_ready
    );
    let mut client = Mcp::new(&server.url);
    client.initialize().await;
}

#[tokio::test]
async fn an_address_others_can_reach_starts_with_a_token_and_warns() {
    let server = Server::start(
        &["--server", "dev", "--listen", "0.0.0.0:0"],
        &[("SB_MCP_TOKEN", TOKEN)],
    )
    .await;
    assert!(
        server
            .before_ready
            .iter()
            .any(|line| line.contains("reachable from other machines")),
        "a warning comes before the address line: {:?}",
        server.before_ready
    );
    // The same machine is still loopback for the `Host` check.
    let mut client = Mcp::new(&format!("http://127.0.0.1:{}/mcp", server.port)).with_token(TOKEN);
    client.initialize().await;
}

#[tokio::test]
async fn what_the_operator_types_wrong_exits_2() {
    let cases: [(&[&str], &str); 7] = [
        (
            &[
                "--server",
                "dev",
                "--transport",
                "stdio",
                "--listen",
                "127.0.0.1:0",
            ],
            "--transport http",
        ),
        (&["--server", "dev", "--path", "/x"], "--transport http"),
        (&["--server", "dev", "--listen", "nonsense"], "--listen"),
        (
            &[
                "--server",
                "dev",
                "--listen",
                "127.0.0.1:0",
                "--max-sessions",
                "0",
            ],
            "max_sessions",
        ),
        (
            &[
                "--server",
                "dev",
                "--listen",
                "127.0.0.1:0",
                "--path",
                "mcp",
            ],
            "path",
        ),
        (
            &[
                "--server",
                "dev",
                "--listen",
                "127.0.0.1:0",
                "--token-file",
                "/no/such/token/file",
            ],
            "--token-file",
        ),
        (
            &[
                "--server",
                "dev",
                "--listen",
                "127.0.0.1:0",
                "--allow-origin",
                "evil.test",
            ],
            "origin",
        ),
    ];
    for (args, expected) in cases {
        let (code, stderr) = run_to_exit(args, &[]).await;
        assert_eq!(code, Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains(expected), "{args:?}: {stderr}");
    }
}

#[tokio::test]
async fn a_port_that_is_taken_exits_with_the_mcp_status() {
    let first = Server::start(&["--server", "dev", "--listen", "127.0.0.1:0"], &[]).await;
    let taken = format!("127.0.0.1:{}", first.port);
    let (code, stderr) = run_to_exit(&["--server", "dev", "--listen", &taken], &[]).await;
    assert_eq!(code, Some(7), "{stderr}");
    assert!(stderr.contains("cannot listen on"), "{stderr}");
}

#[cfg(unix)]
#[tokio::test]
async fn sigint_ends_the_server_cleanly_too() {
    let mut server = Server::start(&["--server", "dev", "--listen", "127.0.0.1:0"], &[]).await;
    let mut client = Mcp::new(&server.url);
    client.initialize().await;
    server.signal("INT");
    assert_eq!(server.exit_code().await, Some(0), "{}", server.log());
}
