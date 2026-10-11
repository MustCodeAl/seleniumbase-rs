//! Redirects by hand: which statuses are followed, where to, and what the next hop carries.
//!
//! The client never follows a redirect itself. The session does, one hop at a time, so that each
//! hop stores its `Set-Cookie` headers, sends the cookies that apply to its own URL, and carries
//! only the headers that may cross to where it is going:
//!
//! - `301` and `302` turn a `POST` into a `GET` without its body, `303` turns anything but
//!   `HEAD` into a `GET`, `307` and `308` keep the method and the body;
//! - a redirect to another origin (scheme, host or port) drops every header the caller set
//!   except `Accept` and `Content-Type`, so an `Authorization` or an API key never follows a
//!   redirect to a host the caller did not name;
//! - once a hop has changed site, the rest of the chain is cross-site for `SameSite` cookies;
//! - a target that is not http or https, or that carries credentials, is refused.

use bytes::Bytes;
use url::Url;
use wreq::Method;

use super::error::HttpError;
use super::reply::Headers;
use super::request::{check_target, Prepared, CROSS_ORIGIN_HEADERS};
use crate::scrape::egress::redact::RedactedUrl;
use crate::scrape::jar::store::site_of;

/// The longest `Location` value followed, in bytes.
pub(super) const MAX_LOCATION_BYTES: usize = 16 * 1024;
/// The statuses that are followed.
const REDIRECT_STATUSES: [u16; 5] = [301, 302, 303, 307, 308];

/// Where a request stands between two hops.
#[derive(Clone)]
pub(super) struct Hop {
    pub(super) method: Method,
    pub(super) url: Url,
    /// The headers the caller set, still to be sent.
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Option<Bytes>,
    /// A hop of the chain has changed site.
    pub(super) crossed_site: bool,
}

impl Hop {
    /// The first hop of a prepared request.
    pub(super) fn first(prepared: Prepared) -> Self {
        Self {
            method: prepared.method,
            url: prepared.url,
            headers: prepared.headers,
            body: prepared.body,
            crossed_site: false,
        }
    }

    /// The hop that follows this one after a redirect with `status` to `next`.
    pub(super) fn follow(self, status: u16, next: Url) -> Self {
        let same_origin = self.url.origin() == next.origin();
        let crossed_site = self.crossed_site || site_name(&self.url) != site_name(&next);
        let (method, keep_body) = rewrite(&self.method, status);
        let body = if keep_body { self.body } else { None };
        let headers = self
            .headers
            .into_iter()
            .filter(|(name, _)| {
                let lower = name.to_ascii_lowercase();
                let travels = same_origin || CROSS_ORIGIN_HEADERS.contains(&lower.as_str());
                travels && (keep_body || lower != "content-type")
            })
            .collect();
        Self {
            method,
            url: next,
            headers,
            body,
            crossed_site,
        }
    }
}

/// The registrable domain of a URL's host, or the host itself when it has none (an address).
pub(super) fn site_name(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    site_of(&host).unwrap_or(host)
}

/// Whether `status` is a redirect the session follows.
pub(super) fn is_redirect(status: u16) -> bool {
    REDIRECT_STATUSES.contains(&status)
}

/// The method of the next hop, and whether it keeps the body.
fn rewrite(method: &Method, status: u16) -> (Method, bool) {
    match status {
        303 if *method != Method::HEAD => (Method::GET, false),
        301 | 302 if *method == Method::POST => (Method::GET, false),
        _ => (method.clone(), true),
    }
}

/// The URL a redirect points to, or `None` when the answer is not a redirect that is followed.
///
/// # Errors
///
/// Returns [`HttpError::RedirectRefused`] when the `Location` is too long, is not a URL, is not
/// http or https, or carries credentials.
pub(super) fn target_of(
    status: u16,
    headers: &Headers,
    from: &Url,
    follow: bool,
) -> Result<Option<Url>, HttpError> {
    if !follow || !is_redirect(status) {
        return Ok(None);
    }
    let Some(location) = headers.get("location") else {
        return Ok(None);
    };
    let refuse = |reason: &'static str| HttpError::RedirectRefused {
        url: RedactedUrl::placeholder(),
        reason,
    };
    if location.len() > MAX_LOCATION_BYTES {
        return Err(refuse("the Location header is too long"));
    }
    let mut target = from
        .join(location.trim())
        .map_err(|_error| refuse("the Location header is not a URL"))?;
    check_target(&target).map_err(|error| HttpError::RedirectRefused {
        url: RedactedUrl::new(target.as_str()),
        reason: match error {
            HttpError::Url { reason } => reason,
            _ => "the target cannot be requested",
        },
    })?;
    target.set_fragment(None);
    Ok(Some(target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::header_spec::RequestShape;

    fn url(raw: &str) -> Url {
        Url::parse(raw).expect("a test URL")
    }

    fn headers(pairs: &[(&str, &str)]) -> Headers {
        Headers::new(
            pairs
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        )
    }

    fn hop(method: Method, at: &str, extra: &[(&str, &str)], body: Option<&'static str>) -> Hop {
        Hop::first(Prepared {
            method,
            url: url(at),
            shape: RequestShape::Fetch,
            headers: extra
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            body: body.map(|text| Bytes::from_static(text.as_bytes())),
            timeout: None,
        })
    }

    #[test]
    fn only_the_five_statuses_are_followed() {
        for status in [301, 302, 303, 307, 308] {
            assert!(is_redirect(status), "{status}");
        }
        for status in [200, 204, 300, 304, 305, 306, 309, 404, 500] {
            assert!(!is_redirect(status), "{status}");
        }
    }

    #[test]
    fn the_method_and_body_follow_the_status() {
        for (method, status, expect, keeps) in [
            (Method::POST, 301, Method::GET, false),
            (Method::POST, 302, Method::GET, false),
            (Method::POST, 303, Method::GET, false),
            (Method::PUT, 303, Method::GET, false),
            (Method::HEAD, 303, Method::HEAD, true),
            (Method::POST, 307, Method::POST, true),
            (Method::POST, 308, Method::POST, true),
            (Method::PUT, 301, Method::PUT, true),
            (Method::GET, 302, Method::GET, true),
        ] {
            let next = hop(method.clone(), "https://a.test/", &[], Some("body"))
                .follow(status, url("https://a.test/next"));
            assert_eq!(next.method, expect, "{method} {status}");
            assert_eq!(next.body.is_some(), keeps, "{method} {status}");
        }
    }

    #[test]
    fn content_type_goes_with_the_body() {
        let next = hop(
            Method::POST,
            "https://a.test/",
            &[("content-type", "application/json"), ("x-a", "1")],
            Some("{}"),
        )
        .follow(303, url("https://a.test/next"));
        assert_eq!(next.headers, [("x-a".to_owned(), "1".to_owned())]);
        let kept = hop(
            Method::POST,
            "https://a.test/",
            &[("content-type", "application/json")],
            Some("{}"),
        )
        .follow(307, url("https://a.test/next"));
        assert_eq!(kept.headers.len(), 1);
    }

    #[test]
    fn a_redirect_to_another_origin_drops_what_the_caller_set_but_accept_and_content_type() {
        let sent = [
            ("Authorization", "Bearer s3cret"),
            ("X-Api-Key", "k"),
            ("Accept", "application/json"),
            ("Content-Type", "text/plain"),
            ("Referer", "https://a.test/"),
        ];
        for target in [
            "https://b.test/x",
            "https://a.test:8443/x",
            "http://a.test/x",
            "https://sub.a.test/x",
        ] {
            let next =
                hop(Method::POST, "https://a.test/", &sent, Some("b")).follow(307, url(target));
            let names: Vec<&str> = next.headers.iter().map(|(name, _)| name.as_str()).collect();
            assert_eq!(names, ["Accept", "Content-Type"], "{target}");
        }
        let same =
            hop(Method::GET, "https://a.test/", &sent, None).follow(302, url("https://a.test/y"));
        assert_eq!(same.headers.len(), 5, "the same origin keeps every header");
    }

    #[test]
    fn a_chain_that_changed_site_stays_cross_site() {
        let there =
            hop(Method::GET, "https://a.test/", &[], None).follow(302, url("https://b.test/"));
        assert!(there.crossed_site);
        let back = there.follow(302, url("https://a.test/again"));
        assert!(
            back.crossed_site,
            "a chain that left the site never becomes same-site again"
        );
        let sub =
            hop(Method::GET, "https://a.test/", &[], None).follow(302, url("https://www.a.test/"));
        assert!(!sub.crossed_site, "a subdomain is the same site");
        let co =
            hop(Method::GET, "https://x.co.uk/", &[], None).follow(302, url("https://y.co.uk/"));
        assert!(co.crossed_site, "a public suffix separates sites");
    }

    #[test]
    fn the_target_is_resolved_against_the_url_and_a_fragment_is_dropped() {
        let from = url("https://a.test/dir/page?q=1");
        let location = |value: &str| {
            target_of(302, &headers(&[("location", value)]), &from, true)
                .expect("followed")
                .expect("a target")
        };
        assert_eq!(location("/abs").as_str(), "https://a.test/abs");
        assert_eq!(location("rel").as_str(), "https://a.test/dir/rel");
        assert_eq!(location("//b.test/p").as_str(), "https://b.test/p");
        assert_eq!(location("http://b.test/p#frag").as_str(), "http://b.test/p");
        assert_eq!(location("  /trim  ").as_str(), "https://a.test/trim");
    }

    #[test]
    fn nothing_is_followed_when_it_is_off_or_there_is_no_location() {
        let from = url("https://a.test/");
        let with = headers(&[("location", "/x")]);
        assert!(target_of(302, &with, &from, false).expect("ok").is_none());
        assert!(target_of(200, &with, &from, true).expect("ok").is_none());
        assert!(target_of(304, &with, &from, true).expect("ok").is_none());
        assert!(target_of(302, &headers(&[]), &from, true)
            .expect("ok")
            .is_none());
    }

    #[test]
    fn a_target_that_is_not_http_or_carries_credentials_is_refused() {
        let from = url("https://a.test/");
        for value in [
            "file:///etc/passwd",
            "ftp://b.test/",
            "javascript:alert(1)",
            "data:text/html,hi",
            "https://alice:s3cret@b.test/",
            "https://alice@b.test/",
        ] {
            let error =
                target_of(302, &headers(&[("location", value)]), &from, true).expect_err(value);
            assert!(
                matches!(error, HttpError::RedirectRefused { .. }),
                "{value}"
            );
            let shown = format!("{error} {error:?}");
            assert!(
                !shown.contains("s3cret") && !shown.contains("alice"),
                "{shown}"
            );
        }
        let long = format!("/{}", "a".repeat(MAX_LOCATION_BYTES));
        let error = target_of(302, &headers(&[("location", long.as_str())]), &from, true)
            .expect_err("too long");
        assert!(matches!(error, HttpError::RedirectRefused { .. }));
    }

    #[test]
    fn the_site_of_an_address_is_the_address() {
        assert_eq!(site_name(&url("https://127.0.0.1:8443/")), "127.0.0.1");
        assert_eq!(
            site_name(&url("https://a.b.example.co.uk/")),
            "example.co.uk"
        );
        assert_eq!(site_name(&url("https://localhost/")), "localhost");
    }
}
