//! The browser scraper through its public API, against scripted browsers, with no Chrome.
//!
//! A scripted site answers every mocked browser the launcher hands out: pages in order, the last one
//! for ever, a CAPTCHA widget on demand, and a clearance cookie once the widget was clicked. The
//! tests assert what a caller can observe: the page or error a fetch returns, the events it emits,
//! the state it keeps, and which protocol commands the browser was sent.
//!
//! Run them with `cargo test --features "scrape test-util" --test scrape_browser`. The same flows on
//! a real Chrome are in `scrape_browser_chrome.rs`.

#![cfg(all(feature = "scrape", feature = "test-util"))]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, MockCtrl};
use seleniumbase_rs::scrape::challenge::detect::ChallengeKind;
use seleniumbase_rs::scrape::egress::geo::CountryCode;
use seleniumbase_rs::scrape::error::ScrapeError;
use seleniumbase_rs::scrape::ledger::events::{CollectingEventSink, ScraperEvent};
use seleniumbase_rs::scrape::ledger::state::{InMemoryStateStore, StateStore};
use seleniumbase_rs::scrape::runtime::browser_scraper::{
    BrowserLauncher, BrowserScraper, BrowserScraperBuilder,
};
use seleniumbase_rs::SeleniumBaseError;
use serde_json::{json, Value};

/// How long a browser sits unused in the idle test: a week, far past any idle timeout a browser
/// library has had.
const IDLE_WINDOW: Duration = Duration::from_hours(7 * 24);

/// The time the tests start at, in seconds since the Unix epoch.
const NOW: u64 = 1_800_000_000;

/// How long a solve is waited for in a test that expects it to fail.
const BRIEF_SOLVE: Duration = Duration::from_millis(80);

/// How long the site is left alone after an outright block, from the default policy.
const BLOCK_COOLDOWN_SECS: u64 = 900;

const URL: &str = "https://shop.example/";

const SCRIPT_CHALLENGE: &str = include_str!("fixtures/challenge/cloudflare_legacy_js.html");
const WIDGET_CHALLENGE: &str =
    include_str!("fixtures/challenge/cloudflare_turnstile_challenge.html");
const BLOCKED: &str = include_str!("fixtures/challenge/cloudflare_blocked.html");
const NORMAL: &str = include_str!("fixtures/challenge/cloudflare_normal_page.html");

/// The domains whose `enable` command a page can feel.
const FORBIDDEN_DOMAINS: [&str; 5] = ["Runtime", "DOM", "Log", "Debugger", "Profiler"];

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One answer of the scripted site.
#[derive(Clone)]
struct Answer {
    status: u16,
    body: &'static str,
    headers: Vec<(&'static str, &'static str)>,
}

fn answer(status: u16, body: &'static str) -> Answer {
    Answer {
        status,
        body,
        headers: vec![("server", "cloudflare"), ("cf-ray", "8a1b2c3d4e5f6071-AMS")],
    }
}

#[derive(Default)]
struct Script {
    answers: VecDeque<Answer>,
    body: &'static str,
    navigations: usize,
    widget: bool,
    cleared: bool,
}

/// A scripted site that mocked browsers talk to.
#[derive(Clone)]
struct Site(Arc<Mutex<Script>>);

impl Site {
    fn new(answers: Vec<Answer>, widget: bool) -> Self {
        Self(Arc::new(Mutex::new(Script {
            answers: answers.into(),
            widget,
            ..Script::default()
        })))
    }

    fn navigations(&self) -> usize {
        lock(&self.0).navigations
    }

    fn attach(&self, mock: &MockCtrl) {
        let script = Arc::clone(&self.0);
        let ctrl = mock.clone();
        mock.on("Page.navigate", move |params| {
            let url = params["url"].as_str().unwrap_or_default().to_owned();
            let served = {
                let mut state = lock(&script);
                let next = if state.answers.len() > 1 {
                    state.answers.pop_front()
                } else {
                    state.answers.front().cloned()
                };
                let served = next.unwrap_or_else(|| answer(200, ""));
                state.body = served.body;
                state.navigations += 1;
                served
            };
            let headers: serde_json::Map<String, Value> = served
                .headers
                .iter()
                .map(|(name, value)| ((*name).to_owned(), json!(value)))
                .collect();
            // The mock does not say which tab navigates, so the answer is announced for both ids it
            // hands out to a tab's main frame.
            for frame in ["T1", "T2"] {
                ctrl.emit(
                    "Network.responseReceived",
                    json!({
                        "requestId": "R1", "loaderId": "L1", "frameId": frame, "type": "Document",
                        "response": { "url": url, "status": served.status, "headers": headers },
                    }),
                    Some("S1"),
                );
            }
            ctrl.emit(
                "Page.loadEventFired",
                json!({ "timestamp": 0.0 }),
                Some("S1"),
            );
            Ok(json!({ "frameId": "F1", "loaderId": "L1" }))
        });
        let script = Arc::clone(&self.0);
        mock.on("Runtime.evaluate", move |params| {
            let expression = params["expression"].as_str().unwrap_or_default();
            let state = lock(&script);
            let value = if expression.contains("outerHTML") {
                json!(state.body)
            } else if expression.contains("innerWidth") {
                json!([1280, 720])
            } else if expression.contains("const selectors") && state.widget {
                json!({ "kind": 0, "x": 100.0, "y": 200.0, "width": 300.0, "height": 65.0 })
            } else {
                Value::Null
            };
            Ok(json!({ "result": { "value": value } }))
        });
        let script = Arc::clone(&self.0);
        mock.on("Network.getAllCookies", move |_| {
            let cleared = lock(&script).cleared;
            let cookies = if cleared {
                vec![json!({
                    "name": "cf_clearance", "value": "fresh-token", "domain": ".shop.example",
                    "path": "/", "expires": -1.0, "httpOnly": false, "secure": true,
                    "sameSite": "Lax",
                })]
            } else {
                vec![]
            };
            Ok(json!({ "cookies": cookies }))
        });
        let script = Arc::clone(&self.0);
        mock.on("Input.dispatchMouseEvent", move |params| {
            if params["type"] == "mousePressed" {
                lock(&script).cleared = true;
            }
            Ok(json!({}))
        });
    }
}

/// Hands out mocked browsers that all talk to one site.
struct Launcher {
    site: Site,
    launched: Mutex<Vec<(Browser, MockCtrl)>>,
}

impl Launcher {
    fn count(&self) -> usize {
        lock(&self.launched).len()
    }

    fn nth(&self, index: usize) -> (Browser, MockCtrl) {
        lock(&self.launched)[index].clone()
    }

    fn methods(&self) -> Vec<String> {
        lock(&self.launched)
            .iter()
            .flat_map(|(_, mock)| mock.calls())
            .map(|call| call.method)
            .collect()
    }
}

#[async_trait]
impl BrowserLauncher for Launcher {
    async fn launch(&self, _options: LaunchOptions) -> Result<Browser, ScrapeError> {
        let (browser, mock) = Browser::new_mocked();
        mock.reply(
            "Browser.getVersion",
            json!({ "product": "Chrome/155.0.8059.40" }),
        );
        let made = Arc::new(AtomicU64::new(0));
        mock.on("Target.createBrowserContext", move |_| {
            let id = made.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(json!({ "browserContextId": format!("C{id}") }))
        });
        self.site.attach(&mock);
        lock(&self.launched).push((browser.clone(), mock));
        Ok(browser)
    }
}

/// A scripted world: the site, its launcher, and what the scraper reports.
struct World {
    site: Site,
    launcher: Arc<Launcher>,
    sink: CollectingEventSink,
    store: InMemoryStateStore,
    clock: Arc<AtomicU64>,
}

impl World {
    fn new(answers: Vec<Answer>, widget: bool) -> Self {
        let site = Site::new(answers, widget);
        Self {
            launcher: Arc::new(Launcher {
                site: site.clone(),
                launched: Mutex::new(Vec::new()),
            }),
            site,
            sink: CollectingEventSink::new(),
            store: InMemoryStateStore::new(),
            clock: Arc::new(AtomicU64::new(NOW)),
        }
    }

    fn builder(&self) -> BrowserScraperBuilder {
        let clock = Arc::clone(&self.clock);
        BrowserScraper::builder()
            .launcher(self.launcher.clone())
            .with_event_sink(Arc::new(self.sink.clone()))
            .with_state_store(Arc::new(self.store.clone()))
            .clock(Arc::new(move || clock.load(Ordering::SeqCst)))
            .solve_timeout(BRIEF_SOLVE)
            .seed(7)
    }

    fn names(&self) -> Vec<&'static str> {
        self.sink.events().iter().map(ScraperEvent::name).collect()
    }
}

fn two_proxies(world: &World) -> BrowserScraperBuilder {
    let de = CountryCode::new("DE").expect("a valid country");
    let fr = CountryCode::new("FR").expect("a valid country");
    world
        .builder()
        .with_geo_proxies([
            ("http://ualpha:s3cret@de.proxy.example:8080", de),
            ("http://ubeta:hunter2@fr.proxy.example:8080", fr),
        ])
        .expect("valid specs")
}

#[tokio::test(start_paused = true)]
async fn a_real_page_is_fetched_and_the_site_is_remembered() {
    let world = World::new(vec![answer(200, NORMAL)], false);
    let mut scraper = world.builder().build().await.expect("the scraper builds");

    let page = scraper.fetch(URL).await.expect("the page is fetched");

    assert_eq!((page.status, page.attempts, page.solved), (200, 1, None));
    assert_eq!(page.detection.kind, ChallengeKind::None);
    assert_eq!(page.url, URL);
    assert_eq!(page.body, NORMAL);
    assert_eq!(world.names(), ["request", "action_chosen"]);
    let state = world
        .store
        .get("shop.example")
        .await
        .expect("the store reads")
        .expect("the site is known");
    assert_eq!((state.successes, state.challenges), (1, 0));
    scraper.close().await.expect("the scraper closes");
}

#[tokio::test(start_paused = true)]
async fn a_widget_challenge_is_solved_and_the_second_load_succeeds() {
    let world = World::new(
        vec![answer(403, WIDGET_CHALLENGE), answer(200, NORMAL)],
        true,
    );
    let mut scraper = world.builder().build().await.expect("the scraper builds");

    let page = scraper.fetch(URL).await.expect("the challenge is passed");

    assert_eq!(page.attempts, 2);
    assert_eq!(page.solved, Some(ChallengeKind::Interactive));
    let (_, mock) = world.launcher.nth(1);
    let pressed = mock
        .calls_to("Input.dispatchMouseEvent")
        .iter()
        .filter(|call| call.params["type"] == "mousePressed")
        .count();
    assert_eq!(pressed, 1, "the widget is clicked once");
    assert!(
        world.names().contains(&"challenge_solved"),
        "{:?}",
        world.names()
    );
}

#[tokio::test(start_paused = true)]
async fn a_challenge_that_stands_is_reported_as_unsolved_with_the_attempts_made() {
    let world = World::new(vec![answer(503, SCRIPT_CHALLENGE)], false);
    let mut scraper = world
        .builder()
        .with_max_challenge_attempts(3)
        .build()
        .await
        .expect("the scraper builds");

    let error = scraper.fetch(URL).await.expect_err("it is never passed");

    assert!(
        matches!(
            error,
            ScrapeError::Unsolved {
                kind: ChallengeKind::JsChallenge,
                attempts: 3,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(matches!(
        SeleniumBaseError::from(error),
        SeleniumBaseError::Stealth { .. }
    ));
}

#[tokio::test(start_paused = true)]
async fn a_block_starts_a_cooldown_and_the_next_fetch_is_refused_without_a_request() {
    let world = World::new(vec![answer(403, BLOCKED)], false);
    let mut scraper = world.builder().build().await.expect("the scraper builds");

    let first = scraper.fetch(URL).await.expect_err("the site blocks");
    let second = scraper
        .fetch(URL)
        .await
        .expect_err("the site is left alone");

    let until = NOW + BLOCK_COOLDOWN_SECS;
    assert!(
        matches!(&first, ScrapeError::CoolingDown { until: at, .. } if *at == until),
        "{first:?}"
    );
    assert!(
        matches!(&second, ScrapeError::CoolingDown { until: at, .. } if *at == until),
        "{second:?}"
    );
    assert_eq!(world.site.navigations(), 1);
    world
        .clock
        .fetch_add(BLOCK_COOLDOWN_SECS + 1, Ordering::SeqCst);
    scraper.fetch(URL).await.expect_err("it blocks again");
    assert_eq!(
        world.site.navigations(),
        2,
        "after the cooldown the site is tried again"
    );
}

#[tokio::test(start_paused = true)]
async fn rotations_change_the_egress_and_the_profile_and_keep_what_is_known_of_the_site() {
    let world = World::new(vec![answer(503, SCRIPT_CHALLENGE)], false);
    let mut scraper = two_proxies(&world)
        .with_max_challenge_attempts(4)
        .build()
        .await
        .expect("the scraper builds");
    let first_seed = scraper.seed();

    let error = scraper.fetch(URL).await.expect_err("it is never passed");

    assert!(matches!(error, ScrapeError::Unsolved { attempts: 4, .. }));
    let names = world.names();
    assert!(
        names.contains(&"egress_rotated") && names.contains(&"profile_rotated"),
        "{names:?}"
    );
    assert_eq!(
        world.launcher.count(),
        4,
        "a probe, the first browser, one for the other country, one for the new profile"
    );
    assert_eq!(
        scraper.egress().map(|key| key.host.as_str()),
        Some("fr.proxy.example")
    );
    assert_ne!(scraper.seed(), first_seed);
    let state = world
        .store
        .get("shop.example")
        .await
        .expect("reads")
        .expect("known");
    assert_eq!(state.challenges, 4);
}

#[tokio::test(start_paused = true)]
async fn a_browser_left_idle_for_a_week_is_still_usable() {
    let world = World::new(vec![answer(200, NORMAL)], false);
    let mut scraper = world.builder().build().await.expect("the scraper builds");
    scraper.fetch(URL).await.expect("the first fetch");

    tokio::time::advance(IDLE_WINDOW).await;

    let (browser, _) = world.launcher.nth(1);
    assert!(
        browser.is_connected(),
        "nothing closed the browser while it idled"
    );
    scraper
        .fetch(URL)
        .await
        .expect("the fetch after the idle window");
    assert!(
        !world
            .launcher
            .methods()
            .iter()
            .any(|method| method == "Browser.close"),
        "the scraper never closes a browser it was not told to"
    );
}

#[tokio::test(start_paused = true)]
async fn no_flow_switches_on_a_domain_a_page_can_feel() {
    let world = World::new(
        vec![answer(403, WIDGET_CHALLENGE), answer(200, NORMAL)],
        true,
    );
    let mut scraper = two_proxies(&world)
        .build()
        .await
        .expect("the scraper builds");

    scraper.fetch(URL).await.expect("the challenge is passed");
    scraper.rotate_egress().await.expect("the egress rotates");
    scraper.rotate_profile().await.expect("the profile rotates");
    let identity = scraper
        .export_identity()
        .await
        .expect("the identity exports");
    scraper
        .import_identity(&identity)
        .await
        .expect("the identity imports");

    let methods = world.launcher.methods();
    assert!(
        methods.iter().any(|method| method == "Page.enable"),
        "the flow really ran"
    );
    for domain in FORBIDDEN_DOMAINS {
        let method = format!("{domain}.enable");
        assert!(!methods.contains(&method), "{method} was sent");
    }
}

#[tokio::test(start_paused = true)]
async fn a_proxy_password_a_token_and_a_cookie_never_leave_the_scraper() {
    let world = World::new(
        vec![answer(403, WIDGET_CHALLENGE), answer(200, NORMAL)],
        true,
    );
    let mut scraper = two_proxies(&world)
        .build()
        .await
        .expect("the scraper builds");

    let page = scraper
        .fetch("https://shop.example/?token=hunter3")
        .await
        .expect("the challenge is passed");
    scraper.rotate_egress().await.expect("the egress rotates");
    let identity = scraper
        .export_identity()
        .await
        .expect("the identity exports");
    let error = scraper
        .fetch("https://ualpha:s3cret@")
        .await
        .expect_err("a URL with no host");

    let state = world.store.get("shop.example").await.expect("reads");
    let printed = format!(
        "{scraper:?} {page:?} {identity:?} {error} {error:?} {} {:?} {state:?}",
        serde_json::to_string(&world.sink.events()).expect("events serialize"),
        world.sink.events(),
    );
    let sent: String = (0..world.launcher.count())
        .flat_map(|index| world.launcher.nth(index).1.calls())
        .map(|call| call.params.to_string())
        .collect();
    for secret in [
        "s3cret",
        "hunter2",
        "hunter3",
        "ualpha",
        "ubeta",
        "fresh-token",
    ] {
        assert!(!printed.contains(secret), "{secret} in {printed}");
    }
    for secret in ["s3cret", "hunter2", "ualpha", "ubeta"] {
        assert!(!sent.contains(secret), "{secret} was sent to the browser");
    }
}
