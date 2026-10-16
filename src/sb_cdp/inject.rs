//! Code and styles a program adds to a tab: `<script>` and `<style>` tags in
//! the current document, and scripts for every document the tab loads later.
//!
//! These are Playwright's `addScriptTag`, `addStyleTag` and `addInitScript`,
//! and what `BaseCase` offers as `add_js_link`, `add_css_link` and
//! `add_js_code` on the WebDriver engine.

use std::time::Duration;

use serde_json::{json, Value};

use super::page::Page;
use crate::error::SeleniumBaseError;

/// Where the code of a tag comes from, for [`Page::add_script_tag`] and
/// [`Page::add_style_tag`].
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::TagSource;
///
/// let file = TagSource::url("https://cdn.jsdelivr.net/npm/intro.js@7.2.0/intro.min.js");
/// let inline = TagSource::content("window.ready = true;");
/// assert_ne!(file, inline);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TagSource {
    /// A file the browser loads from this URL. A relative URL is read against
    /// the document's own address.
    Url(String),
    /// The code itself, put inside the tag.
    Content(String),
}

impl TagSource {
    /// A file the browser loads from `url`.
    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }

    /// `code` put inside the tag.
    pub fn content(code: impl Into<String>) -> Self {
        Self::Content(code.into())
    }
}

/// The identifier of a script added with [`Page::add_init_script`], which
/// [`Page::remove_init_script`] takes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct InitScriptId(String);

impl InitScriptId {
    /// The identifier as the browser gave it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InitScriptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Page {
    /// Adds a `<script>` tag to the current document and waits for it to run.
    ///
    /// With [`TagSource::Url`] the tag loads that file, and this waits until
    /// the browser has loaded and run it, or refused it, up to the page's
    /// [`timeout`](Self::timeout). With [`TagSource::Content`] the code runs as
    /// the tag is added. Only the current document gets the tag; for every
    /// document the tab loads later, use [`add_init_script`](Self::add_init_script).
    ///
    /// A page whose Content Security Policy forbids the file's host refuses
    /// the file, which is an error; one that forbids inline scripts stops the
    /// code of a [`TagSource::Content`] tag without one. [`evaluate`](Self::evaluate)
    /// is not bound by the policy.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the file does not load in time or
    /// at all, or when the code throws.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use seleniumbase_rs::sb_cdp::{Page, TagSource};
    ///
    /// # async fn demo(page: &Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// page.add_script_tag(TagSource::url(
    ///     "https://cdn.jsdelivr.net/npm/jquery@3.7.1/dist/jquery.min.js",
    /// ))
    /// .await?;
    /// assert_eq!(page.evaluate("typeof jQuery").await?, "function");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add_script_tag(&self, source: TagSource) -> Result<(), SeleniumBaseError> {
        self.eval_labelled("", &script_tag_expression(&source, self.timeout()))
            .await
            .map(drop)
    }

    /// Adds a stylesheet to the current document.
    ///
    /// With [`TagSource::Url`] a `<link rel="stylesheet">` loads the file, and
    /// this waits until the browser has loaded it, or refused it, up to the
    /// page's [`timeout`](Self::timeout). With [`TagSource::Content`] the CSS
    /// goes into a `<style>` tag. Only the current document gets it.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the file does not load in time or
    /// at all.
    pub async fn add_style_tag(&self, source: TagSource) -> Result<(), SeleniumBaseError> {
        self.eval_labelled("", &style_tag_expression(&source, self.timeout()))
            .await
            .map(drop)
    }

    /// Runs `source` in every document this tab loads from now on, before the
    /// page's own scripts, and in every frame of it.
    ///
    /// The document already loaded is left as it is, as in Playwright; run the
    /// script there with [`evaluate`](Self::evaluate) if it needs it too. The
    /// script stays until [`remove_init_script`](Self::remove_init_script) is
    /// given the identifier this returns, or the tab closes.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the browser refuses the script or
    /// gives no identifier for it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(page: &seleniumbase_rs::sb_cdp::Page) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    /// let id = page.add_init_script("window.startedAt = Date.now();").await?;
    /// page.goto("https://example.com").await?;
    /// assert!(page.evaluate("window.startedAt > 0").await?.as_bool().unwrap_or(false));
    /// page.remove_init_script(&id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn add_init_script(
        &self,
        source: impl AsRef<str>,
    ) -> Result<InitScriptId, SeleniumBaseError> {
        let response = self
            .execute(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": source.as_ref() }),
            )
            .await?;
        response["identifier"]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(|id| InitScriptId(id.to_owned()))
            .ok_or_else(|| {
                SeleniumBaseError::cdp_driver(
                    "the browser gave no identifier for the init script, so it could not be \
                     removed later",
                )
            })
    }

    /// Stops running the script [`add_init_script`](Self::add_init_script)
    /// added in the documents this tab loads from now on. Documents that
    /// already ran it keep what it did.
    ///
    /// # Errors
    ///
    /// [`SeleniumBaseError::CdpDriver`] when the browser refuses, for
    /// instance for an identifier of another tab.
    pub async fn remove_init_script(&self, id: &InitScriptId) -> Result<(), SeleniumBaseError> {
        self.execute(
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({ "identifier": id.as_str() }),
        )
        .await
        .map(drop)
    }
}

/// `text` as a JavaScript string literal: what a caller gives reaches the page
/// as data, never as code.
fn literal(text: &str) -> String {
    Value::from(text).to_string()
}

/// The expression that adds a `<script>` tag for `source`.
///
/// The code of a [`TagSource::Content`] tag runs while the tag is added, and
/// an exception it throws reaches the window's `error` listeners at once,
/// which is where it is caught.
fn script_tag_expression(source: &TagSource, timeout: Duration) -> String {
    match source {
        TagSource::Url(url) => url_tag_expression(
            "const el = document.createElement('script'); el.src = url;",
            "the script",
            url,
            timeout,
        ),
        TagSource::Content(code) => format!(
            "(() => {{ const el = document.createElement('script'); el.textContent = {code}; \
             let thrown = null; \
             const caught = (event) => {{ if (thrown === null) thrown = event.error ?? new Error(event.message); }}; \
             window.addEventListener('error', caught); \
             try {{ (document.head || document.documentElement).appendChild(el); }} \
             finally {{ window.removeEventListener('error', caught); }} \
             if (thrown !== null) throw thrown; }})()",
            code = literal(code),
        ),
    }
}

/// The expression that adds a stylesheet for `source`.
fn style_tag_expression(source: &TagSource, timeout: Duration) -> String {
    match source {
        TagSource::Url(url) => url_tag_expression(
            "const el = document.createElement('link'); el.rel = 'stylesheet'; el.href = url;",
            "the stylesheet",
            url,
            timeout,
        ),
        TagSource::Content(css) => format!(
            "(() => {{ const el = document.createElement('style'); el.textContent = {css}; \
             (document.head || document.documentElement).appendChild(el); }})()",
            css = literal(css),
        ),
    }
}

/// An expression that runs `create`, which makes `el` for the file at `url`,
/// adds `el` to the document, and settles when the file has loaded, when it
/// failed, or after `timeout`.
fn url_tag_expression(create: &str, what: &str, url: &str, timeout: Duration) -> String {
    let millis = timeout.as_millis();
    format!(
        "(async () => {{ const url = {url}; {create} \
         const loaded = new Promise((resolve, reject) => {{ \
         const timer = setTimeout(() => reject(new Error({what} + ' at ' + url + ' did not load within {millis} ms')), {millis}); \
         el.addEventListener('load', () => {{ clearTimeout(timer); resolve(); }}, {{ once: true }}); \
         el.addEventListener('error', () => {{ clearTimeout(timer); reject(new Error({what} + ' at ' + url + ' did not load')); }}, {{ once: true }}); }}); \
         (document.head || document.documentElement).appendChild(el); \
         await loaded; }})()",
        url = literal(url),
        what = literal(what),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_caller_gives_is_quoted_never_spliced_in() {
        let hostile = "'); alert(1); ('\"`${x}`";
        for expression in [
            script_tag_expression(&TagSource::url(hostile), Duration::from_secs(1)),
            script_tag_expression(&TagSource::content(hostile), Duration::from_secs(1)),
            style_tag_expression(&TagSource::url(hostile), Duration::from_secs(1)),
            style_tag_expression(&TagSource::content(hostile), Duration::from_secs(1)),
        ] {
            assert!(expression.contains(&literal(hostile)), "{expression}");
            assert_eq!(
                expression.matches("alert(1)").count(),
                1,
                "only inside the literal: {expression}"
            );
        }
    }

    #[test]
    fn a_file_is_waited_for_as_long_as_the_timeout_says() {
        let expression = style_tag_expression(
            &TagSource::url("https://example.com/a.css"),
            Duration::from_millis(2500),
        );
        assert!(
            expression.contains("did not load within 2500 ms')), 2500);"),
            "the timer: {expression}"
        );
        assert!(
            expression.contains("did not load within 2500 ms"),
            "{expression}"
        );
    }

    #[test]
    fn an_identifier_shows_as_the_browser_gave_it() {
        let id = InitScriptId("12".to_owned());
        assert_eq!(id.as_str(), "12");
        assert_eq!(id.to_string(), "12");
    }
}
