//! A real-browser check of `BaseCase::switch_to_frame`, `frame_switch`,
//! `switch_to_window` and `switch_to_alert`, which take what Python's do.
//!
//! Python's `switch_to_frame(frame)` takes the id or the name of a frame, a
//! selector or a position, and waits for the frame. `frame_switch` is a context
//! manager that leaves the frame when its block ends. `switch_to_window` takes a
//! window handle or a position (`0`, `-1`) and waits for the window, and
//! `switch_to_alert` waits for the alert and returns it. The pages below are
//! served from the loopback interface: a page with frames of every kind (an id
//! only, a name only, a selector, a hidden one, one that holds another, one that
//! shows up late), a link that opens a window, and prompts.
//!
//! The tests are ignored by default. Run them locally with Chrome and a matching
//! chromedriver installed, one binary at a time:
//!
//! ```bash
//! cargo test --test frames_windows_alerts_chrome -- --ignored --test-threads=1
//! ```
//!
//! Set `SB_WEBDRIVER_URL` to use a remote grid instead of the chromedriver that
//! `BaseCase` starts on port 9515.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use seleniumbase_rs::{BaseCase, BrowserConfig, SeleniumBaseError};

/// Serializes browser sessions so parallel tests do not race to bind
/// chromedriver's default port when auto-starting the driver.
static BROWSER_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The top page: a link that opens a window, and the frames. `by-id` has only
/// an id, `by-name` only a name, `card` is told apart by its class, and
/// `hidden-one` is not displayed. The `card` frame holds a frame of its own.
const TOP_PAGE: &str = "<!doctype html><html><head><title>Top</title></head><body>\
    <h1 id=\"top\">Top</h1>\
    <iframe id=\"by-id\" src=\"/inner?who=by-id\"></iframe>\
    <iframe name=\"by-name\" src=\"/inner?who=by-name\"></iframe>\
    <iframe class=\"card\" title=\"Card\" src=\"/outer\"></iframe>\
    <iframe id=\"hidden-one\" src=\"/inner?who=hidden-one\" style=\"display:none\"></iframe>\
    <a id=\"open-second\" href=\"/second?label=Opened\" target=\"_blank\">second</a>\
    </body></html>";

fn test_config() -> BrowserConfig {
    let mut config = BrowserConfig {
        headless: true,
        auto_start_driver: true,
        ..BrowserConfig::default()
    };
    if let Ok(url) = std::env::var("SB_WEBDRIVER_URL") {
        config.webdriver_url = url;
        config.auto_start_driver = false;
    }
    config
}

/// The value of `key` in a query string such as `who=by-id`.
fn query_value<'a>(query: &'a str, key: &str) -> &'a str {
    query
        .split('&')
        .find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
        .unwrap_or_default()
}

/// Answers one connection: [`TOP_PAGE`], the pages the frames hold, and the
/// page the link opens.
fn answer(mut stream: TcpStream) {
    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let body = match path {
        "/" => Some(TOP_PAGE.to_owned()),
        "/inner" => Some(format!(
            "<!doctype html><html><body><p id=\"who\">{}</p></body></html>",
            query_value(query, "who")
        )),
        "/outer" => Some(
            "<!doctype html><html><body><h2 id=\"outer\">Outer</h2>\
             <iframe id=\"nested\" src=\"/inner?who=nested\"></iframe></body></html>"
                .to_owned(),
        ),
        "/second" => Some(format!(
            "<!doctype html><html><head><title>Second</title></head><body>\
             <h1 id=\"second\">{}</h1></body></html>",
            query_value(query, "label")
        )),
        _ => None,
    };
    let (status, body) = match body {
        Some(body) => ("200 OK", body),
        None => ("404 Not Found", String::new()),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Starts the site and returns its `http://127.0.0.1:port` base.
fn serve() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
    let base = format!("http://{}", listener.local_addr().expect("bound address"));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            // A thread each: Chrome opens idle connections ahead of time.
            std::thread::spawn(move || answer(stream));
        }
    });
    base
}

/// A browser on the top page.
async fn on_the_top_page() -> BaseCase {
    let base = serve();
    let mut sb = BaseCase::new(test_config())
        .await
        .expect("connect to browser");
    sb.open(&format!("{base}/")).await.expect("open the page");
    sb.set_timeout(3).await.expect("a short wait");
    sb
}

/// Who the page the driver is in says it is (`#who` of the frames' pages).
async fn who(sb: &BaseCase) -> String {
    sb.get_text("#who").await.expect("a frame page with #who")
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_frame_is_entered_by_id_name_selector_or_position() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = on_the_top_page().await;

    // The id of a frame: what `switch_to_frame("iframeResult")` does on w3schools.
    sb.switch_to_frame("by-id")
        .await
        .expect("a frame is entered by its id");
    assert_eq!(who(&sb).await, "by-id");
    sb.switch_to_default_content().await.unwrap();
    assert!(sb.is_element_present("#top").await.unwrap());

    // A frame with a name and no id.
    sb.switch_to_frame("by-name")
        .await
        .expect("a frame is entered by its name");
    assert_eq!(who(&sb).await, "by-name");
    sb.switch_to_default_content().await.unwrap();

    // A selector, and a frame inside it, by its id.
    sb.switch_to_frame("iframe.card")
        .await
        .expect("a frame is entered by a selector");
    assert_eq!(sb.get_text("#outer").await.unwrap(), "Outer");
    sb.switch_to_frame("nested")
        .await
        .expect("a frame inside a frame");
    assert_eq!(who(&sb).await, "nested");
    sb.switch_to_parent_frame().await.unwrap();
    assert_eq!(sb.get_text("#outer").await.unwrap(), "Outer");
    sb.switch_to_default_content().await.unwrap();

    // An XPath selector.
    sb.switch_to_frame("//iframe[@title='Card']")
        .await
        .expect("a frame is entered by an XPath selector");
    assert_eq!(sb.get_text("#outer").await.unwrap(), "Outer");
    sb.switch_to_default_content().await.unwrap();

    // A position: the first frame of the document.
    sb.switch_to_frame(0)
        .await
        .expect("a frame is entered by its position");
    assert_eq!(who(&sb).await, "by-id");
    sb.switch_to_default_content().await.unwrap();

    sb.quit().await.expect("the browser closes");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_frame_is_waited_for_and_a_hidden_or_missing_one_is_reported() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = on_the_top_page().await;

    // A frame added by the page after a moment.
    sb.execute_script(
        "setTimeout(function () { var f = document.createElement('iframe'); \
         f.id = 'late'; f.src = '/inner?who=late'; document.body.appendChild(f); }, 700);",
    )
    .await
    .unwrap();
    sb.switch_to_frame("late")
        .await
        .expect("the call waits for the frame");
    assert_eq!(who(&sb).await, "late");
    sb.switch_to_default_content().await.unwrap();

    // A hidden frame: found by its id, but a selector waits for it to be shown
    // unless `invisible` says it may stay hidden.
    sb.switch_to_frame("hidden-one")
        .await
        .expect("an id needs no display");
    assert!(sb.is_element_present("#who").await.unwrap());
    sb.switch_to_default_content().await.unwrap();
    let error = sb
        .switch_to_frame("iframe#hidden-one")
        .timeout(Duration::from_secs(1))
        .await
        .expect_err("a selector needs a displayed frame");
    assert!(
        matches!(&error, SeleniumBaseError::WaitTimeout(message) if message.contains("hidden-one")),
        "{error:?}"
    );
    sb.switch_to_frame("iframe#hidden-one")
        .invisible(true)
        .await
        .expect("a hidden frame may be entered when told so");
    sb.switch_to_default_content().await.unwrap();

    // A frame that is not there, and a position that is not there.
    for missing in ["nothing-here", "iframe#nothing-here"] {
        let error = sb
            .switch_to_frame(missing)
            .timeout(Duration::from_secs(1))
            .await
            .expect_err("there is no such frame");
        assert!(
            matches!(&error, SeleniumBaseError::WaitTimeout(message) if message.contains(missing)),
            "{error:?}"
        );
    }
    let error = sb
        .switch_to_frame(9)
        .timeout(Duration::from_secs(1))
        .await
        .expect_err("there is no ninth frame");
    assert!(
        matches!(error, SeleniumBaseError::WaitTimeout(_)),
        "{error:?}"
    );
    assert!(sb.is_element_present("#top").await.unwrap());

    sb.quit().await.expect("the browser closes");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn frame_switch_runs_in_the_frame_and_leaves_it_after() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = on_the_top_page().await;

    // Python: `with self.frame_switch("by-id"): ...`
    let inside = sb
        .frame_switch("by-id")
        .run(async |sb| sb.get_text("#who").await)
        .await
        .expect("the frame is entered");
    assert_eq!(inside, "by-id");
    assert!(
        sb.is_element_present("#top").await.unwrap(),
        "the block ended in the top document"
    );

    // Blocks nest: each leaves the frame it entered.
    sb.frame_switch("iframe.card")
        .run(async |sb| {
            sb.frame_switch("nested")
                .run(async |sb| {
                    assert_eq!(sb.get_text("#who").await?, "nested");
                    Ok(())
                })
                .await?;
            assert_eq!(sb.get_text("#outer").await?, "Outer");
            Ok(())
        })
        .await
        .expect("both frames are entered");
    assert!(sb.is_element_present("#top").await.unwrap());

    // A block that fails leaves the frame too, and its error comes back.
    let error = sb
        .frame_switch("by-name")
        .run(async |_sb| {
            Err::<(), _>(SeleniumBaseError::AssertionFailed(
                "the step failed".to_owned(),
            ))
        })
        .await
        .expect_err("the block fails");
    assert!(error.to_string().contains("the step failed"), "{error}");
    assert!(
        sb.is_element_present("#top").await.unwrap(),
        "a failed block also ended in the top document"
    );

    // A frame that is not there: the block does not run.
    let error = sb
        .frame_switch("nothing-here")
        .timeout(Duration::from_secs(1))
        .run(async |_sb| -> Result<(), SeleniumBaseError> {
            panic!("the block must not run without its frame")
        })
        .await
        .expect_err("there is no such frame");
    assert!(
        matches!(error, SeleniumBaseError::WaitTimeout(_)),
        "{error:?}"
    );

    sb.quit().await.expect("the browser closes");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn a_window_is_entered_by_position_or_handle_and_waited_for() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = on_the_top_page().await;
    let first = sb.get_current_window_handle().await.unwrap();

    sb.click("#open-second").await.unwrap();
    // The newest window, as `switch_to_window(-1)` in Python.
    sb.switch_to_window(-1)
        .await
        .expect("the newest window is entered");
    assert_eq!(sb.get_text("#second").await.unwrap(), "Opened");
    let second = sb.get_current_window_handle().await.unwrap();
    assert_ne!(second, first);

    // The first window, as `switch_to_window(0)`.
    sb.switch_to_window(0)
        .await
        .expect("the first window is entered");
    assert_eq!(sb.get_current_window_handle().await.unwrap(), first);
    assert!(sb.is_element_present("#top").await.unwrap());

    // A handle, and the one before the newest.
    sb.switch_to_window(second.as_str())
        .await
        .expect("a window is entered by its handle");
    assert_eq!(sb.get_current_window_handle().await.unwrap(), second);
    sb.switch_to_window(-2)
        .await
        .expect("-2 is the window before the newest");
    assert_eq!(sb.get_current_window_handle().await.unwrap(), first);

    // A window that opens a moment later is waited for.
    sb.execute_script("setTimeout(function () { window.open('/second?label=Late'); }, 600);")
        .await
        .unwrap();
    sb.switch_to_window(2)
        .await
        .expect("the call waits for the third window");
    assert_eq!(sb.get_text("#second").await.unwrap(), "Late");

    // A window that is not there.
    let error = sb
        .switch_to_window(7)
        .timeout(Duration::from_secs(1))
        .await
        .expect_err("there are three windows");
    assert!(
        matches!(&error, SeleniumBaseError::WaitTimeout(message) if message.contains("Window {7}")),
        "{error:?}"
    );

    sb.quit().await.expect("the browser closes");
}

#[tokio::test]
#[ignore = "requires a local Chrome/chromedriver"]
async fn switch_to_alert_waits_for_the_alert_and_gives_it_back() {
    let _guard = BROWSER_LOCK.lock().await;
    let mut sb = on_the_top_page().await;

    // A prompt that opens a moment later: the text, the typed answer, accept.
    sb.execute_script(
        "setTimeout(function () { window.answer = window.prompt('My Prompt', 'default'); }, 400);",
    )
    .await
    .unwrap();
    let alert = sb
        .switch_to_alert()
        .await
        .expect("the call waits for the alert");
    assert_eq!(alert.text().await.unwrap(), "My Prompt");
    alert.send_keys("typed").await.unwrap();
    alert.accept().await.unwrap();
    assert!(!sb.is_alert_present().await.unwrap());
    assert_eq!(
        sb.execute_script("return window.answer;").await.unwrap(),
        serde_json::json!("typed")
    );

    // A confirm box that is dismissed: Cancel answers false.
    sb.execute_script("setTimeout(function () { window.sure = window.confirm('Sure?'); }, 300);")
        .await
        .unwrap();
    let alert = sb
        .switch_to_alert()
        .await
        .expect("the confirm box is there");
    assert_eq!(alert.text().await.unwrap(), "Sure?");
    alert.dismiss().await.unwrap();
    assert_eq!(
        sb.execute_script("return window.sure;").await.unwrap(),
        serde_json::json!(false)
    );

    // The case's own alert calls still work, and `wait_for_and_switch_to_alert`
    // gives the alert too.
    sb.execute_script("setTimeout(function () { window.alert('Hello'); }, 300);")
        .await
        .unwrap();
    let alert = sb.wait_for_and_switch_to_alert(5).await.unwrap();
    assert_eq!(sb.get_alert_text().await.unwrap(), "Hello");
    assert_eq!(alert.text().await.unwrap(), "Hello");
    sb.accept_alert().await.unwrap();

    // No alert: the call waits for its timeout and says so.
    let error = sb
        .switch_to_alert()
        .timeout(Duration::from_secs(1))
        .await
        .expect_err("no alert is open");
    assert!(
        matches!(&error, SeleniumBaseError::WaitTimeout(message) if message.contains("Alert was not present")),
        "{error:?}"
    );

    sb.quit().await.expect("the browser closes");
}
