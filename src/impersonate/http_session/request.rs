//! A request being built: method, headers, body, and the checks that keep the browser profile whole.
//!
//! The profile owns the identity of every request (`User-Agent`, `Sec-CH-UA*`, `Accept-Language`,
//! `Accept-Encoding`, `Cookie`, `priority`) and the connection owns its framing (`Host`,
//! `Content-Length`, ...). A header from that set is refused when the request is built, not
//! silently overwritten. Every other header replaces the profile's value in place when the profile
//! lists it and is appended otherwise.

use bytes::Bytes;
use serde::Serialize;
use url::Url;
use wreq::header::{HeaderName, HeaderValue};
use wreq::Method;

use super::error::HttpError;
use super::reply::HttpReply;
use super::HttpSession;
use crate::scrape::egress::redact::RedactedUrl;
use crate::wire::header_spec::{
    RequestShape, MAX_HEADERS, MAX_NAME_LEN, MAX_VALUE_LEN, TRANSPORT_HEADERS,
};

/// Headers the browser profile owns, besides [`TRANSPORT_HEADERS`] and the `sec-ch-ua` family.
const PROFILE_HEADERS: [&str; 6] = [
    "user-agent",
    "accept-language",
    "accept-encoding",
    "cookie",
    "cookie2",
    "priority",
];
/// The prefix of the client-hint headers that name the browser.
const CLIENT_HINT_PREFIX: &str = "sec-ch-ua";
/// Methods a page can never send.
const FORBIDDEN_METHODS: [&str; 3] = ["CONNECT", "TRACE", "TRACK"];
/// The only caller headers that survive a redirect to another origin.
pub(super) const CROSS_ORIGIN_HEADERS: [&str; 2] = ["accept", "content-type"];
/// The content type of a JSON body.
const JSON_TYPE: &str = "application/json";
/// The content type of a form body.
const FORM_TYPE: &str = "application/x-www-form-urlencoded";

/// A request as it leaves [`HttpRequest::send`]: checked, and ready for the hop loop.
#[derive(Clone)]
pub(super) struct Prepared {
    pub(super) method: Method,
    pub(super) url: Url,
    pub(super) shape: RequestShape,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Option<Bytes>,
    pub(super) timeout: Option<std::time::Duration>,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Header values, the body and the query string can hold tokens.
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("Prepared")
            .field("method", &self.method)
            .field("url", &RedactedUrl::new(self.url.as_str()))
            .field("shape", &self.shape)
            .field("headers", &names)
            .field("body_bytes", &self.body.as_ref().map(Bytes::len))
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// Whether the profile or the connection owns the header `name` (any capitalisation).
pub(super) fn is_reserved(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with(CLIENT_HINT_PREFIX)
        || PROFILE_HEADERS.contains(&lower.as_str())
        || TRANSPORT_HEADERS.contains(&lower.as_str())
}

/// Checks a header name and value: not reserved, a valid token and value, within the bounds.
///
/// # Errors
///
/// Returns [`HttpError::ReservedHeader`] or [`HttpError::InvalidHeader`], naming the header and
/// never quoting its value.
pub(super) fn check_header(name: &str, value: &str) -> Result<(), HttpError> {
    if name.len() > MAX_NAME_LEN || value.len() > MAX_VALUE_LEN {
        return Err(HttpError::InvalidHeader(truncated(name)));
    }
    if is_reserved(name) {
        return Err(HttpError::ReservedHeader(name.to_ascii_lowercase()));
    }
    let valid = HeaderName::from_bytes(name.as_bytes()).is_ok()
        && HeaderValue::from_str(value).is_ok()
        && !name.starts_with(':');
    if valid {
        Ok(())
    } else {
        Err(HttpError::InvalidHeader(truncated(name)))
    }
}

/// A header name cut to a printable length, for an error message.
fn truncated(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_graphic)
        .take(64)
        .collect()
}

/// Checks a method.
///
/// # Errors
///
/// Returns [`HttpError::ForbiddenMethod`] for `CONNECT`, `TRACE` and `TRACK`.
pub(super) fn check_method(method: &Method) -> Result<(), HttpError> {
    if FORBIDDEN_METHODS
        .iter()
        .any(|forbidden| method.as_str().eq_ignore_ascii_case(forbidden))
    {
        return Err(HttpError::ForbiddenMethod(method.as_str().to_owned()));
    }
    Ok(())
}

/// The URL of a request: http or https, with a host and no credentials.
///
/// # Errors
///
/// Returns [`HttpError::Url`] with a reason that does not echo the URL.
pub(super) fn parse_url(raw: &str) -> Result<Url, HttpError> {
    let mut url = Url::parse(raw).map_err(|_error| HttpError::Url {
        reason: "it is not a URL",
    })?;
    check_target(&url)?;
    url.set_fragment(None);
    Ok(url)
}

/// Checks a URL that is about to be requested.
///
/// # Errors
///
/// Returns [`HttpError::Url`] for another scheme than http or https, a missing host, or
/// credentials in the URL.
pub(super) fn check_target(url: &Url) -> Result<(), HttpError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(HttpError::Url {
            reason: "only http and https can be requested",
        });
    }
    if url.host().is_none() {
        return Err(HttpError::Url {
            reason: "it has no host",
        });
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(HttpError::Url {
            reason: "it carries credentials; set an Authorization header instead",
        });
    }
    Ok(())
}

/// Puts the caller's headers into the profile's list: in place when the profile lists the name,
/// else before the HTTP/2 `priority` header, which goes last.
pub(super) fn merge_headers(list: &mut Vec<(String, String)>, extra: &[(String, String)]) {
    for (name, value) in extra {
        if let Some(slot) = list
            .iter_mut()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
        {
            slot.1.clone_from(value);
            continue;
        }
        let at = match list.last() {
            Some((tail, _)) if tail.eq_ignore_ascii_case("priority") => list.len() - 1,
            _ => list.len(),
        };
        list.insert(at, (name.clone(), value.clone()));
    }
}

/// A request being built; send it with [`send`](Self::send).
///
/// Created by [`HttpSession::request`]. The headers of a Chrome request are already set from the
/// measured profile; [`header`](Self::header) adds or replaces one, except the ones the profile
/// owns. A mistake in a setter is reported when the request is sent.
///
/// ```no_run
/// use seleniumbase_rs::impersonate::http_session::{HttpSession, Method};
/// use seleniumbase_rs::wire::header_spec::RequestShape;
///
/// # async fn demo(session: HttpSession) -> Result<(), Box<dyn std::error::Error>> {
/// let reply = session
///     .request(Method::POST, "https://example.com/api/search")
///     .shape(RequestShape::Fetch)
///     .header("x-requested-with", "XMLHttpRequest")
///     .json(&serde_json::json!({ "q": "shoes" }))
///     .send()
///     .await?;
/// # drop(reply);
/// # Ok(())
/// # }
/// ```
#[must_use = "a request does nothing until it is sent"]
pub struct HttpRequest<'a> {
    session: &'a HttpSession,
    method: Method,
    url: String,
    shape: Option<RequestShape>,
    headers: Vec<(String, String)>,
    body: Option<Bytes>,
    timeout: Option<std::time::Duration>,
    error: Option<HttpError>,
}

impl std::fmt::Debug for HttpRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Header values, the body and the query string can hold tokens.
        let names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field(
                "url",
                &crate::scrape::egress::redact::RedactedUrl::new(&self.url),
            )
            .field("headers", &names)
            .field("body_bytes", &self.body.as_ref().map(Bytes::len))
            .finish_non_exhaustive()
    }
}

impl<'a> HttpRequest<'a> {
    pub(super) fn new(session: &'a HttpSession, method: Method, url: &str) -> Self {
        Self {
            session,
            method,
            url: url.to_owned(),
            shape: None,
            headers: Vec::new(),
            body: None,
            timeout: None,
            error: None,
        }
    }

    /// Chooses the headers of a navigation ([`RequestShape::Document`], the default for `GET` and
    /// `HEAD`) or of a request a script makes ([`RequestShape::Fetch`], the default for the other
    /// methods).
    pub fn shape(mut self, shape: RequestShape) -> Self {
        self.shape = Some(shape);
        self
    }

    /// Sets a header, replacing the profile's value when it has one.
    ///
    /// A header the profile owns (`User-Agent`, `Sec-CH-UA*`, `Accept-Language`,
    /// `Accept-Encoding`, `Cookie`, `priority`) or the connection owns (`Host`, `Content-Length`,
    /// ...) is refused with [`HttpError::ReservedHeader`] when the request is sent. A header set
    /// here is dropped from a redirect to another origin, except `Accept` and `Content-Type`.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        if self.error.is_some() {
            return self;
        }
        if self.headers.len() >= MAX_HEADERS {
            self.error = Some(HttpError::InvalidHeader(truncated(name)));
            return self;
        }
        match check_header(name, value) {
            Ok(()) => {
                self.headers
                    .retain(|(known, _)| !known.eq_ignore_ascii_case(name));
                self.headers.push((name.to_owned(), value.to_owned()));
            }
            Err(error) => self.error = Some(error),
        }
        self
    }

    /// Sets the body as given. No `Content-Type` is set.
    pub fn body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Sets a JSON body and `Content-Type: application/json`.
    pub fn json<T: Serialize + ?Sized>(mut self, value: &T) -> Self {
        match serde_json::to_vec(value) {
            Ok(bytes) => {
                self.body = Some(Bytes::from(bytes));
                self.header("content-type", JSON_TYPE)
            }
            Err(_error) => {
                self.error.get_or_insert(HttpError::Json {
                    kind: "the value cannot be written as JSON",
                });
                self
            }
        }
    }

    /// Sets a form body and `Content-Type: application/x-www-form-urlencoded`.
    pub fn form(mut self, fields: &[(&str, &str)]) -> Self {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields)
            .finish();
        self.body = Some(Bytes::from(body.into_bytes()));
        self.header("content-type", FORM_TYPE)
    }

    /// Limits this request, redirects included, to `timeout`, in place of the session's.
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Checks the request and turns it into what the hop loop runs.
    pub(super) fn prepare(self) -> Result<(&'a HttpSession, Prepared), HttpError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        check_method(&self.method)?;
        let url = parse_url(&self.url)?;
        let limit = self.session.limits().max_request_bytes;
        if self.body.as_ref().is_some_and(|body| body.len() > limit) {
            return Err(HttpError::RequestTooLarge(limit));
        }
        let shape = self.shape.unwrap_or(
            if self.method == Method::GET || self.method == Method::HEAD {
                RequestShape::Document
            } else {
                RequestShape::Fetch
            },
        );
        let prepared = Prepared {
            method: self.method,
            url,
            shape,
            headers: self.headers,
            body: self.body,
            timeout: self.timeout,
        };
        Ok((self.session, prepared))
    }

    /// Sends the request, following redirects by hand, and reads the whole reply.
    ///
    /// A status other than 2xx is a reply, not an error.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::ReservedHeader`], [`HttpError::InvalidHeader`],
    /// [`HttpError::ForbiddenMethod`], [`HttpError::Url`] or [`HttpError::RequestTooLarge`] for a
    /// request that must not be sent; [`HttpError::TooManyRedirects`] and
    /// [`HttpError::RedirectRefused`] for a bad chain of redirects; [`HttpError::BodyTooLarge`]
    /// for a reply past the limit; and [`HttpError::Request`] when the connection, the TLS
    /// handshake or the time fails.
    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn send(self) -> Result<HttpReply, HttpError> {
        let (session, prepared) = self.prepare()?;
        super::fetch::run(session, prepared).await
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use super::*;
    use crate::impersonate::client::ImpersonationProfile;
    use crate::impersonate::http_session::HttpOptions;
    use crate::scrape::jar::store::CookieJar;
    use crate::wire::BrowserKind;

    fn session_with(options: HttpOptions) -> HttpSession {
        let profile = ImpersonationProfile::measured(BrowserKind::Chrome).expect("measured");
        HttpSession::assemble(profile, CookieJar::new(), None, options).expect("a session")
    }

    fn prepared(request: HttpRequest<'_>) -> Prepared {
        request.prepare().expect("a checked request").1
    }

    #[test]
    fn the_shape_follows_the_method_unless_it_is_chosen() {
        let session = session_with(HttpOptions::new());
        let url = "https://shop.example/";
        for (method, shape) in [
            (Method::GET, RequestShape::Document),
            (Method::HEAD, RequestShape::Document),
            (Method::POST, RequestShape::Fetch),
            (Method::PUT, RequestShape::Fetch),
            (Method::DELETE, RequestShape::Fetch),
        ] {
            let found = prepared(session.request(method.clone(), url)).shape;
            assert_eq!(found, shape, "{method}");
        }
        let chosen = prepared(
            session
                .request(Method::POST, url)
                .shape(RequestShape::Document),
        );
        assert_eq!(chosen.shape, RequestShape::Document);
    }

    #[test]
    fn json_and_form_set_their_bodies_and_content_types() {
        let session = session_with(HttpOptions::new());
        let url = "https://shop.example/";

        let json = prepared(
            session
                .request(Method::POST, url)
                .json(&serde_json::json!({ "a": 1 })),
        );
        let form = prepared(session.request(Method::POST, url).form(&[("k", "v w")]));
        let raw = prepared(session.request(Method::POST, url).body(&b"raw"[..]));

        assert_eq!(json.body.as_deref(), Some(&br#"{"a":1}"#[..]));
        assert_eq!(
            json.headers,
            [("content-type".to_owned(), "application/json".to_owned())]
        );
        assert_eq!(form.body.as_deref(), Some(&b"k=v+w"[..]));
        assert_eq!(form.headers[0].1, "application/x-www-form-urlencoded");
        assert_eq!(raw.body.as_deref(), Some(&b"raw"[..]));
        assert!(raw.headers.is_empty(), "a raw body names no content type");
    }

    #[test]
    fn a_json_value_that_cannot_be_written_fails_the_request_before_it_is_sent() {
        let session = session_with(HttpOptions::new());
        let mut keyed_by_a_pair: HashMap<(u8, u8), u8> = HashMap::new();
        keyed_by_a_pair.insert((1, 2), 3);

        let error = session
            .request(Method::POST, "https://shop.example/")
            .json(&keyed_by_a_pair)
            .prepare()
            .expect_err("not JSON");

        assert!(matches!(error, HttpError::Json { .. }), "{error}");
    }

    #[test]
    fn a_header_set_twice_keeps_the_last_value_and_the_first_error_wins() {
        let session = session_with(HttpOptions::new());
        let url = "https://shop.example/";

        let twice = prepared(
            session
                .request(Method::GET, url)
                .header("X-A", "1")
                .header("x-a", "2"),
        );
        let error = session
            .request(Method::GET, url)
            .header("cookie", "a=b")
            .header("user-agent", "x")
            .prepare()
            .expect_err("refused");

        assert_eq!(twice.headers, [("x-a".to_owned(), "2".to_owned())]);
        assert!(
            matches!(&error, HttpError::ReservedHeader(name) if name == "cookie"),
            "{error}"
        );
    }

    #[test]
    fn the_number_of_headers_is_capped() {
        let session = session_with(HttpOptions::new());
        let mut request = session.request(Method::GET, "https://shop.example/");
        for index in 0..=MAX_HEADERS {
            request = request.header(&format!("x-h{index}"), "v");
        }

        let error = request.prepare().expect_err("too many");

        assert!(matches!(error, HttpError::InvalidHeader(_)), "{error}");
    }

    #[test]
    fn a_body_over_the_sessions_limit_a_bad_url_and_a_forbidden_method_fail_in_prepare() {
        let session = session_with(HttpOptions::new().max_request_bytes(4));

        let big = session
            .request(Method::POST, "https://shop.example/")
            .body(&b"12345"[..])
            .prepare()
            .expect_err("too large");
        let fits = session
            .request(Method::POST, "https://shop.example/")
            .body(&b"1234"[..])
            .prepare();
        let url = session
            .request(Method::GET, "gopher://x/")
            .prepare()
            .expect_err("scheme");
        let method = session
            .request(Method::TRACE, "https://shop.example/")
            .prepare()
            .expect_err("method");

        assert!(matches!(big, HttpError::RequestTooLarge(4)), "{big}");
        assert!(fits.is_ok());
        assert!(matches!(url, HttpError::Url { .. }), "{url}");
        assert!(matches!(method, HttpError::ForbiddenMethod(_)), "{method}");
    }

    #[test]
    fn a_per_request_timeout_is_carried_and_debug_hides_everything_private() {
        let session = session_with(HttpOptions::new());
        let request = session
            .request(
                Method::POST,
                "https://alice:s3cret@shop.example/a?token=hunter2&page=2",
            )
            .header("authorization", "Bearer t0ken")
            .body(&b"private body"[..])
            .timeout(Duration::from_secs(3));
        let shown = format!("{request:?}");

        assert!(
            !shown.contains("s3cret") && !shown.contains("hunter2"),
            "{shown}"
        );
        assert!(
            !shown.contains("t0ken") && !shown.contains("private body"),
            "{shown}"
        );
        assert!(
            shown.contains("authorization") && shown.contains("body_bytes: Some(12)"),
            "{shown}"
        );
        let plain = session
            .request(Method::GET, "https://shop.example/")
            .timeout(Duration::from_secs(3));
        assert_eq!(prepared(plain).timeout, Some(Duration::from_secs(3)));
    }

    #[test]
    fn the_profile_and_the_connection_own_their_headers() {
        for name in [
            "User-Agent",
            "user-agent",
            "Sec-CH-UA",
            "sec-ch-ua-platform",
            "Sec-CH-UA-Full-Version-List",
            "Accept-Language",
            "Accept-Encoding",
            "Cookie",
            "Cookie2",
            "Priority",
            "Host",
            "Content-Length",
            "Transfer-Encoding",
            "Proxy-Authorization",
            "Connection",
            "TE",
            "Upgrade",
        ] {
            assert!(is_reserved(name), "{name}");
            assert!(
                matches!(check_header(name, "x"), Err(HttpError::ReservedHeader(_))),
                "{name}"
            );
        }
        for name in [
            "Authorization",
            "Accept",
            "Content-Type",
            "X-Requested-With",
            "Referer",
        ] {
            assert!(!is_reserved(name), "{name}");
            assert!(check_header(name, "x").is_ok(), "{name}");
        }
    }

    #[test]
    fn a_header_with_a_line_break_or_a_bad_name_is_refused_without_echoing_the_value() {
        let injected = check_header("x-a", "ok\r\nx-injected: 1").expect_err("refused");
        assert!(matches!(&injected, HttpError::InvalidHeader(name) if name == "x-a"));
        let shown = format!("{injected} {injected:?}");
        assert!(!shown.contains("injected"), "{shown}");
        for name in ["", "bad name", "x\r\ny", ":authority", "x:y"] {
            assert!(
                matches!(check_header(name, "v"), Err(HttpError::InvalidHeader(_))),
                "{name:?}"
            );
        }
        let long = "a".repeat(MAX_NAME_LEN + 1);
        assert!(matches!(
            check_header(&long, "v"),
            Err(HttpError::InvalidHeader(_))
        ));
        let value = "v".repeat(MAX_VALUE_LEN + 1);
        assert!(matches!(
            check_header("x-a", &value),
            Err(HttpError::InvalidHeader(_))
        ));
    }

    #[test]
    fn a_secret_in_a_refused_header_value_never_reaches_the_error() {
        let error = check_header("authorization", "Bearer s3cret\nx").expect_err("refused");
        let shown = format!("{error} {error:?}");
        assert!(!shown.contains("s3cret"), "{shown}");
    }

    #[test]
    fn the_methods_a_page_cannot_send_are_refused() {
        for method in ["CONNECT", "TRACE", "TRACK"] {
            let method = Method::from_bytes(method.as_bytes()).expect("a method");
            assert!(matches!(
                check_method(&method),
                Err(HttpError::ForbiddenMethod(_))
            ));
        }
        for method in [
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::HEAD,
        ] {
            assert!(check_method(&method).is_ok());
        }
    }

    #[test]
    fn a_request_url_is_http_with_a_host_and_no_credentials() {
        let ok = parse_url("https://shop.example/a?b=1#frag").expect("url");
        assert_eq!(ok.as_str(), "https://shop.example/a?b=1");
        for (raw, reason) in [
            ("shop.example/a", "it is not a URL"),
            (
                "ftp://shop.example/",
                "only http and https can be requested",
            ),
            ("file:///etc/passwd", "only http and https can be requested"),
            ("data:text/plain,hi", "only http and https can be requested"),
            (
                "https://alice:s3cret@shop.example/",
                "it carries credentials; set an Authorization header instead",
            ),
            (
                "https://alice@shop.example/",
                "it carries credentials; set an Authorization header instead",
            ),
        ] {
            let error = parse_url(raw).expect_err(raw);
            assert!(
                matches!(&error, HttpError::Url { reason: found } if *found == reason),
                "{raw}: {error}"
            );
            let shown = format!("{error} {error:?}");
            assert!(
                !shown.contains("s3cret") && !shown.contains("alice"),
                "{shown}"
            );
        }
    }

    #[test]
    fn caller_headers_replace_in_place_or_go_before_priority() {
        let mut list = vec![
            ("Accept".to_owned(), "text/html".to_owned()),
            ("Sec-Fetch-Dest".to_owned(), "document".to_owned()),
            ("priority".to_owned(), "u=0, i".to_owned()),
        ];
        merge_headers(
            &mut list,
            &[
                ("accept".to_owned(), "application/json".to_owned()),
                ("Authorization".to_owned(), "Bearer t".to_owned()),
            ],
        );
        let names: Vec<&str> = list.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            ["Accept", "Sec-Fetch-Dest", "Authorization", "priority"]
        );
        assert_eq!(list[0].1, "application/json");
    }

    #[test]
    fn without_a_priority_header_extras_go_last() {
        let mut list = vec![("Accept".to_owned(), "*/*".to_owned())];
        merge_headers(&mut list, &[("X-A".to_owned(), "1".to_owned())]);
        assert_eq!(list.last().map(|(name, _)| name.as_str()), Some("X-A"));
    }
}
