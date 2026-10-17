//! One browser for many tests: Python's `--reuse-session` (`--rs`,
//! `--reuse_session`) and `--reuse-class-session` (`--rcs`).
//!
//! With [`BrowserConfig::reuse_session`] on, [`BrowserSession::connect`] (and so
//! [`BaseCase::new`](crate::BaseCase::new)) does not always start a browser: it
//! borrows one that an earlier test started with the same configuration and has
//! finished with, and [`BaseCase::quit`](crate::BaseCase::quit) (or dropping the
//! case) gives it back instead of closing it.
//!
//! # How it works
//!
//! Each `#[tokio::test]` builds its own Tokio runtime, and the HTTP client
//! inside a thirtyfour `WebDriver` belongs to the runtime it was made in, so no
//! client is shared between tests. What is shared is the server's side of the
//! session. A process-wide registry keeps, for each browser that may be reused,
//! the address of its WebDriver server, its session id, the capabilities it
//! reported, and the driver process (and virtual display) this crate started
//! for it. A session that asks for reuse:
//!
//! 1. leases an idle browser whose configuration is the same as its own
//!    (everything but the start page and the reuse settings themselves), and
//!    attaches to it with a new client made in the current runtime;
//! 2. cleans it: dismisses an open dialog, closes every window but the first,
//!    deletes every cookie (all of them through DevTools on a Chromium browser,
//!    those of the open page through WebDriver elsewhere), clears the open
//!    page's local and session storage, and goes to `about:blank` (the start
//!    page, if there is one, is opened by `BaseCase::new` as for a new browser);
//! 3. when no matching browser is idle, starts one as usual and registers it.
//!
//! Only one test holds a browser at a time. A test that starts while every
//! matching browser is held gets a browser of its own, which is shared from then
//! on, as each pytest-xdist worker keeps its own browser in Python. A browser
//! that cannot be attached to or cleaned (it crashed, or its driver is gone) is
//! closed and replaced. Under cargo-nextest each test is a process of its own,
//! so nothing is shared there.
//!
//! # Closing the browsers
//!
//! A Rust test binary has no hook that runs after its last test, so an idle
//! browser is closed, with the driver this crate started for it, by a reaper
//! thread once no test has held it for [`idle_timeout`] (five seconds unless
//! [`set_idle_timeout`] or `SB_REUSE_IDLE_SECS` says otherwise), by
//! [`shutdown_reused_sessions`], and by
//! [`drain_cleanups`](crate::common::shutdown::drain_cleanups), which `sbrs`
//! and the MCP server call before they exit.
//!
//! When the process ends sooner than the idle timeout after the last test that
//! used a browser, as a test binary usually does, nothing in the process gets
//! the chance to close that browser. On Unix it ends anyway: the driver and the
//! browser belong to a process group that a watcher ends when this process
//! exits, by any means. On Windows nothing watches them; call
//! [`shutdown_reused_sessions`] where the run ends (the `main` of a
//! `harness = false` test target, or the end of a program).
//!
//! # What plays the role of a test class
//!
//! Python's `--rcs` shares a browser among the tests of one class. Rust tests
//! have no classes; the nearest thing is the module that holds them. With
//! [`ReuseClass::TestModule`] (`BrowserConfig::with_reuse_module_session`,
//! `sbrs --rcs`, `SB_REUSE_CLASS_SESSION`) the tests of one module share a
//! browser: the module is read from the name libtest gives the thread a test
//! runs on (`login::tests::valid_password` is in `login::tests`, and a test at
//! the root of an integration test file is in the root module, which the whole
//! file shares). With [`ReuseClass::Named`]
//! (`BrowserConfig::with_reuse_class_session`) the tests that give the same name
//! share one, wherever they are.
//!
//! # What differs from Python
//!
//! Python keeps the cookies and the page of the previous test unless `--crumbs`
//! is given; here a reused browser is always cleaned, because Rust tests run in
//! any order and side by side, so what one test leaves must not decide what the
//! next one sees. Python closes the shared browser when pytest ends, and the
//! class's browser when the next class starts; here the idle timeout does both.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thirtyfour::prelude::{WebDriver, WebDriverResult};
use thirtyfour::session::http::{Body, HttpClient};
use tracing::{debug, info, warn};

use crate::browser::config::BrowserConfig;
use crate::browser::launcher::DriverProcess;
use crate::browser::session::BrowserSession;
use crate::browser::virtual_display::XvfbProcess;
use crate::error::SeleniumBaseError;
use crate::stealth::cdp::CdpClient;

/// The environment variable that sets [`idle_timeout`], in whole seconds.
pub const IDLE_TIMEOUT_VAR: &str = "SB_REUSE_IDLE_SECS";

/// How long a reused browser that no test holds stays open, unless
/// [`set_idle_timeout`] or [`IDLE_TIMEOUT_VAR`] says otherwise.
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the WebDriver server is given to end one session when a browser is
/// closed.
const CLOSE_TIMEOUT: Duration = Duration::from_secs(10);

/// The per-request timeout of an attached client when the configuration names
/// none: thirtyfour's own default.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Empties the open page's local and session storage. A page without storage
/// (`about:blank`, `data:`) or one that refuses access throws, which is caught.
const CLEAR_STORAGE: &str = "try { window.localStorage.clear(); } catch (e) {} \
                             try { window.sessionStorage.clear(); } catch (e) {}";

/// Which tests share a browser under Python's `--rcs`. See the
/// [module documentation](self#what-plays-the-role-of-a-test-class).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReuseClass {
    /// The tests of one Rust module, read from the name of the thread the test
    /// runs on.
    TestModule,
    /// The tests that give this name.
    Named(String),
}

impl ReuseClass {
    /// The class this stands for on the calling thread: the name, or the
    /// module of the test running on this thread.
    #[must_use]
    pub fn resolve(&self) -> String {
        match self {
            Self::Named(name) => name.clone(),
            Self::TestModule => test_module(std::thread::current().name()),
        }
    }
}

/// The module of the test a libtest thread runs: the thread's name without its
/// last `::` segment, or the empty string (the root module) for a name without
/// one or a thread without a name.
fn test_module(thread_name: Option<&str>) -> String {
    thread_name
        .and_then(|name| name.rsplit_once("::"))
        .map_or_else(String::new, |(module, _)| module.to_owned())
}

/// What two sessions must share to use the same browser: the configuration the
/// browser was started with, less what does not change the browser (the start
/// page, which the session opens itself, and the reuse settings), and the class
/// under `--rcs`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ReuseKey {
    config: BrowserConfig,
    class: Option<String>,
}

impl ReuseKey {
    /// The key of `config`, with its class resolved on the calling thread.
    pub(crate) fn of(config: &BrowserConfig) -> Self {
        let class = config.launch.reuse_class.as_ref().map(ReuseClass::resolve);
        let mut config = config.clone();
        config.reuse_session = false;
        config.start_page = None;
        config.launch.reuse_class = None;
        // How a test behaves (demo mode, message time) does not change the
        // browser, so a demo test and a plain one still share browsers.
        config.launch.run = crate::api::run_settings::RunSettings::default();
        Self { config, class }
    }
}

/// Where a reusable browser is: what a new client needs to attach to it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SessionAddress {
    /// The WebDriver server.
    pub(crate) server_url: String,
    /// The session on it.
    pub(crate) session_id: String,
    /// The capabilities the server reported when the session was created.
    pub(crate) capabilities: Value,
}

/// A browser in the registry.
struct Entry {
    id: u64,
    key: ReuseKey,
    address: SessionAddress,
    /// The driver this crate started for the browser, stopped when it closes.
    driver_process: Option<DriverProcess>,
    /// The virtual screen of a headed Linux browser, stopped when it closes.
    virtual_display: Option<XvfbProcess>,
    /// When the browser was last given back; `None` while a test holds it.
    idle_since: Option<Instant>,
}

/// A source of the current time, so the reaper can be tested without waiting.
pub(crate) trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

/// The monotonic clock of the operating system.
struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// The browsers kept for reuse, with who holds them.
pub(crate) struct Registry {
    clock: Arc<dyn Clock>,
    entries: Vec<Entry>,
    next_id: u64,
    /// Whether the thread that closes idle browsers is running.
    reaper_running: bool,
}

impl Registry {
    pub(crate) fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            entries: Vec::new(),
            next_id: 1,
            reaper_running: false,
        }
    }

    /// Leases an idle browser for `key`, the one given back last, and returns
    /// its id and address; `None` when every browser for `key` is held or there
    /// is none.
    fn lease(&mut self, key: &ReuseKey) -> Option<(u64, SessionAddress)> {
        let entry = self
            .entries
            .iter_mut()
            .filter(|entry| entry.idle_since.is_some() && entry.key == *key)
            .max_by_key(|entry| entry.idle_since)?;
        entry.idle_since = None;
        Some((entry.id, entry.address.clone()))
    }

    /// Adds a browser, held by the caller, and returns its id.
    fn insert_leased(
        &mut self,
        key: ReuseKey,
        address: SessionAddress,
        driver_process: Option<DriverProcess>,
        virtual_display: Option<XvfbProcess>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push(Entry {
            id,
            key,
            address,
            driver_process,
            virtual_display,
            idle_since: None,
        });
        id
    }

    /// Gives the browser `id` back; its idle time starts now. Returns whether
    /// it was in the registry.
    fn release(&mut self, id: u64) -> bool {
        let now = self.clock.now();
        match self.entries.iter_mut().find(|entry| entry.id == id) {
            Some(entry) => {
                entry.idle_since = Some(now);
                true
            }
            None => false,
        }
    }

    /// Takes the browser `id` out of the registry, held or not.
    fn remove(&mut self, id: u64) -> Option<Entry> {
        let index = self.entries.iter().position(|entry| entry.id == id)?;
        Some(self.entries.swap_remove(index))
    }

    /// Takes out the browsers no test has held for `timeout` or longer: the
    /// reaper's sweep.
    fn take_expired(&mut self, timeout: Duration) -> Vec<Entry> {
        let now = self.clock.now();
        self.take_where(|entry| {
            entry
                .idle_since
                .is_some_and(|since| now.saturating_duration_since(since) >= timeout)
        })
    }

    /// Takes out every browser no test holds.
    fn take_idle(&mut self) -> Vec<Entry> {
        self.take_where(|entry| entry.idle_since.is_some())
    }

    fn take_where(&mut self, pick: impl FnMut(&Entry) -> bool) -> Vec<Entry> {
        let (taken, kept): (Vec<Entry>, Vec<Entry>) = std::mem::take(&mut self.entries)
            .into_iter()
            .partition(pick);
        self.entries = kept;
        taken
    }
}

/// The registry of this process.
fn global() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(Registry::new(Arc::new(SystemClock))))
}

/// The registry of this process, locked.
fn registry() -> MutexGuard<'static, Registry> {
    lock(global())
}

fn lock(registry: &Mutex<Registry>) -> MutexGuard<'_, Registry> {
    // A panic while the registry was held cannot leave it half changed.
    registry.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The idle timeout set with [`set_idle_timeout`], if one was.
static IDLE_TIMEOUT: Mutex<Option<Duration>> = Mutex::new(None);

/// Sets how long a reused browser that no test holds stays open, for the whole
/// process. It overrides [`IDLE_TIMEOUT_VAR`].
pub fn set_idle_timeout(timeout: Duration) {
    *IDLE_TIMEOUT.lock().unwrap_or_else(PoisonError::into_inner) = Some(timeout);
}

/// How long a reused browser that no test holds stays open: what
/// [`set_idle_timeout`] set, else [`IDLE_TIMEOUT_VAR`], else
/// [`DEFAULT_IDLE_TIMEOUT`].
#[must_use]
pub fn idle_timeout() -> Duration {
    let set = *IDLE_TIMEOUT.lock().unwrap_or_else(PoisonError::into_inner);
    set.unwrap_or_else(|| {
        static FROM_ENV: OnceLock<Duration> = OnceLock::new();
        *FROM_ENV.get_or_init(|| {
            let value = std::env::var(IDLE_TIMEOUT_VAR).ok();
            parse_idle_timeout(value.as_deref()).unwrap_or_else(|error| {
                warn!(%error, "ignoring {IDLE_TIMEOUT_VAR}; using the default idle timeout");
                DEFAULT_IDLE_TIMEOUT
            })
        })
    })
}

/// Reads an idle timeout given in whole seconds. `None` and blank text give
/// [`DEFAULT_IDLE_TIMEOUT`].
///
/// # Errors
///
/// Returns [`SeleniumBaseError::InvalidConfig`] if the text is not a whole
/// number of seconds.
pub fn parse_idle_timeout(value: Option<&str>) -> Result<Duration, SeleniumBaseError> {
    let Some(text) = value.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(DEFAULT_IDLE_TIMEOUT);
    };
    text.parse::<u64>()
        .map(Duration::from_secs)
        .map_err(|error| {
            SeleniumBaseError::invalid_config(format!(
                "{IDLE_TIMEOUT_VAR} must be a whole number of seconds, got {text:?}: {error}"
            ))
        })
}

/// How often the reaper looks: often enough that a browser closes soon after
/// its time is up, seldom enough to cost nothing.
fn poll_interval(timeout: Duration) -> Duration {
    (timeout / 5).clamp(Duration::from_millis(20), Duration::from_millis(500))
}

/// Starts the reaper unless it is running. It stops by itself when the registry
/// is empty, and is started again by the next browser registered.
fn ensure_reaper(registry: &mut Registry) {
    if registry.reaper_running {
        return;
    }
    match std::thread::Builder::new()
        .name("sb-reuse-reaper".to_owned())
        .spawn(reap_until_empty)
    {
        Ok(_) => registry.reaper_running = true,
        Err(error) => {
            warn!(%error, "could not start the thread that closes idle reused browsers");
        }
    }
}

/// The reaper: closes the browsers idle for the idle timeout, until the
/// registry is empty.
fn reap_until_empty() {
    loop {
        let timeout = idle_timeout();
        std::thread::sleep(poll_interval(timeout));
        let (expired, done) = {
            let mut registry = registry();
            let expired = registry.take_expired(timeout);
            let done = registry.entries.is_empty();
            if done {
                registry.reaper_running = false;
            }
            (expired, done)
        };
        if !expired.is_empty() {
            info!(
                count = expired.len(),
                ?timeout,
                "closing reused browsers that no test has held for the idle timeout"
            );
            close_all(expired);
        }
        if done {
            return;
        }
    }
}

/// Closes browsers: asks each WebDriver server to end the session, which
/// closes the browser, then stops the driver and the virtual display this
/// crate started. It blocks, so it runs on threads of its own, never in an
/// async runtime.
fn close_all(entries: Vec<Entry>) {
    if entries.is_empty() {
        return;
    }
    crate::install_tls_provider();
    // A runtime of its own: this runs on a plain thread, often while the
    // program exits, never inside another runtime.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .inspect_err(|error| warn!(%error, "no runtime to end reused sessions with"))
        .ok();
    // reqwest builds its client inside a runtime.
    let client = runtime.as_ref().and_then(|runtime| {
        runtime
            .block_on(async { reqwest::Client::builder().timeout(CLOSE_TIMEOUT).build() })
            .inspect_err(|error| warn!(%error, "no HTTP client to end reused sessions with"))
            .ok()
    });
    for entry in entries {
        let url = format!(
            "{}/session/{}",
            entry.address.server_url.trim_end_matches('/'),
            entry.address.session_id
        );
        if let (Some(runtime), Some(client)) = (&runtime, &client) {
            if let Err(error) = runtime.block_on(async { client.delete(&url).send().await }) {
                debug!(%error, %url, "the WebDriver server did not answer the end of a reused session");
            }
        }
        if let Some(mut process) = entry.driver_process {
            process.kill();
        }
        drop(entry.virtual_display);
    }
}

/// Closes browsers on a thread of its own, without waiting.
fn close_in_background(entries: Vec<Entry>) {
    if entries.is_empty() {
        return;
    }
    if let Err(error) = std::thread::Builder::new()
        .name("sb-reuse-close".to_owned())
        .spawn(move || close_all(entries))
    {
        warn!(%error, "could not start a thread to close a reused browser");
    }
}

/// Closes the idle browsers of `registry` and waits for them; see
/// [`shutdown_reused_sessions`].
fn shutdown(registry: &Mutex<Registry>) -> usize {
    let idle = lock(registry).take_idle();
    let count = idle.len();
    if count > 0 {
        // The HTTP calls block; a thread of their own keeps them out of any
        // async runtime the caller is in.
        match std::thread::Builder::new()
            .name("sb-reuse-shutdown".to_owned())
            .spawn(move || close_all(idle))
        {
            Ok(handle) => {
                if handle.join().is_err() {
                    warn!("closing the reused browsers panicked");
                }
            }
            Err(error) => warn!(%error, "could not start a thread to close the reused browsers"),
        }
    }
    count
}

/// Closes every reused browser that no test holds, with the driver this crate
/// started for it, and returns how many it closed.
///
/// A browser a test still holds is left alone; once the test gives it back, the
/// reaper closes it after [`idle_timeout`]. The call returns when the browsers
/// are closed (each WebDriver server is given ten seconds to end its session).
/// It does the blocking work on a thread of its own, so async code may call it,
/// though [`drain_cleanups`](crate::common::shutdown::drain_cleanups), which
/// calls it, is the better fit there.
pub fn shutdown_reused_sessions() -> usize {
    shutdown(global())
}

/// Closes the reused browsers that no test holds, as
/// [`shutdown_reused_sessions`] does, for async code: the closing runs on a
/// thread of its own, and is waited for until `deadline`. Returns `false` if it
/// had not finished by then.
pub(crate) async fn close_idle_until(deadline: tokio::time::Instant) -> bool {
    let idle = registry().take_idle();
    if idle.is_empty() {
        return true;
    }
    let (done, finished) = tokio::sync::oneshot::channel();
    let spawned = std::thread::Builder::new()
        .name("sb-reuse-shutdown".to_owned())
        .spawn(move || {
            close_all(idle);
            let _ = done.send(());
        });
    if let Err(error) = spawned {
        warn!(%error, "could not start a thread to close the reused browsers");
        return false;
    }
    tokio::time::timeout_at(deadline, finished).await.is_ok()
}

/// A browser kept for reuse, as [`reused_sessions`] reports it.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct ReusedSession {
    /// The WebDriver server the session is on.
    pub server_url: String,
    /// The WebDriver session id.
    pub session_id: String,
    /// Whether a test holds the browser now.
    pub leased: bool,
}

/// The browsers this process keeps for reuse.
#[must_use]
pub fn reused_sessions() -> Vec<ReusedSession> {
    registry()
        .entries
        .iter()
        .map(|entry| ReusedSession {
            server_url: entry.address.server_url.clone(),
            session_id: entry.address.session_id.clone(),
            leased: entry.idle_since.is_none(),
        })
        .collect()
}

/// A session's hold on a reused browser. Dropping it gives the browser back.
#[derive(Debug)]
pub(crate) struct Lease {
    id: Option<u64>,
}

impl Lease {
    /// Ends the hold and closes the browser, which must not be used again:
    /// it crashed, or its session was ended.
    pub(crate) fn discard(mut self) {
        if let Some(id) = self.id.take() {
            let entry = registry().remove(id);
            close_in_background(entry.into_iter().collect());
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            registry().release(id);
        }
    }
}

/// Registers a browser that was just started, held by the session that
/// started it. The registry takes the driver process and the virtual display,
/// so they live as long as the browser and not as long as the session.
pub(crate) fn register(
    key: ReuseKey,
    address: SessionAddress,
    driver_process: Option<DriverProcess>,
    virtual_display: Option<XvfbProcess>,
) -> Lease {
    let mut registry = registry();
    let id = registry.insert_leased(key, address, driver_process, virtual_display);
    ensure_reaper(&mut registry);
    Lease { id: Some(id) }
}

/// A session for `config` on a reused browser, or on a new one that is then
/// kept for reuse.
pub(crate) async fn connect(config: BrowserConfig) -> Result<BrowserSession, SeleniumBaseError> {
    let key = ReuseKey::of(&config);
    while let Some((id, address)) = lease_idle(&key) {
        let lease = Lease { id: Some(id) };
        match attach(&config, &address).await {
            Ok(driver) => {
                debug!(session = %address.session_id, "reusing a browser");
                return BrowserSession::attached(&config, driver, lease);
            }
            Err(error) => {
                warn!(
                    %error,
                    session = %address.session_id,
                    "a browser kept for reuse cannot be used again; closing it"
                );
                lease.discard();
            }
        }
    }
    let mut session = BrowserSession::connect_new(config).await?;
    session.share(key);
    Ok(session)
}

/// Leases an idle browser for `key` from the registry of this process. The
/// registry is locked only for the call.
fn lease_idle(key: &ReuseKey) -> Option<(u64, SessionAddress)> {
    registry().lease(key)
}

/// A client attached to the session at `address`, after the browser was
/// cleaned for the next test.
async fn attach(
    config: &BrowserConfig,
    address: &SessionAddress,
) -> Result<WebDriver, SeleniumBaseError> {
    let timeout = config
        .remote_webdriver_timeout_seconds
        .map_or(DEFAULT_REQUEST_TIMEOUT, Duration::from_secs);
    let client = AttachClient::new(address, timeout)?;
    crate::install_tls_provider();
    let server_url = crate::browser::session::server_url_for_joins(&address.server_url);
    let driver = WebDriver::builder(server_url.as_str(), thirtyfour::Capabilities::new())
        .client(client)
        .request_timeout(timeout)
        .connect()
        .await?;
    if let Err(error) = reset(&driver, config).await {
        // Dropped as it is, the client would end the session; the caller
        // closes the browser instead.
        let _ = driver.leak();
        return Err(error);
    }
    Ok(driver)
}

/// Cleans a reused browser for the next test. See the
/// [module documentation](self#how-it-works).
async fn reset(driver: &WebDriver, config: &BrowserConfig) -> Result<(), SeleniumBaseError> {
    // An open dialog would make the commands below fail.
    let _ = driver.dismiss_alert().await;
    let windows = driver.windows().await?;
    if let Some((first, others)) = windows.split_first() {
        for window in others {
            driver.switch_to_window(window.clone()).await?;
            driver.close_window().await?;
        }
        driver.switch_to_window(first.clone()).await?;
    }
    // Every cookie of the browser through DevTools on Chromium; elsewhere, or
    // if DevTools refuses, WebDriver deletes those of the open page.
    let cleared = config.browser.is_chromium_based()
        && CdpClient::from_handle(driver.handle().clone())
            .clear_cookies()
            .await
            .is_ok();
    if !cleared {
        driver.delete_all_cookies().await?;
    }
    let _ = driver.execute(CLEAR_STORAGE, Vec::new()).await;
    driver.goto("about:blank").await?;
    Ok(())
}

/// A thirtyfour HTTP client that answers the new-session request itself, with
/// the session it attaches to, and sends every other request to the server.
///
/// thirtyfour has no public way to make a `WebDriver` for a session that is
/// already running; its builder takes a client, and a new session is just the
/// first request the client sees.
struct AttachClient {
    http: reqwest::Client,
    timeout: Duration,
    /// The `value` of the new-session answer: the session id and capabilities.
    session: Value,
}

impl AttachClient {
    fn new(address: &SessionAddress, timeout: Duration) -> Result<Self, SeleniumBaseError> {
        let http = http_client(timeout).map_err(|error| {
            SeleniumBaseError::unsupported(format!("cannot build an HTTP client: {error}"))
        })?;
        Ok(Self {
            http,
            timeout,
            session: json!({
                "sessionId": address.session_id,
                "capabilities": address.capabilities,
            }),
        })
    }
}

fn http_client(timeout: Duration) -> reqwest::Result<reqwest::Client> {
    crate::install_tls_provider();
    reqwest::Client::builder().timeout(timeout).build()
}

/// Whether `request` asks for a new session: a `POST` to `.../session`.
fn is_new_session<B>(request: &http::Request<B>) -> bool {
    request.method() == http::Method::POST
        && request
            .uri()
            .path()
            .trim_end_matches('/')
            .ends_with("/session")
}

#[async_trait::async_trait]
impl HttpClient for AttachClient {
    async fn send(
        &self,
        request: http::Request<Body<'_>>,
    ) -> WebDriverResult<http::Response<Bytes>> {
        if is_new_session(&request) {
            let body = json!({ "value": self.session }).to_string();
            return Ok(http::Response::new(Bytes::from(body)));
        }
        HttpClient::send(&self.http, request).await
    }

    async fn new(&self) -> Arc<dyn HttpClient> {
        // A client of its own, for a runtime other than the one this one was
        // made in.
        Arc::new(Self {
            http: http_client(self.timeout).unwrap_or_default(),
            timeout: self.timeout,
            session: self.session.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::config::{Browser, DriverMode};

    /// A clock that moves only when told to.
    struct FakeClock(Mutex<Instant>);

    impl FakeClock {
        fn new() -> Arc<Self> {
            Arc::new(Self(Mutex::new(Instant::now())))
        }

        fn advance(&self, by: Duration) {
            *self.0.lock().unwrap() += by;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            *self.0.lock().unwrap()
        }
    }

    fn address(session_id: &str) -> SessionAddress {
        SessionAddress {
            server_url: "http://127.0.0.1:9/".to_owned(),
            session_id: session_id.to_owned(),
            capabilities: json!({}),
        }
    }

    fn key(config: &BrowserConfig) -> ReuseKey {
        ReuseKey::of(config)
    }

    fn insert(registry: &mut Registry, config: &BrowserConfig, session_id: &str) -> u64 {
        registry.insert_leased(key(config), address(session_id), None, None)
    }

    #[test]
    fn the_same_configuration_gives_the_same_key() {
        let config = BrowserConfig::default().with_reuse_session(true);
        assert_eq!(key(&config), key(&config.clone()));
    }

    #[test]
    fn what_starts_the_browser_is_part_of_the_key() {
        let base = BrowserConfig::default().with_reuse_session(true);
        let different = [
            base.clone().with_browser(Browser::Firefox),
            base.clone().with_headless(!base.headless),
            base.clone().with_mode(DriverMode::Uc),
            base.clone().with_proxy("proxy.test:8080"),
            base.clone().with_user_agent("Agent/1.0"),
            base.clone().with_extension_dir("/tmp/extension"),
            base.clone().with_webdriver_url("http://grid.test:4444"),
            base.clone().with_window_size("1280,720".parse().unwrap()),
        ];
        for other in different {
            assert_ne!(key(&base), key(&other), "{other:?}");
        }
    }

    #[test]
    fn the_start_page_and_the_reuse_settings_are_not_part_of_the_key() {
        let base = BrowserConfig::default().with_reuse_session(true);
        assert_eq!(
            key(&base),
            key(&base.clone().with_start_page("https://example.com"))
        );
        assert_eq!(key(&base), key(&base.clone().with_reuse_session(false)));
    }

    #[test]
    fn the_run_settings_are_not_part_of_the_key() {
        let base = BrowserConfig::default().with_reuse_session(true);
        let demo = base.clone().with_run_settings(
            crate::api::run_settings::RunSettings::default().with_demo_mode(true),
        );
        assert_eq!(key(&base), key(&demo));
    }

    #[test]
    fn a_class_is_part_of_the_key() {
        let base = BrowserConfig::default().with_reuse_session(true);
        let login = base.clone().with_reuse_class_session("Login");
        let cart = base.clone().with_reuse_class_session("Cart");
        assert_ne!(key(&base), key(&login));
        assert_ne!(key(&login), key(&cart));
        assert_eq!(key(&login), key(&base.with_reuse_class_session("Login")));
    }

    #[test]
    fn the_module_of_a_test_comes_from_the_name_of_its_thread() {
        assert_eq!(
            test_module(Some("login::tests::valid_password")),
            "login::tests"
        );
        assert_eq!(test_module(Some("valid_password")), "");
        assert_eq!(test_module(None), "");
        let resolved = std::thread::Builder::new()
            .name("cart::tests::adds_an_item".to_owned())
            .spawn(|| ReuseClass::TestModule.resolve())
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(resolved, "cart::tests");
        assert_eq!(ReuseClass::Named("Login".to_owned()).resolve(), "Login");
    }

    #[test]
    fn tests_of_two_modules_get_two_keys() {
        let config = BrowserConfig::default().with_reuse_module_session(true);
        let key_on = |thread: &str| {
            let config = config.clone();
            std::thread::Builder::new()
                .name(thread.to_owned())
                .spawn(move || ReuseKey::of(&config))
                .unwrap()
                .join()
                .unwrap()
        };
        assert_eq!(key_on("login::tests::a"), key_on("login::tests::b"));
        assert_ne!(key_on("login::tests::a"), key_on("cart::tests::a"));
    }

    #[test]
    fn a_held_browser_is_not_leased_until_it_is_given_back() {
        let mut registry = Registry::new(FakeClock::new());
        let config = BrowserConfig::default();
        let id = insert(&mut registry, &config, "s1");
        assert!(
            registry.lease(&key(&config)).is_none(),
            "the first test holds it"
        );
        assert!(registry.release(id));
        assert_eq!(registry.lease(&key(&config)), Some((id, address("s1"))));
        assert!(
            registry.lease(&key(&config)).is_none(),
            "one test at a time"
        );
    }

    #[test]
    fn a_browser_is_leased_only_for_its_own_configuration() {
        let mut registry = Registry::new(FakeClock::new());
        let chrome = BrowserConfig::default();
        let id = insert(&mut registry, &chrome, "s1");
        registry.release(id);
        let firefox = BrowserConfig::default().with_browser(Browser::Firefox);
        assert!(registry.lease(&key(&firefox)).is_none());
        assert!(registry.lease(&key(&chrome)).is_some());
    }

    #[test]
    fn the_browser_given_back_last_is_leased_first() {
        let clock = FakeClock::new();
        let mut registry = Registry::new(clock.clone());
        let config = BrowserConfig::default();
        let older = insert(&mut registry, &config, "older");
        let newer = insert(&mut registry, &config, "newer");
        registry.release(older);
        clock.advance(Duration::from_secs(1));
        registry.release(newer);
        assert_eq!(registry.lease(&key(&config)).map(|(id, _)| id), Some(newer));
    }

    #[test]
    fn the_reaper_takes_a_browser_once_it_has_been_idle_for_the_timeout() {
        let clock = FakeClock::new();
        let mut registry = Registry::new(clock.clone());
        let config = BrowserConfig::default();
        let timeout = Duration::from_secs(5);
        let id = insert(&mut registry, &config, "s1");
        registry.release(id);

        clock.advance(timeout - Duration::from_millis(1));
        assert!(registry.take_expired(timeout).is_empty());
        clock.advance(Duration::from_millis(1));
        let taken = registry.take_expired(timeout);
        assert_eq!(taken.iter().map(|e| e.id).collect::<Vec<_>>(), [id]);
        assert!(registry.entries.is_empty());
        assert!(registry.lease(&key(&config)).is_none());
    }

    #[test]
    fn the_reaper_never_takes_a_held_browser() {
        let clock = FakeClock::new();
        let mut registry = Registry::new(clock.clone());
        insert(&mut registry, &BrowserConfig::default(), "s1");
        clock.advance(Duration::from_secs(3600));
        assert!(registry.take_expired(Duration::from_secs(5)).is_empty());
        assert_eq!(registry.entries.len(), 1);
    }

    #[test]
    fn giving_a_browser_back_starts_its_idle_time_again() {
        let clock = FakeClock::new();
        let mut registry = Registry::new(clock.clone());
        let config = BrowserConfig::default();
        let timeout = Duration::from_secs(5);
        let id = insert(&mut registry, &config, "s1");
        registry.release(id);
        clock.advance(Duration::from_secs(4));
        registry.lease(&key(&config)).unwrap();
        clock.advance(Duration::from_secs(4));
        registry.release(id);
        clock.advance(Duration::from_secs(4));
        assert!(
            registry.take_expired(timeout).is_empty(),
            "idle for 4 s since it was given back"
        );
    }

    #[test]
    fn shutdown_takes_every_idle_browser_and_leaves_the_held_ones() {
        let mut registry = Registry::new(FakeClock::new());
        let config = BrowserConfig::default();
        let idle = insert(&mut registry, &config, "idle");
        let held = insert(&mut registry, &config, "held");
        registry.release(idle);
        let taken = registry.take_idle();
        assert_eq!(taken.iter().map(|e| e.id).collect::<Vec<_>>(), [idle]);
        assert_eq!(
            registry.entries.iter().map(|e| e.id).collect::<Vec<_>>(),
            [held]
        );
    }

    #[test]
    fn a_removed_browser_cannot_be_given_back() {
        let mut registry = Registry::new(FakeClock::new());
        let id = insert(&mut registry, &BrowserConfig::default(), "s1");
        assert!(registry.remove(id).is_some());
        assert!(!registry.release(id));
        assert!(registry.remove(id).is_none());
    }

    #[test]
    fn the_idle_timeout_is_read_in_whole_seconds() {
        assert_eq!(parse_idle_timeout(None).unwrap(), DEFAULT_IDLE_TIMEOUT);
        assert_eq!(parse_idle_timeout(Some(" ")).unwrap(), DEFAULT_IDLE_TIMEOUT);
        assert_eq!(
            parse_idle_timeout(Some("12")).unwrap(),
            Duration::from_secs(12)
        );
        assert_eq!(parse_idle_timeout(Some("0")).unwrap(), Duration::ZERO);
        let error = parse_idle_timeout(Some("1.5")).unwrap_err();
        assert!(error.to_string().contains(IDLE_TIMEOUT_VAR), "{error}");
    }

    #[test]
    fn the_reaper_looks_often_but_not_constantly() {
        assert_eq!(
            poll_interval(Duration::from_secs(5)),
            Duration::from_millis(500)
        );
        assert_eq!(
            poll_interval(Duration::from_secs(1)),
            Duration::from_millis(200)
        );
        assert_eq!(poll_interval(Duration::ZERO), Duration::from_millis(20));
    }

    #[test]
    fn only_a_post_to_the_session_collection_is_a_new_session() {
        let request = |method: &str, uri: &str| {
            http::Request::builder()
                .method(method)
                .uri(uri)
                .body(())
                .unwrap()
        };
        assert!(is_new_session(&request(
            "POST",
            "http://127.0.0.1:9515/session"
        )));
        assert!(is_new_session(&request(
            "POST",
            "http://grid.test/wd/hub/session/"
        )));
        assert!(!is_new_session(&request(
            "DELETE",
            "http://127.0.0.1:9515/session/abc"
        )));
        assert!(!is_new_session(&request(
            "POST",
            "http://127.0.0.1:9515/session/abc/url"
        )));
        assert!(!is_new_session(&request(
            "GET",
            "http://127.0.0.1:9515/session"
        )));
    }
}

/// `BaseCase` and `BrowserSession` with reuse on, against the mock WebDriver
/// of `webdriver_support`.
#[cfg(test)]
mod tests_with_a_driver {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;
    use crate::api::base_case::webdriver_support::{MockWebDriver, Reply, Seen};
    use crate::browser::config::Browser;
    use crate::BaseCase;

    fn method_and_path(request: &Seen) -> (&str, &str) {
        (request.method.as_str(), request.path.as_str())
    }

    /// A mock browser with the windows `windows`.
    fn mock_with_windows(windows: &'static [&'static str]) -> MockWebDriver {
        MockWebDriver::start(json!({ "browserName": "chrome" }), move |request| {
            match method_and_path(request) {
                ("GET", "/window/handles") => Reply::Value(json!(windows)),
                _ => Reply::Value(Value::Null),
            }
        })
    }

    /// A configuration that reuses browsers of `driver`. The server's address
    /// is part of the key, so the tests do not share browsers with each other.
    fn reusing(driver: &MockWebDriver) -> BrowserConfig {
        let mut config = BrowserConfig::default()
            .with_webdriver_url(driver.url())
            .with_reuse_session(true);
        config.auto_start_driver = false;
        config
    }

    fn new_session_requests(driver: &MockWebDriver) -> usize {
        driver
            .requests("/session")
            .iter()
            .filter(|request| request.method == "POST")
            .count()
    }

    fn ended_sessions(driver: &MockWebDriver) -> usize {
        driver
            .requests("")
            .iter()
            .filter(|request| request.method == "DELETE")
            .count()
    }

    /// Runs `work` on a Tokio runtime of its own on another thread, as a second
    /// `#[tokio::test]` would.
    fn on_another_runtime<T, F, Fut>(work: F) -> T
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = T>,
        T: Send + 'static,
    {
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(work())
        })
        .join()
        .unwrap()
    }

    #[tokio::test]
    async fn a_second_case_on_another_runtime_attaches_to_the_first_browser() {
        let driver = mock_with_windows(&["w1"]);
        let config = reusing(&driver);

        let first_config = config.clone();
        let first_id = on_another_runtime(move || async move {
            let mut first = BaseCase::new(first_config).await.unwrap();
            let id = first.session_id();
            first.quit().await.unwrap();
            id
        });
        let before = driver.all_requests().len();

        let mut second = BaseCase::new(config).await.unwrap();
        assert_eq!(second.session_id(), first_id);
        second.quit().await.unwrap();

        assert_eq!(
            new_session_requests(&driver),
            1,
            "only the first case starts a browser"
        );
        assert_eq!(
            ended_sessions(&driver),
            0,
            "quitting gives the browser back"
        );
        let sent: Vec<(String, String)> = driver.all_requests()[before..]
            .iter()
            .map(|r| (r.method.clone(), r.path.clone()))
            .collect();
        let at = |method: &str, path: &str| {
            sent.iter()
                .position(|(m, p)| m == method && p == path)
                .unwrap_or_else(|| panic!("{method} {path} was not sent: {sent:?}"))
        };
        let (dismiss, windows, cookies, storage, blank) = (
            at("POST", "/alert/dismiss"),
            at("GET", "/window/handles"),
            at("POST", "/goog/cdp/execute"),
            at("POST", "/execute/sync"),
            at("POST", "/url"),
        );
        assert!(
            dismiss < windows && windows < cookies && cookies < storage && storage < blank,
            "{sent:?}"
        );
        let requests = driver.all_requests();
        let cdp = &requests[before + cookies];
        assert_eq!(cdp.body["cmd"], "Network.clearBrowserCookies");
        assert_eq!(requests[before + blank].body["url"], "about:blank");
    }

    #[tokio::test]
    async fn every_window_but_the_first_is_closed() {
        let driver = mock_with_windows(&["w1", "w2", "w3"]);
        let config = reusing(&driver);
        BaseCase::new(config.clone())
            .await
            .unwrap()
            .quit()
            .await
            .unwrap();
        let before = driver.all_requests().len();

        let mut second = BaseCase::new(config).await.unwrap();
        second.quit().await.unwrap();

        let windows: Vec<String> = driver.all_requests()[before..]
            .iter()
            .filter(|r| r.path == "/window" && r.method != "GET")
            .map(|r| match r.method.as_str() {
                "DELETE" => "close".to_owned(),
                _ => format!("to {}", r.body["handle"].as_str().unwrap_or_default()),
            })
            .collect();
        assert_eq!(windows, ["to w2", "close", "to w3", "close", "to w1"]);
    }

    #[tokio::test]
    async fn a_firefox_session_deletes_cookies_through_webdriver() {
        let driver = MockWebDriver::start(json!({ "browserName": "firefox" }), |request| {
            match method_and_path(request) {
                ("GET", "/window/handles") => Reply::Value(json!(["w1"])),
                _ => Reply::Value(Value::Null),
            }
        });
        let config = reusing(&driver).with_browser(Browser::Firefox);
        BaseCase::new(config.clone())
            .await
            .unwrap()
            .quit()
            .await
            .unwrap();
        BaseCase::new(config).await.unwrap().quit().await.unwrap();

        assert_eq!(new_session_requests(&driver), 1);
        assert!(driver
            .requests("/cookie")
            .iter()
            .any(|r| r.method == "DELETE"));
        assert!(driver.requests("/goog/cdp/execute").is_empty());
    }

    #[tokio::test]
    async fn two_cases_at_once_get_two_browsers_which_are_both_kept() {
        let driver = mock_with_windows(&["w1"]);
        let config = reusing(&driver);

        let mut first = BaseCase::new(config.clone()).await.unwrap();
        let mut second = BaseCase::new(config.clone()).await.unwrap();
        assert_eq!(
            new_session_requests(&driver),
            2,
            "the first browser is held"
        );
        first.quit().await.unwrap();
        second.quit().await.unwrap();

        let mut third = BaseCase::new(config.clone()).await.unwrap();
        let mut fourth = BaseCase::new(config).await.unwrap();
        assert_eq!(new_session_requests(&driver), 2, "both browsers are reused");
        third.quit().await.unwrap();
        fourth.quit().await.unwrap();
        assert_eq!(ended_sessions(&driver), 0);
    }

    #[tokio::test]
    async fn a_browser_that_no_longer_answers_is_closed_and_replaced() {
        let gone = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&gone);
        let driver =
            MockWebDriver::start(json!({}), move |request| match method_and_path(request) {
                ("GET", "/window/handles") if flag.load(Ordering::SeqCst) => {
                    Reply::Error(404, "invalid session id")
                }
                ("GET", "/window/handles") => Reply::Value(json!(["w1"])),
                _ => Reply::Value(Value::Null),
            });
        let config = reusing(&driver);
        BaseCase::new(config.clone())
            .await
            .unwrap()
            .quit()
            .await
            .unwrap();
        gone.store(true, Ordering::SeqCst);

        let mut second = BaseCase::new(config).await.unwrap();
        assert_eq!(
            new_session_requests(&driver),
            2,
            "a new browser replaces it"
        );
        // The broken one is closed in the background.
        let deadline = Instant::now() + Duration::from_secs(10);
        while ended_sessions(&driver) == 0 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(ended_sessions(&driver), 1);
        second.quit().await.unwrap();
    }

    #[tokio::test]
    async fn a_case_that_is_dropped_gives_its_browser_back() {
        let driver = mock_with_windows(&["w1"]);
        let config = reusing(&driver);
        drop(BaseCase::new(config.clone()).await.unwrap());
        BaseCase::new(config).await.unwrap().quit().await.unwrap();
        assert_eq!(new_session_requests(&driver), 1);
        assert_eq!(ended_sessions(&driver), 0);
    }

    #[tokio::test]
    async fn without_reuse_every_case_starts_and_ends_its_own_session() {
        let driver = mock_with_windows(&["w1"]);
        let config = reusing(&driver).with_reuse_session(false);
        BaseCase::new(config.clone())
            .await
            .unwrap()
            .quit()
            .await
            .unwrap();
        BaseCase::new(config).await.unwrap().quit().await.unwrap();
        assert_eq!(new_session_requests(&driver), 2);
        assert_eq!(ended_sessions(&driver), 2);
    }

    #[tokio::test]
    async fn an_extra_driver_is_never_shared() {
        let driver = mock_with_windows(&["w1"]);
        let config = reusing(&driver);
        let mut sb = BaseCase::new(config.clone()).await.unwrap();
        sb.get_new_driver(None).await.unwrap();
        sb.quit().await.unwrap();
        assert_eq!(
            ended_sessions(&driver),
            1,
            "the extra driver's own session ends"
        );
        drop(sb);
        // The first browser was parked, held, and is given back when the case
        // is dropped; it is reused next.
        BaseCase::new(config).await.unwrap().quit().await.unwrap();
        assert_eq!(new_session_requests(&driver), 2);
    }

    #[tokio::test]
    async fn shutdown_ends_the_idle_sessions_and_forgets_them() {
        let driver = mock_with_windows(&["w1"]);
        let registry = Mutex::new(Registry::new(Arc::new(SystemClock)));
        let config = BrowserConfig::default().with_webdriver_url(driver.url());
        {
            let mut registry = registry.lock().unwrap();
            let address = SessionAddress {
                server_url: format!("{}/", driver.url()),
                session_id: "mock".to_owned(),
                capabilities: json!({}),
            };
            let idle = registry.insert_leased(ReuseKey::of(&config), address.clone(), None, None);
            registry.release(idle);
            registry.insert_leased(ReuseKey::of(&config), address, None, None);
        }

        let closed = tokio::task::spawn_blocking(move || {
            let closed = shutdown(&registry);
            (closed, registry.lock().unwrap().entries.len())
        })
        .await
        .unwrap();

        assert_eq!(closed, (1, 1), "the held session stays");
        assert_eq!(ended_sessions(&driver), 1);
    }
}
