//! The `seleniumbase-mcp` binary, sent real signals. No browser is started:
//! the server only launches Chrome when a tool asks for it.
//! Run with `cargo test --features mcp-server`.

#![cfg(all(unix, feature = "mcp-server"))]

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

const STEP: Duration = Duration::from_secs(60);

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Server {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_seleniumbase-mcp"))
            .args(["--server", "cdp"])
            .env("SB_SHUTDOWN_TIMEOUT_SECS", "5")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("start seleniumbase-mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }

    async fn send(&mut self, line: &str) {
        self.stdin.write_all(line.as_bytes()).await.unwrap();
        self.stdin.write_all(b"\n").await.unwrap();
        self.stdin.flush().await.unwrap();
    }

    async fn read_line(&mut self) -> String {
        let mut line = String::new();
        timeout(STEP, self.stdout.read_line(&mut line))
            .await
            .expect("the server answered in time")
            .unwrap();
        line
    }

    /// Sends the signal the way an orchestrator would.
    fn signal(&self, name: &str) {
        let pid = self.child.id().expect("the server is running");
        let status = std::process::Command::new("kill")
            .arg(format!("-{name}"))
            .arg(pid.to_string())
            .status()
            .expect("run kill");
        assert!(status.success(), "kill -{name} {pid} failed");
    }

    /// Waits for the server to exit and returns its exit code, which is `None`
    /// if a signal killed it.
    async fn exit_code(&mut self) -> Option<i32> {
        timeout(Duration::from_secs(30), self.child.wait())
            .await
            .expect("the server stopped within 30 seconds")
            .unwrap()
            .code()
    }
}

/// A server that answers `ping` is running, with its signal handlers in place.
async fn started() -> Server {
    let mut server = Server::start();
    server
        .send(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#)
        .await;
    let reply = server.read_line().await;
    assert!(reply.contains("\"result\""), "unexpected reply: {reply}");
    server
}

async fn handshake(server: &mut Server) {
    server
        .send(
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}"#,
        )
        .await;
    let reply = server.read_line().await;
    assert!(
        reply.contains("\"protocolVersion\""),
        "unexpected reply: {reply}"
    );
    server
        .send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .await;
}

#[tokio::test]
async fn sigterm_before_a_client_connects_ends_the_server_cleanly() {
    let mut server = started().await;
    server.signal("TERM");
    assert_eq!(server.exit_code().await, Some(0));
}

#[tokio::test]
async fn sigterm_while_a_client_is_connected_ends_the_server_cleanly() {
    let mut server = started().await;
    handshake(&mut server).await;
    server.signal("TERM");
    assert_eq!(server.exit_code().await, Some(0));
}

#[tokio::test]
async fn sigint_ends_the_server_cleanly_too() {
    let mut server = started().await;
    handshake(&mut server).await;
    server.signal("INT");
    assert_eq!(server.exit_code().await, Some(0));
}
