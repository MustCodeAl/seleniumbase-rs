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
///
/// Schemes are matched without regard to case, as browsers do, so
/// `HTTPS://example.com` counts.
pub fn looks_like_a_page_url(url: &str) -> bool {
    PAGE_URL_PREFIXES.iter().any(|prefix| {
        url.get(..prefix.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
    })
}

/// The scheme `url` starts with (`ftp` in `ftp://host`, `mailto` in
/// `mailto:a@b.c`) when it is one [`looks_like_a_page_url`] does not accept.
///
/// `host:8080` is not a scheme but a host and port, and a `:` further in (a
/// query string, a path) does not count either.
fn foreign_scheme(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once(':')?;
    let mut chars = scheme.chars();
    let is_scheme = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !is_scheme {
        return None;
    }
    let port_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    let is_port =
        port_len > 0 && (rest.len() == port_len || rest[port_len..].starts_with(['/', '?', '#']));
    (!is_port).then_some(scheme)
}

/// Normalizes `url` for navigation, adding `https://` when no scheme is present.
///
/// SeleniumBase accepts `"seleniumbase.io"` as readily as
/// `"https://seleniumbase.io"`, so this mirrors that behavior: a string that
/// already looks like a page URL is passed through untouched (apart from
/// surrounding whitespace), and anything else is retried as `https://<url>`.
///
/// Input that cannot be a host is refused rather than turned into a URL that
/// points somewhere unintended: a file system path (`/tmp/page.html`, which
/// would otherwise become `https://tmp/page.html`), a URL with a scheme the
/// browser cannot open (`ftp://host`), and pseudo-URLs such as `javascript:`
/// or `mailto:`.
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] when `url` is empty, looks like
/// a path, names an unsupported scheme, or is still not a valid URL after
/// `https://` has been prepended.
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
/// assert!(normalize_page_url("/tmp/page.html").is_err());
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
    if trimmed.starts_with(['/', '\\']) {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "Invalid URL: {url:?} looks like a file path; \
             use a file:// URL to open a local file"
        )));
    }
    if let Some(scheme) = foreign_scheme(trimmed) {
        return Err(SeleniumBaseError::InvalidConfig(format!(
            "Invalid URL: {url:?} uses the scheme {scheme:?}, which cannot be opened; \
             use http:, https:, file:, data:, about:, chrome: or edge:"
        )));
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
    fn a_scheme_in_capitals_is_still_a_scheme() {
        for url in [
            "HTTPS://EXAMPLE.COM/Path",
            "Http://example.com",
            "FILE:///tmp/x.html",
            "About:Blank",
            "DATA:text/plain,hi",
        ] {
            assert!(looks_like_a_page_url(url), "{url}");
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
    fn adds_https_to_hosts_with_ports_addresses_and_unicode() {
        for (input, expected) in [
            ("localhost:3000", "https://localhost:3000"),
            ("127.0.0.1:8080/health", "https://127.0.0.1:8080/health"),
            ("[::1]:3000/x", "https://[::1]:3000/x"),
            ("user@example.com", "https://user@example.com"),
            ("example.com/日本語", "https://example.com/日本語"),
            ("ünï.example", "https://ünï.example"),
            // A "://" after the host is part of the query, not a scheme.
            (
                "example.com/go?to=http://other.example",
                "https://example.com/go?to=http://other.example",
            ),
            ("example.com#http://x", "https://example.com#http://x"),
        ] {
            assert_eq!(normalize_page_url(input).unwrap(), expected, "{input}");
        }
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
    fn rejects_paths_instead_of_making_them_hosts() {
        for path in ["/tmp/page.html", "/", r"\\server\share\x.html", r"\x"] {
            let error = normalize_page_url(path).unwrap_err().to_string();
            assert!(error.contains("file path"), "{path}: {error}");
        }
    }

    #[test]
    fn rejects_schemes_the_browser_cannot_open() {
        for url in [
            "ftp://example.com/file",
            "ws://example.com/socket",
            "ssh://host",
            "gopher://host",
        ] {
            let error = normalize_page_url(url).unwrap_err().to_string();
            assert!(error.contains("scheme"), "{url}: {error}");
        }
    }

    #[test]
    fn javascript_and_mailto_never_become_a_url() {
        for url in [
            "javascript:alert(1)",
            "JavaScript:alert(document.cookie)",
            "mailto:someone@example.com",
            "tel:+15555550100",
            "blob:https://example.com/id",
        ] {
            let error = normalize_page_url(url);
            assert!(error.is_err(), "{url} -> {error:?}");
        }
    }

    #[test]
    fn a_foreign_scheme_is_told_apart_from_a_host_and_port() {
        assert_eq!(foreign_scheme("ftp://host"), Some("ftp"));
        assert_eq!(foreign_scheme("mailto:a@b.c"), Some("mailto"));
        assert_eq!(foreign_scheme("javascript:alert(1)"), Some("javascript"));
        assert_eq!(foreign_scheme("localhost:3000"), None);
        assert_eq!(foreign_scheme("example.com:8080/x"), None);
        assert_eq!(foreign_scheme("example.com:8080?q=1"), None);
        assert_eq!(foreign_scheme("127.0.0.1:80"), None);
        assert_eq!(foreign_scheme("[::1]:3000"), None);
        assert_eq!(foreign_scheme("example.com"), None);
        assert_eq!(foreign_scheme("example.com/a:b"), None);
        assert_eq!(foreign_scheme("example.com?to=http://x"), None);
    }

    #[test]
    fn rejects_text_that_is_not_a_host() {
        for url in ["not a url", "exa mple.com", "http s://x", "a b/c"] {
            assert!(normalize_page_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn the_error_quotes_what_was_given() {
        let error = normalize_page_url("exa mple.com").unwrap_err().to_string();
        assert!(error.contains(r#""exa mple.com""#), "{error}");
    }

    #[test]
    fn looks_like_a_page_url_matches_known_schemes() {
        assert!(looks_like_a_page_url("https://x.com"));
        assert!(looks_like_a_page_url("about:blank"));
        assert!(!looks_like_a_page_url("x.com"));
        assert!(!looks_like_a_page_url("/local/path"));
        assert!(!looks_like_a_page_url("htt"));
        assert!(!looks_like_a_page_url(""));
        // The prefix check must not split a multi-byte character.
        assert!(!looks_like_a_page_url("日本語日本語"));
    }
}
