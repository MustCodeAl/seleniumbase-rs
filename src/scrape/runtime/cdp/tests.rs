//! Tests of the CDP helpers against a scripted browser, with no Chrome.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;

use super::testkit::{cdp_cookie, mocked_page, response_params, Served, Site, MOCK_SESSION};
use super::*;
use crate::sb_cdp::{Cookie, MockCtrl};
use crate::scrape::egress::geo::{CountryCode, GeoInfo, StaticGeoResolver};
use crate::stealth::fingerprint::OsType;

const CLOUDFLARE_JS: &str =
    include_str!("../../../../tests/fixtures/challenge/cloudflare_legacy_js.html");
const NORMAL_PAGE: &str =
    include_str!("../../../../tests/fixtures/challenge/cloudflare_normal_page.html");

/// A moment well inside every test cookie's life, in seconds since the Unix epoch.
const NOW: u64 = 1_800_000_000;

/// A wait short enough for a test that expects to time out.
const BRIEF: Duration = Duration::from_millis(60);

fn egress(country: Option<&str>) -> EgressRef {
    let country = country.map(|code| CountryCode::new(code).expect("a valid code"));
    EgressRef::parse("http://proxy.example:8080", country).expect("a valid spec")
}

/// Makes `Page.navigate` announce `answers`, in order, as main-frame documents and then finish.
fn announce(mock: &MockCtrl, answers: Vec<(&'static str, Served)>) {
    let ctrl = mock.clone();
    mock.on("Page.navigate", move |_| {
        for (frame, served) in &answers {
            ctrl.emit(
                RESPONSE_EVENT,
                response_params(frame, DOCUMENT_TYPE, "https://shop.example/", served),
                Some(MOCK_SESSION),
            );
        }
        ctrl.emit(
            "Page.loadEventFired",
            json!({ "timestamp": 0.0 }),
            Some(MOCK_SESSION),
        );
        Ok(json!({ "loaderId": "L1" }))
    });
}

fn position(methods: &[String], wanted: &str) -> usize {
    methods
        .iter()
        .position(|method| method == wanted)
        .unwrap_or_else(|| panic!("{wanted} was never sent: {methods:?}"))
}

#[tokio::test]
async fn the_main_frame_document_answers_and_frames_and_subresources_do_not() {
    let (page, mock) = mocked_page().await;
    let ctrl = mock.clone();
    mock.on("Page.navigate", move |_| {
        let served = Served::new(200, "").header("X-Image", "1");
        ctrl.emit(
            RESPONSE_EVENT,
            response_params("T1", "Image", "https://shop.example/a.png", &served),
            Some(MOCK_SESSION),
        );
        let frame = Served::new(200, "").header("X-Frame", "1");
        ctrl.emit(
            RESPONSE_EVENT,
            response_params("F9", DOCUMENT_TYPE, "https://shop.example/frame", &frame),
            Some(MOCK_SESSION),
        );
        let main = Served::new(403, "")
            .header("Server", "cloudflare")
            .header("CF-RAY", "abc-AMS");
        ctrl.emit(
            RESPONSE_EVENT,
            response_params("T1", DOCUMENT_TYPE, "https://shop.example/", &main),
            Some(MOCK_SESSION),
        );
        ctrl.emit(
            "Page.loadEventFired",
            json!({ "timestamp": 0.0 }),
            Some(MOCK_SESSION),
        );
        Ok(json!({ "loaderId": "L1" }))
    });

    let answer = goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");

    assert_eq!(answer.status, 403);
    assert_eq!(answer.url, "https://shop.example/");
    assert!(answer
        .headers
        .iter()
        .any(|(name, value)| name == "CF-RAY" && value == "abc-AMS"));
    assert!(!answer
        .headers
        .iter()
        .any(|(name, _)| name.starts_with("X-")));
    let methods: Vec<String> = mock.calls().into_iter().map(|call| call.method).collect();
    assert!(position(&methods, "Network.enable") < position(&methods, "Page.navigate"));
}

#[tokio::test]
async fn the_end_of_a_redirect_chain_is_the_answer() {
    let (page, mock) = mocked_page().await;
    announce(
        &mock,
        vec![
            ("T1", Served::new(301, "").header("Location", "/next")),
            ("T1", Served::new(200, "")),
        ],
    );

    let answer = goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");

    assert_eq!(answer.status, 200);
}

#[tokio::test]
async fn a_header_sent_twice_appears_twice() {
    let (page, mock) = mocked_page().await;
    announce(
        &mock,
        vec![("T1", Served::new(200, "").header("Set-Cookie", "a=1\nb=2"))],
    );

    let answer = goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");

    let cookies: Vec<&str> = answer
        .headers
        .iter()
        .filter(|(name, _)| name == "Set-Cookie")
        .map(|(_, value)| value.as_str())
        .collect();
    assert_eq!(cookies, ["a=1", "b=2"]);
}

#[tokio::test]
async fn events_of_another_tab_are_not_the_answer() {
    let (page, mock) = mocked_page().await;
    let ctrl = mock.clone();
    mock.on("Page.navigate", move |_| {
        ctrl.emit(
            RESPONSE_EVENT,
            response_params(
                "T1",
                DOCUMENT_TYPE,
                "https://other.example/",
                &Served::new(500, ""),
            ),
            Some("S9"),
        );
        ctrl.emit(
            "Page.loadEventFired",
            json!({ "timestamp": 0.0 }),
            Some(MOCK_SESSION),
        );
        Ok(json!({ "loaderId": "L1" }))
    });

    let answer = goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");

    assert_eq!(answer.status, 0, "no answer from this tab was seen");
}

#[tokio::test]
async fn a_navigation_without_an_answer_reports_status_zero_and_the_url_asked_for() {
    let (page, _mock) = mocked_page().await;

    let answer = goto_with_response(&page, "https://shop.example/a")
        .await
        .expect("the navigation finishes");

    assert_eq!(answer.status, 0);
    assert_eq!(answer.url, "https://shop.example/a");
    assert_eq!(answer.headers.len(), 0);
}

#[tokio::test]
async fn a_failed_navigation_names_the_url_redacted() {
    let (page, mock) = mocked_page().await;
    mock.reply(
        "Page.navigate",
        json!({ "errorText": "net::ERR_NAME_NOT_RESOLVED" }),
    );

    let error = goto_with_response(&page, "https://alice:s3cret@missing.example/?token=hunter2")
        .await
        .expect_err("the host does not resolve");

    assert!(
        matches!(&error, ScrapeError::Navigation { reason, .. } if reason == "net::ERR_NAME_NOT_RESOLVED"),
        "{error:?}"
    );
    let shown = format!("{error} {error:?}");
    for secret in ["alice", "s3cret", "hunter2"] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
}

#[tokio::test]
async fn a_url_the_browser_cannot_be_given_is_refused_without_being_echoed() {
    let (page, mock) = mocked_page().await;

    for raw in ["", "   ", "ualpha s3cret hunter2"] {
        let error = goto_with_response(&page, raw)
            .await
            .expect_err("the text is not a URL");
        let shown = format!("{error} {error:?}");
        assert!(
            matches!(&error, ScrapeError::Navigation { .. }),
            "{error:?}"
        );
        for secret in ["ualpha", "s3cret", "hunter2"] {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }
    }
    assert_eq!(
        mock.calls_to("Page.navigate").len(),
        0,
        "nothing was navigated"
    );
}

#[tokio::test]
async fn the_response_debug_hides_the_url_secrets_and_the_header_values() {
    let answer = NavResponse {
        url: "https://alice:s3cret@shop.example/?token=hunter2".to_owned(),
        status: 200,
        headers: vec![("Set-Cookie".to_owned(), "sid=abc123".to_owned())],
    };

    let shown = format!("{answer:?}");

    for secret in ["alice", "s3cret", "hunter2", "abc123"] {
        assert!(!shown.contains(secret), "{secret} in {shown}");
    }
    assert!(shown.contains("200"));
}

#[tokio::test]
async fn a_challenge_page_is_detected_from_its_status_headers_and_body() {
    let (page, mock) = mocked_page().await;
    Site::serve(
        &mock,
        vec![Served::new(503, CLOUDFLARE_JS)
            .header("Server", "cloudflare")
            .header("cf-ray", "8a1b2c3d4e5f6071-AMS")],
    );

    let answer = goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");
    let snapshot = detect_page(&page, &answer).await.expect("the page reads");

    assert_eq!(snapshot.detection.kind, ChallengeKind::JsChallenge);
    assert_eq!(snapshot.detection.evidence.vendor, Some(Vendor::Cloudflare));
    assert!(snapshot.body.contains("cloudflare") || snapshot.body.contains("challenge"));
}

#[tokio::test]
async fn a_normal_page_behind_a_vendor_is_not_a_challenge() {
    let (page, mock) = mocked_page().await;
    Site::serve(
        &mock,
        vec![Served::new(200, NORMAL_PAGE).header("cf-ray", "8a1b2c3d4e5f6071-AMS")],
    );

    let answer = goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");
    let snapshot = detect_page(&page, &answer).await.expect("the page reads");

    assert_eq!(snapshot.detection.kind, ChallengeKind::None);
}

#[tokio::test]
async fn an_egress_country_gives_its_locale_and_time_zone() {
    let placed = EgressLocale::resolve(&StaticGeoResolver, &egress(Some("DE")))
        .await
        .expect("Germany has a locale");

    assert_eq!(placed.locale.tag, "de-DE");
    assert_eq!(placed.timezone, "Europe/Berlin");
}

#[tokio::test]
async fn an_egress_without_a_country_or_a_locale_is_a_geo_error() {
    let without_country = EgressLocale::resolve(&StaticGeoResolver, &egress(None)).await;
    let without_locale = EgressLocale::resolve(&StaticGeoResolver, &egress(Some("ZZ"))).await;

    assert!(matches!(
        without_country,
        Err(ScrapeError::Geo(GeoError::Unknown(_)))
    ));
    assert!(matches!(
        without_locale,
        Err(ScrapeError::Geo(GeoError::NoLocale(_)))
    ));
}

/// A resolver that knows a time zone the locale table does not give for the country.
struct Vienna;

#[async_trait]
impl GeoResolver for Vienna {
    async fn resolve(&self, egress: &EgressRef) -> Result<GeoInfo, GeoError> {
        let key = egress.key();
        Ok(GeoInfo {
            country: key.country.ok_or_else(|| GeoError::Unknown(key.clone()))?,
            timezone: Some("Europe/Vienna".to_owned()),
            coordinates: None,
        })
    }
}

#[tokio::test]
async fn the_time_zone_a_resolver_reports_wins_over_the_locale_default() {
    let placed = EgressLocale::resolve(&Vienna, &egress(Some("DE")))
        .await
        .expect("Germany has a locale");
    let mut fingerprint = Fingerprint::randomized(OsType::Macos, 7);

    placed.apply_to(&mut fingerprint);

    assert_eq!(placed.timezone, "Europe/Vienna");
    assert_eq!(fingerprint.timezone.as_deref(), Some("Europe/Vienna"));
    assert_eq!(fingerprint.locale.as_deref(), Some("de-DE"));
    assert!(fingerprint
        .accept_languages
        .as_deref()
        .is_some_and(|languages| languages.starts_with("de-DE")));
}

#[tokio::test]
async fn the_locale_override_reaches_the_tab() {
    let (page, mock) = mocked_page().await;
    let placed = EgressLocale::resolve(&StaticGeoResolver, &egress(Some("FR")))
        .await
        .expect("France has a locale");

    apply_locale(&page, &placed)
        .await
        .expect("the overrides apply");

    let locale = mock.calls_to("Emulation.setLocaleOverride");
    let zone = mock.calls_to("Emulation.setTimezoneOverride");
    assert_eq!(locale.len(), 1);
    assert_eq!(locale[0].params, json!({ "locale": "fr-FR" }));
    assert_eq!(zone[0].params, json!({ "timezoneId": "Europe/Paris" }));
    assert_eq!(locale[0].session_id.as_deref(), Some(MOCK_SESSION));
}

#[tokio::test]
async fn a_refused_override_is_a_browser_error() {
    let (page, mock) = mocked_page().await;
    mock.fail("Emulation.setLocaleOverride", "no such locale");
    let placed = EgressLocale::resolve(&StaticGeoResolver, &egress(Some("DE")))
        .await
        .expect("Germany has a locale");

    let error = apply_locale(&page, &placed).await.expect_err("refused");

    assert!(matches!(error, ScrapeError::Browser(_)), "{error:?}");
}

fn script_cookies(mock: &MockCtrl, cookies: &[Value]) {
    mock.reply("Network.getAllCookies", json!({ "cookies": cookies }));
}

#[tokio::test]
async fn exported_cookies_keep_their_scope_and_drop_the_expired() {
    let (page, mock) = mocked_page().await;
    let live = f64::from(u32::try_from(NOW + 10_000).expect("fits"));
    let dead = f64::from(u32::try_from(NOW - 10).expect("fits"));
    script_cookies(
        &mock,
        &[
            cdp_cookie("cf_clearance", "tok", ".shop.example", live),
            cdp_cookie("sid", "abc", "shop.example", -1.0),
            cdp_cookie("old", "x", "shop.example", dead),
        ],
    );

    let jar = export_cookies(&page, NOW).await.expect("the cookies read");

    let names: Vec<&str> = jar.cookies().iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["cf_clearance", "sid"]);
    assert!(!jar.cookies()[0].host_only && jar.cookies()[1].host_only);
    assert_eq!(jar.cookies()[1].expires, None);
}

#[tokio::test]
async fn imported_cookies_are_set_in_the_browser_and_counted() {
    let (page, mock) = mocked_page().await;
    let live = f64::from(u32::try_from(NOW + 10_000).expect("fits"));
    let jar = CookieJar::from_cdp(
        &[
            Cookie::new("cf_clearance", "tok")
                .domain(".shop.example")
                .secure(true)
                .expires(live),
            Cookie::new("sid", "abc")
                .domain("shop.example")
                .secure(true),
        ],
        NOW,
    );

    let count = import_cookies(&page, &jar, NOW)
        .await
        .expect("they are set");

    assert_eq!(count, 2);
    let sent = mock.calls_to("Network.setCookies");
    assert_eq!(sent.len(), 1);
    let names: Vec<&str> = sent[0].params["cookies"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|cookie| cookie["name"].as_str())
        .collect();
    assert_eq!(names, ["cf_clearance", "sid"]);
}

#[tokio::test]
async fn an_empty_jar_sends_nothing() {
    let (page, mock) = mocked_page().await;

    let count = import_cookies(&page, &CookieJar::new(), NOW)
        .await
        .expect("nothing to do");

    assert_eq!(count, 0);
    assert_eq!(mock.calls_to("Network.setCookies").len(), 0);
}

#[tokio::test]
async fn the_clearance_value_is_the_vendors_cookie() {
    let (page, mock) = mocked_page().await;
    script_cookies(
        &mock,
        &[
            cdp_cookie("sid", "abc", "shop.example", -1.0),
            cdp_cookie("cf_clearance", "tok", ".shop.example", -1.0),
        ],
    );

    let cloudflare = clearance_value(&page, Some(Vendor::Cloudflare)).await;
    let datadome = clearance_value(&page, Some(Vendor::DataDome)).await;
    let kasada = clearance_value(&page, Some(Vendor::Kasada)).await;
    let unknown = clearance_value(&page, None).await;

    assert_eq!(cloudflare.expect("reads").as_deref(), Some("tok"));
    assert_eq!(datadome.expect("reads"), None);
    assert_eq!(
        kasada.expect("reads"),
        None,
        "Kasada has no clearance cookie"
    );
    assert_eq!(unknown.expect("reads"), None);
}

#[tokio::test(start_paused = true)]
async fn a_clearance_cookie_that_appears_ends_the_wait() {
    let (page, mock) = mocked_page().await;
    let polls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&polls);
    mock.on("Network.getAllCookies", move |_| {
        let cookies = if seen.fetch_add(1, Ordering::SeqCst) < 2 {
            vec![]
        } else {
            vec![cdp_cookie("cf_clearance", "fresh", ".shop.example", -1.0)]
        };
        Ok(json!({ "cookies": cookies }))
    });

    let cleared = wait_for_clearance(
        &page,
        Some(Vendor::Cloudflare),
        None,
        Duration::from_secs(30),
    )
    .await
    .expect("the cookies read");

    assert!(cleared);
    assert_eq!(polls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn the_cookie_from_before_the_solve_does_not_count_while_the_challenge_shows() {
    let (page, mock) = mocked_page().await;
    Site::serve(&mock, vec![Served::new(200, CLOUDFLARE_JS)]);
    // The cookie mock goes in after the site, which answers the cookies on its own.
    script_cookies(
        &mock,
        &[cdp_cookie("cf_clearance", "old", ".shop.example", -1.0)],
    );
    goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");

    let cleared = wait_for_clearance(&page, Some(Vendor::Cloudflare), Some("old"), BRIEF)
        .await
        .expect("the cookies read");

    assert!(!cleared, "a stale clearance is not a solved challenge");
}

#[tokio::test]
async fn an_unchanged_cookie_counts_once_the_page_is_no_longer_a_challenge() {
    let (page, mock) = mocked_page().await;
    Site::serve(&mock, vec![Served::new(200, NORMAL_PAGE)]);
    script_cookies(
        &mock,
        &[cdp_cookie(
            "cf_clearance",
            "set-by-the-script",
            ".shop.example",
            -1.0,
        )],
    );
    goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");

    let cleared = wait_for_clearance(
        &page,
        Some(Vendor::Cloudflare),
        Some("set-by-the-script"),
        BRIEF,
    )
    .await
    .expect("the cookies read");

    assert!(
        cleared,
        "the script set the cookie before the stale value was read, and the page is real"
    );
}

#[tokio::test]
async fn a_changed_cookie_value_counts_even_when_one_was_there() {
    let (page, mock) = mocked_page().await;
    script_cookies(
        &mock,
        &[cdp_cookie("cf_clearance", "new", ".shop.example", -1.0)],
    );

    let cleared = wait_for_clearance(&page, Some(Vendor::Cloudflare), Some("old"), BRIEF)
        .await
        .expect("the cookies read");

    assert!(cleared);
}

#[tokio::test]
async fn a_vendor_without_a_cookie_is_cleared_when_the_page_stops_being_a_challenge() {
    let (page, mock) = mocked_page().await;
    Site::serve(
        &mock,
        vec![
            Served::new(200, CLOUDFLARE_JS),
            Served::new(200, NORMAL_PAGE),
        ],
    );
    goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");

    let still_challenged = wait_for_clearance(&page, Some(Vendor::Kasada), None, BRIEF)
        .await
        .expect("the page reads");
    goto_with_response(&page, "https://shop.example/")
        .await
        .expect("the navigation finishes");
    let cleared = wait_for_clearance(&page, Some(Vendor::Kasada), None, BRIEF)
        .await
        .expect("the page reads");

    assert!(!still_challenged);
    assert!(cleared);
}

#[tokio::test]
async fn a_browser_failure_during_the_wait_is_an_error() {
    let (page, mock) = mocked_page().await;
    mock.fail("Network.getAllCookies", "target closed");

    let error = wait_for_clearance(&page, Some(Vendor::Cloudflare), None, BRIEF)
        .await
        .expect_err("the browser failed");

    assert!(matches!(error, ScrapeError::Browser(_)), "{error:?}");
}

#[tokio::test]
async fn a_session_is_captured_with_its_cookies_and_local_storage() {
    let (page, mock) = mocked_page().await;
    let site = Site::serve(&mock, vec![Served::new(200, NORMAL_PAGE)]);
    site.set_local_storage(json!({ "theme": "dark" }));
    script_cookies(&mock, &[cdp_cookie("sid", "abc", "shop.example", -1.0)]);
    goto_with_response(&page, "https://shop.example/cart")
        .await
        .expect("the navigation finishes");

    let session = capture_session(&page).await.expect("the session reads");

    assert_eq!(session.cookies().len(), 1);
    assert_eq!(session.origin(), Some("https://shop.example"));
    assert_eq!(
        session.local_storage().get("theme").map(String::as_str),
        Some("dark")
    );
    assert!(!format!("{session:?}").contains("abc"));
}

#[tokio::test]
async fn a_restored_session_skips_expired_cookies_and_foreign_storage() {
    let (page, mock) = mocked_page().await;
    let site = Site::serve(&mock, vec![Served::new(200, NORMAL_PAGE)]);
    goto_with_response(&page, "https://other.example/")
        .await
        .expect("the navigation finishes");
    let session = Session::new(vec![
        Cookie::new("live", "1").domain("shop.example").expires(2e9),
        Cookie::new("dead", "2").domain("shop.example").expires(1.0),
    ])
    .with_local_storage(
        "https://shop.example",
        std::collections::BTreeMap::from([("k".to_owned(), "v".to_owned())]),
    );

    restore_session(&page, session, 1.8e9)
        .await
        .expect("the session is restored");

    let sent = mock.calls_to("Network.setCookies");
    let names: Vec<&str> = sent[0].params["cookies"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|cookie| cookie["name"].as_str())
        .collect();
    assert_eq!(names, ["live"]);
    let wrote_storage = mock.calls_to("Runtime.evaluate").iter().any(|call| {
        call.params["expression"]
            .as_str()
            .is_some_and(|expression| expression.contains("setItem"))
    });
    assert!(!wrote_storage, "the tab is on another origin");
    assert_eq!(site.navigations().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn the_warm_up_scrolls_the_page_and_never_fails() {
    let (page, mock) = mocked_page().await;
    Site::serve(&mock, vec![Served::new(200, NORMAL_PAGE)]);

    human_warm_up(&page, 11).await;

    assert_ne!(mock.calls_to("Input.dispatchMouseEvent").len(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_warm_up_the_browser_refuses_is_only_logged() {
    let (page, mock) = mocked_page().await;
    mock.reply(
        "Runtime.evaluate",
        json!({ "result": { "value": [1280, 720] } }),
    );
    mock.fail("Input.dispatchMouseEvent", "no input");

    human_warm_up(&page, 11).await;

    assert_ne!(mock.calls_to("Input.dispatchMouseEvent").len(), 0);
}
