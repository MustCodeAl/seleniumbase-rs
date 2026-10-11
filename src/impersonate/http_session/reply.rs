//! What a request answers: the status, the headers, the body, and the chain of redirects.
//!
//! A reply keeps the whole body in memory, already decompressed and within the session's limit.
//! Its `Debug` shows the status, the redacted URL and the sizes, never a header value that can
//! hold a credential and never the body.

use std::fmt;
use std::time::Duration;

use bytes::Bytes;
use serde::de::DeserializeOwned;
use url::Url;

use super::error::HttpError;
use crate::scrape::challenge::detect::{detect, Detection, ResponseView, MAX_SCAN_BYTES};
use crate::scrape::egress::redact::RedactedUrl;

/// The headers whose values `Debug` hides.
const HIDDEN_HEADERS: [&str; 5] = [
    "set-cookie",
    "set-cookie2",
    "cookie",
    "authorization",
    "proxy-authenticate",
];

/// The headers of a response, in the order received, with lower-case names.
///
/// The values of `Set-Cookie` and the credential headers are hidden from `Debug`.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Headers(Vec<(String, String)>);

impl Headers {
    /// Takes the pairs as they came, with the names lower-cased.
    pub(super) fn new(pairs: Vec<(String, String)>) -> Self {
        Self(pairs)
    }

    /// The first value of `name`, matched without regard to case.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Every value of `name`, matched without regard to case.
    pub fn get_all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.0
            .iter()
            .filter(move |(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Every header, in the order received.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    /// The pairs, for code that reads them as a slice.
    #[must_use]
    pub fn as_pairs(&self) -> &[(String, String)] {
        &self.0
    }

    /// How many headers there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no headers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Headers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();
        for (key, value) in &self.0 {
            if HIDDEN_HEADERS.contains(&key.as_str()) {
                map.entry(key, &"<redacted>");
            } else {
                map.entry(key, value);
            }
        }
        map.finish()
    }
}

/// The protocol a reply travelled on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplyProtocol {
    /// HTTP/2.
    Http2,
    /// HTTP/1.1 or older.
    Http1,
}

/// One answer, with its whole body.
///
/// ```no_run
/// use seleniumbase_rs::impersonate::http_session::HttpSession;
///
/// # async fn demo(session: HttpSession) -> Result<(), Box<dyn std::error::Error>> {
/// let reply = session.get("https://example.com/api/me").await?;
/// if reply.is_success() {
///     let me: serde_json::Value = reply.json()?;
///     println!("{me}");
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct HttpReply {
    pub(super) status: u16,
    pub(super) url: Url,
    pub(super) headers: Headers,
    pub(super) body: Bytes,
    pub(super) protocol: ReplyProtocol,
    pub(super) redirects: Vec<RedactedUrl>,
    pub(super) elapsed: Duration,
}

impl fmt::Debug for HttpReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpReply")
            .field("status", &self.status)
            .field("url", &self.redacted_url())
            .field("headers", &self.headers)
            .field("body_bytes", &self.body.len())
            .field("protocol", &self.protocol)
            .field("redirects", &self.redirects.len())
            .finish_non_exhaustive()
    }
}

impl HttpReply {
    /// The status code.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Whether the status is 2xx.
    #[must_use]
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The final URL, after any redirects, with credentials and secrets in the query removed.
    #[must_use]
    pub fn redacted_url(&self) -> RedactedUrl {
        RedactedUrl::new(self.url.as_str())
    }

    /// The final URL, after any redirects, as it was requested.
    ///
    /// The query can hold a token: use [`redacted_url`](Self::redacted_url) for anything logged.
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// The response headers.
    #[must_use]
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    /// The body, decompressed.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.body
    }

    /// The body as text.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::NotUtf8`] when the body is not UTF-8; the charset the server names
    /// is not consulted.
    pub fn text(&self) -> Result<&str, HttpError> {
        std::str::from_utf8(&self.body).map_err(|_error| HttpError::NotUtf8)
    }

    /// The body parsed as JSON.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::Json`] when the body is not a `T`. The text of the body is not part of
    /// the error: it may be a secret.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, HttpError> {
        serde_json::from_slice(&self.body).map_err(|error| HttpError::Json {
            kind: json_kind(&error),
        })
    }

    /// The protocol the reply travelled on.
    #[must_use]
    pub fn protocol(&self) -> ReplyProtocol {
        self.protocol
    }

    /// The URLs the request was redirected through before this reply, redacted, in order. Empty
    /// when it was answered at once.
    #[must_use]
    pub fn redirects(&self) -> &[RedactedUrl] {
        &self.redirects
    }

    /// How long the request took, redirects included.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Whether the reply is a bot-protection challenge, with the evidence.
    ///
    /// Only the first [`MAX_SCAN_BYTES`] of the body are read. A reply that is not valid UTF-8 is
    /// read with the broken bytes replaced.
    #[must_use]
    pub fn detect(&self) -> Detection {
        let window = self.body.get(..MAX_SCAN_BYTES).unwrap_or(&self.body);
        let body = String::from_utf8_lossy(window);
        let url = self.redacted_url();
        detect(&ResponseView {
            url: url.as_str(),
            status: self.status,
            headers: self.headers.as_pairs(),
            body: &body,
        })
    }
}

/// In which way a JSON failure happened, without the text it met.
fn json_kind(error: &serde_json::Error) -> &'static str {
    use serde_json::error::Category;
    match error.classify() {
        Category::Io => "the body could not be read",
        Category::Syntax => "the text is not valid JSON",
        Category::Data => "the data does not have the expected layout",
        Category::Eof => "the text ends too early",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrape::challenge::detect::ChallengeKind;

    fn reply(status: u16, headers: &[(&str, &str)], body: &[u8]) -> HttpReply {
        HttpReply {
            status,
            url: Url::parse("https://shop.example/a?token=hunter2&page=2").expect("url"),
            headers: Headers::new(
                headers
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                    .collect(),
            ),
            body: Bytes::copy_from_slice(body),
            protocol: ReplyProtocol::Http1,
            redirects: Vec::new(),
            elapsed: Duration::from_millis(5),
        }
    }

    #[test]
    fn headers_are_found_without_regard_to_case_and_keep_their_order() {
        let r = reply(
            200,
            &[("set-cookie", "a=1"), ("x-n", "1"), ("set-cookie", "b=2")],
            b"",
        );
        let all: Vec<&str> = r.headers().get_all("Set-Cookie").collect();
        assert_eq!(all, ["a=1", "b=2"]);
        assert_eq!(r.headers().get("X-N"), Some("1"));
        assert_eq!(r.headers().len(), 3);
        assert!(!r.headers().is_empty());
        assert_eq!(r.headers().iter().next(), Some(("set-cookie", "a=1")));
    }

    #[test]
    fn debug_hides_cookies_credentials_the_body_and_the_query_secret() {
        let r = reply(
            200,
            &[
                ("set-cookie", "sid=s3cret"),
                ("authorization", "Bearer t0ken"),
                ("server", "x"),
            ],
            b"private body",
        );
        let shown = format!("{r:?} {:?}", r.headers());
        for secret in ["s3cret", "t0ken", "private body", "hunter2"] {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }
        assert!(shown.contains("page=2") && shown.contains("body_bytes: 12"));
        assert!(shown.contains("server"));
    }

    #[test]
    fn text_needs_utf8_and_json_does_not_echo_the_body() {
        let good = reply(200, &[], br#"{"a":1}"#);
        assert_eq!(good.text().expect("utf8"), r#"{"a":1}"#);
        let value: serde_json::Value = good.json().expect("json");
        assert_eq!(value["a"], 1);

        let binary = reply(200, &[], &[0xff, 0xfe, 0x00]);
        assert!(matches!(binary.text(), Err(HttpError::NotUtf8)));

        let secret = reply(200, &[], b"password=hunter2");
        let error = secret.json::<serde_json::Value>().expect_err("not json");
        let shown = format!("{error} {error:?}");
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(matches!(error, HttpError::Json { .. }));
        let layout = reply(200, &[], b"[1]");
        assert!(matches!(
            layout.json::<std::collections::HashMap<String, u8>>(),
            Err(HttpError::Json { .. })
        ));
    }

    #[test]
    fn success_is_2xx() {
        assert!(reply(204, &[], b"").is_success());
        assert!(!reply(301, &[], b"").is_success());
        assert!(!reply(199, &[], b"").is_success());
    }

    #[test]
    fn a_challenge_page_is_detected_from_the_body_and_the_headers() {
        let page = reply(
            403,
            &[("server", "cloudflare"), ("cf-mitigated", "challenge")],
            b"<html><title>Just a moment...</title><div id=\"challenge-form\"></div></html>",
        );
        assert_ne!(
            page.detect().kind,
            ChallengeKind::None,
            "{:?}",
            page.detect()
        );
        let plain = reply(200, &[], b"<html><body>hello</body></html>");
        assert_eq!(plain.detect().kind, ChallengeKind::None);
    }

    #[test]
    fn a_marker_past_the_scan_window_is_not_read() {
        let mut body = vec![b'a'; MAX_SCAN_BYTES + 16];
        body.extend_from_slice(b"Just a moment...");
        let page = reply(200, &[], &body);
        assert_eq!(page.detect().kind, ChallengeKind::None);
    }

    #[test]
    fn a_rate_limit_status_is_a_verdict_with_its_wait() {
        let limited = reply(429, &[("retry-after", "7")], b"slow down");
        assert_eq!(
            limited.detect().kind,
            ChallengeKind::RateLimited {
                retry_after_secs: Some(7)
            }
        );
    }
}
