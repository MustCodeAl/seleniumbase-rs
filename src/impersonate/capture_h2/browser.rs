//! Drives the local Chrome through one pass of the HTTP/2 and header capture.
//!
//! The browser is pointed at the capture site's navigation address, then a script run in that page
//! makes it request the other probes. The site records the first request on each connection while
//! the browser works, so both sides run together.

use serde_json::json;

use super::probes::{page_script, DOCUMENT, PROBES};
use super::site::{Observed, Protocol, Site};
use super::CaptureH2Error;
use crate::sb_cdp::{Browser, LaunchOptions, Page};
use crate::scrape::runtime::capture::chrome::{host_resolver_rule, ChromeOptions};
use crate::scrape::runtime::capture::record::BrowserVersion;
use crate::scrape::runtime::capture::CaptureError;

/// Starts a browser that trusts the site's CA and sends every capture host to loopback, makes it
/// request every probe over `protocol`, and returns its version and what the site saw.
///
/// The browser is closed before this returns, whatever the outcome.
///
/// # Errors
///
/// Returns a [`CaptureH2Error`] when the site or the browser cannot be started, the browser is
/// headless, or the page cannot be driven. Probes the browser never requested are not an error
/// here; the evidence step reports them.
pub async fn run_pass(
    options: &ChromeOptions,
    protocol: Protocol,
) -> Result<(BrowserVersion, Vec<Observed>), CaptureH2Error> {
    let site = Site::bind(protocol, options.timeout).await?;
    let args = [host_resolver_rule(), site.chrome_arg()];
    let browser = Browser::launch(launch_options(options, &args)?)
        .await
        .map_err(CaptureError::from)?;
    let outcome = probe(&browser, &site, options).await;
    if let Err(error) = browser.close().await {
        tracing::warn!(%error, "closing the measured browser failed");
    }
    outcome
}

/// The launch options for a measured browser, with `extra` Chrome arguments.
pub(super) fn launch_options(
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

/// Whether every probe has been seen at least once.
pub(super) fn all_probes_seen(observed: &[Observed]) -> bool {
    PROBES
        .iter()
        .all(|probe| observed.iter().any(|one| one.probe == *probe))
}

/// Asks the browser for its version, then drives the page while the site records.
async fn probe(
    browser: &Browser,
    site: &Site,
    options: &ChromeOptions,
) -> Result<(BrowserVersion, Vec<Observed>), CaptureH2Error> {
    let result = browser
        .execute("Browser.getVersion", json!({}))
        .await
        .map_err(CaptureError::from)?;
    let version = BrowserVersion::from_get_version(&result)?;
    if version.is_headless() {
        return Err(CaptureError::Headless.into());
    }
    let page = browser
        .default_page()
        .await
        .map_err(CaptureError::from)?
        .with_timeout(options.timeout);
    let (observed, driven) = tokio::join!(
        site.collect_until(options.timeout, all_probes_seen),
        drive(&page, site.port())
    );
    driven?;
    Ok((version, observed))
}

/// Opens the navigation address, then makes the page request the other probes.
async fn drive(page: &Page, port: u16) -> Result<(), CaptureH2Error> {
    page.goto(&DOCUMENT.url(port))
        .await
        .map_err(CaptureError::from)?;
    page.evaluate(page_script(port))
        .await
        .map_err(CaptureError::from)?;
    Ok(())
}
