//! Redacts URLs, userinfo and sensitive query values so that no secret reaches `Debug`, `Display`, state or logs.

use std::fmt;

use serde::Serialize;

/// Query keys whose values are replaced by `***`. Matching ignores ASCII case.
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
    /// Redacts `raw`. A string that does not parse as a URL becomes a fixed placeholder.
    #[must_use]
    pub fn new(raw: &str) -> Self {
        let Ok(mut url) = url::Url::parse(raw) else {
            return Self(UNPARSEABLE.to_owned());
        };
        // Userinfo and the fragment are never kept, whatever the query says.
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_fragment(None);
        if url.query().is_some() {
            let pairs: Vec<(String, String)> = url
                .query_pairs()
                .map(|(key, value)| {
                    let value = if is_sensitive(&key) {
                        REDACTED_VALUE.to_owned()
                    } else {
                        value.into_owned()
                    };
                    (key.into_owned(), value)
                })
                .collect();
            url.query_pairs_mut().clear().extend_pairs(pairs);
        }
        Self(url.to_string())
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

fn is_sensitive(key: &str) -> bool {
    SENSITIVE_QUERY_KEYS
        .iter()
        .any(|sensitive| key.eq_ignore_ascii_case(sensitive))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stealth::humanize::Rng;

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
    fn sensitive_query_values_are_replaced_whatever_the_case() {
        let url = RedactedUrl::new("https://example.com/a?token=abc&page=2&API_KEY=zz");
        assert_eq!(
            url.as_str(),
            "https://example.com/a?token=***&page=2&API_KEY=***"
        );
    }

    #[test]
    fn an_unparseable_string_is_replaced_not_echoed() {
        let url = RedactedUrl::new("not a url password=hunter2");
        assert_eq!(url.as_str(), UNPARSEABLE);
        assert!(!format!("{url:?} {url}").contains("hunter2"));
    }

    #[test]
    fn secret_prints_redacted_and_exposes_only_on_request() {
        let secret = Secret::new("hunter2");
        assert_eq!(format!("{secret:?}"), "<redacted>");
        assert_eq!(format!("{secret}"), "<redacted>");
        assert_eq!(secret.expose(), "hunter2");
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
            let json = serde_json::to_string(&url).unwrap_or_default();
            for output in [format!("{url:?}"), format!("{url}"), json] {
                for secret in [&user, &pass, &token] {
                    assert!(!output.contains(secret.as_str()), "{secret} in {output}");
                }
            }
        }
    }
}
