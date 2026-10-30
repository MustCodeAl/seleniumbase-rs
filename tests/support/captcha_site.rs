//! A loopback site for the CAPTCHA fixtures in `tests/fixtures/captcha/`.
//!
//! It listens on two loopback ports, which are two origins: pages are served
//! from the first (`home`) and name the second (`far`) for their widget frames,
//! so a frame is cross-origin and its inside cannot be read by the page that
//! holds it, as with a real Turnstile or reCAPTCHA frame. Both ports answer
//! every route. No real challenge site is contacted.
//!
//! Include it from a test with
//! `#[path = "support/captcha_site.rs"] mod captcha_site;`.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

const IFRAME: &str = include_str!("../fixtures/captcha/iframe.html");
const SHADOW: &str = include_str!("../fixtures/captcha/shadow.html");
const LATE: &str = include_str!("../fixtures/captcha/late.html");
const MOVING: &str = include_str!("../fixtures/captcha/moving.html");
const DECOY: &str = include_str!("../fixtures/captcha/decoy.html");
const HUMAN: &str = include_str!("../fixtures/captcha/human.html");
const INTERSTITIAL: &str = include_str!("../fixtures/captcha/interstitial.html");
const CLEARED: &str = include_str!("../fixtures/captcha/cleared.html");
const WIDGET: &str = include_str!("../fixtures/captcha/widget.html");

/// The two origins of the fixture site.
#[derive(Debug, Clone)]
pub struct Site {
    /// The origin the pages are opened from, such as `http://127.0.0.1:41000`.
    pub home: String,
    /// The second origin, which holds the widget frames.
    pub far: String,
}

impl Site {
    /// Starts both servers; they run until the test process ends.
    pub fn start() -> Self {
        let home = TcpListener::bind("127.0.0.1:0").expect("bind the home port");
        let far = TcpListener::bind("127.0.0.1:0").expect("bind the far port");
        let site = Self {
            home: format!("http://{}", home.local_addr().expect("home address")),
            far: format!("http://{}", far.local_addr().expect("far address")),
        };
        serve(home, site.far.clone());
        serve(far, site.far.clone());
        site
    }

    /// The address of `path` on the home origin, such as `/iframe`.
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.home)
    }
}

struct Answer {
    content_type: &'static str,
    cookie: Option<&'static str>,
    status: &'static str,
    body: String,
}

impl Answer {
    fn html(body: &str, far: &str) -> Self {
        Self {
            content_type: "text/html; charset=utf-8",
            cookie: None,
            status: "200 OK",
            body: body.replace("FAR", far),
        }
    }
}

fn route(path: &str, far: &str) -> Answer {
    match path {
        "/iframe" => Answer::html(IFRAME, far),
        "/shadow" => Answer::html(SHADOW, far),
        "/late" => Answer::html(LATE, far),
        "/moving" => Answer::html(MOVING, far),
        "/decoy" => Answer::html(DECOY, far),
        "/human" => Answer::html(HUMAN, far),
        "/interstitial" => Answer::html(INTERSTITIAL, far),
        "/w" => Answer::html(WIDGET, far),
        // What the real challenge answers with once the widget is solved.
        "/cleared" => Answer {
            cookie: Some("cf_clearance=fixture-clearance-token; Path=/; Max-Age=1800; HttpOnly"),
            ..Answer::html(CLEARED, far)
        },
        // The script a real challenge page carries; its address is a marker.
        "/cdn-cgi/challenge-platform/h/b/orchestrate/chl_page/v1" => Answer {
            content_type: "text/javascript",
            cookie: None,
            status: "200 OK",
            body: "// fixture challenge script\n".to_owned(),
        },
        _ => Answer {
            content_type: "text/plain",
            cookie: None,
            status: "404 Not Found",
            body: String::new(),
        },
    }
}

/// Reads the request head, so the path is not cut by a short read.
fn request_path(stream: &mut TcpStream) -> String {
    let mut head = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") && head.len() < 16 * 1024 {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => head.extend_from_slice(&buffer[..read]),
        }
    }
    let text = String::from_utf8_lossy(&head);
    let target = text.split_whitespace().nth(1).unwrap_or("/");
    target.split('?').next().unwrap_or("/").to_owned()
}

fn answer(mut stream: TcpStream, far: &str) {
    let path = request_path(&mut stream);
    let Answer {
        content_type,
        cookie,
        status,
        body,
    } = route(&path, far);
    let cookie = cookie.map_or_else(String::new, |value| format!("Set-Cookie: {value}\r\n"));
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n{cookie}Content-Length: {}\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// A thread for each connection: Chrome opens sockets it never writes to.
fn serve(listener: TcpListener, far: String) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let far = far.clone();
            std::thread::spawn(move || answer(stream, &far));
        }
    });
}
