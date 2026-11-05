//! A [`rustwright`] client attached beside one of this project's handles.
//!
//! [`PlaywrightSidecar`] holds rustwright's handles to a browser that is already running and
//! to the one tab the handle it was attached beside is on. It wraps no Playwright call: the
//! stealthy browser is driven with this project's own API, and the sidecar is for what only
//! Playwright does. [`run`](PlaywrightSidecar::run) hands a closure the rustwright page on
//! Tokio's blocking pool, because rustwright's calls block the thread they run on.
//!
//! The sidecar never starts a browser and never closes one. It attaches over the DevTools
//! Protocol (`connect_over_cdp`), which is how Python SeleniumBase's *Stealthy Playwright
//! Mode* reaches its stealthy Chrome: the stealth comes from the browser that was launched, and
//! the attached client inherits it. Attach late and detach early: while it is attached,
//! rustwright is one more DevTools client of the browser.

use std::fmt;
use std::time::Duration;

use rustwright::{chromium, Browser, ConnectOptions, Error as RwError, Page};
use seleniumbase_rs::sb_cdp::DevToolsEndpoint;
use seleniumbase_rs::tokio;
use seleniumbase_rs::SeleniumBaseError;

use crate::target::{attachment_of, PlaywrightTarget, TabHints};

/// A [`rustwright`] client attached to a running Chromium, on one tab.
///
/// Create it with [`attach`](Self::attach) beside a handle of this project (an
/// [`sb_cdp::Page`](seleniumbase_rs::sb_cdp::Page) or a
/// [`BaseCase`](seleniumbase_rs::BaseCase)), or with [`connect`](Self::connect) for any other
/// Chromium's DevTools endpoint. Drive the browser with that handle's own API; reach for
/// [`run`](Self::run), [`page`](Self::page) and [`browser`](Self::browser) only for what
/// Playwright does and this project's API does not.
///
/// The attached page keeps whatever fingerprint, proxy and launch switches the browser was
/// started with; the sidecar changes none of them. [`close`](Self::close) only detaches, so a
/// browser that another handle owns is never shut down from here.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
/// use seleniumbase_rs::{AssertionApi, BrowserApi};
/// use seleniumbase_rs_playwright::PlaywrightSidecar;
///
/// # async fn demo() -> seleniumbase_rs::Result<()> {
/// let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
/// let mut page = browser.default_page().await?;
///
/// // This project's API on the stealthy browser.
/// page.open("https://example.com").await?;
/// page.assert_text("Example Domain", "h1").await?;
///
/// // Playwright only for what that API does not do, on the same tab.
/// let playwright = PlaywrightSidecar::attach(&page).await?;
/// // `title` stands in for a call only Playwright has: our own API reads it with
/// // `page.title()`, and the closure sees the same tab.
/// let ours = page.title().await?;
/// let theirs = playwright.run(|pw| pw.title(Default::default())).await?;
/// assert_eq!(ours, theirs);
/// println!("{theirs}");
///
/// playwright.close().await?; // detaches; the browser and the tab stay
/// browser.close().await
/// # }
/// ```
pub struct PlaywrightSidecar {
    browser: Browser,
    page: Page,
}

// rustwright's handles are not `Debug`, so the sidecar reports what it can tell.
impl fmt::Debug for PlaywrightSidecar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaywrightSidecar")
            .field("connected", &self.is_connected())
            .finish_non_exhaustive()
    }
}

/// How [`PlaywrightSidecar::open`] chooses its page among the browser's.
enum Pick {
    /// The first page the browser lists, or a new one if it has none.
    First,
    /// The page the hints name, or an error when they name none or several.
    Tab(TabHints),
}

/// A page the browser lists, as far as the choice of one needs to know it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    target_id: String,
    url: String,
}

impl PlaywrightSidecar {
    /// Attaches rustwright to the browser of `target`, on the tab `target` is on.
    ///
    /// The target is an [`sb_cdp::Page`](seleniumbase_rs::sb_cdp::Page) or a
    /// [`BaseCase`](seleniumbase_rs::BaseCase) ([`PlaywrightTarget`]). The tab is found by
    /// its DevTools target id, then by its URL when exactly one open page has it; the attach
    /// waits as long as the target waits by default. For a [`BaseCase`](seleniumbase_rs::BaseCase)
    /// on chromedriver the tab is the window the case is on, so call this after the case
    /// has opened the page you want.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Playwright`] if nothing answers at the endpoint, if the browser
    /// cannot be controlled, or if no single open page is the target's tab (two tabs that
    /// cannot be told apart are refused, with the URLs that are open, rather than guessed).
    /// The target's own errors: for a [`BaseCase`](seleniumbase_rs::BaseCase),
    /// [`SeleniumBaseError::Unsupported`] when its browser has no DevTools endpoint to share
    /// (Firefox, Safari) and [`SeleniumBaseError::SessionNotStarted`] after `quit`.
    pub async fn attach(target: &impl PlaywrightTarget) -> Result<Self, SeleniumBaseError> {
        let attachment = attachment_of(target).await?;
        Self::open(
            &attachment.endpoint,
            Some(attachment.timeout),
            Pick::Tab(attachment.hints),
        )
        .await
    }

    /// Attaches rustwright to any Chromium's DevTools endpoint, on the first page it lists
    /// (a new page when it has none).
    ///
    /// For a browser that no handle of this project drives, such as one started with
    /// `--remote-debugging-port`; with a handle, [`attach`](Self::attach) picks the right tab.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Playwright`] if nothing answers at the endpoint or the browser
    /// cannot be controlled.
    pub async fn connect(endpoint: &DevToolsEndpoint) -> Result<Self, SeleniumBaseError> {
        Self::open(endpoint, None, Pick::First).await
    }

    /// Attaches to `endpoint`, waiting `timeout` for the attach (rustwright's own default
    /// without one), and takes the page `pick` names.
    async fn open(
        endpoint: &DevToolsEndpoint,
        timeout: Option<Duration>,
        pick: Pick,
    ) -> Result<Self, SeleniumBaseError> {
        let mut options = ConnectOptions::new(endpoint.http_url());
        if let Some(timeout) = timeout {
            options = options.timeout(timeout);
        }
        let (browser, page) = blocking(move || {
            let browser = chromium()
                .connect_over_cdp(options)
                .map_err(|e| SeleniumBaseError::playwright(format!("attach failed: {e}")))?;
            match page_for(&browser, &pick) {
                Ok(page) => Ok((browser, page)),
                Err(error) => {
                    // Detach the client that was just attached; the browser stays.
                    let _ = browser.close();
                    Err(error)
                }
            }
        })
        .await?;
        Ok(Self { browser, page })
    }

    /// The rustwright browser this sidecar is attached to.
    #[must_use]
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    /// The rustwright page this sidecar is on. It is a cheap handle: clone it to move it
    /// elsewhere. Its calls block; see [`run`](Self::run).
    #[must_use]
    pub fn page(&self) -> &Page {
        &self.page
    }

    /// Whether the connection to the browser is still open.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.browser.is_connected()
    }

    /// Runs `work` with the rustwright page on Tokio's blocking pool and returns its result.
    ///
    /// rustwright's page calls block the thread they run on; this is the way to use any of
    /// them from async code without stalling the runtime.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::BrowserDisconnected`] when the connection or the page is gone,
    /// [`SeleniumBaseError::WaitTimeout`] when a rustwright call times out, and
    /// [`SeleniumBaseError::Playwright`] with the text of the rustwright error for anything
    /// else. A panic in `work` is a [`SeleniumBaseError::Playwright`] too.
    pub async fn run<T, F>(&self, work: F) -> Result<T, SeleniumBaseError>
    where
        F: FnOnce(Page) -> Result<T, RwError> + Send + 'static,
        T: Send + 'static,
    {
        let page = self.page.clone();
        blocking(move || work(page).map_err(|error| classify_run_error(&error))).await
    }

    /// Detaches this client. The browser and its tabs stay: the browser is shut down by
    /// whoever started it (for one from [`sb_cdp`](seleniumbase_rs::sb_cdp), its own
    /// `close`). Closing a sidecar whose connection has already ended is `Ok`.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::Playwright`] if the connection does not close cleanly.
    pub async fn close(&self) -> Result<(), SeleniumBaseError> {
        let browser = self.browser.clone();
        blocking(move || {
            if !browser.is_connected() {
                return Ok(());
            }
            match browser.close() {
                Ok(())
                | Err(RwError::Closed | RwError::Disconnected | RwError::TargetClosed(_)) => Ok(()),
                Err(e) => Err(SeleniumBaseError::playwright(format!("close failed: {e}"))),
            }
        })
        .await
    }
}

/// The page of `browser` that `pick` names.
fn page_for(browser: &Browser, pick: &Pick) -> Result<Page, SeleniumBaseError> {
    let pages = browser
        .pages()
        .map_err(|e| SeleniumBaseError::playwright(format!("listing pages failed: {e}")))?;
    match pick {
        Pick::First => match pages.into_iter().next() {
            Some(page) => Ok(page),
            None => browser
                .new_page()
                .map_err(|e| SeleniumBaseError::playwright(format!("new page failed: {e}"))),
        },
        Pick::Tab(hints) => {
            let candidates: Vec<Candidate> = pages
                .iter()
                .map(|page| Candidate {
                    target_id: page.target_id(),
                    url: page.url(),
                })
                .collect();
            let index = pick_page(&candidates, hints)?;
            pages.into_iter().nth(index).ok_or_else(|| {
                SeleniumBaseError::playwright("the page that was picked is no longer open")
            })
        }
    }
}

/// The index in `candidates` of the page `hints` name.
///
/// A page whose target id is the id hint wins; else the one page whose URL is the URL
/// hint. It never guesses: no match, or two pages with the same URL and no id to tell them
/// apart, is an error that names the URLs that are open.
fn pick_page(candidates: &[Candidate], hints: &TabHints) -> Result<usize, SeleniumBaseError> {
    if let Some(id) = hints.target_id.as_deref() {
        let mut by_id = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.target_id == id);
        if let (Some((index, _)), None) = (by_id.next(), by_id.next()) {
            return Ok(index);
        }
    }
    if let Some(url) = hints.url.as_deref() {
        let mut by_url = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.url == url);
        if let (Some((index, _)), None) = (by_url.next(), by_url.next()) {
            return Ok(index);
        }
    }
    let open = if candidates.is_empty() {
        "the browser has no open page".to_owned()
    } else {
        let urls: Vec<&str> = candidates.iter().map(|c| c.url.as_str()).collect();
        format!("open pages: {}", urls.join(", "))
    };
    let unknown = "unknown";
    Err(SeleniumBaseError::playwright(format!(
        "no single open page is the tab to attach to (target id {}, URL {}); {open}. \
         Give the tabs different URLs, or open the page in the tab first",
        hints.target_id.as_deref().unwrap_or(unknown),
        hints.url.as_deref().unwrap_or(unknown),
    )))
}

/// Runs `work` on Tokio's blocking pool, where rustwright's blocking calls belong.
///
/// A task that panics or is cancelled is reported as a [`SeleniumBaseError::Playwright`].
async fn blocking<T, F>(work: F) -> Result<T, SeleniumBaseError>
where
    F: FnOnce() -> Result<T, SeleniumBaseError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| SeleniumBaseError::playwright(format!("blocking task failed: {e}")))?
}

/// Maps a rustwright failure of a call made through [`PlaywrightSidecar::run`] to the errors
/// the rest of this project returns for the same trouble.
///
/// Only a lost browser or page (`BrowserDisconnected`) and a timeout (`WaitTimeout`) mean
/// the same here as everywhere else; every other failure stays `Playwright` with the text
/// rustwright gave. There is no selector to name, so no element error is made up.
fn classify_run_error(error: &RwError) -> SeleniumBaseError {
    match error {
        RwError::Closed
        | RwError::Disconnected
        | RwError::TargetClosed(_)
        | RwError::PageCrashed => SeleniumBaseError::browser_disconnected(error.to_string()),
        RwError::ActionTimeout(_) | RwError::Timeout(_) => {
            SeleniumBaseError::wait_timeout(format!("a Playwright call: {error}"), None)
        }
        RwError::ActionFailure(failure) => match classify_run_error(failure.cause()) {
            // The failure's own text says which action it was; keep it.
            SeleniumBaseError::Playwright(_) => {
                SeleniumBaseError::playwright(format!("page call failed: {error}"))
            }
            mapped => mapped,
        },
        other => SeleniumBaseError::playwright(format!("page call failed: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use rustwright::ActionabilityError;

    use super::*;

    fn candidate(target_id: &str, url: &str) -> Candidate {
        Candidate {
            target_id: target_id.to_owned(),
            url: url.to_owned(),
        }
    }

    fn hints(target_id: Option<&str>, url: Option<&str>) -> TabHints {
        TabHints {
            target_id: target_id.map(str::to_owned),
            url: url.map(str::to_owned),
        }
    }

    fn playwright_text(error: SeleniumBaseError) -> String {
        match error {
            SeleniumBaseError::Playwright(text) => text,
            other => panic!("expected a Playwright error, got {other:?}"),
        }
    }

    #[test]
    fn the_target_id_wins_over_the_url() {
        // Both tabs show the same URL: only the id tells them apart.
        let open = [candidate("A", "http://x/"), candidate("B", "http://x/")];

        assert_eq!(
            pick_page(&open, &hints(Some("B"), Some("http://x/"))).unwrap(),
            1
        );
        assert_eq!(pick_page(&open, &hints(Some("A"), None)).unwrap(), 0);
    }

    #[test]
    fn a_unique_url_picks_the_page_when_the_id_matches_none() {
        let open = [candidate("A", "http://x/"), candidate("B", "http://y/")];

        // No id hint, and an id that no page has (a window handle that is not a target id).
        assert_eq!(
            pick_page(&open, &hints(None, Some("http://y/"))).unwrap(),
            1
        );
        assert_eq!(
            pick_page(&open, &hints(Some("CDwindow-?"), Some("http://x/"))).unwrap(),
            0
        );
    }

    #[test]
    fn two_pages_that_cannot_be_told_apart_are_refused_naming_the_urls() {
        let open = [
            candidate("A", "http://same/"),
            candidate("B", "http://same/"),
            candidate("C", "http://other/"),
        ];

        let text =
            playwright_text(pick_page(&open, &hints(None, Some("http://same/"))).unwrap_err());

        assert!(
            text.contains("http://same/") && text.contains("http://other/"),
            "{text}"
        );
    }

    #[test]
    fn no_matching_page_is_refused_naming_the_urls_that_are_open() {
        let open = [candidate("A", "http://x/"), candidate("B", "http://y/")];

        let text =
            playwright_text(pick_page(&open, &hints(Some("Z"), Some("http://gone/"))).unwrap_err());

        assert!(
            text.contains("http://x/") && text.contains("http://y/"),
            "{text}"
        );
        assert!(text.contains("http://gone/"), "{text}");
    }

    #[test]
    fn no_hint_never_picks_a_page_not_even_the_only_one() {
        let only = [candidate("A", "http://x/")];

        assert!(pick_page(&only, &TabHints::default()).is_err());
        let text = playwright_text(pick_page(&[], &hints(Some("A"), None)).unwrap_err());
        assert!(text.contains("no open page"), "{text}");
    }

    #[test]
    fn a_lost_browser_or_page_is_browser_disconnected() {
        for error in [RwError::Closed, RwError::Disconnected, RwError::PageCrashed] {
            assert!(
                matches!(
                    classify_run_error(&error),
                    SeleniumBaseError::BrowserDisconnected { .. }
                ),
                "{error:?}"
            );
        }
    }

    #[test]
    fn a_timeout_is_a_wait_timeout() {
        assert!(matches!(
            classify_run_error(&RwError::Timeout(30_000)),
            SeleniumBaseError::WaitTimeout(_)
        ));
    }

    #[test]
    fn every_other_failure_stays_a_playwright_error_with_rustwrights_text() {
        // There is no selector, so a failed actionability check is not an element error.
        let not_visible =
            classify_run_error(&RwError::Actionability(ActionabilityError::NotVisible));
        assert!(
            matches!(not_visible, SeleniumBaseError::Playwright(_)),
            "{not_visible:?}"
        );

        let text = playwright_text(classify_run_error(&RwError::Message(
            "something else".to_owned(),
        )));
        assert!(text.contains("something else"), "{text}");
    }

    #[tokio::test]
    async fn an_endpoint_nothing_answers_at_is_reported_as_a_playwright_error() {
        // Port 1 is reserved and nothing listens there, so the attach is refused at once;
        // the short timeout keeps a slow refusal from holding the test.
        let endpoint =
            DevToolsEndpoint::new("http://127.0.0.1:1", "ws://127.0.0.1:1/devtools/browser/x");

        let error =
            PlaywrightSidecar::open(&endpoint, Some(Duration::from_millis(500)), Pick::First)
                .await
                .expect_err("there is no browser to attach to");

        assert!(
            matches!(&error, SeleniumBaseError::Playwright(text) if text.starts_with("attach failed")),
            "{error:?}"
        );
    }

    #[tokio::test]
    async fn blocking_work_that_panics_is_an_error_not_a_hang() {
        let outcome: Result<(), SeleniumBaseError> =
            blocking(|| panic!("rustwright blew up")).await;

        assert!(
            matches!(&outcome, Err(SeleniumBaseError::Playwright(text)) if text.contains("blocking task failed")),
            "{outcome:?}"
        );
    }
}
