//! The scripted world the scraper tests run in: a launcher that hands out mocked browsers, a site
//! that answers them, a clock the test moves, and the sinks that record what the scraper did.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::sb_cdp::sync::locked;
use crate::sb_cdp::{Browser, LaunchOptions, MockCtrl};
use crate::scrape::egress::geo::CountryCode;
use crate::scrape::error::ScrapeError;
use crate::scrape::ledger::events::CollectingEventSink;
use crate::scrape::ledger::state::InMemoryStateStore;
use crate::scrape::runtime::browser_scraper::{BrowserLauncher, BrowserScraperBuilder};
use crate::scrape::runtime::cdp::testkit::{Served, Site};

/// The time every test starts at, in seconds since the Unix epoch.
pub(super) const NOW: u64 = 1_800_000_000;

/// The product string a mocked Chrome reports.
pub(super) const CHROME_155: &str = "Chrome/155.0.8059.40";

/// How long a solve is waited for in a test that expects it to fail.
pub(super) const BRIEF_SOLVE: Duration = Duration::from_millis(80);

/// The seed of the first profile in the tests.
pub(super) const SEED: u64 = 7;

pub(super) const TURNSTILE: &str =
    include_str!("../../../../../tests/fixtures/challenge/cloudflare_turnstile_challenge.html");
pub(super) const LEGACY_JS: &str =
    include_str!("../../../../../tests/fixtures/challenge/cloudflare_legacy_js.html");
pub(super) const BLOCKED: &str =
    include_str!("../../../../../tests/fixtures/challenge/cloudflare_blocked.html");
pub(super) const NORMAL: &str =
    include_str!("../../../../../tests/fixtures/challenge/cloudflare_normal_page.html");

/// A browser the launcher handed out.
#[derive(Clone)]
pub(super) struct Launched {
    pub(super) browser: Browser,
    pub(super) mock: MockCtrl,
    pub(super) options: LaunchOptions,
}

/// What a test does to the mock of a browser as it is launched: the index of the launch and the mock.
type LaunchHook = Box<dyn Fn(usize, &MockCtrl) + Send + Sync>;

/// Hands out mocked browsers that all answer like one site.
pub(super) struct TestLauncher {
    site: Site,
    product: &'static str,
    launched: Mutex<Vec<Launched>>,
    hook: Mutex<Option<LaunchHook>>,
}

impl TestLauncher {
    /// Runs `hook` on the mock of every browser launched from now on, before it is handed out.
    pub(super) fn on_launch(&self, hook: impl Fn(usize, &MockCtrl) + Send + Sync + 'static) {
        *locked(&self.hook) = Some(Box::new(hook));
    }

    pub(super) fn count(&self) -> usize {
        locked(&self.launched).len()
    }

    /// The `index`th browser launched, oldest first.
    pub(super) fn nth(&self, index: usize) -> Launched {
        locked(&self.launched)
            .get(index)
            .cloned()
            .unwrap_or_else(|| panic!("browser #{index} was never launched"))
    }

    /// The browser launched last.
    pub(super) fn last(&self) -> Launched {
        self.nth(self.count() - 1)
    }

    /// Every protocol command any browser received.
    pub(super) fn all_methods(&self) -> Vec<String> {
        locked(&self.launched)
            .iter()
            .flat_map(|launched| launched.mock.calls())
            .map(|call| call.method)
            .collect()
    }
}

#[async_trait]
impl BrowserLauncher for TestLauncher {
    async fn launch(&self, options: LaunchOptions) -> Result<Browser, ScrapeError> {
        let (browser, mock) = Browser::new_mocked();
        mock.reply("Browser.getVersion", json!({ "product": self.product }));
        script_contexts(&mock);
        self.site.attach(&mock);
        let index = self.count();
        if let Some(hook) = locked(&self.hook).as_ref() {
            hook(index, &mock);
        }
        locked(&self.launched).push(Launched {
            browser: browser.clone(),
            mock,
            options,
        });
        Ok(browser)
    }
}

/// Makes a mocked browser hand out the contexts `C1`, `C2`, and so on.
fn script_contexts(mock: &MockCtrl) {
    let made = Arc::new(AtomicU64::new(0));
    mock.on("Target.createBrowserContext", move |_| {
        let id = made.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(json!({ "browserContextId": format!("C{id}") }))
    });
}

/// Everything a test needs to run a scraper and look at what it did.
pub(super) struct Rig {
    pub(super) launcher: Arc<TestLauncher>,
    pub(super) site: Site,
    pub(super) sink: CollectingEventSink,
    pub(super) store: InMemoryStateStore,
    pub(super) clock: Arc<AtomicU64>,
}

impl Rig {
    /// A rig whose site answers with `pages`.
    pub(super) fn new(pages: Vec<Served>) -> Self {
        Self::with_product(pages, CHROME_155)
    }

    pub(super) fn with_product(pages: Vec<Served>, product: &'static str) -> Self {
        let site = Site::new(pages);
        Self {
            launcher: Arc::new(TestLauncher {
                site: site.clone(),
                product,
                launched: Mutex::new(Vec::new()),
                hook: Mutex::new(None),
            }),
            site,
            sink: CollectingEventSink::new(),
            store: InMemoryStateStore::new(),
            clock: Arc::new(AtomicU64::new(NOW)),
        }
    }

    /// A builder wired to this rig, with the seed and a short solve timeout.
    pub(super) fn builder(&self) -> BrowserScraperBuilder {
        let clock = Arc::clone(&self.clock);
        crate::scrape::runtime::browser_scraper::BrowserScraper::builder()
            .launcher(self.launcher.clone())
            .with_event_sink(Arc::new(self.sink.clone()))
            .with_state_store(Arc::new(self.store.clone()))
            .clock(Arc::new(move || clock.load(Ordering::SeqCst)))
            .solve_timeout(BRIEF_SOLVE)
            .seed(SEED)
    }

    /// Moves the scraper's clock forward.
    pub(super) fn advance(&self, seconds: u64) {
        self.clock.fetch_add(seconds, Ordering::SeqCst);
    }

    /// The names of the events so far, oldest first.
    pub(super) fn event_names(&self) -> Vec<&'static str> {
        self.sink
            .events()
            .iter()
            .map(|event| event.name())
            .collect()
    }
}

/// A country code that is known to be valid.
pub(super) fn country(code: &str) -> CountryCode {
    CountryCode::new(code).expect("a valid country code")
}

/// A challenge page of Cloudflare's script challenge, as the site serves it.
pub(super) fn script_challenge() -> Served {
    Served::new(503, LEGACY_JS)
        .header("server", "cloudflare")
        .header("cf-ray", "8a1b2c3d4e5f6071-AMS")
}

/// A page with a CAPTCHA widget on it, as the site serves it.
pub(super) fn widget_challenge() -> Served {
    Served::new(403, TURNSTILE)
        .header("server", "cloudflare")
        .header("cf-ray", "8a1b2c3d4e5f6071-AMS")
}

/// A real page, as the site serves it.
pub(super) fn real_page() -> Served {
    Served::new(200, NORMAL).header("server", "cloudflare")
}

/// An outright block, as the site serves it.
pub(super) fn block() -> Served {
    Served::new(403, BLOCKED)
        .header("server", "cloudflare")
        .header("cf-ray", "8a1b2c3d4e5f6071-AMS")
}
