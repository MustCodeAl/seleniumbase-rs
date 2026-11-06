//! The cookie both engines share.
//!
//! [`Cookie`] is what [`BaseCase::get_cookie`](crate::BaseCase::get_cookie) and
//! [`BaseCase::get_cookies`](crate::BaseCase::get_cookies) return and what
//! [`BaseCase::add_cookie`](crate::BaseCase::add_cookie) takes in the WebDriver engine, and what
//! [`Cookies`](crate::sb_cdp::Cookies) hands out and takes in the pure CDP engine. It is one type,
//! with the same fields and the same defaults, whichever engine the browser is driven by.
//!
//! The conversions to and from the WebDriver library's cookie are private to the crate: no
//! public item here names a type of that library.

use std::fmt;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thirtyfour::cookie::{Cookie as WebDriverCookie, SameSite as WebDriverSameSite};

/// The `SameSite` attribute of a cookie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SameSite {
    /// Sent only for same-site requests.
    Strict,
    /// Sent for same-site requests and top-level navigations.
    Lax,
    /// Sent for all requests; requires `secure`.
    None,
}

/// The path of a cookie that names none.
fn root_path() -> String {
    "/".to_owned()
}

/// A browser cookie.
///
/// The value is a credential, so [`Debug`](fmt::Debug) hides it.
///
/// The same type comes out of both engines. `domain` is empty and `path` is `/` when the
/// browser reported none, `secure` and `http_only` are `false` when it reported none, and
/// `expires` is `None` for a session cookie (a negative expiry, which the DevTools Protocol
/// uses for "session", reads as `None` too).
///
/// It deserializes from the W3C cookie object that Python SeleniumBase saves as well: `expiry`
/// is an alias of `expires`, and a missing `path`, `domain`, `secure` or `httpOnly` takes the
/// default above.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::Cookie;
///
/// let cookie = Cookie::new("session", "s3cret").domain("example.com").secure(true);
/// assert!(!format!("{cookie:?}").contains("s3cret"));
///
/// let saved: Cookie = serde_json::from_str(r#"{"name": "sid", "value": "abc", "expiry": 4102444800}"#)?;
/// assert_eq!(saved.path, "/");
/// assert_eq!(saved.expires, Some(4_102_444_800.0));
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cookie {
    /// The cookie name.
    pub name: String,
    /// The cookie value.
    pub value: String,
    /// The domain it applies to; empty when the browser reported none.
    #[serde(default)]
    pub domain: String,
    /// The path it applies to.
    #[serde(default = "root_path")]
    pub path: String,
    /// Expiry as seconds since the Unix epoch; `None` for a session cookie.
    #[serde(alias = "expiry")]
    pub expires: Option<f64>,
    /// Hidden from page scripts.
    #[serde(default)]
    pub http_only: bool,
    /// Sent only over HTTPS.
    #[serde(default)]
    pub secure: bool,
    /// The `SameSite` policy, if one is set.
    pub same_site: Option<SameSite>,
}

impl Cookie {
    /// Creates a session cookie for the current page's site.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            domain: String::new(),
            path: "/".to_owned(),
            expires: None,
            http_only: false,
            secure: false,
            same_site: None,
        }
    }

    /// Sets the domain.
    #[must_use]
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = domain.into();
        self
    }

    /// Sets the path.
    #[must_use]
    pub fn path(mut self, path: impl Into<String>) -> Self {
        self.path = path.into();
        self
    }

    /// Sets the expiry, in seconds since the Unix epoch.
    #[must_use]
    pub fn expires(mut self, expires: f64) -> Self {
        self.expires = Some(expires);
        self
    }

    /// Hides the cookie from page scripts.
    #[must_use]
    pub fn http_only(mut self, http_only: bool) -> Self {
        self.http_only = http_only;
        self
    }

    /// Restricts the cookie to HTTPS.
    #[must_use]
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// Sets the `SameSite` policy.
    #[must_use]
    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = Some(same_site);
        self
    }

    /// A cookie as the DevTools Protocol reports it (`Network.getCookies`,
    /// `Network.getAllCookies`). A negative `expires` is the protocol's mark of a session
    /// cookie and reads as `None`.
    pub(crate) fn from_protocol(value: &Value) -> Self {
        let expires = value["expires"].as_f64().filter(|seconds| *seconds >= 0.0);
        Self {
            name: value["name"].as_str().unwrap_or_default().to_owned(),
            value: value["value"].as_str().unwrap_or_default().to_owned(),
            domain: value["domain"].as_str().unwrap_or_default().to_owned(),
            path: value["path"].as_str().unwrap_or("/").to_owned(),
            expires,
            http_only: value["httpOnly"].as_bool().unwrap_or_default(),
            secure: value["secure"].as_bool().unwrap_or_default(),
            same_site: match value["sameSite"].as_str() {
                Some("Strict") => Some(SameSite::Strict),
                Some("Lax") => Some(SameSite::Lax),
                Some("None") => Some(SameSite::None),
                _ => None,
            },
        }
    }

    /// A cookie as `Network.setCookies` takes it.
    pub(crate) fn to_protocol(&self) -> Value {
        let mut cookie = json!({
            "name": self.name,
            "value": self.value,
            "path": self.path,
            "httpOnly": self.http_only,
            "secure": self.secure,
        });
        if !self.domain.is_empty() {
            cookie["domain"] = json!(self.domain);
        }
        if let Some(expires) = self.expires {
            cookie["expires"] = json!(expires);
        }
        if let Some(same_site) = self.same_site {
            cookie["sameSite"] = json!(match same_site {
                SameSite::Strict => "Strict",
                SameSite::Lax => "Lax",
                SameSite::None => "None",
            });
        }
        cookie
    }

    /// A cookie as the WebDriver library reports it.
    ///
    /// A missing `path` becomes `/`, a missing `domain` becomes empty, a missing flag becomes
    /// `false`, the whole-second expiry becomes `expires` (a negative one reads as a session
    /// cookie), and the library's `Default` policy, which means "let the driver pick", becomes
    /// no policy.
    pub(crate) fn from_webdriver(cookie: &WebDriverCookie) -> Self {
        Self {
            name: cookie.name.clone(),
            value: cookie.value.clone(),
            domain: cookie.domain.clone().unwrap_or_default(),
            path: cookie.path.clone().unwrap_or_else(root_path),
            expires: cookie
                .expiry
                .filter(|seconds| *seconds >= 0)
                .map(|seconds| seconds as f64),
            http_only: cookie.http_only.unwrap_or_default(),
            secure: cookie.secure.unwrap_or_default(),
            same_site: match cookie.same_site {
                Some(WebDriverSameSite::Strict) => Some(SameSite::Strict),
                Some(WebDriverSameSite::Lax) => Some(SameSite::Lax),
                Some(WebDriverSameSite::None) => Some(SameSite::None),
                Some(WebDriverSameSite::Default) | None => None,
            },
        }
    }

    /// The cookie as the WebDriver library takes it.
    ///
    /// An empty `domain` is left out, so the browser uses the page's. The expiry is cut to
    /// whole seconds, as WebDriver counts them, and a negative one (a session cookie) is left
    /// out.
    pub(crate) fn to_webdriver(&self) -> WebDriverCookie {
        let mut cookie = WebDriverCookie::new(self.name.as_str(), self.value.as_str());
        cookie.set_path(self.path.as_str());
        if !self.domain.is_empty() {
            cookie.set_domain(self.domain.as_str());
        }
        cookie.set_secure(self.secure);
        cookie.set_http_only(self.http_only);
        if let Some(seconds) = self.expires.filter(|seconds| *seconds >= 0.0) {
            cookie.set_expiry(seconds.trunc() as i64);
        }
        if let Some(same_site) = self.same_site {
            cookie.set_same_site(match same_site {
                SameSite::Strict => WebDriverSameSite::Strict,
                SameSite::Lax => WebDriverSameSite::Lax,
                SameSite::None => WebDriverSameSite::None,
            });
        }
        cookie
    }

    /// Whether `pattern` matches the domain, name or value.
    pub(crate) fn matches(&self, pattern: &Regex) -> bool {
        pattern.is_match(&self.domain)
            || pattern.is_match(&self.name)
            || pattern.is_match(&self.value)
    }
}

impl fmt::Debug for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cookie")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .field("domain", &self.domain)
            .field("path", &self.path)
            .field("expires", &self.expires)
            .field("http_only", &self.http_only)
            .field("secure", &self.secure)
            .field("same_site", &self.same_site)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WebDriver cookie with every field set.
    fn full_webdriver_cookie() -> WebDriverCookie {
        let mut cookie = WebDriverCookie::new("sid", "abc");
        cookie.set_path("/shop");
        cookie.set_domain(".example.test");
        cookie.set_secure(true);
        cookie.set_http_only(true);
        cookie.set_expiry(4_102_444_800);
        cookie.set_same_site(WebDriverSameSite::Strict);
        cookie
    }

    #[test]
    fn a_webdriver_cookie_with_every_field_set_converts_field_by_field() {
        let cookie = Cookie::from_webdriver(&full_webdriver_cookie());
        assert_eq!(
            cookie,
            Cookie::new("sid", "abc")
                .path("/shop")
                .domain(".example.test")
                .secure(true)
                .http_only(true)
                .expires(4_102_444_800.0)
                .same_site(SameSite::Strict)
        );
    }

    #[test]
    fn a_webdriver_cookie_with_nothing_but_a_name_and_value_gets_the_defaults() {
        let cookie = Cookie::from_webdriver(&WebDriverCookie::new("a", "b"));
        assert_eq!(cookie.path, "/");
        assert_eq!(cookie.domain, "");
        assert!(!cookie.secure);
        assert!(!cookie.http_only);
        assert_eq!(cookie.expires, None, "no expiry is a session cookie");
        assert_eq!(cookie.same_site, None);
        assert_eq!(cookie, Cookie::new("a", "b"));
    }

    #[test]
    fn the_default_policy_of_the_driver_is_no_policy() {
        for (theirs, ours) in [
            (WebDriverSameSite::Strict, Some(SameSite::Strict)),
            (WebDriverSameSite::Lax, Some(SameSite::Lax)),
            (WebDriverSameSite::None, Some(SameSite::None)),
            (WebDriverSameSite::Default, None),
        ] {
            let mut cookie = WebDriverCookie::new("a", "b");
            cookie.set_same_site(theirs);
            assert_eq!(
                Cookie::from_webdriver(&cookie).same_site,
                ours,
                "{theirs:?}"
            );
        }
    }

    #[test]
    fn a_negative_expiry_is_a_session_cookie_on_every_path() {
        let mut webdriver = WebDriverCookie::new("a", "b");
        webdriver.set_expiry(-1);
        assert_eq!(Cookie::from_webdriver(&webdriver).expires, None);

        let protocol = json!({ "name": "a", "value": "b", "expires": -1 });
        assert_eq!(Cookie::from_protocol(&protocol).expires, None);
        let protocol = json!({ "name": "a", "value": "b", "expires": -1.0 });
        assert_eq!(Cookie::from_protocol(&protocol).expires, None);

        // Going back, a negative expiry is not sent as a date in 1969.
        let session = Cookie {
            expires: Some(-1.0),
            ..Cookie::new("a", "b")
        };
        assert_eq!(session.to_webdriver().expiry, None);
    }

    #[test]
    fn a_cookie_goes_to_the_webdriver_library_field_by_field() {
        let cookie = Cookie::new("sid", "abc")
            .path("/shop")
            .domain(".example.test")
            .secure(true)
            .http_only(true)
            .expires(4_102_444_800.0)
            .same_site(SameSite::Lax);
        let theirs = cookie.to_webdriver();
        assert_eq!(theirs.name, "sid");
        assert_eq!(theirs.value, "abc");
        assert_eq!(theirs.path.as_deref(), Some("/shop"));
        assert_eq!(theirs.domain.as_deref(), Some(".example.test"));
        assert_eq!(theirs.secure, Some(true));
        assert_eq!(theirs.http_only, Some(true));
        assert_eq!(theirs.expiry, Some(4_102_444_800));
        assert!(matches!(theirs.same_site, Some(WebDriverSameSite::Lax)));
    }

    #[test]
    fn an_empty_domain_is_left_out_and_the_expiry_is_cut_to_whole_seconds() {
        let theirs = Cookie::new("a", "b")
            .expires(1_700_000_000.9)
            .to_webdriver();
        assert_eq!(theirs.domain, None, "the browser uses the page's domain");
        assert_eq!(theirs.expiry, Some(1_700_000_000));
        assert!(theirs.same_site.is_none());

        let session = Cookie::new("a", "b").to_webdriver();
        assert_eq!(session.expiry, None, "a session cookie has no expiry");
        assert_eq!(session.path.as_deref(), Some("/"));
        assert_eq!(session.secure, Some(false));
        assert_eq!(session.http_only, Some(false));
    }

    #[test]
    fn a_cookie_survives_the_trip_to_the_webdriver_library_and_back() {
        let cookie = Cookie::new("sid", "abc")
            .path("/shop")
            .domain(".example.test")
            .secure(true)
            .http_only(true)
            .expires(4_102_444_800.0)
            .same_site(SameSite::None);
        assert_eq!(Cookie::from_webdriver(&cookie.to_webdriver()), cookie);
        let session = Cookie::new("a", "b");
        assert_eq!(Cookie::from_webdriver(&session.to_webdriver()), session);
    }

    #[test]
    fn a_cookie_survives_the_trip_through_the_protocol_and_back() {
        let cookie = Cookie::new("sid", "abc")
            .path("/shop")
            .domain(".example.test")
            .secure(true)
            .http_only(true)
            .expires(4_102_444_800.5)
            .same_site(SameSite::Lax);
        assert_eq!(Cookie::from_protocol(&cookie.to_protocol()), cookie);
    }

    #[test]
    fn a_w3c_cookie_object_deserializes_with_expiry_and_without_a_path() {
        let cookie: Cookie = serde_json::from_value(json!({
            "name": "sid", "value": "abc", "expiry": 4_102_444_800_i64, "httpOnly": true,
            "sameSite": "Lax"
        }))
        .unwrap();
        assert_eq!(
            cookie,
            Cookie::new("sid", "abc")
                .expires(4_102_444_800.0)
                .http_only(true)
                .same_site(SameSite::Lax)
        );
    }

    #[test]
    fn a_w3c_cookie_object_with_only_a_name_and_value_gets_the_defaults() {
        let cookie: Cookie = serde_json::from_value(json!({ "name": "a", "value": "b" })).unwrap();
        assert_eq!(cookie, Cookie::new("a", "b"));
        assert_eq!((cookie.path.as_str(), cookie.domain.as_str()), ("/", ""));
    }

    #[test]
    fn a_file_saved_by_python_reads_as_cookies() {
        // `json.dumps(driver.get_cookies())`, as `save_cookies` writes it.
        let saved = include_str!("../tests/fixtures/cookies/python_saved_cookies.txt");
        let cookies: Vec<Cookie> = serde_json::from_str(saved).unwrap();
        assert_eq!(
            cookies,
            [
                Cookie::new("sid", "abc")
                    .domain(".other.test")
                    .http_only(true)
                    .expires(4_102_444_800.0)
                    .same_site(SameSite::Lax),
                Cookie::new("theme", "dark")
                    .domain(".other.test")
                    .secure(true),
                Cookie::new("lang", "en")
                    .domain("localhost")
                    .path("/docs")
                    .secure(true)
                    .expires(1_893_456_000.0)
                    .same_site(SameSite::None),
            ]
        );
    }

    #[test]
    fn the_name_and_value_are_still_required() {
        assert!(serde_json::from_value::<Cookie>(json!({ "value": "b" })).is_err());
        assert!(serde_json::from_value::<Cookie>(json!({ "name": "a" })).is_err());
    }

    #[test]
    fn the_serialized_form_is_the_one_the_cdp_engine_has_always_saved() {
        let cookie = Cookie::new("a", "b").domain("example.test");
        assert_eq!(
            serde_json::to_value(&cookie).unwrap(),
            json!({
                "name": "a", "value": "b", "domain": "example.test", "path": "/",
                "expires": null, "httpOnly": false, "secure": false, "sameSite": null
            })
        );
        let saved: Cookie = serde_json::from_value(serde_json::to_value(&cookie).unwrap()).unwrap();
        assert_eq!(saved, cookie);
    }

    #[test]
    fn the_value_is_not_in_the_debug_text() {
        let text = format!("{:?}", Cookie::new("session", "s3cret"));
        assert!(text.contains("session"), "{text}");
        assert!(!text.contains("s3cret"), "{text}");
    }
}
