//! `HttpSession` continued from a page of a real Chrome, against the loopback TLS site.
//!
//! The mocked tests of `impersonate_http` script what Chrome would answer; these check it. They
//! show that the real `navigator.userAgent`, `navigator.languages`, `Browser.getVersion` and
//! `Network.getAllCookies` are what `HttpSession::from_page` reads, that a host-only cookie set by
//! a response reaches Chrome as host-only, that a removal reaches it too, and that a page that
//! announces itself as headless is refused, and that a browser started with a browser-wide proxy,
//! launched here or attached to by its debugging port, is not taken for a direct one: its command
//! line names the proxy. A Chrome that has updated past the measured major
//! fails the first test with `VersionMismatch`, which is the drift check the plan asks for: run
//! the capture tools again.
//!
//! They need Chrome, so they are ignored by default. Only one Chrome-driving test binary may run
//! on the machine at a time. Run them with:
//!
//! ```text
//! cargo test --features impersonate --test impersonate_http_chrome -- --ignored --test-threads=1
//! ```
#![cfg(feature = "impersonate")]

use std::time::Duration;

use seleniumbase_rs::impersonate::client::ImpersonationProfile;
use seleniumbase_rs::impersonate::http_session::{HttpError, HttpOptions, HttpSession};
use seleniumbase_rs::sb_cdp::{Browser, LaunchOptions, Page};
use seleniumbase_rs::scrape::egress::route::EgressRef;
use seleniumbase_rs::wire::BrowserKind;
use serde_json::json;

#[allow(
    dead_code,
    reason = "the site is shared with impersonate_http, which uses more of it"
)]
#[path = "impersonate_http/site.rs"]
mod site;

use site::{Answer, Site};

const HOST: &str = "shop.test";
/// The languages the page is told to report, as Chrome takes them: a plain comma list.
const LANGUAGES: &str = "de-DE,de";
/// The `Accept-Language` Chrome builds from them, and the session must send.
const ACCEPT_LANGUAGE: &str = "de-DE,de;q=0.9";

async fn open() -> (Browser, Page) {
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .build()
        .expect("launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser
        .default_page()
        .await
        .expect("a tab")
        .with_timeout(Duration::from_secs(10));
    (browser, page)
}

/// Makes the page report the user agent of the measured Chrome instead of the headless one.
async fn announce_as_measured(page: &Page) -> String {
    let agent = ImpersonationProfile::measured(BrowserKind::Chrome)
        .expect("measured")
        .user_agent()
        .to_owned();
    page.execute(
        "Emulation.setUserAgentOverride",
        json!({ "userAgent": agent, "acceptLanguage": LANGUAGES, "platform": "MacIntel" }),
    )
    .await
    .expect("the override");
    agent
}

/// Stores host-only cookies for `HOST` in the browser, given by URL as a page would have them.
async fn hold(page: &Page, pairs: &[(&str, &str)]) {
    let cookies: Vec<_> = pairs
        .iter()
        .map(|(name, value)| {
            json!({
                "name": name, "value": value, "url": format!("https://{HOST}/"),
                "secure": true,
            })
        })
        .collect();
    page.execute("Network.setCookies", json!({ "cookies": cookies }))
        .await
        .expect("the cookies");
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn the_cookies_of_a_real_page_go_out_and_what_the_site_sets_comes_back_in() {
    let (browser, page) = open().await;
    let agent = announce_as_measured(&page).await;
    hold(&page, &[("old", "O"), ("untouched", "U")]).await;
    let site = Site::start(|seen| match seen.path() {
        "/update" => Answer::ok("")
            .cookie("fresh=1; Max-Age=3600; Secure; HttpOnly")
            .cookie("wide=2; Domain=shop.test; Path=/; Secure")
            .cookie("old=; Max-Age=0"),
        _ => Answer::ok(""),
    })
    .await;

    let session = HttpSession::from_page(&page, None, site.options(&[HOST]))
        .await
        .expect("a session from the real page");
    session
        .get(&site.url(HOST, "/update"))
        .await
        .expect("a reply");
    let written = session.sync_cookies_back(&page).await.expect("synced");
    let held = page.cookies().all().await.expect("the cookies");
    browser.close().await.expect("Chrome closes");

    let sent = &site.seen()[0];
    let cookie = sent.cookie().expect("the page's cookies were sent");
    assert!(
        cookie.contains("old=O") && cookie.contains("untouched=U"),
        "{cookie}"
    );
    assert_eq!(sent.header("user-agent"), Some(agent.as_str()));
    assert_eq!(sent.header("accept-language"), Some(ACCEPT_LANGUAGE));
    assert_eq!(written, 3, "two stored, one removed");
    let find = |name: &str| held.iter().find(|c| c.name == name);
    let fresh = find("fresh").expect("the new cookie reached Chrome");
    assert_eq!(fresh.value, "1");
    assert_eq!(
        fresh.domain, HOST,
        "a host-only cookie stays host-only: no leading dot"
    );
    assert!(fresh.secure && fresh.http_only && fresh.expires.is_some());
    let wide = find("wide").expect("the domain cookie reached Chrome");
    assert_eq!(wide.domain, format!(".{HOST}"));
    assert!(find("old").is_none(), "the removal reached Chrome");
    assert_eq!(find("untouched").map(|c| c.value.as_str()), Some("U"));
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_browser_started_with_a_proxy_is_not_taken_for_a_direct_one_launched_or_attached() {
    // Nothing listens on the discard port; the page never loads anything.
    let proxy = "http://127.0.0.1:9";
    let options = LaunchOptions::builder()
        .headless(true)
        .no_sandbox(std::env::var_os("CI").is_some())
        .proxy(proxy)
        .build()
        .expect("launch options");
    let browser = Browser::launch(options).await.expect("Chrome starts");
    let page = browser
        .default_page()
        .await
        .expect("a tab")
        .with_timeout(Duration::from_secs(10));
    announce_as_measured(&page).await;
    let attached = Browser::connect("127.0.0.1", browser.debugging_port())
        .await
        .expect("attaches to the running Chrome");
    let tab = attached
        .default_page()
        .await
        .expect("a tab")
        .with_timeout(Duration::from_secs(10));

    let launched_none = HttpSession::from_page(&page, None, HttpOptions::new()).await;
    let attached_none = HttpSession::from_page(&tab, None, HttpOptions::new()).await;
    let real = EgressRef::parse(proxy, None).expect("an egress");
    let named = HttpSession::from_page(&page, Some(real.clone()), HttpOptions::new()).await;
    let direct = HttpSession::from_page(
        &page,
        None,
        HttpOptions::new().browser_proxy_exits_directly(true),
    )
    .await;
    browser.close().await.expect("Chrome closes");

    for refused in [launched_none, attached_none] {
        let error = refused.expect_err("the browser has a proxy");
        assert!(
            matches!(&error, HttpError::BrowserProxy { proxy: shown } if shown.contains("127.0.0.1:9")),
            "{error}"
        );
    }
    assert_eq!(named.expect("egress named").egress(), Some(real.key()));
    assert_eq!(direct.expect("said to be direct").egress(), None);
}

#[tokio::test]
#[ignore = "launches a real Chrome"]
async fn a_page_that_announces_itself_as_headless_is_refused() {
    let (browser, page) = open().await;

    let error = HttpSession::from_page(&page, None, HttpOptions::new())
        .await
        .expect_err("the user agent says HeadlessChrome");
    browser.close().await.expect("Chrome closes");

    assert!(matches!(error, HttpError::Impersonate(_)), "{error}");
}
