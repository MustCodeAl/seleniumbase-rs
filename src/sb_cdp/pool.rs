//! A pool of Chrome browsers for many concurrent workers.
//!
//! [`BrowserPool::acquire`] hands a worker a [`Lease`]: an isolated
//! [`BrowserContext`] with one tab, inside one of at most `max_browsers`
//! Chrome processes. Contexts share nothing, so workers cannot see each
//! other's cookies, and releasing a lease discards its context. Chrome
//! processes are launched on demand, reused, and replaced after
//! `retire_after` leases so a long-running pool does not accumulate leaked
//! memory.
//!
//! Waiting workers queue fairly. A pool never holds more than
//! `max_browsers * contexts_per_browser` leases at once, and
//! [`acquire`](BrowserPool::acquire) gives up after `acquire_timeout`.
//!
//! The pool's [`SessionStore`] carries logins between workers, in memory only:
//! one worker signs in and calls [`Lease::save_session`]; any other calls
//! [`Lease::load_session`] and starts signed in.
//!
//! # Examples
//!
//! ```no_run
//! use seleniumbase_rs::sb_cdp::{BrowserPool, LaunchOptions, PoolOptions};
//!
//! # async fn demo() -> Result<(), seleniumbase_rs::SeleniumBaseError> {
//! let pool = BrowserPool::new(
//!     LaunchOptions::builder().headless(true).build()?,
//!     PoolOptions::builder().max_browsers(2).contexts_per_browser(4).build()?,
//! );
//!
//! let worker = pool.acquire().await?;
//! worker.page().goto("https://example.com/login").await?;
//! // ... sign in ...
//! worker.save_session("alice").await?;
//! worker.release().await;
//!
//! let another = pool.acquire().await?;
//! another.page().goto("https://example.com").await?;
//! another.load_session("alice").await?; // signed in, without logging in again
//! another.release().await;
//!
//! pool.close().await;
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::{Mutex, Notify, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

use super::{Browser, BrowserContext, ContextOptions, LaunchOptions, Page, Session, SessionStore};
use crate::error::SeleniumBaseError;

type LaunchFuture = Pin<Box<dyn Future<Output = Result<Browser, SeleniumBaseError>> + Send>>;
type Launcher = Arc<dyn Fn() -> LaunchFuture + Send + Sync>;

/// How a [`BrowserPool`] is sized and when it gives up waiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolOptions {
    max_browsers: usize,
    contexts_per_browser: usize,
    retire_after: usize,
    acquire_timeout: Duration,
}

impl PoolOptions {
    /// A builder holding the defaults: 2 browsers, 4 contexts each, retired
    /// after 100 leases, with a 60 second wait.
    #[must_use]
    pub fn builder() -> PoolOptionsBuilder {
        PoolOptionsBuilder::default()
    }

    /// Most Chrome processes at once.
    #[must_use]
    pub fn max_browsers(&self) -> usize {
        self.max_browsers
    }

    /// Most simultaneous leases on one Chrome process.
    #[must_use]
    pub fn contexts_per_browser(&self) -> usize {
        self.contexts_per_browser
    }

    /// Most leases the pool will ever hold at once.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.max_browsers * self.contexts_per_browser
    }
}

/// Builds [`PoolOptions`], checking the values in [`build`](Self::build).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolOptionsBuilder {
    max_browsers: usize,
    contexts_per_browser: usize,
    retire_after: usize,
    acquire_timeout: Duration,
}

impl Default for PoolOptionsBuilder {
    fn default() -> Self {
        Self {
            max_browsers: 2,
            contexts_per_browser: 4,
            retire_after: 100,
            acquire_timeout: Duration::from_secs(60),
        }
    }
}

impl PoolOptionsBuilder {
    /// Most Chrome processes at once.
    #[must_use]
    pub fn max_browsers(mut self, count: usize) -> Self {
        self.max_browsers = count;
        self
    }

    /// Most simultaneous leases on one Chrome process.
    #[must_use]
    pub fn contexts_per_browser(mut self, count: usize) -> Self {
        self.contexts_per_browser = count;
        self
    }

    /// Replaces a Chrome process once it has served this many leases.
    #[must_use]
    pub fn retire_after(mut self, leases: usize) -> Self {
        self.retire_after = leases;
        self
    }

    /// How long [`acquire`](BrowserPool::acquire) waits for a free slot.
    #[must_use]
    pub fn acquire_timeout(mut self, timeout: Duration) -> Self {
        self.acquire_timeout = timeout;
        self
    }

    /// Checks the values and builds the options.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::InvalidConfig`] if a count is zero or above
    /// 256, or the timeout is zero.
    pub fn build(self) -> Result<PoolOptions, SeleniumBaseError> {
        for (name, value) in [
            ("max_browsers", self.max_browsers),
            ("contexts_per_browser", self.contexts_per_browser),
            ("retire_after", self.retire_after),
        ] {
            if !(1..=256).contains(&value) {
                return Err(SeleniumBaseError::InvalidConfig(format!(
                    "{name} must be between 1 and 256, got {value}"
                )));
            }
        }
        if self.acquire_timeout.is_zero() {
            return Err(SeleniumBaseError::InvalidConfig(
                "acquire_timeout must be greater than zero".to_owned(),
            ));
        }
        Ok(PoolOptions {
            max_browsers: self.max_browsers,
            contexts_per_browser: self.contexts_per_browser,
            retire_after: self.retire_after,
            acquire_timeout: self.acquire_timeout,
        })
    }
}

/// A snapshot of a pool's activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolStats {
    /// Chrome processes currently running.
    pub browsers: usize,
    /// Leases currently held.
    pub leased: usize,
    /// Chrome processes launched over the pool's life.
    pub launched: usize,
    /// Chrome processes retired or found dead and replaced.
    pub retired: usize,
    /// Sessions in the shared store.
    pub sessions: usize,
}

struct Slot {
    id: u64,
    browser: Browser,
    leases: usize,
    served: usize,
    /// Takes no new leases; closed once its last lease ends.
    retiring: bool,
}

#[derive(Default)]
struct State {
    slots: Vec<Slot>,
    next_id: u64,
}

struct Shared {
    launcher: Launcher,
    options: PoolOptions,
    permits: Arc<Semaphore>,
    state: Mutex<State>,
    freed: Notify,
    sessions: SessionStore,
    closed: AtomicBool,
    launched: AtomicUsize,
    retired: AtomicUsize,
}

fn closed_error() -> SeleniumBaseError {
    SeleniumBaseError::browser_disconnected("the browser pool is closed")
}

impl Shared {
    /// A browser with room for one more lease, launching one if the pool is
    /// below its limit. `None` means every browser is full.
    async fn checkout(&self) -> Result<Option<(u64, Browser)>, SeleniumBaseError> {
        let mut state = self.state.lock().await;

        // A browser that has died and has no leases left is just a leak.
        let before = state.slots.len();
        state
            .slots
            .retain(|slot| slot.leases > 0 || slot.browser.is_connected());
        self.retired
            .fetch_add(before - state.slots.len(), Ordering::Relaxed);

        let cap = self.options.contexts_per_browser;
        let retire_after = self.options.retire_after;
        if let Some(slot) = state
            .slots
            .iter_mut()
            .filter(|slot| !slot.retiring && slot.browser.is_connected() && slot.leases < cap)
            .min_by_key(|slot| slot.leases)
        {
            slot.leases += 1;
            slot.served += 1;
            slot.retiring = slot.served >= retire_after;
            return Ok(Some((slot.id, slot.browser.clone())));
        }

        if state.slots.len() < self.options.max_browsers {
            let browser = (self.launcher)().await?;
            self.launched.fetch_add(1, Ordering::Relaxed);
            let id = state.next_id;
            state.next_id += 1;
            state.slots.push(Slot {
                id,
                browser: browser.clone(),
                leases: 1,
                served: 1,
                retiring: retire_after <= 1,
            });
            return Ok(Some((id, browser)));
        }
        Ok(None)
    }

    /// Ends one lease on a slot, closing the browser if it is spent.
    async fn give_back(&self, slot_id: u64) {
        let spent = {
            let mut state = self.state.lock().await;
            let position = state.slots.iter().position(|slot| slot.id == slot_id);
            position.and_then(|position| {
                let slot = &mut state.slots[position];
                slot.leases = slot.leases.saturating_sub(1);
                let done = slot.leases == 0 && (slot.retiring || !slot.browser.is_connected());
                done.then(|| state.slots.remove(position).browser)
            })
        };
        if let Some(browser) = spent {
            self.retired.fetch_add(1, Ordering::Relaxed);
            // The browser is being thrown away; a failure to close is moot.
            let _ = browser.close().await;
        }
        self.freed.notify_one();
    }
}

/// A pool of Chrome browsers; see the [module docs](crate::sb_cdp::pool).
///
/// Cloning gives another handle to the same pool.
#[derive(Clone)]
pub struct BrowserPool {
    shared: Arc<Shared>,
}

impl fmt::Debug for BrowserPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrowserPool")
            .field("options", &self.shared.options)
            .field("closed", &self.shared.closed.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl BrowserPool {
    /// A pool that launches Chrome with `launch`. Nothing starts until the
    /// first [`acquire`](Self::acquire).
    #[must_use]
    pub fn new(launch: LaunchOptions, options: PoolOptions) -> Self {
        Self::with_launcher(options, move || {
            let launch = launch.clone();
            async move { Browser::launch(launch).await }
        })
    }

    /// A pool that gets its browsers from `launcher`.
    ///
    /// Use it to connect to existing browsers, to launch with per-browser
    /// options, or, with the `test-util` feature, to hand out mocked browsers.
    #[must_use]
    pub fn with_launcher<F, Fut>(options: PoolOptions, launcher: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Browser, SeleniumBaseError>> + Send + 'static,
    {
        Self {
            shared: Arc::new(Shared {
                launcher: Arc::new(move || Box::pin(launcher())),
                options,
                permits: Arc::new(Semaphore::new(options.capacity())),
                state: Mutex::new(State::default()),
                freed: Notify::new(),
                sessions: SessionStore::new(),
                closed: AtomicBool::new(false),
                launched: AtomicUsize::new(0),
                retired: AtomicUsize::new(0),
            }),
        }
    }

    /// The sessions shared by every lease.
    #[must_use]
    pub fn sessions(&self) -> &SessionStore {
        &self.shared.sessions
    }

    /// Takes a free slot, waiting its turn if the pool is full.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::WaitTimeout`] if no slot frees up in time,
    /// [`SeleniumBaseError::BrowserDisconnected`] if the pool is closed, and
    /// any error from launching a browser or opening its context.
    pub async fn acquire(&self) -> Result<Lease, SeleniumBaseError> {
        self.acquire_with(ContextOptions::default()).await
    }

    /// Like [`acquire`](Self::acquire), but the lease's context is set up as
    /// `context` describes. Use it to give one worker its own proxy while the
    /// others go direct.
    ///
    /// # Errors
    ///
    /// See [`acquire`](Self::acquire).
    pub async fn acquire_with(&self, context: ContextOptions) -> Result<Lease, SeleniumBaseError> {
        let shared = &self.shared;
        let timeout = shared.options.acquire_timeout;
        let deadline = Instant::now() + timeout;
        let timed_out =
            || SeleniumBaseError::wait_timeout("a free browser in the pool", Some(timeout));

        let permit = tokio::time::timeout(timeout, Arc::clone(&shared.permits).acquire_owned())
            .await
            .ok()
            .ok_or_else(timed_out)?
            .ok()
            .ok_or_else(closed_error)?;

        let (slot, browser) = loop {
            if shared.closed.load(Ordering::Acquire) {
                return Err(closed_error());
            }
            if let Some(found) = shared.checkout().await? {
                break found;
            }
            // Every browser is full or retiring: wait for a lease to end.
            tokio::time::timeout_at(deadline, shared.freed.notified())
                .await
                .ok()
                .ok_or_else(timed_out)?;
        };

        let opened = async {
            let context = browser.new_context_with(context).await?;
            let page = context.new_page(None::<&str>).await?;
            Ok::<_, SeleniumBaseError>((context, page))
        }
        .await;
        match opened {
            Ok((context, page)) => Ok(Lease {
                shared: Arc::clone(shared),
                slot,
                context,
                page,
                permit: Some(permit),
                released: false,
            }),
            Err(error) => {
                shared.give_back(slot).await;
                drop(permit);
                Err(error)
            }
        }
    }

    /// A snapshot of the pool's activity.
    pub async fn stats(&self) -> PoolStats {
        let state = self.shared.state.lock().await;
        PoolStats {
            browsers: state.slots.len(),
            leased: state.slots.iter().map(|slot| slot.leases).sum(),
            launched: self.shared.launched.load(Ordering::Relaxed),
            retired: self.shared.retired.load(Ordering::Relaxed),
            sessions: self.shared.sessions.len(),
        }
    }

    /// Closes every browser and refuses further leases. Workers waiting in
    /// [`acquire`](Self::acquire) get an error; leases already held keep
    /// working until their browser goes away.
    pub async fn close(&self) {
        self.shared.closed.store(true, Ordering::Release);
        self.shared.permits.close();
        let slots = std::mem::take(&mut self.shared.state.lock().await.slots);
        for slot in slots {
            // Shutting down; a browser that will not close is on its own.
            let _ = slot.browser.close().await;
        }
        self.shared.freed.notify_waiters();
    }
}

/// One worker's isolated browser context and tab, from a [`BrowserPool`].
///
/// Release it with [`release`](Self::release). Dropping it also releases it,
/// but in the background, so the slot may take a moment to free.
pub struct Lease {
    shared: Arc<Shared>,
    slot: u64,
    context: BrowserContext,
    page: Page,
    permit: Option<OwnedSemaphorePermit>,
    released: bool,
}

impl fmt::Debug for Lease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lease")
            .field("context", &self.context.id())
            .field("page", &self.page.id())
            .finish_non_exhaustive()
    }
}

impl Lease {
    /// The tab the lease opened with.
    #[must_use]
    pub fn page(&self) -> &Page {
        &self.page
    }

    /// The isolated context the lease owns.
    #[must_use]
    pub fn context(&self) -> &BrowserContext {
        &self.context
    }

    /// The Chrome process the context lives in. Other leases may be using it.
    #[must_use]
    pub fn browser(&self) -> &Browser {
        self.context.browser()
    }

    /// The pool's shared sessions.
    #[must_use]
    pub fn sessions(&self) -> &SessionStore {
        &self.shared.sessions
    }

    /// Opens another tab in the lease's context, optionally at `url`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the tab cannot be created.
    pub async fn new_page(&self, url: Option<impl AsRef<str>>) -> Result<Page, SeleniumBaseError> {
        self.context.new_page(url).await
    }

    /// Saves the context's cookies, and the local storage of the current
    /// page's origin, in the pool's store as `name`.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser cannot be read.
    pub async fn save_session(&self, name: impl Into<String>) -> Result<(), SeleniumBaseError> {
        let mut session = Session::new(self.page.cookies().all().await?);
        let origin: String = self.page.evaluate_as("location.origin").await?;
        if origin.starts_with("http") {
            let entries = self.page.local_storage().entries().await?;
            if !entries.is_empty() {
                session = session.with_local_storage(origin, entries);
            }
        }
        self.shared.sessions.save(name, session);
        Ok(())
    }

    /// Loads the session saved as `name` into the context: its cookies, and its
    /// local storage if the current page is on the origin it was captured from.
    /// Cookies that have expired are skipped.
    ///
    /// Returns `false`, changing nothing, if there is no such session.
    ///
    /// # Errors
    ///
    /// Returns [`SeleniumBaseError::CdpDriver`] if the browser refuses.
    pub async fn load_session(&self, name: &str) -> Result<bool, SeleniumBaseError> {
        let Some(session) = self.shared.sessions.load(name) else {
            return Ok(false);
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_secs_f64());
        let session = session.without_expired(now);

        self.page.cookies().set(session.cookies()).await?;
        if let Some(origin) = session.origin() {
            let here: String = self.page.evaluate_as("location.origin").await?;
            if here == origin {
                let storage = self.page.local_storage();
                for (key, value) in session.local_storage() {
                    storage.set(key, value).await?;
                }
            }
        }
        Ok(true)
    }

    /// Discards the context, with its cookies and tabs, and frees the slot.
    pub async fn release(mut self) {
        self.released = true;
        // The context is being discarded; if it is already gone, so much the better.
        let _ = self.context.dispose().await;
        self.shared.give_back(self.slot).await;
        drop(self.permit.take());
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        let shared = Arc::clone(&self.shared);
        let context = self.context.clone();
        let slot = self.slot;
        let permit = self.permit.take();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = context.dispose().await;
                shared.give_back(slot).await;
                drop(permit);
            });
        }
    }
}
