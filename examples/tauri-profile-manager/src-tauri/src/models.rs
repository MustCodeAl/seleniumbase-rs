use std::fmt;

use serde::{Deserialize, Serialize};

use seleniumbase_rs::profile_payloads::ProfileParams;
use seleniumbase_rs::{Browser, DriverMode};

/// Common API status wrapper.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiStatus {
    pub error_code: String,
    pub http_code: u16,
    pub message: String,
}

impl ApiStatus {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            error_code: "".into(),
            http_code: 200,
            message: message.into(),
        }
    }

    pub fn err(code: impl Into<String>, msg: impl Into<String>) -> Self {
        Self {
            error_code: code.into(),
            http_code: 400,
            message: msg.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub status: ApiStatus,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            data: Some(data),
            status: ApiStatus::ok(""),
        }
    }

    pub fn ok_msg(data: T, msg: impl Into<String>) -> Self {
        Self {
            data: Some(data),
            status: ApiStatus::ok(msg),
        }
    }

    pub fn err(status: ApiStatus) -> Self {
        Self { data: None, status }
    }
}

/// What drives the browser for a profile.
///
/// Profiles saved before this existed have no engine and keep using
/// WebDriver, so nothing about an existing profile changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Engine {
    /// A WebDriver server, normally a `selenium/standalone-chrome` container.
    #[default]
    WebDriver,
    /// Chrome launched directly and driven over the DevTools Protocol, with no
    /// Docker and no WebDriver. Each launch gets its own isolated browser
    /// context.
    #[serde(alias = "pure_cdp")]
    PureCdp,
}

impl Engine {
    /// Whether the engine needs the URL of a WebDriver server.
    pub fn needs_container(self) -> bool {
        self == Self::WebDriver
    }
}

/// A saved browser profile: one isolated browser identity.
///
/// `Debug` hides everything that can hold a secret (proxy credentials,
/// cookies and the external or fingerprint payloads), so a profile is safe to
/// log.
#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// WebDriver URL. Empty for a Pure CDP profile, which needs none.
    #[serde(default)]
    pub container_url: String,
    /// Which engine drives the browser. Absent in profiles saved by older
    /// versions, which are WebDriver profiles.
    #[serde(default)]
    pub engine: Engine,
    #[serde(default)]
    pub browser: Browser,
    #[serde(default)]
    pub mode: DriverMode,
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub locale: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub accuracy: Option<f64>,
    #[serde(default)]
    pub headless: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub folder_id: String,
    #[serde(default)]
    pub cookies: Vec<BrowserCookie>,
    /// Full external profile parameters, when supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_profile: Option<ProfileParams>,
    /// Custom masking / anti-fingerprint settings for this profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<seleniumbase_rs::Fingerprint>,
}

impl fmt::Debug for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Profile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("engine", &self.engine)
            .field("container_url", &self.container_url)
            .field("proxy", &self.proxy.as_ref().map(|_| "<redacted>"))
            .field("cookies", &self.cookies.len())
            .finish_non_exhaustive()
    }
}

impl Profile {
    /// Builds a profile from a creation payload.
    ///
    /// # Errors
    ///
    /// See [`validate`](Self::validate).
    pub fn from_new(id: String, new: NewProfile) -> Result<Self, String> {
        let profile = Self {
            id,
            name: new.name,
            container_url: new.container_url,
            engine: new.engine,
            browser: new.browser,
            mode: new.mode,
            user_agent: new.user_agent,
            proxy: new.proxy,
            locale: new.locale,
            latitude: new.latitude,
            longitude: new.longitude,
            accuracy: new.accuracy,
            headless: new.headless,
            tags: new.tags,
            folder_id: if new.folder_id.is_empty() {
                "default".into()
            } else {
                new.folder_id
            },
            cookies: vec![],
            external_profile: new.external_profile,
            fingerprint: new.fingerprint,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// Checks that the profile can be launched.
    ///
    /// # Errors
    ///
    /// Returns a message when the name is empty, or when a WebDriver profile
    /// has no WebDriver URL. A Pure CDP profile needs no URL.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("A profile needs a name".to_owned());
        }
        if self.engine.needs_container() && self.container_url.trim().is_empty() {
            return Err("A WebDriver profile needs a WebDriver URL".to_owned());
        }
        Ok(())
    }
}

/// Input payload for creating a profile.
#[derive(Clone, Debug, Deserialize)]
pub struct NewProfile {
    pub name: String,
    /// WebDriver URL; may be left out for a Pure CDP profile.
    #[serde(default)]
    pub container_url: String,
    #[serde(default)]
    pub engine: Engine,
    #[serde(default)]
    pub browser: Browser,
    #[serde(default)]
    pub mode: DriverMode,
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub locale: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub accuracy: Option<f64>,
    #[serde(default)]
    pub headless: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub folder_id: String,
    /// Raw external profile parameters (flags, fingerprints, storage, proxy, ...).
    #[serde(default, rename = "parameters")]
    pub external_profile: Option<ProfileParams>,
    /// Custom masking / anti-fingerprint settings (partial payloads allowed).
    #[serde(default)]
    pub fingerprint: Option<seleniumbase_rs::Fingerprint>,
}

/// Information returned after launching a profile.
#[derive(Clone, Debug, Serialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub profile_id: String,
    pub profile_name: String,
    pub container_url: String,
    pub engine: Engine,
}

/// A cookie as stored in a profile.
///
/// The value is a credential, so `Debug` hides it.
#[derive(Clone, Serialize, Deserialize)]
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<f64>,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub same_site: String,
}

impl fmt::Debug for BrowserCookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserCookie")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .field("domain", &self.domain)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// Whether the profile store is usable, for the app window to show.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct StorageStatus {
    /// `true` when profiles are loaded and changes are saved.
    pub ok: bool,
    /// Why the store could not be opened. Never contains a passphrase.
    pub error: Option<String>,
    /// Something worth knowing even though the store works.
    pub warning: Option<String>,
    /// Where the passphrase came from: `environment` or `keychain`.
    pub passphrase_source: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProxyValidateRequest {
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProxyValidateData {
    pub ip: String,
    pub country_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub timezone: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct CookieImportRequest {
    pub profile_id: String,
    #[serde(default)]
    pub folder_id: String,
    #[serde(default)]
    pub import_advanced_cookies: bool,
    #[serde(default)]
    pub cookies: Vec<BrowserCookie>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CookieExportRequest {
    pub profile_id: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(dead_code)]
pub struct StartProfileRequest {
    pub profile_id: String,
    #[serde(default)]
    pub automation: String,
    #[serde(default)]
    pub prefs: serde_json::Value,
}

#[derive(Clone, Debug, Serialize)]
pub struct StartProfileData {
    pub profile_id: String,
    pub session_id: String,
    pub port: u16,
    pub ws_endpoint: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RunScriptRequest {
    pub profile_ids: Vec<String>,
    pub script: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE_PROXY_PASSWORD: &str = "fake-proxy-password";
    const FAKE_COOKIE: &str = "fake-cookie-value-123";

    fn profile_with_secrets() -> Profile {
        serde_json::from_value(serde_json::json!({
            "id": "p1",
            "name": "Shop",
            "container_url": "http://localhost:4444",
            "user_agent": null,
            "proxy": format!("http://alice:{FAKE_PROXY_PASSWORD}@proxy.example:8080"),
            "locale": null,
            "latitude": null,
            "longitude": null,
            "accuracy": null,
            "cookies": [{
                "name": "session",
                "value": FAKE_COOKIE,
                "domain": ".example.com",
                "path": "/",
            }],
        }))
        .unwrap()
    }

    #[test]
    fn debug_output_hides_proxy_passwords_and_cookie_values() {
        let profile = profile_with_secrets();

        let shown = format!("{profile:?} {:?}", profile.cookies);

        assert!(!shown.contains(FAKE_PROXY_PASSWORD), "{shown}");
        assert!(!shown.contains(FAKE_COOKIE), "{shown}");
        assert!(shown.contains("Shop"), "the name is still shown: {shown}");
    }

    #[test]
    fn a_profile_saved_before_engines_existed_is_a_webdriver_profile() {
        let profile = profile_with_secrets();

        assert_eq!(profile.engine, Engine::WebDriver);
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn the_engine_accepts_both_spellings_and_round_trips() {
        for spelling in ["PureCdp", "pure_cdp"] {
            let engine: Engine = serde_json::from_value(serde_json::json!(spelling)).unwrap();
            assert_eq!(engine, Engine::PureCdp, "{spelling}");
        }
        let json = serde_json::to_value(Engine::PureCdp).unwrap();
        assert_eq!(
            serde_json::from_value::<Engine>(json).unwrap(),
            Engine::PureCdp
        );
        assert!(serde_json::from_value::<Engine>(serde_json::json!("Telnet")).is_err());
    }

    #[test]
    fn a_pure_cdp_profile_needs_no_webdriver_url() {
        let new: NewProfile = serde_json::from_value(serde_json::json!({
            "name": "Direct",
            "engine": "PureCdp",
        }))
        .unwrap();

        let profile = Profile::from_new("p".to_owned(), new).unwrap();

        assert_eq!(profile.engine, Engine::PureCdp);
        assert!(profile.container_url.is_empty());
    }

    #[test]
    fn a_webdriver_profile_without_a_url_is_refused() {
        let new: NewProfile = serde_json::from_value(serde_json::json!({
            "name": "Needs a server",
        }))
        .unwrap();

        let error = Profile::from_new("p".to_owned(), new).unwrap_err();

        assert!(error.contains("WebDriver URL"), "{error}");
    }

    #[test]
    fn a_profile_needs_a_name() {
        let new: NewProfile = serde_json::from_value(serde_json::json!({
            "name": "   ",
            "container_url": "http://localhost:4444",
        }))
        .unwrap();

        assert!(Profile::from_new("p".to_owned(), new).is_err());
    }

    #[test]
    fn a_new_profile_lands_in_the_default_folder_with_no_cookies() {
        let new: NewProfile = serde_json::from_value(serde_json::json!({
            "name": "Shop",
            "container_url": "http://localhost:4444",
        }))
        .unwrap();

        let profile = Profile::from_new("p".to_owned(), new).unwrap();

        assert_eq!(profile.folder_id, "default");
        assert!(profile.cookies.is_empty());
    }
}
