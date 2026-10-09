//! A running browser session, whichever engine drives it.
//!
//! A [`Engine::WebDriver`] profile is driven through [`BaseCase`] against a
//! Selenium server. A [`Engine::PureCdp`] profile is a Chrome the app launches
//! itself and drives over the DevTools Protocol: no Docker and no WebDriver.
//! Each Pure CDP launch gets a new browser and, inside it, its own isolated
//! browser context, so its cookies and storage are shared with nothing else.
//!
//! [`Session`] is the one type the commands and the REST API hold, so they do
//! not care which engine is behind a session.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use seleniumbase_rs::sb_cdp::{
    Browser as CdpBrowser, BrowserContext, ContextOptions, Cookie, LaunchOptions, Page, Proxy,
    SameSite,
};
use seleniumbase_rs::stealth::evasions::{bootstrap_script, cdp_overrides};
use seleniumbase_rs::{BaseCase, BrowserConfig, Fingerprint};

use crate::models::{BrowserCookie, Engine, Profile};
use crate::store::build_config;

/// Where screenshots go, relative to the working directory.
const LOGS_DIR: &str = "latest_logs";

/// The step of starting a session that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Starting the browser, or reaching the WebDriver server.
    Launch,
    /// Applying the profile's identity: fingerprint, location, screen.
    Overrides,
    /// Setting the profile's saved cookies.
    Cookies,
    /// Opening the start page.
    Open,
}

impl Stage {
    /// The error code the REST API reports for this stage.
    pub fn code(self) -> &'static str {
        match self {
            Self::Launch => "LAUNCH_FAILED",
            Self::Overrides => "OVERRIDE_FAILED",
            Self::Cookies => "COOKIE_FAILED",
            Self::Open => "OPEN_FAILED",
        }
    }
}

/// A session that could not be started, and where it went wrong.
#[derive(Debug)]
pub struct LaunchError {
    pub stage: Stage,
    pub message: String,
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LaunchError {}

fn at(stage: Stage, message: impl Into<String>) -> LaunchError {
    LaunchError {
        stage,
        message: message.into(),
    }
}

/// A running browser session.
pub enum Session {
    /// Driven through WebDriver.
    WebDriver(Box<BaseCase>),
    /// Driven directly over the DevTools Protocol.
    PureCdp(PureCdpSession),
}

impl Session {
    /// Starts a session for `profile` with its identity applied: fingerprint,
    /// location, screen size and saved cookies. Opens `start_url` last.
    ///
    /// A session that fails part-way is shut down before the error is
    /// returned, so a failed launch leaves no browser behind.
    pub async fn launch(profile: &Profile, start_url: Option<&str>) -> Result<Self, LaunchError> {
        match profile.engine {
            Engine::WebDriver => launch_webdriver(profile, start_url).await,
            Engine::PureCdp => PureCdpSession::launch(profile, start_url)
                .await
                .map(Self::PureCdp),
        }
    }

    /// The port and WebSocket address an automation client can connect to,
    /// for a session that has its own DevTools endpoint.
    pub fn debug_endpoint(&self) -> Option<(u16, String)> {
        match self {
            Self::WebDriver(_) => None,
            Self::PureCdp(session) => Some(session.debug_endpoint()),
        }
    }

    /// Navigates to `url`.
    pub async fn open(&mut self, url: &str) -> Result<(), String> {
        match self {
            Self::WebDriver(sb) => sb.open(url).await.map_err(|e| e.to_string()),
            Self::PureCdp(session) => session.open(url).await,
        }
    }

    /// Saves a screenshot under `latest_logs` and returns its path.
    pub async fn screenshot(&mut self) -> Result<PathBuf, String> {
        match self {
            Self::WebDriver(sb) => sb
                .save_screenshot_to_logs()
                .await
                .map_err(|e| e.to_string()),
            Self::PureCdp(session) => session.screenshot_into(Path::new(LOGS_DIR)).await,
        }
    }

    /// Reports a location to pages.
    pub async fn set_geolocation(
        &mut self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> Result<(), String> {
        match self {
            Self::WebDriver(sb) => set_webdriver_geolocation(sb, latitude, longitude, accuracy)
                .await
                .map_err(|e| e.to_string()),
            Self::PureCdp(session) => session.set_geolocation(latitude, longitude, accuracy).await,
        }
    }

    /// Sets cookies in the session's browser.
    pub async fn set_cookies(&mut self, cookies: &[BrowserCookie]) -> Result<(), String> {
        match self {
            Self::WebDriver(sb) => set_webdriver_cookies(sb, cookies).await,
            Self::PureCdp(session) => session.set_cookies(cookies).await,
        }
    }

    /// Runs a script in the current page and returns its result as text.
    ///
    /// A WebDriver session runs `script` as the body of a function, so it
    /// needs `return`. A Pure CDP session evaluates it as an expression.
    pub async fn execute_script(&mut self, script: &str) -> Result<String, String> {
        match self {
            Self::WebDriver(sb) => sb
                .execute_script(script)
                .await
                .map(|value| value.to_string())
                .map_err(|e| e.to_string()),
            Self::PureCdp(session) => session.execute_script(script).await,
        }
    }

    /// Ends the session and closes its browser.
    pub async fn quit(&mut self) -> Result<(), String> {
        match self {
            Self::WebDriver(sb) => sb.quit().await.map_err(|e| e.to_string()),
            Self::PureCdp(session) => session.quit().await,
        }
    }
}

// ----------------------------------------------------------------------
// WebDriver
// ----------------------------------------------------------------------

async fn launch_webdriver(
    profile: &Profile,
    start_url: Option<&str>,
) -> Result<Session, LaunchError> {
    let mut sb = BaseCase::new(build_config(profile))
        .await
        .map_err(|e| at(Stage::Launch, e.to_string()))?;
    match prepare_webdriver(&mut sb, profile, start_url).await {
        Ok(()) => Ok(Session::WebDriver(Box::new(sb))),
        Err(error) => {
            // The session exists on the server by now; do not leave it there.
            let _ = sb.quit().await;
            Err(error)
        }
    }
}

async fn prepare_webdriver(
    sb: &mut BaseCase,
    profile: &Profile,
    start_url: Option<&str>,
) -> Result<(), LaunchError> {
    apply_webdriver_overrides(sb, profile)
        .await
        .map_err(|message| at(Stage::Overrides, message))?;
    if !profile.cookies.is_empty() {
        set_webdriver_cookies(sb, &profile.cookies)
            .await
            .map_err(|message| at(Stage::Cookies, message))?;
    }
    if let Some(url) = start_url {
        sb.open(url)
            .await
            .map_err(|e| at(Stage::Open, e.to_string()))?;
    }
    Ok(())
}

async fn apply_webdriver_overrides(sb: &mut BaseCase, profile: &Profile) -> Result<(), String> {
    if let Some((latitude, longitude, accuracy)) = geolocation(profile) {
        set_webdriver_geolocation(sb, latitude, longitude, accuracy)
            .await
            .map_err(|e| format!("Failed to set geolocation: {e}"))?;
    }

    if let Some(screen) = profile
        .external_profile
        .as_ref()
        .and_then(|p| p.parameters.fingerprint.screen.as_ref())
    {
        sb.set_window_size(screen.width, screen.height)
            .await
            .map_err(|e| format!("Failed to set screen size: {e}"))?;
    }

    Ok(())
}

async fn set_webdriver_geolocation(
    sb: &mut BaseCase,
    latitude: f64,
    longitude: f64,
    accuracy: f64,
) -> seleniumbase_rs::Result<()> {
    let params = json!({
        "latitude": latitude,
        "longitude": longitude,
        "accuracy": accuracy,
    });
    sb.execute_cdp_with_params("Emulation.setGeolocationOverride", params)
        .await
        .map(|_| ())
}

async fn set_webdriver_cookies(sb: &mut BaseCase, cookies: &[BrowserCookie]) -> Result<(), String> {
    let cdp_cookies: Vec<Value> = cookies.iter().map(cookie_params).collect();
    sb.execute_cdp_with_params("Network.setCookies", json!({ "cookies": cdp_cookies }))
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// A cookie as `Network.setCookies` takes it. Absent values are left out
/// rather than sent empty, which the protocol rejects.
fn cookie_params(cookie: &BrowserCookie) -> Value {
    let mut params = json!({
        "name": cookie.name,
        "value": cookie.value,
        "domain": cookie.domain,
        "path": cookie.path,
        "secure": cookie.secure,
        "httpOnly": cookie.http_only,
    });
    if !cookie.same_site.is_empty() {
        params["sameSite"] = json!(cookie.same_site);
    }
    if let Some(expires) = cookie.expires {
        params["expires"] = json!(expires);
    }
    params
}

/// The latitude, longitude and accuracy a profile asks to report: the
/// external profile's geolocation when it has one, else the flat fields.
fn geolocation(profile: &Profile) -> Option<(f64, f64, f64)> {
    let external = profile
        .external_profile
        .as_ref()
        .and_then(|p| p.parameters.fingerprint.geolocation.as_ref())
        .map(|g| (g.latitude, g.longitude, g.accuracy));
    external.or_else(|| {
        let (latitude, longitude) = profile.latitude.zip(profile.longitude)?;
        Some((latitude, longitude, profile.accuracy.unwrap_or(100.0)))
    })
}

// ----------------------------------------------------------------------
// Pure CDP
// ----------------------------------------------------------------------

/// A Chrome the app launched and drives over the DevTools Protocol.
///
/// The tab lives in its own browser context, so the profile's cookies and
/// storage are separate from everything else in the browser. Quitting disposes
/// the context and closes the browser.
#[derive(Debug)]
pub struct PureCdpSession {
    browser: CdpBrowser,
    context: BrowserContext,
    page: Page,
}

impl PureCdpSession {
    async fn launch(profile: &Profile, start_url: Option<&str>) -> Result<Self, LaunchError> {
        let config = build_config(profile);
        let options = launch_options(&config).map_err(|message| at(Stage::Launch, message))?;
        let browser = CdpBrowser::launch(options)
            .await
            .map_err(|e| at(Stage::Launch, e.to_string()))?;
        match Self::start(browser.clone(), profile, &config, start_url).await {
            Ok(session) => Ok(session),
            Err(error) => {
                let _ = browser.close().await;
                Err(error)
            }
        }
    }

    /// Everything after the browser is up: the context, the tab and the
    /// profile's identity. Takes the browser so a test can hand it a mock.
    ///
    /// A failure disposes the context again; closing `browser` is the
    /// caller's job.
    async fn start(
        browser: CdpBrowser,
        profile: &Profile,
        config: &BrowserConfig,
        start_url: Option<&str>,
    ) -> Result<Self, LaunchError> {
        let options = context_options(config.proxy.as_deref())
            .map_err(|message| at(Stage::Launch, message))?;
        let context = browser
            .new_context_with(options)
            .await
            .map_err(|e| at(Stage::Launch, e.to_string()))?;
        match prepare(&browser, &context, profile, config, start_url).await {
            Ok(page) => Ok(Self {
                browser,
                context,
                page,
            }),
            Err(error) => {
                let _ = context.dispose().await;
                Err(error)
            }
        }
    }

    fn debug_endpoint(&self) -> (u16, String) {
        (
            self.browser.debugging_port(),
            self.browser.websocket_url().to_owned(),
        )
    }

    async fn open(&self, url: &str) -> Result<(), String> {
        self.page.goto(url).await.map_err(|e| e.to_string())
    }

    async fn screenshot_into(&self, dir: &Path) -> Result<PathBuf, String> {
        let png = self.page.screenshot().await.map_err(|e| e.to_string())?;
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_millis());
        let path = dir.join(format!("screenshot_{millis}.png"));
        tokio::fs::write(&path, png)
            .await
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        Ok(path)
    }

    async fn set_geolocation(
        &self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> Result<(), String> {
        // The profile may not have had a location when it launched.
        grant_geolocation(&self.browser, &self.context)
            .await
            .map_err(|e| e.to_string())?;
        self.page
            .emulation()
            .geolocation(latitude, longitude, accuracy)
            .await
            .map_err(|e| e.to_string())
    }

    async fn set_cookies(&self, cookies: &[BrowserCookie]) -> Result<(), String> {
        set_page_cookies(&self.page, cookies).await
    }

    async fn execute_script(&self, script: &str) -> Result<String, String> {
        self.page
            .evaluate(script)
            .await
            .map(|value| value.to_string())
            .map_err(|e| e.to_string())
    }

    async fn quit(&self) -> Result<(), String> {
        let disposed = self.context.dispose().await;
        let closed = self.browser.close().await;
        disposed.and(closed).map_err(|e| e.to_string())
    }
}

/// How to start Chrome for a profile.
///
/// The proxy is not set here. It goes to the profile's browser context
/// instead, so its password is answered for that context's tabs only.
fn launch_options(config: &BrowserConfig) -> Result<LaunchOptions, String> {
    let mut builder = LaunchOptions::builder().headless(config.headless);
    if let Some(user_agent) = &config.user_agent {
        builder = builder.user_agent(user_agent);
    }
    if let Some(locale) = &config.locale {
        builder = builder.lang(locale);
    }
    if let Some((width, height)) = config
        .fingerprint
        .as_ref()
        .and_then(|fp| fp.screen_width.zip(fp.screen_height))
    {
        builder = builder.window_size(width, height);
    }
    builder.build().map_err(|e| e.to_string())
}

/// Options for the profile's browser context: its proxy, if it has one.
fn context_options(proxy: Option<&str>) -> Result<ContextOptions, String> {
    let options = ContextOptions::new();
    match proxy.map(str::trim).filter(|spec| !spec.is_empty()) {
        Some(spec) => Ok(options.proxy(Proxy::parse(spec).map_err(|e| e.to_string())?)),
        None => Ok(options),
    }
}

/// Opens the tab and gives it the profile's identity, cookies and start page.
async fn prepare(
    browser: &CdpBrowser,
    context: &BrowserContext,
    profile: &Profile,
    config: &BrowserConfig,
    start_url: Option<&str>,
) -> Result<Page, LaunchError> {
    let page = context
        .new_page(None::<&str>)
        .await
        .map_err(|e| at(Stage::Launch, e.to_string()))?;
    apply_identity(browser, context, &page, profile, config)
        .await
        .map_err(|message| at(Stage::Overrides, message))?;
    if !profile.cookies.is_empty() {
        set_page_cookies(&page, &profile.cookies)
            .await
            .map_err(|message| at(Stage::Cookies, message))?;
    }
    if let Some(url) = start_url.or(config.start_page.as_deref()) {
        page.goto(url)
            .await
            .map_err(|e| at(Stage::Open, e.to_string()))?;
    }
    Ok(page)
}

/// Applies the fingerprint first and the profile's own settings after it, so
/// a profile's location and screen win over the fingerprint's.
async fn apply_identity(
    browser: &CdpBrowser,
    context: &BrowserContext,
    page: &Page,
    profile: &Profile,
    config: &BrowserConfig,
) -> Result<(), String> {
    let fingerprint = config.fingerprint.as_ref();
    if let Some(fingerprint) = fingerprint {
        apply_fingerprint(browser, context, page, fingerprint).await?;
    }

    // A second locale override on the same tab can be refused, so only set one
    // when the fingerprint did not.
    if let Some(locale) = config
        .locale
        .as_deref()
        .filter(|_| fingerprint.is_none_or(|fp| fp.locale.is_none()))
    {
        page.emulation()
            .locale(locale)
            .await
            .map_err(|e| format!("Failed to set the locale: {e}"))?;
        // The locale override changes `Intl`, but `navigator.language` and the
        // `Accept-Language` header follow the user agent override, so a page
        // would otherwise see two different languages.
        let user_agent = page
            .user_agent()
            .await
            .map_err(|e| format!("Failed to read the user agent: {e}"))?;
        page.execute(
            "Emulation.setUserAgentOverride",
            json!({
                // A headless Chrome names itself in its user agent.
                "userAgent": user_agent.replace("HeadlessChrome", "Chrome"),
                "acceptLanguage": accept_language(locale),
            }),
        )
        .await
        .map_err(|e| format!("Failed to set the language: {e}"))?;
    }

    if let Some((latitude, longitude, accuracy)) = geolocation(profile) {
        grant_geolocation(browser, context)
            .await
            .map_err(|e| format!("Failed to allow geolocation: {e}"))?;
        page.emulation()
            .geolocation(latitude, longitude, accuracy)
            .await
            .map_err(|e| format!("Failed to set geolocation: {e}"))?;
    }

    if let Some(screen) = profile
        .external_profile
        .as_ref()
        .and_then(|p| p.parameters.fingerprint.screen.as_ref())
    {
        page.execute(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": screen.width,
                "height": screen.height,
                "deviceScaleFactor": screen.pixel_ratio,
                "mobile": false,
            }),
        )
        .await
        .map_err(|e| format!("Failed to set screen size: {e}"))?;
    }
    Ok(())
}

/// An `Accept-Language` value for a locale: `fr-FR` becomes `fr-FR,fr;q=0.9`,
/// the way a browser set to French sends it.
fn accept_language(locale: &str) -> String {
    match locale.split_once('-') {
        Some((language, _region)) if !language.is_empty() => format!("{locale},{language};q=0.9"),
        _ => locale.to_owned(),
    }
}

/// Installs the fingerprint's script, to run before any page script, and
/// applies the protocol overrides it recommends.
///
/// The extra `Proxy-Authorization` header the fingerprint can ask for is
/// skipped: sent to every site, it would hand the proxy's password to the
/// sites themselves. The profile's proxy is answered per context instead.
async fn apply_fingerprint(
    browser: &CdpBrowser,
    context: &BrowserContext,
    page: &Page,
    fingerprint: &Fingerprint,
) -> Result<(), String> {
    page.execute(
        "Page.addScriptToEvaluateOnNewDocument",
        json!({ "source": bootstrap_script(fingerprint) }),
    )
    .await
    .map_err(|e| format!("Failed to install the fingerprint script: {e}"))?;
    // Some overrides, such as the blocked URLs, only apply once the network
    // domain is enabled.
    page.execute("Network.enable", json!({}))
        .await
        .map_err(|e| format!("Failed to enable the network domain: {e}"))?;

    let mut overrides: Vec<(String, Value)> = cdp_overrides(fingerprint)
        .into_iter()
        .filter(|(method, _)| method != "Network.setExtraHTTPHeaders")
        .collect();
    overrides.sort_by(|a, b| a.0.cmp(&b.0));
    for (method, mut params) in overrides {
        let outcome = if method == "Browser.grantPermissions" {
            // Permissions are per context; without this they would go to the
            // browser's default context, which this profile does not use.
            params["browserContextId"] = json!(context.id());
            browser.execute(&method, params).await
        } else {
            page.execute(&method, params).await
        };
        outcome.map_err(|e| format!("Failed to apply {method}: {e}"))?;
    }
    Ok(())
}

/// Lets pages in the profile's context read the location it reports.
async fn grant_geolocation(
    browser: &CdpBrowser,
    context: &BrowserContext,
) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
    browser
        .execute(
            "Browser.grantPermissions",
            json!({ "permissions": ["geolocation"], "browserContextId": context.id() }),
        )
        .await
        .map(drop)
}

async fn set_page_cookies(page: &Page, cookies: &[BrowserCookie]) -> Result<(), String> {
    let cookies: Vec<Cookie> = cookies.iter().map(to_cdp_cookie).collect();
    page.cookies()
        .set(&cookies)
        .await
        .map_err(|e| e.to_string())
}

fn to_cdp_cookie(cookie: &BrowserCookie) -> Cookie {
    let mut converted = Cookie::new(&cookie.name, &cookie.value)
        .domain(&cookie.domain)
        .path(&cookie.path)
        .http_only(cookie.http_only)
        .secure(cookie.secure);
    if let Some(expires) = cookie.expires {
        converted = converted.expires(expires);
    }
    match cookie.same_site.to_ascii_lowercase().as_str() {
        "strict" => converted.same_site(SameSite::Strict),
        "lax" => converted.same_site(SameSite::Lax),
        "none" | "no_restriction" => converted.same_site(SameSite::None),
        // Unset, or a value the protocol has no word for.
        _ => converted,
    }
}

#[cfg(test)]
mod tests {
    use seleniumbase_rs::sb_cdp::MockCtrl;
    use seleniumbase_rs::{OsType, ProxyConfig};

    use super::*;

    const FAKE_PROXY_PASSWORD: &str = "fake-proxy-password";
    const FAKE_COOKIE: &str = "fake-cookie-value-123";

    fn pure_cdp_profile(extra: Value) -> Profile {
        let mut profile = json!({
            "id": "p1",
            "name": "Direct",
            "engine": "PureCdp",
            "user_agent": null,
            "proxy": null,
            "locale": null,
            "latitude": null,
            "longitude": null,
            "accuracy": null,
            "headless": true,
        });
        for (key, value) in extra.as_object().expect("an object") {
            profile[key] = value.clone();
        }
        serde_json::from_value(profile).expect("a valid profile")
    }

    fn mocked() -> (CdpBrowser, MockCtrl) {
        let (browser, mock) = CdpBrowser::new_mocked();
        mock.reply(
            "Target.createBrowserContext",
            json!({ "browserContextId": "CTX1" }),
        );
        (browser, mock)
    }

    async fn start(
        browser: &CdpBrowser,
        profile: &Profile,
        start_url: Option<&str>,
    ) -> Result<PureCdpSession, LaunchError> {
        let config = build_config(profile);
        PureCdpSession::start(browser.clone(), profile, &config, start_url).await
    }

    fn methods(mock: &MockCtrl) -> Vec<String> {
        mock.calls().into_iter().map(|call| call.method).collect()
    }

    fn position(mock: &MockCtrl, method: &str) -> usize {
        methods(mock)
            .iter()
            .position(|m| m == method)
            .unwrap_or_else(|| panic!("{method} was never sent"))
    }

    #[tokio::test]
    async fn a_session_gets_its_own_context_and_a_tab_inside_it() {
        let (browser, mock) = mocked();

        start(&browser, &pure_cdp_profile(json!({})), None)
            .await
            .unwrap();

        let contexts = mock.calls_to("Target.createBrowserContext");
        assert_eq!(contexts.len(), 1);
        assert!(contexts[0].params.get("proxyServer").is_none());
        let tabs = mock.calls_to("Target.createTarget");
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].params["browserContextId"], "CTX1");
    }

    #[tokio::test]
    async fn the_proxy_goes_to_the_context_and_its_password_goes_nowhere_else() {
        let (browser, mock) = mocked();
        let profile = pure_cdp_profile(json!({
            "proxy": format!("http://alice:{FAKE_PROXY_PASSWORD}@proxy.example:8080"),
        }));

        start(&browser, &profile, None).await.unwrap();

        let contexts = mock.calls_to("Target.createBrowserContext");
        assert_eq!(
            contexts[0].params["proxyServer"],
            "http://proxy.example:8080"
        );
        let everything = format!("{:?}", mock.calls());
        assert!(
            !everything.contains(FAKE_PROXY_PASSWORD),
            "the password must not be sent as a protocol parameter"
        );
    }

    #[tokio::test]
    async fn an_unusable_proxy_is_refused_before_anything_is_created() {
        let (browser, mock) = mocked();
        let profile = pure_cdp_profile(json!({ "proxy": format!("alice:{FAKE_PROXY_PASSWORD}@") }));

        let error = start(&browser, &profile, None).await.unwrap_err();

        assert_eq!(error.stage, Stage::Launch);
        assert!(!error.message.contains(FAKE_PROXY_PASSWORD), "{error}");
        assert!(mock.calls_to("Target.createBrowserContext").is_empty());
    }

    #[tokio::test]
    async fn a_blank_proxy_means_no_proxy() {
        let (browser, mock) = mocked();

        start(&browser, &pure_cdp_profile(json!({ "proxy": "   " })), None)
            .await
            .unwrap();

        let contexts = mock.calls_to("Target.createBrowserContext");
        assert!(contexts[0].params.get("proxyServer").is_none());
    }

    #[tokio::test]
    async fn the_location_is_allowed_in_the_context_and_reported() {
        let (browser, mock) = mocked();
        let profile = pure_cdp_profile(json!({ "latitude": 51.5, "longitude": -0.12 }));

        start(&browser, &profile, None).await.unwrap();

        let grants = mock.calls_to("Browser.grantPermissions");
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].params["browserContextId"], "CTX1");
        assert_eq!(grants[0].params["permissions"], json!(["geolocation"]));
        let location = mock.calls_to("Emulation.setGeolocationOverride");
        assert_eq!(location.len(), 1);
        assert_eq!(location[0].params["latitude"], 51.5);
        assert_eq!(location[0].params["longitude"], -0.12);
        assert_eq!(location[0].params["accuracy"], 100.0);
    }

    #[tokio::test]
    async fn no_location_means_no_geolocation_calls() {
        let (browser, mock) = mocked();

        start(&browser, &pure_cdp_profile(json!({})), None)
            .await
            .unwrap();

        assert!(mock.calls_to("Emulation.setGeolocationOverride").is_empty());
        assert!(mock.calls_to("Browser.grantPermissions").is_empty());
    }

    #[tokio::test]
    async fn the_locale_and_the_language_header_are_set_when_there_is_no_fingerprint() {
        let (browser, mock) = mocked();
        mock.reply(
            "Runtime.evaluate",
            json!({ "result": { "value": "Mozilla/5.0 HeadlessChrome/130.0.0.0" } }),
        );

        start(
            &browser,
            &pure_cdp_profile(json!({ "locale": "de-DE" })),
            None,
        )
        .await
        .unwrap();

        let locale = mock.calls_to("Emulation.setLocaleOverride");
        assert_eq!(locale.len(), 1);
        assert_eq!(locale[0].params["locale"], "de-DE");
        let language = mock.calls_to("Emulation.setUserAgentOverride");
        assert_eq!(language.len(), 1);
        assert_eq!(language[0].params["acceptLanguage"], "de-DE,de;q=0.9");
        assert_eq!(
            language[0].params["userAgent"], "Mozilla/5.0 Chrome/130.0.0.0",
            "a headless browser must not name itself"
        );
    }

    #[test]
    fn accept_language_adds_the_bare_language_after_a_regional_locale() {
        assert_eq!(accept_language("fr-FR"), "fr-FR,fr;q=0.9");
        assert_eq!(accept_language("en"), "en");
        assert_eq!(accept_language("-US"), "-US");
    }

    #[tokio::test]
    async fn a_fingerprint_is_installed_before_the_page_loads_anything() {
        let (browser, mock) = mocked();
        let fingerprint = Fingerprint::randomized(OsType::Macos, 7);
        let profile = pure_cdp_profile(json!({
            "fingerprint": serde_json::to_value(&fingerprint).unwrap(),
            "locale": "fr-FR",
        }));

        start(&browser, &profile, Some("https://example.com/"))
            .await
            .unwrap();

        let script = bootstrap_script(&fingerprint);
        let installed = mock
            .calls_to("Page.addScriptToEvaluateOnNewDocument")
            .iter()
            .position(|call| call.params["source"] == script.as_str());
        assert!(
            installed.is_some(),
            "the fingerprint script was not installed"
        );
        let user_agent = mock.calls_to("Network.setUserAgentOverride");
        assert_eq!(user_agent.len(), 1);
        assert_eq!(
            user_agent[0].params["userAgent"].as_str(),
            fingerprint.user_agent.as_deref()
        );
        assert!(
            position(&mock, "Network.setUserAgentOverride") < position(&mock, "Page.navigate"),
            "the identity must be in place before the first navigation"
        );
        // The fingerprint's locale is the one in effect, set once.
        assert_eq!(mock.calls_to("Emulation.setLocaleOverride").len(), 1);
    }

    #[tokio::test]
    async fn a_fingerprints_proxy_password_is_never_sent_as_a_header() {
        let (browser, mock) = mocked();
        let mut fingerprint = Fingerprint::randomized(OsType::Linux, 3);
        fingerprint.proxy = Some(ProxyConfig {
            r#type: "http".to_owned(),
            host: "proxy.example".to_owned(),
            port: 8080,
            username: Some("alice".to_owned()),
            password: Some(FAKE_PROXY_PASSWORD.to_owned()),
            save_traffic: false,
        });
        let profile = pure_cdp_profile(json!({
            "fingerprint": serde_json::to_value(&fingerprint).unwrap(),
        }));

        start(&browser, &profile, None).await.unwrap();

        assert!(mock.calls_to("Network.setExtraHTTPHeaders").is_empty());
        assert!(
            !format!("{:?}", mock.calls()).contains(FAKE_PROXY_PASSWORD),
            "a proxy password reached the protocol"
        );
    }

    #[tokio::test]
    async fn saved_cookies_are_set_in_the_tab_before_the_start_page() {
        let (browser, mock) = mocked();
        let profile = pure_cdp_profile(json!({
            "cookies": [{
                "name": "session",
                "value": FAKE_COOKIE,
                "domain": ".example.com",
                "path": "/",
                "secure": true,
                "http_only": true,
                "same_site": "Lax",
                "expires": 1893456000,
            }],
        }));

        start(&browser, &profile, Some("https://example.com/"))
            .await
            .unwrap();

        let set = mock.calls_to("Network.setCookies");
        assert_eq!(set.len(), 1);
        let cookie = &set[0].params["cookies"][0];
        assert_eq!(cookie["name"], "session");
        assert_eq!(cookie["value"], FAKE_COOKIE);
        assert_eq!(cookie["domain"], ".example.com");
        assert_eq!(cookie["sameSite"], "Lax");
        assert_eq!(cookie["httpOnly"], true);
        assert_eq!(cookie["secure"], true);
        assert!(position(&mock, "Network.setCookies") < position(&mock, "Page.navigate"));
        let navigations = mock.calls_to("Page.navigate");
        assert_eq!(navigations[0].params["url"], "https://example.com/");
    }

    #[tokio::test]
    async fn without_a_start_url_nothing_is_opened() {
        let (browser, mock) = mocked();

        start(&browser, &pure_cdp_profile(json!({})), None)
            .await
            .unwrap();

        assert!(mock.calls_to("Page.navigate").is_empty());
    }

    #[tokio::test]
    async fn a_failed_step_disposes_the_context_and_names_the_stage() {
        let (browser, mock) = mocked();
        mock.fail("Network.setCookies", "no cookies today");
        let profile = pure_cdp_profile(json!({
            "cookies": [{ "name": "a", "value": "b", "domain": "example.com", "path": "/" }],
        }));

        let error = start(&browser, &profile, None).await.unwrap_err();

        assert_eq!(error.stage, Stage::Cookies);
        assert_eq!(error.stage.code(), "COOKIE_FAILED");
        let disposed = mock.calls_to("Target.disposeBrowserContext");
        assert_eq!(disposed.len(), 1, "the context must not be left behind");
        assert_eq!(disposed[0].params["browserContextId"], "CTX1");
    }

    #[tokio::test]
    async fn a_failing_override_is_an_override_error_that_names_the_command() {
        let (browser, mock) = mocked();
        mock.fail("Emulation.setGeolocationOverride", "denied");
        let profile = pure_cdp_profile(json!({ "latitude": 1.0, "longitude": 2.0 }));

        let error = start(&browser, &profile, None).await.unwrap_err();

        assert_eq!(error.stage, Stage::Overrides);
        assert!(error.message.contains("geolocation"), "{error}");
    }

    #[tokio::test]
    async fn a_context_the_browser_refuses_is_a_launch_error() {
        let (browser, mock) = mocked();
        mock.fail("Target.createBrowserContext", "no contexts");

        let error = start(&browser, &pure_cdp_profile(json!({})), None)
            .await
            .unwrap_err();

        assert_eq!(error.stage, Stage::Launch);
        assert!(mock.calls_to("Target.disposeBrowserContext").is_empty());
    }

    #[tokio::test]
    async fn a_failing_start_page_is_an_open_error() {
        let (browser, mock) = mocked();
        mock.fail("Page.navigate", "net::ERR_NAME_NOT_RESOLVED");

        let error = start(
            &browser,
            &pure_cdp_profile(json!({})),
            Some("https://nowhere.invalid/"),
        )
        .await
        .unwrap_err();

        assert_eq!(error.stage, Stage::Open);
    }

    #[tokio::test]
    async fn quitting_disposes_the_context_and_closes_the_browser() {
        let (browser, mock) = mocked();
        let session = start(&browser, &pure_cdp_profile(json!({})), None)
            .await
            .unwrap();
        let mut session = Session::PureCdp(session);

        session.quit().await.unwrap();

        assert_eq!(mock.calls_to("Target.disposeBrowserContext").len(), 1);
        assert!(!browser.is_connected());
    }

    #[tokio::test]
    async fn a_session_navigates_runs_scripts_and_sets_cookies() {
        let (browser, mock) = mocked();
        mock.reply("Runtime.evaluate", json!({ "result": { "value": 42 } }));
        let mut session = Session::PureCdp(
            start(&browser, &pure_cdp_profile(json!({})), None)
                .await
                .unwrap(),
        );

        session.open("https://example.com/next").await.unwrap();
        let answer = session.execute_script("6 * 7").await.unwrap();
        session
            .set_cookies(&[BrowserCookie {
                name: "a".into(),
                value: "b".into(),
                domain: "example.com".into(),
                path: "/".into(),
                expires: None,
                secure: false,
                http_only: false,
                same_site: String::new(),
            }])
            .await
            .unwrap();
        session.set_geolocation(1.0, 2.0, 5.0).await.unwrap();

        assert_eq!(answer, "42");
        let navigated = mock.calls_to("Page.navigate");
        assert_eq!(
            navigated.last().unwrap().params["url"],
            "https://example.com/next"
        );
        assert_eq!(mock.calls_to("Network.setCookies").len(), 1);
        let location = mock.calls_to("Emulation.setGeolocationOverride");
        assert_eq!(location.last().unwrap().params["accuracy"], 5.0);
        assert_eq!(
            mock.calls_to("Browser.grantPermissions")
                .last()
                .unwrap()
                .params["browserContextId"],
            "CTX1",
            "a location set after launch needs the permission too"
        );
    }

    #[tokio::test]
    async fn a_screenshot_is_saved_as_a_png_file() {
        let (browser, mock) = mocked();
        // "PNGDATA" in base64.
        mock.reply("Page.captureScreenshot", json!({ "data": "UE5HREFUQQ==" }));
        let session = start(&browser, &pure_cdp_profile(json!({})), None)
            .await
            .unwrap();
        let dir = tempfile::tempdir().unwrap();

        let path = session.screenshot_into(dir.path()).await.unwrap();

        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));
        assert!(path.starts_with(dir.path()));
        assert_eq!(std::fs::read(&path).unwrap(), b"PNGDATA");
    }

    #[tokio::test]
    async fn the_endpoint_of_a_pure_cdp_session_is_the_browsers_own() {
        let (browser, _mock) = mocked();
        let session = Session::PureCdp(
            start(&browser, &pure_cdp_profile(json!({})), None)
                .await
                .unwrap(),
        );

        let (port, address) = session.debug_endpoint().expect("an endpoint");

        assert_eq!(port, browser.debugging_port());
        assert_eq!(address, browser.websocket_url());
    }

    // ---- pure helpers ----

    #[test]
    fn launch_options_follow_the_profile_and_leave_the_proxy_to_the_context() {
        let profile = pure_cdp_profile(json!({
            "headless": true,
            "proxy": format!("http://alice:{FAKE_PROXY_PASSWORD}@proxy.example:8080"),
        }));

        let options = launch_options(&build_config(&profile)).unwrap();

        assert!(options.is_headless());
        assert!(
            options.proxy().is_none(),
            "the proxy belongs to the context, not the browser"
        );
    }

    #[test]
    fn a_headed_profile_launches_headed() {
        let profile = pure_cdp_profile(json!({ "headless": false }));

        let options = launch_options(&build_config(&profile)).unwrap();

        assert!(!options.is_headless());
    }

    #[test]
    fn cookie_conversion_maps_same_site_and_expiry() {
        let make = |same_site: &str, expires: Option<f64>| BrowserCookie {
            name: "n".into(),
            value: "v".into(),
            domain: "example.com".into(),
            path: "/".into(),
            expires,
            secure: true,
            http_only: false,
            same_site: same_site.into(),
        };

        assert_eq!(
            to_cdp_cookie(&make("Strict", None)).same_site,
            Some(SameSite::Strict)
        );
        assert_eq!(
            to_cdp_cookie(&make("lax", None)).same_site,
            Some(SameSite::Lax)
        );
        assert_eq!(
            to_cdp_cookie(&make("None", None)).same_site,
            Some(SameSite::None)
        );
        assert_eq!(to_cdp_cookie(&make("", None)).same_site, None);
        assert_eq!(to_cdp_cookie(&make("unspecified", None)).same_site, None);
        assert_eq!(to_cdp_cookie(&make("Lax", Some(5.0))).expires, Some(5.0));
        assert_eq!(to_cdp_cookie(&make("Lax", None)).expires, None);
    }

    #[test]
    fn webdriver_cookie_params_leave_out_what_is_absent() {
        let cookie = BrowserCookie {
            name: "n".into(),
            value: "v".into(),
            domain: "example.com".into(),
            path: "/".into(),
            expires: None,
            secure: false,
            http_only: true,
            same_site: String::new(),
        };

        let params = cookie_params(&cookie);

        assert!(params.get("sameSite").is_none());
        assert!(params.get("expires").is_none());
        assert_eq!(params["httpOnly"], true);
        assert_eq!(params["value"], "v");
    }

    #[test]
    fn the_flat_location_fields_are_used_when_the_profile_has_no_external_one() {
        let profile = pure_cdp_profile(json!({
            "latitude": 10.0,
            "longitude": 20.0,
            "accuracy": 7.0,
        }));

        assert_eq!(geolocation(&profile), Some((10.0, 20.0, 7.0)));
    }

    #[test]
    fn half_a_location_is_no_location() {
        assert_eq!(
            geolocation(&pure_cdp_profile(json!({ "latitude": 10.0 }))),
            None
        );
        assert_eq!(geolocation(&pure_cdp_profile(json!({}))), None);
    }

    #[test]
    fn the_location_defaults_to_100_metres_of_accuracy() {
        let profile = pure_cdp_profile(json!({ "latitude": 1.0, "longitude": 2.0 }));

        assert_eq!(geolocation(&profile), Some((1.0, 2.0, 100.0)));
    }

    /// Needs Google Chrome or Chromium installed, so it only runs on request:
    /// `cargo test -- --ignored real_chrome`.
    #[tokio::test]
    #[ignore = "launches a real browser"]
    async fn real_chrome_end_to_end() {
        let profile = pure_cdp_profile(json!({
            "latitude": 48.85,
            "longitude": 2.35,
            "locale": "fr-FR",
            "cookies": [{
                "name": "session",
                "value": FAKE_COOKIE,
                "domain": "example.com",
                "path": "/",
            }],
        }));
        let url = "data:text/html,<title>hello</title><body>hi</body>";

        let mut session = Session::launch(&profile, Some(url))
            .await
            .expect("Chrome should launch");
        let title = session.execute_script("document.title").await.unwrap();
        let language = session.execute_script("navigator.language").await.unwrap();
        let intl = session
            .execute_script("Intl.DateTimeFormat().resolvedOptions().locale")
            .await
            .unwrap();
        let user_agent = session.execute_script("navigator.userAgent").await.unwrap();
        let (port, address) = session.debug_endpoint().expect("an endpoint");
        let dir = tempfile::tempdir().unwrap();
        let shot = match &session {
            Session::PureCdp(s) => s.screenshot_into(dir.path()).await.unwrap(),
            Session::WebDriver(_) => unreachable!(),
        };
        let size = std::fs::metadata(&shot).unwrap().len();
        session.quit().await.unwrap();

        assert_eq!(title, "\"hello\"");
        assert_eq!(language, "\"fr-FR\"");
        assert_eq!(intl, "\"fr-FR\"");
        assert!(!user_agent.contains("Headless"), "{user_agent}");
        assert!(port > 0 && address.starts_with("ws://"), "{port} {address}");
        assert!(size > 100, "the screenshot is empty");
    }
}
