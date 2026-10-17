//! `sbrs devtools` against a real Chrome: the endpoint it prints answers
//! `/json/version`, and Ctrl-C (SIGINT) or SIGTERM closes the browser and ends
//! the command with status 0.
//!
//! These tests need Chrome, so they are ignored by default. Run them one test
//! binary at a time:
//!
//! ```text
//! cargo test --test devtools_cli_chrome -- --ignored --test-threads=1
//! ```

#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

/// How long Chrome may take to start and the command to print its endpoint.
const START: Duration = Duration::from_secs(60);

/// How long the command may take to close Chrome and exit after a signal.
const STOP: Duration = Duration::from_secs(30);

/// A running `sbrs devtools`. A test that fails before it stops the command
/// still ends it, and with it the browser, instead of leaving both running.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = Command::new("kill")
                .args(["-INT", &self.0.id().to_string()])
                .status();
            let deadline = Instant::now() + STOP;
            while matches!(self.0.try_wait(), Ok(None)) && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(100));
            }
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// Starts `sbrs devtools` with `args` and hands back its standard output, a
/// line at a time.
fn start(args: &[&str]) -> (Running, Receiver<String>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_sbrs"))
        .arg("devtools")
        .args(args)
        .env("RUST_LOG", "off")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("sbrs starts");
    let stdout = child.stdout.take().expect("piped output");
    let (sender, lines) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    (Running(child), lines)
}

/// The next line the command printed, waiting at most [`START`].
fn next_line(lines: &Receiver<String>) -> String {
    lines
        .recv_timeout(START)
        .expect("sbrs devtools printed a line in time")
}

/// `GET path` on `127.0.0.1:port`, returning the response body. Chrome keeps
/// the connection open, so the body is read by its `Content-Length`.
fn http_get(port: u16, path: &str) -> std::io::Result<String> {
    let stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    (&stream).write_all(
        format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
            .as_bytes(),
    )?;
    let mut reader = BufReader::new(&stream);
    let mut length = 0_usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body)?;
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// The port in an address such as `http://127.0.0.1:9222`.
fn port_of(http_url: &str) -> u16 {
    http_url
        .rsplit(':')
        .next()
        .and_then(|port| port.parse().ok())
        .expect("a port in the address")
}

/// Sends `signal` (such as `-INT`) to the command and waits for it to end.
fn stop(running: &mut Running, signal: &str) -> ExitStatus {
    let child = &mut running.0;
    let killed = Command::new("kill")
        .args([signal, &child.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(killed.success(), "kill {signal} failed");
    let deadline = Instant::now() + STOP;
    loop {
        if let Some(status) = child.try_wait().expect("the status is readable") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "sbrs devtools did not end within {STOP:?} of {signal}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// Waits until nothing answers on `port` any more: Chrome has gone.
fn assert_endpoint_closes(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while http_get(port, "/json/version").is_ok() {
        assert!(
            Instant::now() < deadline,
            "the DevTools endpoint on {port} still answers: the browser was left running"
        );
        thread::sleep(Duration::from_millis(200));
    }
}

#[test]
#[ignore = "needs Chrome; run with --ignored --test-threads=1"]
fn the_json_endpoint_answers_version_and_ctrl_c_closes_chrome_with_status_0() {
    let (mut child, lines) = start(&["--headless", "--port", "0", "--json"]);
    let printed: Value = serde_json::from_str(&next_line(&lines)).expect("one line of JSON");
    println!("sbrs devtools --json printed: {printed}");

    let http_url = printed["http_url"].as_str().expect("an HTTP address");
    let websocket_url = printed["websocket_url"]
        .as_str()
        .expect("a WebSocket address");
    assert!(http_url.starts_with("http://127.0.0.1:"), "{http_url}");
    let port = port_of(http_url);
    assert_ne!(port, 0, "Chrome picked a real port");

    let version: Value =
        serde_json::from_str(&http_get(port, "/json/version").expect("/json/version answers"))
            .expect("/json/version is JSON");
    println!("/json/version: {version}");
    assert!(
        version["Browser"]
            .as_str()
            .is_some_and(|browser| browser.contains("Chrome")),
        "{version}"
    );
    assert_eq!(version["webSocketDebuggerUrl"], websocket_url);
    let chrome_args =
        &printed["mcp_configs"]["chrome_devtools_mcp"]["mcpServers"]["chrome-devtools"]["args"];
    assert!(
        chrome_args
            .to_string()
            .contains(&format!("--browserUrl={http_url}")),
        "{chrome_args}"
    );

    let status = stop(&mut child, "-INT");
    assert!(
        status.success(),
        "Ctrl-C ends sbrs devtools with 0, got {status}"
    );
    assert_endpoint_closes(port);
}

#[test]
#[ignore = "needs Chrome; run with --ignored --test-threads=1"]
fn the_text_output_names_the_fixed_port_and_sigterm_closes_chrome() {
    // A port that is free now, so the fixed-port path is the one tested.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port();
    let port_text = port.to_string();
    let (mut child, lines) = start(&["--headless", "--port", &port_text]);

    let mut text = Vec::new();
    loop {
        let line = next_line(&lines);
        let done = line.starts_with("Press Ctrl+C");
        text.push(line);
        if done {
            break;
        }
    }
    let text = text.join("\n");
    println!("{text}");
    assert!(
        text.contains(&format!("DevTools HTTP endpoint:  http://127.0.0.1:{port}")),
        "{text}"
    );
    assert!(text.contains(&format!("--browserUrl=http://127.0.0.1:{port}")));
    assert!(text.contains(&format!("--cdp-endpoint=http://127.0.0.1:{port}")));
    assert!(http_get(port, "/json/version")
        .expect("/json/version answers on the fixed port")
        .contains("webSocketDebuggerUrl"));

    let status = stop(&mut child, "-TERM");
    assert!(
        status.success(),
        "SIGTERM ends sbrs devtools with 0, got {status}"
    );
    assert_endpoint_closes(port);
}

/// Needs no browser: the port is checked before Chrome is looked for.
#[test]
fn a_taken_port_is_refused_before_chrome_starts() {
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = taken.local_addr().expect("address").port().to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_sbrs"))
        .args(["devtools", "--headless", "--port", &port])
        .stdin(Stdio::null())
        .output()
        .expect("sbrs runs");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--port 0"), "{stderr}");
}
