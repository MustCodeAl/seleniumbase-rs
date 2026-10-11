//! Drives a local Chrome through `sb_cdp` and returns what it sent.
//!
//! [`measure_tls`] points a browser at a loopback listener that answers nothing, so every HTTPS
//! connection it makes ends after its `ClientHello`. A made-up host name is mapped to the listener
//! with `--host-resolver-rules`, which keeps the server name in the hello real. [`measure_hints`]
//! loads the [`HintsServer`](super::hints::HintsServer) page and collects what the browser reports.
//!
//! Both start a browser, so the tests that call them are ignored by default.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;

use super::hints::{measurement_from, HintsMeasurement, HintsReport, HintsServer};
use super::record::{BrowserVersion, TlsMeasurement};
use super::stream::{Capture, CapturedHello};
use super::CaptureError;
use crate::sb_cdp::{Browser, LaunchOptions, Page};

/// The domain every probe host lives under: `*.capture.test` is mapped to the loopback listener.
pub const CAPTURE_DOMAIN: &str = "capture.test";
/// The host name the WebSocket probe connects to, to tell its hello from the page hellos.
pub const WEBSOCKET_HOST: &str = "ws.capture.test";
/// How many ordinary HTTPS connections the TLS probe makes by default.
pub const DEFAULT_PAGE_PROBES: usize = 4;
/// How long a measurement waits for the browser by default.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The `Accept-Language` locale the browser is started with by default.
pub const DEFAULT_LANG: &str = "en-US";

/// How to start the browser that is measured.
#[derive(Debug, Clone)]
pub struct ChromeOptions {
    /// The browser binary; `None` finds the system Chrome.
    pub executable: Option<PathBuf>,
    /// Whether to run headless. A headless browser reports a different identity, so
    /// [`measure_hints`] refuses it.
    pub headless: bool,
    /// The `--lang` the browser is started with, which sets its `Accept-Language`.
    pub lang: String,
    /// How long to wait for the browser to connect or report.
    pub timeout: Duration,
    /// Whether to pass `--no-sandbox`, which Chrome needs as root in a container.
    pub no_sandbox: bool,
    /// How many ordinary HTTPS connections the TLS probe makes.
    pub page_probes: usize,
}

impl Default for ChromeOptions {
    fn default() -> Self {
        Self {
            executable: None,
            headless: false,
            lang: DEFAULT_LANG.to_owned(),
            timeout: DEFAULT_TIMEOUT,
            no_sandbox: false,
            page_probes: DEFAULT_PAGE_PROBES,
        }
    }
}

/// The fewest ordinary connections that show whether the browser shuffles its extensions.
const MIN_PAGE_HELLOS: usize = 2;

/// The Chrome argument that sends every `*.capture.test` host to the loopback interface.
#[must_use]
pub fn host_resolver_rule() -> String {
    format!("--host-resolver-rules=MAP *.{CAPTURE_DOMAIN} 127.0.0.1")
}

/// The script that makes the browser open `probes` HTTPS connections and one WebSocket to `port`
/// on the probe hosts. It does not wait for them, since none can complete.
#[must_use]
pub fn probe_script(port: u16, probes: usize) -> String {
    let mut lines: Vec<String> = (0..probes)
        .map(|index| {
            format!(
                "  fetch(\"https://h{index}.{CAPTURE_DOMAIN}:{port}/\", \
                 {{ mode: \"no-cors\", cache: \"no-store\" }}).catch(() => {{}});"
            )
        })
        .collect();
    lines.push(format!(
        "  try {{ const socket = new WebSocket(\"wss://{WEBSOCKET_HOST}:{port}/\"); \
         socket.onerror = () => {{}}; }} catch (error) {{}}"
    ));
    format!("(() => {{\n{}\n}})();", lines.join("\n"))
}

/// Splits the hellos of a TLS probe into the ordinary connections and the WebSocket one, by the
/// server name each asked for. Hellos for other names are dropped. Returns the raw bytes.
fn split_probe_hellos(hellos: &[CapturedHello]) -> (Vec<Vec<u8>>, Option<Vec<u8>>) {
    let mut page = Vec::new();
    let mut websocket = None;
    for captured in hellos {
        match captured.hello.sni.as_deref() {
            Some(WEBSOCKET_HOST) => {
                websocket.get_or_insert_with(|| captured.raw.clone());
            }
            Some(name) if name.ends_with(CAPTURE_DOMAIN) => page.push(captured.raw.clone()),
            _ => {}
        }
    }
    (page, websocket)
}

/// Whether the hellos so far are enough: `probes` ordinary connections and a WebSocket one.
fn probes_done(hellos: &[CapturedHello], probes: usize) -> bool {
    let (page, websocket) = split_probe_hellos(hellos);
    page.len() >= probes && websocket.is_some()
}

/// The launch options for a measured browser, with `extra` Chrome arguments.
fn launch_options(
    options: &ChromeOptions,
    extra: &[String],
) -> Result<LaunchOptions, CaptureError> {
    let mut builder = LaunchOptions::builder()
        .headless(options.headless)
        .lang(options.lang.clone())
        .no_sandbox(options.no_sandbox)
        .startup_timeout(options.timeout);
    if let Some(path) = &options.executable {
        builder = builder.executable_path(path.clone());
    }
    for arg in extra {
        builder = builder.arg(arg.clone());
    }
    Ok(builder.build()?)
}

/// Asks the browser for its version.
async fn browser_version(browser: &Browser) -> Result<BrowserVersion, CaptureError> {
    let result = browser.execute("Browser.getVersion", json!({})).await?;
    BrowserVersion::from_get_version(&result)
}

/// Closes the browser, logging a failure instead of hiding the measurement's own result.
async fn close(browser: &Browser) {
    if let Err(error) = browser.close().await {
        tracing::warn!(%error, "closing the measured browser failed");
    }
}

/// Measures the TLS `ClientHello` of the browser.
///
/// Returns the browser's version and what its hellos show. The browser is closed before this
/// returns, whatever the outcome.
///
/// # Errors
///
/// Returns a [`CaptureError`] when the browser cannot be started or driven, sends fewer hellos
/// than needed before `options.timeout`, or its hellos do not show one stable client.
pub async fn measure_tls(
    options: &ChromeOptions,
) -> Result<(BrowserVersion, TlsMeasurement), CaptureError> {
    let capture = Capture::bind().await?;
    let args = [host_resolver_rule()];
    let browser = Browser::launch(launch_options(options, &args)?).await?;
    let outcome = probe_tls(&browser, &capture, options).await;
    close(&browser).await;
    outcome
}

/// Makes an open browser connect to `capture` and derives the measurement from its hellos.
async fn probe_tls(
    browser: &Browser,
    capture: &Capture,
    options: &ChromeOptions,
) -> Result<(BrowserVersion, TlsMeasurement), CaptureError> {
    let version = browser_version(browser).await?;
    let page = browser.default_page().await?.with_timeout(options.timeout);
    let script = probe_script(capture.port(), options.page_probes);
    let probes = options.page_probes;
    let (hellos, fired) = tokio::join!(
        capture.collect_client_hellos_until(options.timeout, |so_far| probes_done(so_far, probes)),
        page.evaluate(script)
    );
    fired?;
    let (page_hellos, websocket) = split_probe_hellos(&hellos);
    if page_hellos.len() < MIN_PAGE_HELLOS {
        return Err(CaptureError::NoHello {
            what: "enough ordinary connections to compare their extension order",
        });
    }
    let measurement = TlsMeasurement::from_hellos(&page_hellos, websocket.as_deref())?;
    Ok((version, measurement))
}

/// Measures the client hints, `User-Agent` and `Accept-Language` of the browser.
///
/// # Errors
///
/// Returns a [`CaptureError`] when the browser cannot be started or driven, is headless, does not
/// report before `options.timeout`, or reports hints that disagree with its request headers.
pub async fn measure_hints(
    options: &ChromeOptions,
) -> Result<(BrowserVersion, HintsMeasurement), CaptureError> {
    let server = HintsServer::start().await?;
    let browser = Browser::launch(launch_options(options, &[])?).await?;
    let outcome = probe_hints(&browser, &server, options).await;
    close(&browser).await;
    outcome
}

/// Loads the hints page in an open browser and derives the measurement from what it reports.
async fn probe_hints(
    browser: &Browser,
    server: &HintsServer,
    options: &ChromeOptions,
) -> Result<(BrowserVersion, HintsMeasurement), CaptureError> {
    let version = browser_version(browser).await?;
    if version.is_headless() {
        return Err(CaptureError::Headless);
    }
    let page = browser.default_page().await?.with_timeout(options.timeout);
    let mut report = load_hints_page(&page, server, &server.url(), options.timeout).await?;
    if !has_full_hints(&server.requests()) {
        // The browser has seen `Accept-CH` now, so a second visit carries the hints.
        let again = format!("{}?again=1", server.url());
        report = load_hints_page(&page, server, &again, options.timeout).await?;
    }
    let measurement = measurement_from(&report, &server.requests())?;
    Ok((version, measurement))
}

/// Opens `url` and waits for the report the page posts.
async fn load_hints_page(
    page: &Page,
    server: &HintsServer,
    url: &str,
    within: Duration,
) -> Result<HintsReport, CaptureError> {
    page.goto(url).await?;
    server.next_report(within).await
}

/// Whether the last document request carried the high-entropy hints.
fn has_full_hints(requests: &[super::stream::Http1Request]) -> bool {
    requests
        .iter()
        .rfind(|request| request.method == "GET" && request.path() == "/")
        .is_some_and(|request| request.header("sec-ch-ua-platform-version").is_some())
}

#[cfg(test)]
mod tests;
