//! Allure results for a run (`SB_ALLURE_DIR`, feature `allure`).
//!
//! [`AllureSink`] is a [`RunSink`]: it hears about every test of a [`TestRun`]
//! and writes the [Allure 2 results format] into one folder, so that
//! `allure serve <folder>` shows the run with its statuses, durations, failure
//! messages and traces, labels, and the screenshot and page source of a failure
//! as attachments. A run adds the sink by itself when `SB_ALLURE_DIR` (or
//! `--alluredir`, or `allure_dir` in the `[report]` table) is set and the
//! feature is on; see [`attach`].
//!
//! ```text
//! <dir>/<uuid>-result.json                    one per test attempt
//! <dir>/<uuid>-screenshot-attachment.png      the evidence of that attempt
//! <dir>/<uuid>-page_source-attachment.html
//! <dir>/<uuid>-basic_test_info-attachment.txt
//! <dir>/environment.properties                written when it is missing
//! ```
//!
//! The results are written by hand with `serde`: the format is a small, stable
//! JSON document, and the `allure-rust-commons` crate would add a dependency
//! (and its lifecycle, which stamps the time of its own calls) for a sink that is
//! told afterwards how long each test took.
//!
//! # What a result holds
//!
//! | Field | Value |
//! |---|---|
//! | `fullName` | the test's display id, such as `login_chrome::tests::test_login` |
//! | `name` | the last path segment, or the scenario's name |
//! | `historyId`, `testCaseId` | the first 32 hex digits of the SHA-256 of `fullName` |
//! | `status` | `passed`, `failed`, `broken` or `skipped`; see [`AllureStatus`] |
//! | `statusDetails` | `message` is the first line of the error or the skip reason, `trace` the error with its causes |
//! | `start`, `stop` | milliseconds since the epoch; `stop` is when the test ended, `start` is `stop` minus its duration |
//! | `labels` | `framework`, `language`, `suite` (the test binary or feature file), `package`, `testClass`, `subSuite`, `testMethod`, `testId`; for a scenario `feature`, `story` and a `tag` for each tag |
//! | `parameters` | `browser` and `driver`, when the page could tell |
//! | `attachments` | `screenshot.png`, `page_source.html` and `basic_test_info.txt`, for each one in the test's log folder |
//! | `description` | the page's URL and the error's hint, for a failure |
//!
//! No container file is written: nothing here has a set-up or tear-down to
//! group.
//!
//! # One result per attempt
//!
//! A test is never recorded twice. The attempts of a process are remembered by
//! folder and test id: a second sink on the same folder, a second `finished`
//! for one attempt, or a recorder that is finished and then dropped, write the
//! same files again instead of a second result. A test that starts again after
//! it ended (a retry of the `sb` fixture) is a new attempt with its own result
//! and the same `historyId`, which Allure shows as a retry.
//!
//! `allure-cargotest` writes a result of its own for a test with
//! `#[allure_test]`, into `ALLURE_RESULTS_DIR` (by default `target/allure-results`).
//! This sink cannot see that writer. Point `SB_ALLURE_DIR` somewhere else, or
//! leave the macro off the tests that use the `sb` fixture; in one folder Allure
//! would list such a test twice.
//!
//! [Allure 2 results format]: https://allurereport.org/docs/how-it-works-test-result-file/

use std::collections::BTreeMap;
use std::env::consts::{ARCH, OS};
use std::fmt;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use std::{fs, io};

use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};
use serde::Serialize;

use crate::core::log_helper::{TestId, BASIC_INFO_FILE, PAGE_SOURCE_FILE, SCREENSHOT_FILE};
use crate::core::report_helper::{write_atomically, TestStatus};
use crate::core::test_run::{FinishedTest, RunSink, TestRun};
use crate::error::SeleniumBaseError;

/// The value of the `framework` label of every result.
pub const FRAMEWORK: &str = "seleniumbase-rs";

/// The file with the run's environment, in the results folder.
pub const ENVIRONMENT_FILE: &str = "environment.properties";

/// Where a sink reads the time: milliseconds since the Unix epoch.
pub type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// Where a sink gets the uuid of each result.
pub type IdSource = Arc<dyn Fn() -> String + Send + Sync>;

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

/// The status of an Allure result.
///
/// Allure tells a test that failed a check (`failed`) from one that could not
/// run to its end (`broken`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AllureStatus {
    /// The test passed.
    Passed,
    /// The test failed a check: an assertion, a text or element that did not
    /// match, or a panic.
    Failed,
    /// The test ended on any other error, or never reported its end.
    Broken,
    /// The test was skipped.
    Skipped,
}

impl AllureStatus {
    /// The word Allure reads.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Broken => "broken",
            Self::Skipped => "skipped",
        }
    }

    /// The status of a test that ended.
    ///
    /// A failure with the category `assertion_failed` or `element_not_found` is
    /// `failed`; so is a failure that has no error behind it, which is how the
    /// recorder records a panic. Any other error, and a test that never
    /// reported, is `broken`.
    #[must_use]
    pub fn of(finished: &FinishedTest<'_>) -> Self {
        match finished.result.status {
            TestStatus::Passed => Self::Passed,
            TestStatus::Skipped => Self::Skipped,
            TestStatus::Untested | TestStatus::Unreported => Self::Broken,
            TestStatus::Failed => {
                let kind = finished.error.map(SeleniumBaseError::category).or_else(|| {
                    let failure = finished.result.failure.as_ref();
                    failure.map(|failure| failure.kind.as_str())
                });
                match kind {
                    Some("assertion_failed" | "element_not_found") | None => Self::Failed,
                    Some("test_lifecycle") if finished.error.is_none() => Self::Failed,
                    Some(_) => Self::Broken,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The result file
// ---------------------------------------------------------------------------

/// A `name`/`value` pair: a label or a parameter.
#[derive(Debug, Serialize)]
struct Pair {
    name: String,
    value: String,
}

impl Pair {
    fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_owned(),
            value: value.to_owned(),
        }
    }
}

/// A file next to the result, which the result points to.
#[derive(Debug, Serialize)]
struct AttachmentRef {
    name: String,
    source: String,
    #[serde(rename = "type")]
    mime: String,
}

#[derive(Debug, Serialize)]
struct StatusDetails {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<String>,
}

/// One `<uuid>-result.json`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResultFile {
    uuid: String,
    history_id: String,
    test_case_id: String,
    full_name: String,
    name: String,
    status: AllureStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_details: Option<StatusDetails>,
    stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    start: i64,
    stop: i64,
    labels: Vec<Pair>,
    parameters: Vec<Pair>,
    attachments: Vec<AttachmentRef>,
    steps: Vec<serde_json::Value>,
    links: Vec<serde_json::Value>,
}

/// The evidence a test leaves in its log folder, and how a result names it.
struct Evidence {
    /// The file in the log folder.
    file: &'static str,
    /// The part of the attachment's file name after the result's uuid.
    stem: &'static str,
    /// The name the report shows.
    name: &'static str,
    /// The attachment's media type.
    mime: &'static str,
}

const EVIDENCE: [Evidence; 3] = [
    Evidence {
        file: SCREENSHOT_FILE,
        stem: "screenshot",
        name: "screenshot.png",
        mime: "image/png",
    },
    Evidence {
        file: PAGE_SOURCE_FILE,
        stem: "page_source",
        name: "page_source.html",
        mime: "text/html",
    },
    Evidence {
        file: BASIC_INFO_FILE,
        stem: "basic_test_info",
        name: "basic_test_info.txt",
        mime: "text/plain",
    },
];

/// The first 32 hex digits of the SHA-256 of `text`: stable across runs, so
/// Allure can follow a test's history.
fn history_id(text: &str) -> String {
    let sum = digest::digest(&digest::SHA256, text.as_bytes());
    hex(&sum.as_ref()[..16])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

/// A random version 4 uuid.
fn random_uuid() -> String {
    static FALLBACK: AtomicU64 = AtomicU64::new(0);
    let mut bytes = [0_u8; 16];
    if SystemRandom::new().fill(&mut bytes).is_err() {
        // The system has no randomness: the time, the process and a counter are unique enough.
        let seed = format!(
            "{}-{}-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
            std::process::id(),
            FALLBACK.fetch_add(1, Ordering::Relaxed)
        );
        let sum = digest::digest(&digest::SHA256, seed.as_bytes());
        bytes.copy_from_slice(&sum.as_ref()[..16]);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let digits = hex(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &digits[0..8],
        &digits[8..12],
        &digits[12..16],
        &digits[16..20],
        &digits[20..32]
    )
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// `seconds` as whole milliseconds; a negative or invalid duration is zero.
fn duration_ms(seconds: f64) -> i64 {
    Duration::try_from_secs_f64(seconds)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

/// The name of a test: the last path segment, or the scenario's name.
fn short_name(test: &TestId) -> String {
    match &test.scenario {
        Some(scenario) => scenario.name.clone(),
        None => test
            .display
            .rsplit_once("::")
            .map_or(test.display.as_str(), |(_, method)| method)
            .to_owned(),
    }
}

fn labels(test: &TestId) -> Vec<Pair> {
    let mut labels = vec![
        Pair::new("framework", FRAMEWORK),
        Pair::new("language", "rust"),
    ];
    if !test.suite.is_empty() {
        labels.push(Pair::new("suite", &test.suite));
    }
    if let Some(scenario) = &test.scenario {
        labels.push(Pair::new("feature", &scenario.feature));
        labels.push(Pair::new("story", &scenario.name));
        for tag in &scenario.tags {
            labels.push(Pair::new("tag", tag));
        }
    } else if let Some((module, method)) = test.display.rsplit_once("::") {
        labels.push(Pair::new("package", module));
        labels.push(Pair::new("testClass", module));
        let inside_suite = module
            .strip_prefix(test.suite.as_str())
            .and_then(|rest| rest.strip_prefix("::"))
            .filter(|rest| !rest.is_empty());
        if let Some(sub_suite) = inside_suite.filter(|_| !test.suite.is_empty()) {
            labels.push(Pair::new("subSuite", sub_suite));
        }
        labels.push(Pair::new("testMethod", method));
    } else {
        labels.push(Pair::new("testMethod", &test.display));
    }
    labels.push(Pair::new("testId", &test.id));
    labels
}

fn parameters(finished: &FinishedTest<'_>) -> Vec<Pair> {
    let mut parameters = Vec::new();
    if let Some(browser) = finished.browser {
        parameters.push(Pair::new("browser", browser));
    }
    if let Some(driver) = finished.driver {
        parameters.push(Pair::new("driver", driver));
    }
    parameters
}

fn status_details(finished: &FinishedTest<'_>, status: AllureStatus) -> Option<StatusDetails> {
    if status == AllureStatus::Passed {
        return None;
    }
    let message = finished.result.message.lines().next().unwrap_or("").trim();
    let trace = finished
        .result
        .failure
        .as_ref()
        .and_then(|failure| failure.detail.clone());
    if message.is_empty() && trace.is_none() {
        return None;
    }
    Some(StatusDetails {
        message: message.to_owned(),
        trace,
    })
}

fn description(finished: &FinishedTest<'_>) -> Option<String> {
    let failure = finished.result.failure.as_ref()?;
    let mut parts = Vec::new();
    if let Some(url) = &failure.url {
        parts.push(format!("Last page: {url}"));
    }
    if let Some(hint) = &failure.hint {
        parts.push(format!("Hint: {hint}"));
    }
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

// ---------------------------------------------------------------------------
// The sink
// ---------------------------------------------------------------------------

/// One try of one test in one results folder.
struct Attempt {
    uuid: String,
    finished: bool,
}

/// The attempts of this process, by results folder and test id. Every sink on a
/// folder shares them, so a test is one result however many sinks hear of it.
static ATTEMPTS: Mutex<BTreeMap<(PathBuf, String), Attempt>> = Mutex::new(BTreeMap::new());

/// Writes the results of a run in the Allure format. See the [module](self)
/// documentation.
pub struct AllureSink {
    dir: PathBuf,
    now: Clock,
    new_id: IdSource,
}

impl fmt::Debug for AllureSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AllureSink")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl AllureSink {
    /// A sink that writes to `dir`, made if it is missing, with the system clock
    /// and random uuids. `environment.properties` is written there when it is
    /// not there yet.
    ///
    /// # Errors
    ///
    /// Returns an error if the folder or the environment file cannot be
    /// written.
    pub fn new(dir: impl Into<PathBuf>) -> io::Result<Self> {
        Self::with_sources(dir, Arc::new(now_ms), Arc::new(random_uuid))
    }

    /// Like [`new`](Self::new), reading the time from `now` and the uuids from
    /// `new_id`; tests use fixed ones to get the same files every time.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn with_sources(dir: impl Into<PathBuf>, now: Clock, new_id: IdSource) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        let dir = fs::canonicalize(&dir).unwrap_or(dir);
        write_environment(&dir)?;
        Ok(Self { dir, now, new_id })
    }

    /// The results folder.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The uuid of the attempt of test `id`. With `restart`, an attempt that
    /// ended is followed by a new one.
    fn attempt(&self, id: &str, restart: bool) -> String {
        let mut attempts = ATTEMPTS.lock().unwrap_or_else(PoisonError::into_inner);
        let key = (self.dir.clone(), id.to_owned());
        if let Some(attempt) = attempts.get_mut(&key) {
            if restart && attempt.finished {
                attempt.uuid = (self.new_id)();
                attempt.finished = false;
            }
            return attempt.uuid.clone();
        }
        let uuid = (self.new_id)();
        attempts.insert(
            key,
            Attempt {
                uuid: uuid.clone(),
                finished: false,
            },
        );
        uuid
    }

    fn mark_finished(&self, id: &str) {
        let mut attempts = ATTEMPTS.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(attempt) = attempts.get_mut(&(self.dir.clone(), id.to_owned())) {
            attempt.finished = true;
        }
    }

    /// Copies the evidence of the test into the results folder and returns the
    /// references to put in the result.
    fn attach_evidence(&self, uuid: &str, log_dir: &Path) -> Vec<AttachmentRef> {
        let mut attachments = Vec::new();
        for evidence in &EVIDENCE {
            let from = log_dir.join(evidence.file);
            if !from.is_file() {
                continue;
            }
            let extension = Path::new(evidence.name)
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or("bin");
            let source = format!("{uuid}-{}-attachment.{extension}", evidence.stem);
            match fs::copy(&from, self.dir.join(&source)) {
                Ok(_) => attachments.push(AttachmentRef {
                    name: evidence.name.to_owned(),
                    source,
                    mime: evidence.mime.to_owned(),
                }),
                Err(error) => {
                    tracing::warn!(%error, file = %from.display(), "an Allure attachment was not copied");
                }
            }
        }
        attachments
    }

    fn write(&self, finished: &FinishedTest<'_>) -> io::Result<()> {
        if finished.result.status == TestStatus::Untested {
            return Ok(());
        }
        let uuid = self.attempt(&finished.test.id, false);
        let status = AllureStatus::of(finished);
        let stop = (self.now)();
        let start = stop.saturating_sub(duration_ms(finished.result.duration_secs));
        let full_name = finished.test.display.clone();
        let identity = history_id(&full_name);
        let file = ResultFile {
            attachments: self.attach_evidence(&uuid, finished.log_dir),
            uuid: uuid.clone(),
            history_id: identity.clone(),
            test_case_id: identity,
            name: short_name(finished.test),
            full_name,
            status,
            status_details: status_details(finished, status),
            stage: "finished",
            description: description(finished),
            start,
            stop,
            labels: labels(finished.test),
            parameters: parameters(finished),
            steps: Vec::new(),
            links: Vec::new(),
        };
        let mut json = serde_json::to_vec_pretty(&file).map_err(io::Error::other)?;
        json.push(b'\n');
        write_atomically(&self.dir.join(format!("{uuid}-result.json")), &json)?;
        self.mark_finished(&finished.test.id);
        Ok(())
    }
}

impl RunSink for AllureSink {
    fn started(&self, test: &TestId) {
        self.attempt(&test.id, true);
    }

    fn finished(&self, finished: &FinishedTest<'_>) {
        if let Err(error) = self.write(finished) {
            tracing::warn!(
                %error,
                test = %finished.test.display,
                dir = %self.dir.display(),
                "the Allure result was not written"
            );
        }
    }
}

/// `value` as the text of a Java properties file.
fn property(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

/// Writes `environment.properties` unless the folder has one: a person may
/// keep their own, and every process of a run asks.
fn write_environment(dir: &Path) -> io::Result<()> {
    let path = dir.join(ENVIRONMENT_FILE);
    if path.exists() {
        return Ok(());
    }
    let text = format!(
        "framework={FRAMEWORK}\nversion={}\nos={}\narch={}\n",
        property(env!("CARGO_PKG_VERSION")),
        property(OS),
        property(ARCH)
    );
    write_atomically(&path, text.as_bytes())
}

/// Adds an [`AllureSink`] to `run` when the run's settings name a results
/// folder (`SB_ALLURE_DIR`), and returns it. A relative folder is under the
/// run's root. A folder that cannot be made is logged and the run goes on
/// without Allure.
///
/// [`TestRun::open`] calls this, so a run needs no other set-up.
pub fn attach(run: &TestRun) -> Option<Arc<AllureSink>> {
    let dir = run.settings().allure_dir.as_ref()?;
    let dir = if dir.is_absolute() {
        dir.clone()
    } else {
        run.root().join(dir)
    };
    match AllureSink::new(&dir) {
        Ok(sink) => {
            let sink = Arc::new(sink);
            run.add_sink(Arc::clone(&sink) as Arc<dyn RunSink>);
            Some(sink)
        }
        Err(error) => {
            tracing::warn!(%error, dir = %dir.display(), "the Allure results folder cannot be used");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;

    use serde_json::{json, Value};

    use super::*;
    use crate::core::report_helper::{FailureDetails, TestResult};

    const NOW: i64 = 1_700_000_000_000;
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake";

    /// Ids that count: `00000000-0000-4000-8000-000000000001`, `...02`, and so on.
    fn counting_ids() -> IdSource {
        let counter = AtomicU32::new(0);
        Arc::new(move || {
            let next = counter.fetch_add(1, Ordering::SeqCst) + 1;
            format!("00000000-0000-4000-8000-{next:012}")
        })
    }

    fn sink(dir: &Path) -> AllureSink {
        AllureSink::with_sources(dir, Arc::new(|| NOW), counting_ids()).unwrap()
    }

    fn rust_test(display: &str, id: &str) -> TestId {
        TestId {
            id: id.to_owned(),
            display: display.to_owned(),
            suite: display.split("::").next().unwrap_or_default().to_owned(),
            scenario: None,
        }
    }

    fn finished<'a>(
        test: &'a TestId,
        result: &'a TestResult,
        error: Option<&'a SeleniumBaseError>,
        log_dir: &'a Path,
    ) -> FinishedTest<'a> {
        FinishedTest {
            test,
            result,
            error,
            browser: None,
            driver: None,
            log_dir,
        }
    }

    /// Every result in `dir`, in the order of their names.
    fn results(dir: &Path) -> Vec<Value> {
        let mut files: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.to_string_lossy().ends_with("-result.json"))
            .collect();
        files.sort();
        files
            .iter()
            .map(|path| serde_json::from_slice(&fs::read(path).unwrap()).unwrap())
            .collect()
    }

    fn pairs(list: &[(&str, &str)]) -> Value {
        Value::Array(
            list.iter()
                .map(|(name, value)| json!({ "name": name, "value": value }))
                .collect(),
        )
    }

    #[test]
    fn a_passing_test_is_a_result_with_labels_and_no_status_details() {
        let (dir, logs) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let test = rust_test(
            "login_chrome::tests::test_login",
            "login_chrome.tests.test_login",
        );
        let result = TestResult::new(test.display.clone(), TestStatus::Passed, 1.5, "ok");
        let sink = sink(dir.path());
        sink.started(&test);
        sink.finished(&finished(&test, &result, None, logs.path()));

        let written = results(dir.path());
        assert_eq!(written.len(), 1);
        assert_eq!(
            written[0],
            json!({
                "uuid": "00000000-0000-4000-8000-000000000001",
                "historyId": "4c628872d1c96be8d17fb628d0f39648",
                "testCaseId": "4c628872d1c96be8d17fb628d0f39648",
                "fullName": "login_chrome::tests::test_login",
                "name": "test_login",
                "status": "passed",
                "stage": "finished",
                "start": 1_699_999_998_500_i64,
                "stop": 1_700_000_000_000_i64,
                "labels": pairs(&[
                    ("framework", "seleniumbase-rs"),
                    ("language", "rust"),
                    ("suite", "login_chrome"),
                    ("package", "login_chrome::tests"),
                    ("testClass", "login_chrome::tests"),
                    ("subSuite", "tests"),
                    ("testMethod", "test_login"),
                    ("testId", "login_chrome.tests.test_login"),
                ]),
                "parameters": [],
                "attachments": [],
                "steps": [],
                "links": [],
            })
        );
    }

    #[test]
    fn a_failed_test_carries_its_message_trace_page_and_three_attachments() {
        let (dir, logs) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        fs::write(logs.path().join(SCREENSHOT_FILE), PNG).unwrap();
        fs::write(logs.path().join(PAGE_SOURCE_FILE), "<html>Login</html>").unwrap();
        fs::write(
            logs.path().join(BASIC_INFO_FILE),
            "Last Page: https://example.com/login",
        )
        .unwrap();
        let test = rust_test("shop::checkout", "shop.checkout");
        let error = SeleniumBaseError::AssertionFailed("title".to_owned());
        let mut result = TestResult::new(
            test.display.clone(),
            TestStatus::Failed,
            0.25,
            "assertion failed: title\nsecond line",
        );
        result.failure = Some(FailureDetails {
            kind: "assertion_failed".to_owned(),
            hint: Some("check the title".to_owned()),
            url: Some("https://example.com/login".to_owned()),
            detail: Some("assertion failed: title\ncaused by: boom".to_owned()),
            ..FailureDetails::default()
        });
        let mut ended = finished(&test, &result, Some(&error), logs.path());
        ended.browser = Some("Chrome 155 / headless");
        ended.driver = Some("WebDriver");
        let sink = sink(dir.path());
        sink.started(&test);
        sink.finished(&ended);

        let written = results(dir.path());
        assert_eq!(
            written[0],
            json!({
                "uuid": "00000000-0000-4000-8000-000000000001",
                "historyId": "0c566b9619aa195832390c12f35a318f",
                "testCaseId": "0c566b9619aa195832390c12f35a318f",
                "fullName": "shop::checkout",
                "name": "checkout",
                "status": "failed",
                "statusDetails": {
                    "message": "assertion failed: title",
                    "trace": "assertion failed: title\ncaused by: boom",
                },
                "stage": "finished",
                "description": "Last page: https://example.com/login\n\nHint: check the title",
                "start": 1_699_999_999_750_i64,
                "stop": 1_700_000_000_000_i64,
                "labels": pairs(&[
                    ("framework", "seleniumbase-rs"),
                    ("language", "rust"),
                    ("suite", "shop"),
                    ("package", "shop"),
                    ("testClass", "shop"),
                    ("testMethod", "checkout"),
                    ("testId", "shop.checkout"),
                ]),
                "parameters": pairs(&[("browser", "Chrome 155 / headless"), ("driver", "WebDriver")]),
                "attachments": [
                    {
                        "name": "screenshot.png",
                        "source": "00000000-0000-4000-8000-000000000001-screenshot-attachment.png",
                        "type": "image/png",
                    },
                    {
                        "name": "page_source.html",
                        "source": "00000000-0000-4000-8000-000000000001-page_source-attachment.html",
                        "type": "text/html",
                    },
                    {
                        "name": "basic_test_info.txt",
                        "source": "00000000-0000-4000-8000-000000000001-basic_test_info-attachment.txt",
                        "type": "text/plain",
                    },
                ],
                "steps": [],
                "links": [],
            })
        );
        let copied = dir
            .path()
            .join("00000000-0000-4000-8000-000000000001-screenshot-attachment.png");
        assert_eq!(fs::read(copied).unwrap(), PNG);
    }

    #[test]
    fn a_skip_is_skipped_with_its_reason_and_no_trace() {
        let (dir, logs) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let test = rust_test("shop::coupon", "shop.coupon");
        let result = TestResult::new(
            test.display.clone(),
            TestStatus::Skipped,
            0.0,
            "no coupon today",
        );
        let sink = sink(dir.path());
        sink.finished(&finished(&test, &result, None, logs.path()));

        let written = results(dir.path());
        assert_eq!(written[0]["status"], "skipped");
        assert_eq!(
            written[0]["statusDetails"],
            json!({ "message": "no coupon today" })
        );
        assert_eq!(written[0]["start"], written[0]["stop"]);
    }

    #[test]
    fn a_scenario_gets_feature_story_and_tag_labels() {
        let (dir, logs) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let test = TestId::scenario(
            "features/login.feature",
            12,
            "Successful login",
            ["@smoke", "fast"],
        );
        let result = TestResult::new(test.display.clone(), TestStatus::Passed, 2.0, "ok");
        sink(dir.path()).finished(&finished(&test, &result, None, logs.path()));

        let written = &results(dir.path())[0];
        assert_eq!(written["name"], "Successful login");
        assert_eq!(
            written["fullName"],
            "features/login.feature:12 => Successful login"
        );
        assert_eq!(
            written["labels"],
            pairs(&[
                ("framework", "seleniumbase-rs"),
                ("language", "rust"),
                ("suite", "features/login.feature"),
                ("feature", "features/login.feature"),
                ("story", "Successful login"),
                ("tag", "smoke"),
                ("tag", "fast"),
                ("testId", test.id.as_str()),
            ])
        );
    }

    #[test]
    fn the_status_follows_the_error() {
        let logs = tempfile::tempdir().unwrap();
        let test = rust_test("t::x", "t.x");
        let status = |status: TestStatus, kind: Option<&str>, error: Option<&SeleniumBaseError>| {
            let mut result = TestResult::new("t::x", status, 0.1, "m");
            result.failure = kind.map(|kind| FailureDetails {
                kind: kind.to_owned(),
                ..FailureDetails::default()
            });
            AllureStatus::of(&finished(&test, &result, error, logs.path()))
        };
        let assertion = SeleniumBaseError::AssertionFailed("x".to_owned());
        let config = SeleniumBaseError::InvalidConfig("x".to_owned());

        assert_eq!(status(TestStatus::Passed, None, None), AllureStatus::Passed);
        assert_eq!(
            status(TestStatus::Skipped, None, None),
            AllureStatus::Skipped
        );
        assert_eq!(
            status(TestStatus::Unreported, None, None),
            AllureStatus::Broken
        );
        assert_eq!(
            status(TestStatus::Untested, None, None),
            AllureStatus::Broken
        );
        assert_eq!(
            status(TestStatus::Failed, None, Some(&assertion)),
            AllureStatus::Failed
        );
        assert_eq!(
            status(TestStatus::Failed, None, Some(&config)),
            AllureStatus::Broken
        );
        // The recorder saw a panic: there is no error, only the lifecycle's note.
        assert_eq!(
            status(TestStatus::Failed, Some("test_lifecycle"), None),
            AllureStatus::Failed
        );
        assert_eq!(
            status(TestStatus::Failed, Some("io"), None),
            AllureStatus::Broken
        );
        assert_eq!(
            status(TestStatus::Failed, Some("assertion_failed"), None),
            AllureStatus::Failed
        );
    }

    #[test]
    fn a_broken_test_is_written_as_broken_with_its_error() {
        let (dir, logs) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let test = rust_test("t::slow", "t.slow");
        let error = SeleniumBaseError::InvalidConfig("no such file".to_owned());
        let result = TestResult::new("t::slow", TestStatus::Failed, 0.1, error.to_string());
        sink(dir.path()).finished(&finished(&test, &result, Some(&error), logs.path()));

        let written = &results(dir.path())[0];
        assert_eq!(written["status"], "broken");
        assert_eq!(written["statusDetails"]["message"], error.to_string());
    }

    #[test]
    fn an_untested_test_writes_nothing() {
        let (dir, logs) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let test = rust_test("t::never", "t.never");
        let result = TestResult::new("t::never", TestStatus::Untested, 0.0, "");
        sink(dir.path()).finished(&finished(&test, &result, None, logs.path()));
        assert!(results(dir.path()).is_empty());
    }

    #[test]
    fn two_sinks_and_two_finishes_write_one_result_and_a_restart_writes_another() {
        let (dir, logs) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let test = rust_test("t::once", "t.once");
        let result = TestResult::new("t::once", TestStatus::Passed, 0.1, "ok");
        let (first, second) = (sink(dir.path()), sink(dir.path()));

        for sink in [&first, &second] {
            sink.started(&test);
        }
        for sink in [&first, &second, &first] {
            sink.finished(&finished(&test, &result, None, logs.path()));
        }
        assert_eq!(results(dir.path()).len(), 1, "one attempt is one result");

        // The test runs again: a retry has a result of its own.
        for sink in [&first, &second] {
            sink.started(&test);
            sink.finished(&finished(&test, &result, None, logs.path()));
        }
        let written = results(dir.path());
        assert_eq!(written.len(), 2);
        assert_eq!(written[0]["historyId"], written[1]["historyId"]);
        assert_ne!(written[0]["uuid"], written[1]["uuid"]);
    }

    #[test]
    fn the_environment_is_written_once_and_a_persons_own_file_is_kept() {
        let (fresh, kept) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let _ = sink(fresh.path());
        let text = fs::read_to_string(fresh.path().join(ENVIRONMENT_FILE)).unwrap();
        assert!(text.starts_with("framework=seleniumbase-rs\nversion="));
        assert!(text.contains(&format!("\nos={OS}\n")));

        fs::write(kept.path().join(ENVIRONMENT_FILE), "team=qa\n").unwrap();
        let _ = sink(kept.path());
        assert_eq!(
            fs::read_to_string(kept.path().join(ENVIRONMENT_FILE)).unwrap(),
            "team=qa\n"
        );
    }

    #[test]
    fn a_random_uuid_is_a_version_4_uuid_and_a_new_one_each_time() {
        let uuid = random_uuid();
        let bytes = uuid.as_bytes();
        assert_eq!(uuid.len(), 36);
        assert!([8, 13, 18, 23].iter().all(|&at| bytes[at] == b'-'));
        assert_eq!(bytes[14], b'4');
        assert!(matches!(bytes[19], b'8' | b'9' | b'a' | b'b'));
        assert_ne!(uuid, random_uuid());
    }

    #[test]
    fn properties_escape_what_a_properties_file_reads_specially() {
        assert_eq!(property("a\\b\nc"), "a\\\\b\\nc");
    }
}
