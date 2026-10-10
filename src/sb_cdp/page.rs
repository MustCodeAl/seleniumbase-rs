//! A single browser tab: navigation, scripting, capture and the handles for
//! everything you can do inside it.

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use super::browser::Browser;
use super::client::Events;
use super::locator::Locator;
use super::sync::locked;
use super::types::{PageInfo, Scroll, State};
use crate::error::SeleniumBaseError;
use crate::utils::selectors::SelectorBuf;
use crate::utils::urls::normalize_page_url;

/// The page helper library, injected into every document.
pub(crate) const HELPER_JS: &str = include_str!("helper.js");

/// How long interactions wait for an element, from SeleniumBase's
/// `SMALL_TIMEOUT`.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(7);

/// How long a navigation may take before it is reported as failed.
///
/// Slow pages on busy CI machines can legitimately take a long while to fire
/// their load event; a minute separates "slow" from "stuck".
const NAVIGATION_TIMEOUT: Duration = Duration::from_secs(60);

/// How long to keep checking `document.readyState` after a load event was
/// missed, which happens when the page finished before anything listened.
const LOAD_FALLBACK: Duration = Duration::from_secs(2);

/// How often waits re-check their condition. Short enough to feel instant,
/// long enough not to flood the protocol connection.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// One browser tab.
///
/// A `Page` is a cheap handle: cloning it gives another handle to the same
/// tab, and several can be used at once, from different tasks. Get one from a
/// [`Browser`].
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
///
/// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let browser = Browser::launch(LaunchOptions::default()).await?;
/// let page = browser.default_page().await?;
/// page.goto("https://seleniumbase.io").await?;
/// println!("{}", page.title().await?);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Page {
    browser: Browser,
    id: Arc<str>,
    session: Arc<str>,
    timeout: Duration,
}

impl Page {
    pub(crate) fn new(
        browser: Browser,
        id: Arc<str>,
        session: Arc<str>,
        timeout: Duration,
    ) -> Self {
        Self {
            browser,
            id,
            session,
            timeout,
        }
    }

    /// The tab's DevTools target id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The protocol session this tab is attached through.
    pub(super) fn session_id(&self) -> &str {
        &self.session
    }

    /// The browser this tab belongs to.
    #[must_use]
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    /// How long this handle waits for elements and conditions by default.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Returns a handle to the same tab that waits `timeout` by default.
    ///
    /// Locators created from the new handle inherit the timeout.
    #[must_use]
    pub fn with_timeout(&self, timeout: Duration) -> Self {
        Self {
            timeout,
            ..self.clone()
        }
    }

    /// Finds elements by `selector`, lazily.
    ///
    /// Nothing is looked up until the locator is used, and every use looks
    /// again, so it keeps working when the page re-renders. See [`Locator`].
    /// Strings are classified like SeleniumBase does: `"#id"` is CSS,
    /// `"//div"` is XPath, `"link=Home"` is a link.
    pub fn locator(&self, selector: impl Into<SelectorBuf>) -> Locator {
        Locator::new(self.clone(), selector.into())
    }

    // ------------------------------------------------------------------
    // Protocol access
    // ------------------------------------------------------------------

    /// Sends a raw protocol command to this tab.
    ///
    /// An escape hatch for commands this API does not wrap.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn execute(&self, method: &str, params: Value) -> Result<Value, SeleniumBaseError> {
        self.browser
            .inner()
            .client
            .send(method, params, Some(&self.session))
            .await
    }

    /// Sends several commands to this tab in order, without waiting between
    /// them, and returns their results in order.
    ///
    /// The browser runs a tab's commands in the order they arrive, so this does
    /// the same as awaiting each in turn, only sooner. Use it where one command
    /// would otherwise sit waiting on a slow answer ahead of the next.
    pub(crate) async fn execute_ordered(
        &self,
        commands: Vec<(&str, Value)>,
    ) -> Result<Vec<Value>, SeleniumBaseError> {
        self.browser
            .inner()
            .client
            .send_ordered(commands, Some(&self.session))
            .await
    }

    /// Subscribes to protocol events from this tab.
    #[must_use]
    pub fn events(&self) -> Events {
        self.browser.events().for_session(&self.session)
    }

    // ------------------------------------------------------------------
    // Scripting
    // ------------------------------------------------------------------

    /// Evaluates a JavaScript expression and returns its value.
    ///
    /// Promises are awaited. The result must be JSON-representable.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the script throws.
    pub async fn evaluate(&self, expression: impl AsRef<str>) -> Result<Value, SeleniumBaseError> {
        self.eval_labelled("", expression.as_ref()).await
    }

    /// Evaluates a JavaScript expression and deserialises the result.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the script throws, or
    /// [`SeleniumBaseError::Json`] if the result is not a `T`.
    pub async fn evaluate_as<T: DeserializeOwned>(
        &self,
        expression: impl AsRef<str>,
    ) -> Result<T, SeleniumBaseError> {
        Ok(serde_json::from_value(self.evaluate(expression).await?)?)
    }

    /// Evaluates `expression`, installing the page helpers first if the
    /// document does not have them yet.
    ///
    /// `label` names the locator involved, for error messages.
    pub(crate) async fn eval_labelled(
        &self,
        label: &str,
        expression: &str,
    ) -> Result<Value, SeleniumBaseError> {
        match self.eval_once(label, expression).await {
            Err(SeleniumBaseError::CdpDriver(message))
                if message.contains("__sbcdp is not defined") =>
            {
                self.execute("Runtime.evaluate", json!({ "expression": HELPER_JS }))
                    .await?;
                self.eval_once(label, expression).await
            }
            other => other,
        }
    }

    async fn eval_once(&self, label: &str, expression: &str) -> Result<Value, SeleniumBaseError> {
        let response = self
            .execute(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                    "userGesture": true,
                }),
            )
            .await?;
        if let Some(details) = response.get("exceptionDetails") {
            let description = details["exception"]["description"]
                .as_str()
                .or_else(|| details["text"].as_str())
                .unwrap_or("script error");
            return Err(map_script_error(label, description));
        }
        Ok(response["result"]["value"].clone())
    }

    /// Evaluates `expression` and returns it as a string, or an empty string
    /// for `null` and `undefined`.
    async fn string_of(&self, expression: &str) -> Result<String, SeleniumBaseError> {
        Ok(match self.evaluate(expression).await? {
            Value::String(text) => text,
            Value::Null => String::new(),
            other => other.to_string(),
        })
    }

    /// Repeats `check` until it returns `true` or `timeout` passes.
    ///
    /// Errors caused by a navigation tearing down the page mid-check count as
    /// "not yet" rather than as failures.
    pub(crate) async fn poll<F, Fut>(
        &self,
        timeout: Duration,
        mut check: F,
    ) -> Result<bool, SeleniumBaseError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<bool, SeleniumBaseError>>,
    {
        let deadline = Instant::now() + timeout;
        loop {
            match check().await {
                Ok(true) => return Ok(true),
                Ok(false) => {}
                Err(SeleniumBaseError::CdpDriver(message)) if is_navigation_race(&message) => {}
                Err(error) => return Err(error),
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    // ------------------------------------------------------------------
    // Navigation
    // ------------------------------------------------------------------

    /// Navigates to `url` and waits for the page to load.
    ///
    /// A URL without a scheme is opened over HTTPS.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Navigation`] if the browser cannot load
    /// the page, and [`SeleniumBaseError::WaitTimeout`] if it takes too long.
    pub async fn goto(&self, url: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        let url = normalize_page_url(url.as_ref())?;
        // Listen first: the load event can arrive before the command returns.
        let mut events = self.events();
        let response = self.execute("Page.navigate", json!({ "url": url })).await?;
        if let Some(error) = response["errorText"].as_str() {
            // ERR_ABORTED is what a navigation that starts a download reports.
            return if error == "net::ERR_ABORTED" {
                Ok(())
            } else {
                Err(SeleniumBaseError::navigation(url, error))
            };
        }
        // A same-document navigation (a #fragment change) has no loader id and
        // never fires a load event.
        if response.get("loaderId").is_none() {
            return Ok(());
        }
        self.await_load_event(&mut events).await
    }

    /// Reloads the page and waits for it to load.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the reload takes too long.
    pub async fn reload(&self) -> Result<(), SeleniumBaseError> {
        self.reload_with(false).await
    }

    /// Reloads the page, bypassing the browser cache, and waits for it to load.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the reload takes too long.
    pub async fn hard_reload(&self) -> Result<(), SeleniumBaseError> {
        self.reload_with(true).await
    }

    async fn reload_with(&self, ignore_cache: bool) -> Result<(), SeleniumBaseError> {
        let mut events = self.events();
        self.execute("Page.reload", json!({ "ignoreCache": ignore_cache }))
            .await?;
        self.await_load_event(&mut events).await
    }

    /// Goes back one step in the tab's history. Does nothing at the start.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the history is unreadable.
    pub async fn back(&self) -> Result<(), SeleniumBaseError> {
        self.step_history(-1).await
    }

    /// Goes forward one step in the tab's history. Does nothing at the end.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the history is unreadable.
    pub async fn forward(&self) -> Result<(), SeleniumBaseError> {
        self.step_history(1).await
    }

    async fn step_history(&self, delta: i64) -> Result<(), SeleniumBaseError> {
        let history = self.execute("Page.getNavigationHistory", json!({})).await?;
        let target = history["currentIndex"].as_i64().unwrap_or(0) + delta;
        let Some(entry) = usize::try_from(target)
            .ok()
            .and_then(|index| history["entries"].as_array()?.get(index))
        else {
            return Ok(());
        };
        let expected = entry["url"].as_str().unwrap_or_default().to_owned();
        let before = self.url().await?;
        self.execute(
            "Page.navigateToHistoryEntry",
            json!({ "entryId": entry["id"] }),
        )
        .await?;
        // Going back or forward can restore the page from the back-forward
        // cache, which fires no load event, so check the document itself.
        let arrived = self
            .poll(NAVIGATION_TIMEOUT, || async {
                let url = self.url().await?;
                let complete = self.evaluate("document.readyState").await? == "complete";
                Ok(complete && (url == expected || url != before))
            })
            .await?;
        if arrived {
            Ok(())
        } else {
            Err(SeleniumBaseError::navigation(
                expected,
                "the page did not finish loading",
            ))
        }
    }

    async fn await_load_event(&self, events: &mut Events) -> Result<(), SeleniumBaseError> {
        match events
            .wait_for("Page.loadEventFired", NAVIGATION_TIMEOUT)
            .await
        {
            Ok(_) => Ok(()),
            // The page may have finished before anything listened, so check
            // the document itself before declaring the navigation stuck.
            Err(SeleniumBaseError::WaitTimeout { .. }) => {
                self.wait_for_ready_state(LOAD_FALLBACK).await
            }
            Err(other) => Err(other),
        }
    }

    /// Waits until the document has finished loading.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if it does not in time.
    pub async fn wait_for_load(&self) -> Result<(), SeleniumBaseError> {
        self.wait_for_ready_state(NAVIGATION_TIMEOUT).await
    }

    async fn wait_for_ready_state(&self, timeout: Duration) -> Result<(), SeleniumBaseError> {
        let ready = self
            .poll(timeout, || async {
                Ok(self.evaluate("document.readyState").await? == "complete")
            })
            .await?;
        if ready {
            Ok(())
        } else {
            Err(SeleniumBaseError::wait_timeout(
                "document.readyState to be complete",
                Some(timeout),
            ))
        }
    }

    /// Waits until a script expression is truthy.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if it never is.
    pub async fn wait_for_function(
        &self,
        expression: impl AsRef<str>,
    ) -> Result<(), SeleniumBaseError> {
        let wrapped = format!("!!({})", expression.as_ref());
        let satisfied = self
            .poll(self.timeout, || async {
                Ok(self.evaluate(&wrapped).await? == Value::Bool(true))
            })
            .await?;
        if satisfied {
            Ok(())
        } else {
            Err(SeleniumBaseError::wait_timeout(
                format!("`{}` to be truthy", expression.as_ref()),
                Some(self.timeout),
            ))
        }
    }

    /// Waits until any one of `locators` reaches `state`, and returns which.
    ///
    /// The result is an index into `locators`; earlier entries win when
    /// several are ready at once.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if none does in time.
    pub async fn wait_for_any(
        &self,
        locators: &[&Locator],
        state: State,
    ) -> Result<usize, SeleniumBaseError> {
        // A mutex rather than a `RefCell`, so the future stays `Send`.
        let winner = Mutex::new(None);
        let found = self
            .poll(self.timeout, || {
                let winner = &winner;
                async move {
                    for (index, locator) in locators.iter().enumerate() {
                        if locator.is_in_state(state).await? {
                            *locked(winner) = Some(index);
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
            })
            .await?;
        let winner = winner
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match (found, winner) {
            (true, Some(index)) => Ok(index),
            _ => Err(SeleniumBaseError::wait_timeout(
                format!("any of {} locators {}", locators.len(), state.describe()),
                Some(self.timeout),
            )),
        }
    }

    // ------------------------------------------------------------------
    // Page facts
    // ------------------------------------------------------------------

    /// The tab's current URL.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot be queried.
    pub async fn url(&self) -> Result<String, SeleniumBaseError> {
        self.string_of("window.location.href").await
    }

    /// The tab's title.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot be queried.
    pub async fn title(&self) -> Result<String, SeleniumBaseError> {
        self.string_of("document.title").await
    }

    /// The page's current HTML, including changes made by scripts.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot be queried.
    pub async fn content(&self) -> Result<String, SeleniumBaseError> {
        self.string_of("document.documentElement.outerHTML").await
    }

    /// The browser's `User-Agent` string.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot be queried.
    pub async fn user_agent(&self) -> Result<String, SeleniumBaseError> {
        self.string_of("navigator.userAgent").await
    }

    /// The uncaught script errors and unhandled promise rejections the current
    /// document has raised, oldest first.
    ///
    /// Collected from the moment the document starts, and cleared when the tab
    /// navigates to a new one. A resource that failed to load, such as a missing
    /// image, is not a script error and is not listed.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot be queried.
    pub async fn js_errors(&self) -> Result<Vec<String>, SeleniumBaseError> {
        self.evaluate_as("__sbcdp.errors.slice()").await
    }

    /// Fails if the current document has raised an uncaught script error.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] naming the first error
    /// (and how many followed it), or [`SeleniumBaseError::CdpDriver`] if the
    /// page cannot be queried.
    pub async fn assert_no_js_errors(&self) -> Result<(), SeleniumBaseError> {
        let errors = self.js_errors().await?;
        match errors.as_slice() {
            [] => Ok(()),
            [only] => Err(SeleniumBaseError::AssertionFailed(format!(
                "JS error detected: {only}"
            ))),
            [first, rest @ ..] => Err(SeleniumBaseError::AssertionFailed(format!(
                "JS error detected: {first} (and {} more)",
                rest.len()
            ))),
        }
    }

    /// Whether the browser believes it has network access.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot be queried.
    pub async fn is_online(&self) -> Result<bool, SeleniumBaseError> {
        Ok(self.evaluate("navigator.onLine").await? == Value::Bool(true))
    }

    /// Describes this tab: id, URL and title.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot be queried.
    pub async fn info(&self) -> Result<PageInfo, SeleniumBaseError> {
        Ok(PageInfo {
            id: self.id.to_string(),
            url: self.url().await?,
            title: self.title().await?,
        })
    }

    // ------------------------------------------------------------------
    // Tab control
    // ------------------------------------------------------------------

    /// Brings the tab to the foreground.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn bring_to_front(&self) -> Result<(), SeleniumBaseError> {
        self.execute("Page.bringToFront", json!({})).await?;
        Ok(())
    }

    /// Closes the tab.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn close(&self) -> Result<(), SeleniumBaseError> {
        self.browser
            .execute("Target.closeTarget", json!({ "targetId": &*self.id }))
            .await?;
        self.browser.forget_page(&self.id);
        Ok(())
    }

    // ------------------------------------------------------------------
    // Scrolling and capture
    // ------------------------------------------------------------------

    /// Scrolls the page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] on a script error.
    pub async fn scroll(&self, to: Scroll) -> Result<(), SeleniumBaseError> {
        let script = match to {
            Scroll::Top => "window.scrollTo(0, 0)".to_owned(),
            Scroll::Bottom => {
                "window.scrollTo(0, document.documentElement.scrollHeight)".to_owned()
            }
            Scroll::To(y) => format!("window.scrollTo(0, {y})"),
            Scroll::By(dy) => format!("window.scrollBy(0, {dy})"),
            Scroll::PageDown(percent) => {
                format!("window.scrollBy(0, window.innerHeight * {percent} / 100)")
            }
            Scroll::PageUp(percent) => {
                format!("window.scrollBy(0, -window.innerHeight * {percent} / 100)")
            }
        };
        self.evaluate(script).await?;
        Ok(())
    }

    /// Captures the viewport as PNG bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser cannot capture.
    pub async fn screenshot(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        let response = self
            .execute("Page.captureScreenshot", json!({ "format": "png" }))
            .await?;
        decode_payload(&response, "screenshot").map_err(SeleniumBaseError::screenshot)
    }

    /// Renders the page to PDF bytes.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser cannot print.
    pub async fn pdf(&self) -> Result<Vec<u8>, SeleniumBaseError> {
        let response = self
            .execute("Page.printToPDF", json!({ "printBackground": true }))
            .await?;
        decode_payload(&response, "PDF").map_err(SeleniumBaseError::pdf)
    }
}

/// Decodes the base64 `data` field of a capture response.
fn decode_payload(response: &Value, what: &str) -> Result<Vec<u8>, String> {
    let data = response["data"]
        .as_str()
        .ok_or_else(|| format!("the browser returned no {what} data"))?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| format!("the {what} data was not valid base64: {e}"))
}

/// Turns a script exception into the closest typed error.
///
/// The page helpers throw errors whose messages start with `sbcdp:` so that
/// element problems are not reported as generic script failures.
fn map_script_error(label: &str, description: &str) -> SeleniumBaseError {
    let first_line = |rest: &str| rest.lines().next().unwrap_or_default().to_owned();
    if description.contains("sbcdp:not-found") {
        return SeleniumBaseError::element_not_found(label);
    }
    if let Some(rest) = description.split("sbcdp:not-interactable:").nth(1) {
        return SeleniumBaseError::element_not_interactable(label, first_line(rest));
    }
    if let Some(rest) = description.split("sbcdp:no-option:").nth(1) {
        return SeleniumBaseError::InvalidSelector(format!(
            "no such option in {label}: {}",
            first_line(rest)
        ));
    }
    SeleniumBaseError::cdp_driver(description.lines().next().unwrap_or(description))
}

/// Whether an error just means a navigation replaced the page mid-command.
fn is_navigation_race(message: &str) -> bool {
    message.contains("Execution context was destroyed")
        || message.contains("Cannot find context")
        || message.contains("Inspected target navigated")
        || message.contains("Target closed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_problems_in_scripts_become_typed_errors() {
        assert!(matches!(
            map_script_error("#login", "Error: sbcdp:not-found\n    at one"),
            SeleniumBaseError::ElementNotFound { ref selector, .. } if selector == "#login"
        ));
        assert!(matches!(
            map_script_error("#hidden", "Error: sbcdp:not-interactable:the element has no size"),
            SeleniumBaseError::ElementNotInteractable { ref selector, ref reason }
                if selector == "#hidden" && reason == "the element has no size"
        ));
        assert!(matches!(
            map_script_error("select", "Error: sbcdp:no-option:text=Mars"),
            SeleniumBaseError::InvalidSelector(_)
        ));
    }

    #[test]
    fn ordinary_script_errors_keep_only_their_first_line() {
        let error = map_script_error(
            "",
            "ReferenceError: nope is not defined\n    at <anonymous>:1:1",
        );
        assert!(
            matches!(error, SeleniumBaseError::CdpDriver(ref m) if m == "ReferenceError: nope is not defined")
        );
    }

    #[test]
    fn only_page_teardown_errors_count_as_navigation_races() {
        assert!(is_navigation_race(
            "Runtime.evaluate: Execution context was destroyed."
        ));
        assert!(is_navigation_race("Target closed"));
        assert!(!is_navigation_race("TypeError: x is not a function"));
    }

    #[test]
    fn capture_payloads_are_decoded_and_bad_ones_rejected() {
        assert_eq!(
            decode_payload(&json!({ "data": "aGk=" }), "x").unwrap(),
            b"hi"
        );
        assert!(decode_payload(&json!({}), "x").is_err());
        assert!(decode_payload(&json!({ "data": "!!not base64!!" }), "x").is_err());
    }
}
