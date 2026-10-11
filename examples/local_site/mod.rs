//! A tiny loopback web server, shared by the browser examples.
//!
//! Each example that drives a page serves its own HTML from `127.0.0.1` instead
//! of depending on a public website. That keeps the examples runnable offline,
//! makes their assertions deterministic (the page cannot change under them),
//! and gives cookies and `localStorage` a real `http://` origin, which a
//! `data:` URL does not have.
//!
//! An example pulls this file in with `mod local_site;`. Cargo does not build
//! the folder as an example of its own, because it has no `main.rs`.

#![allow(
    dead_code,
    reason = "every example uses a different part of this module"
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use seleniumbase_rs::SeleniumBaseError;

/// The path [`Site::serve_demo`] serves [`DEMO_PAGE`] at.
pub const DEMO_PATH: &str = "/";

/// A page with one of everything the examples need: a heading, a form, a text
/// input, buttons, two dropdowns, links, a menu that opens on hover, an element
/// that appears 700 ms after the page loads, an alert, and a footer far below
/// the fold.
pub const DEMO_PAGE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Web Testing Page</title>
<style>
  body { font-family: sans-serif; margin: 2rem; }
  nav { height: 3rem; }
  .menu { position: relative; display: inline-block; }
  /* Absolutely positioned, so opening the menu never moves the page below it. */
  #submenu { display: none; position: absolute; top: 100%; left: 0; margin: 0;
             padding: 0.25rem 1rem; list-style: none; background: #eee; }
  .menu:hover #submenu { display: block; }
  .spacer { height: 1600px; }
</style>
</head>
<body>
<nav aria-label="Main">
  <div class="menu">Products
    <ul id="submenu"><li><a href="#widgets">Widgets</a></li></ul>
  </div>
</nav>
<main>
  <h1>Demo Page</h1>
  <p id="pText" style="color: green">This Text is Green</p>
  <form id="login">
    <label for="username">Username</label>
    <input id="username" name="username" autocomplete="off">
    <label for="password">Password</label>
    <input id="password" name="password" type="password" autocomplete="off">
    <button id="submit" type="submit">Sign in</button>
  </form>
  <p id="status" aria-live="polite"></p>
  <label for="myTextInput">Text</label>
  <input id="myTextInput" autocomplete="off">
  <button id="myButton" type="button">Click Me</button>
  <label for="country">Country</label>
  <select id="country">
    <option value="us">United States</option>
    <option value="de">Germany</option>
  </select>
  <label for="language">Language</label>
  <select id="language">
    <option value="en">English</option>
    <option value="de">German</option>
  </select>
  <p><a id="docs" href="#docs-section">Read the docs</a> | <a href="#more">More information</a></p>
  <button id="alertButton" type="button">Show alert</button>
  <div id="late" hidden>Loaded after a delay</div>
</main>
<div class="spacer"></div>
<footer id="footer">Footer text</footer>
<script>
  document.getElementById('login').addEventListener('submit', (event) => {
    event.preventDefault();
    document.getElementById('status').textContent =
      'Signed in as ' + document.getElementById('username').value;
  });
  document.getElementById('myButton').addEventListener('click', () => {
    const text = document.getElementById('pText');
    text.style.color = 'purple';
    text.textContent = 'This Text is Purple';
  });
  document.getElementById('alertButton').addEventListener('click', () => {
    setTimeout(() => alert('Hello from the page'), 0);
  });
  setTimeout(() => { document.getElementById('late').hidden = false; }, 700);
</script>
</body>
</html>
"##;

/// Succeeds if `condition` holds, and otherwise fails with an assertion error
/// that says what was expected. Examples use it to check their own claims.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::AssertionFailed`] when `condition` is false.
pub fn ensure(condition: bool, expectation: &str) -> seleniumbase_rs::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(SeleniumBaseError::AssertionFailed(format!(
            "expected: {expectation}"
        )))
    }
}

/// A web server on a free loopback port. It serves until the process exits.
pub struct Site {
    origin: String,
}

impl Site {
    /// Serves `pages`, a list of `(path, html)` pairs, and answers any other
    /// path with a 404.
    ///
    /// # Errors
    ///
    /// Returns an error if no loopback port can be bound.
    pub fn serve(pages: &[(&str, &str)]) -> std::io::Result<Self> {
        let pages: Arc<Vec<(String, String)>> = Arc::new(
            pages
                .iter()
                .map(|(path, html)| ((*path).to_owned(), (*html).to_owned()))
                .collect(),
        );
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let origin = format!("http://{}", listener.local_addr()?);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let pages = Arc::clone(&pages);
                // One thread per connection: Chrome opens spare sockets that
                // never send a request, and they must not hold up the others.
                thread::spawn(move || respond(stream, &pages));
            }
        });
        Ok(Self { origin })
    }

    /// Serves [`DEMO_PAGE`] at `/`.
    ///
    /// # Errors
    ///
    /// Returns an error if no loopback port can be bound.
    pub fn serve_demo() -> std::io::Result<Self> {
        Self::serve(&[(DEMO_PATH, DEMO_PAGE)])
    }

    /// The full URL of `path` on this server, such as `http://127.0.0.1:5000/`.
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }
}

/// Answers one request. A client that disconnects mid-answer is not an error
/// worth reporting from a throwaway example server, so write failures end the
/// connection quietly.
fn respond(mut stream: TcpStream, pages: &[(String, String)]) {
    // A spare socket that never sends anything is dropped after the timeout.
    if stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .is_err()
    {
        return;
    }
    let mut buffer = [0_u8; 4096];
    let Ok(read) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..read]);
    let target = request.split_whitespace().nth(1).unwrap_or("/");
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let (status, body) = match pages.iter().find(|(served, _)| served == path) {
        Some((_, html)) => ("200 OK", html.as_str()),
        None => ("404 Not Found", "not found"),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    if stream.write_all(head.as_bytes()).is_ok() {
        // The result is ignored on purpose; see the function comment.
        let _ = stream.write_all(body.as_bytes());
    }
}
