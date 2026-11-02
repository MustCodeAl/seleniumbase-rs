//! A trace of a tab: its actions, pictures, console and network in a folder,
//! with a viewer to open in a browser.
//!
//! [`Page::start_trace`] starts a [`TraceRecorder`]. While it runs, each action
//! a [`Locator`](super::Locator) performs, and each [`Page::goto`], is noted with
//! its name, selector, time and result; the page is photographed after it and
//! a copy of its DOM is kept; the console and the network are collected on the
//! side. [`TraceRecorder::stop`] writes the folder:
//!
//! ```text
//! <dir>/
//!   index.html          the viewer: a single file, nothing is loaded from outside
//!   trace.json          version, start time, duration, actions, console, limits
//!   network.har         the HAR, without bodies; request bodies and credentials removed
//!   screenshots/0001.jpg
//!   snapshots/0001.html
//! ```
//!
//! The engine never switches on the protocol domains a page can feel, so the
//! console is read by a small script in the page (`trace/capture.js`) and the
//! DOM snapshot is built by the same script; the network comes from the HAR
//! recorder. With no trace running, an action costs one look into an empty
//! map and sends nothing.
//!
//! Everything that comes from a page or a caller is untrusted, and the trace is
//! opened in a browser; see the `viewer` module for how the file is made safe.
//! The text given to `fill`, `type_text`, `send_keys` and `set_value` is never
//! written. A trace folder still holds what the page showed (screenshots, DOM,
//! URLs, cookies), so it is sensitive.

mod viewer;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;

use super::har::{Har, HarOptions, HarRecorder};
use super::page::Page;
use super::sync::locked;
use crate::error::SeleniumBaseError;

/// The page half of a trace: wraps the console calls and builds the snapshot.
const CAPTURE_JS: &str = include_str!("trace/capture.js");

/// The start of the expression that hands over what the page collected.
const TAKE_PREFIX: &str = "(window.__sbtrace ? window.__sbtrace.take(";

/// Hands over the last console messages and puts the page back as it was.
const RELEASE_EXPRESSION: &str = "(function () { var t = window.__sbtrace; if (!t) { return null; } var r = t.take(false); t.release(); return r; })()";

/// What stands in the trace for the text a caller typed.
const MASKED_TEXT: &str = "[text not recorded]";

/// What stands in the HAR for the body of a request.
const MASKED_BODY: &str = "[request body not recorded]";

/// What stands in the HAR for the value of a credential header.
const REMOVED: &str = "[removed]";

/// The most bytes of a DOM snapshot that are kept.
const MAX_SNAPSHOT_BYTES: usize = 512 * 1024;

/// The largest screenshot, in bytes, that is kept.
const MAX_SCREENSHOT_BYTES: usize = 4 * 1024 * 1024;

/// The most characters of a console text, selector or error that are kept.
const MAX_TEXT_CHARS: usize = 2000;

/// The most characters of a URL that are kept.
const MAX_URL_CHARS: usize = 2048;

/// The JPEG quality of the screenshots: they are for reading, not for pixels.
const SCREENSHOT_QUALITY: u8 = 60;

/// The hard maximums that clamp [`TraceOptions`].
const HARD_MAX_ACTIONS: usize = 10_000;
const HARD_MAX_BYTES: usize = 1 << 30;
const HARD_MAX_CONSOLE: usize = 20_000;
const HARD_MAX_REQUESTS: usize = 20_000;

/// What a trace may record before it stops recording that kind of thing.
///
/// Every value is clamped to a hard maximum (10 000 actions, 1 GiB of pictures,
/// 20 000 console messages, 20 000 requests), so a long run cannot fill the
/// disk. Past a limit, actions are still noted, only the thing that hit the
/// limit is left out, and the viewer says so.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::TraceOptions;
///
/// let options = TraceOptions::new().max_actions(200).max_bytes(16 << 20);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "each field is named for the builder method that sets it"
)]
pub struct TraceOptions {
    max_actions: usize,
    max_bytes: usize,
    max_console_messages: usize,
    max_requests: usize,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            max_actions: 1000,
            max_bytes: 64 << 20,
            max_console_messages: 2000,
            max_requests: 2000,
        }
    }
}

impl TraceOptions {
    /// The default limits: 1000 actions, 64 MiB of screenshots and snapshots, 2000
    /// console messages and 2000 requests.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The most actions noted. Later actions run as usual and are not noted.
    #[must_use]
    pub fn max_actions(mut self, count: usize) -> Self {
        self.max_actions = count.min(HARD_MAX_ACTIONS);
        self
    }

    /// The most bytes of screenshots and snapshots together. Once it is reached,
    /// actions are still noted but without pictures. The viewer file repeats
    /// the pictures, so the folder takes about twice this.
    #[must_use]
    pub fn max_bytes(mut self, bytes: usize) -> Self {
        self.max_bytes = bytes.min(HARD_MAX_BYTES);
        self
    }

    /// The most console messages kept.
    #[must_use]
    pub fn max_console_messages(mut self, count: usize) -> Self {
        self.max_console_messages = count.min(HARD_MAX_CONSOLE);
        self
    }

    /// The most requests kept in `network.har` and in the viewer.
    #[must_use]
    pub fn max_requests(mut self, count: usize) -> Self {
        self.max_requests = count.min(HARD_MAX_REQUESTS);
        self
    }
}

/// What a finished trace wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Trace {
    /// The folder.
    pub dir: PathBuf,
    /// The viewer, `index.html` in the folder: open it in a browser.
    pub viewer: PathBuf,
    /// How many actions were noted.
    pub actions: usize,
    /// How many console messages were kept.
    pub console_messages: usize,
    /// How many requests were kept.
    pub requests: usize,
    /// The bytes of screenshots and snapshots written.
    pub picture_bytes: usize,
    /// How long the trace ran.
    pub duration: Duration,
}

/// A trace in progress; see [`Page::start_trace`].
///
/// [`stop`](Self::stop) ends it and writes the folder. Dropping the recorder
/// ends the trace without writing anything beyond the pictures already saved.
#[derive(Debug)]
pub struct TraceRecorder {
    page: Page,
    state: Arc<TraceState>,
    har: Option<HarRecorder>,
    script_id: Option<String>,
}

/// One action of the trace, as `trace.json` has it.
#[derive(Debug, Clone, Serialize)]
struct ActionRecord {
    id: usize,
    name: &'static str,
    selector: String,
    detail: Option<String>,
    start_ms: u64,
    end_ms: u64,
    ok: bool,
    error: Option<String>,
    url: String,
    title: String,
    screenshot: Option<String>,
    snapshot: Option<String>,
    snapshot_truncated: bool,
}

/// One console message of the trace.
#[derive(Debug, Clone, Serialize)]
struct ConsoleRecord {
    at_ms: u64,
    level: &'static str,
    text: String,
    action: usize,
}

/// One request, as the viewer lists it.
#[derive(Debug, Clone, Serialize)]
struct NetworkRecord {
    started_ms: u64,
    method: String,
    status: u16,
    url: String,
    time_ms: f64,
    #[serde(rename = "type")]
    kind: String,
}

/// What was left out of the trace, and why.
#[derive(Debug, Clone, Default, Serialize)]
struct Limits {
    actions_skipped: usize,
    pictures_skipped: usize,
    console_dropped: usize,
    requests_dropped: usize,
    capture_failures: usize,
    network_error: Option<String>,
}

/// The shape of `trace.json`.
#[derive(Serialize)]
struct TraceFile<'a> {
    version: u32,
    started_at: String,
    duration_ms: u64,
    actions: &'a [ActionRecord],
    console: &'a [ConsoleRecord],
    network: &'a [NetworkRecord],
    limits: &'a Limits,
}

/// Everything noted so far.
#[derive(Debug, Default)]
struct Record {
    actions: Vec<ActionRecord>,
    console: Vec<ConsoleRecord>,
    picture_bytes: usize,
    limits: Limits,
}

/// A trace that is running: what a tab's actions look up to know whether to
/// note themselves.
#[derive(Debug)]
pub(super) struct TraceState {
    dir: PathBuf,
    options: TraceOptions,
    started: Instant,
    started_wall_ms: i64,
    next_id: AtomicUsize,
    record: Mutex<Record>,
}

/// What the page script hands over. Everything in it is untrusted: a page can
/// replace the script, so every field has a default and is checked again here.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PageData {
    url: String,
    title: String,
    html: Option<String>,
    console: Vec<RawMessage>,
    dropped: f64,
}

/// A console message as the page script recorded it.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawMessage {
    at: f64,
    level: String,
    text: String,
}

/// The detail of an action that has none.
pub(super) fn no_detail() -> Option<String> {
    None
}

/// The detail of an action that takes text: the text is never recorded.
#[expect(
    clippy::unnecessary_wraps,
    reason = "it is passed where a detail closure, which may return nothing, is expected"
)]
pub(super) fn masked() -> Option<String> {
    Some(MASKED_TEXT.to_owned())
}

/// `url` without a user name or password, and cut to a bounded length.
pub(super) fn plain_url(url: &str) -> String {
    let parsed = url::Url::parse(url).or_else(|error| {
        // An address without a scheme is opened over HTTPS, as `goto` does.
        if url.contains("://") {
            Err(error)
        } else {
            url::Url::parse(&format!("https://{url}"))
        }
    });
    match parsed {
        Ok(mut parsed) => {
            // Refused only for a URL that cannot hold credentials anyway.
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
            cut(parsed.as_str(), MAX_URL_CHARS)
        }
        Err(_) => "[address not recorded]".to_owned(),
    }
}

/// `text`, cut to `max` characters with `...` marking the cut.
fn cut(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => format!("{}...", &text[..end]),
        None => text.to_owned(),
    }
}

/// `text`, cut to at most `max` bytes at a character boundary, and whether it
/// was cut.
fn cut_bytes(mut text: String, max: usize) -> (String, bool) {
    if text.len() <= max {
        return (text, false);
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    (text, true)
}

/// A time in milliseconds as a whole number; out-of-range values saturate.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a saturating conversion of a time is what is wanted"
)]
fn whole(value: f64) -> i64 {
    value.round() as i64
}

/// The console level, or `log` for anything a page made up.
fn level_name(level: &str) -> &'static str {
    match level {
        "debug" => "debug",
        "info" => "info",
        "warn" => "warn",
        "error" => "error",
        _ => "log",
    }
}

/// The expression that hands over what the page collected.
fn capture_expression(snapshot: bool) -> String {
    format!("{TAKE_PREFIX}{snapshot}) : null)")
}

/// Evaluates `expression` in the tab's page and reads what it returns as the
/// page script's answer; `None` when the script is not there or answered
/// something else.
async fn read_page(page: &Page, expression: &str) -> Option<PageData> {
    let reply = page
        .execute(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true }),
        )
        .await
        .ok()?;
    if reply.get("exceptionDetails").is_some() {
        return None;
    }
    serde_json::from_value(reply.pointer("/result/value")?.clone()).ok()
}

/// A screenshot of the tab as JPEG bytes, if one fits the per-item bound.
async fn take_screenshot(page: &Page) -> Option<Vec<u8>> {
    let reply = page
        .execute(
            "Page.captureScreenshot",
            json!({ "format": "jpeg", "quality": SCREENSHOT_QUALITY }),
        )
        .await
        .ok()?;
    let data = reply.get("data")?.as_str()?;
    if data.len() > MAX_SCREENSHOT_BYTES / 3 * 4 + 4 {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;
    (bytes.len() <= MAX_SCREENSHOT_BYTES).then_some(bytes)
}

/// Creates the file `path` and writes `bytes` to it. It never replaces a file
/// and never follows a link that is already there.
async fn write_new(path: &Path, bytes: &[u8]) -> Result<(), SeleniumBaseError> {
    let fail = |error: std::io::Error| {
        SeleniumBaseError::Io(std::io::Error::new(
            error.kind(),
            format!("cannot write the trace file {}: {error}", path.display()),
        ))
    };
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await
        .map_err(fail)?;
    file.write_all(bytes).await.map_err(fail)?;
    file.flush().await.map_err(fail)
}

/// The error for a trace folder that is refused.
fn refuse(path: &Path, why: &str) -> SeleniumBaseError {
    SeleniumBaseError::invalid_config(format!("cannot trace into {}: {why}", path.display()))
}

/// An I/O failure while preparing the trace folder.
fn folder_error(path: &Path, error: &std::io::Error) -> SeleniumBaseError {
    SeleniumBaseError::Io(std::io::Error::new(
        error.kind(),
        format!(
            "cannot prepare the trace folder {}: {error}",
            path.display()
        ),
    ))
}

/// Makes `path` ready to hold a trace and returns it.
///
/// It must be a folder the caller names: either absent, with a parent that
/// exists (it is created), or an existing real folder, not a symbolic link,
/// that is empty. Anything else is refused, so a trace never writes into a
/// folder that holds other files or through a link.
fn prepare_dir(path: &Path) -> Result<PathBuf, SeleniumBaseError> {
    if path.as_os_str().is_empty() {
        return Err(refuse(path, "no folder is named"));
    }
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(refuse(path, "it is a symbolic link"));
        }
        Ok(meta) if !meta.is_dir() => {
            return Err(refuse(path, "it exists and is not a folder"));
        }
        Ok(_) => {
            let mut entries =
                std::fs::read_dir(path).map_err(|error| folder_error(path, &error))?;
            if entries.next().is_some() {
                return Err(refuse(path, "the folder is not empty"));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = match path.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => parent,
                _ => Path::new("."),
            };
            if !parent.is_dir() {
                return Err(refuse(path, "its parent folder does not exist"));
            }
            std::fs::create_dir(path).map_err(|error| folder_error(path, &error))?;
        }
        Err(error) => return Err(folder_error(path, &error)),
    }
    for sub in ["screenshots", "snapshots"] {
        std::fs::create_dir(path.join(sub)).map_err(|error| folder_error(path, &error))?;
    }
    Ok(path.to_path_buf())
}

impl TraceState {
    fn new(dir: PathBuf, options: TraceOptions) -> Self {
        Self {
            dir,
            options,
            started: Instant::now(),
            started_wall_ms: jiff::Timestamp::now().as_millisecond(),
            next_id: AtomicUsize::new(0),
            record: Mutex::new(Record::default()),
        }
    }

    /// Milliseconds since the trace started.
    fn now_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// The id of the next action, or `None` when the action limit is reached.
    fn begin(&self) -> Option<usize> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        if id > self.options.max_actions {
            locked(&self.record).limits.actions_skipped += 1;
            return None;
        }
        Some(id)
    }

    /// Whether another picture may still be taken.
    fn has_room(&self) -> bool {
        locked(&self.record).picture_bytes < self.options.max_bytes
    }

    /// Keeps what the page logged, up to the console limit.
    fn add_console(&self, messages: Vec<RawMessage>, dropped: f64, action: usize) {
        let mut record = locked(&self.record);
        record.limits.console_dropped += usize::try_from(whole(dropped)).unwrap_or(0);
        for message in messages {
            if record.console.len() >= self.options.max_console_messages {
                record.limits.console_dropped += 1;
                continue;
            }
            let at = whole(message.at) - self.started_wall_ms;
            record.console.push(ConsoleRecord {
                at_ms: u64::try_from(at).unwrap_or(0),
                level: level_name(&message.level),
                text: cut(&message.text, MAX_TEXT_CHARS),
                action,
            });
        }
    }

    /// Notes that the network log could not be kept.
    fn note_network_error(&self, error: &SeleniumBaseError) {
        locked(&self.record).limits.network_error = Some(cut(&error.to_string(), MAX_TEXT_CHARS));
    }

    /// Completes `action` with what the page shows now, and notes it.
    ///
    /// A failure here is counted and swallowed: it never changes the result of
    /// the action it describes.
    async fn finish(&self, page: &Page, mut action: ActionRecord) {
        let room = self.has_room();
        let data = read_page(page, &capture_expression(room)).await;
        let shot = if room {
            take_screenshot(page).await
        } else {
            None
        };
        let mut failures = usize::from(data.is_none()) + usize::from(room && shot.is_none());
        let mut html = None;
        if let Some(data) = data {
            action.url = plain_url(&data.url);
            action.title = cut(&data.title, MAX_TEXT_CHARS);
            html = data
                .html
                .filter(|text| !text.is_empty())
                .map(|text| cut_bytes(text, MAX_SNAPSHOT_BYTES));
            self.add_console(data.console, data.dropped, action.id);
        }
        let bytes =
            shot.as_ref().map_or(0, Vec::len) + html.as_ref().map_or(0, |(text, _)| text.len());
        let keep = {
            let mut record = locked(&self.record);
            if !room || record.picture_bytes + bytes > self.options.max_bytes {
                record.limits.pictures_skipped += 1;
                false
            } else {
                record.picture_bytes += bytes;
                true
            }
        };
        if keep {
            if let Some(bytes) = &shot {
                let name = format!("screenshots/{:04}.jpg", action.id);
                if write_new(&self.dir.join(&name), bytes).await.is_ok() {
                    action.screenshot = Some(name);
                } else {
                    failures += 1;
                }
            }
            if let Some((text, truncated)) = &html {
                let name = format!("snapshots/{:04}.html", action.id);
                if write_new(&self.dir.join(&name), text.as_bytes())
                    .await
                    .is_ok()
                {
                    action.snapshot = Some(name);
                    action.snapshot_truncated = *truncated;
                } else {
                    failures += 1;
                }
            }
        }
        let mut record = locked(&self.record);
        record.limits.capture_failures += failures;
        record.actions.push(action);
    }

    /// Removes request bodies and credentials from `har`, keeps at most the
    /// request limit, and lists what is left for the viewer.
    fn masked_network(&self, har: &mut Har) -> Vec<NetworkRecord> {
        let total = har.log.entries.len();
        if total > self.options.max_requests {
            har.log.entries.truncate(self.options.max_requests);
            locked(&self.record).limits.requests_dropped = total - self.options.max_requests;
        }
        for entry in &mut har.log.entries {
            if let Some(body) = &mut entry.request.post_data {
                MASKED_BODY.clone_into(&mut body.text);
            }
            for header in &mut entry.request.headers {
                if header.name.eq_ignore_ascii_case("authorization")
                    || header.name.eq_ignore_ascii_case("proxy-authorization")
                {
                    REMOVED.clone_into(&mut header.value);
                }
            }
        }
        har.log
            .entries
            .iter()
            .map(|entry| {
                let started = entry
                    .started_date_time
                    .parse::<jiff::Timestamp>()
                    .map_or(0, |time| time.as_millisecond() - self.started_wall_ms);
                NetworkRecord {
                    started_ms: u64::try_from(started).unwrap_or(0),
                    method: cut(&entry.request.method, 16),
                    status: entry.response.status,
                    url: cut(&entry.request.url, MAX_URL_CHARS),
                    time_ms: entry.time,
                    kind: cut(entry.resource_type.as_deref().unwrap_or_default(), 32),
                }
            })
            .collect()
    }

    /// The data of the viewer: `trace.json`'s, with each picture's content in
    /// place of its file name.
    async fn viewer_data(&self, file: &TraceFile<'_>) -> Result<Value, SeleniumBaseError> {
        let mut data = serde_json::to_value(file)?;
        if let Some(items) = data.get_mut("actions").and_then(Value::as_array_mut) {
            for (item, action) in items.iter_mut().zip(file.actions) {
                if let Some(name) = &action.screenshot {
                    if let Ok(bytes) = tokio::fs::read(self.dir.join(name)).await {
                        item["screenshot"] =
                            Value::String(base64::engine::general_purpose::STANDARD.encode(bytes));
                    }
                }
                if let Some(name) = &action.snapshot {
                    if let Ok(bytes) = tokio::fs::read(self.dir.join(name)).await {
                        item["snapshot"] =
                            Value::String(String::from_utf8_lossy(&bytes).into_owned());
                    }
                }
            }
        }
        Ok(data)
    }

    /// Writes `network.har`, `trace.json` and `index.html`.
    async fn write_out(&self, har: Option<Har>) -> Result<Trace, SeleniumBaseError> {
        let mut network = Vec::new();
        if let Some(mut har) = har {
            network = self.masked_network(&mut har);
            write_new(&self.dir.join("network.har"), har.to_json()?.as_bytes()).await?;
        }
        let duration_ms = self.now_ms();
        let (actions, console, limits, picture_bytes) = {
            let record = locked(&self.record);
            let mut actions = record.actions.clone();
            actions.sort_by_key(|action| action.id);
            let mut console = record.console.clone();
            console.sort_by_key(|message| message.at_ms);
            (
                actions,
                console,
                record.limits.clone(),
                record.picture_bytes,
            )
        };
        let file = TraceFile {
            version: 1,
            started_at: jiff::Timestamp::from_millisecond(self.started_wall_ms)
                .map(|time| time.to_string())
                .unwrap_or_default(),
            duration_ms,
            actions: &actions,
            console: &console,
            network: &network,
            limits: &limits,
        };
        write_new(
            &self.dir.join("trace.json"),
            &serde_json::to_vec_pretty(&file)?,
        )
        .await?;
        let html = viewer::render(&self.viewer_data(&file).await?)?;
        let viewer = self.dir.join("index.html");
        write_new(&viewer, html.as_bytes()).await?;
        Ok(Trace {
            dir: self.dir.clone(),
            viewer,
            actions: actions.len(),
            console_messages: console.len(),
            requests: network.len(),
            picture_bytes,
            duration: Duration::from_millis(duration_ms),
        })
    }
}

impl Page {
    /// The trace running on this tab, if any.
    fn running_trace(&self) -> Option<Arc<TraceState>> {
        locked(&self.browser().inner().traces)
            .get(self.id())
            .cloned()
    }

    /// Runs `body` as one action of the tab's trace.
    ///
    /// With no trace running it awaits `body` and does nothing else: it sends no
    /// command, builds none of the text and cannot change the result. With one
    /// it notes the start, awaits `body`, notes the end and the result, then
    /// photographs the page; a failure of that is swallowed.
    pub(super) async fn traced<T, F>(
        &self,
        name: &'static str,
        selector: impl FnOnce() -> String,
        detail: impl FnOnce() -> Option<String>,
        body: F,
    ) -> Result<T, SeleniumBaseError>
    where
        F: Future<Output = Result<T, SeleniumBaseError>>,
    {
        let Some(state) = self.running_trace() else {
            return body.await;
        };
        let Some(id) = state.begin() else {
            return body.await;
        };
        let start_ms = state.now_ms();
        let result = body.await;
        let action = ActionRecord {
            id,
            name,
            selector: cut(&selector(), MAX_TEXT_CHARS),
            detail: detail().map(|text| cut(&text, MAX_TEXT_CHARS)),
            start_ms,
            end_ms: state.now_ms(),
            ok: result.is_ok(),
            error: result
                .as_ref()
                .err()
                .map(|error| cut(&error.to_string(), MAX_TEXT_CHARS)),
            url: String::new(),
            title: String::new(),
            screenshot: None,
            snapshot: None,
            snapshot_truncated: false,
        };
        state.finish(self, action).await;
        result
    }

    /// Starts a trace of this tab into the folder `dir`, with the default
    /// limits. Same as [`start_trace_with`](Self::start_trace_with) with
    /// [`TraceOptions::default`].
    ///
    /// # Errors
    ///
    /// See [`start_trace_with`](Self::start_trace_with).
    pub async fn start_trace(
        &self,
        dir: impl AsRef<Path>,
    ) -> Result<TraceRecorder, SeleniumBaseError> {
        self.start_trace_with(dir, TraceOptions::default()).await
    }

    /// Starts a trace of this tab into the folder `dir`.
    ///
    /// Only what happens after this returns is recorded. Each action of a
    /// [`Locator`](super::Locator) of this tab (`click`, `fill`, `press`, ...)
    /// and each [`goto`](Self::goto) is noted with its name, selector, time and
    /// result, followed by a screenshot and a DOM snapshot. The page's console
    /// messages and its network traffic are kept beside them.
    /// [`TraceRecorder::stop`] writes the folder; open `index.html` in it in a
    /// browser to step through the run.
    ///
    /// The tab gets a small script (`window.__sbtrace`) that wraps the console
    /// methods, so the page can see a trace is running; use it for debugging,
    /// not for a run that must look untouched. Messages of iframes, and of a page
    /// in the instant before it navigates away, can be missing.
    ///
    /// # What is recorded, and what is not
    ///
    /// The text given to `fill`, `type_text`, `send_keys` and `set_value` is
    /// never written, whatever the field. Request bodies and the
    /// `Authorization` and `Proxy-Authorization` header values are removed from
    /// the HAR, and a `goto` address loses its user name and password.
    /// Everything else is kept as it is: screenshots show what the page shows
    /// (a value typed into a plain field, personal data), the HAR holds cookies
    /// and full URLs with their query strings, and a snapshot has the page's
    /// text. **A trace folder is sensitive**: keep it out of version control and
    /// out of public CI artifacts.
    ///
    /// # Safety of the viewer
    ///
    /// The viewer is one HTML file that loads nothing from outside. What came
    /// from the page is shown as text, never as markup, and a DOM snapshot is
    /// shown only in a sandboxed frame with no scripts.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if `dir` is not a folder the
    /// trace may use (it must be absent, in a parent that exists, or an existing
    /// empty folder that is not a symbolic link) or if a trace already runs on
    /// this tab, and [`SeleniumBaseError::CdpDriver`] if the browser refuses
    /// the page script.
    pub async fn start_trace_with(
        &self,
        dir: impl AsRef<Path>,
        options: TraceOptions,
    ) -> Result<TraceRecorder, SeleniumBaseError> {
        if self.running_trace().is_some() {
            return Err(SeleniumBaseError::invalid_config(
                "a trace is already running on this tab",
            ));
        }
        let dir = prepare_dir(dir.as_ref())?;
        let installed = self
            .execute(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": CAPTURE_JS }),
            )
            .await?;
        let script_id = installed
            .get("identifier")
            .and_then(Value::as_str)
            .map(str::to_owned);
        // The document that is open now; later ones get the script above.
        let _ = self
            .execute("Runtime.evaluate", json!({ "expression": CAPTURE_JS }))
            .await;
        let state = Arc::new(TraceState::new(dir, options));
        let har = match self.start_har_with(HarOptions::new()).await {
            Ok(har) => Some(har),
            Err(error) => {
                state.note_network_error(&error);
                None
            }
        };
        locked(&self.browser().inner().traces).insert(self.id().to_owned(), Arc::clone(&state));
        Ok(TraceRecorder {
            page: self.clone(),
            state,
            har,
            script_id,
        })
    }
}

impl TraceRecorder {
    /// The folder the trace is written to.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.state.dir
    }

    /// Stops the trace and writes `index.html`, `trace.json` and `network.har`
    /// beside the pictures already saved.
    ///
    /// The page gets its console methods back and loses `window.__sbtrace`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if a file cannot be written (the
    /// folder, or a file in it, was changed meanwhile) and
    /// [`SeleniumBaseError::Json`] if the data cannot be serialised. A failure
    /// of the network log alone is not an error: the viewer says it is missing.
    pub async fn stop(mut self) -> Result<Trace, SeleniumBaseError> {
        self.unregister();
        let state = Arc::clone(&self.state);
        if let Some(last) = read_page(&self.page, RELEASE_EXPRESSION).await {
            state.add_console(last.console, last.dropped, 0);
        }
        if let Some(identifier) = self.script_id.take() {
            let _ = self
                .page
                .execute(
                    "Page.removeScriptToEvaluateOnNewDocument",
                    json!({ "identifier": identifier }),
                )
                .await;
        }
        let har = match self.har.take() {
            Some(recorder) => match recorder.finish().await {
                Ok(har) => Some(har),
                Err(error) => {
                    state.note_network_error(&error);
                    None
                }
            },
            None => None,
        };
        state.write_out(har).await
    }

    /// Takes this trace out of the tab's list, so later actions note nothing.
    fn unregister(&self) {
        let mut traces = locked(&self.page.browser().inner().traces);
        if traces
            .get(self.page.id())
            .is_some_and(|running| Arc::ptr_eq(running, &self.state))
        {
            traces.remove(self.page.id());
        }
    }
}

impl Drop for TraceRecorder {
    fn drop(&mut self) {
        self.unregister();
        if let Some(identifier) = self.script_id.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                let page = self.page.clone();
                runtime.spawn(async move {
                    let _ = page
                        .execute(
                            "Page.removeScriptToEvaluateOnNewDocument",
                            json!({ "identifier": identifier }),
                        )
                        .await;
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sb_cdp::{Browser, MockCtrl};

    const SECRET: &str = "hunter2-secret-value";

    /// A `Runtime.evaluate` answer holding `v`.
    fn value(v: Value) -> Value {
        json!({ "result": { "value": v } })
    }

    /// The `Runtime.evaluate` answer for `expression` on a page with one visible
    /// element, as the locator's page helper gives it.
    fn one_visible_element(expression: &str) -> Value {
        if expression.contains("__sbcdp.center(") {
            value(json!({ "x": 120.0, "y": 80.0, "covered": false }))
        } else if expression.contains("window.innerWidth") {
            value(json!([1000.0, 800.0]))
        } else if expression.contains("__sbcdp.info(") {
            value(json!({
                "tag": "button", "text": "", "html": "", "attributes": {}, "visible": true,
                "rect": { "x": 70.0, "y": 60.0, "width": 100.0, "height": 40.0 },
            }))
        } else if expression.contains(".some(") || expression.contains(".length > 0") {
            value(json!(true))
        } else if expression.contains("resolve(") && expression.contains(".length") {
            value(json!(1))
        } else {
            value(Value::Null)
        }
    }

    /// Scripts a page whose script hands over `console` and `title` once, whose
    /// screenshot is a few bytes, and, when `element`, which has a visible
    /// element every selector finds.
    fn script(mock: &MockCtrl, console: Value, title: &str, element: bool) {
        let title = title.to_owned();
        let mut console = Some(console);
        mock.on("Runtime.evaluate", move |params| {
            let expression = params["expression"].as_str().unwrap_or_default();
            if expression.starts_with(TAKE_PREFIX) {
                return Ok(value(json!({
                    "url": "https://user:pw@shop.test/cart?token=abc",
                    "title": title,
                    "html": "<!doctype html><html><body><p>Hello</p></body></html>",
                    "console": console.take().unwrap_or_else(|| json!([])),
                    "dropped": 0,
                })));
            }
            if expression == RELEASE_EXPRESSION {
                return Ok(value(json!({ "console": [], "dropped": 0 })));
            }
            Ok(if element {
                one_visible_element(expression)
            } else {
                value(Value::Null)
            })
        });
        mock.reply(
            "Page.captureScreenshot",
            json!({ "data": base64::engine::general_purpose::STANDARD.encode(b"\xff\xd8jpeg\xff\xd9") }),
        );
        mock.reply(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "identifier": "42" }),
        );
    }

    /// A tab of a mocked browser that waits only briefly for elements.
    async fn tab() -> (Page, MockCtrl) {
        let (browser, mock) = Browser::new_mocked();
        let page = browser
            .default_page()
            .await
            .unwrap()
            .with_timeout(Duration::from_millis(150));
        (page, mock)
    }

    /// A page ready to be traced, and the folder (absent) to trace into.
    async fn traced_tab(console: Value) -> (Page, MockCtrl, tempfile::TempDir, PathBuf) {
        let (page, mock) = tab().await;
        script(&mock, console, "Cart", true);
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("trace");
        (page, mock, temp, dir)
    }

    fn json_file(dir: &Path, name: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap()
    }

    /// Every file under `dir`, read as lossy text.
    fn all_text(dir: &Path) -> String {
        let mut text = String::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(path) = stack.pop() {
            for entry in std::fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    text.push_str(&String::from_utf8_lossy(&std::fs::read(&path).unwrap()));
                }
            }
        }
        text
    }

    fn count(text: &str, needle: &str) -> usize {
        text.to_ascii_lowercase().matches(needle).count()
    }

    #[tokio::test]
    async fn a_click_and_a_fill_are_noted_in_order_with_their_pictures() {
        let (page, mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page.start_trace(&dir).await.unwrap();

        page.locator("#go").click().await.unwrap();
        page.locator("#name").fill(SECRET).await.unwrap();
        let trace = recorder.stop().await.unwrap();

        let file = json_file(&dir, "trace.json");
        assert_eq!(file["version"], 1);
        let actions = file["actions"].as_array().unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0]["name"], "click");
        assert_eq!(actions[0]["selector"], "#go");
        assert_eq!(actions[0]["ok"], true);
        assert_eq!(actions[1]["name"], "fill");
        assert_eq!(actions[1]["detail"], MASKED_TEXT);
        assert_eq!(actions[0]["screenshot"], "screenshots/0001.jpg");
        assert_eq!(actions[1]["snapshot"], "snapshots/0002.html");
        assert_eq!(actions[1]["title"], "Cart");
        // The credentials of the page's own address are not in the note.
        assert_eq!(actions[0]["url"], "https://shop.test/cart?token=abc");
        assert!(dir.join("screenshots/0002.jpg").is_file());
        assert!(dir.join("index.html").is_file());
        assert!(dir.join("network.har").is_file());
        assert_eq!(trace.actions, 2);
        assert_eq!(trace.viewer, dir.join("index.html"));
        assert_eq!(mock.calls_to("Page.captureScreenshot").len(), 2);
    }

    #[tokio::test]
    async fn a_failing_action_is_noted_with_its_error_and_still_fails() {
        let (page, mock) = tab().await;
        script(&mock, json!([]), "Cart", false);
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("trace");
        let recorder = page.start_trace(&dir).await.unwrap();

        let result = page.locator("#missing").click().await;
        recorder.stop().await.unwrap();

        assert!(result.is_err());
        let file = json_file(&dir, "trace.json");
        let action = &file["actions"][0];
        assert_eq!(action["ok"], false);
        assert!(!action["error"].as_str().unwrap().is_empty());
        assert_eq!(action["selector"], "#missing");
    }

    #[tokio::test]
    async fn goto_is_noted_with_the_address_and_without_its_credentials() {
        let (page, mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page.start_trace(&dir).await.unwrap();

        page.goto("https://ada:lovelace@shop.test/login?next=%2F")
            .await
            .unwrap();
        recorder.stop().await.unwrap();

        let file = json_file(&dir, "trace.json");
        assert_eq!(file["actions"][0]["name"], "goto");
        assert_eq!(
            file["actions"][0]["detail"],
            "https://shop.test/login?next=%2F"
        );
        assert!(!all_text(&dir).contains("lovelace"));
        assert!(!mock.calls_to("Page.navigate").is_empty());
    }

    #[tokio::test]
    async fn with_no_trace_an_action_sends_nothing_for_it() {
        let (page, mock, _temp, _dir) = traced_tab(json!([])).await;
        let before = mock.calls().len();

        page.locator("#go").click().await.unwrap();
        page.locator("#name").fill(SECRET).await.unwrap();

        let calls = mock.calls();
        assert!(calls.len() > before, "the actions sent nothing at all");
        for call in &calls[before..] {
            assert_ne!(call.method, "Page.captureScreenshot");
            assert_ne!(call.method, "Page.addScriptToEvaluateOnNewDocument");
            let expression = call.params["expression"].as_str().unwrap_or_default();
            assert!(!expression.contains("__sbtrace"), "{expression}");
        }
    }

    #[tokio::test]
    async fn a_stopped_trace_leaves_nothing_behind() {
        let (page, mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page.start_trace(&dir).await.unwrap();
        recorder.stop().await.unwrap();

        let removed = mock.calls_to("Page.removeScriptToEvaluateOnNewDocument");
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].params["identifier"], "42");
        assert!(mock
            .calls_to("Runtime.evaluate")
            .iter()
            .any(|call| call.params["expression"] == RELEASE_EXPRESSION));
        let shots = mock.calls_to("Page.captureScreenshot").len();
        page.locator("#go").click().await.unwrap();
        assert_eq!(mock.calls_to("Page.captureScreenshot").len(), shots);
        assert!(page.running_trace().is_none());
    }

    #[tokio::test]
    async fn dropping_the_recorder_ends_the_trace() {
        let (page, mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page.start_trace(&dir).await.unwrap();
        drop(recorder);

        page.locator("#go").click().await.unwrap();

        assert_eq!(mock.calls_to("Page.captureScreenshot").len(), 0);
        assert!(page.running_trace().is_none());
    }

    #[tokio::test]
    async fn a_second_trace_on_the_tab_is_refused() {
        let (page, _mock, temp, dir) = traced_tab(json!([])).await;
        let _first = page.start_trace(&dir).await.unwrap();

        let second = page.start_trace(temp.path().join("other")).await;

        assert!(matches!(second, Err(SeleniumBaseError::InvalidConfig(_))));
        assert!(!temp.path().join("other").exists());
    }

    #[tokio::test]
    async fn typed_text_is_in_no_file_though_the_page_received_it() {
        let (page, mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page.start_trace(&dir).await.unwrap();
        let field = page.locator("#pw");

        field.fill(SECRET).await.unwrap();
        field.type_text(SECRET).await.unwrap();
        field.send_keys(SECRET).await.unwrap();
        field.set_value(SECRET).await.unwrap();
        recorder.stop().await.unwrap();

        let typed = format!("{:?}", mock.calls());
        assert!(typed.contains(SECRET), "the text never reached the page");
        assert!(!all_text(&dir).contains(SECRET));
        let file = json_file(&dir, "trace.json");
        let details: Vec<_> = file["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|action| action["detail"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(details, vec![MASKED_TEXT; 4]);
    }

    #[tokio::test]
    async fn request_bodies_and_credentials_are_not_in_the_har() {
        let (page, mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page.start_trace(&dir).await.unwrap();
        mock.emit(
            "Network.requestWillBeSent",
            json!({
                "requestId": "R1", "loaderId": "L1", "frameId": "T1", "type": "XHR",
                "timestamp": 1.0, "wallTime": 1_700_000_000.5,
                "request": {
                    "url": "https://shop.test/login", "method": "POST",
                    "headers": {
                        "Authorization": "Bearer top-secret-token",
                        "Proxy-Authorization": "Basic cHJveHk6c2VjcmV0",
                        "Accept": "*/*",
                    },
                    "postData": format!("password={SECRET}"),
                },
            }),
            Some("S1"),
        );
        mock.emit(
            "Network.responseReceived",
            json!({
                "requestId": "R1", "timestamp": 1.1,
                "response": { "url": "x", "status": 401, "statusText": "no",
                              "headers": {}, "mimeType": "text/plain", "protocol": "http/1.1" },
            }),
            Some("S1"),
        );
        mock.emit(
            "Network.loadingFinished",
            json!({ "requestId": "R1", "timestamp": 1.2, "encodedDataLength": 10 }),
            Some("S1"),
        );

        let trace = recorder.stop().await.unwrap();

        let text = all_text(&dir);
        assert!(!text.contains(SECRET), "a request body is in the trace");
        assert!(!text.contains("top-secret-token"));
        assert!(!text.contains("cHJveHk6c2VjcmV0"));
        let har = std::fs::read_to_string(dir.join("network.har")).unwrap();
        assert!(har.contains(MASKED_BODY));
        assert!(har.contains(REMOVED));
        assert!(har.contains("Accept"));
        assert_eq!(trace.requests, 1);
        let file = json_file(&dir, "trace.json");
        assert_eq!(file["network"][0]["method"], "POST");
        assert_eq!(file["network"][0]["status"], 401);
        assert_eq!(file["network"][0]["type"], "xhr");
    }

    #[tokio::test]
    async fn the_action_limit_notes_the_first_actions_and_runs_the_rest() {
        let (page, _mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page
            .start_trace_with(&dir, TraceOptions::new().max_actions(1))
            .await
            .unwrap();

        page.locator("#a").click().await.unwrap();
        page.locator("#b").click().await.unwrap();
        recorder.stop().await.unwrap();

        let file = json_file(&dir, "trace.json");
        assert_eq!(file["actions"].as_array().unwrap().len(), 1);
        assert_eq!(file["limits"]["actions_skipped"], 1);
    }

    #[tokio::test]
    async fn the_byte_limit_keeps_the_actions_and_drops_their_pictures() {
        let (page, mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page
            .start_trace_with(&dir, TraceOptions::new().max_bytes(0))
            .await
            .unwrap();

        page.locator("#a").click().await.unwrap();
        let trace = recorder.stop().await.unwrap();

        let file = json_file(&dir, "trace.json");
        let action = &file["actions"][0];
        assert_eq!(action["screenshot"], Value::Null);
        assert_eq!(action["snapshot"], Value::Null);
        assert_eq!(file["limits"]["pictures_skipped"], 1);
        assert_eq!(trace.picture_bytes, 0);
        assert_eq!(mock.calls_to("Page.captureScreenshot").len(), 0);
    }

    #[tokio::test]
    async fn the_byte_limit_also_bounds_the_pictures_of_one_action() {
        let (page, _mock, _temp, dir) = traced_tab(json!([])).await;
        // The screenshot (8 bytes) and the snapshot together do not fit, and
        // the pictures of one action are kept whole or not at all.
        let recorder = page
            .start_trace_with(&dir, TraceOptions::new().max_bytes(20))
            .await
            .unwrap();

        page.locator("#a").click().await.unwrap();
        let trace = recorder.stop().await.unwrap();

        assert!(trace.picture_bytes <= 20, "{}", trace.picture_bytes);
        assert_eq!(
            json_file(&dir, "trace.json")["limits"]["pictures_skipped"],
            1
        );
    }

    #[tokio::test]
    async fn console_messages_are_kept_up_to_the_limit_and_the_rest_counted() {
        let at = jiff::Timestamp::now().as_millisecond() + 5;
        let console = json!([
            { "at": at, "level": "warn", "text": "one" },
            { "at": at, "level": "bogus", "text": "two" },
            { "at": at, "level": "error", "text": "three" },
        ]);
        let (page, _mock, _temp, dir) = traced_tab(console).await;
        let recorder = page
            .start_trace_with(&dir, TraceOptions::new().max_console_messages(2))
            .await
            .unwrap();

        page.locator("#a").click().await.unwrap();
        recorder.stop().await.unwrap();

        let file = json_file(&dir, "trace.json");
        let messages = file["console"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["level"], "warn");
        assert_eq!(
            messages[1]["level"], "log",
            "a made-up level is a plain log"
        );
        assert_eq!(messages[0]["action"], 1);
        assert_eq!(file["limits"]["console_dropped"], 1);
    }

    #[tokio::test]
    async fn a_hostile_page_and_selector_cannot_open_a_script_in_the_viewer() {
        let hostile = "</script><script>alert('console')</script>\u{2028}<!--";
        let console = json!([{
            "at": jiff::Timestamp::now().as_millisecond(), "level": "error", "text": hostile,
        }]);
        let (page, mock) = tab().await;
        script(
            &mock,
            console,
            "</title><script>alert('title')</script>",
            true,
        );
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("trace");
        let recorder = page.start_trace(&dir).await.unwrap();

        page.locator("#a\"></script><script>alert('selector')</script>")
            .click()
            .await
            .unwrap();
        recorder.stop().await.unwrap();

        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
        // The data element and the viewer script, and no other.
        assert_eq!(count(&html, "<script"), 2, "{html}");
        assert_eq!(count(&html, "</script"), 2);
        assert_eq!(count(&html, "<!--"), 0);
        assert!(!html.contains("</script><script>"), "page text is markup");
        // The hostile text is still there, as data.
        let file = json_file(&dir, "trace.json");
        assert_eq!(file["console"][0]["text"], hostile);
        assert!(file["actions"][0]["selector"]
            .as_str()
            .unwrap()
            .contains("alert('selector')"));
    }

    #[tokio::test]
    async fn the_viewer_carries_the_pictures_and_the_folder_has_them_too() {
        let (page, _mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page.start_trace(&dir).await.unwrap();
        page.locator("#a").click().await.unwrap();
        recorder.stop().await.unwrap();

        let html = std::fs::read_to_string(dir.join("index.html")).unwrap();
        let jpeg = base64::engine::general_purpose::STANDARD.encode(b"\xff\xd8jpeg\xff\xd9");
        assert!(html.contains(&jpeg), "the screenshot is not in the viewer");
        assert!(html.contains("Hello"), "the snapshot is not in the viewer");
        assert_eq!(
            std::fs::read(dir.join("screenshots/0001.jpg")).unwrap(),
            b"\xff\xd8jpeg\xff\xd9"
        );
    }

    #[tokio::test]
    async fn a_trace_enables_none_of_the_domains_a_page_can_feel() {
        let (page, mock, _temp, dir) = traced_tab(json!([])).await;
        let recorder = page.start_trace(&dir).await.unwrap();
        page.locator("#a").click().await.unwrap();
        recorder.stop().await.unwrap();

        for domain in ["Runtime", "DOM", "Log", "Debugger", "Profiler"] {
            let method = format!("{domain}.enable");
            assert!(mock.calls_to(&method).is_empty(), "{method} was sent");
        }
        assert!(mock
            .calls()
            .iter()
            .all(|call| !call.method.starts_with("DOM.")));
    }

    #[tokio::test]
    async fn a_trace_path_that_is_not_a_fresh_folder_is_refused_and_sends_nothing() {
        let (page, mock) = tab().await;
        script(&mock, json!([]), "Cart", true);
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("file");
        std::fs::write(&file, "x").unwrap();
        let full = temp.path().join("full");
        std::fs::create_dir(&full).unwrap();
        std::fs::write(full.join("keep.txt"), "x").unwrap();
        let mut refused = vec![
            PathBuf::new(),
            file,
            full.clone(),
            temp.path().join("no_parent").join("trace"),
        ];
        #[cfg(unix)]
        {
            let empty = temp.path().join("empty");
            std::fs::create_dir(&empty).unwrap();
            let link = temp.path().join("link");
            std::os::unix::fs::symlink(&empty, &link).unwrap();
            refused.push(link);
        }
        let before = mock.calls().len();

        for path in refused {
            let result = page.start_trace(&path).await;
            assert!(
                matches!(result, Err(SeleniumBaseError::InvalidConfig(_))),
                "{} was not refused",
                path.display()
            );
        }

        assert_eq!(mock.calls().len(), before, "a refusal sent commands");
        assert!(full.join("keep.txt").is_file());
        assert!(!temp.path().join("no_parent").exists());
    }

    #[tokio::test]
    async fn an_existing_empty_folder_is_accepted() {
        let (page, _mock, temp, _dir) = traced_tab(json!([])).await;
        let empty = temp.path().join("empty");
        std::fs::create_dir(&empty).unwrap();

        let recorder = page.start_trace(&empty).await.unwrap();
        recorder.stop().await.unwrap();

        assert!(empty.join("index.html").is_file());
    }

    #[test]
    fn cutting_text_keeps_whole_characters_and_marks_the_cut() {
        assert_eq!(cut("abc", 3), "abc");
        assert_eq!(cut("abcd", 3), "abc...");
        assert_eq!(cut("ééééé", 2), "éé...");
        let (kept, cut_off) = cut_bytes("éé".to_owned(), 3);
        assert_eq!((kept.as_str(), cut_off), ("é", true));
        assert_eq!(cut_bytes("abc".to_owned(), 3), ("abc".to_owned(), false));
    }

    #[test]
    fn an_address_loses_its_credentials_and_an_unparsable_one_is_not_recorded() {
        assert_eq!(
            plain_url("https://user:pw@example.com/a?b=c"),
            "https://example.com/a?b=c"
        );
        assert_eq!(plain_url("example.com/x"), "https://example.com/x");
        assert_eq!(plain_url("http://[bad"), "[address not recorded]");
    }

    #[test]
    fn the_limits_are_clamped_to_their_hard_maximums() {
        let options = TraceOptions::new()
            .max_actions(usize::MAX)
            .max_bytes(usize::MAX)
            .max_console_messages(usize::MAX)
            .max_requests(usize::MAX);

        assert_eq!(options.max_actions, HARD_MAX_ACTIONS);
        assert_eq!(options.max_bytes, HARD_MAX_BYTES);
        assert_eq!(options.max_console_messages, HARD_MAX_CONSOLE);
        assert_eq!(options.max_requests, HARD_MAX_REQUESTS);
    }

    #[test]
    fn the_page_script_is_safe_to_run_twice_and_undoes_itself() {
        // The properties the Rust side relies on; they are exercised in a real
        // browser by the Chrome test.
        for needle in [
            "window.__sbtrace",
            "release",
            "configurable: true",
            "window.top !== window",
        ] {
            assert!(CAPTURE_JS.contains(needle), "{needle} is missing");
        }
        assert!(!CAPTURE_JS.contains("innerHTML"));
    }
}
