//! `sbrs mkrec` in a real headless Chrome: the page drives itself (a script clicks a button,
//! then presses SHIFT+ESC again and again), so `--ee` ends the recording with nobody at the
//! keyboard, and the test file it writes holds the click.
//!
//! This starts Chrome, so it is ignored by default. Run it by itself:
//!
//! ```text
//! cargo test -p seleniumbase-rs-cli --test cli_mkrec_chrome -- --ignored --test-threads=1
//! ```

#[path = "support/sbrs.rs"]
mod support;

use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

use support::{command_in, stderr, stdout, SBRS};

/// A page that clicks `#go` after a second, and from two seconds on presses SHIFT+ESC every
/// half second (the Recorder arms its listener on each look, so one press could come too soon).
const PAGE: &str = r##"<!doctype html><title>Self-driving</title>
<h1 id="title">Recorder fixture</h1>
<button id="go" type="button" onclick="document.title='clicked'">Go</button>
<script>
// The Recorder keeps a click as a mouse down and a mouse up on the same element.
setTimeout(() => {
  const go = document.getElementById('go');
  for (const type of ['mousedown', 'mouseup', 'click']) {
    go.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, view: window }));
  }
}, 1000);
setTimeout(() => setInterval(() => {
  // The Recorder's listener wants a Shift key down, then Escape.
  for (const key of ['Shift', 'Escape']) {
    document.dispatchEvent(new KeyboardEvent('keydown', { key, code: key, bubbles: true }));
  }
}, 500), 2000);
</script>
"##;

/// Waits for `child`, which must end within `limit`; kills it otherwise.
fn finish_within(mut child: std::process::Child, limit: Duration) -> Output {
    let start = Instant::now();
    loop {
        if child.try_wait().expect("poll sbrs").is_some() {
            return child.wait_with_output().expect("collect output");
        }
        if start.elapsed() >= limit {
            // Ask it to stop (it closes its browser), so a failed run leaves nothing behind.
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &child.id().to_string()])
                .status();
            std::thread::sleep(Duration::from_secs(10));
            let _ = child.kill();
            let _ = child.wait();
            panic!("sbrs mkrec did not end in {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
#[ignore = "launches a real Chrome through chromedriver"]
fn a_self_driving_page_is_recorded_until_shift_esc() {
    let dir = tempfile::tempdir().expect("temp dir");
    let page = dir.path().join("page.html");
    std::fs::write(&page, PAGE).expect("write the page");
    let url = format!("file://{}", page.display());

    let mut command = command_in(SBRS, dir.path(), &[]);
    let child = command
        .args(["--headless", "mkrec", "login.rs", "--ee", "--url", &url])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("sbrs runs");
    let output = finish_within(child, Duration::from_secs(240));

    let out = stdout(&output);
    assert!(output.status.success(), "{out}\n{}", stderr(&output));
    assert!(
        out.contains(">>> Use [SHIFT + ESC] in the browser to end recording!"),
        "{out}"
    );
    assert!(out.contains("*** RECORDING COPIED to: login.rs"), "{out}");
    let test = std::fs::read_to_string(dir.path().join("login.rs")).expect("the test file");
    assert!(test.contains("page.html"), "the page was opened: {test}");
    assert!(test.contains("sb.click("), "the click was recorded: {test}");
    assert_eq!(
        test,
        std::fs::read_to_string(dir.path().join("recordings/login_rec.rs")).unwrap()
    );
    assert!(dir.path().join("recordings/login_rec.json").is_file());
}
