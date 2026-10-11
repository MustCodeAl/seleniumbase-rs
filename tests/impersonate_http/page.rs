//! A session continued from a page, and the cookies written back, against a scripted browser.
//!
//! The browser is the mock of `sb_cdp` (`Browser::new_mocked`), so these tests need no Chrome and
//! see exactly which protocol commands the session sends. The HTTP side is the loopback site.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use seleniumbase_rs::impersonate::client::ImpersonationProfile;
use seleniumbase_rs::impersonate::http_session::{HttpError, HttpOptions, HttpSession};
use seleniumbase_rs::sb_cdp::{Browser, MockCtrl, Page};
use seleniumbase_rs::scrape::passport::identity::IdentityError;
use seleniumbase_rs::scrape::runtime::egress_switch::{ContextProxySwitch, EgressSwitch};
use seleniumbase_rs::wire::BrowserKind;
use serde_json::{json, Value};

use crate::site::{egress, measured_major, Answer, Site};

const HOST: &str = "shop.test";

/// The user agent of the measured Chrome.
fn measured_agent() -> String {
    ImpersonationProfile::measured(BrowserKind::Chrome)
        .expect("measured")
        .user_agent()
        .to_owned()
}

/// A mocked browser whose page reports `agent`, `languages` and `product`, and holds `cookies`.
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

fn cdp_cookie(name: &str, value: &str, domain: &str) -> Value {
    json!({
        "name": name, "value": value, "domain": domain, "path": "/",
        "expires": -1.0, "httpOnly": false, "secure": true, "sameSite": "Lax",
    })
}

fn product() -> String {
    format!("Chrome/{}.0.0.0", measured_major())
}

#[tokio::test]
async fn a_page_hands_over_its_cookies_its_agent_and_its_languages() {
    let site = Site::start(|_| Answer::ok("")).await;
    let agent = measured_agent();
    let (_browser, page, _mock) = scripted(
        &agent,
        &["de-DE", "de", "en"],
        &product(),
        vec![
            cdp_cookie("sid", "PAGE", HOST),
            cdp_cookie("wide", "W", &format!(".{HOST}")),
        ],
    )
    .await;

    let session = HttpSession::from_page(&page, None, site.options(&[HOST]))
        .await
        .expect("a session");
    session.get(&site.url(HOST, "/")).await.expect("a reply");

    let seen = &site.seen()[0];
    assert_eq!(seen.header("user-agent"), Some(agent.as_str()));
    assert_eq!(
        seen.header("accept-language"),
        Some("de-DE,de;q=0.9,en;q=0.8")
    );
    let sent = seen.cookie().expect("the page's cookies");
    assert!(
        sent.contains("sid=PAGE") && sent.contains("wide=W"),
        "{sent}"
    );
    assert_eq!(session.cookie_count(), 2);
}

#[tokio::test]
async fn a_page_of_another_chrome_is_refused_whoever_reports_the_version() {
    let site = Site::start(|_| Answer::ok("")).await;
    let measured = measured_major();
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
        let error = HttpSession::from_page(page, None, site.options(&[HOST]))
            .await
            .expect_err("refused");
        assert!(
            matches!(error, HttpError::VersionMismatch { browser, measured: m } if browser == newer && m == measured),
            "{error}"
        );
    }
}

#[tokio::test]
async fn a_page_that_is_not_a_measured_system_is_refused() {
    let site = Site::start(|_| Answer::ok("")).await;
    let windows = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";
    let (_browser, page, _mock) = scripted(windows, &[], &product(), Vec::new()).await;

    let error = HttpSession::from_page(&page, None, site.options(&[HOST]))
        .await
        .expect_err("refused");

    assert!(matches!(error, HttpError::Impersonate(_)), "{error}");
}

#[tokio::test]
async fn sync_back_writes_what_responses_set_and_removed_and_nothing_else() {
    let site = Site::start(|seen| match seen.path() {
        "/update" => Answer::ok("")
            .cookie("fresh=1; Max-Age=3600; Secure; HttpOnly")
            .cookie("old=; Max-Age=0")
            .cookie("wide=2; Domain=shop.test; Path=/"),
        _ => Answer::ok(""),
    })
    .await;
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
    let session = HttpSession::from_page(&page, None, site.options(&[HOST]))
        .await
        .expect("a session");

    session
        .get(&site.url(HOST, "/update"))
        .await
        .expect("a reply");
    let written = session.sync_cookies_back(&page).await.expect("synced");
    let again = session.sync_cookies_back(&page).await.expect("synced");

    assert_eq!(written, 3, "two stored and one removed");
    assert_eq!(again, 0);
    let stored = mock.calls_to("Network.setCookies");
    assert_eq!(stored.len(), 1);
    let cookies = stored[0].params["cookies"].as_array().expect("a list");
    let names: Vec<&str> = cookies.iter().filter_map(|c| c["name"].as_str()).collect();
    assert_eq!(names, ["fresh", "wide"]);
    assert_eq!(
        cookies[0]["url"],
        format!("https://{HOST}/"),
        "a host cookie is given by URL"
    );
    assert!(cookies[0].get("domain").is_none());
    assert_eq!(cookies[0]["httpOnly"], true);
    assert_eq!(cookies[1]["domain"], format!(".{HOST}"));
    let removed = mock.calls_to("Network.deleteCookies");
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].params["name"], "old");
    assert_eq!(removed[0].params["domain"], HOST);
    assert!(
        !stored[0].params.to_string().contains("untouched"),
        "what the page holds and the session did not change is left alone"
    );
}

#[tokio::test]
async fn a_failed_sync_keeps_the_changes_for_the_next_attempt() {
    let site = Site::start(|_| Answer::ok("").cookie("fresh=1")).await;
    let (_browser, page, mock) = scripted(&measured_agent(), &[], &product(), Vec::new()).await;
    let session = HttpSession::from_page(&page, None, site.options(&[HOST]))
        .await
        .expect("a session");
    session.get(&site.url(HOST, "/")).await.expect("a reply");

    mock.fail("Network.setCookies", "no room");
    let failed = session.sync_cookies_back(&page).await.expect_err("refused");
    mock.reply("Network.setCookies", json!({}));
    let retried = session.sync_cookies_back(&page).await.expect("synced");

    assert!(matches!(failed, HttpError::Browser(_)), "{failed}");
    assert_eq!(retried, 1);
}

#[tokio::test]
async fn the_proxy_of_the_pages_context_is_taken_when_no_egress_is_given() {
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
    let proxy = egress("http://alice:s3cret@proxy.example:8080");
    let page = ContextProxySwitch::new()
        .switch(&browser, Some(&proxy))
        .await
        .expect("a tab behind the proxy");

    let adopted = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect("a session");
    let same = HttpSession::from_page(&page, Some(proxy.clone()), HttpOptions::new())
        .await
        .expect("a session");
    let other = egress("http://elsewhere.example:8080");
    let refused = HttpSession::from_page(&page, Some(other), HttpOptions::new())
        .await
        .expect_err("mismatch");

    assert_eq!(
        adopted.egress().map(|key| key.host.as_str()),
        Some("proxy.example")
    );
    assert_eq!(same.egress(), Some(proxy.key()));
    assert!(
        matches!(
            refused,
            HttpError::Identity(IdentityError::EgressMismatch { .. })
        ),
        "{refused}"
    );
    let shown = format!("{adopted:?} {refused} {refused:?}");
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
}

#[tokio::test]
async fn a_page_that_reports_no_user_agent_is_a_browser_error() {
    let (browser, _mock) = Browser::new_mocked();
    let page = browser.default_page().await.expect("a tab");

    let error = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect_err("no agent");

    assert!(matches!(error, HttpError::Browser(_)), "{error}");
}

/// The command line of a Chrome, as `SystemInfo.getInfo` reports it, up to its proxy switches.
const CHROME_LINE: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome \
     --remote-debugging-port=0 --user-data-dir=/tmp/profile";

/// A mocked browser, measured Chrome, whose process was started with `switches` after
/// [`CHROME_LINE`].
async fn started_with(switches: &str) -> (Browser, Page, MockCtrl) {
    let (browser, page, mock) = scripted(&measured_agent(), &[], &product(), Vec::new()).await;
    mock.reply(
        "SystemInfo.getInfo",
        json!({ "commandLine": format!("{CHROME_LINE} {switches}"), "gpu": {} }),
    );
    (browser, page, mock)
}

#[tokio::test]
async fn a_browser_with_a_browser_wide_proxy_is_not_taken_for_a_direct_one() {
    for proxy in [
        "http://127.0.0.1:43210",
        "http://proxy.example:8080",
        "socks5://proxy.example:1080",
        "proxy.example:8080",
        "http=proxy.example:8080;https=proxy.example:8080",
    ] {
        let (_browser, page, _mock) =
            started_with(&format!("--proxy-server={proxy} --disable-quic")).await;

        let error = HttpSession::from_page(&page, None, HttpOptions::new())
            .await
            .expect_err(proxy);

        assert!(
            matches!(error, HttpError::BrowserProxy { .. }),
            "{proxy}: {error}"
        );
        let first = proxy.split(';').next().unwrap_or(proxy);
        let first = first.split_once('=').map_or(first, |(_, address)| address);
        assert!(error.to_string().contains(first), "{first} in {error}");
    }
}

#[tokio::test]
async fn the_egress_a_loopback_proxy_really_leaves_through_is_what_the_session_takes() {
    let site = Site::start(|_| Answer::ok("")).await;
    let real = egress("http://alice:s3cret@resi.example:8080");
    let (_browser, page, _mock) = started_with("--proxy-server=http://127.0.0.1:43210").await;

    let session = HttpSession::from_page(&page, Some(real.clone()), site.options(&[HOST]))
        .await
        .expect("the caller named the real egress");

    assert_eq!(session.egress(), Some(real.key()));
    let shown = format!("{session:?}");
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
}

#[tokio::test]
async fn an_egress_that_is_not_the_browsers_own_proxy_is_refused() {
    let (_browser, page, _mock) = started_with("--proxy-server=http://proxy.example:8080").await;
    let same = egress("http://alice:s3cret@proxy.example:8080");
    let other = egress("http://alice:s3cret@elsewhere.example:8080");

    let accepted = HttpSession::from_page(&page, Some(same.clone()), HttpOptions::new())
        .await
        .expect("its own proxy");
    let refused = HttpSession::from_page(&page, Some(other), HttpOptions::new())
        .await
        .expect_err("another proxy");

    assert_eq!(accepted.egress(), Some(same.key()));
    assert!(
        matches!(
            refused,
            HttpError::Identity(IdentityError::EgressMismatch { .. })
        ),
        "{refused}"
    );
    let shown = format!("{refused} {refused:?}");
    assert!(
        !shown.contains("s3cret") && !shown.contains("alice"),
        "{shown}"
    );
}

#[tokio::test]
async fn a_proxy_that_leaves_directly_can_be_said_to_do_so() {
    let (_browser, page, _mock) = started_with("--proxy-server=http://127.0.0.1:43210").await;

    let session = HttpSession::from_page(
        &page,
        None,
        HttpOptions::new().browser_proxy_exits_directly(true),
    )
    .await
    .expect("said to leave directly");

    assert_eq!(session.egress(), None);
}

#[tokio::test]
async fn a_browser_with_no_proxy_switch_or_a_direct_one_is_a_direct_browser() {
    for switches in [
        "",
        "--no-proxy-server",
        "--proxy-server=direct://",
        "--proxy-bypass-list=<-loopback>",
    ] {
        let (_browser, page, _mock) = started_with(switches).await;

        let session = HttpSession::from_page(&page, None, HttpOptions::new())
            .await
            .expect(switches);

        assert_eq!(session.egress(), None, "{switches}");
    }
}

#[tokio::test]
async fn a_browser_that_will_not_say_how_it_was_started_is_taken_as_before() {
    let (_browser, page, _mock) = scripted(&measured_agent(), &[], &product(), Vec::new()).await;

    let session = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect("nothing to go by");

    assert_eq!(session.egress(), None);
}
