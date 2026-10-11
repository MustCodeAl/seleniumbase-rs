//! Helpers over `sb_cdp` that navigate, detect the page, apply the locale, and move cookies and clearances.
//!
//! Each helper takes the [`Page`] it works on and nothing else, and reads no clock: a function that
//! needs the time takes `now` in seconds since the Unix epoch. They never switch on a protocol
//! domain a page can feel (see `tests/sb_cdp_domain_hygiene.rs`); the one domain they enable is
//! `Network`, which the fingerprint of the browser enables as well.
//!
//! - [`goto_with_response`] navigates and returns what the server answered the main frame.
//! - [`detect_page`] reads the rendered page and decides whether it is a challenge.
//! - [`EgressLocale`] and [`apply_locale`] make a tab speak the language and keep the time zone of
//!   the country its egress is in.
//! - [`export_cookies`] and [`import_cookies`] move cookies between the browser and a
//!   [`CookieJar`]; [`capture_session`] and [`restore_session`] do the same for a whole login.
//! - [`wait_for_clearance`] waits for a solved challenge to show in the browser.

use std::time::Duration;

use serde_json::{json, Value};

use crate::sb_cdp::{CdpEvent, Page, Session};
use crate::scrape::challenge::detect::{
    clearance_cookie, detect, ChallengeKind, Detection, ResponseView, Vendor,
};
use crate::scrape::egress::geo::{GeoError, GeoResolver, Locale};
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::egress::route::EgressRef;
use crate::scrape::error::ScrapeError;
use crate::scrape::jar::store::CookieJar;
use crate::stealth::behavior::Behavior;
use crate::stealth::fingerprint::Fingerprint;
use crate::utilities::retry::{poll_until, RetryPolicy};
use crate::utils::urls::normalize_page_url;

#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;

/// How often a wait for a clearance looks at the browser again.
pub const CLEARANCE_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// How far the pointer warm-up scrolls the page, in CSS pixels.
const WARM_UP_SCROLL_PIXELS: f64 = 60.0;

/// The protocol event that carries the answer to a request.
const RESPONSE_EVENT: &str = "Network.responseReceived";

/// The resource type of a navigation, as the protocol names it.
const DOCUMENT_TYPE: &str = "Document";

/// Why a URL the browser cannot load is refused, without echoing it.
const INVALID_URL: &str = "the URL is not valid";

/// What the server answered the main frame of a navigation.
#[derive(Clone, PartialEq, Eq)]
pub struct NavResponse {
    /// The URL that answered, after redirects.
    pub url: String,
    /// The HTTP status, or `0` when the browser reported no response (a download, say).
    pub status: u16,
    /// The response headers as (name, value) pairs; a header sent twice appears twice.
    pub headers: Vec<(String, String)>,
}

impl std::fmt::Debug for NavResponse {
    /// The URL redacted and the headers counted: a header can carry a cookie.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NavResponse")
            .field("url", &RedactedUrl::new(&self.url))
            .field("status", &self.status)
            .field("headers", &self.headers.len())
            .finish()
    }
}

/// The page as the browser rendered it, with the verdict on it.
#[derive(Clone, Debug)]
pub struct PageSnapshot {
    /// Whether the page is a challenge, and the evidence.
    pub detection: Detection,
    /// The page HTML, scripts' changes included.
    pub body: String,
}

/// Navigates `page` to `url` and returns what the server answered the main frame.
///
/// The load is awaited as [`Page::goto`] awaits it. Of the `Document` responses of the page's own
/// frame the last one is kept, so a redirect chain gives its end. When the browser reports none, the
/// result has status `0` and the URL asked for.
///
/// # Errors
///
/// Returns [`ScrapeError::Navigation`] (the URL redacted) when the browser cannot load the page, and
/// [`ScrapeError::Browser`] for any other protocol failure.
pub async fn goto_with_response(page: &Page, url: &str) -> Result<NavResponse, ScrapeError> {
    let url = normalize_page_url(url).map_err(|_error| ScrapeError::Navigation {
        url: RedactedUrl::new(url),
        reason: INVALID_URL.to_owned(),
    })?;
    page.execute("Network.enable", json!({})).await?;
    let mut events = page.events();
    let mut seen = None;
    {
        // Drained while the page loads: a heavy page announces more resources than an idle
        // subscriber can hold, and the answer to the document is the first of them.
        let navigation = page.goto(&url);
        tokio::pin!(navigation);
        loop {
            tokio::select! {
                biased;
                result = &mut navigation => {
                    result?;
                    break;
                }
                Some(event) = events.next() => note_response(page.id(), &event, &mut seen),
            }
        }
    }
    while let Ok(Some(event)) = tokio::time::timeout(Duration::ZERO, events.next()).await {
        note_response(page.id(), &event, &mut seen);
    }
    Ok(seen.unwrap_or(NavResponse {
        url,
        status: 0,
        headers: Vec::new(),
    }))
}

/// Reads the rendered page and decides whether it is a challenge, using the status and headers of
/// `response`.
///
/// # Errors
///
/// Returns [`ScrapeError::Browser`] when the page cannot be read.
pub async fn detect_page(page: &Page, response: &NavResponse) -> Result<PageSnapshot, ScrapeError> {
    let body = page.content().await?;
    let detection = detect(&ResponseView {
        url: &response.url,
        status: response.status,
        headers: &response.headers,
        body: &body,
    });
    Ok(PageSnapshot { detection, body })
}

/// The locale and time zone an egress puts a tab in.
#[derive(Clone, Debug)]
pub struct EgressLocale {
    /// The locale of the egress country.
    pub locale: &'static Locale,
    /// The IANA time zone: the resolver's when it knew one, else the locale's.
    pub timezone: String,
}

impl EgressLocale {
    /// Asks `resolver` where `egress` is and finds the locale of that country.
    ///
    /// # Errors
    ///
    /// Returns [`ScrapeError::Geo`] when the egress has no country or the country has no locale.
    pub async fn resolve(
        resolver: &dyn GeoResolver,
        egress: &EgressRef,
    ) -> Result<Self, ScrapeError> {
        let info = resolver.resolve(egress).await?;
        let locale = Locale::for_country(info.country).ok_or(GeoError::NoLocale(info.country))?;
        Ok(Self {
            locale,
            timezone: info.timezone.unwrap_or_else(|| locale.timezone.to_owned()),
        })
    }

    /// Sets the locale fields of `fingerprint` (language, `Accept-Language`, time zone, position)
    /// from this locale, with the time zone this egress reported.
    pub fn apply_to(&self, fingerprint: &mut Fingerprint) {
        self.locale.apply_to(fingerprint);
        fingerprint.timezone = Some(self.timezone.clone());
    }
}

/// Makes `page` report the locale and time zone of `placed` over the protocol, for the documents it
/// loads from now on.
///
/// This changes `Intl` and the time zone as the browser reports them. The `Accept-Language` header
/// and `navigator.languages` of a tab come from the fingerprint the browser was launched with. A
/// tab of a browser launched with a fingerprint also has a script written into each document that
/// sets the time zone itself, and that script runs after this override: such a tab keeps the time
/// zone of its fingerprint. The browser scraper therefore starts the browser again for an egress in
/// another country; this function serves a tab that has no fingerprint script.
///
/// # Errors
///
/// Returns [`ScrapeError::Browser`] when the browser refuses an override.
pub async fn apply_locale(page: &Page, placed: &EgressLocale) -> Result<(), ScrapeError> {
    let emulation = page.emulation();
    emulation.locale(placed.locale.tag).await?;
    emulation.timezone(&placed.timezone).await?;
    Ok(())
}

/// The cookies of the tab's browser context, as a jar. Cookies the jar would refuse are left out.
///
/// # Errors
///
/// Returns [`ScrapeError::Browser`] when the browser cannot be queried.
pub async fn export_cookies(page: &Page, now: u64) -> Result<CookieJar, ScrapeError> {
    Ok(CookieJar::from_cdp(&page.cookies().all().await?, now))
}

/// Sets the unexpired cookies of `jar` in the tab's browser context and returns how many.
///
/// # Errors
///
/// Returns [`ScrapeError::Browser`] when the browser refuses them.
pub async fn import_cookies(page: &Page, jar: &CookieJar, now: u64) -> Result<usize, ScrapeError> {
    let cookies = jar.to_cdp(now);
    if !cookies.is_empty() {
        page.cookies().set(&cookies).await?;
    }
    Ok(cookies.len())
}

/// The value of the clearance cookie of `vendor` in the tab's browser context, when there is one.
/// A vendor without such a cookie gives `None`.
///
/// # Errors
///
/// Returns [`ScrapeError::Browser`] when the browser cannot be queried.
pub async fn clearance_value(
    page: &Page,
    vendor: Option<Vendor>,
) -> Result<Option<String>, ScrapeError> {
    let Some(name) = vendor.and_then(clearance_cookie) else {
        return Ok(None);
    };
    let cookies = page.cookies().all().await?;
    Ok(cookies
        .into_iter()
        .find(|cookie| cookie.name == name && !cookie.value.is_empty())
        .map(|cookie| cookie.value))
}

/// Waits until a solved challenge shows in the browser, for at most `timeout`.
///
/// For a vendor with a clearance cookie the wait ends when that cookie holds a value other than
/// `stale` (the value read before the solve), or holds the same value while the page no longer
/// shows a challenge (a script that set the cookie while the page loaded). A vendor without a
/// clearance cookie ends when the page no longer shows a challenge. Returns whether that
/// happened in time.
///
/// # Errors
///
/// Returns [`ScrapeError::Browser`] when the browser cannot be queried.
pub async fn wait_for_clearance(
    page: &Page,
    vendor: Option<Vendor>,
    stale: Option<&str>,
    timeout: Duration,
) -> Result<bool, ScrapeError> {
    let polled = poll_until(
        timeout,
        RetryPolicy::every(CLEARANCE_POLL_INTERVAL),
        || async {
            match is_cleared(page, vendor, stale).await {
                Ok(false) => None,
                Ok(true) => Some(Ok(())),
                Err(error) => Some(Err(error)),
            }
        },
    )
    .await;
    polled.transpose().map(|done| done.is_some())
}

/// Whether one look at the browser shows a solved challenge. See [`wait_for_clearance`].
async fn is_cleared(
    page: &Page,
    vendor: Option<Vendor>,
    stale: Option<&str>,
) -> Result<bool, ScrapeError> {
    if vendor.and_then(clearance_cookie).is_some() {
        let Some(fresh) = clearance_value(page, vendor).await? else {
            return Ok(false);
        };
        if stale != Some(fresh.as_str()) {
            return Ok(true);
        }
        // A challenge script that ran while the page loaded sets its cookie before the caller
        // reads the "stale" value, so an unchanged cookie is cleared too once the page no longer
        // shows a challenge.
    }
    let body = page.content().await?;
    let seen = detect(&ResponseView {
        url: "",
        status: 0,
        headers: &[],
        body: &body,
    });
    Ok(seen.kind == ChallengeKind::None)
}

/// Captures the cookies of the tab's browser context and the local storage of the tab's origin as a
/// [`Session`], the way a pool lease saves its login.
///
/// # Errors
///
/// Returns [`ScrapeError::Browser`] when the browser cannot be queried.
pub async fn capture_session(page: &Page) -> Result<Session, ScrapeError> {
    let mut session = Session::new(page.cookies().all().await?);
    let origin: String = page.evaluate_as("location.origin").await?;
    if origin.starts_with("http") {
        let entries = page.local_storage().entries().await?;
        if !entries.is_empty() {
            session = session.with_local_storage(origin, entries);
        }
    }
    Ok(session)
}

/// Puts a [`Session`] into the tab: its unexpired cookies, and its local storage when the tab is on
/// the origin it was captured from.
///
/// # Errors
///
/// Returns [`ScrapeError::Browser`] when the browser refuses.
pub async fn restore_session(page: &Page, session: Session, now: f64) -> Result<(), ScrapeError> {
    let session = session.without_expired(now);
    page.cookies().set(session.cookies()).await?;
    if let Some(origin) = session.origin() {
        let here: String = page.evaluate_as("location.origin").await?;
        if here == origin {
            let storage = page.local_storage();
            for (key, value) in session.local_storage() {
                storage.set(key, value).await?;
            }
        }
    }
    Ok(())
}

/// Lets the pointer arrive on the page the way a person's would.
///
/// The pointer is placed somewhere plausible and the page is scrolled a little, with the pace of
/// [`Page::human`]. It is cosmetic, so a failure is logged and does not stop the caller.
pub async fn human_warm_up(page: &Page, seed: u64) {
    let behavior = match Behavior::builder().seed(seed).build() {
        Ok(behavior) => behavior,
        Err(error) => {
            tracing::debug!(%error, "no human behavior could be built for the warm-up");
            return;
        }
    };
    if let Err(error) = page.human(behavior).scroll_by(WARM_UP_SCROLL_PIXELS).await {
        tracing::debug!(%error, "the pointer warm-up failed");
    }
}

/// Reads the answer to the main-frame navigation out of one protocol event.
fn note_response(main_frame: &str, event: &CdpEvent, seen: &mut Option<NavResponse>) {
    if event.method != RESPONSE_EVENT {
        return;
    }
    let params = &event.params;
    // The main frame of a tab has the id of the tab's target.
    let ours = params["frameId"].as_str() == Some(main_frame);
    if !ours || params["type"].as_str() != Some(DOCUMENT_TYPE) {
        return;
    }
    let response = &params["response"];
    *seen = Some(NavResponse {
        url: response["url"].as_str().unwrap_or_default().to_owned(),
        status: response["status"]
            .as_u64()
            .and_then(|status| u16::try_from(status).ok())
            .unwrap_or(0),
        headers: headers_of(&response["headers"]),
    });
}

/// The headers of a protocol `Headers` object as pairs.
fn headers_of(headers: &Value) -> Vec<(String, String)> {
    let Some(map) = headers.as_object() else {
        return Vec::new();
    };
    map.iter()
        .flat_map(|(name, value)| {
            // The protocol joins the values of a header sent twice with a newline.
            value
                .as_str()
                .unwrap_or_default()
                .split('\n')
                .map(move |part| (name.clone(), part.to_owned()))
        })
        .collect()
}
