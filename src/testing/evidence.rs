//! What a test leaves behind when it fails (or, with `--screenshot`, when it
//! passes): the folder `latest_logs/<test id>/` of Python SeleniumBase.
//!
//! A failed attempt writes, while its browser is still open:
//!
//! - `screenshot.png`, unless `--no-screenshot`;
//! - `page_source.html`;
//! - `basic_test_info.txt`, in the layout of Python's `log_helper`: the test
//!   id, the last page, the duration, the browser, the time and the error.
//!
//! Taking evidence never changes how a test ends: a page that cannot be asked
//! (the browser crashed, the tab is gone) is logged and the files that could
//! be taken are still written.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common::debug_prompt::{folder_name, PromptPage};
use crate::core::log_helper::{
    log_page_source, log_screenshot, log_test_failure_data, FailureInfo,
};
use crate::core::report_helper::FailureDetails;
use crate::error::SeleniumBaseError;

/// What the lifecycle knows about the attempt, for the info file.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Subject<'a> {
    /// The id of the test; it names the folder.
    pub test_id: &'a str,
    /// The name a report shows; the first line of the info file.
    pub display: &'a str,
    /// The browser as configured: `chrome`, `firefox`, ...
    pub browser: &'a str,
    /// Whether the browser ran without a window.
    pub headless: bool,
    /// What drove the browser: `WebDriver` or `CDP (no driver)`.
    pub driver: &'a str,
    /// How long the attempt ran.
    pub duration: Duration,
}

/// The files one call wrote.
#[derive(Debug, Default)]
pub(crate) struct Taken {
    /// The test's folder.
    pub folder: PathBuf,
    /// The address of the page the test ended on, when it could be read.
    pub last_page: Option<String>,
    /// The files that were written.
    pub files: Vec<PathBuf>,
}

/// The folder of `test_id` under `root`: one plain name, so an id cannot name a
/// folder outside the logs.
pub(crate) fn test_folder(root: &Path, test_id: &str) -> PathBuf {
    root.join(folder_name(test_id))
}

/// Takes the evidence of a failed attempt from `page`. `screenshot` is false
/// for `--no-screenshot`. The files are written by the `log_helper` writers,
/// so they have the layout Python's do.
pub(crate) async fn take_failure(
    root: &Path,
    subject: &Subject<'_>,
    page: &dyn PromptPage,
    error: &SeleniumBaseError,
    screenshot: bool,
) -> Taken {
    let mut taken = Taken {
        folder: test_folder(root, subject.test_id),
        ..Taken::default()
    };
    taken.last_page = page.url().await.ok();
    if screenshot {
        take_screenshot(page, &mut taken).await;
    }
    take_source(page, &mut taken).await;
    let version = browser_version(subject.browser, page).await;
    let info = FailureInfo {
        test_id: subject.display.to_owned(),
        last_page: taken.last_page.clone(),
        duration: subject.duration,
        browser: Some(browser_label(subject, version.as_deref())),
        driver: Some(subject.driver.to_owned()),
        traceback: FailureDetails::from_error(error).detail,
        exception: error.to_string(),
    };
    let written = log_test_failure_data(&taken.folder, &info);
    note(&mut taken, "basic_test_info.txt", written);
    taken
}

/// Takes the screenshot and page source of a passed attempt (`--screenshot`).
pub(crate) async fn take_pass(root: &Path, subject: &Subject<'_>, page: &dyn PromptPage) -> Taken {
    let mut taken = Taken {
        folder: test_folder(root, subject.test_id),
        ..Taken::default()
    };
    taken.last_page = page.url().await.ok();
    take_screenshot(page, &mut taken).await;
    take_source(page, &mut taken).await;
    taken
}

async fn take_screenshot(page: &dyn PromptPage, taken: &mut Taken) {
    match page.screenshot_png().await {
        Ok(png) => {
            let written = log_screenshot(&taken.folder, &png);
            note(taken, "screenshot.png", written);
        }
        Err(error) => tracing::warn!(%error, "no screenshot of the page was taken"),
    }
}

async fn take_source(page: &dyn PromptPage, taken: &mut Taken) {
    match page.page_source().await {
        Ok(html) => {
            let written = log_page_source(&taken.folder, taken.last_page.as_deref(), &html);
            note(taken, "page_source.html", written);
        }
        Err(error) => tracing::warn!(%error, "the page source was not taken"),
    }
}

/// Keeps the path of a file that was written. A failure is logged: evidence
/// never fails a test.
fn note(taken: &mut Taken, what: &str, written: io::Result<PathBuf>) {
    match written {
        Ok(path) => taken.files.push(path),
        Err(error) => tracing::warn!(%error, file = what, "evidence was not written"),
    }
}

/// The browser line of the info file: `Chrome 120.0.6099.109 / headless`.
fn browser_label(subject: &Subject<'_>, version: Option<&str>) -> String {
    let headless = if subject.headless { " / headless" } else { "" };
    let name = capitalized(subject.browser);
    match version {
        Some(version) => format!("{name} {version}{headless}"),
        None => format!("{name}{headless}"),
    }
}

/// `chrome` as `Chrome`.
fn capitalized(name: &str) -> String {
    let mut letters = name.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

/// The version of the browser, read from its user agent, when the page can
/// tell.
async fn browser_version(browser: &str, page: &dyn PromptPage) -> Option<String> {
    let agent = page.evaluate("navigator.userAgent").await.ok()?;
    version_in(agent.as_str()?, browser)
}

/// The number after the product token of `browser` in `agent`: `Chrome/120.0.6099.109`
/// for Chrome and Chromium, `Edg/` for Edge, `Firefox/` for Firefox.
fn version_in(agent: &str, browser: &str) -> Option<String> {
    let token = match browser {
        "edge" => "Edg/",
        "firefox" => "Firefox/",
        _ => "Chrome/",
    };
    let (_, rest) = agent.split_once(token)?;
    let version: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    (!version.is_empty()).then_some(version)
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use serde_json::{json, Value};

    use super::*;
    use crate::core::log_helper::{BASIC_INFO_FILE, PAGE_SOURCE_FILE, SCREENSHOT_FILE};

    const RULE: &str = "--------------------------------------------------------------------";

    /// A page with fixed answers; an `Err` message makes that answer fail.
    struct FakePage {
        url: Result<&'static str, &'static str>,
        png: Result<&'static [u8], &'static str>,
        source: Result<&'static str, &'static str>,
        agent: Result<&'static str, &'static str>,
    }

    impl FakePage {
        fn working() -> Self {
            Self {
                url: Ok("https://example.com/login"),
                png: Ok(b"\x89PNG-bytes"),
                source: Ok("<html>login</html>"),
                agent: Ok("Mozilla/5.0 Chrome/120.0.6099.109 Safari/537.36"),
            }
        }
    }

    fn failed<T>(message: &str) -> Result<T, SeleniumBaseError> {
        Err(SeleniumBaseError::test_lifecycle(message))
    }

    #[async_trait]
    impl PromptPage for FakePage {
        async fn url(&self) -> Result<String, SeleniumBaseError> {
            self.url.map(str::to_owned).or_else(failed)
        }
        async fn title(&self) -> Result<String, SeleniumBaseError> {
            Ok("Login".to_owned())
        }
        async fn screenshot_png(&self) -> Result<Vec<u8>, SeleniumBaseError> {
            self.png.map(<[u8]>::to_vec).or_else(failed)
        }
        async fn page_source(&self) -> Result<String, SeleniumBaseError> {
            self.source.map(str::to_owned).or_else(failed)
        }
        async fn evaluate(&self, _expression: &str) -> Result<Value, SeleniumBaseError> {
            self.agent.map(|agent| json!(agent)).or_else(failed)
        }
    }

    fn subject(test_id: &str) -> Subject<'_> {
        Subject {
            test_id,
            display: test_id,
            browser: "chrome",
            headless: true,
            driver: "WebDriver",
            duration: Duration::from_millis(1234),
        }
    }

    fn boom() -> SeleniumBaseError {
        SeleniumBaseError::AssertionFailed("boom".to_owned())
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[tokio::test]
    async fn a_failure_leaves_the_screenshot_the_source_and_the_info() {
        let logs = tempfile::tempdir().unwrap();
        let taken = take_failure(
            logs.path(),
            &subject("login.valid_login"),
            &FakePage::working(),
            &boom(),
            true,
        )
        .await;
        let folder = logs.path().join("login.valid_login");
        assert_eq!(taken.folder, folder);
        assert_eq!(
            taken.last_page.as_deref(),
            Some("https://example.com/login")
        );
        assert_eq!(
            std::fs::read(folder.join(SCREENSHOT_FILE)).unwrap(),
            b"\x89PNG-bytes"
        );
        let source = read(&folder.join(PAGE_SOURCE_FILE));
        assert!(source.contains("<html>login</html>"), "{source}");
        assert!(
            source.contains("<base href"),
            "the saved page finds its assets"
        );
        let info = read(&folder.join(BASIC_INFO_FILE));
        let lines: Vec<&str> = info.lines().collect();
        assert_eq!(lines[0], "login.valid_login");
        assert_eq!(lines[1], RULE);
        assert_eq!(lines[2], "Last Page: https://example.com/login");
        assert_eq!(lines[3], " Duration: 1.23s");
        assert_eq!(lines[4], "  Browser: Chrome 120.0.6099.109 / headless");
        assert_eq!(lines[5], "   Driver: WebDriver");
        assert!(lines[6].starts_with("Timestamp: "), "{info}");
        assert!(lines[6].ends_with("(Unix Timestamp)"), "{info}");
        assert_eq!(lines[9], RULE);
        assert_eq!(lines[10], "Traceback:");
        assert_eq!(lines[11], "  assertion failed: boom");
        assert_eq!(lines[12], "Exception: assertion failed: boom");
        assert!(info.contains("\r\n"), "Python's file has CRLF line ends");
        assert_eq!(taken.files.len(), 3);
    }

    #[tokio::test]
    async fn no_screenshot_leaves_out_the_png_only() {
        let logs = tempfile::tempdir().unwrap();
        let taken = take_failure(
            logs.path(),
            &subject("t"),
            &FakePage::working(),
            &boom(),
            false,
        )
        .await;
        let folder = logs.path().join("t");
        assert!(!folder.join(SCREENSHOT_FILE).exists());
        assert!(folder.join(PAGE_SOURCE_FILE).is_file());
        assert!(folder.join(BASIC_INFO_FILE).is_file());
        assert_eq!(taken.files.len(), 2);
    }

    #[tokio::test]
    async fn a_page_that_cannot_answer_still_leaves_what_could_be_taken() {
        let logs = tempfile::tempdir().unwrap();
        let page = FakePage {
            url: Err("tab gone"),
            png: Err("tab gone"),
            source: Err("tab gone"),
            agent: Err("tab gone"),
        };
        let taken = take_failure(logs.path(), &subject("t"), &page, &boom(), true).await;
        assert_eq!(taken.last_page, None);
        assert_eq!(taken.files.len(), 1, "only the info file");
        let info = read(&logs.path().join("t").join(BASIC_INFO_FILE));
        assert!(info.contains("Last Page: (Unknown Page)"), "{info}");
        assert!(info.contains("  Browser: Chrome / headless"), "{info}");
    }

    #[tokio::test]
    async fn a_pass_leaves_the_screenshot_and_the_source_but_no_info() {
        let logs = tempfile::tempdir().unwrap();
        let taken = take_pass(logs.path(), &subject("t"), &FakePage::working()).await;
        let folder = logs.path().join("t");
        assert!(folder.join(SCREENSHOT_FILE).is_file());
        assert!(folder.join(PAGE_SOURCE_FILE).is_file());
        assert!(!folder.join(BASIC_INFO_FILE).exists());
        assert_eq!(taken.files.len(), 2);
    }

    #[test]
    fn a_test_id_cannot_name_a_folder_outside_the_logs() {
        let root = Path::new("latest_logs");
        for id in ["../../etc/passwd", "/abs/olute", "a/b", ".."] {
            let folder = test_folder(root, id);
            assert_eq!(folder.parent(), Some(root), "{id}");
        }
    }

    #[test]
    fn the_version_comes_from_the_product_token_of_the_browser() {
        let edge = "Mozilla/5.0 Chrome/120.0.0.0 Safari/537.36 Edg/119.0.2151.44";
        assert_eq!(version_in(edge, "edge").as_deref(), Some("119.0.2151.44"));
        assert_eq!(version_in(edge, "chrome").as_deref(), Some("120.0.0.0"));
        let firefox = "Mozilla/5.0 (X11) Gecko/20100101 Firefox/121.0";
        assert_eq!(version_in(firefox, "firefox").as_deref(), Some("121.0"));
        assert_eq!(version_in(firefox, "chrome"), None);
        assert_eq!(version_in("Chrome/", "chrome"), None);
    }
}
