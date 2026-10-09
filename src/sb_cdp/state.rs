//! Browser state attached to a tab: cookies, web storage, the window, and
//! environment emulation.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::page::Page;
use crate::error::SeleniumBaseError;

// ----------------------------------------------------------------------
// Cookies
// ----------------------------------------------------------------------

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

/// A browser cookie.
///
/// The value is a credential, so [`Debug`](fmt::Debug) hides it.
///
/// # Examples
///
/// ```
/// use seleniumbase_rs::sb_cdp::Cookie;
///
/// let cookie = Cookie::new("session", "s3cret").domain("example.com").secure(true);
/// assert!(!format!("{cookie:?}").contains("s3cret"));
/// ```
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cookie {
    /// The cookie name.
    pub name: String,
    /// The cookie value.
    pub value: String,
    /// The domain it applies to.
    pub domain: String,
    /// The path it applies to.
    pub path: String,
    /// Expiry as seconds since the Unix epoch; `None` for a session cookie.
    pub expires: Option<f64>,
    /// Hidden from page scripts.
    pub http_only: bool,
    /// Sent only over HTTPS.
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

    fn from_protocol(value: &Value) -> Self {
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

    fn to_protocol(&self) -> Value {
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

    /// Whether `pattern` matches the domain, name or value.
    fn matches(&self, pattern: &Regex) -> bool {
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

/// The cookie jar of one tab's browser profile.
#[derive(Debug, Clone)]
pub struct Cookies {
    page: Page,
}

impl Cookies {
    pub(crate) fn new(page: Page) -> Self {
        Self { page }
    }

    /// Every cookie in the profile.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser cannot be queried.
    pub async fn all(&self) -> Result<Vec<Cookie>, SeleniumBaseError> {
        let response = self
            .page
            .execute("Network.getAllCookies", json!({}))
            .await?;
        Ok(response["cookies"]
            .as_array()
            .map(|cookies| cookies.iter().map(Cookie::from_protocol).collect())
            .unwrap_or_default())
    }

    /// Adds or replaces cookies.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects them.
    pub async fn set(&self, cookies: &[Cookie]) -> Result<(), SeleniumBaseError> {
        let cookies: Vec<_> = cookies.iter().map(Cookie::to_protocol).collect();
        self.page
            .execute("Network.setCookies", json!({ "cookies": cookies }))
            .await?;
        Ok(())
    }

    /// Deletes every cookie in the profile.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] on a protocol error.
    pub async fn clear(&self) -> Result<(), SeleniumBaseError> {
        self.page
            .execute("Network.clearBrowserCookies", json!({}))
            .await?;
        Ok(())
    }

    /// The cookies visible to the page, as a `Cookie:` header value.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page cannot be queried.
    pub async fn header(&self) -> Result<String, SeleniumBaseError> {
        Ok(self
            .page
            .evaluate("document.cookie")
            .await?
            .as_str()
            .unwrap_or_default()
            .to_owned())
    }

    /// Writes cookies to `path` as JSON, keeping those matching `filter`.
    ///
    /// `filter` is a regular expression tested against each cookie's domain,
    /// name and value; `None` keeps all of them. Returns how many were saved.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] for a bad `filter`, and
    /// [`SeleniumBaseError::Io`] if the file cannot be written.
    pub async fn save(
        &self,
        path: impl AsRef<Path>,
        filter: Option<&str>,
    ) -> Result<usize, SeleniumBaseError> {
        let kept = filtered(self.all().await?, filter)?;
        std::fs::write(path, serde_json::to_vec_pretty(&kept)?)?;
        Ok(kept.len())
    }

    /// Reads cookies from a file written by [`save`](Self::save) and adds them.
    ///
    /// Returns how many were loaded.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if the file cannot be read, and
    /// [`SeleniumBaseError::Json`] if it is not a cookie file.
    pub async fn load(
        &self,
        path: impl AsRef<Path>,
        filter: Option<&str>,
    ) -> Result<usize, SeleniumBaseError> {
        let stored: Vec<Cookie> = serde_json::from_slice(&std::fs::read(path)?)?;
        let kept = filtered(stored, filter)?;
        self.set(&kept).await?;
        Ok(kept.len())
    }
}

fn filtered(cookies: Vec<Cookie>, filter: Option<&str>) -> Result<Vec<Cookie>, SeleniumBaseError> {
    let Some(pattern) = filter else {
        return Ok(cookies);
    };
    let pattern = Regex::new(pattern)
        .map_err(|e| SeleniumBaseError::invalid_config(format!("bad cookie filter: {e}")))?;
    Ok(cookies
        .into_iter()
        .filter(|cookie| cookie.matches(&pattern))
        .collect())
}

// ----------------------------------------------------------------------
// Web storage
// ----------------------------------------------------------------------

/// Which web storage area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StorageArea {
    /// `localStorage`, which persists across sessions.
    Local,
    /// `sessionStorage`, which lasts as long as the tab.
    Session,
}

/// `localStorage` or `sessionStorage` of the page's origin.
#[derive(Debug, Clone)]
pub struct Storage {
    page: Page,
    area: StorageArea,
}

impl Storage {
    pub(crate) fn new(page: Page, area: StorageArea) -> Self {
        Self { page, area }
    }

    fn object(&self) -> &'static str {
        match self.area {
            StorageArea::Local => "localStorage",
            StorageArea::Session => "sessionStorage",
        }
    }

    /// The value stored under `key`, if any.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page denies access.
    pub async fn get(&self, key: impl AsRef<str>) -> Result<Option<String>, SeleniumBaseError> {
        let key = serde_json::to_string(key.as_ref())?;
        let value = self
            .page
            .evaluate(format!("{}.getItem({key})", self.object()))
            .await?;
        Ok(value.as_str().map(str::to_owned))
    }

    /// Stores `value` under `key`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page denies access.
    pub async fn set(
        &self,
        key: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<(), SeleniumBaseError> {
        let key = serde_json::to_string(key.as_ref())?;
        let value = serde_json::to_string(value.as_ref())?;
        self.page
            .evaluate(format!("{}.setItem({key}, {value})", self.object()))
            .await?;
        Ok(())
    }

    /// Removes `key`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page denies access.
    pub async fn remove(&self, key: impl AsRef<str>) -> Result<(), SeleniumBaseError> {
        let key = serde_json::to_string(key.as_ref())?;
        self.page
            .evaluate(format!("{}.removeItem({key})", self.object()))
            .await?;
        Ok(())
    }

    /// Removes everything.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page denies access.
    pub async fn clear(&self) -> Result<(), SeleniumBaseError> {
        self.page
            .evaluate(format!("{}.clear()", self.object()))
            .await?;
        Ok(())
    }

    /// Every key and value.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the page denies access.
    pub async fn entries(&self) -> Result<BTreeMap<String, String>, SeleniumBaseError> {
        self.page
            .evaluate_as(format!(
                "Object.fromEntries(Object.entries({}))",
                self.object()
            ))
            .await
    }
}

// ----------------------------------------------------------------------
// Window
// ----------------------------------------------------------------------

/// How a window is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum WindowState {
    /// A regular window with its own size and position.
    Normal,
    /// Minimised to the dock or taskbar.
    Minimized,
    /// Filling the screen's work area.
    Maximized,
    /// Fullscreen.
    Fullscreen,
}

/// Position, size and state of a browser window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowBounds {
    /// Distance from the left edge of the screen.
    pub x: i64,
    /// Distance from the top edge of the screen.
    pub y: i64,
    /// Width in pixels.
    pub width: i64,
    /// Height in pixels.
    pub height: i64,
    /// How the window is shown.
    pub state: WindowState,
}

/// The browser window that holds a tab.
#[derive(Debug, Clone)]
pub struct Window {
    page: Page,
}

impl Window {
    pub(crate) fn new(page: Page) -> Self {
        Self { page }
    }

    async fn locate(&self) -> Result<(i64, WindowBounds), SeleniumBaseError> {
        let response = self
            .page
            .browser()
            .execute(
                "Browser.getWindowForTarget",
                json!({ "targetId": self.page.id() }),
            )
            .await?;
        let bounds = &response["bounds"];
        let state = match bounds["windowState"].as_str() {
            Some("minimized") => WindowState::Minimized,
            Some("maximized") => WindowState::Maximized,
            Some("fullscreen") => WindowState::Fullscreen,
            _ => WindowState::Normal,
        };
        Ok((
            response["windowId"].as_i64().unwrap_or_default(),
            WindowBounds {
                x: bounds["left"].as_i64().unwrap_or_default(),
                y: bounds["top"].as_i64().unwrap_or_default(),
                width: bounds["width"].as_i64().unwrap_or_default(),
                height: bounds["height"].as_i64().unwrap_or_default(),
                state,
            },
        ))
    }

    /// The window's current position, size and state.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser cannot be queried.
    pub async fn bounds(&self) -> Result<WindowBounds, SeleniumBaseError> {
        Ok(self.locate().await?.1)
    }

    async fn apply(&self, bounds: Value) -> Result<(), SeleniumBaseError> {
        let (id, _) = self.locate().await?;
        self.page
            .browser()
            .execute(
                "Browser.setWindowBounds",
                json!({ "windowId": id, "bounds": bounds }),
            )
            .await?;
        Ok(())
    }

    /// Moves and resizes the window, restoring it first if it is not normal.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn set_bounds(
        &self,
        x: i64,
        y: i64,
        width: i64,
        height: i64,
    ) -> Result<(), SeleniumBaseError> {
        // Chrome ignores size changes while a window is minimised or maximised.
        self.apply(json!({ "windowState": "normal" })).await?;
        self.apply(json!({ "left": x, "top": y, "width": width, "height": height }))
            .await
    }

    /// Maximises the window.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn maximize(&self) -> Result<(), SeleniumBaseError> {
        self.apply(json!({ "windowState": "maximized" })).await
    }

    /// Minimises the window.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn minimize(&self) -> Result<(), SeleniumBaseError> {
        self.apply(json!({ "windowState": "minimized" })).await
    }

    /// Makes the window fullscreen.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn fullscreen(&self) -> Result<(), SeleniumBaseError> {
        self.apply(json!({ "windowState": "fullscreen" })).await
    }

    /// Returns the window to its normal state.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn restore(&self) -> Result<(), SeleniumBaseError> {
        self.apply(json!({ "windowState": "normal" })).await
    }
}

// ----------------------------------------------------------------------
// Emulation
// ----------------------------------------------------------------------

/// Overrides for what the tab reports about its environment.
#[derive(Debug, Clone)]
pub struct Emulation {
    page: Page,
}

impl Emulation {
    pub(crate) fn new(page: Page) -> Self {
        Self { page }
    }

    /// Reports the given IANA timezone, such as `Europe/Berlin`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] for an unknown timezone.
    pub async fn timezone(&self, id: &str) -> Result<(), SeleniumBaseError> {
        self.page
            .execute("Emulation.setTimezoneOverride", json!({ "timezoneId": id }))
            .await?;
        Ok(())
    }

    /// Reports the given locale, such as `de-DE`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] for an unknown locale.
    pub async fn locale(&self, locale: &str) -> Result<(), SeleniumBaseError> {
        self.page
            .execute("Emulation.setLocaleOverride", json!({ "locale": locale }))
            .await?;
        Ok(())
    }

    /// Reports a location, in degrees, with `accuracy` in metres.
    ///
    /// The page must also have the geolocation permission, see
    /// [`Browser::grant_permissions`](super::Browser::grant_permissions).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn geolocation(
        &self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> Result<(), SeleniumBaseError> {
        self.page
            .execute(
                "Emulation.setGeolocationOverride",
                json!({ "latitude": latitude, "longitude": longitude, "accuracy": accuracy }),
            )
            .await?;
        Ok(())
    }

    /// Reports a `User-Agent` string, optionally with a platform.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn user_agent(
        &self,
        user_agent: &str,
        platform: Option<&str>,
    ) -> Result<(), SeleniumBaseError> {
        let mut params = json!({ "userAgent": user_agent });
        if let Some(platform) = platform {
            params["platform"] = json!(platform);
        }
        self.page
            .execute("Emulation.setUserAgentOverride", params)
            .await?;
        Ok(())
    }

    /// Simulates losing (`true`) or regaining (`false`) the network.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn offline(&self, offline: bool) -> Result<(), SeleniumBaseError> {
        self.page.execute("Network.enable", json!({})).await?;
        self.page
            .execute(
                "Network.emulateNetworkConditions",
                json!({
                    "offline": offline,
                    "latency": 0,
                    "downloadThroughput": -1,
                    "uploadThroughput": -1,
                }),
            )
            .await?;
        Ok(())
    }
}

impl Page {
    /// The cookie jar.
    #[must_use]
    pub fn cookies(&self) -> Cookies {
        Cookies::new(self.clone())
    }

    /// `localStorage` for the page's origin.
    #[must_use]
    pub fn local_storage(&self) -> Storage {
        Storage::new(self.clone(), StorageArea::Local)
    }

    /// `sessionStorage` for the page's origin.
    #[must_use]
    pub fn session_storage(&self) -> Storage {
        Storage::new(self.clone(), StorageArea::Session)
    }

    /// The window that holds this tab.
    #[must_use]
    pub fn window(&self) -> Window {
        Window::new(self.clone())
    }

    /// Overrides of the environment the tab reports to pages.
    #[must_use]
    pub fn emulation(&self) -> Emulation {
        Emulation::new(self.clone())
    }
}
