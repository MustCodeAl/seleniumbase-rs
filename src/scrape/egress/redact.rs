//! Redacts URLs, userinfo and sensitive query values so that no secret reaches `Debug`, `Display`, state or logs.

use std::fmt;

use serde::Serialize;

/// Query keys whose values are replaced by `***`. Matching ignores ASCII case and sees through
/// percent-encoding, so `to%6Ben` is `token`.
pub const SENSITIVE_QUERY_KEYS: [&str; 12] = [
    "token",
    "access_token",
    "key",
    "api_key",
    "sig",
    "signature",
    "password",
    "pass",
    "auth",
    "session",
    "sid",
    "secret",
];

/// What a sensitive query value becomes.
const REDACTED_VALUE: &str = "***";

/// What a string that does not parse as a URL becomes. The input is never echoed.
const UNPARSEABLE: &str = "<unparseable url>";

/// What follows the scheme of a URL that has no host, such as `data:` or `mailto:`. Such a URL
/// can hold anything, userinfo included, so none of it is kept.
const OPAQUE: &str = "<opaque>";

/// A secret string. It prints as `<redacted>` and is never serialized. [`Secret::expose`] is the
/// one way out, and it belongs where the value must leave the process.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wraps a secret value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The secret value, for the one consumer that needs it, such as a launch command.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// A URL with its userinfo and fragment removed and its sensitive query values replaced. The
/// only constructor redacts, so every value of this type is safe to log and to serialize.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct RedactedUrl(String);

impl RedactedUrl {
    /// Redacts `raw`. A string that does not parse as a URL becomes a fixed placeholder, and a
    /// URL without a host (`data:`, `mailto:`) keeps only its scheme.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        let Ok(mut url) = url::Url::parse(raw) else {
            return Self(UNPARSEABLE.to_owned());
        };
        if url.cannot_be_a_base() {
            return Self(format!("{}:{OPAQUE}", url.scheme()));
        }
        // Userinfo and the fragment are never kept, whatever the query says. Both setters only
        // fail for a URL that cannot carry userinfo, and there is none to remove then.
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_fragment(None);
        if let Some(query) = url.query() {
            let redacted = redact_query(query);
            url.set_query(Some(&redacted));
        }
        Self(url.to_string())
    }

    /// The stand-in for an address that could not be read, for an error that must not echo what
    /// it was given.
    #[must_use]
    pub fn placeholder() -> Self {
        Self(UNPARSEABLE.to_owned())
    }

    /// The redacted URL as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for RedactedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for RedactedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Replaces the value of every sensitive `key=value` pair of `query` and keeps every other byte.
fn redact_query(query: &str) -> String {
    let pairs: Vec<String> = query
        .split('&')
        .map(|segment| match segment.split_once('=') {
            Some((key, _)) if is_sensitive(key) => format!("{key}={REDACTED_VALUE}"),
            _ => segment.to_owned(),
        })
        .collect();
    pairs.join("&")
}

/// Whether the (possibly percent-encoded) query key `raw_key` names a secret.
fn is_sensitive(raw_key: &str) -> bool {
    url::form_urlencoded::parse(raw_key.as_bytes())
        .next()
        .is_some_and(|(key, _)| {
            SENSITIVE_QUERY_KEYS
                .iter()
                .any(|sensitive| key.eq_ignore_ascii_case(sensitive))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::humanize::Rng;

    /// Compiles only for a type that does not implement `Serialize`: for one that does, the
    /// call below is ambiguous. This is how a test proves a secret type cannot be serialized.
    trait AmbiguousIfSerialize<A> {
        fn check() {}
    }
    impl<T: ?Sized> AmbiguousIfSerialize<()> for T {}
    impl<T: ?Sized + Serialize> AmbiguousIfSerialize<u8> for T {}

    #[test]
    fn a_secret_cannot_be_serialized() {
        <Secret as AmbiguousIfSerialize<_>>::check();
    }

    #[test]
    fn userinfo_and_fragment_never_survive() {
        let url = RedactedUrl::new("https://alice:s3cret@example.com/path?page=2#frag");
        let text = url.as_str();
        assert!(text.contains("example.com/path"), "{text}");
        assert!(text.contains("page=2"), "{text}");
        for secret in ["alice", "s3cret", "frag"] {
            assert!(!text.contains(secret), "{secret} leaked into {text}");
        }
    }

    #[test]
    fn a_proxy_url_loses_its_credentials_and_keeps_its_address() {
        let url = RedactedUrl::new("socks5://user:pw@10.0.0.1:1080");
        assert_eq!(url.as_str(), "socks5://10.0.0.1:1080");
    }

    #[test]
    fn sensitive_query_values_are_replaced_whatever_the_case() {
        let url = RedactedUrl::new("https://example.com/a?token=abc&page=2&API_KEY=zz");
        assert_eq!(
            url.as_str(),
            "https://example.com/a?token=***&page=2&API_KEY=***"
        );
    }

    #[test]
    fn every_listed_key_is_redacted() {
        for key in SENSITIVE_QUERY_KEYS {
            let url = RedactedUrl::new(&format!("https://example.com/?{key}=VALUE&n=1"));
            assert_eq!(
                url.as_str(),
                format!("https://example.com/?{key}=***&n=1"),
                "{key}"
            );
        }
    }

    #[test]
    fn a_key_is_matched_after_percent_decoding_and_in_every_repeat() {
        let url = RedactedUrl::new("https://example.com/?to%6Ben=a&Token=b&n=1&token=c&sid=");
        assert_eq!(
            url.as_str(),
            "https://example.com/?to%6Ben=***&Token=***&n=1&token=***&sid=***"
        );
    }

    #[test]
    fn query_text_that_is_not_secret_is_kept_byte_for_byte() {
        let raw = "https://example.com/s?q=a%20b+c&lang=de&empty=&flag&x=1=2";
        assert_eq!(RedactedUrl::new(raw).as_str(), raw);
    }

    #[test]
    fn a_key_that_only_contains_a_sensitive_word_is_not_redacted() {
        let raw = "https://example.com/?tokens=1&keyboard=us&passenger=2";
        assert_eq!(RedactedUrl::new(raw).as_str(), raw);
    }

    #[test]
    fn a_token_in_the_fragment_goes_with_the_fragment() {
        let url = RedactedUrl::new("https://example.com/cb#access_token=abc123&state=x");
        assert_eq!(url.as_str(), "https://example.com/cb");
    }

    #[test]
    fn redacting_twice_changes_nothing() {
        let once = RedactedUrl::new("https://u:p@example.com/a?token=abc&q=1#f");
        assert_eq!(RedactedUrl::new(once.as_str()), once);
    }

    #[test]
    fn an_unparseable_string_is_replaced_not_echoed() {
        let url = RedactedUrl::new("not a url password=hunter2");
        assert_eq!(url.as_str(), UNPARSEABLE);
        assert!(!format!("{url:?} {url}").contains("hunter2"));
    }

    #[test]
    fn a_spec_without_a_scheme_never_echoes_its_userinfo() {
        for raw in [
            "alice:s3cret@:1",
            "alice:s3cret@proxy.example:8080",
            "user:pw@host",
        ] {
            let shown = RedactedUrl::new(raw);
            let text = shown.as_str();
            assert!(
                !text.contains("s3cret") && !text.contains("pw@"),
                "{raw}: {text}"
            );
        }
    }

    #[test]
    fn a_url_without_a_host_keeps_only_its_scheme() {
        assert_eq!(
            RedactedUrl::new("data:text/plain;base64,c2VjcmV0").as_str(),
            "data:<opaque>"
        );
        assert_eq!(
            RedactedUrl::new("mailto:bob@example.com?subject=hi").as_str(),
            "mailto:<opaque>"
        );
    }

    #[test]
    fn secret_prints_redacted_and_exposes_only_on_request() {
        let secret = Secret::new("hunter2");
        assert_eq!(format!("{secret:?}"), "<redacted>");
        assert_eq!(format!("{secret}"), "<redacted>");
        assert_eq!(format!("{secret:#?}"), "<redacted>");
        assert_eq!(secret.expose(), "hunter2");
    }

    #[test]
    fn a_redacted_url_serializes_as_a_bare_string() {
        let url = RedactedUrl::new("https://example.com/?token=abc");
        assert_eq!(
            serde_json::to_string(&url).expect("serializes"),
            "\"https://example.com/?token=***\""
        );
    }

    #[test]
    fn no_secret_reaches_debug_display_or_serde_for_a_seeded_run() {
        // A fixed seed makes every run check the same 64 generated secrets.
        let mut rng = Rng::new(7);
        for _ in 0..64 {
            let user = format!("u{:x}", rng.next_u64());
            let pass = format!("p{:x}", rng.next_u64());
            let token = format!("t{:x}", rng.next_u64());
            let raw = format!("https://{user}:{pass}@example.com/x?access_token={token}&n=1");
            let url = RedactedUrl::new(&raw);
            let json = serde_json::to_string(&url).expect("serializes");
            for output in [format!("{url:?}"), format!("{url}"), json] {
                for secret in [&user, &pass, &token] {
                    assert!(!output.contains(secret.as_str()), "{secret} in {output}");
                }
            }
            let wrapped = Secret::new(pass.clone());
            let output = format!("{wrapped:?} {wrapped}");
            assert!(!output.contains(pass.as_str()), "{pass} in {output}");
        }
    }
}
