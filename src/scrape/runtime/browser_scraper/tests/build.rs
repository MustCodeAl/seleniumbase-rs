//! Tests of how a scraper is built and of the small readers it uses.

use super::rig::{country, real_page, Rig, SEED};
use crate::sb_cdp::LaunchOptions;
use crate::scrape::challenge::detect::{ChallengeKind, Detection, Evidence};
use crate::scrape::egress::geo::GeoError;
use crate::scrape::error::ScrapeError;
use crate::scrape::runtime::browser_scraper::fetch::Target;
use crate::scrape::runtime::browser_scraper::launcher::{claimed_major, major_of, persona_for};
use crate::scrape::runtime::browser_scraper::{BrowserFetch, BrowserScraper};
use crate::stealth::fingerprint::{Fingerprint, OsType};

#[test]
fn the_major_version_is_read_from_a_product_token() {
    assert_eq!(major_of("Chrome/155.0.8059.40"), Some(155));
    assert_eq!(major_of("HeadlessChrome/155.0.8059.40"), Some(155));
    assert_eq!(major_of("Chromium/9.1"), Some(9));
    for unreadable in ["Chrome", "Chrome/x.y", "", "/"] {
        assert_eq!(major_of(unreadable), None, "{unreadable:?}");
    }
}

#[test]
fn a_persona_claims_the_given_major_and_the_claim_reads_back() {
    let claiming = persona_for(OsType::Macos, 3, Some(155));
    let drawn = persona_for(OsType::Macos, 3, None);

    assert_eq!(claimed_major(&claiming), Some(155));
    assert!(claimed_major(&drawn).is_some());
    assert_eq!(claimed_major(&Fingerprint::default()), None);
}

#[tokio::test]
async fn building_reads_the_installed_chrome_and_starts_a_profile_that_claims_it() {
    let rig = Rig::new(vec![real_page()]);

    let scraper = rig.builder().build().await.expect("the scraper builds");

    assert_eq!(
        rig.launcher.count(),
        2,
        "one probe, then the browser that stays"
    );
    let probe = rig.launcher.nth(0);
    assert!(probe.options.is_headless(), "the probe is headless");
    assert!(!probe.browser.is_connected(), "the probe is closed again");
    let kept = rig.launcher.nth(1);
    assert!(kept.browser.is_connected());
    let agent = kept
        .options
        .user_agent
        .clone()
        .expect("the profile sets the agent");
    assert!(agent.contains("Chrome/155."), "{agent}");
    assert_eq!(
        scraper.fingerprint().user_agent.as_deref(),
        Some(agent.as_str())
    );
    assert_eq!(scraper.seed(), SEED);
    assert_eq!(scraper.persona.major, Some(155));
    assert_eq!(scraper.page().id(), "T1");
}

#[tokio::test]
async fn a_chrome_that_does_not_say_its_version_still_gets_a_profile() {
    let rig = Rig::with_product(vec![real_page()], "");

    let scraper = rig.builder().build().await.expect("the scraper builds");

    assert_eq!(scraper.persona.major, None);
    assert!(claimed_major(scraper.fingerprint()).is_some());
}

#[tokio::test]
async fn a_browser_that_refuses_to_report_a_version_is_read_as_unknown() {
    let rig = Rig::new(vec![real_page()]);
    rig.launcher
        .on_launch(|_, mock| mock.fail("Browser.getVersion", "not supported"));

    let scraper = rig.builder().build().await.expect("the scraper builds");

    assert_eq!(scraper.persona.major, None);
}

#[tokio::test]
async fn a_given_fingerprint_skips_the_probe_and_reads_the_version_after_the_launch() {
    let rig = Rig::new(vec![real_page()]);
    let mine = Fingerprint::randomized_with_major(OsType::Macos, 99, 150);

    let scraper = rig
        .builder()
        .fingerprint(mine.clone())
        .build()
        .await
        .expect("the scraper builds");

    assert_eq!(rig.launcher.count(), 1, "no probe");
    assert_eq!(scraper.fingerprint(), &mine);
    assert_eq!(
        scraper.persona.major,
        Some(155),
        "the Chrome that runs, not the one claimed"
    );
}

#[tokio::test]
async fn the_same_seed_is_the_same_profile() {
    let first = Rig::new(vec![real_page()]);
    let second = Rig::new(vec![real_page()]);

    let a = first.builder().build().await.expect("builds");
    let b = second.builder().build().await.expect("builds");
    let other = second
        .builder()
        .seed(SEED + 1)
        .build()
        .await
        .expect("builds");

    assert_eq!(a.fingerprint(), b.fingerprint());
    assert_ne!(a.fingerprint(), other.fingerprint());
}

#[tokio::test]
async fn the_launch_options_of_the_caller_win_over_the_profile() {
    let rig = Rig::new(vec![real_page()]);
    let launch = LaunchOptions::builder()
        .user_agent("Custom/1.0")
        .lang("fr-FR");

    rig.builder()
        .launch_options(launch)
        .build()
        .await
        .expect("the scraper builds");

    let kept = rig.launcher.last();
    assert_eq!(kept.options.user_agent.as_deref(), Some("Custom/1.0"));
    assert_eq!(kept.options.lang.as_deref(), Some("fr-FR"));
}

#[tokio::test]
async fn the_first_egress_sets_the_locale_and_the_browser_works_behind_its_proxy() {
    let rig = Rig::new(vec![real_page()]);

    let scraper = rig
        .builder()
        .with_geo_proxies([("http://alice:s3cret@de.proxy.example:8080", country("DE"))])
        .expect("a valid spec")
        .build()
        .await
        .expect("the scraper builds");

    assert_eq!(scraper.fingerprint().locale.as_deref(), Some("de-DE"));
    assert_eq!(
        scraper.fingerprint().timezone.as_deref(),
        Some("Europe/Berlin")
    );
    let kept = rig.launcher.last();
    assert_eq!(kept.options.lang.as_deref(), Some("de-DE"));
    let created = kept.mock.calls_to("Target.createBrowserContext");
    assert_eq!(
        created[0].params["proxyServer"],
        "http://de.proxy.example:8080"
    );
    assert_eq!(
        scraper.page().id(),
        "T2",
        "the tab is the one in the proxy's context"
    );
    let key = scraper.egress().expect("an egress is in use");
    assert_eq!(key.to_string(), "http://de.proxy.example:8080");
    assert_eq!(key.country, Some(country("DE")));
    let shown = format!("{scraper:?}");
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
}

#[tokio::test]
async fn a_country_without_a_locale_fails_before_any_browser_starts() {
    let rig = Rig::new(vec![real_page()]);

    let result = rig
        .builder()
        .with_geo_proxies([("http://p.example:8080", country("ZZ"))])
        .expect("a valid spec")
        .build()
        .await;

    assert!(
        matches!(result, Err(ScrapeError::Geo(GeoError::NoLocale(_)))),
        "{result:?}"
    );
    assert_eq!(rig.launcher.count(), 0);
}

#[test]
fn a_bad_proxy_spec_is_refused_without_echoing_its_credentials() {
    let error = BrowserScraper::builder()
        .with_geo_proxies([("ftp://alice:s3cret@p.example:21", country("DE"))])
        .expect_err("ftp is not a proxy scheme");

    let shown = format!("{error} {error:?}");
    assert!(matches!(error, ScrapeError::Egress(_)));
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
}

#[tokio::test]
async fn a_tab_that_cannot_open_closes_the_browser_and_fails_the_build() {
    let rig = Rig::new(vec![real_page()]);
    rig.launcher.on_launch(|index, mock| {
        if index == 1 {
            mock.fail("Target.getTargets", "no tabs");
        }
    });

    let result = rig.builder().build().await;

    assert!(matches!(result, Err(ScrapeError::Browser(_))), "{result:?}");
    assert!(!rig.launcher.nth(1).browser.is_connected());
}

#[tokio::test]
async fn the_builder_and_a_fetch_print_no_secret() {
    let rig = Rig::new(vec![real_page()]);
    let builder = rig
        .builder()
        .with_geo_proxies([("http://alice:s3cret@de.proxy.example:8080", country("DE"))])
        .expect("a valid spec");
    let shown = format!("{builder:?}");
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
    assert!(shown.contains("egresses: 1"), "{shown}");

    let fetched = BrowserFetch {
        url: "https://alice:s3cret@shop.example/?token=hunter2".to_owned(),
        status: 200,
        headers: vec![("Set-Cookie".to_owned(), "sid=abc123".to_owned())],
        body: "<html>secret body</html>".to_owned(),
        detection: Detection {
            kind: ChallengeKind::None,
            evidence: Evidence::default(),
        },
        attempts: 1,
        solved: None,
        egress: None,
    };
    let shown = format!("{fetched:?}");
    for secret in ["alice", "s3cret", "hunter2", "abc123", "secret body"] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
}

#[test]
fn a_target_is_normalised_and_belongs_to_its_registrable_domain() {
    let bare = Target::parse("shop.example/a?x=1").expect("a bare host is read as https");
    let nested = Target::parse("https://www.shop.co.uk/x").expect("a URL");
    let address = Target::parse("http://127.0.0.1:8080/").expect("an address");

    assert_eq!(bare.url, "https://shop.example/a?x=1");
    assert_eq!(bare.site, "shop.example");
    assert_eq!(nested.site, "shop.co.uk");
    assert_eq!(address.site, "127.0.0.1");
}

#[test]
fn a_url_with_no_host_or_no_sense_is_refused_without_being_echoed() {
    for raw in ["about:blank", "", "http://alice:s3cret@/?token=hunter2"] {
        let error = match Target::parse(raw) {
            Err(error) => error,
            Ok(target) => panic!("{raw:?} was accepted as {}", target.site),
        };
        let shown = format!("{error} {error:?}");
        assert!(
            matches!(error, ScrapeError::Navigation { .. }),
            "{raw:?}: {error:?}"
        );
        for secret in ["alice", "s3cret", "hunter2"] {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }
    }
}
