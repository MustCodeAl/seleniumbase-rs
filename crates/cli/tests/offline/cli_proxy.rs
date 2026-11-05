//! `sbrs proxy`, run as the real binary against servers on this machine. No network and no
//! browser are needed.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const SBRS: &str = env!("CARGO_BIN_EXE_sbrs");
const STEP: Duration = Duration::from_secs(20);

/// A running `sbrs proxy`, stopped when dropped.
struct Proxy {
    child: Child,
    address: SocketAddr,
    stdout: BufReader<ChildStdout>,
    stderr: BufReader<ChildStderr>,
}

impl Proxy {
    fn start(args: &[&str]) -> Self {
        let mut child = Command::new(SBRS)
            .arg("proxy")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("sbrs starts");
        let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let stderr = BufReader::new(child.stderr.take().expect("piped stderr"));
        let mut line = String::new();
        stdout
            .read_line(&mut line)
            .expect("the proxy says where it listens");
        let url = line
            .trim()
            .strip_prefix("Proxy listening on http://")
            .unwrap_or_else(|| panic!("unexpected first line: {line:?}"));
        let address = url
            .parse()
            .unwrap_or_else(|_| panic!("not an address: {url:?}"));
        Self {
            child,
            address,
            stdout,
            stderr,
        }
    }

    fn connect(&self) -> TcpStream {
        let stream = TcpStream::connect(self.address).expect("connects to the proxy");
        stream.set_read_timeout(Some(STEP)).expect("sets a timeout");
        stream
    }

    /// Waits for the process to end and returns its exit code.
    fn wait(&mut self) -> Option<i32> {
        let end = Instant::now() + STEP;
        loop {
            if let Some(status) = self.child.try_wait().expect("polls the child") {
                return status.code();
            }
            assert!(Instant::now() < end, "the proxy did not stop in time");
            thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A server on this machine that answers every request with `ok` and tells the test the
/// head it was sent.
fn origin() -> (SocketAddr, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("binds");
    let address = listener.local_addr().expect("address");
    let (sent, received) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accepts");
        let mut seen = Vec::new();
        let mut chunk = [0_u8; 1024];
        while !seen.windows(4).any(|window| window == b"\r\n\r\n") {
            let count = stream.read(&mut chunk).expect("reads");
            assert!(count > 0, "the proxy closed before the head ended");
            seen.extend_from_slice(&chunk[..count]);
        }
        sent.send(String::from_utf8_lossy(&seen).into_owned())
            .expect("the test listens");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            .expect("writes");
    });
    (address, received)
}

/// A server on this machine that sends back what it is sent.
fn echo() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("binds");
    let address = listener.local_addr().expect("address");
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || {
                let mut stream = stream;
                let mut chunk = [0_u8; 1024];
                while let Ok(count) = stream.read(&mut chunk) {
                    if count == 0 || stream.write_all(&chunk[..count]).is_err() {
                        break;
                    }
                }
            });
        }
    });
    address
}

fn read_all(mut stream: TcpStream) -> String {
    let mut answer = Vec::new();
    // The proxy closes the connection after one request.
    let _ = stream.read_to_end(&mut answer);
    String::from_utf8_lossy(&answer).into_owned()
}

#[test]
fn the_help_shows_an_example_and_the_options() {
    let output = Command::new(SBRS)
        .args(["proxy", "--help"])
        .output()
        .expect("sbrs runs");
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for word in [
        "--hostname",
        "--port",
        "Examples:",
        "sbrs proxy --port 8899",
    ] {
        assert!(help.contains(word), "the help lacks {word}:\n{help}");
    }
}

#[test]
fn it_listens_on_loopback_and_says_how_to_use_it() {
    let mut proxy = Proxy::start(&["--port", "0"]);
    assert!(proxy.address.ip().is_loopback(), "{}", proxy.address);
    let mut line = String::new();
    proxy.stdout.read_line(&mut line).expect("reads");
    assert_eq!(
        line.trim(),
        format!(
            "Use it with: sbrs --proxy {} open https://example.com",
            proxy.address
        )
    );
}

#[test]
fn connect_opens_a_tunnel_to_a_local_server() {
    let proxy = Proxy::start(&["--port", "0"]);
    let echo = echo();
    let mut stream = proxy.connect();
    write!(stream, "CONNECT {echo} HTTP/1.1\r\nHost: {echo}\r\n\r\n").expect("writes");
    let expected = b"HTTP/1.1 200 Connection Established\r\n\r\n";
    let mut answer = vec![0_u8; expected.len()];
    stream.read_exact(&mut answer).expect("the proxy answers");
    assert_eq!(answer, expected);
    stream
        .write_all(b"ping through the tunnel")
        .expect("writes");
    let mut echoed = [0_u8; 23];
    stream.read_exact(&mut echoed).expect("the echo comes back");
    assert_eq!(&echoed, b"ping through the tunnel");
}

#[test]
fn an_absolute_request_reaches_the_server_in_origin_form() {
    let proxy = Proxy::start(&["--port", "0"]);
    let (origin, seen) = origin();
    let mut stream = proxy.connect();
    write!(
        stream,
        "GET http://{origin}/some/page?x=1 HTTP/1.1\r\nHost: {origin}\r\nProxy-Connection: keep-alive\r\n\r\n"
    )
    .expect("writes");
    let answer = read_all(stream);
    assert!(answer.starts_with("HTTP/1.1 200 OK\r\n"), "{answer}");
    assert!(answer.ends_with("ok"), "{answer}");
    let head = seen.recv_timeout(STEP).expect("the server saw the head");
    assert!(
        head.starts_with("GET /some/page?x=1 HTTP/1.1\r\n"),
        "{head}"
    );
    assert!(head.contains(&format!("Host: {origin}\r\n")), "{head}");
    assert!(head.contains("Connection: close\r\n"), "{head}");
    assert!(
        !head.to_ascii_lowercase().contains("proxy-connection"),
        "{head}"
    );
}

#[test]
fn a_request_that_is_not_for_a_proxy_is_refused() {
    let proxy = Proxy::start(&["--port", "0"]);
    let mut stream = proxy.connect();
    stream
        .write_all(b"GET /index.html HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .expect("writes");
    let answer = read_all(stream);
    assert!(
        answer.starts_with("HTTP/1.1 400 Bad Request\r\n"),
        "{answer}"
    );
}

#[test]
fn a_host_that_cannot_be_reached_is_a_bad_gateway() {
    let proxy = Proxy::start(&["--port", "0"]);
    let closed = TcpListener::bind("127.0.0.1:0").expect("binds");
    let address = closed.local_addr().expect("address");
    drop(closed);
    let mut stream = proxy.connect();
    write!(stream, "GET http://{address}/ HTTP/1.1\r\n\r\n").expect("writes");
    let answer = read_all(stream);
    assert!(
        answer.starts_with("HTTP/1.1 502 Bad Gateway\r\n"),
        "{answer}"
    );
}

#[test]
fn an_address_other_than_loopback_gets_a_warning() {
    let mut proxy = Proxy::start(&["--hostname", "0.0.0.0", "--port", "0"]);
    let mut warning = String::new();
    proxy.stderr.read_line(&mut warning).expect("reads");
    assert!(
        warning.starts_with("warning: ")
            && warning.contains("accepts connections from other machines; this is an open proxy"),
        "{warning:?}"
    );
}

#[test]
fn a_port_that_is_taken_is_reported() {
    let taken = TcpListener::bind("127.0.0.1:0").expect("binds");
    let port = taken.local_addr().expect("address").port().to_string();
    let output = Command::new(SBRS)
        .args(["proxy", "--port", &port])
        .stdin(Stdio::null())
        .output()
        .expect("sbrs runs");
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("cannot listen on 127.0.0.1"), "{error}");
}

#[test]
fn a_port_that_is_not_a_port_is_a_usage_error() {
    let output = Command::new(SBRS)
        .args(["proxy", "--port", "70000"])
        .output()
        .expect("sbrs runs");
    assert_eq!(output.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn ctrl_c_and_sigterm_stop_it_with_status_zero() {
    for signal in ["INT", "TERM"] {
        let mut proxy = Proxy::start(&["--port", "0"]);
        let status = Command::new("kill")
            .arg(format!("-{signal}"))
            .arg(proxy.child.id().to_string())
            .status()
            .expect("kill runs");
        assert!(status.success());
        assert_eq!(proxy.wait(), Some(0), "SIG{signal}");
    }
}
