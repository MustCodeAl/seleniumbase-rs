//! The page leg against the scripted browser of `sb_cdp`: no Chrome, no network.
//!
//! The responses that change the jar are played into it directly (`SessionJar::absorb`), so what
//! is asserted is what the session sends the browser. The end-to-end version, with a real TLS
//! site answering, is `tests/impersonate_http/page.rs`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use url::Url;

use super::*;
use crate::sb_cdp::{Browser, MockCtrl};
use crate::scrape::runtime::egress_switch::{ContextProxySwitch, EgressSwitch};

const HOST: &str = "shop.test";

fn measured_agent() -> String {
    ImpersonationProfile::measured(BrowserKind::Chrome)
        .expect("measured")
        .user_agent()
        .to_owned()
}

fn product() -> String {
    let major = ImpersonationProfile::measured(BrowserKind::Chrome)
        .expect("measured")
        .major();
    format!("Chrome/{major}.0.0.0")
}

fn cdp_cookie(name: &str, value: &str, domain: &str) -> Value {
    json!({
        "name": name, "value": value, "domain": domain, "path": "/",
        "expires": -1.0, "httpOnly": false, "secure": true, "sameSite": "Lax",
    })
}

/// A mocked browser whose page reports `agent`, `languages` and `product` and holds `cookies`.
async fn scripted(
    agent: &str,
    languages: &[&str],
    product: &str,
    cookies: Vec<Value>,
) -> (Browser, Page, MockCtrl) {
    let (browser, mock) = Browser::new_mocked();
    let report = json!({ "userAgent": agent, "languages": languages });
    mock.on("Runtime.evaluate", move |_| {
        Ok(json!({ "result": { "value": report.clone() } }))
    });
    mock.reply("Browser.getVersion", json!({ "product": product }));
    mock.reply("Network.getAllCookies", json!({ "cookies": cookies }));
    let page = browser.default_page().await.expect("a tab");
    (browser, page, mock)
}

fn at(path: &str) -> Url {
    Url::parse(&format!("https://{HOST}{path}")).expect("a url")
}

#[tokio::test]
async fn a_page_hands_over_its_cookies_its_agent_and_its_languages() {
    let agent = measured_agent();
    let (_browser, page, _mock) = scripted(
        &agent,
        &["de-DE", "de"],
        &product(),
        vec![
            cdp_cookie("sid", "PAGE", HOST),
            cdp_cookie("wide", "W", &format!(".{HOST}")),
        ],
    )
    .await;

    let session = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect("a session");

    assert_eq!(session.profile().user_agent(), agent);
    assert_eq!(session.profile().accept_language(), Some("de-DE,de;q=0.9"));
    assert_eq!(session.cookie_count(), 2);
    assert!(session.egress().is_none());
}

#[tokio::test]
async fn a_page_of_another_chrome_is_refused_whoever_reports_the_version() {
    let measured = ImpersonationProfile::measured(BrowserKind::Chrome)
        .expect("measured")
        .major();
    let newer = measured + 1;
    let agent = measured_agent().replace(&measured.to_string(), &newer.to_string());
    let (_b1, by_agent, _m1) = scripted(&agent, &[], &product(), Vec::new()).await;
    let (_b2, by_product, _m2) = scripted(
        &measured_agent(),
        &[],
        &format!("Chrome/{newer}.0.0.0"),
        Vec::new(),
    )
    .await;

    for page in [&by_agent, &by_product] {
        let error = HttpSession::from_page(page, None, HttpOptions::new())
            .await
            .expect_err("refused");
        assert!(
            matches!(error, HttpError::VersionMismatch { browser, measured: m } if browser == newer && m == measured),
            "{error}"
        );
    }
}

#[tokio::test]
async fn a_page_that_reports_nothing_is_a_browser_error() {
    let (browser, _mock) = Browser::new_mocked();
    let page = browser.default_page().await.expect("a tab");

    let error = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect_err("no agent");

    assert!(matches!(error, HttpError::Browser(_)), "{error}");
}

#[tokio::test]
async fn sync_back_writes_what_responses_set_and_removed_and_nothing_else() {
    let (_browser, page, mock) = scripted(
        &measured_agent(),
        &[],
        &product(),
        vec![
            cdp_cookie("old", "O", HOST),
            cdp_cookie("untouched", "U", HOST),
        ],
    )
    .await;
    let session = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect("a session");
    session.inner().jar.absorb(
        &at("/update"),
        &[
            "fresh=1; Max-Age=3600; Secure; HttpOnly",
            "old=; Max-Age=0",
            "wide=2; Domain=shop.test; Path=/",
        ],
        session.now(),
    );

    let written = session.sync_cookies_back(&page).await.expect("synced");
    let again = session.sync_cookies_back(&page).await.expect("synced");

    assert_eq!((written, again), (3, 0));
    let stored = mock.calls_to("Network.setCookies");
    assert_eq!(stored.len(), 1);
    let cookies = stored[0].params["cookies"].as_array().expect("a list");
    let names: Vec<&str> = cookies.iter().filter_map(|c| c["name"].as_str()).collect();
    assert_eq!(names, ["fresh", "wide"]);
    assert_eq!(cookies[0]["url"], format!("https://{HOST}/"));
    assert_eq!(cookies[1]["domain"], format!(".{HOST}"));
    let removed = mock.calls_to("Network.deleteCookies");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].params["name"], "old");
    assert_eq!(removed[0].params["domain"], HOST);
    assert!(!stored[0].params.to_string().contains("untouched"));
}

#[tokio::test]
async fn a_failed_sync_keeps_the_changes_for_the_next_attempt() {
    let (_browser, page, mock) = scripted(&measured_agent(), &[], &product(), Vec::new()).await;
    let session = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect("a session");
    session
        .inner()
        .jar
        .absorb(&at("/"), &["fresh=1"], session.now());

    mock.fail("Network.setCookies", "no room");
    let failed = session.sync_cookies_back(&page).await.expect_err("refused");
    mock.reply("Network.setCookies", json!({}));
    let retried = session.sync_cookies_back(&page).await.expect("synced");

    assert!(matches!(failed, HttpError::Browser(_)), "{failed}");
    assert_eq!(retried, 1);
}

#[tokio::test]
async fn a_failed_removal_is_kept_too() {
    let (_browser, page, mock) = scripted(
        &measured_agent(),
        &[],
        &product(),
        vec![cdp_cookie("old", "O", HOST)],
    )
    .await;
    let session = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect("a session");
    session
        .inner()
        .jar
        .absorb(&at("/"), &["old=; Max-Age=0"], session.now());

    mock.fail("Network.deleteCookies", "gone already");
    let failed = session.sync_cookies_back(&page).await.expect_err("refused");
    mock.reply("Network.deleteCookies", json!({}));
    let retried = session.sync_cookies_back(&page).await.expect("synced");

    assert!(matches!(failed, HttpError::Browser(_)), "{failed}");
    assert_eq!(retried, 1);
}

#[tokio::test]
async fn the_proxy_of_the_pages_context_is_taken_or_must_match() {
    let (browser, mock) = Browser::new_mocked();
    let made = Arc::new(AtomicUsize::new(0));
    mock.on("Target.createBrowserContext", move |_| {
        let id = made.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(json!({ "browserContextId": format!("C{id}") }))
    });
    mock.reply(
        "Target.getTargetInfo",
        json!({ "targetInfo": { "browserContextId": "C1" } }),
    );
    let report = json!({ "userAgent": measured_agent(), "languages": [] });
    mock.on("Runtime.evaluate", move |_| {
        Ok(json!({ "result": { "value": report.clone() } }))
    });
    let proxy = EgressRef::parse("http://alice:s3cret@proxy.example:8080", None).expect("a spec");
    let page = ContextProxySwitch::new()
        .switch(&browser, Some(&proxy))
        .await
        .expect("a tab behind the proxy");

    let adopted = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect("a session");
    let other = EgressRef::parse("http://elsewhere.example:8080", None).expect("a spec");
    let refused = HttpSession::from_page(&page, Some(other), HttpOptions::new())
        .await
        .expect_err("mismatch");

    assert_eq!(adopted.egress(), Some(proxy.key()));
    assert!(matches!(
        refused,
        HttpError::Identity(IdentityError::EgressMismatch { .. })
    ));
    let shown = format!("{adopted:?} {refused} {refused:?}");
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
}
