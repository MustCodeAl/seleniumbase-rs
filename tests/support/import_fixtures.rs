//! What the Playwright and Puppeteer importer tests share: the fixtures under
//! `tests/fixtures/`, the loopback shop page they are written against, and a
//! scratch crate that builds (and runs) the Rust the importers write.
//!
//! The fixtures name the page `http://127.0.0.1:4173`; the browser tests
//! replace that with the address [`serve`] returns before importing.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::thread;

use seleniumbase_rs::utilities::importer::{ScriptImportOptions, ScriptLanguage};

#[path = "scratch_crate.rs"]
mod scratch_crate;

/// The address the fixtures are written against.
pub const PLACEHOLDER: &str = "http://127.0.0.1:4173";

/// The shop page: a form with every kind of control, a list, an image and a link.
pub const PAGE: &str = r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>Shop Fixture</title></head>
<body>
<h1>Welcome to the shop</h1>
<p id="intro">Fresh fruit every day</p>
<form id="order" onsubmit="event.preventDefault(); var gift = document.getElementById('gift').checked; document.getElementById('status').textContent = 'Ordered ' + document.getElementById('qty').value + ' ' + document.getElementById('fruit').value + (gift ? ' as a gift' : ''); document.title = 'Ordered';">
  <label for="email">Email</label>
  <input id="email" name="email" type="email" placeholder="you@example.test">
  <label>Quantity <input id="qty" name="qty" type="number" value="1"></label>
  <label for="fruit">Fruit</label>
  <select id="fruit" name="fruit">
    <option value="apple">Apple</option>
    <option value="banana">Banana</option>
    <option value="cherry">Cherry</option>
  </select>
  <input id="gift" type="checkbox"> <label for="gift">Gift wrap</label>
  <button id="submit" type="submit" data-testid="place-order">Place order</button>
  <button id="disabled" type="button" disabled>Not yet</button>
</form>
<p id="status" role="status"></p>
<ul id="items"><li>Apple</li><li>Banana</li><li>Cherry</li></ul>
<img alt="Shop logo" width="20" height="20" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='20' height='20'%3E%3Crect width='20' height='20'/%3E%3C/svg%3E">
<a href="/about" title="About the shop">About us</a>
<input id="upload" type="file">
</body></html>"##;

/// The page the link leads to.
pub const ABOUT: &str = r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>About</title></head>
<body><h1>About us</h1><a href="/">Back</a></body></html>"##;

/// Serves the shop on a loopback port, a thread to each connection; any other
/// path is a 404. Returns the base URL, without a trailing slash.
pub fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let address = listener.local_addr().expect("bound address");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let (status, body) = match path {
                    "/" => ("200 OK", PAGE),
                    "/about" => ("200 OK", ABOUT),
                    _ => ("404 Not Found", "not here"),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    format!("http://{address}")
}

/// The fixtures of `tool` (`playwright` or `puppeteer`), by file name.
pub fn fixture_path(tool: &str, name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(tool)
        .join(name)
}

pub fn read_fixture(tool: &str, name: &str) -> String {
    let path = fixture_path(tool, name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The module name for a fixture: `shop.spec.ts` is `shop_spec`.
pub fn module_name(name: &str) -> String {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    stem.replace(['.', '-'], "_")
}

/// The options `sbrs import-*` would use for the fixture file `name`.
pub fn options(name: &str) -> ScriptImportOptions {
    ScriptImportOptions {
        language: ScriptLanguage::from_path(Path::new(name)),
        test_name: module_name(name),
    }
}

/// The scratch crate folder for `tool`, beside the build: one fixed place, so
/// the compile check and the browser run share what Cargo builds there.
pub fn scratch_dir(tool: &str) -> PathBuf {
    let dir = scratch_crate::target_dir()
        .join("importer-scratch")
        .join(tool);
    std::fs::create_dir_all(&dir).expect("create the scratch crate folder");
    dir
}

/// Writes a scratch crate in `dir` whose test target `target` holds each
/// generated file as a module (`mod name { include!(...) }`).
pub fn write_crate(dir: &Path, crate_name: &str, targets: &[(&str, Vec<(String, String)>)]) {
    scratch_crate::create(dir, crate_name);
    let generated = dir.join("generated");
    std::fs::create_dir_all(&generated).expect("create the generated folder");
    for (target, files) in targets {
        let mut modules = String::new();
        for (module, rust) in files {
            std::fs::write(generated.join(format!("{module}.rs")), rust)
                .expect("write a generated file");
            modules.push_str(&format!(
                "mod {module} {{\n    include!(\"../generated/{module}.rs\");\n}}\n"
            ));
        }
        std::fs::write(dir.join("tests").join(format!("{target}.rs")), modules)
            .expect("write the test target");
    }
}

/// Runs `cargo` with `args` in the scratch crate in `dir`.
pub fn cargo(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut command = scratch_crate::cargo(dir);
    command.args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("run cargo")
}

/// Everything `cargo` printed, for a failure message.
pub fn printed(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The number of tests a `test result:` line says passed, failed and were
/// ignored, summed over every line.
pub fn results(text: &str) -> (usize, usize, usize) {
    let mut totals = (0, 0, 0);
    for line in text.lines().filter(|l| l.starts_with("test result:")) {
        let number = |word: &str| -> usize {
            line.split(';')
                .find(|part| part.trim_end().ends_with(word))
                .and_then(|part| part.split_whitespace().rev().nth(1))
                .and_then(|n| n.parse().ok())
                .unwrap_or(0)
        };
        totals.0 += number("passed");
        totals.1 += number("failed");
        totals.2 += number("ignored");
    }
    totals
}
