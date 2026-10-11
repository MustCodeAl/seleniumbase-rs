//! A scripted Chrome for the tests of `StealthSession`: a launcher that hands out mocked browsers
//! which behave like a visitor of the shop in `site.rs`.
//!
//! A scripted browser meets the challenge page until it holds the clearance. It earns it by
//! itself, the way the page's script would: when its cookies have been read a number of times
//! (the session polls them while it waits), or never, for a test of a challenge that cannot be
//! passed. It also holds a clearance it is handed with `Network.setCookies`, if the value is
//! the one the site accepts. No Chrome is started and no traffic reaches the site.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use seleniumbase_rs::error::SeleniumBaseError;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, MockCtrl};
use seleniumbase_rs::scrape::error::ScrapeError;
use seleniumbase_rs::scrape::runtime::browser_scraper::BrowserLauncher;
use serde_json::{json, Value};

use crate::site::{CHALLENGE_HTML, CLEARANCE, DEFAULT_TTL_SECS, HOST, REAL_HTML};

/// The frames a mocked browser gives the main frame of its tabs, and its tab session.
const MAIN_FRAMES: [&str; 2] = ["T1", "T2"];
const MOCK_SESSION: &str = "S1";
/// The reads of the cookies after which a scripted browser holds the clearance.
const DEFAULT_POLLS_TO_SOLVE: usize = 2;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How the scripted browsers of one launcher behave.
#[derive(Clone, Copy)]
struct Knobs {
    polls_to_solve: usize,
    never_solves: bool,
    ttl_secs: u64,
}

/// What one scripted browser has done and holds.
#[derive(Default)]
struct Tab {
    cleared: bool,
    polls: usize,
    imported: Vec<Value>,
}

/// A browser the launcher handed out.
#[derive(Clone)]
pub struct Launched {
    pub browser: Browser,
    pub options: LaunchOptions,
    tab: Arc<Mutex<Tab>>,
}

impl Launched {
    /// The names of the cookies the browser was handed with `Network.setCookies`.
    pub fn imported_names(&self) -> Vec<String> {
        locked(&self.tab)
            .imported
            .iter()
            .filter_map(|cookie| cookie["name"].as_str().map(str::to_owned))
            .collect()
    }

    /// Whether the browser holds the clearance.
    pub fn is_cleared(&self) -> bool {
        locked(&self.tab).cleared
    }

    /// The server of the proxy the browser was launched with.
    pub fn proxy_server(&self) -> Option<String> {
        self.options.proxy().map(|proxy| proxy.server().to_owned())
    }
}

/// Hands out scripted browsers.
pub struct ScriptedLauncher {
    product: Mutex<String>,
    token: Arc<dyn Fn() -> String + Send + Sync>,
    clock: Arc<AtomicU64>,
    knobs: Mutex<Knobs>,
    launched: Mutex<Vec<Launched>>,
    /// How many launches from now on fail.
    failing: AtomicUsize,
    /// For each launch, how many earlier browsers were still connected when it was asked for.
    alive_at_launch: Mutex<Vec<usize>>,
}

impl ScriptedLauncher {
    /// A launcher whose browsers report `product` and accept the clearance `token` tells. The
    /// cookies they hold expire [`DEFAULT_TTL_SECS`] after `clock`.
    pub fn new(
        product: String,
        token: Arc<dyn Fn() -> String + Send + Sync>,
        clock: Arc<AtomicU64>,
    ) -> Self {
        Self {
            product: Mutex::new(product),
            token,
            clock,
            knobs: Mutex::new(Knobs {
                polls_to_solve: DEFAULT_POLLS_TO_SOLVE,
                never_solves: false,
                ttl_secs: DEFAULT_TTL_SECS,
            }),
            launched: Mutex::new(Vec::new()),
            failing: AtomicUsize::new(0),
            alive_at_launch: Mutex::new(Vec::new()),
        }
    }

    /// Makes the next `count` launches fail, as a Chrome that cannot start does.
    pub fn fail_next_launches(&self, count: usize) {
        self.failing.store(count, Ordering::SeqCst);
    }

    /// For each launch asked for, oldest first, how many browsers launched before it were still
    /// connected at that moment.
    pub fn alive_at_launch(&self) -> Vec<usize> {
        locked(&self.alive_at_launch).clone()
    }

    /// Makes the browsers launched from now on fail every challenge.
    pub fn never_solve(&self) {
        locked(&self.knobs).never_solves = true;
    }

    /// Changes the product the browsers launched from now on report.
    pub fn set_product(&self, product: &str) {
        product.clone_into(&mut locked(&self.product));
    }

    /// How many browsers were launched.
    pub fn count(&self) -> usize {
        locked(&self.launched).len()
    }

    /// The `index`th browser launched, oldest first.
    pub fn nth(&self, index: usize) -> Launched {
        locked(&self.launched)
            .get(index)
            .cloned()
            .unwrap_or_else(|| panic!("browser #{index} was never launched"))
    }

    /// The browser launched last.
    pub fn last(&self) -> Launched {
        self.nth(self.count() - 1)
    }
}

#[async_trait]
impl BrowserLauncher for ScriptedLauncher {
    async fn launch(&self, options: LaunchOptions) -> Result<Browser, ScrapeError> {
        let alive = locked(&self.launched)
            .iter()
            .filter(|launched| launched.browser.is_connected())
            .count();
        locked(&self.alive_at_launch).push(alive);
        let failing = self
            .failing
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
                left.checked_sub(1)
            });
        if failing.is_ok() {
            return Err(ScrapeError::Browser(SeleniumBaseError::stealth(
                "the scripted Chrome cannot start",
            )));
        }
        let (browser, mock) = Browser::new_mocked();
        let product = locked(&self.product).clone();
        mock.reply("Browser.getVersion", json!({ "product": product }));
        let tab = Arc::new(Mutex::new(Tab::default()));
        let knobs = *locked(&self.knobs);
        install_navigation(&mock, &tab);
        install_evaluation(&mock, &tab);
        install_cookies(&mock, &tab, knobs, self);
        locked(&self.launched).push(Launched {
            browser: browser.clone(),
            options,
            tab,
        });
        Ok(browser)
    }
}

/// Answers a navigation with the challenge, or the real page once the browser holds the
/// clearance, and announces the answer the way Chrome announces it.
fn install_navigation(mock: &MockCtrl, tab: &Arc<Mutex<Tab>>) {
    let tab = Arc::clone(tab);
    let ctrl = mock.clone();
    mock.on("Page.navigate", move |params| {
        let url = params["url"].as_str().unwrap_or_default().to_owned();
        let cleared = locked(&tab).cleared;
        let (status, headers) = if cleared {
            (200, json!({}))
        } else {
            (403, json!({ "cf-mitigated": "challenge" }))
        };
        for frame in MAIN_FRAMES {
            ctrl.emit(
                "Network.responseReceived",
                json!({
                    "requestId": "R1",
                    "loaderId": "L1",
                    "frameId": frame,
                    "type": "Document",
                    "response": { "url": url, "status": status, "headers": headers },
                }),
                Some(MOCK_SESSION),
            );
        }
        ctrl.emit(
            "Page.loadEventFired",
            json!({ "timestamp": 0.0 }),
            Some(MOCK_SESSION),
        );
        Ok(json!({ "frameId": "F1", "loaderId": "L1" }))
    });
}

/// Answers the reads of the page: its HTML and its origin.
fn install_evaluation(mock: &MockCtrl, tab: &Arc<Mutex<Tab>>) {
    let tab = Arc::clone(tab);
    mock.on("Runtime.evaluate", move |params| {
        let expression = params["expression"].as_str().unwrap_or_default();
        let state = locked(&tab);
        let value = if expression.contains("outerHTML") {
            json!(if state.cleared {
                REAL_HTML
            } else {
                CHALLENGE_HTML
            })
        } else if expression.contains("location.origin") {
            json!(format!("https://{HOST}"))
        } else {
            Value::Null
        };
        Ok(json!({ "result": { "value": value } }))
    });
}

/// Answers the reads and writes of the cookies.
fn install_cookies(
    mock: &MockCtrl,
    tab: &Arc<Mutex<Tab>>,
    knobs: Knobs,
    launcher: &ScriptedLauncher,
) {
    let (state, token, clock) = (
        Arc::clone(tab),
        Arc::clone(&launcher.token),
        Arc::clone(&launcher.clock),
    );
    mock.on("Network.getAllCookies", move |_| {
        let mut tab = locked(&state);
        tab.polls += 1;
        if !knobs.never_solves && tab.polls >= knobs.polls_to_solve {
            tab.cleared = true;
        }
        let cookies = if tab.cleared {
            let expires = clock.load(Ordering::SeqCst) + knobs.ttl_secs;
            vec![clearance_cookie(&token(), expires)]
        } else {
            Vec::new()
        };
        Ok(json!({ "cookies": cookies }))
    });
    let (state, token) = (Arc::clone(tab), Arc::clone(&launcher.token));
    mock.on("Network.setCookies", move |params| {
        let cookies = params["cookies"].as_array().cloned().unwrap_or_default();
        let mut tab = locked(&state);
        for cookie in cookies {
            let valid = cookie["name"] == CLEARANCE && cookie["value"] == json!(token());
            tab.cleared |= valid;
            tab.imported.push(cookie);
        }
        Ok(json!({}))
    });
}

/// The clearance as `Network.getAllCookies` reports it: host-only, secure, with an expiry.
fn clearance_cookie(value: &str, expires: u64) -> Value {
    json!({
        "name": CLEARANCE,
        "value": value,
        "domain": HOST,
        "path": "/",
        "expires": expires as f64,
        "httpOnly": true,
        "secure": true,
        "sameSite": "None",
    })
}
