//! The [`Browser`]: a Chrome process, its tabs, and browser-wide settings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
#[cfg(feature = "plugins")]
use std::sync::atomic::{AtomicBool, Ordering};
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
use crate::common::orphan_guard::OrphanGuard;
use crate::error::SeleniumBaseError;

/// How long to wait for `Browser.close` to end the process before killing it.
const GRACEFUL_CLOSE: Duration = Duration::from_secs(3);

/// Where the DevTools endpoint of a browser this process launched listens.
const LOCAL_HOST: &str = "127.0.0.1";

/// The DevTools address a mocked browser reports.
#[cfg(any(test, feature = "test-util"))]
const MOCK_WS_URL: &str = "ws://mock.invalid/devtools/browser/mock";

/// Where downloads are expected until [`Browser::set_download_dir`] says
/// otherwise: the folder SeleniumBase's CDP Mode uses, in the working directory.
pub(super) const DEFAULT_DOWNLOAD_DIR: &str = "downloaded_files";

/// `host:port` for a URL, with an IPv6 address in the brackets a URL needs.
fn devtools_authority(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

#[cfg(test)]
mod host_tests {
    use super::*;
    use crate::sb_cdp::mock::MockCtrl;

    fn attached_to(host: &str, port: u16) -> Browser {
        Browser::from_parts(
            Arc::new(Client::mocked(MockCtrl::new())),
            None,
            None,
            None,
            (host.to_owned(), port),
            format!("ws://{host}:{port}/devtools/browser/x"),
            LaunchOptions::default(),
        )
    }

    #[test]
    fn a_browser_attached_on_another_machine_reports_that_machine() {
        let browser = attached_to("10.0.0.5", 9222);

        assert_eq!(browser.debugging_host(), "10.0.0.5");
        assert_eq!(browser.debugging_port(), 9222);
        assert_eq!(browser.http_url(), "http://10.0.0.5:9222");
    }

    #[test]
    fn an_ipv6_host_is_bracketed_in_the_url() {
        assert_eq!(devtools_authority("::1", 9222), "[::1]:9222");
        assert_eq!(devtools_authority("[::1]", 9222), "[::1]:9222");
        assert_eq!(devtools_authority("localhost", 9222), "localhost:9222");
        assert_eq!(attached_to("::1", 9222).http_url(), "http://[::1]:9222");
    }

    #[test]
    fn the_devtools_endpoint_names_the_browsers_own_addresses() {
        let endpoint = attached_to("127.0.0.1", 9333).devtools_endpoint();

        assert_eq!(endpoint.http_url(), "http://127.0.0.1:9333");
        assert_eq!(
            endpoint.websocket_url(),
            "ws://127.0.0.1:9333/devtools/browser/x"
        );
        assert_eq!(
            endpoint.chrome_devtools_mcp_args()[2],
            "--browserUrl=http://127.0.0.1:9333"
        );
    }

    #[test]
    fn a_mocked_browser_is_local() {
        let (browser, _mock) = Browser::new_mocked();

        assert_eq!(browser.debugging_host(), LOCAL_HOST);
    }
}

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
    /// Ends the launched browser if this program ends without closing it.
    _guard: Option<OrphanGuard>,
    /// The machine the DevTools endpoint is on: this one for a launched
    /// browser, the given one for a browser attached with `connect`.
    host: String,
    port: u16,
    ws_url: String,
    pub(crate) options: LaunchOptions,
    /// Tab id to the protocol session attached to it.
    sessions: StdMutex<HashMap<String, Arc<str>>>,
    tasks: StdMutex<Vec<JoinHandle<()>>>,
    /// Who answers which proxy's password prompts.
    pub(super) auth: Arc<ProxyAuth>,
    /// The folder the browser was last told to save downloads in.
    download_dir: StdMutex<Option<PathBuf>>,
    /// The proxy of every context created with its own, by context id, so a
    /// handoff to plain HTTP can follow the route a tab's traffic takes.
    pub(super) context_routes: StdMutex<HashMap<String, super::context::ContextRoute>>,
    /// The ids Chrome gave the extensions the launch loaded, in load order.
    extension_ids: StdMutex<Vec<String>>,
    /// The blocklist of each tab session, which the library's ad-block list and
    /// the plugins' raw `Network.setBlockedURLs` commands all add to.
    #[cfg(feature = "plugins")]
    pub(super) plugin_blocked: super::plugin_hooks::BlockedUrls,
    /// Whether the plugins' `on_close` is still owed: set when the browser is
    /// handed out, cleared when `close` or the last drop runs the hooks. A
    /// launch attempt that fails is never set, so a retry does not find its
    /// plugins already told the browser closed.
    #[cfg(feature = "plugins")]
    plugins_open: AtomicBool,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(tasks) = self.tasks.get_mut() {
            for task in tasks.drain(..) {
                task.abort();
            }
        }
        #[cfg(feature = "plugins")]
        self.close_plugins_in_background();
    }
}

#[cfg(feature = "plugins")]
impl Inner {
    /// Runs the plugins' `on_close` on the current runtime when the last handle
    /// goes without [`Browser::close`]. A drop cannot wait, so the hooks run
    /// after it, while the process is being killed.
    fn close_plugins_in_background(&mut self) {
        let plugins = &self.options.plugins;
        if !std::mem::replace(self.plugins_open.get_mut(), false) || plugins.is_empty() {
            return;
        }
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let plugins = plugins.clone();
            runtime.spawn(async move { super::plugin_hooks::on_close(&plugins).await });
        } else {
            tracing::event!(
                name: "cdp.plugin.close.skipped",
                tracing::Level::WARN,
                "the browser was dropped outside an async runtime, so its plugins' on_close did not run",
            );
        }
    }
}

impl Browser {
    /// Launches a browser.
    ///
    /// A failed launch is retried once, because Chrome occasionally loses a
    /// race with a leftover profile lock on the first attempt.
    ///
    /// With plugins (feature `plugins`), the set is checked and each plugin's
    /// `before_launch` may change the start-up settings first; `on_browser`
    /// runs once the browser answers, and the first tab gets the plugins'
    /// packages and `on_page_created`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::BrowserLaunch`] when no browser can be
    /// started, or [`SeleniumBaseError::CdpDriver`] when it starts but cannot
    /// be controlled. A plugin that refuses, or whose hook fails, stops the
    /// launch with [`SeleniumBaseError::Plugin`], and the launch is not
    /// retried.
    pub async fn launch(options: LaunchOptions) -> Result<Self, SeleniumBaseError> {
        #[cfg(feature = "plugins")]
        let options = super::plugin_hooks::before_launch(options).await?;
        let launched = Self::launch_retrying(&options).await;
        #[cfg(feature = "plugins")]
        Self::settle_plugins(&launched, &options.plugins).await;
        launched
    }

    /// Launches, retrying once when the browser itself failed.
    async fn launch_retrying(options: &LaunchOptions) -> Result<Self, SeleniumBaseError> {
        let mut attempt = 0_u8;
        loop {
            attempt += 1;
            match Self::launch_once(options).await {
                Ok(browser) => return Ok(browser),
                // A plugin that refused would refuse again, and its hooks may
                // already have acted: only a browser that failed is retried.
                Err(error) if attempt < 2 && !matches!(error, SeleniumBaseError::Plugin { .. }) => {
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
            Some(launched.guard),
            (LOCAL_HOST.to_owned(), launched.port),
            launched.ws_url,
            options.clone(),
        );
        browser.start().await?;
        Ok(browser)
    }

    /// Ends a launch for its plugins: a browser that is handed out owes them
    /// `on_close` when it closes, and a launch that failed after
    /// `before_launch` runs it now, once.
    #[cfg(feature = "plugins")]
    async fn settle_plugins(
        launched: &Result<Self, SeleniumBaseError>,
        plugins: &seleniumbase_rs_plugin::PluginSet,
    ) {
        match launched {
            Ok(browser) => browser.inner.plugins_open.store(true, Ordering::SeqCst),
            Err(_) => super::plugin_hooks::on_close(plugins).await,
        }
    }

    /// What a launch does once the browser answers: proxy passwords, the
    /// extensions, the plugins' `on_browser`, the first tab and the start URL.
    async fn start(&self) -> Result<(), SeleniumBaseError> {
        if self.inner.auth.has_launch_credentials() {
            self.ensure_proxy_auth();
        }
        for dir in &self.inner.options.extensions {
            let loaded = self
                .execute(
                    "Extensions.loadUnpacked",
                    json!({ "path": dir.to_string_lossy() }),
                )
                .await
                .map_err(|error| {
                    SeleniumBaseError::invalid_config(format!(
                        "the extension in {} could not be loaded: {error}",
                        dir.display()
                    ))
                })?;
            // Chrome names the extension it loaded; the id is what tells the
            // extensions of a launch apart.
            if let Some(id) = loaded["id"].as_str() {
                locked(&self.inner.extension_ids).push(id.to_owned());
            }
        }
        #[cfg(feature = "plugins")]
        super::plugin_hooks::on_browser(&self.inner.options.plugins, &self.inner.client).await?;
        let page = self.default_page().await?;
        if let Some(url) = &self.inner.options.url {
            page.goto(url).await?;
        }
        Ok(())
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
        Self::attach(host, port, LaunchOptions::default()).await
    }

    /// Attaches to a running browser, as [`connect`](Self::connect) does, and
    /// runs these plugins' hooks (feature `plugins`).
    ///
    /// The set is checked as attaching (a `LAUNCH_ONLY` plugin refuses) and
    /// against the browser's headless state, read from its user agent; then
    /// `on_browser` runs, and every tab the library attaches to gets the
    /// plugins' packages and `on_page_created`. Nothing is launched, so
    /// `before_launch` does not run.
    ///
    /// # Errors
    ///
    /// What [`connect`](Self::connect) returns, and
    /// [`SeleniumBaseError::Plugin`] when a plugin refuses or `on_browser`
    /// fails.
    #[cfg(feature = "plugins")]
    pub async fn connect_with_plugins(
        host: &str,
        port: u16,
        plugins: seleniumbase_rs_plugin::PluginSet,
    ) -> Result<Self, SeleniumBaseError> {
        Self::attach(
            host,
            port,
            LaunchOptions::builder().plugins(plugins).build()?,
        )
        .await
    }

    async fn attach(
        host: &str,
        port: u16,
        options: LaunchOptions,
    ) -> Result<Self, SeleniumBaseError> {
        crate::install_tls_provider();
        let version: Value = reqwest::get(format!(
            "http://{}/json/version",
            devtools_authority(host, port)
        ))
        .await
        .map_err(|e| SeleniumBaseError::cdp_driver(format!("cannot reach {host}:{port}: {e}")))?
        .json()
        .await
        .map_err(|e| SeleniumBaseError::cdp_driver(format!("bad /json/version reply: {e}")))?;
        let ws_url = version["webSocketDebuggerUrl"]
            .as_str()
            .ok_or_else(|| SeleniumBaseError::cdp_driver("no webSocketDebuggerUrl advertised"))?
            .to_owned();
        // Each attach is a launch of its own for the plugins: a set shared by
        // several browsers gets fresh instances of the per-launch ones.
        #[cfg(feature = "plugins")]
        let options = LaunchOptions {
            plugins: super::plugin_hooks::for_attach(&options.plugins, &version)?,
            ..options
        };
        let client = Arc::new(Client::connect(&ws_url).await?);
        let browser = Self::from_parts(
            client,
            None,
            None,
            None,
            (host.to_owned(), port),
            ws_url,
            options,
        );
        #[cfg(feature = "plugins")]
        super::plugin_hooks::on_browser(&browser.inner.options.plugins, &browser.inner.client)
            .await?;
        #[cfg(feature = "plugins")]
        browser.inner.plugins_open.store(true, Ordering::SeqCst);
        Ok(browser)
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
            None,
            (LOCAL_HOST.to_owned(), 0),
            MOCK_WS_URL.to_owned(),
            LaunchOptions::default(),
        );
        (browser, ctrl)
    }

    /// Runs the steps of [`launch`](Self::launch) against `mock` instead of
    /// Chrome.
    ///
    /// No process starts, but everything after it happens as in a real launch:
    /// the plugins are checked and their hooks run (feature `plugins`), the
    /// extensions are loaded, and the first tab is prepared and opened at the
    /// start URL. Script `mock` first for the answers the launch needs.
    ///
    /// # Errors
    ///
    /// What [`launch`](Self::launch) returns for the same steps, such as
    /// [`SeleniumBaseError::Plugin`] when a plugin refuses or a hook fails.
    #[cfg(any(test, feature = "test-util"))]
    pub async fn launch_mocked(
        options: LaunchOptions,
        mock: &super::mock::MockCtrl,
    ) -> Result<Self, SeleniumBaseError> {
        #[cfg(feature = "plugins")]
        let options = super::plugin_hooks::before_launch(options).await?;
        #[cfg(feature = "plugins")]
        let plugins = options.plugins.clone();
        let browser = Self::from_parts(
            Arc::new(Client::mocked(mock.clone())),
            None,
            None,
            None,
            (LOCAL_HOST.to_owned(), 0),
            MOCK_WS_URL.to_owned(),
            options,
        );
        let launched = browser.start().await.map(|()| browser);
        #[cfg(feature = "plugins")]
        Self::settle_plugins(&launched, &plugins).await;
        launched
    }

    fn from_parts(
        client: Arc<Client>,
        process: Option<Child>,
        profile: Option<TempDir>,
        guard: Option<OrphanGuard>,
        (host, port): (String, u16),
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
                _guard: guard,
                host,
                port,
                ws_url,
                options,
                sessions: StdMutex::new(HashMap::new()),
                tasks: StdMutex::new(Vec::new()),
                download_dir: StdMutex::new(None),
                context_routes: StdMutex::new(HashMap::new()),
                extension_ids: StdMutex::new(Vec::new()),
                #[cfg(feature = "plugins")]
                plugin_blocked: super::plugin_hooks::BlockedUrls::default(),
                #[cfg(feature = "plugins")]
                plugins_open: AtomicBool::new(false),
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

    /// The machine the DevTools endpoint is on: `127.0.0.1` for a browser this
    /// handle launched, and the host given to [`connect`](Self::connect) for one
    /// that was attached.
    #[must_use]
    pub fn debugging_host(&self) -> &str {
        &self.inner.host
    }

    /// The HTTP address of the DevTools endpoint, such as `http://127.0.0.1:9222`.
    ///
    /// For a browser attached with [`connect`](Self::connect) the host is the
    /// one that was given, not `127.0.0.1`.
    #[must_use]
    pub fn http_url(&self) -> String {
        format!(
            "http://{}",
            devtools_authority(&self.inner.host, self.inner.port)
        )
    }

    /// The browser-level DevTools WebSocket address.
    #[must_use]
    pub fn websocket_url(&self) -> &str {
        &self.inner.ws_url
    }

    /// The ids Chrome gave the unpacked extensions this launch loaded
    /// ([`LaunchOptionsBuilder::extension`](super::LaunchOptionsBuilder::extension),
    /// or a plugin's `LaunchSettings::extensions`), in the order they were
    /// loaded.
    ///
    /// Empty for a browser attached with [`connect`](Self::connect), which
    /// loads none, and for an extension Chrome loaded without naming it.
    #[must_use]
    pub fn extension_ids(&self) -> Vec<String> {
        locked(&self.inner.extension_ids).clone()
    }

    /// The DevTools endpoint, with the configs other clients (Google's
    /// `chrome-devtools-mcp`, Microsoft's Playwright MCP) need to attach to
    /// this browser while this handle drives it.
    #[must_use]
    pub fn devtools_endpoint(&self) -> super::DevToolsEndpoint {
        super::DevToolsEndpoint::new(self.http_url(), self.websocket_url())
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
        let session = match self.session_for(id).await {
            Ok(session) => session,
            Err(error) => {
                if matches!(error, SeleniumBaseError::Plugin { .. }) {
                    // A tab its plugins did not prepare is neither handed out
                    // nor left open.
                    let _ = self
                        .execute("Target.closeTarget", json!({ "targetId": id }))
                        .await;
                }
                return Err(error);
            }
        };
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
            #[cfg(feature = "plugins")]
            self.inner.plugin_blocked.forget(&session);
        }
    }

    /// Enables the domains a tab needs and injects the page helpers, then
    /// gives the tab the plugins' packages and `on_page_created`.
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
            // A plugin's own blocklist is added to this one, not put in its place.
            #[cfg(feature = "plugins")]
            self.inner.plugin_blocked.seed(session, &AD_BLOCK_PATTERNS);
        }
        if self.inner.auth.has_launch_credentials() {
            send(
                "Fetch.enable",
                json!({ "handleAuthRequests": true, "patterns": [{ "urlPattern": "*" }] }),
            )
            .await?;
        }
        // Last, so a plugin sees the tab as the library left it and may
        // change any of it.
        #[cfg(feature = "plugins")]
        super::plugin_hooks::on_page_created(
            &self.inner.options.plugins,
            client,
            session,
            &self.inner.plugin_blocked,
        )
        .await?;
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
        let names: Vec<_> = permissions.iter().map(|p| p.name()).collect();
        self.execute("Browser.grantPermissions", json!({ "permissions": names }))
            .await?;
        Ok(())
    }

    /// Grants permissions without a prompt, to one origin only.
    ///
    /// Python's `grant_permissions(permissions, origin)`; without an origin the
    /// grant applies to all of them, as [`grant_permissions`](Self::grant_permissions)
    /// does.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects them.
    pub async fn grant_permissions_for(
        &self,
        origin: &str,
        permissions: &[Permission],
    ) -> Result<(), SeleniumBaseError> {
        let names: Vec<_> = permissions.iter().map(|p| p.name()).collect();
        self.execute(
            "Browser.grantPermissions",
            json!({ "permissions": names, "origin": origin }),
        )
        .await?;
        Ok(())
    }

    /// Grants every permission in [`Permission::ALL`] for all origins, which is
    /// the list Python's `grant_all_permissions` grants.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser rejects them.
    pub async fn grant_all_permissions(&self) -> Result<(), SeleniumBaseError> {
        self.grant_permissions(&Permission::ALL).await
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
    /// The browser also reports each download as it begins and ends, which
    /// [`Page::download_file`](super::Page::download_file) and its relatives
    /// wait for. [`download_dir`](Self::download_dir) says where downloads go
    /// from now on.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::Io`] if `dir` cannot be created, or
    /// [`SeleniumBaseError::CdpDriver`] if the browser rejects it.
    pub async fn set_download_dir(&self, dir: impl AsRef<Path>) -> Result<(), SeleniumBaseError> {
        self.start_downloads_in(dir.as_ref()).await?;
        Ok(())
    }

    /// [`set_download_dir`](Self::set_download_dir), returning the absolute path
    /// of the folder.
    pub(super) async fn start_downloads_in(
        &self,
        dir: &Path,
    ) -> Result<PathBuf, SeleniumBaseError> {
        std::fs::create_dir_all(dir)?;
        // Chrome requires an absolute path, and on Windows a plain `C:\...` one.
        let absolute = super::download::plain_path(std::fs::canonicalize(dir)?);
        self.execute(
            "Browser.setDownloadBehavior",
            json!({ "behavior": "allow", "downloadPath": absolute, "eventsEnabled": true }),
        )
        .await?;
        *locked(&self.inner.download_dir) = Some(absolute.clone());
        Ok(absolute)
    }

    /// The folder downloads are saved in: the last one given to
    /// [`set_download_dir`](Self::set_download_dir) or to one of the `Page`
    /// download methods, as an absolute path.
    ///
    /// Until one is given this is `downloaded_files`, relative to the working
    /// directory, the folder SeleniumBase's CDP Mode uses. That is only where
    /// [`Page::assert_downloaded_file`](super::Page::assert_downloaded_file)
    /// looks: the browser saves wherever it would by default until it is told
    /// otherwise.
    #[must_use]
    pub fn download_dir(&self) -> PathBuf {
        locked(&self.inner.download_dir)
            .clone()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DOWNLOAD_DIR))
    }

    /// Closes the browser if this handle launched it.
    ///
    /// A browser attached with [`connect`](Self::connect) is left running.
    /// The plugins' `on_close` hooks run first, once; their errors are logged,
    /// not returned.
    ///
    /// # Errors
    ///
    /// Never fails in practice; the `Result` leaves room for cleanup errors.
    pub async fn close(&self) -> Result<(), SeleniumBaseError> {
        #[cfg(feature = "plugins")]
        if self.inner.plugins_open.swap(false, Ordering::SeqCst) {
            super::plugin_hooks::on_close(&self.inner.options.plugins).await;
        }
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
