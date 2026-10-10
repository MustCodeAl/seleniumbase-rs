// Remaining public BaseCase helpers to close the parity gap with Python SeleniumBase.
// This file is included at the end of `base_case.rs` so it shares the same scope.
//
// It also holds the two helpers every wait loop in the `base_case_impls` files
// builds on: `poll_until_true`, which polls a condition, and `http_status_code`,
// which asks a server for the status of a link.

use regex::Regex;

// -------------------------------------------------------------------------
// Internal helpers
// -------------------------------------------------------------------------

/// How often the `wait_for_*` helpers re-check their condition.
///
/// Short enough that a condition that becomes true is noticed promptly, long
/// enough that polling does not flood the driver with commands.
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// How many links `print_unique_links_with_status_codes` checks at once.
const LINK_CHECK_CONCURRENCY: usize = 8;

/// Time allowed for one link-status request.
const LINK_CHECK_TIMEOUT: Duration = Duration::from_secs(15);

/// Whether `error` describes the call rather than the page, so that waiting
/// longer cannot make the condition hold.
///
/// A malformed selector, an unsupported operation or a missing session fails
/// the same way on every poll, so the wait helpers report such an error at
/// once instead of polling until the timeout.
fn is_permanent_wait_error(error: &SeleniumBaseError) -> bool {
    matches!(
        error,
        SeleniumBaseError::InvalidSelector(_)
            | SeleniumBaseError::InvalidConfig(_)
            | SeleniumBaseError::Unsupported(_)
            | SeleniumBaseError::SessionNotStarted
            | SeleniumBaseError::BrowserDisconnected { .. }
    )
}

/// Polls `check` until it returns `Ok(true)` or `timeout_secs` have passed.
///
/// `check` always runs at least once, even with a zero timeout. A check that
/// returns `Err` does not end the wait unless the error is permanent (see
/// [`is_permanent_wait_error`]): a page that is mid-navigation or a stale
/// element can recover. When the wait times out, the returned
/// [`SeleniumBaseError::WaitTimeout`] carries `failure`, the time waited and
/// the last error the check reported, so a broken check is not mistaken for a
/// condition that never held.
async fn poll_until_true<F, Fut>(
    timeout_secs: u64,
    failure: String,
    mut check: F,
) -> Result<(), SeleniumBaseError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<bool, SeleniumBaseError>>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let last_error = match check().await {
            Ok(true) => return Ok(()),
            Ok(false) => None,
            Err(error) if is_permanent_wait_error(&error) => return Err(error),
            Err(error) => Some(error),
        };
        let now = tokio::time::Instant::now();
        if now >= deadline {
            let detail = last_error
                .map(|error| format!(" (last error: {error})"))
                .unwrap_or_default();
            return Err(SeleniumBaseError::WaitTimeout(format!(
                "{failure} after {timeout_secs}s{detail}"
            )));
        }
        tokio::time::sleep(WAIT_POLL_INTERVAL.min(deadline - now)).await;
    }
}

/// `s` as a double-quoted JavaScript string literal, safe to splice into a
/// script.
fn js_string_literal(s: &str) -> String {
    // Serializing a `&str` cannot fail; the fallback only keeps this total.
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_owned())
}

/// The client every link check shares: it does not follow redirects, because
/// the status of the link itself is the answer, and gives up after
/// [`LINK_CHECK_TIMEOUT`].
fn link_check_client() -> Result<&'static reqwest::Client, SeleniumBaseError> {
    static CLIENT: std::sync::OnceLock<Result<reqwest::Client, String>> =
        std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(LINK_CHECK_TIMEOUT)
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|reason| SeleniumBaseError::InvalidConfig(reason.clone()))
}

/// HTTP status code `url` answers with.
///
/// Redirects are not followed, so a moved page reports `301` or `302` rather
/// than the status of wherever it points; that matches Python SeleniumBase's
/// `get_link_status_code`. The check is a `HEAD` request, repeated as a `GET`
/// when the server does not implement `HEAD` (405 or 501).
///
/// # Errors
///
/// Returns [`SeleniumBaseError::Network`] when the request cannot be made or
/// gets no answer, for example for a refused connection or a URL that is not
/// HTTP(S).
async fn http_status_code(url: &str) -> Result<u16, SeleniumBaseError> {
    let client = link_check_client()?;
    let failed = |e: reqwest::Error| SeleniumBaseError::network(url, 0, e.to_string());
    let status = client.head(url).send().await.map_err(failed)?.status();
    if matches!(status.as_u16(), 405 | 501) {
        return Ok(client.get(url).send().await.map_err(failed)?.status().as_u16());
    }
    Ok(status.as_u16())
}

/// Status codes of `links`, checked [`LINK_CHECK_CONCURRENCY`] at a time and
/// returned in the order given.
async fn check_links(links: Vec<String>) -> Vec<(String, Result<u16, SeleniumBaseError>)> {
    use futures_util::stream::{self, StreamExt};
    stream::iter(links)
        .map(|link| async move {
            let status = http_status_code(&link).await;
            (link, status)
        })
        .buffered(LINK_CHECK_CONCURRENCY)
        .collect()
        .await
}

fn regex_or_err(pattern: &str) -> Result<Regex, SeleniumBaseError> {
    Regex::new(pattern).map_err(|e| SeleniumBaseError::InvalidConfig(e.to_string()))
}

impl BaseCase {
    // =====================================================================
    // Generic / assertion helpers
    // =====================================================================

    /// Fails the test immediately with `message`.
    pub fn fail(&self, message: &str) -> Result<(), SeleniumBaseError> {
        Err(SeleniumBaseError::AssertionFailed(message.to_owned()))
    }

    /// Alias for sleeping `seconds`.
    pub async fn wait(&self, seconds: u64) {
        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
    }

    /// Sets the default soft timeout used by wait helpers.
    pub fn set_default_timeout(&mut self, seconds: u64) {
        self.time_limit_secs = Some(seconds);
    }

    /// Clears the default soft timeout.
    pub fn reset_default_timeout(&mut self) {
        self.time_limit_secs = None;
    }

    /// Alias for `get_property`.
    pub async fn get_property_value(
        &mut self,
        css: &str,
        property: &str,
    ) -> Result<Option<String>, SeleniumBaseError> {
        self.get_property(css, property).await
    }

    /// Asserts that `css` is present in the DOM.
    pub async fn assert_element_present(&self, css: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_element_present(css).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Element '{}' was not present",
                css
            )));
        }
        Ok(())
    }

    /// Asserts that every selector in `selectors` is present.
    pub async fn assert_elements(&self, selectors: &[&str]) -> Result<(), SeleniumBaseError> {
        self.assert_elements_present(selectors).await
    }

    /// Asserts that every selector in `selectors` is visible.
    pub async fn assert_elements_visible(
        &mut self,
        selectors: &[&str],
    ) -> Result<(), SeleniumBaseError> {
        for css in selectors {
            self.assert_element_visible(css).await?;
        }
        Ok(())
    }

    /// Asserts that at least one selector in `selectors` is present.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::AssertionFailed`] when none is present, and
    /// the error itself when a selector is invalid or the lookup fails, rather
    /// than counting it as "not present".
    pub async fn assert_any_of_elements_present(
        &self,
        selectors: &[&str],
    ) -> Result<(), SeleniumBaseError> {
        for css in selectors {
            if self.is_element_present(css).await? {
                return Ok(());
            }
        }
        Err(SeleniumBaseError::AssertionFailed(format!(
            "None of the selectors were present: {}",
            selectors.join(", ")
        )))
    }

    /// Asserts that `text` is not visible inside `css`.
    pub async fn assert_exact_text_not_visible(
        &mut self,
        css: &str,
        text: &str,
    ) -> Result<(), SeleniumBaseError> {
        if self.is_exact_text_visible(text, css).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Exact text '{}' was unexpectedly visible in '{}'",
                text, css
            )));
        }
        Ok(())
    }

    /// Asserts that `css` has non-empty visible text.
    pub async fn assert_non_empty_text(&self, css: &str) -> Result<(), SeleniumBaseError> {
        if !self.is_non_empty_text_visible(css).await? {
            return Err(SeleniumBaseError::AssertionFailed(format!(
                "Element '{}' did not have non-empty visible text",
                css
            )));
        }
        Ok(())
    }

    // =====================================================================
    // Wait helpers
    //
    // Each helper polls through `poll_until_true`: a check that fails with an
    // error is retried until the timeout (permanent errors such as an invalid
    // selector are returned at once), and the timeout error names the last
    // failure.
    // =====================================================================

    /// Waits up to `timeout_secs` for `css` to leave the DOM.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element is still
    /// present after the timeout.
    pub async fn wait_for_element_not_present(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            format!("Element '{css}' was still present"),
            || async { Ok(!self.is_element_present(css).await?) },
        )
        .await
    }

    /// Waits up to `timeout_secs` for exact `text` to be visible inside `css`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the text is not visible
    /// after the timeout.
    pub async fn wait_for_exact_text_visible(
        &self,
        css: &str,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            format!("Exact text '{text}' was not visible in '{css}'"),
            || self.is_exact_text_visible(text, css),
        )
        .await
    }

    /// Waits up to `timeout_secs` for exact `text` to not be visible inside `css`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the text is still visible
    /// after the timeout.
    pub async fn wait_for_exact_text_not_visible(
        &self,
        css: &str,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            format!("Exact text '{text}' was still visible in '{css}'"),
            || async { Ok(!self.is_exact_text_visible(text, css).await?) },
        )
        .await
    }

    /// Waits up to `timeout_secs` for `css` to contain non-empty visible text.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the element has no visible
    /// text after the timeout.
    pub async fn wait_for_non_empty_text(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            format!("Non-empty text was not visible in '{css}'"),
            || self.is_non_empty_text_visible(css),
        )
        .await
    }

    /// Alias for `wait_for_non_empty_text`.
    pub async fn wait_for_non_empty_text_visible(
        &self,
        css: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        self.wait_for_non_empty_text(css, timeout_secs).await
    }

    /// Whether the rendered text of the page contains `text`.
    async fn page_text_contains(&self, text: &str) -> Result<bool, SeleniumBaseError> {
        let script = format!(
            "return document.body != null && document.body.innerText.includes({});",
            js_string_literal(text)
        );
        Ok(self.execute_script(&script).await?.as_bool() == Some(true))
    }

    /// Waits up to `timeout_secs` for `text` to be visible anywhere on the page.
    ///
    /// Unlike Python's `wait_for_text_visible(text, selector="html", ...)`, this
    /// takes no selector: it looks at the whole page. To wait for text inside
    /// one element, use [`BaseCase::wait_for_text`], which takes the selector
    /// first.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the text is not visible
    /// after the timeout.
    pub async fn wait_for_text_visible(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            format!("Text '{text}' was not visible"),
            || self.page_text_contains(text),
        )
        .await
    }

    /// Waits up to `timeout_secs` for `text` to not be visible anywhere.
    ///
    /// Like [`BaseCase::wait_for_text_visible`], this looks at the whole page.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if the text is still visible
    /// after the timeout.
    pub async fn wait_for_text_not_visible(
        &self,
        text: &str,
        timeout_secs: u64,
    ) -> Result<(), SeleniumBaseError> {
        poll_until_true(
            self.effective_timeout(timeout_secs),
            format!("Text '{text}' was still visible"),
            || async { Ok(!self.page_text_contains(text).await?) },
        )
        .await
    }
}

#[cfg(test)]
mod tests_common {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Serves `respond(method, path) -> (status, location)` on a loopback port.
    fn serve(respond: fn(&str, &str) -> (u16, Option<&'static str>)) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let address = listener.local_addr().expect("bound address");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buffer = [0_u8; 2048];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
                let mut parts = request.lines().next().unwrap_or("").split(' ');
                let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
                let (status, location) = respond(method, path);
                let location = location.map_or_else(String::new, |l| format!("Location: {l}\r\n"));
                let response = format!(
                    "HTTP/1.1 {status} X\r\n{location}Content-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{address}")
    }

    #[tokio::test(start_paused = true)]
    async fn poll_returns_as_soon_as_the_condition_holds() {
        let calls = AtomicUsize::new(0);
        let result = poll_until_true(10, "never".to_owned(), || async {
            Ok(calls.fetch_add(1, Ordering::SeqCst) == 2)
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn poll_checks_once_even_with_a_zero_timeout() {
        let calls = AtomicUsize::new(0);
        let result = poll_until_true(0, "gone".to_owned(), || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(false)
        })
        .await;
        assert!(matches!(result, Err(SeleniumBaseError::WaitTimeout(m)) if m == "gone after 0s"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn poll_timeout_names_the_last_error_instead_of_hiding_it() {
        let result = poll_until_true(1, "Element 'x' was not visible".to_owned(), || async {
            Err(SeleniumBaseError::StaleElement {
                selector: "x".to_owned(),
            })
        })
        .await;
        let Err(SeleniumBaseError::WaitTimeout(message)) = result else {
            panic!("expected a timeout, got {result:?}");
        };
        assert!(message.contains("Element 'x' was not visible after 1s"), "{message}");
        assert!(message.contains("stale element reference"), "{message}");
    }

    #[tokio::test(start_paused = true)]
    async fn poll_recovers_from_a_transient_error() {
        let calls = AtomicUsize::new(0);
        let result = poll_until_true(5, "never".to_owned(), || async {
            match calls.fetch_add(1, Ordering::SeqCst) {
                0 => Err(SeleniumBaseError::StaleElement {
                    selector: "x".to_owned(),
                }),
                _ => Ok(true),
            }
        })
        .await;
        assert!(result.is_ok(), "{result:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn poll_returns_a_permanent_error_without_waiting_for_the_timeout() {
        let calls = AtomicUsize::new(0);
        let result = poll_until_true(3600, "never".to_owned(), || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(SeleniumBaseError::InvalidSelector("(".to_owned()))
        })
        .await;
        assert!(matches!(result, Err(SeleniumBaseError::InvalidSelector(_))), "{result:?}");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "a permanent error must not be retried");
    }

    #[tokio::test(start_paused = true)]
    async fn poll_never_sleeps_past_the_deadline() {
        let started = tokio::time::Instant::now();
        let _ = poll_until_true(1, "slow".to_owned(), || async { Ok(false) }).await;
        assert_eq!(started.elapsed(), Duration::from_secs(1));
    }

    #[test]
    fn js_string_literal_escapes_everything_a_script_could_trip_on() {
        assert_eq!(js_string_literal("plain"), "\"plain\"");
        assert_eq!(js_string_literal("it's"), "\"it's\"");
        assert_eq!(js_string_literal("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
    }

    #[tokio::test]
    async fn status_code_is_the_first_response_not_the_end_of_a_redirect() {
        let base = serve(|_, path| match path {
            "/moved" => (302, Some("/ok")),
            "/gone" => (404, None),
            _ => (200, None),
        });
        assert_eq!(http_status_code(&format!("{base}/moved")).await.unwrap(), 302);
        assert_eq!(http_status_code(&format!("{base}/gone")).await.unwrap(), 404);
        assert_eq!(http_status_code(&format!("{base}/ok")).await.unwrap(), 200);
    }

    #[tokio::test]
    async fn status_code_asks_again_with_get_when_head_is_not_allowed() {
        let base = serve(|method, _| if method == "HEAD" { (405, None) } else { (200, None) });
        assert_eq!(http_status_code(&format!("{base}/page")).await.unwrap(), 200);
    }

    #[tokio::test]
    async fn status_code_reports_a_failed_request_as_an_error() {
        // Nothing listens on a port that was just released.
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let refused = http_status_code(&format!("http://127.0.0.1:{port}/")).await;
        assert!(matches!(refused, Err(SeleniumBaseError::Network { .. })), "{refused:?}");
        let not_http = http_status_code("javascript:void(0)").await;
        assert!(matches!(not_http, Err(SeleniumBaseError::Network { .. })), "{not_http:?}");
    }

    #[tokio::test]
    async fn check_links_keeps_the_order_it_was_given() {
        let base = serve(|_, path| match path {
            "/a" => (200, None),
            "/b" => (404, None),
            _ => (500, None),
        });
        let links: Vec<String> = ["/c", "/a", "/b"].iter().map(|p| format!("{base}{p}")).collect();
        let checked = check_links(links.clone()).await;
        let codes: Vec<u16> = checked.iter().map(|(_, r)| *r.as_ref().unwrap()).collect();
        assert_eq!(codes, [500, 200, 404]);
        assert_eq!(checked.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>(), links);
    }
}

/// Helpers for the tests that need a real Chrome (marked `#[ignore]`).
///
/// Run them with `cargo test --lib -- --ignored`. They launch a headless
/// Chrome through the Pure CDP engine and serve pages from a loopback port, so
/// they need no WebDriver and no network.
#[cfg(test)]
pub(crate) mod chrome_support {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use crate::sb_cdp::{Browser, LaunchOptions, Page};

    /// Serves `(path, status, extra headers, body)` routes on a loopback port
    /// and returns its `http://host:port` base. An unknown path answers 404.
    pub fn serve(routes: Vec<(&'static str, u16, String, String)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        let address = listener.local_addr().expect("bound address");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..read]).into_owned();
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split(' ').nth(1))
                    .unwrap_or("/");
                let (status, headers, body) = routes
                    .iter()
                    .find(|route| route.0 == path)
                    .map_or((404, String::new(), String::new()), |route| {
                        (route.1, route.2.clone(), route.3.clone())
                    });
                let response = format!(
                    "HTTP/1.1 {status} X\r\n{headers}Content-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{address}")
    }

    /// Launches a headless Chrome and opens `url` in its first tab.
    pub async fn open(url: &str) -> (Browser, Page) {
        let options = LaunchOptions::builder()
            .headless(true)
            .no_sandbox(std::env::var_os("CI").is_some())
            .window_size(1000, 800)
            .build()
            .expect("launch options");
        let browser = Browser::launch(options).await.expect("Chrome starts");
        let page = browser.default_page().await.expect("a first tab");
        page.goto(url).await.expect("the page loads");
        (browser, page)
    }

    /// Serves `html` at `/` and opens it.
    pub async fn open_html(html: &str) -> (Browser, Page) {
        let base = serve(vec![("/", 200, String::new(), html.to_owned())]);
        open(&format!("{base}/")).await
    }
}
