//! Makes a browser report its client hints, and turns the report into [`ClientHints`].
//!
//! [`HintsServer`] is a web site on a loopback port with three routes:
//!
//! - `GET /` is a page that asks for the high-entropy hints (`Accept-CH` and `Critical-CH`), reads
//!   `navigator.userAgentData` and `navigator.languages`, fetches `/echo` and posts what it read to
//!   `/report`. The browser's own request headers (`User-Agent`, `Accept-Language`, the `Sec-CH-UA`
//!   family) are recorded for each request.
//! - `GET /echo` answers `204`; it exists so a same-origin sub-request is recorded too.
//! - `POST /report` takes the page's JSON.
//!
//! The page's values and the request headers describe one browser through two channels, so
//! [`measurement_from`] checks that they agree and fails when they do not.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::task::{JoinHandle, JoinSet};

use super::stream::{http1_response, read_http1_request, Http1Request};
use super::CaptureError;
use crate::sb_cdp::sync::locked;
use crate::stealth::fingerprint::{BrandVersion, ClientHints};

/// The hints the page asks for beyond the low-entropy ones the browser always sends.
pub const ACCEPT_CH: &str = "Sec-CH-UA-Arch, Sec-CH-UA-Bitness, Sec-CH-UA-Form-Factors, \
                             Sec-CH-UA-Full-Version, Sec-CH-UA-Full-Version-List, Sec-CH-UA-Model, \
                             Sec-CH-UA-Platform-Version, Sec-CH-UA-WoW64";

/// The high-entropy values `navigator.userAgentData.getHighEntropyValues` is asked for.
pub const HIGH_ENTROPY_KEYS: [&str; 8] = [
    "architecture",
    "bitness",
    "formFactors",
    "fullVersionList",
    "model",
    "platformVersion",
    "uaFullVersion",
    "wow64",
];

/// How long a request to the hints server may take to arrive.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// The most requests the server remembers; later ones are answered but not recorded.
const MAX_RECORDED_REQUESTS: usize = 256;
/// The most reports waiting to be collected; a page posts one per load.
const REPORT_QUEUE: usize = 16;
/// The path of the hints page.
const DOCUMENT_PATH: &str = "/";
/// A template for the collector; `__KEYS__` is replaced by [`HIGH_ENTROPY_KEYS`].
const COLLECT_TEMPLATE: &str = r"async function collectHints() {
  const data = navigator.userAgentData;
  const report = {
    userAgent: navigator.userAgent,
    languages: Array.from(navigator.languages),
    language: navigator.language,
    platform: navigator.platform,
    secureContext: window.isSecureContext,
    uaData: null,
  };
  if (data) {
    const high = await data.getHighEntropyValues([__KEYS__]);
    report.uaData = {
      brands: data.brands.map((b) => ({ brand: b.brand, version: b.version })),
      mobile: data.mobile,
      platform: data.platform,
      high: high,
    };
  }
  return report;
}";

/// A brand and version as `navigator.userAgentData` lists them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brand {
    /// The brand, such as `Chromium`.
    pub brand: String,
    /// The version, major only or full.
    pub version: String,
}

/// The high-entropy values of `navigator.userAgentData`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HighEntropy {
    /// The CPU architecture, such as `arm`.
    #[serde(default)]
    pub architecture: String,
    /// The CPU word size in bits, such as `64`.
    #[serde(default)]
    pub bitness: String,
    /// The form factors, such as `Desktop`.
    #[serde(default)]
    pub form_factors: Vec<String>,
    /// The brands with their full versions.
    #[serde(default)]
    pub full_version_list: Vec<Brand>,
    /// The device model, empty on a desktop.
    #[serde(default)]
    pub model: String,
    /// The operating system version.
    #[serde(default)]
    pub platform_version: String,
    /// The full browser version.
    #[serde(default)]
    pub ua_full_version: Option<String>,
    /// Whether a 32-bit browser runs on a 64-bit system.
    #[serde(default)]
    pub wow64: bool,
}

/// `navigator.userAgentData`, low- and high-entropy values together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UaData {
    /// The brands with their major versions.
    pub brands: Vec<Brand>,
    /// Whether the browser is a mobile one.
    pub mobile: bool,
    /// The platform, such as `macOS`.
    pub platform: String,
    /// The values only a script that asks for them gets.
    pub high: HighEntropy,
}

/// What the hints page read from the browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HintsReport {
    /// `navigator.userAgent`.
    pub user_agent: String,
    /// `navigator.languages`.
    pub languages: Vec<String>,
    /// `navigator.language`.
    pub language: String,
    /// `navigator.platform`.
    pub platform: String,
    /// `window.isSecureContext`.
    pub secure_context: bool,
    /// `navigator.userAgentData`, absent in browsers that have none or outside secure contexts.
    pub ua_data: Option<UaData>,
}

/// What the hints capture learned about a browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HintsMeasurement {
    /// The `User-Agent` the browser sends and `navigator.userAgent` reports.
    pub user_agent: String,
    /// The `Accept-Language` request header.
    pub accept_language: String,
    /// `navigator.languages`.
    pub languages: Vec<String>,
    /// The client hints in the shape the profile tables use.
    pub client_hints: ClientHints,
    /// The headers of the first document request, before the browser knew what the page asks for.
    pub first_visit_headers: Vec<(String, String)>,
    /// The headers of the last document request, after the browser had seen `Accept-CH`.
    pub request_headers: Vec<(String, String)>,
}

/// The JavaScript that reads the browser's identity: defines `collectHints()`.
#[must_use]
pub fn collect_js() -> String {
    let keys: Vec<String> = HIGH_ENTROPY_KEYS
        .iter()
        .map(|key| format!("\"{key}\""))
        .collect();
    COLLECT_TEMPLATE.replace("__KEYS__", &keys.join(", "))
}

/// The page the hints server serves at `/`.
#[must_use]
pub fn page_html() -> String {
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>hints</title></head>\n\
         <body><pre id=\"out\">collecting</pre>\n<script>\n{}\n\
         (async () => {{\n  const out = document.getElementById(\"out\");\n  try {{\n    \
         await fetch(\"/echo\", {{ cache: \"no-store\" }});\n    \
         const report = await collectHints();\n    \
         await fetch(\"/report\", {{ method: \"POST\", \
         headers: {{ \"content-type\": \"application/json\" }}, \
         body: JSON.stringify(report) }});\n    out.textContent = \"reported\";\n  }} catch (error) {{\n    \
         out.textContent = String(error);\n  }}\n}})();\n</script></body></html>\n",
        collect_js()
    )
}

/// A snippet to paste into the developer console of a browser this tool cannot drive: it prints the
/// same JSON the page posts.
#[must_use]
pub fn console_snippet() -> String {
    format!(
        "(async () => {{\n{}\nconsole.log(JSON.stringify(await collectHints(), null, 2));\n}})();",
        collect_js()
    )
}

fn bad_report(reason: &'static str) -> CaptureError {
    CaptureError::HintsReport { reason }
}

/// Reads one structured-field string from the start of `input`: the text inside the quotes and
/// what follows the closing quote.
fn parse_quoted(input: &str) -> Option<(String, &str)> {
    let rest = input.strip_prefix('"')?;
    let mut text = String::new();
    let mut chars = rest.char_indices();
    while let Some((index, ch)) = chars.next() {
        match ch {
            '"' => return Some((text, rest.get(index + 1..)?)),
            '\\' => text.push(chars.next()?.1),
            _ => text.push(ch),
        }
    }
    None
}

/// Steps past the comma between two list items; `None` means the list ended.
fn next_item<'a>(after: &'a str, what: &'static str) -> Result<Option<&'a str>, CaptureError> {
    let after = after.trim_start();
    if after.is_empty() {
        return Ok(None);
    }
    match after.strip_prefix(',').map(str::trim_start) {
        Some(next) if !next.is_empty() => Ok(Some(next)),
        _ => Err(bad_report(what)),
    }
}

/// Parses a structured-field list of `"brand";v="version"` items, as `Sec-CH-UA` is written.
///
/// # Errors
///
/// Returns [`CaptureError::HintsReport`] when the value is not such a list.
pub fn parse_brand_list(value: &str) -> Result<Vec<BrandVersion>, CaptureError> {
    const WHAT: &str = "a brand list that is not `\"brand\";v=\"version\"` items";
    let mut brands = Vec::new();
    let mut rest = Some(value.trim()).filter(|rest| !rest.is_empty());
    while let Some(item) = rest {
        let (brand, after) = parse_quoted(item).ok_or(bad_report(WHAT))?;
        let param = after
            .trim_start()
            .strip_prefix(';')
            .map(str::trim_start)
            .and_then(|param| param.strip_prefix("v="))
            .ok_or(bad_report(WHAT))?;
        let (version, after) = parse_quoted(param).ok_or(bad_report(WHAT))?;
        brands.push(BrandVersion::new(brand, version));
        rest = next_item(after, WHAT)?;
    }
    Ok(brands)
}

/// Parses a structured-field list of strings, such as `"Desktop", "EmbeddedDevice"`.
///
/// # Errors
///
/// Returns [`CaptureError::HintsReport`] when the value is not such a list.
pub fn parse_string_list(value: &str) -> Result<Vec<String>, CaptureError> {
    const WHAT: &str = "a list of strings that are not quoted";
    let mut items = Vec::new();
    let mut rest = Some(value.trim()).filter(|rest| !rest.is_empty());
    while let Some(item) = rest {
        let (text, after) = parse_quoted(item).ok_or(bad_report(WHAT))?;
        items.push(text);
        rest = next_item(after, WHAT)?;
    }
    Ok(items)
}

/// Reads a header value that is one quoted string.
fn parse_string(value: &str) -> Result<String, CaptureError> {
    match parse_quoted(value.trim()) {
        Some((text, rest)) if rest.trim().is_empty() => Ok(text),
        _ => Err(bad_report("a header value that is not one quoted string")),
    }
}

/// Reads a structured-field boolean: `?1` or `?0`.
fn parse_bool(value: &str) -> Result<bool, CaptureError> {
    match value.trim() {
        "?1" => Ok(true),
        "?0" => Ok(false),
        _ => Err(bad_report("a header value that is not `?0` or `?1`")),
    }
}

/// The version of the brand that names the browser itself.
fn browser_version(list: &[Brand]) -> Option<String> {
    ["Google Chrome", "Chromium"].iter().find_map(|name| {
        list.iter()
            .find(|entry| entry.brand == *name)
            .map(|entry| entry.version.clone())
    })
}

fn brand_versions(list: &[Brand]) -> Vec<BrandVersion> {
    list.iter()
        .map(|entry| BrandVersion::new(entry.brand.clone(), entry.version.clone()))
        .collect()
}

/// Turns the page's report into the profile's [`ClientHints`].
///
/// # Errors
///
/// Returns [`CaptureError::NoUserAgentData`] when the browser has none and
/// [`CaptureError::HintsReport`] when it reports no full version.
pub fn client_hints_from(report: &HintsReport) -> Result<ClientHints, CaptureError> {
    let data = report
        .ua_data
        .as_ref()
        .ok_or(CaptureError::NoUserAgentData)?;
    let high = &data.high;
    let ua_full_version = high
        .ua_full_version
        .clone()
        .or_else(|| browser_version(&high.full_version_list))
        .ok_or(bad_report("no full browser version"))?;
    Ok(ClientHints {
        brands: brand_versions(&data.brands),
        full_version_list: brand_versions(&high.full_version_list),
        platform: data.platform.clone(),
        platform_version: high.platform_version.clone(),
        architecture: high.architecture.clone(),
        bitness: high.bitness.clone(),
        model: high.model.clone(),
        ua_full_version,
        mobile: data.mobile,
        wow64: high.wow64,
        form_factors: high.form_factors.clone(),
    })
}

/// Checks the hints against the `Sec-CH-UA` request headers the browser sent. A header the
/// browser did not send is skipped; one it sent must say the same.
///
/// # Errors
///
/// Returns [`CaptureError::HintsDisagree`] naming the first header that differs, and
/// [`CaptureError::HintsReport`] when a header cannot be read.
pub fn cross_check(hints: &ClientHints, headers: &[(String, String)]) -> Result<(), CaptureError> {
    let mut verdicts: Vec<(&str, Result<bool, CaptureError>)> = Vec::new();
    let mut check = |name: &'static str, same: &dyn Fn(&str) -> Result<bool, CaptureError>| {
        let sent = headers
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name));
        if let Some((_, value)) = sent {
            verdicts.push((name, same(value)));
        }
    };
    check("sec-ch-ua", &|v| Ok(parse_brand_list(v)? == hints.brands));
    check("sec-ch-ua-mobile", &|v| Ok(parse_bool(v)? == hints.mobile));
    check("sec-ch-ua-platform", &|v| {
        Ok(parse_string(v)? == hints.platform)
    });
    check("sec-ch-ua-full-version-list", &|v| {
        Ok(parse_brand_list(v)? == hints.full_version_list)
    });
    check("sec-ch-ua-full-version", &|v| {
        Ok(parse_string(v)? == hints.ua_full_version)
    });
    check("sec-ch-ua-platform-version", &|v| {
        Ok(parse_string(v)? == hints.platform_version)
    });
    check("sec-ch-ua-arch", &|v| {
        Ok(parse_string(v)? == hints.architecture)
    });
    check("sec-ch-ua-bitness", &|v| {
        Ok(parse_string(v)? == hints.bitness)
    });
    check("sec-ch-ua-model", &|v| Ok(parse_string(v)? == hints.model));
    check("sec-ch-ua-wow64", &|v| Ok(parse_bool(v)? == hints.wow64));
    check("sec-ch-ua-form-factors", &|v| {
        Ok(parse_string_list(v)? == hints.form_factors)
    });
    for (header, verdict) in verdicts {
        if !verdict? {
            return Err(CaptureError::HintsDisagree {
                header: header.to_owned(),
            });
        }
    }
    Ok(())
}

/// Builds the measurement from the page's report and the requests the server recorded.
///
/// The first document request (`GET /`) is the browser's first visit; the last one carries every
/// hint the page asked for.
///
/// # Errors
///
/// Returns [`CaptureError::Headless`] for a headless browser, [`CaptureError::HintsReport`] when no
/// document request or no `Accept-Language` was recorded or the browser sent no high-entropy hints
/// after `Accept-CH`, and the errors of [`client_hints_from`] and [`cross_check`].
pub fn measurement_from(
    report: &HintsReport,
    requests: &[Http1Request],
) -> Result<HintsMeasurement, CaptureError> {
    if report.user_agent.contains("Headless") {
        return Err(CaptureError::Headless);
    }
    let documents: Vec<&Http1Request> = requests
        .iter()
        .filter(|request| request.method == "GET" && request.path() == DOCUMENT_PATH)
        .collect();
    let (Some(first), Some(last)) = (documents.first(), documents.last()) else {
        return Err(bad_report("no document request"));
    };
    if last.header("sec-ch-ua-platform-version").is_none() {
        return Err(bad_report(
            "no high-entropy hints in the last document request",
        ));
    }
    if last.header("user-agent") != Some(report.user_agent.as_str()) {
        return Err(CaptureError::HintsDisagree {
            header: "user-agent".to_owned(),
        });
    }
    let accept_language = last.header("accept-language").ok_or(bad_report(
        "no Accept-Language in the last document request",
    ))?;
    let client_hints = client_hints_from(report)?;
    cross_check(&client_hints, &last.headers)?;
    Ok(HintsMeasurement {
        user_agent: report.user_agent.clone(),
        accept_language: accept_language.to_owned(),
        languages: report.languages.clone(),
        client_hints,
        first_visit_headers: first.headers.clone(),
        request_headers: last.headers.clone(),
    })
}

/// State shared by the connections of a [`HintsServer`].
#[derive(Debug)]
struct Shared {
    requests: std::sync::Mutex<Vec<Http1Request>>,
    reports: mpsc::Sender<Vec<u8>>,
}

impl Shared {
    /// Records `request` and builds the answer to it.
    fn respond(&self, request: Http1Request) -> Vec<u8> {
        let answer = match (request.method.as_str(), request.path()) {
            ("GET", DOCUMENT_PATH) => http1_response(
                200,
                "OK",
                &[
                    ("Content-Type", "text/html; charset=utf-8"),
                    ("Cache-Control", "no-store"),
                    ("Accept-CH", ACCEPT_CH),
                    ("Critical-CH", ACCEPT_CH),
                ],
                page_html().as_bytes(),
            ),
            ("GET", "/echo") => http1_response(204, "No Content", &[], b""),
            ("POST", "/report") => match self.reports.try_send(request.body.clone()) {
                Ok(()) => http1_response(204, "No Content", &[], b""),
                Err(_full) => http1_response(503, "Service Unavailable", &[], b""),
            },
            _ => http1_response(404, "Not Found", &[], b""),
        };
        let mut recorded = locked(&self.requests);
        if recorded.len() < MAX_RECORDED_REQUESTS {
            recorded.push(request);
        }
        answer
    }
}

/// A web site on a loopback port that makes a browser report its client hints.
#[derive(Debug)]
pub struct HintsServer {
    addr: SocketAddr,
    shared: Arc<Shared>,
    reports: Mutex<mpsc::Receiver<Vec<u8>>>,
    accept: JoinHandle<()>,
}

/// Accepts connections and answers each in its own task, until the task is aborted.
async fn accept_loop(listener: TcpListener, shared: Arc<Shared>) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _peer)) => {
                    connections.spawn(serve_connection(stream, Arc::clone(&shared)));
                }
                Err(error) => {
                    tracing::debug!(%error, "the hints server could not accept");
                    return;
                }
            },
            Some(_finished) = connections.join_next() => {}
        }
    }
}

/// Reads one request, answers it and closes the connection.
async fn serve_connection(mut stream: TcpStream, shared: Arc<Shared>) {
    let request = match read_http1_request(&mut stream, REQUEST_TIMEOUT).await {
        Ok(request) => request,
        Err(error) => {
            tracing::debug!(%error, "the hints server dropped a connection");
            return;
        }
    };
    let answer = shared.respond(request);
    if let Err(error) = stream.write_all(&answer).await {
        tracing::debug!(%error, "the hints server could not answer");
        return;
    }
    if let Err(error) = stream.shutdown().await {
        tracing::debug!(%error, "the hints server could not close a connection");
    }
}

impl HintsServer {
    /// Starts serving on `127.0.0.1` and a port the system picks.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Io`] when the socket cannot be bound.
    pub async fn start() -> Result<Self, CaptureError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        let (reports_tx, reports_rx) = mpsc::channel(REPORT_QUEUE);
        let shared = Arc::new(Shared {
            requests: std::sync::Mutex::new(Vec::new()),
            reports: reports_tx,
        });
        let accept = tokio::spawn(accept_loop(listener, Arc::clone(&shared)));
        Ok(Self {
            addr,
            shared,
            reports: Mutex::new(reports_rx),
            accept,
        })
    }

    /// The address the server listens on.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// The URL of the hints page. It is on `127.0.0.1`, which browsers treat as a secure context.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    /// Every request the server has answered, in arrival order.
    #[must_use]
    pub fn requests(&self) -> Vec<Http1Request> {
        locked(&self.shared.requests).clone()
    }

    /// Waits for the next report the page posts.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError::Timeout`] when none arrives within `within` and
    /// [`CaptureError::HintsReport`] when the body is not a [`HintsReport`].
    pub async fn next_report(&self, within: Duration) -> Result<HintsReport, CaptureError> {
        let timed_out = || CaptureError::Timeout {
            what: "a hints report",
        };
        let body = tokio::time::timeout(within, self.reports.lock().await.recv())
            .await
            .map_err(|_elapsed| timed_out())?
            .ok_or_else(timed_out)?;
        serde_json::from_slice(&body)
            .map_err(|_invalid| bad_report("a body that is not the expected JSON"))
    }
}

impl Drop for HintsServer {
    fn drop(&mut self) {
        self.accept.abort();
    }
}

#[cfg(test)]
mod tests;
