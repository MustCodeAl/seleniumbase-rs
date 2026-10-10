//! The [`Browser`]: a Chrome process, its tabs, and browser-wide settings.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::process::Child;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::client::{Client, Events};
use super::launch::{launch, LaunchOptions};
use super::page::{Page, DEFAULT_TIMEOUT};
use super::proxy_auth::{Credentials, ProxyAuth};
use super::sync::locked;
use super::types::{PageInfo, Permission};
use crate::error::SeleniumBaseError;

/// How long to wait for `Browser.close` to end the process before killing it.
const GRACEFUL_CLOSE: Duration = Duration::from_secs(3);

/// Hosts blocked by [`LaunchOptionsBuilder::ad_block`](super::LaunchOptionsBuilder::ad_block).
///
/// Deliberately short: well-known advertising and tracking endpoints only.
const AD_BLOCK_PATTERNS: [&str; 14] = [
    "*://*.doubleclick.net/*",
    "*://*.googlesyndication.com/*",
    "*://*.googleadservices.com/*",
    "*://*.google-analytics.com/*",
    "*://*.googletagmanager.com/*",
    "*://*.adnxs.com/*",
    "*://*.adsrvr.org/*",
    "*://*.advertising.com/*",
    "*://*.amazon-adsystem.com/*",
    "*://*.criteo.com/*",
    "*://*.outbrain.com/*",
    "*://*.taboola.com/*",
    "*://*.scorecardresearch.com/*",
    "*://*.facebook.net/*",
];

/// Where a new tab opens when it is not in the default context.
#[derive(Debug, Clone, Copy)]
pub(super) struct ContextTarget<'a> {
    pub(super) id: &'a str,
    /// The password of the proxy the context routes through, if it has one.
    pub(super) credentials: Option<&'a Credentials>,
}

/// A Chrome browser driven over the DevTools Protocol, with no WebDriver.
///
/// A `Browser` owns the process and is cheap to clone; every clone refers to
/// the same browser. Open tabs are [`Page`]s. The process is stopped by
/// [`close`](Self::close), or killed when the last clone is dropped.
///
/// # Examples
///
/// ```no_run
/// use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
///
/// # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
/// let browser = Browser::launch(LaunchOptions::builder().headless(true).build()?).await?;
/// let page = browser.default_page().await?;
/// page.goto("https://seleniumbase.io/simple/login").await?;
/// page.locator("#username").fill("demo_user").await?;
/// page.locator("button").click().await?;
/// browser.close().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Browser {
    inner: Arc<Inner>,
}

#[derive(Debug)]
pub(crate) struct Inner {
    pub(crate) client: Arc<Client>,
    process: Mutex<Option<Child>>,
    _profile: Option<TempDir>,
    port: u16,
    ws_url: String,
    pub(crate) options: LaunchOptions,
    /// Tab id to the protocol session attached to it.
    sessions: StdMutex<HashMap<String, Arc<str>>>,
    tasks: StdMutex<Vec<JoinHandle<()>>>,
    /// Who answers which proxy's password prompts.
    pub(super) auth: Arc<ProxyAuth>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(tasks) = self.tasks.get_mut() {
            for task in tasks.drain(..) {
                task.abort();
            }
        }
    }
}

impl Browser {
    /// Launches a browser.
    ///
    /// A failed launch is retried once, because Chrome occasionally loses a
    /// race with a leftover profile lock on the first attempt.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::BrowserLaunch`] when no browser can be
    /// started, or [`SeleniumBaseError::CdpDriver`] when it starts but cannot
    /// be controlled.
    pub async fn launch(options: LaunchOptions) -> Result<Self, SeleniumBaseError> {
        let mut attempt = 0_u8;
        loop {
            attempt += 1;
            match Self::launch_once(&options).await {
                Ok(browser) => return Ok(browser),
                Err(error) if attempt < 2 => {
                    tracing::event!(
                        name: "cdp.browser.launch.retry",
                        tracing::Level::WARN,
                        attempt,
                        error = %error,
                        "launch attempt {{attempt}} failed, retrying: {{error}}",
                    );
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn launch_once(options: &LaunchOptions) -> Result<Self, SeleniumBaseError> {
        let launched = launch(options).await?;
        let client = Arc::new(Client::connect(&launched.ws_url).await?);
        let browser = Self::from_parts(
            client,
            Some(launched.child),
            launched.profile,
            launched.port,
            launched.ws_url,
            options.clone(),
        );
        if browser.inner.auth.has_launch_credentials() {
            browser.ensure_proxy_auth();
        }
        let page = browser.default_page().await?;
        if let Some(url) = &options.url {
            page.goto(url).await?;
        }
        Ok(browser)
    }

    /// Attaches to a browser that is already running with remote debugging on.
    ///
    /// For example one started with `--remote-debugging-port=9222`. The
    /// browser keeps running after [`close`](Self::close).
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the endpoint cannot be
    /// reached or does not describe a DevTools browser.
    pub async fn connect(host: &str, port: u16) -> Result<Self, SeleniumBaseError> {
        let version: Value = reqwest::get(format!("http://{host}:{port}/json/version"))
            .await
            .map_err(|e| SeleniumBaseError::cdp_driver(format!("cannot reach {host}:{port}: {e}")))?
            .json()
            .await
            .map_err(|e| SeleniumBaseError::cdp_driver(format!("bad /json/version reply: {e}")))?;
        let ws_url = version["webSocketDebuggerUrl"]
            .as_str()
            .ok_or_else(|| SeleniumBaseError::cdp_driver("no webSocketDebuggerUrl advertised"))?
            .to_owned();
        let client = Arc::new(Client::connect(&ws_url).await?);
        Ok(Self::from_parts(
            client,
            None,
            None,
            port,
            ws_url,
            LaunchOptions::default(),
        ))
    }

    /// Creates a browser that talks to a scripted mock instead of Chrome.
    ///
    /// For testing code that drives pages. The returned [`MockCtrl`] scripts
    /// the browser's answers and records what it was asked.
    ///
    /// [`MockCtrl`]: super::MockCtrl
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn new_mocked() -> (Self, super::mock::MockCtrl) {
        let ctrl = super::mock::MockCtrl::new();
        let client = Arc::new(Client::mocked(ctrl.clone()));
        let browser = Self::from_parts(
            client,
            None,
            None,
            0,
            "ws://mock.invalid/devtools/browser/mock".to_owned(),
            LaunchOptions::default(),
        );
        (browser, ctrl)
    }

    fn from_parts(
        client: Arc<Client>,
        process: Option<Child>,
        profile: Option<TempDir>,
        port: u16,
        ws_url: String,
        options: LaunchOptions,
    ) -> Self {
        let launch_credentials = options
            .proxy()
            .and_then(super::launch::Proxy::credentials)
            .map(|(user, pass)| (user.to_owned(), pass.to_owned()));
        Self {
            inner: Arc::new(Inner {
                auth: Arc::new(ProxyAuth::new(launch_credentials)),
                client,
                process: Mutex::new(process),
                _profile: profile,
                port,
                ws_url,
                options,
                sessions: StdMutex::new(HashMap::new()),
                tasks: StdMutex::new(Vec::new()),
            }),
        }
    }

    pub(crate) fn inner(&self) -> &Inner {
        &self.inner
    }

    /// The port the browser's DevTools endpoint listens on.
    #[must_use]
    pub fn debugging_port(&self) -> u16 {
        self.inner.port
    }

    /// The HTTP address of the DevTools endpoint, such as `http://127.0.0.1:9222`.
    #[must_use]
    pub fn http_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.inner.port)
    }

    /// The browser-level DevTools WebSocket address.
    #[must_use]
    pub fn websocket_url(&self) -> &str {
        &self.inner.ws_url
    }

    /// Who answers this browser's proxy password prompts.
    pub(super) fn proxy_auth(&self) -> &ProxyAuth {
        &self.inner.auth
    }

    /// Whether the connection to the browser is still open.
    ///
    /// Turns `false` once the browser exits or the connection drops.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.inner.client.is_open()
    }

    /// Subscribes to protocol events from every tab.
    #[must_use]
    pub fn events(&self) -> Events {
        self.inner.client.events()
    }

    /// Sends a raw protocol command to the browser itself.
    ///
    /// An escape hatch for commands this API does not wrap. Use
    /// [`Page::execute`] to address a tab.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn execute(&self, method: &str, params: Value) -> Result<Value, SeleniumBaseError> {
        self.inner.client.send(method, params, None).await
    }

    // ------------------------------------------------------------------
    // Pages
    // ------------------------------------------------------------------

    /// Lists the open tabs without attaching to them.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser cannot be queried.
    pub async fn list_pages(&self) -> Result<Vec<PageInfo>, SeleniumBaseError> {
        let response = self.execute("Target.getTargets", json!({})).await?;
        Ok(response["targetInfos"]
            .as_array()
            .map(|infos| {
                infos
                    .iter()
                    .filter(|info| info["type"] == "page")
                    .map(|info| PageInfo {
                        id: info["targetId"].as_str().unwrap_or_default().to_owned(),
                        url: info["url"].as_str().unwrap_or_default().to_owned(),
                        title: info["title"].as_str().unwrap_or_default().to_owned(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Returns a handle to every open tab.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser cannot be queried
    /// or a tab cannot be attached to.
    pub async fn pages(&self) -> Result<Vec<Page>, SeleniumBaseError> {
        let infos = self.list_pages().await?;
        let mut pages = Vec::with_capacity(infos.len());
        for info in infos {
            pages.push(self.page(&info.id).await?);
        }
        Ok(pages)
    }

    /// Returns the first tab, creating a blank one if the browser has none.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if no tab can be attached to.
    pub async fn default_page(&self) -> Result<Page, SeleniumBaseError> {
        match self.list_pages().await?.into_iter().next() {
            Some(info) => self.page(&info.id).await,
            None => self.new_page(None::<&str>).await,
        }
    }

    /// Returns the most recently opened tab.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::SessionNotStarted`] if no tab is open.
    pub async fn newest_page(&self) -> Result<Page, SeleniumBaseError> {
        let infos = self.list_pages().await?;
        let newest = infos.last().ok_or(SeleniumBaseError::SessionNotStarted)?;
        self.page(&newest.id).await
    }

    /// Opens a new tab, optionally at `url`.
    ///
    /// A URL without a scheme is opened over HTTPS.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the tab cannot be created.
    pub async fn new_page(&self, url: Option<impl AsRef<str>>) -> Result<Page, SeleniumBaseError> {
        self.open_target(url.as_ref().map(AsRef::as_ref), false, None)
            .await
    }

    /// Opens a new window, optionally at `url`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the window cannot be created.
    pub async fn new_window(
        &self,
        url: Option<impl AsRef<str>>,
    ) -> Result<Page, SeleniumBaseError> {
        self.open_target(url.as_ref().map(AsRef::as_ref), true, None)
            .await
    }

    pub(super) async fn open_target(
        &self,
        url: Option<&str>,
        window: bool,
        context: Option<ContextTarget<'_>>,
    ) -> Result<Page, SeleniumBaseError> {
        // Open blank and navigate once attached. Creating the tab at the URL
        // would start loading before anything is listening, and a fresh tab's
        // blank document already reports "complete", so waiting on it can
        // return before the real navigation has begun.
        let mut params = json!({ "url": "about:blank", "newWindow": window });
        if let Some(context) = &context {
            params["browserContextId"] = json!(context.id);
        }
        let response = self.execute("Target.createTarget", params).await?;
        let id = response["targetId"].as_str().ok_or_else(|| {
            SeleniumBaseError::cdp_driver("Target.createTarget returned no targetId")
        })?;
        let session = self.session_for(id).await?;
        // A tab behind its own password-protected proxy needs its prompts
        // answered from its very first request.
        if let Some(credentials) = context.and_then(|context| context.credentials) {
            self.inner.auth.register(&session, credentials.clone());
            self.inner
                .client
                .send(
                    "Fetch.enable",
                    json!({ "handleAuthRequests": true, "patterns": [{ "urlPattern": "*" }] }),
                    Some(&session),
                )
                .await?;
            self.ensure_proxy_auth();
        }
        let page = Page::new(self.clone(), id.into(), session, DEFAULT_TIMEOUT);
        if let Some(url) = url {
            page.goto(url).await?;
        }
        Ok(page)
    }

    /// Returns the tab with this id, attaching to it if needed.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if there is no such tab.
    pub async fn page(&self, id: &str) -> Result<Page, SeleniumBaseError> {
        let session = self.session_for(id).await?;
        Ok(Page::new(self.clone(), id.into(), session, DEFAULT_TIMEOUT))
    }

    /// The protocol session attached to a tab, attaching if needed.
    async fn session_for(&self, id: &str) -> Result<Arc<str>, SeleniumBaseError> {
        let known = locked(&self.inner.sessions).get(id).cloned();
        let session = if let Some(session) = known {
            session
        } else {
            let response = self
                .execute(
                    "Target.attachToTarget",
                    json!({ "targetId": id, "flatten": true }),
                )
                .await?;
            let session: Arc<str> = response["sessionId"]
                .as_str()
                .ok_or_else(|| SeleniumBaseError::cdp_driver("attach returned no sessionId"))?
                .into();
            self.prepare_session(id, &session).await?;
            locked(&self.inner.sessions).insert(id.to_owned(), Arc::clone(&session));
            session
        };
        Ok(session)
    }

    /// Forgets a closed tab's session.
    pub(crate) fn forget_page(&self, id: &str) {
        let removed = locked(&self.inner.sessions).remove(id);
        if let Some(session) = removed {
            self.inner.auth.forget(&session);
        }
    }

    /// Enables the domains a tab needs and injects the page helpers.
    async fn prepare_session(&self, target: &str, session: &str) -> Result<(), SeleniumBaseError> {
        let client = &self.inner.client;
        let send = |method: &'static str, params: Value| client.send(method, params, Some(session));
        send("Page.enable", json!({})).await?;
        // Chrome only acknowledges input for the tab that has focus, so a click
        // on any other tab waits until the command times out. Telling every tab
        // it is focused lets several tabs be driven at once.
        send(
            "Emulation.setFocusEmulationEnabled",
            json!({ "enabled": true }),
        )
        .await?;
        send(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": super::page::HELPER_JS }),
        )
        .await?;
        send(
            "Runtime.evaluate",
            json!({ "expression": super::page::HELPER_JS }),
        )
        .await?;
        if self.inner.options.shield_webrtc {
            let shim = super::webrtc::RELAY_ONLY_SHIM;
            send(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": shim }),
            )
            .await?;
            send("Runtime.evaluate", json!({ "expression": shim })).await?;
        }
        if let Some(identity) = &self.inner.options.identity {
            for (method, params) in identity.tab_commands() {
                client
                    .send(method, params.clone(), Some(session))
                    .await
                    .map_err(|error| super::identity::refused(method, &error))?;
            }
            // Permissions belong to a browser context, and which one this tab
            // is in is only known to the browser.
            if identity.permission_grant(None).is_some() {
                let info = self
                    .execute("Target.getTargetInfo", json!({ "targetId": target }))
                    .await?;
                let context = info["targetInfo"]["browserContextId"].as_str();
                if let Some(grant) = identity.permission_grant(context) {
                    self.execute("Browser.grantPermissions", grant)
                        .await
                        .map_err(|error| {
                            super::identity::refused("Browser.grantPermissions", &error)
                        })?;
                }
            }
        }
        if self.inner.options.ad_block {
            send("Network.enable", json!({})).await?;
            send(
                "Network.setBlockedURLs",
                json!({ "urls": AD_BLOCK_PATTERNS }),
            )
            .await?;
        }
        if self.inner.auth.has_launch_credentials() {
            send(
                "Fetch.enable",
                json!({ "handleAuthRequests": true, "patterns": [{ "urlPattern": "*" }] }),
            )
            .await?;
        }
        Ok(())
    }

    /// Answers proxy authentication challenges with the configured credentials.
    ///
    /// Chrome ignores credentials placed in `--proxy-server`, so they have to
    /// be supplied when the proxy asks. Requests paused by `Fetch.enable` are
    /// released unchanged.
    fn ensure_proxy_auth(&self) {
        if !self.inner.auth.claim_start() {
            return;
        }
        let auth = Arc::clone(&self.inner.auth);
        let client = Arc::clone(&self.inner.client);
        let mut events = client.events();
        let task = tokio::spawn(async move {
            while let Some(event) = events.next().await {
                let Some(session) = event.session_id.as_deref() else {
                    continue;
                };
                if let Some((method, params)) = auth.reply(session, &event.method, &event.params) {
                    // A request that already finished has nothing to answer.
                    let _ = client.send(method, params, Some(session)).await;
                }
            }
        });
        locked(&self.inner.tasks).push(task);
    }

    // ------------------------------------------------------------------
    // Browser-wide settings
    // ------------------------------------------------------------------

    /// Grants permissions without a prompt.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects them.
    pub async fn grant_permissions(
        &self,
        permissions: &[Permission],
    ) -> Result<(), SeleniumBaseError> {
        let names: Vec<_> = permissions.iter().map(|p| p.protocol_name()).collect();
        self.execute("Browser.grantPermissions", json!({ "permissions": names }))
            .await?;
        Ok(())
    }

    /// Restores every permission to its default, prompting again.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] on a protocol error.
    pub async fn reset_permissions(&self) -> Result<(), SeleniumBaseError> {
        self.execute("Browser.resetPermissions", json!({})).await?;
        Ok(())
    }

    /// Saves downloads into `dir` instead of asking.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if `dir` cannot be created, or
    /// [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn set_download_dir(&self, dir: impl AsRef<Path>) -> Result<(), SeleniumBaseError> {
        std::fs::create_dir_all(dir.as_ref())?;
        // Chrome requires an absolute path.
        let absolute = std::fs::canonicalize(dir.as_ref())?;
        self.execute(
            "Browser.setDownloadBehavior",
            json!({ "behavior": "allow", "downloadPath": absolute, "eventsEnabled": true }),
        )
        .await?;
        Ok(())
    }

    /// Closes the browser if this handle launched it.
    ///
    /// A browser attached with [`connect`](Self::connect) is left running.
    ///
    /// # Errors
    ///
    /// Never fails in practice; the `Result` leaves room for cleanup errors.
    pub async fn close(&self) -> Result<(), SeleniumBaseError> {
        let tasks: Vec<_> = locked(&self.inner.tasks).drain(..).collect();
        for task in tasks {
            task.abort();
        }
        if let Some(mut child) = self.inner.process.lock().await.take() {
            // Ask politely first so the profile is flushed, then insist.
            let _ = tokio::time::timeout(GRACEFUL_CLOSE, self.execute("Browser.close", json!({})))
                .await;
            if tokio::time::timeout(GRACEFUL_CLOSE, child.wait())
                .await
                .is_err()
            {
                let _ = child.kill().await;
            }
        }
        #[cfg(any(test, feature = "test-util"))]
        self.inner.client.mark_closed();
        Ok(())
    }
}
