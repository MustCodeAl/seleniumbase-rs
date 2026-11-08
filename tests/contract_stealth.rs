//! The contract with `seleniumbase-rs-stealth`, the experimental stealth layer (crates
//! `seleniumbase-rs-codec`, `-scrape` and `-wire`) that builds on this crate's public API.
//!
//! The stealth repository depends on this crate by git, with no tag or revision, so it takes
//! whatever its lockfile holds and meets a change here the day it updates. This file is what
//! makes that day a planned one. It has two parts:
//!
//! * `imports`: the `use` lines of the stealth repository that name a `seleniumbase_rs::` item,
//!   verbatim, one block each (so the same name can be imported twice), plus the paths it writes
//!   in code and in rustdoc links. If a change here removes or renames one, this file stops
//!   compiling, and the error names the item.
//! * `calls`: the signatures the stealth crates depend on, as they call them. A function here
//!   mirrors a call site (the file is named above it) and is compiled, never run. A changed
//!   argument or return type breaks it.
//!
//! When this fails to compile, change this file in the same commit as the change that broke it,
//! and list what the stealth repository has to change in the commit message. Do not add a shim:
//! the crate has no users yet, and the stealth repository follows.
//!
//! `scripts/check-consumers.sh stealth` (`mise run contracts`) reads the stealth checkout and fails
//! when it imports a `seleniumbase_rs::` path that is not in this file, so the list cannot lag.
//!
//! The stealth crates take this crate with its default features, which are none. `Browser::new_mocked`
//! and `MockCtrl` need `test-util` here (the stealth tests use them), so the calls that use them
//! are behind `#[cfg(feature = "test-util")]`.

#![allow(
    dead_code,
    unused_imports,
    clippy::unused_async,
    clippy::items_after_statements
)]

mod imports {
    //! The imports of the stealth repository, as written there.

    // crates/codec/src/measurement.rs, tests, crates/wire/*, crates/scrape/*: `evasion::fingerprint`.
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::OsType;
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::{ClientHints, OsType};
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::{BrandVersion, ClientHints};
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::{BrowserType, Fingerprint, OsType};
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::{BrandVersion, Fingerprint, OsType, Spoof};
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::{Fingerprint, OsType};
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::{Fingerprint, FingerprintError, Geolocation};
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::{OsType, Spoof};
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::{Fingerprint, FingerprintError, OsType, Spoof};
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::{BatteryProfile, ProxyConfig};
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::Fingerprint;
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::FingerprintError;
    };
    const _: () = {
        use seleniumbase_rs::evasion::fingerprint::ProxyConfig;
    };
    // `evasion::humanize`, `randomize` and `behavior`.
    const _: () = {
        use seleniumbase_rs::evasion::humanize::Rng;
    };
    pub use seleniumbase_rs::evasion::randomize::{Locale, LOCALES};
    const _: () = {
        use seleniumbase_rs::evasion::behavior::Behavior;
    };
    // `sb_cdp`.
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::LaunchOptions;
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{LaunchOptions, Proxy};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{LaunchOptions, Page};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Browser, Page};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::LaunchOptions;
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::SameSite;
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Page, Proxy};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::Captcha;
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Cookie, SameSite};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::Cookie;
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page, SessionStore};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{LaunchOptions, SessionStore};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{CdpEvent, Page, Session, SolveOptions};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Browser, BrowserContext, ContextOptions, Page};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Browser, ContextOptions, LaunchOptions, Page};
    };
    const _: () = {
        use seleniumbase_rs::sb_cdp;
    };
    // Items the stealth tests and its examples import, which need `test-util` here.
    #[cfg(feature = "test-util")]
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Browser, MockCtrl};
    };
    #[cfg(feature = "test-util")]
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Browser, MockCtrl, Page};
    };
    #[cfg(feature = "test-util")]
    const _: () = {
        use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, MockCtrl};
    };
    // The error, the retry and shutdown helpers, and the URL normaliser.
    const _: () = {
        use seleniumbase_rs::error::SeleniumBaseError;
    };
    const _: () = {
        use seleniumbase_rs::SeleniumBaseError;
    };
    const _: () = {
        use seleniumbase_rs::Fingerprint;
    };
    const _: () = {
        use seleniumbase_rs::utilities::retry::RateLimiter;
    };
    const _: () = {
        use seleniumbase_rs::utilities::retry::RetryPolicy;
    };
    const _: () = {
        use seleniumbase_rs::utilities::retry::{poll_until, RetryPolicy};
    };
    const _: () = {
        use seleniumbase_rs::utils::urls::normalize_page_url;
    };
    const _: () = {
        use seleniumbase_rs::common::shutdown::{cleanup_within, spawn_cleanup, timeout_from_env};
    };
    const _: () = {
        use seleniumbase_rs::common::shutdown::cleanup_within;
    };

    // The facade crate (src/lib.rs) re-exports the whole of the core crate.
    pub mod facade {
        pub use seleniumbase_rs::*;
    }

    /// `[`Page::goto`](seleniumbase_rs::sb_cdp::Page::goto)` is linked from the fetch code, and
    /// every fetch calls it with a `&str`.
    async fn goto_is_called_with_a_str(
        page: &seleniumbase_rs::sb_cdp::Page,
    ) -> Result<(), seleniumbase_rs::SeleniumBaseError> {
        let goto = seleniumbase_rs::sb_cdp::Page::goto;
        goto(page, "https://example.com/").await
    }

    // Paths the stealth crates write in code or in rustdoc links, not in a `use` line.
    fn paths_in_code() {
        let _ = seleniumbase_rs::evasion::fingerprint::BrowserType::Firefox;
        let _ = seleniumbase_rs::evasion::fingerprint::OsType::Macos;
        let _ = seleniumbase_rs::evasion::fingerprint::ProxyConfig {
            host: "proxy.example".to_owned(),
            ..Default::default()
        };
        let _ = seleniumbase_rs::ProxyConfig {
            host: "proxy.example".to_owned(),
            ..seleniumbase_rs::ProxyConfig::default()
        };
        let _ = seleniumbase_rs::SeleniumBaseError::stealth("a stealth failure".to_owned());
        let _ = seleniumbase_rs::evasion::randomize::chromium_hints(120).brands;
        let _ = seleniumbase_rs::sb_cdp::Proxy::has_credentials;
        let _ = goto_is_called_with_a_str;
        let _: Option<seleniumbase_rs::sb_cdp::BrowserPool> = None;
        let _: Option<seleniumbase_rs::sb_cdp::SessionStore> = None;
        let _: Option<seleniumbase_rs::evasion::fingerprint::BrandVersion> = None;
        let _: Option<seleniumbase_rs::evasion::fingerprint::ClientHints> = None;
    }
}

mod calls {
    //! The signatures the stealth crates call, as they call them.

    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use serde_json::{json, Value};

    use seleniumbase_rs::config::proxy_list::{
        ProxyEntry, ProxyList, ProxyOutcome, RotationStrategy,
    };
    use seleniumbase_rs::evasion::behavior::Behavior;
    use seleniumbase_rs::evasion::fingerprint::Geolocation;
    use seleniumbase_rs::evasion::fingerprint::{
        BrandVersion, BrowserType, ClientHints, Fingerprint, FingerprintError, OsType, ProxyConfig,
    };
    use seleniumbase_rs::evasion::humanize::Rng;
    use seleniumbase_rs::evasion::randomize::chromium_hints;
    use seleniumbase_rs::evasion::randomize::{Locale, LOCALES};
    use std::time::{SystemTime, UNIX_EPOCH};

    use seleniumbase_rs::sb_cdp::{
        Browser, BrowserContext, Captcha, CdpEvent, ContextOptions, Cookie, LaunchOptions, Page,
        Proxy, SameSite, Session, SessionStore, SolveOptions,
    };
    use seleniumbase_rs::utilities::retry::{poll_until, RateLimiter, RetryPolicy};
    use seleniumbase_rs::utils::urls::normalize_page_url;
    use seleniumbase_rs::SeleniumBaseError;
    use seleniumbase_rs::{common::shutdown, common::shutdown::cleanup_within};

    fn is_error<E: std::error::Error + Send + Sync + 'static>() {}

    fn is_debug_clone_eq<T: std::fmt::Debug + Clone + PartialEq>() {}

    /// What the stealth crates convert with `?`, or keep in their own error types.
    #[test]
    fn the_error_and_value_types_have_the_traits_the_stealth_crates_use() {
        is_error::<SeleniumBaseError>();
        is_error::<FingerprintError>();
        is_debug_clone_eq::<Fingerprint>();
        is_debug_clone_eq::<RetryPolicy>();
        is_debug_clone_eq::<ProxyConfig>();
        is_debug_clone_eq::<Cookie>();
        is_debug_clone_eq::<CdpEvent>();
        let _: fn(&Proxy) -> bool = Proxy::has_credentials;
    }

    /// crates/wire/src/stealth_session/fetch.rs and crates/scrape/src/runtime/cdp.rs.
    #[test]
    fn a_page_url_is_normalised_as_page_goto_reads_it() {
        let _: fn(&str) -> Result<String, SeleniumBaseError> = normalize_page_url;
        assert_eq!(
            normalize_page_url("example.com").expect("a bare host"),
            "https://example.com"
        );
    }

    /// crates/wire/src/http_session/pace.rs, stealth_session/pace.rs and
    /// crates/scrape/src/runtime/browser_scraper/pacing.rs.
    async fn the_pacers(calls: usize, window: Duration) {
        let limiter: Arc<RateLimiter> = Arc::new(RateLimiter::new(calls, window));
        limiter.acquire().await;
        let mut limiters: HashMap<String, Arc<RateLimiter>> = HashMap::new();
        limiters.insert("shop.example".to_owned(), Arc::clone(&limiter));
    }

    /// crates/scrape/src/challenge/mitigation.rs and runtime/cdp.rs.
    async fn the_polling(timeout: Duration) -> Option<()> {
        let backoff: RetryPolicy = RetryPolicy::network();
        assert_eq!(backoff, RetryPolicy::network());
        poll_until(
            timeout,
            RetryPolicy::every(Duration::from_millis(250)),
            || async { Some(()) },
        )
        .await
    }

    /// crates/wire/src/mitm/mod.rs and mitm/websocket.rs.
    async fn the_shutdown_helpers(grace: Duration) {
        let _: Duration = shutdown::timeout_from_env();
        let ended: bool = cleanup_within(grace, async {}).await;
        let _: bool = ended;
        let _: bool = shutdown::spawn_cleanup(async {});
    }

    /// crates/scrape/src/egress/*.rs, jar/*.rs, challenge/detect.rs and the tests.
    #[test]
    fn the_random_source_and_the_hints() {
        let mut rng = Rng::new(7);
        let _: u64 = rng.next_u64();
        let hints: ClientHints = chromium_hints(120);
        let names = |list: &[BrandVersion]| -> Vec<String> {
            list.iter().map(|entry| entry.brand.clone()).collect()
        };
        assert_eq!(names(&hints.brands), names(&chromium_hints(120).brands));
        let _: usize = hints.full_version_list.len();
        let _: &str = hints
            .brands
            .first()
            .map(|entry| entry.brand.as_str())
            .expect("brands");
    }

    /// crates/scrape/src/runtime/browser_scraper/launcher.rs and rotate.rs; the identity
    /// tests of crates/wire.
    #[test]
    fn a_fingerprint_is_made_read_and_changed_as_the_stealth_crates_do() {
        let fingerprint: Fingerprint = Fingerprint::randomized(OsType::Macos, 7);
        let with_major: Fingerprint = Fingerprint::randomized_with_major(OsType::Linux, 7, 120);
        let _: Option<&str> = fingerprint.user_agent().as_str();
        let _: Option<&str> = fingerprint.locale().as_str();
        let _: Option<&str> = fingerprint.timezone().as_str();
        let _: u64 = fingerprint.seed();
        let _ = with_major;
        let _ = BrowserType::Firefox;
        let proxied: Result<Fingerprint, FingerprintError> = fingerprint
            .to_builder()
            .proxy(ProxyConfig {
                host: "proxy.example".to_owned(),
                ..Default::default()
            })
            .build();
        assert!(proxied.is_ok());
    }

    /// crates/codec/src/header_spec.rs (`ClientHeaders::from_fingerprint`), crates/wire/src/client.rs
    /// (`WireFormat::from_fingerprint`) and crates/scrape/src/passport/identity.rs: what the
    /// stealth crates read of a core fingerprint, and what they need it to implement.
    #[test]
    fn the_fingerprint_is_read_for_the_wire_format_and_kept_in_an_identity() {
        fn is_serde<T: serde::Serialize + serde::de::DeserializeOwned>() {}
        is_serde::<Fingerprint>();
        is_serde::<Cookie>();

        let fingerprint: Fingerprint = Fingerprint::randomized_with_major(OsType::Windows, 7, 120);
        let _: BrowserType = fingerprint.browser_type();
        let _: bool = fingerprint.browser_type() != BrowserType::Chromium;
        let _: OsType = fingerprint.os_type();
        let _: bool = fingerprint.os_type().is_mobile();
        let _: Option<&str> = fingerprint.user_agent().as_str();
        let _: Option<&str> = fingerprint.accept_languages().as_str();
        let _: Option<&str> = fingerprint.languages().as_str();
        let _: bool = fingerprint.client_hints().is_off();
        let _: Option<&ClientHints> = fingerprint.client_hints().value();
        let _: bool = fingerprint.proxy().is_set();
        let _: Vec<String> = fingerprint.validate().errors;

        let mut computed: ClientHints = chromium_hints(120);
        "Windows".clone_into(&mut computed.platform);
        computed.mobile = false;
    }

    /// crates/scrape/src/egress/geo.rs: a fingerprint moved to a locale and a position, and the
    /// place a geo answer holds.
    #[test]
    fn a_fingerprint_is_moved_to_a_locale_and_a_position_as_the_scraper_does() {
        let locale: &Locale = &LOCALES[0];
        let _: (&str, &str, &str) = (locale.tag, locale.country, locale.timezone);
        let _: (f64, f64) = (locale.latitude, locale.longitude);
        let languages: String = locale.accept_language();

        let place: Geolocation = Geolocation::new(locale.latitude, locale.longitude).accuracy(40.0);
        assert_eq!(
            place,
            Geolocation::new(locale.latitude, locale.longitude).accuracy(40.0)
        );
        is_debug_clone_eq::<Geolocation>();

        let fingerprint = Fingerprint::randomized(OsType::Linux, 7);
        let _: bool = fingerprint.geolocation().is_off();
        let moved: Result<Fingerprint, FingerprintError> = fingerprint
            .to_builder()
            .locale(locale.tag)
            .languages(languages.clone())
            .accept_languages(languages)
            .timezone(locale.timezone)
            .geolocation(place)
            .build();
        assert!(moved.is_ok());
    }

    /// crates/wire/src/http_session/page.rs: the `Accept-Language` of the raw list a page
    /// reports, which the wire crate no longer writes itself.
    #[test]
    fn the_accept_language_of_a_raw_list_is_written_as_core_writes_it() {
        let _: fn(&[String]) -> Option<String> = Locale::accept_language_list::<String>;
        let reported: Vec<String> = ["de-DE", " de ", "de", "en-US"]
            .iter()
            .map(|language| (*language).to_owned())
            .collect();
        assert_eq!(
            Locale::accept_language_list(&reported).as_deref(),
            Some("de-DE,de;q=0.9,en-US;q=0.8")
        );
        assert_eq!(Locale::accept_language_list::<String>(&[]), None);
        assert_eq!(Locale::accept_language_list(&["fr"]).as_deref(), Some("fr"));
    }

    /// crates/scrape/src/challenge/mitigation.rs, runtime/cdp.rs and
    /// crates/wire/src/stealth_session/browser.rs: what the policy sets on the solver, and reads
    /// back to decide a rotation and to bound the wait for a result.
    #[test]
    fn the_solver_options_the_scrapers_set_and_read() {
        is_debug_clone_eq::<SolveOptions>();
        let options: SolveOptions = SolveOptions::new()
            .locate_timeout(Duration::from_secs(6))
            .interstitial_timeout(Duration::from_secs(20))
            .verify_timeout(Duration::from_secs(30))
            .attempts(3)
            .backoff(Duration::from_millis(600))
            .retry_policy(RetryPolicy::network())
            .rounds(2)
            .cooldown(Duration::from_secs(5))
            .rotate_proxy_after(2)
            .rotate_fingerprint_after(3)
            .clearance_cookie("cf_clearance")
            .success_marker("document.title");
        let _: u32 = options.attempts_allowed();
        let _: u32 = options.rounds_allowed();
        let _: Duration = options.cooldown_between_rounds();
        let _: Duration = options.verify_timeout_allowed();
        let _: bool = options.proxy_rotation_due(2);
        let _: bool = options.fingerprint_rotation_due(3);
        assert_eq!(options.verify_timeout_allowed(), Duration::from_secs(30));
        let _: Duration = RetryPolicy::network().delay_for_attempt(2);
    }

    /// crates/scrape/src/passport/identity.rs and runtime/cdp.rs: a session as an identity holds
    /// it, and writes it to a file and back.
    #[test]
    fn a_session_is_kept_in_an_identity_and_read_back_field_by_field() {
        let held: Session = Session::default();
        assert_eq!(held, Session::new(Vec::new()));
        let storage: std::collections::BTreeMap<String, String> =
            [("theme".to_owned(), "dark".to_owned())].into();
        let session: Session = Session::new(vec![a_cdp_cookie(None)])
            .with_local_storage("https://shop.example", storage.clone());
        let _: &[Cookie] = session.cookies();
        let _: Option<&str> = session.origin();
        let _: &std::collections::BTreeMap<String, String> = session.local_storage();
        assert_eq!(session.local_storage(), &storage);
        is_debug_clone_eq::<Session>();
    }

    /// crates/scrape/src/egress/pool.rs: the proxy pool keeps core's `ProxyList` and drives it
    /// with the clock it holds (Unix seconds), so the forms that take the time are what it calls.
    #[test]
    fn the_proxy_list_picks_and_reports_at_the_time_the_pool_gives() {
        fn is_send_sync<T: Send + Sync>() {}
        is_send_sync::<ProxyList>();

        let mut list: ProxyList = ProxyList::new();
        list.insert(Some("de"), "http://de.example:8080")
            .expect("a proxy");
        list.insert(Some("fr"), "http://fr.example:8080")
            .expect("a proxy");
        let list = list
            .with_rotation(RotationStrategy::Random { seed: Some(7) })
            .with_quarantine(3, Duration::from_secs(600));
        let now: SystemTime = UNIX_EPOCH + Duration::from_secs(1_000);

        let picked: Option<&ProxyEntry> = list.pick_at(Some("shop.example"), now);
        let picked = picked.expect("a proxy");
        let _: Option<&str> = picked.name();
        let _: &str = picked.server();
        let _: &str = picked.spec();
        let _: Option<&ProxyEntry> = list.pick_other_at(picked, None, now);
        let _: Option<&ProxyEntry> =
            list.pick_where_at(|entry: &ProxyEntry| entry.name() == Some("de"), None, now);
        list.report_at(picked, ProxyOutcome::Challenged, now);
        list.report_at(picked, ProxyOutcome::Success, now);
        for strategy in [
            RotationStrategy::RoundRobin,
            RotationStrategy::LeastFailures,
            RotationStrategy::StickyPerSite,
        ] {
            let _ = strategy;
        }
        let _: usize = seleniumbase_rs::config::proxy_list::MAX_STICKY_SITES;
    }

    /// crates/wire/src/capture_h2/browser.rs: the launch options a measured browser gets.
    fn launch_options(
        headless: bool,
        lang: String,
        no_sandbox: bool,
        timeout: Duration,
        executable: Option<PathBuf>,
        extra: &[String],
    ) -> Result<LaunchOptions, SeleniumBaseError> {
        let mut builder = LaunchOptions::builder()
            .headless(headless)
            .lang(lang)
            .no_sandbox(no_sandbox)
            .startup_timeout(timeout);
        if let Some(path) = &executable {
            builder = builder.executable_path(path.clone());
        }
        for arg in extra {
            builder = builder.arg(arg.clone());
        }
        builder.build()
    }

    /// crates/wire/src/mitm/mod.rs, `MitmProxy::configure_launch`, and the stealth session's
    /// use of the fingerprint on the built options it holds.
    fn configure_launch(
        options: LaunchOptions,
        proxy: String,
        args: Vec<String>,
        start_page: String,
        fingerprint: &Fingerprint,
    ) -> Result<LaunchOptions, SeleniumBaseError> {
        let prime = options.url().is_none().then_some(start_page);
        let options = options.with_proxy(proxy)?.with_args(args)?;
        let options = match prime {
            Some(url) => options.with_url(url)?,
            None => options,
        };
        options.with_fingerprint(fingerprint)
    }

    /// crates/scrape/src/runtime/browser_scraper/builder.rs and rotate.rs: the launch options
    /// the scraper holds, the one-off headless probe, and the read of the fingerprint they hold.
    fn the_launch_options_a_scraper_holds(
        held: &LaunchOptions,
        fingerprint: &Fingerprint,
    ) -> Result<(LaunchOptions, LaunchOptions, Option<Fingerprint>), SeleniumBaseError> {
        let probe: LaunchOptions = held.clone().with_headless(true)?;
        let started: LaunchOptions = held.clone().with_fingerprint(fingerprint)?;
        let first: Option<Fingerprint> = held.fingerprint().cloned();
        Ok((probe, started, first))
    }

    /// crates/wire/tests/stealth_session/flows.rs, line 111.
    fn a_proxy_with_credentials(options: &LaunchOptions) -> bool {
        options.proxy().is_some_and(Proxy::has_credentials)
    }

    /// crates/wire/src/capture_h2/browser.rs and crates/scrape/src/runtime/browser_scraper/*.
    async fn the_browser(options: LaunchOptions) -> Result<(), SeleniumBaseError> {
        let browser: Browser = Browser::launch(options).await?;
        let _: Value = browser.execute("Browser.getVersion", json!({})).await?;
        let page: Page = browser
            .default_page()
            .await?
            .with_timeout(Duration::from_secs(20));
        let attached: Browser = Browser::connect("127.0.0.1", browser.debugging_port()).await?;
        let _ = attached;
        browser.close().await?;
        drop(page);
        Ok(())
    }

    /// crates/scrape/src/runtime/egress_switch.rs.
    async fn the_egress_switch(
        browser: &Browser,
        proxy: Option<&Proxy>,
    ) -> Result<Page, SeleniumBaseError> {
        let options = proxy.map_or_else(ContextOptions::new, |proxy| {
            ContextOptions::new().proxy(proxy.clone())
        });
        let context: BrowserContext = browser.new_context_with(options).await?;
        let _: &str = context.id();
        let page = context.new_page().await?;
        context.dispose().await?;
        Ok(page)
    }

    /// crates/scrape/src/runtime/cdp.rs: what a scrape reads from and does to a tab.
    async fn the_page(page: &Page, url: &str, now: u64) -> Result<(), SeleniumBaseError> {
        let url = normalize_page_url(url)?;
        let _: Value = page.execute("Network.enable", json!({})).await?;
        let mut events = page.events();
        {
            let navigation = page.goto(&url);
            navigation.await?;
        }
        let event: Option<CdpEvent> = events.next().await;
        if let Some(event) = event {
            let _: &str = event.method.as_str();
            let _: &Value = &event.params;
        }
        let _: &str = page.id();
        let _: String = page.content().await?;
        let _: Value = page.evaluate("1 + 1").await?;
        let origin: String = page.evaluate_as("location.origin").await?;

        let emulation = page.emulation();
        emulation.locale("de-DE").await?;
        emulation.timezone("Europe/Berlin").await?;

        let cookies: Vec<Cookie> = page.cookies().all().await?;
        page.cookies().set(&cookies).await?;
        let entries = page.local_storage().entries().await?;
        let mut session = Session::new(cookies);
        if !entries.is_empty() {
            session = session.with_local_storage(origin, entries);
        }
        let session = session.without_expired(now as f64);
        page.cookies().set(session.cookies()).await?;
        if let Some(origin) = session.origin() {
            let here: String = page.evaluate_as("location.origin").await?;
            if here == origin {
                let storage = page.local_storage();
                for (key, value) in session.local_storage() {
                    storage.set(key, value).await?;
                }
            }
        }

        // The warm-up and the CAPTCHA attempt.
        let behavior: Behavior = Behavior::builder()
            .seed(7)
            .build()
            .map_err(|error| SeleniumBaseError::stealth(error.to_string()))?;
        page.human(behavior).scroll_by(60.0).await?;
        match page.solve_captcha_with(&SolveOptions::default()).await {
            Ok(Some(solved)) => drop(format!("{solved}")),
            Ok(None) => {}
            Err(SeleniumBaseError::Captcha {
                captcha,
                attempts,
                reason,
            }) => drop(format!("{captcha} {attempts} {reason}")),
            Err(error) => return Err(error),
        }
        Ok(())
    }

    /// crates/scrape/src/jar/store/cdp.rs: a CDP cookie, field by field.
    fn a_cdp_cookie(same_site: Option<SameSite>) -> Cookie {
        let cookie = Cookie {
            name: "a".to_owned(),
            value: "b".to_owned(),
            domain: ".example.com".to_owned(),
            path: "/".to_owned(),
            expires: Some(1.0),
            http_only: true,
            secure: true,
            same_site,
        };
        let _: Option<f64> = cookie.expires;
        let _: &str = cookie.domain.as_str();
        cookie
    }

    /// crates/scrape/src/runtime/browser_scraper/rotate.rs and builder.rs.
    fn the_session_store(sessions: &SessionStore, session: Session) -> Option<Session> {
        sessions.save("name", session);
        let _loaded: Option<Session> = sessions.load("name");
        sessions.load("name")
    }

    /// crates/scrape/src/challenge/markers.rs and detect.rs.
    #[test]
    fn the_captcha_widgets_the_challenge_detector_names() {
        let widgets = [
            Captcha::Turnstile,
            Captcha::Recaptcha,
            Captcha::Hcaptcha,
            Captcha::FriendlyCaptcha,
            Captcha::DataDome,
        ];
        assert_eq!(widgets[0], Captcha::Turnstile);
        assert_eq!(Some(widgets[1]), Some(Captcha::Recaptcha));
    }

    /// The mock browser the stealth tests drive pages with (needs `test-util`).
    #[cfg(feature = "test-util")]
    #[test]
    fn the_mock_browser_the_stealth_tests_use() {
        let (browser, mock): (Browser, seleniumbase_rs::sb_cdp::MockCtrl) = Browser::new_mocked();
        let _ = (browser, mock);
    }
}
