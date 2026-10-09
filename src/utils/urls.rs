//! URL normalization shared by the navigation helpers.

use crate::error::SeleniumBaseError;

/// Prefixes that mark a string as already being a page URL.
///
/// This is the same set SeleniumBase recognizes before it decides whether to
/// add a scheme of its own.
const PAGE_URL_PREFIXES: [&str; 8] = [
    "http:", "https:", "://", "data:", "file:", "about:", "chrome:", "edge:",
];

/// Returns `true` when `url` already carries a scheme a browser can navigate to.
pub fn looks_like_a_page_url(url: &str) -> bool {
    PAGE_URL_PREFIXES
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

/// Normalizes `url` for navigation, adding `https://` when no scheme is present.
///
/// SeleniumBase accepts `"seleniumbase.io"` as readily as
/// `"https://seleniumbase.io"`, so this mirrors that behavior: a string that
/// already looks like a page URL is passed through untouched, and anything
/// else is retried as `https://<url>`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] when `url` is empty, or when it
/// is still not a valid URL after `https://` has been prepended.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::utils::urls::normalize_page_url;
///
/// assert_eq!(normalize_page_url("seleniumbase.io")?, "https://seleniumbase.io");
/// assert_eq!(
///     normalize_page_url("https://seleniumbase.io")?,
///     "https://seleniumbase.io"
/// );
/// assert_eq!(normalize_page_url("about:blank")?, "about:blank");
/// # Ok::<(), seleniumbase_rs::SeleniumBaseError>(())
/// ```
pub fn normalize_page_url(url: &str) -> Result<String, SeleniumBaseError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(SeleniumBaseError::InvalidConfig(
            "URL cannot be empty".to_owned(),
        ));
    }
    if looks_like_a_page_url(trimmed) {
        return Ok(trimmed.to_owned());
    }
    let candidate = format!("https://{trimmed}");
    if url::Url::parse(&candidate).is_ok() {
        Ok(candidate)
    } else {
        Err(SeleniumBaseError::InvalidConfig(format!(
            "Invalid URL: {url:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_urls_that_already_have_a_scheme() {
        for url in [
            "https://seleniumbase.io",
            "http://example.com/a?b=c",
            "file:///tmp/page.html",
            "about:blank",
            "data:text/html,<h1>hi</h1>",
            "chrome://version",
            "edge://settings",
            "://example.com",
        ] {
            assert_eq!(normalize_page_url(url).unwrap(), url, "changed {url}");
        }
    }

    #[test]
    fn adds_https_when_the_scheme_is_missing() {
        assert_eq!(
            normalize_page_url("seleniumbase.io").unwrap(),
            "https://seleniumbase.io"
        );
        assert_eq!(
            normalize_page_url("example.com/path?q=1").unwrap(),
            "https://example.com/path?q=1"
        );
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(
            normalize_page_url("  seleniumbase.io \n").unwrap(),
            "https://seleniumbase.io"
        );
        assert_eq!(
            normalize_page_url("\thttps://example.com ").unwrap(),
            "https://example.com"
        );
    }

    #[test]
    fn rejects_empty_input() {
        assert!(normalize_page_url("").is_err());
        assert!(normalize_page_url("   ").is_err());
    }

    #[test]
    fn looks_like_a_page_url_matches_known_schemes() {
        assert!(looks_like_a_page_url("https://x.com"));
        assert!(looks_like_a_page_url("about:blank"));
        assert!(!looks_like_a_page_url("x.com"));
        assert!(!looks_like_a_page_url("/local/path"));
    }
}
