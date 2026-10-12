//! Downloads on the Pure CDP engine against a scripted browser, with no Chrome.
//!
//! The scripted browser behaves as Chrome does where it matters here: it
//! remembers the folder it was told to save in, starts a download when the page
//! runs the fetching script or the mouse is released over a link, writes the
//! file (adding a number to the name if one of that name is already there),
//! and reports the download's progress as events. The tests assert which
//! commands are sent and in what order, and how the browser's events become a
//! path or an error. The test with a real Chrome is `sb_cdp_download_chrome.rs`.
//! Run these with `cargo test --features test-util`.

#![cfg(feature = "test-util")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use seleniumbase_rs::js_code::quote;
use seleniumbase_rs::sb_cdp::{Browser, MockCtrl, Page};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};

/// How the scripted browser ends a download.
#[derive(Clone, Copy)]
enum Ending {
    /// The file is saved and the browser says so.
    Completes,
    /// The browser gives up on it.
    Cancelled,
    /// It starts and the browser never says more.
    NeverFinishes,
}

/// What starts the download.
#[derive(Clone, Copy)]
enum Trigger {
    /// The page script that fetches an address and saves what comes back.
    Fetch,
    /// A click on a link.
    Click,
}

/// The download the scripted browser performs.
#[derive(Clone)]
struct Download {
    /// The name the server suggests, which the browser saves under unless the
    /// page script names the file.
    suggested: &'static str,
    /// What is written to the file; `None` writes nothing.
    contents: Option<&'static str>,
    /// Whether the event that ends the download says where the file is.
    reports_path: bool,
    ending: Ending,
}

impl Download {
    fn completing(suggested: &'static str, contents: &'static str) -> Self {
        Self {
            suggested,
            contents: Some(contents),
            reports_path: true,
            ending: Ending::Completes,
        }
    }

    fn ending(self, ending: Ending) -> Self {
        Self { ending, ..self }
    }
}

/// Wraps a script result the way `Runtime.evaluate` returns it.
fn value(v: Value) -> Value {
    json!({ "result": { "value": v } })
}

/// A script exception, as the page throws it.
fn exception(description: &str) -> Value {
    json!({ "exceptionDetails": { "exception": { "description": description } } })
}

/// A page on a mocked browser that waits 300 ms for a download to happen, so
/// the failure tests finish quickly.
async fn quick_page() -> (Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    let page = browser.default_page().await.unwrap();
    (page.with_timeout(Duration::from_millis(300)), mock)
}

/// The path the browser saves a file called `name` at in `dir`: with
/// ` (1)` before the extension, then ` (2)`, while a file of that name is there.
fn saved_name(dir: &Path, name: &str) -> PathBuf {
    let plain = dir.join(name);
    if !plain.exists() {
        return plain;
    }
    let (stem, extension) = name
        .rsplit_once('.')
        .map_or((name, ""), |(stem, ext)| (stem, ext));
    (1..)
        .map(|n| {
            let numbered = if extension.is_empty() {
                format!("{stem} ({n})")
            } else {
                format!("{stem} ({n}).{extension}")
            };
            dir.join(numbered)
        })
        .find(|path| !path.exists())
        .expect("a free name")
}

/// What the fetching script was told to name the file: its last argument.
fn named_in(expression: &str) -> Option<String> {
    let last = expression
        .trim_end()
        .strip_suffix(')')?
        .rsplit_once(", ")?
        .1;
    serde_json::from_str::<Option<String>>(last).ok().flatten()
}

/// Starts the scripted download: the events, and the file.
fn begin(
    mock: &MockCtrl,
    folder: &Mutex<Option<PathBuf>>,
    download: &Download,
    named: Option<String>,
) {
    let dir = folder
        .lock()
        .unwrap()
        .clone()
        .expect("the browser was told where to save before the download began");
    let name = named.unwrap_or_else(|| download.suggested.to_owned());
    mock.emit(
        "Browser.downloadWillBegin",
        json!({ "frameId": "F1", "guid": "g1", "url": "blob:https://files.test/1",
                "suggestedFilename": name }),
        None,
    );
    mock.emit(
        "Browser.downloadProgress",
        json!({ "guid": "g1", "totalBytes": 8, "receivedBytes": 4, "state": "inProgress" }),
        None,
    );
    match download.ending {
        Ending::Completes => {
            let saved = saved_name(&dir, &name);
            if let Some(contents) = download.contents {
                fs::write(&saved, contents).unwrap();
            }
            let mut done = json!({ "guid": "g1", "totalBytes": 8, "receivedBytes": 8,
                                   "state": "completed" });
            if download.reports_path {
                done["filePath"] = json!(saved);
            }
            mock.emit("Browser.downloadProgress", done, None);
        }
        Ending::Cancelled => mock.emit(
            "Browser.downloadProgress",
            json!({ "guid": "g1", "totalBytes": 8, "receivedBytes": 4, "state": "canceled" }),
            None,
        ),
        Ending::NeverFinishes => {}
    }
}

/// Makes the mocked browser perform `download` when `trigger` happens.
fn script_browser(mock: &MockCtrl, download: &Download, trigger: Trigger) {
    let folder: Arc<Mutex<Option<PathBuf>>> = Arc::default();
    let remembered = Arc::clone(&folder);
    mock.on("Browser.setDownloadBehavior", move |params| {
        *remembered.lock().unwrap() = params["downloadPath"].as_str().map(PathBuf::from);
        Ok(json!({}))
    });
    let (browser, download) = (mock.clone(), download.clone());
    match trigger {
        Trigger::Fetch => mock.on("Runtime.evaluate", move |params| {
            let expression = params["expression"].as_str().unwrap_or_default();
            if expression.contains("fetch(") {
                begin(&browser, &folder, &download, named_in(expression));
                Ok(value(json!(true)))
            } else {
                Ok(value(Value::Null))
            }
        }),
        Trigger::Click => {
            // One visible element is there to be clicked.
            mock.on("Runtime.evaluate", |params| {
                let expression = params["expression"].as_str().unwrap_or_default();
                Ok(if expression.contains("__sbcdp.center(") {
                    value(json!({ "x": 20.0, "y": 10.0, "covered": false }))
                } else if expression.contains(".some(__sbcdp.visible)") {
                    value(json!(true))
                } else if expression.contains("resolve(") && expression.contains(".length === 0") {
                    value(json!(false))
                } else if expression.contains("resolve(") && expression.contains(".length > 0") {
                    value(json!(true))
                } else if expression.contains("resolve(") && expression.contains(".length") {
                    value(json!(1))
                } else {
                    value(Value::Null)
                })
            });
            mock.on("Input.dispatchMouseEvent", move |params| {
                if params["type"] == "mouseReleased" {
                    begin(&browser, &folder, &download, None);
                }
                Ok(json!({}))
            });
        }
    }
}

/// What was asked of the browser that concerns a download, in order: the
/// folder was chosen, the page was asked to fetch, the mouse was released.
fn steps(mock: &MockCtrl) -> Vec<&'static str> {
    mock.calls()
        .into_iter()
        .filter_map(|call| match call.method.as_str() {
            "Browser.setDownloadBehavior" => Some("choose the folder"),
            "Runtime.evaluate"
                if call.params["expression"]
                    .as_str()
                    .is_some_and(|e| e.contains("fetch(")) =>
            {
                Some("fetch")
            }
            "Input.dispatchMouseEvent" if call.params["type"] == "mouseReleased" => Some("click"),
            _ => None,
        })
        .collect()
}

/// The scripts the page was asked to run to fetch an address.
fn fetching_scripts(mock: &MockCtrl) -> Vec<String> {
    mock.calls_to("Runtime.evaluate")
        .into_iter()
        .filter_map(|call| call.params["expression"].as_str().map(str::to_owned))
        .filter(|expression| expression.contains("fetch("))
        .collect()
}

#[tokio::test]
async fn downloading_a_url_tells_the_browser_where_to_save_and_returns_the_saved_file() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    script_browser(
        &mock,
        &Download::completing("report.csv", "a,b\n1,2\n"),
        Trigger::Fetch,
    );

    let saved = page
        .download_file("https://files.test/export?id=7", folder.path())
        .await
        .unwrap();

    let dir = folder.path().canonicalize().unwrap();
    assert_eq!(saved, dir.join("report.csv"));
    assert_eq!(fs::read_to_string(&saved).unwrap(), "a,b\n1,2\n");
    assert_eq!(
        steps(&mock),
        ["choose the folder", "fetch"],
        "the browser is told where to save before the page is asked to download"
    );
    assert_eq!(
        mock.calls_to("Browser.setDownloadBehavior")[0].params,
        json!({ "behavior": "allow", "downloadPath": dir, "eventsEnabled": true })
    );
    assert_eq!(page.browser().download_dir(), dir);
}

#[tokio::test]
async fn the_script_that_fetches_keeps_the_url_inside_its_string() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    script_browser(&mock, &Download::completing("x.bin", "x"), Trigger::Fetch);
    let hostile = "https://files.test/a'); alert(1); ('</script>\u{2028}";

    page.download_file(hostile, folder.path()).await.unwrap();

    let scripts = fetching_scripts(&mock);
    assert_eq!(scripts.len(), 1);
    let script = &scripts[0];
    assert!(script.contains(&quote(hostile)), "{script}");
    assert!(
        !script.replace(&quote(hostile), "").contains("alert"),
        "the address escaped its string: {script}"
    );
}

#[tokio::test]
async fn without_a_reported_path_the_file_is_the_suggested_name_in_the_folder() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    let mut download = Download::completing("notes.txt", "hello");
    download.reports_path = false;
    script_browser(&mock, &download, Trigger::Fetch);

    let saved = page
        .download_file("https://files.test/notes", folder.path())
        .await
        .unwrap();

    assert_eq!(
        saved,
        folder.path().canonicalize().unwrap().join("notes.txt")
    );
}

#[tokio::test]
async fn a_name_that_would_leave_the_folder_is_refused() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    let mut download = Download::completing("../escape.txt", "x");
    download.reports_path = false;
    download.contents = None;
    script_browser(&mock, &download, Trigger::Fetch);

    let error = page
        .download_file("https://files.test/evil", folder.path())
        .await
        .expect_err("a suggested name with a directory part is not a file name");

    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_download_the_browser_says_finished_but_left_no_file_for_is_an_error() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    let mut download = Download::completing("ghost.bin", "");
    download.contents = None;
    script_browser(&mock, &download, Trigger::Fetch);

    let error = page
        .download_file("https://files.test/ghost", folder.path())
        .await
        .expect_err("there is no file");

    assert!(
        matches!(&error, SeleniumBaseError::Download { reason, .. } if reason.contains("ghost.bin")),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_second_download_of_a_name_keeps_the_first_and_returns_the_numbered_file() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    script_browser(
        &mock,
        &Download::completing("data.csv", "second"),
        Trigger::Fetch,
    );
    fs::write(folder.path().join("data.csv"), "first").unwrap();

    let saved = page
        .download_file("https://files.test/data", folder.path())
        .await
        .unwrap();

    assert_eq!(saved.file_name().unwrap(), "data (1).csv");
    assert_eq!(fs::read_to_string(&saved).unwrap(), "second");
    assert_eq!(
        fs::read_to_string(folder.path().join("data.csv")).unwrap(),
        "first"
    );
}

#[tokio::test]
async fn a_download_the_browser_cancels_is_reported_as_failed() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    let download = Download::completing("big.iso", "").ending(Ending::Cancelled);
    script_browser(&mock, &download, Trigger::Fetch);

    let error = page
        .download_file("https://files.test/big", folder.path())
        .await
        .expect_err("the download was cancelled");

    assert!(
        matches!(&error, SeleniumBaseError::Download { reason, .. } if reason.contains("cancel")),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_download_that_never_begins_times_out_naming_the_address() {
    let (page, _mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();

    let error = page
        .download_file("https://files.test/never", folder.path())
        .await
        .expect_err("nothing was downloaded");

    assert!(
        matches!(&error, SeleniumBaseError::WaitTimeout(message)
            if message.contains("https://files.test/never") && message.contains("begin")),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_download_that_never_finishes_times_out_and_says_it_began() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    let download = Download::completing("slow.bin", "").ending(Ending::NeverFinishes);
    script_browser(&mock, &download, Trigger::Fetch);

    let error = page
        .download_file("https://files.test/slow", folder.path())
        .await
        .expect_err("the download never ends");

    assert!(
        matches!(&error, SeleniumBaseError::WaitTimeout(message) if message.contains("finish")),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_page_that_cannot_fetch_the_url_fails_at_once_with_the_reason() {
    let (browser, mock) = Browser::new_mocked();
    let page = browser
        .default_page()
        .await
        .unwrap()
        .with_timeout(Duration::from_secs(30));
    let folder = tempfile::tempdir().unwrap();
    mock.on("Runtime.evaluate", |_| {
        Ok(exception("Error: HTTP 404 Not Found"))
    });
    let started = Instant::now();

    let error = page
        .download_file("https://files.test/missing", folder.path())
        .await
        .expect_err("the server has no such file");

    assert!(
        matches!(&error, SeleniumBaseError::Download { url, reason }
            if url == "https://files.test/missing" && reason.contains("404")),
        "{error:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "it waited for the download"
    );
}

#[tokio::test]
async fn saving_a_file_as_a_path_names_the_file_and_returns_that_path() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    script_browser(
        &mock,
        &Download::completing("server-name.csv", "rows"),
        Trigger::Fetch,
    );
    let target = folder.path().join("exports").join("renamed.csv");

    let saved = page
        .save_file_as("https://files.test/data", &target)
        .await
        .unwrap();

    assert_eq!(
        saved,
        folder
            .path()
            .canonicalize()
            .unwrap()
            .join("exports")
            .join("renamed.csv")
    );
    assert_eq!(fs::read_to_string(&saved).unwrap(), "rows");
    let scripts = fetching_scripts(&mock);
    assert_eq!(scripts.len(), 1);
    let script = &scripts[0];
    assert!(
        script.trim_end().ends_with(r#", "renamed.csv")"#),
        "the page is told what to name the file: {script}"
    );
}

#[tokio::test]
async fn saving_over_a_file_replaces_it_and_leaves_no_numbered_copy() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    script_browser(&mock, &Download::completing("x.csv", "new"), Trigger::Fetch);
    let target = folder.path().join("renamed.csv");
    fs::write(&target, "old").unwrap();

    let saved = page
        .save_file_as("https://files.test/data", &target)
        .await
        .unwrap();

    assert_eq!(saved, target.canonicalize().unwrap());
    assert_eq!(fs::read_to_string(&saved).unwrap(), "new");
    let names: Vec<_> = fs::read_dir(folder.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        names,
        ["renamed.csv"],
        "the browser's numbered copy was moved over the old file"
    );
}

#[tokio::test]
async fn a_path_with_no_file_name_is_refused_before_the_browser_is_asked() {
    let (page, mock) = quick_page().await;

    let error = page
        .save_file_as("https://files.test/data", "/")
        .await
        .expect_err("there is no file name in the path");

    assert!(
        matches!(error, SeleniumBaseError::InvalidConfig(_)),
        "{error:?}"
    );
    assert_eq!(steps(&mock), Vec::<&str>::new());
}

#[tokio::test]
async fn clicking_a_link_downloads_what_the_browser_saves() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    script_browser(
        &mock,
        &Download::completing("invoice.pdf", "%PDF"),
        Trigger::Click,
    );

    let saved = page
        .download_by_click("#download", folder.path())
        .await
        .unwrap();

    assert_eq!(
        saved,
        folder.path().canonicalize().unwrap().join("invoice.pdf")
    );
    assert_eq!(fs::read_to_string(&saved).unwrap(), "%PDF");
    assert_eq!(
        steps(&mock),
        ["choose the folder", "click"],
        "the folder is chosen before the click"
    );
}

#[tokio::test]
async fn a_click_that_starts_no_download_times_out_naming_the_selector() {
    let (page, mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    // A download is scripted, but never triggered: nothing is a download link.
    script_browser(&mock, &Download::completing("a.bin", "x"), Trigger::Click);
    mock.on("Input.dispatchMouseEvent", |_| Ok(json!({})));

    let error = page
        .download_by_click("#plain-link", folder.path())
        .await
        .expect_err("the click started no download");

    assert!(
        matches!(&error, SeleniumBaseError::WaitTimeout(message)
            if message.contains("#plain-link") && message.contains("begin")),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_downloaded_file_is_found_in_the_folder_downloads_go_to() {
    let (page, _mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    page.browser()
        .set_download_dir(folder.path())
        .await
        .unwrap();
    let late = folder.path().join("late.txt");
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        fs::write(late, "arrived").unwrap();
    });

    let found = page
        .assert_downloaded_file("late.txt", Duration::from_secs(5))
        .await
        .unwrap();

    assert_eq!(
        found,
        folder.path().canonicalize().unwrap().join("late.txt")
    );
}

#[tokio::test]
async fn a_file_that_does_not_arrive_times_out_naming_it_and_the_folder() {
    let (page, _mock) = quick_page().await;
    let folder = tempfile::tempdir().unwrap();
    page.browser()
        .set_download_dir(folder.path())
        .await
        .unwrap();
    // The browser's unfinished file does not count.
    fs::write(folder.path().join("half.zip.crdownload"), "partial").unwrap();

    let error = page
        .assert_downloaded_file("half.zip", Duration::from_millis(200))
        .await
        .expect_err("the download did not finish");

    let SeleniumBaseError::WaitTimeout(message) = &error else {
        panic!("expected a WaitTimeout, got {error:?}");
    };
    assert!(message.contains("half.zip"), "{message}");
    assert!(
        message.contains(&folder.path().canonicalize().unwrap().display().to_string()),
        "{message}"
    );
}

#[tokio::test]
async fn only_a_plain_file_name_can_be_asserted() {
    let (page, _mock) = quick_page().await;
    let started = Instant::now();

    for name in ["../secret.txt", "dir/file.txt", ""] {
        let error = page
            .assert_downloaded_file(name, Duration::from_secs(30))
            .await
            .expect_err("not a plain file name");
        assert!(
            matches!(error, SeleniumBaseError::InvalidConfig(_)),
            "{name:?}: {error:?}"
        );
    }

    assert!(
        started.elapsed() < Duration::from_secs(5),
        "it waited for a name it refuses"
    );
}

#[tokio::test]
async fn until_a_folder_is_chosen_downloads_are_expected_in_downloaded_files() {
    let (browser, _mock) = Browser::new_mocked();

    assert_eq!(browser.download_dir(), PathBuf::from("downloaded_files"));
}
