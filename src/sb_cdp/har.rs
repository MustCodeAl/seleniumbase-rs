//! Recording a page's network traffic as a HAR (HTTP Archive 1.2) file; see
//! [`HarRecorder`].

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};

use super::client::{CdpEvent, Events};
use super::Page;
use crate::error::SeleniumBaseError;

/// The most requests one recording keeps.
const MAX_ENTRIES: usize = 50_000;

/// The body size [`HarOptions::content`] keeps unless
/// [`HarOptions::max_content_bytes`] says otherwise: 1 MiB.
pub const DEFAULT_MAX_CONTENT_BYTES: usize = 1024 * 1024;

/// What a [`HarRecorder`] records besides the requests and responses.
///
/// The default records no bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarOptions {
    content: bool,
    max_content_bytes: usize,
}

impl Default for HarOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl HarOptions {
    /// No response bodies, and a 1 MiB limit for when they are turned on.
    #[must_use]
    pub fn new() -> Self {
        Self {
            content: false,
            max_content_bytes: DEFAULT_MAX_CONTENT_BYTES,
        }
    }

    /// With `true`, the body of each response is fetched when it has finished
    /// loading and kept in `content.text` (base64 for a binary body, with
    /// `content.encoding` saying so). A body over
    /// [`max_content_bytes`](Self::max_content_bytes) is left out, and
    /// `content.comment` says why.
    #[must_use]
    pub fn content(mut self, record: bool) -> Self {
        self.content = record;
        self
    }

    /// The largest body, in bytes once decoded, that [`content`](Self::content)
    /// keeps. Defaults to [`DEFAULT_MAX_CONTENT_BYTES`].
    #[must_use]
    pub fn max_content_bytes(mut self, bytes: usize) -> Self {
        self.max_content_bytes = bytes;
        self
    }
}

/// A recording of one tab's network traffic, in progress.
///
/// [`Page::start_har`] starts one; [`finish`](Self::finish) stops it and
/// returns the [`Har`], which [`Har::save`] writes as JSON that browsers'
/// developer tools, HAR viewers and proxies such as mitmproxy can open.
/// Response bodies are left out unless [`HarOptions::content`] asks for them.
/// Dropping a recorder stops the recording and discards what it holds.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::{Browser, HarOptions, LaunchOptions};
///
/// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let browser = Browser::launch(LaunchOptions::default()).await?;
/// let page = browser.default_page().await?;
///
/// let recorder = page.start_har().await?; // or start_har_with(HarOptions::new().content(true))
/// page.goto("https://example.com").await?;
/// let har = recorder.finish().await?;
///
/// println!("{} requests", har.log.entries.len());
/// har.save("example.har")?;
/// # Ok(())
/// # }
/// ```
///
/// # What is recorded
///
/// Every request the tab makes while recording, from the DevTools `Network`
/// events: `requestWillBeSent` (the request, and the end of a redirect),
/// `requestWillBeSentExtraInfo` and `responseReceivedExtraInfo` (the headers
/// as they went over the wire, cookies included), `responseReceived` (status,
/// headers, protocol, server address and the connection timing),
/// `dataReceived` (the size of the body), `loadingFinished` and
/// `loadingFailed`. A redirect becomes one entry per hop. A failed request has
/// status 0 and the browser's error (such as `net::ERR_NAME_NOT_RESOLVED`) in
/// the response's `_error` field, as Chrome's own HAR export writes it. A
/// request still waiting for its response when the recording finishes is kept,
/// with status 0 and a comment saying so.
///
/// Each top-level navigation starts a new page (`log.pages`), titled with its
/// URL, with `onContentLoad` and `onLoad` from the `Page` domain's events.
/// `timings` split each request into `blocked`, `dns`, `connect` (`ssl`
/// included, as HAR 1.2 asks), `send`, `wait` and `receive`; a phase that did
/// not happen, such as `dns` on a reused connection, is -1. Without the
/// browser's connection timing (a response from the cache), only `wait` and
/// `receive` are known. Header and body sizes are not reported by the browser
/// apart, so `headersSize` and `bodySize` are -1 and the bytes received are in
/// the response's `_transferSize`, again as in Chrome's export.
///
/// Recording enables the `Network` domain for the tab and leaves it enabled
/// after [`finish`](Self::finish), because other features of the tab may use
/// it too. At most 50 000 requests are kept; `log.comment` says how many more
/// there were.
#[derive(Debug)]
pub struct HarRecorder {
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Recording>>,
    browser: Option<HarCreator>,
}

impl Page {
    /// Starts recording this tab's network traffic as a HAR, without
    /// response bodies. Same as [`start_har_with`](Self::start_har_with) with
    /// the default [`HarOptions`].
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses to
    /// report network traffic.
    pub async fn start_har(&self) -> Result<HarRecorder, SeleniumBaseError> {
        self.start_har_with(HarOptions::default()).await
    }

    /// Starts recording this tab's network traffic as a HAR; see
    /// [`HarRecorder`] for what is kept.
    ///
    /// Only what happens after this returns is recorded, so start it before
    /// the navigation you want to see. Other tabs are not recorded.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses to
    /// report network traffic.
    pub async fn start_har_with(
        &self,
        options: HarOptions,
    ) -> Result<HarRecorder, SeleniumBaseError> {
        // Listen before enabling, so no request is missed.
        let events = self.events();
        self.execute("Network.enable", json!({})).await?;
        let browser = self
            .browser()
            .execute("Browser.getVersion", json!({}))
            .await
            .ok()
            .and_then(|version| browser_creator(&version));
        let recording = Recording::new(self.id().to_owned(), options);
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(record(events, stopped, self.clone(), recording));
        Ok(HarRecorder {
            stop: Some(stop),
            task: Some(task),
            browser,
        })
    }
}

impl HarRecorder {
    /// Stops recording and returns the HAR.
    ///
    /// Events the browser had already sent are taken in first, and the
    /// bodies still being fetched are waited for.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the recording task failed.
    pub async fn finish(mut self) -> Result<Har, SeleniumBaseError> {
        if let Some(stop) = self.stop.take() {
            // The task may already have ended with the connection.
            let _ = stop.send(());
        }
        let task = self
            .task
            .take()
            .ok_or_else(|| SeleniumBaseError::cdp_driver("the HAR recording already ended"))?;
        let recording = task.await.map_err(|error| {
            SeleniumBaseError::cdp_driver(format!("the HAR recording stopped: {error}"))
        })?;
        Ok(recording.into_har(self.browser.take()))
    }
}

impl Drop for HarRecorder {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Takes in events until told to stop (or the connection closes), then waits
/// for the bodies being fetched.
async fn record(
    mut events: Events,
    mut stop: oneshot::Receiver<()>,
    page: Page,
    mut recording: Recording,
) -> Recording {
    let mut bodies = JoinSet::new();
    loop {
        tokio::select! {
            // Events first: what the browser has sent before the stop is kept.
            biased;
            event = events.next() => {
                let Some(event) = event else { break };
                if let Some((index, request_id)) = recording.note(&event) {
                    let page = page.clone();
                    bodies.spawn(async move {
                        let body = page
                            .execute("Network.getResponseBody", json!({ "requestId": request_id }))
                            .await;
                        (index, body)
                    });
                }
            }
            _ = &mut stop => break,
        }
    }
    while let Some(fetched) = bodies.join_next().await {
        if let Ok((index, body)) = fetched {
            recording.set_content(index, body);
        }
    }
    recording
}

/// One request as the events describe it, until the HAR is built.
#[derive(Debug)]
struct EntryState {
    page: Option<usize>,
    /// Seconds since the Unix epoch.
    started_wall: f64,
    /// The browser's monotonic clock, in seconds, as are the other times.
    issued: f64,
    request: Value,
    request_extra: Option<Value>,
    resource_type: Option<String>,
    response: Option<Value>,
    response_extra: Option<Value>,
    response_at: Option<f64>,
    finished_at: Option<f64>,
    encoded_length: Option<f64>,
    data_length: u64,
    error: Option<String>,
    content: ContentState,
}

#[derive(Debug, Default)]
struct ContentState {
    text: Option<String>,
    encoding: Option<String>,
    size: Option<u64>,
    comment: Option<String>,
}

#[derive(Debug)]
struct PageState {
    url: String,
    started_wall: f64,
    started: f64,
    content_loaded: Option<f64>,
    loaded: Option<f64>,
}

/// Everything recorded so far.
#[derive(Debug)]
struct Recording {
    /// The tab's target id, which is also its main frame's id.
    main_frame: String,
    options: HarOptions,
    entries: Vec<EntryState>,
    /// The latest entry of each request id (a redirect reuses the id).
    current: HashMap<String, usize>,
    /// Headers that arrived before the request or response they belong to.
    early_request_extra: HashMap<String, Value>,
    early_response_extra: HashMap<String, Value>,
    pages: Vec<PageState>,
    /// A moment on the browser's monotonic clock and the wall-clock time then.
    clock: Option<(f64, f64)>,
    dropped: usize,
}

impl Recording {
    fn new(main_frame: String, options: HarOptions) -> Self {
        Self {
            main_frame,
            options,
            entries: Vec::new(),
            current: HashMap::new(),
            early_request_extra: HashMap::new(),
            early_response_extra: HashMap::new(),
            pages: Vec::new(),
            clock: None,
            dropped: 0,
        }
    }

    /// Takes in one event. Returns the entry and request id of a body to
    /// fetch, when one has finished loading and bodies are recorded.
    fn note(&mut self, event: &CdpEvent) -> Option<(usize, String)> {
        let params = &event.params;
        let timestamp = params["timestamp"].as_f64();
        match event.method.as_str() {
            "Network.requestWillBeSent" => self.request_will_be_sent(params),
            "Network.requestWillBeSentExtraInfo" => self.request_extra(params),
            "Network.responseReceived" => self.response_received(params),
            "Network.responseReceivedExtraInfo" => self.response_extra(params),
            "Network.dataReceived" => {
                if let Some(entry) = self.entry_mut(params) {
                    entry.data_length += params["dataLength"].as_u64().unwrap_or(0);
                }
            }
            "Network.loadingFinished" => return self.loading_finished(params),
            "Network.loadingFailed" => {
                if let Some(entry) = self.entry_mut(params) {
                    entry.finished_at = timestamp.or(entry.finished_at);
                    let mut error = params["errorText"].as_str().unwrap_or("failed").to_owned();
                    if params["canceled"].as_bool() == Some(true) {
                        error.push_str(" (canceled)");
                    }
                    if let Some(reason) = params["blockedReason"].as_str() {
                        error = format!("{error} (blocked: {reason})");
                    }
                    entry.error = Some(error);
                }
            }
            "Page.domContentEventFired" => {
                if let (Some(page), Some(at)) = (self.pages.last_mut(), timestamp) {
                    page.content_loaded.get_or_insert(at);
                }
            }
            "Page.loadEventFired" => {
                if let (Some(page), Some(at)) = (self.pages.last_mut(), timestamp) {
                    page.loaded.get_or_insert(at);
                }
            }
            _ => {}
        }
        None
    }

    fn entry_mut(&mut self, params: &Value) -> Option<&mut EntryState> {
        let index = *self.current.get(params["requestId"].as_str()?)?;
        self.entries.get_mut(index)
    }

    /// The wall-clock time at `timestamp` on the browser's monotonic clock.
    fn wall_at(&self, timestamp: f64) -> f64 {
        match self.clock {
            Some((at, wall)) => wall + (timestamp - at),
            None => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0.0, |since| since.as_secs_f64()),
        }
    }

    fn request_will_be_sent(&mut self, params: &Value) {
        let Some(id) = params["requestId"].as_str() else {
            return;
        };
        let timestamp = params["timestamp"].as_f64().unwrap_or(0.0);
        let wall = match params["wallTime"].as_f64() {
            Some(wall) => {
                self.clock = Some((timestamp, wall));
                wall
            }
            None => self.wall_at(timestamp),
        };

        let redirect = params.get("redirectResponse").filter(|r| r.is_object());
        if let Some(redirect) = redirect {
            // The request id stays; the previous hop ends with this response.
            if let Some(entry) = self.entry_mut(params) {
                entry.response = Some(redirect.clone());
                entry.response_at = Some(timestamp);
                entry.finished_at = Some(timestamp);
            }
        } else if params["type"] == "Document"
            && params["frameId"].as_str() == Some(self.main_frame.as_str())
            && params["loaderId"].as_str() == Some(id)
        {
            self.pages.push(PageState {
                url: params["request"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                started_wall: wall,
                started: timestamp,
                content_loaded: None,
                loaded: None,
            });
        }

        if self.entries.len() >= MAX_ENTRIES {
            self.dropped += 1;
            self.current.remove(id);
            return;
        }
        let index = self.entries.len();
        self.entries.push(EntryState {
            page: self.pages.len().checked_sub(1),
            started_wall: wall,
            issued: timestamp,
            request: params["request"].clone(),
            request_extra: self.early_request_extra.remove(id),
            resource_type: params["type"].as_str().map(str::to_ascii_lowercase),
            response: None,
            response_extra: None,
            response_at: None,
            finished_at: None,
            encoded_length: None,
            data_length: 0,
            error: None,
            content: ContentState::default(),
        });
        self.current.insert(id.to_owned(), index);
    }

    fn request_extra(&mut self, params: &Value) {
        let Some(id) = params["requestId"].as_str() else {
            return;
        };
        let extra = params["headers"].clone();
        match self.entry_mut(params) {
            Some(entry) if entry.request_extra.is_none() => entry.request_extra = Some(extra),
            // Before its request, or for the next hop of a redirect.
            _ => {
                self.early_request_extra.insert(id.to_owned(), extra);
            }
        }
    }

    fn response_received(&mut self, params: &Value) {
        let Some(id) = params["requestId"].as_str() else {
            return;
        };
        let early = self.early_response_extra.remove(id);
        if let Some(entry) = self.entry_mut(params) {
            entry.response = Some(params["response"].clone());
            entry.response_at = params["timestamp"].as_f64();
            if let Some(kind) = params["type"].as_str() {
                entry.resource_type = Some(kind.to_ascii_lowercase());
            }
            if entry.response_extra.is_none() {
                entry.response_extra = early.filter(|extra| fits(entry, extra));
            }
        }
    }

    fn response_extra(&mut self, params: &Value) {
        let Some(id) = params["requestId"].as_str() else {
            return;
        };
        if let Some(entry) = self.entry_mut(params) {
            if entry.response_extra.is_none() && fits(entry, params) {
                entry.response_extra = Some(params.clone());
                return;
            }
        }
        // Before its request, or another hop's.
        self.early_response_extra
            .insert(id.to_owned(), params.clone());
    }

    fn loading_finished(&mut self, params: &Value) -> Option<(usize, String)> {
        let id = params["requestId"].as_str()?.to_owned();
        let index = *self.current.get(&id)?;
        let options = self.options;
        let entry = self.entries.get_mut(index)?;
        entry.finished_at = params["timestamp"].as_f64().or(entry.finished_at);
        entry.encoded_length = params["encodedDataLength"].as_f64();
        if !options.content || entry.response.is_none() {
            return None;
        }
        let limit = u64::try_from(options.max_content_bytes).unwrap_or(u64::MAX);
        if entry.data_length > limit {
            entry.content.comment = Some(too_large(entry.data_length, options.max_content_bytes));
            return None;
        }
        Some((index, id))
    }

    /// Keeps a fetched body, or why there is none.
    fn set_content(&mut self, index: usize, body: Result<Value, SeleniumBaseError>) {
        let limit = self.options.max_content_bytes;
        let Some(entry) = self.entries.get_mut(index) else {
            return;
        };
        let body = match body {
            Ok(body) => body,
            Err(error) => {
                entry.content.comment = Some(format!("content not recorded: {error}"));
                return;
            }
        };
        let text = body["body"].as_str().unwrap_or_default();
        let base64 = body["base64Encoded"].as_bool() == Some(true);
        let size = if base64 {
            decoded_len(text)
        } else {
            text.len()
        };
        let bytes = u64::try_from(size).unwrap_or(u64::MAX);
        if size > limit {
            entry.content.comment = Some(too_large(bytes, limit));
            return;
        }
        entry.content = ContentState {
            text: Some(text.to_owned()),
            encoding: base64.then(|| "base64".to_owned()),
            size: Some(bytes),
            comment: None,
        };
    }

    fn into_har(self, browser: Option<HarCreator>) -> Har {
        let pages = self
            .pages
            .iter()
            .enumerate()
            .map(|(index, page)| HarPage {
                started_date_time: iso_time(page.started_wall),
                id: page_id(index),
                title: page.url.clone(),
                page_timings: HarPageTimings {
                    on_content_load: page
                        .content_loaded
                        .map_or(-1.0, |at| millis(at - page.started).max(0.0)),
                    on_load: page
                        .loaded
                        .map_or(-1.0, |at| millis(at - page.started).max(0.0)),
                    comment: None,
                },
                comment: None,
            })
            .collect();
        let entries = self.entries.iter().map(har_entry).collect();
        let comment = (self.dropped > 0).then(|| {
            format!(
                "{} more requests after the first {MAX_ENTRIES} were not recorded",
                self.dropped
            )
        });
        Har {
            log: HarLog {
                version: "1.2".to_owned(),
                creator: HarCreator {
                    name: "seleniumbase-rs".to_owned(),
                    version: env!("CARGO_PKG_VERSION").to_owned(),
                    comment: None,
                },
                browser,
                pages,
                entries,
                comment,
            },
        }
    }
}

/// Whether `extra` can be the extra information of `entry`'s response: its
/// status matches, or the response has not arrived to tell.
fn fits(entry: &EntryState, extra: &Value) -> bool {
    let status = entry.response.as_ref().and_then(|r| r["status"].as_u64());
    match (status, extra["statusCode"].as_u64()) {
        (Some(status), Some(code)) => status == code,
        _ => true,
    }
}

fn too_large(size: u64, limit: usize) -> String {
    format!("content not recorded: {size} bytes is over the {limit}-byte limit")
}

/// The decoded size of base64 `text`.
fn decoded_len(text: &str) -> usize {
    let padding = text.bytes().rev().take_while(|b| *b == b'=').count();
    (text.len() / 4 * 3).saturating_sub(padding)
}

fn page_id(index: usize) -> String {
    format!("page_{}", index + 1)
}

fn browser_creator(version: &Value) -> Option<HarCreator> {
    let product = version["product"].as_str()?;
    let (name, version) = product.split_once('/').unwrap_or((product, ""));
    Some(HarCreator {
        name: name.to_owned(),
        version: version.to_owned(),
        comment: None,
    })
}

/// Seconds as milliseconds, to a thousandth.
fn millis(seconds: f64) -> f64 {
    round3(seconds * 1000.0)
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a saturating conversion of a byte count or a time is what is wanted"
)]
fn whole(value: f64) -> i64 {
    value.round() as i64
}

/// Seconds since the Unix epoch as an ISO 8601 time with milliseconds.
fn iso_time(wall: f64) -> String {
    chrono::DateTime::from_timestamp_millis(whole(wall * 1000.0))
        .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_default()
}

fn har_entry(entry: &EntryState) -> HarEntry {
    let timings = timings(entry);
    let time = round3(
        [
            timings.blocked,
            timings.dns,
            timings.connect,
            timings.send,
            timings.wait,
            timings.receive,
        ]
        .iter()
        .filter(|phase| **phase > 0.0)
        // Not `sum`: an empty float sum is -0.0, which JSON writes as such.
        .fold(0.0, |total, phase| total + phase),
    );
    let response = entry.response.as_ref();
    let protocol = response.and_then(|r| r["protocol"].as_str());
    HarEntry {
        pageref: entry.page.map(page_id),
        started_date_time: iso_time(entry.started_wall),
        time,
        request: har_request(entry, protocol),
        response: har_response(entry, protocol),
        cache: HarCache::default(),
        timings,
        server_ip_address: response
            .and_then(|r| r["remoteIPAddress"].as_str())
            .map(str::to_owned),
        connection: response
            .and_then(|r| r["connectionId"].as_f64())
            .filter(|id| *id > 0.0)
            .map(|id| whole(id).to_string()),
        comment: None,
        resource_type: entry.resource_type.clone(),
    }
}

fn har_request(entry: &EntryState, protocol: Option<&str>) -> HarRequest {
    let request = &entry.request;
    let mut url = request["url"].as_str().unwrap_or_default().to_owned();
    if let Some(fragment) = request["urlFragment"].as_str() {
        url.push_str(fragment);
    }
    let headers = name_values(entry.request_extra.as_ref().unwrap_or(&request["headers"]));
    let cookies = header_values(&headers, "cookie")
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| {
            let (name, value) = pair.trim().split_once('=')?;
            Some(HarCookie::new(name, value))
        })
        .collect();
    let query_string = url::Url::parse(&url)
        .map(|parsed| {
            parsed
                .query_pairs()
                .map(|(name, value)| HarNameValue::new(&name, &value))
                .collect()
        })
        .unwrap_or_default();
    let post_data = post_text(request).map(|text| HarPostData {
        mime_type: header_values(&headers, "content-type")
            .next()
            .unwrap_or_default()
            .to_owned(),
        text,
        comment: None,
    });
    let body_size = post_data
        .as_ref()
        .map_or(0, |post| i64::try_from(post.text.len()).unwrap_or(i64::MAX));
    HarRequest {
        method: request["method"].as_str().unwrap_or("GET").to_owned(),
        url,
        http_version: http_version(protocol),
        cookies,
        headers,
        query_string,
        post_data,
        headers_size: -1,
        body_size,
        comment: None,
    }
}

/// The request body, from `postData` or, when Chrome sends only those, the
/// base64 `postDataEntries`.
fn post_text(request: &Value) -> Option<String> {
    if let Some(text) = request["postData"].as_str() {
        return Some(text.to_owned());
    }
    let entries = request["postDataEntries"].as_array()?;
    let mut bytes = Vec::new();
    for part in entries {
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(part["bytes"].as_str().unwrap_or_default())
            .unwrap_or_default();
        bytes.extend(decoded);
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn har_response(entry: &EntryState, protocol: Option<&str>) -> HarResponse {
    let Some(response) = entry.response.as_ref() else {
        let (error, comment) = match &entry.error {
            Some(error) => (Some(error.clone()), None),
            None => (
                None,
                Some("no response before the recording finished".to_owned()),
            ),
        };
        return HarResponse {
            status: 0,
            status_text: String::new(),
            http_version: String::new(),
            cookies: Vec::new(),
            headers: Vec::new(),
            content: HarContent {
                size: 0,
                compression: None,
                mime_type: "x-unknown".to_owned(),
                text: None,
                encoding: None,
                comment: None,
            },
            redirect_url: String::new(),
            headers_size: -1,
            body_size: -1,
            comment,
            transfer_size: None,
            error,
        };
    };
    let headers = name_values(
        entry
            .response_extra
            .as_ref()
            .map_or(&response["headers"], |extra| &extra["headers"]),
    );
    let cookies = header_values(&headers, "set-cookie")
        .filter_map(set_cookie)
        .collect();
    let status = response["status"]
        .as_u64()
        .and_then(|status| u16::try_from(status).ok())
        .unwrap_or(0);
    let redirect_url = if (300..400).contains(&status) {
        header_values(&headers, "location")
            .next()
            .unwrap_or_default()
            .to_owned()
    } else {
        String::new()
    };
    let size = entry.content.size.unwrap_or(entry.data_length);
    HarResponse {
        status,
        status_text: response["statusText"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        http_version: http_version(protocol),
        cookies,
        headers,
        content: HarContent {
            size: i64::try_from(size).unwrap_or(i64::MAX),
            compression: None,
            mime_type: response["mimeType"]
                .as_str()
                .unwrap_or("x-unknown")
                .to_owned(),
            text: entry.content.text.clone(),
            encoding: entry.content.encoding.clone(),
            comment: entry.content.comment.clone(),
        },
        redirect_url,
        headers_size: -1,
        body_size: -1,
        comment: None,
        transfer_size: entry.encoded_length.map(whole),
        error: entry.error.clone(),
    }
}

/// One `Set-Cookie` line as a cookie.
fn set_cookie(line: &str) -> Option<HarCookie> {
    let mut parts = line.split(';');
    let (name, value) = parts.next()?.trim().split_once('=')?;
    let mut cookie = HarCookie::new(name, value);
    for attribute in parts {
        let (key, value) = attribute
            .trim()
            .split_once('=')
            .unwrap_or((attribute.trim(), ""));
        match key.to_ascii_lowercase().as_str() {
            "path" => cookie.path = Some(value.to_owned()),
            "domain" => cookie.domain = Some(value.to_owned()),
            "expires" => cookie.expires = Some(value.to_owned()),
            "httponly" => cookie.http_only = Some(true),
            "secure" => cookie.secure = Some(true),
            _ => {}
        }
    }
    Some(cookie)
}

/// A protocol header object as HAR headers; a value holding several lines
/// (Chrome joins repeated `Set-Cookie` headers so) becomes one header each.
fn name_values(headers: &Value) -> Vec<HarNameValue> {
    let Some(map) = headers.as_object() else {
        return Vec::new();
    };
    map.iter()
        .flat_map(|(name, value)| {
            let text = value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_owned);
            text.split('\n')
                .map(|line| HarNameValue::new(name, line))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The values of the headers called `name`, in any case.
fn header_values<'a>(
    headers: &'a [HarNameValue],
    name: &'a str,
) -> impl Iterator<Item = &'a str> + 'a {
    headers
        .iter()
        .filter(move |header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}

/// The protocol Chrome reports (`h2`, `http/1.1`, ...) as a HAR `httpVersion`.
fn http_version(protocol: Option<&str>) -> String {
    let Some(protocol) = protocol else {
        return String::new();
    };
    match protocol.to_ascii_lowercase().as_str() {
        "h2" | "http/2" | "http/2.0" => "HTTP/2".to_owned(),
        "http/1.1" => "HTTP/1.1".to_owned(),
        "http/1.0" => "HTTP/1.0".to_owned(),
        quic if quic.starts_with("h3") || quic.starts_with("quic") => "HTTP/3".to_owned(),
        _ => protocol.to_owned(),
    }
}

/// The phases of one request, in milliseconds.
fn timings(entry: &EntryState) -> HarTimings {
    let end = entry
        .finished_at
        .or(entry.response_at)
        .unwrap_or(entry.issued);
    let timing = entry
        .response
        .as_ref()
        .map(|response| &response["timing"])
        .filter(|timing| timing["requestTime"].is_number());
    let Some(timing) = timing else {
        let wait = entry
            .response_at
            .map_or(end - entry.issued, |at| at - entry.issued);
        let receive = entry.response_at.map_or(0.0, |at| end - at);
        return HarTimings {
            blocked: -1.0,
            dns: -1.0,
            connect: -1.0,
            send: 0.0,
            wait: millis(wait.max(0.0)),
            receive: millis(receive.max(0.0)),
            ssl: -1.0,
            comment: None,
        };
    };
    // Offsets from `requestTime` in milliseconds, -1 where a phase did not
    // happen.
    let at = |field: &str| timing[field].as_f64().filter(|value| *value >= 0.0);
    let span = |start: &str, finish: &str| match (at(start), at(finish)) {
        (Some(start), Some(finish)) => round3((finish - start).max(0.0)),
        _ => -1.0,
    };
    let request_time = timing["requestTime"].as_f64().unwrap_or(entry.issued);
    let queued = millis((request_time - entry.issued).max(0.0));
    let started = ["dnsStart", "connectStart", "sendStart"]
        .into_iter()
        .filter_map(at)
        .reduce(f64::min)
        .unwrap_or(0.0);
    let send_end = at("sendEnd").unwrap_or(started);
    let headers_end = at("receiveHeadersEnd").unwrap_or(send_end);
    HarTimings {
        blocked: round3(queued + started),
        dns: span("dnsStart", "dnsEnd"),
        connect: span("connectStart", "connectEnd"),
        send: span("sendStart", "sendEnd").max(0.0),
        wait: round3((headers_end - send_end).max(0.0)),
        receive: round3((millis(end - request_time) - headers_end).max(0.0)),
        ssl: span("sslStart", "sslEnd"),
        comment: None,
    }
}

// ----------------------------------------------------------------------
// HAR 1.2 (http://www.softwareishard.com/blog/har-12-spec/)
// ----------------------------------------------------------------------

/// An HTTP Archive: the root of a HAR file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Har {
    /// Everything the archive holds.
    pub log: HarLog,
}

impl Har {
    /// The archive as indented JSON.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Json`] if it cannot be serialised.
    pub fn to_json(&self) -> Result<String, SeleniumBaseError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Writes the archive to `path` as JSON (a `.har` file), replacing a file
    /// that is there.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if the file cannot be written.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SeleniumBaseError> {
        let path = path.as_ref();
        std::fs::write(path, self.to_json()?).map_err(|error| {
            SeleniumBaseError::Io(std::io::Error::new(
                error.kind(),
                format!("cannot write the HAR {}: {error}", path.display()),
            ))
        })
    }
}

/// The `log` object: who made the archive, its pages and its requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct HarLog {
    /// The HAR format version, `1.2`.
    pub version: String,
    /// The program that wrote the archive.
    pub creator: HarCreator,
    /// The browser the traffic came from, when it said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<HarCreator>,
    /// One entry for each top-level navigation.
    #[serde(default)]
    pub pages: Vec<HarPage>,
    /// One entry for each request (each hop of a redirect).
    pub entries: Vec<HarEntry>,
    /// A note, such as how many requests were not recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// A program or browser: the `creator` and `browser` objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HarCreator {
    /// Its name.
    pub name: String,
    /// Its version.
    pub version: String,
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// One top-level page load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct HarPage {
    /// When the navigation started, ISO 8601.
    pub started_date_time: String,
    /// The id entries name in their `pageref`, `page_1`, `page_2`, ...
    pub id: String,
    /// The page's title; here the URL it was opened with.
    pub title: String,
    /// When its load events fired.
    pub page_timings: HarPageTimings,
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// When a page's load events fired, in milliseconds after it started, or -1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct HarPageTimings {
    /// `DOMContentLoaded`.
    #[serde(default = "not_applicable")]
    pub on_content_load: f64,
    /// `load`.
    #[serde(default = "not_applicable")]
    pub on_load: f64,
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

fn not_applicable() -> f64 {
    -1.0
}

/// One request and its response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct HarEntry {
    /// The page it belongs to, if a page was recorded before it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pageref: Option<String>,
    /// When it was issued, ISO 8601.
    pub started_date_time: String,
    /// Its total time in milliseconds: the sum of its known timings.
    pub time: f64,
    /// The request.
    pub request: HarRequest,
    /// The response.
    pub response: HarResponse,
    /// Cache information; always empty here.
    pub cache: HarCache,
    /// Its phases.
    pub timings: HarTimings,
    /// The server's IP address.
    #[serde(
        rename = "serverIPAddress",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub server_ip_address: Option<String>,
    /// The browser's id for the connection it used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<String>,
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// What it was for (`document`, `script`, `image`, `xhr`, ...), as
    /// Chrome's HAR export names it.
    #[serde(
        rename = "_resourceType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub resource_type: Option<String>,
}

/// The request of an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct HarRequest {
    /// The HTTP method.
    pub method: String,
    /// The full URL, fragment included.
    pub url: String,
    /// `HTTP/1.1`, `HTTP/2`, `HTTP/3`, or empty when unknown.
    pub http_version: String,
    /// The cookies of its `Cookie` header.
    pub cookies: Vec<HarCookie>,
    /// Its headers, as sent when the browser reported them.
    pub headers: Vec<HarNameValue>,
    /// The URL's query parameters, decoded.
    pub query_string: Vec<HarNameValue>,
    /// Its body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_data: Option<HarPostData>,
    /// The size of its headers; -1, not known.
    pub headers_size: i64,
    /// The size of its body in bytes.
    pub body_size: i64,
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// The response of an entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct HarResponse {
    /// The status code, 0 when there was no response.
    pub status: u16,
    /// The status text.
    pub status_text: String,
    /// `HTTP/1.1`, `HTTP/2`, `HTTP/3`, or empty when unknown.
    pub http_version: String,
    /// The cookies of its `Set-Cookie` headers.
    pub cookies: Vec<HarCookie>,
    /// Its headers.
    pub headers: Vec<HarNameValue>,
    /// Its body.
    pub content: HarContent,
    /// The `Location` of a redirect, else empty.
    #[serde(rename = "redirectURL")]
    pub redirect_url: String,
    /// The size of its headers; -1, not known.
    pub headers_size: i64,
    /// The size of its body as sent; -1, not known (see `_transferSize`).
    pub body_size: i64,
    /// A note, such as that no response came.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// The bytes received for it, headers included, as Chrome's HAR export
    /// names it.
    #[serde(
        rename = "_transferSize",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub transfer_size: Option<i64>,
    /// Why it failed, such as `net::ERR_CONNECTION_REFUSED`, as Chrome's HAR
    /// export names it.
    #[serde(rename = "_error", default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A header or a query parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HarNameValue {
    /// Its name.
    pub name: String,
    /// Its value.
    pub value: String,
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl HarNameValue {
    fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_owned(),
            value: value.to_owned(),
            comment: None,
        }
    }
}

/// A cookie sent or set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct HarCookie {
    /// Its name.
    pub name: String,
    /// Its value.
    pub value: String,
    /// Its `Path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Its `Domain`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Its `Expires`, as the server wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    /// Whether it is `HttpOnly`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http_only: Option<bool>,
    /// Whether it is `Secure`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secure: Option<bool>,
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

impl HarCookie {
    fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_owned(),
            value: value.to_owned(),
            path: None,
            domain: None,
            expires: None,
            http_only: None,
            secure: None,
            comment: None,
        }
    }
}

/// The body of a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct HarPostData {
    /// Its `Content-Type`.
    pub mime_type: String,
    /// The body (binary bytes are replaced as in lossy UTF-8).
    pub text: String,
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// The body of a response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct HarContent {
    /// Its size in bytes once decoded.
    pub size: i64,
    /// Bytes saved by compression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<i64>,
    /// Its MIME type.
    pub mime_type: String,
    /// The body, when [`HarOptions::content`] kept it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// `base64` when `text` is base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    /// A note, such as why the body was not kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// Cache information; this recorder leaves it empty, as HAR allows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HarCache {
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

/// The phases of a request in milliseconds, -1 for one that did not happen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HarTimings {
    /// Waiting in the queue and for a connection.
    pub blocked: f64,
    /// Resolving the host name.
    pub dns: f64,
    /// Opening the connection, TLS included.
    pub connect: f64,
    /// Sending the request.
    pub send: f64,
    /// Waiting for the first byte of the response.
    pub wait: f64,
    /// Receiving the response.
    pub receive: f64,
    /// The TLS handshake (also counted in `connect`).
    pub ssl: f64,
    /// A note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}
