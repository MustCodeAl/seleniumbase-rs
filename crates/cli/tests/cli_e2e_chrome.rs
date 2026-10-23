//! The `sbrs` commands that drive a browser, run as the real binary against a loopback
//! page and a real headless Chrome. `cli_e2e.rs` has the commands that need none.
//!
//! These tests start Chrome, so they are ignored by default. Run them one at a time:
//!
//! ```text
//! cargo test --features "mcp-server test-util" --test cli_e2e_chrome -- --ignored --test-threads=1
//! ```
//!
//! Every `sbrs` browser command opens a browser of its own, does one thing and closes it.
//! The commands here are the ones that do something useful alone: they open a page, or act
//! on a URL they are given.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const SBASE: &str = env!("CARGO_BIN_EXE_sbrs");

/// A page with a title, a form and a checkbox, so a command has something to find.
const PAGE: &str = r##"<!doctype html>
<html lang="en"><head><title>CLI Fixture</title></head>
<body>
<h1 id="title">Welcome to the fixture</h1>
<form onsubmit="document.title='submitted:'+document.getElementById('user').value;return false;">
  <input id="user" name="user"> <button id="go" type="submit">Go</button>
</form>
<input type="file" id="upload">
<a href="/about">About us</a>
</body></html>"##;

const ABOUT: &str = r##"<!doctype html><title>About</title><h1>About us</h1>"##;

/// Serves the fixture on a loopback port, a thread to each connection. `/file.txt` is a small
/// download and any other path is a 404. Returns the base URL without a trailing slash.
fn serve() -> String {
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
                let (status, kind, body) = match path {
                    "/" => ("200 OK", "text/html", PAGE),
                    "/about" => ("200 OK", "text/html", ABOUT),
                    "/file.txt" => ("200 OK", "text/plain", "hello from the download\n"),
                    _ => ("404 Not Found", "text/plain", "not here"),
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            });
        }
    });
    format!("http://{address}")
}

/// Every `SB_*` variable the browser commands read, cleared so the host's own cannot leak in.
const SETTINGS_VARS: [&str; 18] = [
    "SB_BROWSER",
    "SB_HEADLESS",
    "SB_MODE",
    "SB_USER_AGENT",
    "SB_LOCALE",
    "SB_AD_BLOCK",
    "SB_PROXY",
    "SB_PROXY_PAC_URL",
    "SB_USER_DATA_DIR",
    "SB_EXTENSION_DIR",
    "SB_REUSE_SESSION",
    "SB_MOBILE",
    "SB_THREADS",
    "SB_ESC_END",
    "SB_REC_SB_CDP",
    "SB_WEBDRIVER_URL",
    "SB_START_PAGE",
    "SB_LOG_LEVEL",
];

fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(SBASE);
    command
        .args(args)
        .current_dir(dir)
        .env_remove("NO_COLOR")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null());
    for var in SETTINGS_VARS {
        command.env_remove(var);
    }
    command
}

/// Runs `sbrs --headless <args>` and waits for it, up to `limit`.
fn headless(dir: &Path, args: &[&str]) -> Output {
    let mut all = vec!["--headless"];
    all.extend_from_slice(args);
    finish_within(command(dir, &all), Duration::from_secs(180))
}

fn finish_within(mut command: Command, limit: Duration) -> Output {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sbrs starts");
    let start = Instant::now();
    loop {
        if child.try_wait().expect("poll sbrs").is_some() {
            return child.wait_with_output().expect("collect the output");
        }
        assert!(start.elapsed() < limit, "sbrs did not finish in {limit:?}");
        thread::sleep(Duration::from_millis(100));
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn open_and_smoke_print_their_answer_and_only_that_on_standard_output() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();

    // The default log level is on: the answer must still be all that stdout carries.
    let opened = headless(dir.path(), &["open", &format!("{base}/")]);
    assert!(opened.status.success(), "{}", stderr(&opened));
    assert_eq!(stdout(&opened), "Title: CLI Fixture\n");

    let passed = headless(
        dir.path(),
        &["smoke", &format!("{base}/"), "--title-contains", "Fixture"],
    );
    assert!(passed.status.success(), "{}", stderr(&passed));
    assert_eq!(
        stdout(&passed),
        "Assertion passed: title contains 'Fixture'\n"
    );

    let failed = headless(
        dir.path(),
        &["smoke", &format!("{base}/"), "--title-contains", "Nope"],
    );
    assert_eq!(failed.status.code(), Some(1), "{}", stderr(&failed));
    assert!(stdout(&failed).is_empty(), "{}", stdout(&failed));
    assert!(
        stderr(&failed).contains("expected title to contain 'Nope', got 'CLI Fixture'"),
        "{}",
        stderr(&failed)
    );
    assert!(stderr(&failed).contains("hint:"), "{}", stderr(&failed));

    // SB_HEADLESS does what --headless does.
    let by_environment = finish_within(
        {
            let mut command = command(dir.path(), &["open", &format!("{base}/")]);
            command.env("SB_HEADLESS", "true");
            command
        },
        Duration::from_secs(180),
    );
    assert!(
        by_environment.status.success(),
        "{}",
        stderr(&by_environment)
    );
    assert_eq!(stdout(&by_environment), "Title: CLI Fixture\n");
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn every_mode_opens_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    for mode in ["--cdp", "--uc"] {
        let opened = headless(dir.path(), &[mode, "open", &format!("{base}/")]);
        assert!(opened.status.success(), "{mode}: {}", stderr(&opened));
        assert_eq!(stdout(&opened), "Title: CLI Fixture\n", "{mode}");
    }
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn the_commands_that_speak_the_devtools_protocol_work_without_asking_for_a_mode() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let page = format!("{base}/");

    let evaluated = headless(
        dir.path(),
        &[
            "cdp",
            "--cmd",
            "Runtime.evaluate",
            "--params",
            r#"{"expression":"1+2","returnByValue":true}"#,
        ],
    );
    assert!(evaluated.status.success(), "cdp: {}", stderr(&evaluated));
    assert!(
        stdout(&evaluated).contains("\"value\":3"),
        "{}",
        stdout(&evaluated)
    );

    // These act on a page, and the two that change the profile need one to keep.
    let profile = dir.path().join("profile").display().to_string();
    for (args, said) in [
        (
            vec!["--url", &page, "--user-data-dir", &profile, "cache-clear"],
            "CDP cache clear command sent.",
        ),
        (
            vec!["--url", &page, "--user-data-dir", &profile, "clear-cookies"],
            "Cookies cleared",
        ),
        (vec!["--url", &page, "get-cookies"], "Cookies:"),
    ] {
        let output = headless(dir.path(), &args);
        assert!(output.status.success(), "{args:?}: {}", stderr(&output));
        assert!(
            stdout(&output).contains(said),
            "{args:?}: {}",
            stdout(&output)
        );
    }

    let printed = headless(dir.path(), &["print-pdf", &page, "-o", "fixture.pdf"]);
    assert!(printed.status.success(), "print-pdf: {}", stderr(&printed));
    let pdf = std::fs::read(dir.path().join("fixture.pdf")).expect("the PDF is written");
    assert!(pdf.starts_with(b"%PDF"), "a real PDF");

    // Reading the text back needs the `pdf` feature.
    #[cfg(feature = "pdf")]
    {
        let text = headless(dir.path(), &["pdf-text", &page]);
        assert!(text.status.success(), "pdf-text: {}", stderr(&text));
        assert!(
            stdout(&text).contains("Welcome to the fixture"),
            "{}",
            stdout(&text)
        );

        let found = headless(
            dir.path(),
            &["assert-pdf-text", &page, "--text", "Welcome to the fixture"],
        );
        assert!(found.status.success(), "{}", stderr(&found));
        let absent = headless(
            dir.path(),
            &["assert-pdf-text", &page, "--text", "no such words"],
        );
        assert!(!absent.status.success());
        assert!(stdout(&absent).is_empty(), "{}", stdout(&absent));
    }
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn pages_files_and_assertions_are_handled_by_their_own_commands() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let page = format!("{base}/");

    // A file by URL: the browser cannot save it here, so the HTTP fallback does, and says so.
    let downloaded = headless(
        dir.path(),
        &[
            "download-file",
            &format!("{base}/file.txt"),
            "-o",
            "got.txt",
            "--timeout",
            "2",
        ],
    );
    assert!(downloaded.status.success(), "{}", stderr(&downloaded));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("got.txt")).unwrap(),
        "hello from the download\n"
    );

    // A file into a file input.
    std::fs::write(dir.path().join("up.txt"), "upload me").unwrap();
    let chosen = headless(
        dir.path(),
        &["choose-file", "#upload", "up.txt", "--url", &page],
    );
    assert!(chosen.status.success(), "{}", stderr(&chosen));
    assert!(
        stdout(&chosen).contains("Uploaded 'up.txt' into '#upload'"),
        "{}",
        stdout(&chosen)
    );

    // Assertions that all run, then all report.
    let deferred = headless(
        dir.path(),
        &[
            "deferred",
            "--url",
            &page,
            "--assert",
            "element=#go",
            "--assert",
            "title=Fixture",
        ],
    );
    assert!(deferred.status.success(), "{}", stderr(&deferred));
    assert!(
        stdout(&deferred).contains("PASS  element exists: #go"),
        "{}",
        stdout(&deferred)
    );
    let broken = headless(
        dir.path(),
        &[
            "deferred",
            "--url",
            &page,
            "--assert",
            "element=#nope",
            "--assert",
            "title=Nope",
        ],
    );
    assert_eq!(broken.status.code(), Some(1), "{}", stderr(&broken));
    let report = stdout(&broken);
    assert!(report.contains("FAIL  element exists: #nope"), "{report}");
    assert!(
        report.contains("FAIL  title contains \"Nope\""),
        "both failures: {report}"
    );
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn a_scenario_runs_and_leaves_a_dashboard_and_a_recording_replays() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let page = format!("{base}/");

    std::fs::write(
        dir.path().join("scenario.json"),
        format!(
            r##"{{"name":"fixture","steps":[
                {{"action":"open","url":"{page}"}},
                {{"action":"type_text","css":"#user","text":"bob"}},
                {{"action":"click","css":"#go"}},
                {{"action":"assert_element","css":"#title"}}
            ]}}"##
        ),
    )
    .unwrap();
    let ran = headless(
        dir.path(),
        &[
            "run-scenario",
            "--file",
            "scenario.json",
            "--dashboard",
            "dash.html",
        ],
    );
    assert!(ran.status.success(), "{}", stderr(&ran));
    let dashboard = std::fs::read_to_string(dir.path().join("dash.html")).expect("a dashboard");
    assert!(dashboard.contains("fixture"), "{dashboard}");

    let failing = dir.path().join("failing.json");
    std::fs::write(
        &failing,
        format!(
            r##"{{"name":"broken","steps":[
                {{"action":"open","url":"{page}"}},
                {{"action":"click","css":"#nope"}}
            ]}}"##
        ),
    )
    .unwrap();
    let broke = headless(
        dir.path(),
        &["run-scenario", "--file", failing.to_str().unwrap()],
    );
    assert!(!broke.status.success(), "a failed step is a failed run");

    // A recording (`recordings/NAME_rec.json`) replays in a browser of its own.
    std::fs::write(
        dir.path().join("actions.json"),
        format!(r##"[{{"name":"goto","target":"{page}"}},{{"name":"click","target":"#title"}}]"##),
    )
    .unwrap();
    let replayed = headless(dir.path(), &["replay", "actions.json"]);
    assert!(replayed.status.success(), "{}", stderr(&replayed));
    assert!(
        stdout(&replayed).contains("0 failed"),
        "{}",
        stdout(&replayed)
    );
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn screenshot_and_save_source_write_where_they_are_told_and_a_blank_session_has_no_element() {
    let dir = tempfile::tempdir().unwrap();

    let shot = headless(
        dir.path(),
        &["--url", "about:blank", "screenshot", "--path", "shot.png"],
    );
    assert!(shot.status.success(), "{}", stderr(&shot));
    let png = std::fs::read(dir.path().join("shot.png")).expect("a screenshot");
    assert!(png.starts_with(b"\x89PNG"));

    let source = headless(
        dir.path(),
        &["--url", "about:blank", "save-source", "--path", "src.html"],
    );
    assert!(source.status.success(), "{}", stderr(&source));
    assert!(std::fs::read_to_string(dir.path().join("src.html"))
        .unwrap()
        .contains("<html"));

    // Each command has a browser of its own, so on its blank page nothing is found, and
    // the error says so with the exit status of a failed check.
    let nothing = headless(
        dir.path(),
        &["--url", "about:blank", "assert-element", "--css", "#nope"],
    );
    assert_eq!(nothing.status.code(), Some(1));
    assert!(stderr(&nothing).contains("#nope"), "{}", stderr(&nothing));
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn get_text_reads_the_start_page_under_any_of_its_spellings() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let page = format!("{base}/");
    let answer = "Text for '#title': Welcome to the fixture\n";

    for args in [
        vec!["--url", &page, "get-text", "--css", "#title"],
        vec!["--start-page", &page, "get-text", "--css", "#title"],
        vec!["--start_page", &page, "get-text", "--css", "#title"],
        // A global option may follow the command.
        vec!["get-text", "--css", "#title", "--url", &page],
    ] {
        let output = headless(dir.path(), &args);
        assert!(output.status.success(), "{args:?}: {}", stderr(&output));
        assert_eq!(stdout(&output), answer, "{args:?}");
    }

    // SB_START_PAGE is the page when the command line names none.
    let mut by_environment = command(dir.path(), &["--headless", "get-text", "--css", "#title"]);
    by_environment.env("SB_START_PAGE", &page);
    let output = finish_within(by_environment, Duration::from_secs(180));
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), answer);

    // A local file is a page too.
    std::fs::write(
        dir.path().join("local.html"),
        "<!doctype html><title>Local</title><h1 id=\"title\">From a file</h1>",
    )
    .unwrap();
    let output = headless(
        dir.path(),
        &["--url", "local.html", "get-text", "--css", "#title"],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "Text for '#title': From a file\n");
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn save_screenshot_writes_a_png_just_before_the_browser_quits() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let page = format!("{base}/");

    let output = headless(
        dir.path(),
        &["--save-screenshot", "--url", &page, "get-title"],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("Title: CLI Fixture"), "{text}");
    let saved = lines
        .next()
        .and_then(|line| line.strip_prefix("Saved screenshot: "))
        .unwrap_or_else(|| panic!("no screenshot was reported:\n{text}"));
    assert!(lines.next().is_none(), "{text}");
    let png = std::fs::read(dir.path().join(saved)).expect("the screenshot exists");
    assert!(
        png.starts_with(b"\x89PNG\r\n\x1a\n"),
        "not a PNG: {saved} ({} bytes)",
        png.len()
    );
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn a_click_that_navigates_says_where_the_browser_ended() {
    let dir = tempfile::tempdir().unwrap();
    let base = serve();
    let page = format!("{base}/");

    let output = headless(
        dir.path(),
        &["--url", &page, "click-partial-link-text", "--text", "About"],
    );

    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!("Clicked partial link text 'About'\nNow at: {base}/about (About)\n")
    );
}
